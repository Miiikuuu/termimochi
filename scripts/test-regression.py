#!/usr/bin/env python3
"""Run each opt-in native/GTK regression in its own process on a test X11 display.

Requires Fastfetch, Bubblewrap, prlimit, Ptyxis, dbus-run-session and a running Xvfb.
Example: python3 scripts/test-regression.py --display :91 --scale 1 --scale 2
The regular workspace tests, Clippy and packaging checks remain separate.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--display", required=True, help="Dedicated Xvfb display, not your desktop")
    parser.add_argument("--scale", action="append", type=int, choices=[1, 2])
    parser.add_argument("--filter", action="append", default=[],
                        help="Test-name substring; repeat to match any of several names")
    args = parser.parse_args()
    if not args.display.startswith(":") or args.display.split(".")[0] in (":0", ":1"):
        parser.error("Use a dedicated Xvfb display (never the user's :0 / :1 desktop)")
    repo = Path(__file__).resolve().parent.parent
    output = Path(tempfile.mkdtemp(prefix="termimochi-regression-"))
    print(f"Logs: {output}", flush=True)
    build = subprocess.run(["cargo", "test", "-p", "termimochi", "--locked", "--no-run",
                            "--message-format=json"], cwd=repo, check=False,
                           stdout=subprocess.PIPE, text=True)
    artifacts = [json.loads(line) for line in build.stdout.splitlines()]
    if build.returncode:
        for item in artifacts:
            if item.get("reason") == "compiler-message":
                print(item["message"].get("rendered", item["message"]["message"]), flush=True)
        raise SystemExit(build.returncode)
    binary = next(item["executable"] for item in artifacts
                  if item.get("reason") == "compiler-artifact" and item.get("executable")
                  and item["profile"]["test"] and item["target"]["name"] == "termimochi")
    # Pin one build for the entire matrix, even if sources change while it runs.
    pinned = output / "termimochi-tests"
    pinned.write_bytes(Path(binary).read_bytes())
    pinned.chmod(0o700)
    worker_build = subprocess.run(["cargo", "build", "-p", "termimochi", "--locked",
                                   "--message-format=json"], cwd=repo, check=True,
                                  stdout=subprocess.PIPE, text=True)
    worker_binary = next(item["executable"] for item in map(json.loads, worker_build.stdout.splitlines())
                         if item.get("reason") == "compiler-artifact" and item.get("executable")
                         and item["target"]["name"] == "termimochi")
    worker = output / "termimochi-worker"
    worker.write_bytes(Path(worker_binary).read_bytes())
    worker.chmod(0o700)
    (output / "build.json").write_text(json.dumps({"sha256": hashlib.sha256(pinned.read_bytes()).hexdigest(),
        "worker_sha256": hashlib.sha256(worker.read_bytes()).hexdigest()}, indent=2) + "\n")
    listing = subprocess.run([str(pinned), "--ignored", "--list"], cwd=repo,
                             check=True, capture_output=True, text=True).stdout
    tests = [line.removesuffix(": test") for line in listing.splitlines()
             if line.endswith(": test") and
             (not args.filter or any(fragment in line for fragment in args.filter))]
    if not tests:
        raise SystemExit("No matching tests; nothing was verified")
    results = []
    for scale in args.scale or [1, 2]:
        for test in tests:
            name = f"{scale}x-{test.replace('::', '-')}"
            case = output / name
            case.mkdir()
            env = os.environ.copy()
            # Build with the normal toolchain, then isolate each GUI process's
            # HOME as well as XDG state. Never source the user's startup files.
            isolated_home = case / "home"
            isolated_home.mkdir()
            env["HOME"] = str(isolated_home)
            # AF_UNIX paths (notably Ptyxis' child helper) have a 108-byte
            # limit. A deeply nested report path is not a valid runtime root.
            runtime = Path(tempfile.mkdtemp(prefix="termimochi-runtime-", dir="/tmp"))
            env["XDG_RUNTIME_DIR"] = str(runtime)
            for key in ("WAYLAND_DISPLAY", "SWAYSOCK", "GNOME_KEYRING_CONTROL", "SSH_AUTH_SOCK"):
                env.pop(key, None)
            for key in ("CONFIG", "DATA", "STATE", "CACHE"):
                directory = case / key.lower()
                directory.mkdir()
                env[f"XDG_{key}_HOME"] = str(directory)
            env.update(DISPLAY=args.display, GDK_BACKEND="x11", GSK_RENDERER="cairo",
                       GTK_A11Y="none", GTK_IM_MODULE="simple", GIO_USE_VFS="local",
                       G_DEBUG="fatal-criticals", GSETTINGS_BACKEND="memory",
                       GDK_SCALE=str(scale), TERMIMOCHI_FASTFETCH_TEST_BIN="/usr/bin/fastfetch",
                       TERMIMOCHI_SVG_WORKER_BIN=str(worker))
            started = time.monotonic()
            with (case / "output.log").open("w") as log:
                process = subprocess.Popen(["dbus-run-session", "--", str(pinned),
                    test, "--exact", "--ignored", "--test-threads=1", "--nocapture"],
                    cwd=repo, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    code = process.wait(timeout=180)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGTERM)
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
                    code = 124
            text = (case / "output.log").read_text()
            passed = code == 0 and "1 passed; 0 failed" in text
            result = dict(test=test, scale=scale, passed=passed, status=code,
                          seconds=round(time.monotonic() - started, 2), log=str(case / "output.log"))
            results.append(result)
            (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
            print(f"{'PASS' if passed else 'FAIL'} {scale}x {test} ({result['seconds']}s)", flush=True)
    failed = [r for r in results if not r["passed"]]
    print(f"{len(results) - len(failed)}/{len(results)} passed; report: {output / 'results.json'}", flush=True)
    raise SystemExit(bool(failed))


if __name__ == "__main__":
    main()
