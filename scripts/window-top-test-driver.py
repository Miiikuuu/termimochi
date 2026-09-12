"""Real Kitty chrome checks, restricted to a fresh test HOME and Xvfb."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from PIL import Image

assert os.environ['GSETTINGS_BACKEND'] == 'memory'
assert os.environ['DISPLAY'].split('.')[0] not in (':0', ':1')
request = json.loads(sys.argv[1])
evidence = Path(os.environ['XDG_CACHE_HOME']) / 'window-top-kitty'
evidence.mkdir()
for case in request['cases']:
    def remote(*args):
        return subprocess.run([case['program'], '@', '--to', case['socket'], *args],
                              capture_output=True, text=True, timeout=5)

    def capture(name):
        path = evidence / f"{case['name']}-{name}.png"
        subprocess.run(['/usr/bin/python3', request['driver'], 'capture', '0', '0'],
                       env=dict(os.environ, TERMIMOCHI_INSPECT_SCREENSHOT=str(path)),
                       check=True, timeout=8, stdout=subprocess.DEVNULL)
        image = Image.open(path).convert('RGB')
        return [(x, y) for y in range(image.height) for x in range(image.width)
                if image.getpixel((x, y)) == (34, 68, 102)], image.height

    with (evidence / f"{case['name']}.log").open('w') as log:
        child = subprocess.Popen([case['program'], *case['args']], env=case['env'],
                                 cwd=os.environ['HOME'], stdout=log, stderr=subprocess.STDOUT)
        try:
            until = time.monotonic() + 15
            while remote('ls').returncode:
                assert child.poll() is None
                assert time.monotonic() < until
                time.sleep(.1)
            colors = remote('get-colors').stdout
            (evidence / f"{case['name']}-colors.txt").write_text(colors)
            actual = dict(line.split() for line in colors.splitlines())
            for key, value in {'active_tab_foreground': '#fafafa', 'active_tab_background': '#224466',
                               'inactive_tab_foreground': '#dddddd', 'inactive_tab_background': '#735421'}.items():
                assert actual[key].lower() == value, (key, actual)
            time.sleep(.25)
            pixels, height = capture('one-tab')
            assert bool(pixels) == (case['visible'] and case['minimum'] == 1), case
            launched = remote('launch', '--type', 'tab', '--tab-title', 'Second',
                              '/usr/bin/bash', '--noprofile', '--norc', '-i')
            assert launched.returncode == 0, launched.stderr
            time.sleep(.35)
            pixels, height = capture('two-tabs')
            assert bool(pixels) == case['visible'], (case, height)
            if pixels:
                y = sum(y for _, y in pixels) / len(pixels)
                assert (y < height / 2) == (case['edge'] == 'top'), (case, y, height)
            assert len(json.loads(remote('ls').stdout)[0]['tabs']) == 2
            print('PASS real Kitty', case['name'], 'configured colors, visibility and tab-bar edge', flush=True)
        finally:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
