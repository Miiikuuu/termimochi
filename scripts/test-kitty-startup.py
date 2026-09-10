"""Compare immediate Kitty startup with/without the managed settling guard.

Diagnostic only: requires a dedicated X11 display, TERMIMOCHI_SVG_WORKER_BIN,
and the red/blue PNG fixture from an isolated pixel-trial test. Baseline failures
are recorded, not treated as test failures; guarded failures exit nonzero.
"""
import importlib.util
import json
import os
from pathlib import Path
import signal
import shlex
import subprocess
import sys
import tempfile
import time

spec = importlib.util.spec_from_file_location("trial", Path(__file__).with_name("test-pixel-trial.py"))
trial = importlib.util.module_from_spec(spec)
spec.loader.exec_module(trial)


def inside(folder):
    started = time.monotonic()
    changes = []
    signal.signal(signal.SIGWINCH, lambda *_: changes.append(round(time.monotonic()-started, 4)))
    result = subprocess.run(["/usr/bin/fastfetch", "--config", str(folder / "config.jsonc"),
                             "--pipe", "false", "--show-errors", "true"], timeout=12)
    (folder / "ready.json").write_text(json.dumps({"status": result.returncode,
        "seconds": time.monotonic()-started, "winch": changes}))
    sys.stdin.readline()


def main(png, repetitions):
    assert os.environ.get("DISPLAY") not in (None, ":0", ":1")
    root = Path(tempfile.mkdtemp(prefix="termimochi-kitty-startup-"))
    print(root, flush=True)
    results = []
    for variant in ("direct", "guarded"):
        for index in range(repetitions):
            folder = root / f"{variant}-{index}"
            folder.mkdir()
            config = {"logo": {"type": "kitty-direct",
                "source": str(png), "width": 24, "height": 8,
                "padding": {"right": 2}, "printRemaining": True},
                "modules": [{"type": "custom", "format": "STARTUP TEST"}, "os", "shell", "cpu", "memory", "datetime", "gpu", "disk", "colors"]}
            if variant == "guarded":
                config["general"] = {"preRun": shlex.quote(os.environ["TERMIMOCHI_SVG_WORKER_BIN"]) + " --termimochi-settle-kitty"}
            (folder / "config.jsonc").write_text(json.dumps(config))
            helper = ["/usr/bin/python3", str(Path(__file__).resolve()), "--inside", str(folder)]
            with (folder / "terminal.log").open("w") as log:
                command = trial.terminal_command("kitty", helper)
                command[1:1] = ["--dump-commands", "--dump-bytes", str(folder / "output.bin")]
                process = subprocess.Popen(command, cwd="/",
                    env=dict(os.environ, LIBGL_ALWAYS_SOFTWARE="1"), stdout=log, stderr=log)
                try:
                    deadline = time.monotonic() + 15
                    while not (folder / "ready.json").exists():
                        assert process.poll() is None, (folder / "terminal.log").read_text()
                        assert time.monotonic() < deadline
                        time.sleep(0.02)
                    time.sleep(0.2)  # Observe persistence AFTER the immediate Fastfetch invocation.
                    capture = folder / "screen.png"
                    trial.driver("capture", capture)
                    detail = json.loads((folder / "ready.json").read_text())
                    try:
                        detail["bounds"] = trial.check_picture(capture, False)
                        detail["passed"] = True
                    except AssertionError as error:
                        detail.update(passed=False, error=str(error))
                    detail.update(variant=variant, index=index)
                    results.append(detail)
                    print(detail, flush=True)
                finally:
                    process.terminate()
                    process.wait(timeout=5)
    (root / "results.json").write_text(json.dumps(results, indent=2))
    raise SystemExit(any(not r["passed"] for r in results if r["variant"] == "guarded"))


if __name__ == "__main__":
    if sys.argv[1] == "--inside":
        inside(Path(sys.argv[2]))
    else:
        main(Path(sys.argv[1]).resolve(), int(sys.argv[2]))
