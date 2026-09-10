#!/usr/bin/env python3
"""Exercise a trusted GIF bundle in a real, isolated Kitty on the test X server.

Never run this with an imported/unreviewed config. Does not load user Kitty or
shell settings. The inside helper forwards queries immediately so Kitty, not a
fake responder, supplies terminal geometry and graphics capabilities.
"""
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import time
from PIL import Image


def inside(folder):
    process = subprocess.Popen(["/usr/bin/fastfetch", "--config", "config.jsonc",
        "--pipe", "false", "--show-errors", "true"], cwd=folder,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    output = bytearray()
    deadline = time.monotonic() + 12
    try:
        while True:
            if time.monotonic() > deadline:
                raise AssertionError("Fastfetch GIF rendering timed out")
            if not select.select([process.stdout], [], [], 0.1)[0]:
                continue
            chunk = os.read(process.stdout.fileno(), 65536)
            if not chunk:
                break
            output.extend(chunk)
            if len(output) > 8 * 1024 * 1024:
                raise AssertionError("Unexpectedly large graphics output")
            sys.stdout.buffer.write(chunk)
            sys.stdout.buffer.flush()
        assert process.wait(timeout=2) == 0
        assert b"PIXEL_EXPORT_OK" in output
        assert b"Logo:" not in output and b"Logo (" not in output, repr(output[-400:])
        assert b"\x1b_G" in output, "Missing Kitty graphics transfer"
        assert b"a=f" in output, "Missing animation frame command"
        assert b"a=a" in output, "Missing animation playback command"
        sys.stdout.write("\x1b[?25l")
        sys.stdout.flush()
        (folder / "native-result.json").write_text(json.dumps({"passed": True, "bytes": len(output)}))
        # Keep this test window alive until the parent has captured it.
        sys.stdin.readline()
    except Exception as error:
        (folder / "native-result.json").write_text(json.dumps({"passed": False, "error": str(error)}))
        raise
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


def main(folder):
    assert os.environ.get("DISPLAY") not in (None, ":0", ":1"), "Use a dedicated Xvfb display"
    env = dict(os.environ, LIBGL_ALWAYS_SOFTWARE="1")
    log = folder / "kitty-native.log"
    with log.open("w") as stream:
        process = subprocess.Popen(["/usr/bin/kitty", "--config", "NONE",
            "-o", "linux_display_server=x11", "-o", "shell_integration=disabled",
            "-o", "background=#F7F7F5", "-o", "foreground=#30363D",
            "-o", "font_size=12", "-o", "remember_window_size=no",
            "-o", "initial_window_width=800", "-o", "initial_window_height=600",
            "--title", "TermiMochi point-to-edit test",
            "/usr/bin/python3", str(Path(__file__).resolve()), "--inside", str(folder)],
            env=env, stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            deadline = time.monotonic() + 20
            result = folder / "native-result.json"
            while not result.exists():
                assert process.poll() is None, log.read_text()
                assert time.monotonic() < deadline, "No successful GIF rendering result: " + log.read_text()
                time.sleep(0.05)
            report = json.loads(result.read_text())
            assert report["passed"], report
            capture_dir = Path(os.environ.get("XDG_CACHE_HOME", str(folder)))
            captures = []
            for index in range(4):
                screenshot = capture_dir / f"kitty-animation-{index}.png"
                subprocess.run(["/usr/bin/python3", str(Path(__file__).with_name("preview-pointer-driver.py")),
                    "capture", "0", "0"], env=dict(env, TERMIMOCHI_INSPECT_SCREENSHOT=str(screenshot)),
                    check=True, timeout=8, stdout=subprocess.DEVNULL)
                captures.append(screenshot.read_bytes())
                # One narrow red rectangle per frame. Mere movement is not
                # enough: a broken blend can leave all three behind.
                pixels = Image.open(screenshot).convert("RGB")
                red_x = [x for y in range(pixels.height) for x in range(pixels.width)
                         if (p := pixels.getpixel((x,y)))[0] > 120 and p[1] < 90 and p[2] < 120]
                assert red_x, "Rendered GIF fixture is missing"
                assert max(red_x) - min(red_x) < 120, "Transparent-frame ghosting: old red rectangles remain visible"
            assert len(set(captures)) > 1, "Kitty screenshots did not change across animation frames"
            print(f"PASS: real Kitty animation, {report['bytes']} protocol bytes; changing screenshots in {capture_dir}")
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == "__main__":
    if sys.argv[1] == "--inside":
        inside(Path(sys.argv[2]))
    else:
        main(Path(sys.argv[1]))
