"""Exercise the actual trial worker in isolated terminals, then approve only
after checking captured pixels. No terminal responses are synthesized.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from PIL import Image


def driver(action, capture=None):
    env = dict(os.environ)
    if capture:
        env["TERMIMOCHI_INSPECT_SCREENSHOT"] = str(capture)
    subprocess.run(["python3", str(Path(__file__).with_name("preview-pointer-driver.py")),
                    action, "0", "0"], env=env, check=True, timeout=8, stdout=subprocess.DEVNULL)


def check_picture(path, animated):
    pixels = Image.open(path).convert("RGB")
    color = (200, 30, 70) if animated else (220, 45, 70)
    points = [(x, y) for y in range(pixels.height) for x in range(pixels.width)
              if max(abs(a-b) for a, b in zip(pixels.getpixel((x, y)), color)) <= 5]
    assert len(points) > 100, f"Artwork missing: {path}, {len(points)} pixels"
    left, right = min(x for x, _ in points), max(x for x, _ in points)
    top, bottom = min(y for _, y in points), max(y for _, y in points)
    assert bottom < pixels.height - 30, "Artwork clipped below the terminal"
    if animated:
        assert right - left < 140, "Previous animated frames left trails"
    return (left, right, top, bottom)


def terminal_command(name, helper):
    title = "TermiMochi point-to-edit test"
    if name == "kitty":
        return ["/usr/bin/kitty", "--config", "NONE", "-o", "linux_display_server=x11",
                "-o", "shell_integration=disabled", "-o", "remember_window_size=no",
                "-o", "background=#20252B", "-o", "font_size=12",
                "-o", "initial_window_width=100c", "-o", "initial_window_height=36c",
                "--title", title] + helper
    if name == "xterm":
        return [os.environ.get("TERMIMOCHI_XTERM", "xterm"), "-ti", "vt340",
                "-xrm", "XTerm*decGraphicsID: 340", "-xrm", "XTerm*allowWindowOps: true",
                "-xrm", "XTerm*sixelScrolling: true", "-bg", "#20252B", "-fg", "white",
                "-fa", "monospace", "-fs", "12", "-geometry", "100x36",
                "-title", title, "-e"] + helper
    return ["/usr/bin/ptyxis", "--standalone", "--new-window", f"--title={title}", "--"] + helper


def main(folder, terminal, mode):
    assert os.environ.get("DISPLAY") not in (None, ":0", ":1"), "Dedicated Xvfb only"
    out = Path(os.environ["XDG_CACHE_HOME"]) / "pixel-trial"
    out.mkdir(exist_ok=True)
    label = f"{terminal}-{mode}"
    log = out / f"{label}.log"
    installed = mode.startswith("installed")
    animated = mode in ("animation", "installed-animation")
    if installed:
        (folder / "installed-ready").unlink(missing_ok=True)
        helper = ["/usr/bin/python3", str(Path(__file__).resolve()), "--inside-installed", str(folder)]
    else:
        helper = [os.environ["TERMIMOCHI_SVG_WORKER_BIN"], "--termimochi-pixel-trial", str(folder)]
    env = dict(os.environ, LIBGL_ALWAYS_SOFTWARE="1")
    env.pop("NO_COLOR", None)  # Test a normal color-enabled terminal, not the CI log policy.
    if mode == "rejected" or installed:
        env["NO_COLOR"] = "1"  # The explicit real-TTY trial must still request graphics.
    with log.open("w") as stream:
        process = subprocess.Popen(terminal_command(terminal, helper), cwd="/",
            env=env, stdout=stream, stderr=subprocess.STDOUT,
            start_new_session=True)
        try:
            deadline = time.monotonic() + 25
            report = None
            while True:
                assert process.poll() is None, log.read_text()
                assert time.monotonic() < deadline, log.read_text()
                if installed:
                    if (folder / "installed-ready").exists():
                        break
                elif (folder / "result.json").exists():
                    report = json.loads((folder / "result.json").read_text())
                    if report["done"] or report["message"].startswith(("Does the animation", "Is the artwork", "No conclusive")):
                        break
                time.sleep(0.05)
            if mode == "unverified":
                assert not report["done"] and report["sixel"] == "Unverified", report
                assert report["asset_hash"] is None, "Unverified output sent before consent"
                driver("capture", out / f"{label}.png")
                driver("key_q")
                deadline = time.monotonic() + 3
                while not json.loads((folder / "result.json").read_text())["done"]:
                    assert time.monotonic() < deadline
                    time.sleep(0.05)
                report = json.loads((folder / "result.json").read_text())
                assert report["visual"] == "Unverified" and report["asset_hash"] is None, report
            elif mode == "unsupported":
                assert report["done"] and report["visual"] == "Unverified", report
                assert report["asset_hash"] is None, "An unsupported pixel stream was sent"
                assert "no support" in report["message"], report
                driver("capture", out / f"{label}.png")
                driver("key_q")
            else:
                if report:
                    assert not report["done"], report
                    if terminal == "kitty":
                        assert report["kitty"] == "Advertised", report
                        assert report["animation"] == "Unverified", "Graphics ACK incorrectly verified animation"
                    if terminal == "xterm":
                        assert report["sixel"] == "Advertised" and report["cells"], report
                bounds = []
                for index in range(4 if animated else 1):
                    capture = out / f"{label}-{index}.png"
                    driver("capture", capture)
                    bounds.append(check_picture(capture, animated))
                    time.sleep(0.06)
                if animated:
                    assert len(set(bounds)) > 1, "Animation did not move"
                if not installed:
                    driver("key_n" if mode == "rejected" else "key_y")
                    deadline = time.monotonic() + 3
                    while not json.loads((folder / "result.json").read_text())["done"]:
                        assert time.monotonic() < deadline
                        time.sleep(0.05)
                    report = json.loads((folder / "result.json").read_text())
                    assert report["visual"] == ("Failed" if mode == "rejected" else "Confirmed"), report
                    if mode == "animation":
                        assert report["animation"] == "Confirmed", report
                    driver("key_q")
            (out / f"{label}.json").write_text(json.dumps(report or {"installed_rendered": True}, indent=2))
            print(f"PASS {label}: real terminal replies and displayed output; cwd=/", flush=True)
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


if __name__ == "__main__":
    if sys.argv[1] == "--observe-existing":
        assert os.environ.get("DISPLAY") not in (None, ":0", ":1")
        os.environ["TERMIMOCHI_TEST_WINDOW_TITLE"] = "TermiMochi Image Trial"
        out = Path(os.environ["XDG_CACHE_HOME"])
        bounds = []
        for index in range(4):
            capture = out / f"gui-native-animation-{index}.png"
            driver("capture", capture)
            bounds.append(check_picture(capture, True))
            time.sleep(0.06)
        assert len(set(bounds)) > 1, "Animation did not move"
        print("PASS: actual animation moves; no approval or configuration was written by the observer")
    elif sys.argv[1] == "--inside-installed":
        folder = Path(sys.argv[2])
        # Intentionally immediate: the installed configuration owns startup
        # stabilization, not a sleep hidden in the test launcher.
        subprocess.run(["/usr/bin/fastfetch", "--config", str(folder / "config.jsonc")], cwd="/", check=True, timeout=12)
        (folder / "installed-ready").write_text("ok")
        sys.stdin.readline()
    else:
        main(Path(sys.argv[1]), sys.argv[2], sys.argv[3])
