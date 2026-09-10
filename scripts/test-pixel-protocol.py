#!/usr/bin/env python3
"""Native-byte contract test; run only with the generated, trusted test bundle.

The PTY advertises a fixed cell geometry. It is not a Kitty/Sixel renderer and
does not prove visual compatibility. No user Fastfetch configuration is loaded.
"""
import errno
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import time


def main():
    folder, protocol = sys.argv[1:]
    pid, master = pty.fork()
    if pid == 0:
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 960, 640))
        os.chdir(folder)
        env = dict(os.environ, TERM="xterm-256color")
        os.execve("/usr/bin/fastfetch", ["fastfetch", "--config", "config.jsonc",
                  "--pipe", "false", "--show-errors", "true"], env)
    output = bytearray()
    deadline = time.monotonic() + 10
    try:
        while True:
            if time.monotonic() > deadline:
                raise AssertionError("Fastfetch pixel protocol timed out")
            if not select.select([master], [], [], 0.1)[0]:
                continue
            try:
                chunk = os.read(master, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            output.extend(chunk)
            if len(output) > 8 * 1024 * 1024:
                raise AssertionError("Unexpectedly large protocol output")
        _, status = os.waitpid(pid, 0)
        pid = None
        assert os.waitstatus_to_exitcode(status) == 0, status
        assert b"PIXEL_EXPORT_OK" in output, "Missing test field"
        assert b"Logo:" not in output, "Fastfetch reported a logo error"
        if protocol == "kitty-direct":
            assert b"\x1b_G" in output and b"f=100" in output, "Missing Kitty PNG transfer"
        else:
            assert b"\x1bP" in output and b"\x1b\\" in output, "Missing Sixel payload"
        print(f"PASS {protocol}: native output, {len(output)} bytes; not a visual rendering test")
    finally:
        if pid is not None:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.waitpid(pid, 0)
        os.close(master)


if __name__ == "__main__":
    main()
