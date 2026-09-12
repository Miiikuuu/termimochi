#!/usr/bin/python3
"""Own the private bus and native client's descendants, never the host session.

Linux subreaper + pidfds keep double-forked shells in this lifetime. No matching
by executable name, no host systemd service, and no daily environment mutation.
"""
import ctypes
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


def descendants():
    parents = {}
    for item in Path('/proc').iterdir():
        if not item.name.isdecimal():
            continue
        try:
            fields = (item / 'stat').read_text().rsplit(')', 1)[1].split()
            parents[int(item.name)] = int(fields[1])
        except (OSError, ValueError, IndexError):
            pass
    owned = {os.getpid()}
    while True:
        expanded = owned | {pid for pid, ppid in parents.items() if ppid in owned}
        if expanded == owned:
            return owned - {os.getpid()}
        owned = expanded


def main():
    parent = os.getppid()
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(36, 1, 0, 0, 0) or libc.prctl(1, signal.SIGTERM, 0, 0, 0):
        raise OSError(ctypes.get_errno(), 'subreaper / parent-death setup failed')
    stopping = False

    def stop(signum, frame):
        nonlocal stopping
        stopping = True

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGHUP, stop)
    if os.getppid() != parent or parent == 1:
        return 1
    root = Path(sys.argv[1]).resolve(strict=True)
    fd = int(os.environ['WAYLAND_SOCKET'])
    env = os.environ.copy()
    # A dedicated bus config has no activation directories: no daily services
    # or user manager can be reached through an accidental activation fallback.
    bus = subprocess.Popen(['dbus-daemon', '--nofork', '--config-file=' + str(root/'bus.conf')],
                           stdin=subprocess.DEVNULL)
    address = 'unix:path=' + str(root/'runtime/bus')
    deadline = time.monotonic() + 5
    while not (root/'runtime/bus').exists():
        if stopping or bus.poll() is not None or time.monotonic() > deadline:
            stopping = True
            break
        time.sleep(.02)
    client = None
    handles = {}
    result = 0
    try:
        if not stopping:
            env['DBUS_SESSION_BUS_ADDRESS'] = address
            client = subprocess.Popen(sys.argv[2:], env=env, pass_fds=(fd,))
            print(f'NATIVE_CLIENT_PID={client.pid}', flush=True)
        os.close(fd)
        while not stopping and client is not None and client.poll() is None:
            if bus.poll() is not None:
                result = 1
                break
            time.sleep(.1)
        if not stopping and client is not None and client.returncode is not None:
            result = client.returncode if client.returncode >= 0 else 128 - client.returncode
    finally:
        deadline = time.monotonic() + 2
        while True:
            for pid in descendants():
                if pid not in handles:
                    try:
                        handles[pid] = os.pidfd_open(pid)
                    except ProcessLookupError:
                        pass
            for handle in handles.values():
                try:
                    signal.pidfd_send_signal(handle, signal.SIGKILL if time.monotonic() >= deadline else signal.SIGTERM)
                except ProcessLookupError:
                    pass
            try:
                while os.waitpid(-1, os.WNOHANG)[0]:
                    pass
            except ChildProcessError:
                break
            time.sleep(.03)
        for handle in handles.values():
            os.close(handle)
    return result


if __name__ == '__main__':
    sys.exit(main())
