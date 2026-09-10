"""Render trusted Rust-generated bundles in real Kitty/xterm on isolated Xvfb.

Uses actual terminal replies, never a synthetic graphics decoder. Requires Pillow,
GStreamer, the existing X11 capture driver, Kitty and a SIXEL-enabled xterm.
TERMIMOCHI_XTERM may name an unpacked test binary; no system installation needed.
"""
import json
import os
from pathlib import Path
import select
import shutil
import subprocess
import sys
import time
from PIL import Image


def inside(folder):
    # Let the new terminal map and finish its initial resize before testing
    # steady-state rendering. This does not test instant shell-startup races.
    time.sleep(0.75)
    command = ["/usr/bin/fastfetch", "--config", "config.jsonc",
        "--pipe", "false", "--show-errors", "true"]
    # Diagnosis only: compare relative versus absolute filenames without
    # rewriting the exported config or changing the application's exporter.
    if os.environ.get("TERMIMOCHI_VISUAL_ABSOLUTE_SOURCE") == "1":
        command += ["--logo", str(folder / "logo.png")]
    if os.environ.get("TERMIMOCHI_VISUAL_PIPE_OUTPUT") != "1":
        # Fastfetch must own the real terminal directly. Forwarding PIPE output
        # produced false blank Kitty images; retain it as an opt-in diagnostic,
        # never as the default visual acceptance path.
        result = subprocess.run(command, cwd=folder, timeout=12)
        assert result.returncode == 0
        (folder / "visual-output.bin").write_bytes(b"Direct TTY control; no byte capture")
        (folder / "visual-ready.json").write_text(json.dumps({"direct_tty": True}))
        sys.stdin.readline()
        return
    child = subprocess.Popen(command, cwd=folder,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    output = bytearray()
    try:
        deadline = time.monotonic() + 12
        while True:
            assert time.monotonic() < deadline, "Fastfetch timed out"
            if not select.select([child.stdout], [], [], 0.1)[0]:
                continue
            chunk = os.read(child.stdout.fileno(), 65536)
            if not chunk:
                break
            output.extend(chunk)
            assert len(output) < 8 * 1024 * 1024
            sys.stdout.buffer.write(chunk)
            sys.stdout.buffer.flush()
        assert child.wait(timeout=2) == 0
        assert b"PIXEL_EXPORT_OK" in output
        assert b"Logo:" not in output and b"Logo (" not in output, repr(output[-500:])
        assert b"\x1b_G" in output or b"\x1bP" in output, "No graphics emitted"
        sys.stdout.write("\x1b[?25l")
        sys.stdout.flush()
        (folder / "visual-output.bin").write_bytes(output)
        (folder / "visual-ready.json").write_text(json.dumps({"bytes": len(output)}))
        sys.stdin.readline()
    finally:
        if child.poll() is None:
            child.kill()
            child.wait()


def check_chart(path, background, position):
    screen = Image.open(path).convert("RGB")
    colors = [(200, 30, 70), (30, 180, 100), (40, 100, 220), (230, 180, 30)]
    groups = [[] for _ in colors]
    for y in range(screen.height):
        for x in range(screen.width):
            p = screen.getpixel((x, y))
            for group, color in zip(groups, colors):
                if max(abs(a-b) for a, b in zip(p, color)) <= 5:
                    group.append((x, y))
                    break
    assert all(len(g) > 150 for g in groups), f"Missing or discolored quadrants: {[len(g) for g in groups]}"
    centers = [(sum(x for x, _ in g)/len(g), sum(y for _, y in g)/len(g)) for g in groups]
    assert centers[0][0] < centers[1][0] and centers[2][0] < centers[3][0]
    assert centers[0][1] < centers[2][1] and centers[1][1] < centers[3][1]
    points = sum(groups, [])
    left, right = min(x for x, _ in points), max(x for x, _ in points)
    top, bottom = min(y for _, y in points), max(y for _, y in points)
    assert right - left < screen.width * 0.6 and bottom < screen.height - 20, "Clipped or oversized artwork"
    # Fully transparent center hole and removed outer background, on both themes.
    for x, y in [((left+right)//2, (top+bottom)//2),
                 (max(0, left-(right-left)//12), (top+bottom)//2)]:
        actual = screen.getpixel((x, y))
        assert max(abs(a-b) for a, b in zip(actual, background)) <= 5, f"Opaque background at {(x,y)}: {actual}, expected {background}"
    if position == "right":
        assert left > screen.width * 0.3, "Right logo did not move to the right"
    elif position == "left":
        assert right < screen.width * 0.5, "Left logo overlaps the field area"
    return {"color_pixels": [len(g) for g in groups], "bounds": [left, top, right, bottom]}


def main(folder, protocol, fixture):
    assert os.environ.get("DISPLAY") not in (None, ":0", ":1"), "Use dedicated Xvfb"
    # Generated test config is regular JSON apart from optional comments; Rust
    # fixture intentionally has none. No user config is ever executed here.
    config = json.loads((folder / "config.jsonc").read_text())
    position = config["logo"]["position"]
    columns = {"left": 80, "right": 100, "top": 120}[position]
    capture_dir = Path(os.environ["XDG_CACHE_HOME"]) / "pixel-visual"
    capture_dir.mkdir(exist_ok=True)
    shutil.copyfile(folder / "logo.png", capture_dir / f"{protocol}-{fixture}-{position}-source.png")
    failures = []
    for theme, bg, fg in [("light", "#F7F7F5", "#30363D"), ("dark", "#20252B", "#E9E9E7")]:
        label = f"{protocol}-{fixture}-{position}-{theme}-{columns}"
        ready = folder / "visual-ready.json"
        ready.unlink(missing_ok=True)
        helper = ["/usr/bin/python3", str(Path(__file__).resolve()), "--inside", str(folder)]
        title = "TermiMochi point-to-edit test"
        if protocol == "kitty-direct":
            command = ["/usr/bin/kitty", "--config", "NONE", "-o", "linux_display_server=x11",
                "-o", "shell_integration=disabled", "-o", "remember_window_size=no",
                "-o", f"background={bg}", "-o", f"foreground={fg}", "-o", "font_size=12",
                "-o", f"initial_window_width={columns}c", "-o", "initial_window_height=32c",
                "--title", title] + helper
        else:
            command = [os.environ.get("TERMIMOCHI_XTERM", "xterm"), "-ti", "vt340",
                "-xrm", "XTerm*decGraphicsID: 340", "-xrm", "XTerm*numColorRegisters: 256",
                "-xrm", "XTerm*sixelScrolling: true", "-xrm", "XTerm*allowWindowOps: true",
                "-fa", "monospace", "-fs", "12", "-geometry", f"{columns}x32",
                "-bg", bg, "-fg", fg, "-title", title, "-e"] + helper
        log_path = capture_dir / f"{label}.log"
        with log_path.open("w") as log:
            child = subprocess.Popen(command, cwd=folder, env=dict(os.environ, LIBGL_ALWAYS_SOFTWARE="1"),
                stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                deadline = time.monotonic() + 20
                while not ready.exists():
                    assert child.poll() is None, log_path.read_text()
                    assert time.monotonic() < deadline, f"No render result: {log_path.read_text()}"
                    time.sleep(0.05)
                path = capture_dir / f"{label}.png"
                shutil.copyfile(folder / "visual-output.bin", capture_dir / f"{label}.bin")
                subprocess.run(["python3", str(Path(__file__).with_name("preview-pointer-driver.py")),
                    "capture", "0", "0"], env=dict(os.environ, TERMIMOCHI_INSPECT_SCREENSHOT=str(path)),
                    check=True, timeout=8, stdout=subprocess.DEVNULL)
                detail = check_chart(path, tuple(bytes.fromhex(bg[1:])), position) if fixture == "chart" else {"manual_review_required": True}
                (capture_dir / f"{label}.json").write_text(json.dumps(detail))
                print(f"{'PASS' if fixture == 'chart' else 'CAPTURE'} {label}: {detail}", flush=True)
            except Exception as error:
                failures.append(f"{label}: {error}")
                print(f"FAIL {label}: {error}", flush=True)
            finally:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
    assert not failures, "\n".join(failures)


if __name__ == "__main__":
    if sys.argv[1] == "--inside":
        inside(Path(sys.argv[2]))
    else:
        main(Path(sys.argv[1]), sys.argv[2], sys.argv[3])
