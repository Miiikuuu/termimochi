#!/usr/bin/python3
"""Private test-shell probe: ask actual VTE for its background and grid."""
import fcntl
import json
import os
from pathlib import Path
import select
import struct
import sys
import termios
import time
import tty

root = Path(sys.argv[1]).resolve()
assert os.environ['HOME'] == str(root / 'home')
assert os.environ['GSETTINGS_BACKEND'] == 'keyfile'
assert os.environ.get('DISPLAY') == ':96'
fd = os.open('/dev/tty', os.O_RDWR | os.O_NOCTTY)
saved = termios.tcgetattr(fd)
reply = b''
try:
    tty.setraw(fd)
    os.write(fd, b'\x1b]11;?\x07')
    until = time.monotonic() + 3
    while time.monotonic() < until:
        if select.select([fd], [], [], .1)[0]:
            reply += os.read(fd, 1024)
            if b'\x07' in reply or b'\x1b\\' in reply:
                break
    rows, cols, _, _ = struct.unpack('HHHH', fcntl.ioctl(fd, termios.TIOCGWINSZ, b'\0' * 8))
finally:
    termios.tcsetattr(fd, termios.TCSANOW, saved)
    os.close(fd)
record = dict(profile=os.environ.get('PTYXIS_PROFILE'), background=reply.decode('ascii'),
              rows=rows, columns=cols, pid=os.getpid(), shell_pid=os.getppid())
with (root / 'shell-probes.jsonl').open('a') as out:
    out.write(json.dumps(record) + '\n')
print('MMMMMMMMMMMMMMMMMMMM')
print('PTYXIS APPLY QA — real shell, private configuration')
