#!/usr/bin/env python3
"""Private native test driver; never launches against the user's desktop."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from PIL import Image

r = json.loads(sys.argv[1])
assert os.environ['GSETTINGS_BACKEND'] == 'memory'
assert os.environ['DISPLAY'].split('.')[0] not in (':0', ':1')
root = Path(os.environ['XDG_CACHE_HOME']) / 'daily-launcher-evidence'
root.mkdir(exist_ok=True)


def remote(socket, *args):
    return subprocess.run(['/usr/bin/kitty', '@', '--to', 'unix:' + str(socket), *args],
                          capture_output=True, text=True, timeout=5)


for phase in ('first', 'reopen'):
    log = root / (phase + '.log')
    socket = None
    with log.open('w') as output:
        # GIO parses the actual installed desktop file and invokes the retained
        # production binary. No editor is running and no manually built command
        # substitutes for its Exec field.
        started = subprocess.run(['gio', 'launch', r['desktop']], stdout=output,
                                 stderr=subprocess.STDOUT, timeout=10)
        assert started.returncode == 0, log.read_text()
    try:
        deadline = time.monotonic() + 25
        while True:
            sockets = list(Path(os.environ['XDG_RUNTIME_DIR']).glob('daily-kitty*'))
            for candidate in sockets:
                result = remote(candidate, 'get-text', '--extent', 'all')
                if result.returncode == 0 and 'CURRENT_PROMPT_MARKER' in result.stdout:
                    socket, text = candidate, result.stdout
                    break
            if socket:
                break
            assert time.monotonic() < deadline, ('Missing live prompt', log.read_text())
            time.sleep(.1)
        assert text.count('GREETING_ONCE') == 1, text
        assert 'LOCAL_RC_STDOUT' not in text, text
        colors = remote(socket, 'get-colors').stdout
        assert any(line.split() == ['background', r['background']] for line in colors.lower().splitlines()), colors
        (root / (phase + '-colors.txt')).write_text(colors)
        evidence = Path(os.environ['HOME']) / (phase + '-shell.txt')
        # Only the private test shell receives keystrokes. Alias, function, PATH,
        # history and prompt are inspected after real interactive initialization.
        command = ("{ alias daily_alias; declare -f daily_function; printf '%s\\n' \"$PATH\" \"$HISTFILE\" \"$STARSHIP_CONFIG\"; "
                   "kitty +kitten query_terminal font_family font_size; } > " + str(evidence) + "\n")
        assert remote(socket, 'send-text', command).returncode == 0
        deadline = time.monotonic() + 15
        while not evidence.exists() or 'font_size:' not in evidence.read_text():
            assert time.monotonic() < deadline
            time.sleep(.1)
        data = evidence.read_text()
        assert 'daily_alias=' in data and 'daily_function' in data, data
        assert '/daily-test-bin:' in data and '.daily_history' in data, data
        assert 'starship.toml' in data, data
        assert 'LiberationMono' in data.replace(' ', ''), data
        assert float(dict(line.split(':', 1) for line in data.splitlines() if line.startswith('font_size:'))['font_size']) == 15.0, data
        (root / (phase + '-shell.txt')).write_text(data)
        time.sleep(.2)
        (root / (phase + '-windows.txt')).write_text(subprocess.run(['xwininfo', '-root', '-tree'], capture_output=True, text=True, check=True).stdout)
        positions = []
        for frame in range(8):
            shot = root / f'{phase}-{frame}.png'
            env = dict(os.environ, TERMIMOCHI_TEST_WINDOW_TITLE='TermiMochi · Daily Launcher Native',
                       TERMIMOCHI_INSPECT_SCREENSHOT=str(shot))
            subprocess.run(['/usr/bin/python3', r['driver'], 'capture', '0', '0'], env=env,
                           check=True, timeout=10, stdout=subprocess.DEVNULL)
            image = Image.open(shot).convert('RGB')
            red = [x for y in range(image.height) for x in range(image.width)
                   if (p := image.getpixel((x, y)))[0] > 150 and p[1] < 70 and p[2] < 110]
            assert red, 'Missing GIF pixels'
            positions.append(min(red))
            time.sleep(.07)
        assert len(set(positions)) > 1, ('GIF did not move', positions)
        text = remote(socket, 'get-text', '--extent', 'all').stdout
        assert text.count('GREETING_ONCE') == 1, text
        assert text.count('CURRENT_PROMPT_MARKER') >= 2, text
        (root / (phase + '-text.txt')).write_text(text)
        assert not (Path(os.environ['HOME']) / 'profile-must-not-run').exists()
        print('PASS', phase, 'GIO desktop → retained runtime → real Kitty: palette, font, Starship, GIF, personal Bash', positions, flush=True)
    finally:
        if socket:
            remote(socket, 'close-window')
            deadline = time.monotonic() + 5
            while socket.exists() and time.monotonic() < deadline:
                time.sleep(.05)
