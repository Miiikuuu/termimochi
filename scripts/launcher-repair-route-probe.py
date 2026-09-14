#!/usr/bin/python3
"""Private GUI probe of the actual retained runtime's error -> targeted repair route."""
import json
import os
from pathlib import Path
import signal
import subprocess as sp
import sys
import time

import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi
from PIL import ImageGrab

runtime, receipt, checksum = sys.argv[1:]
assert os.environ['GSETTINGS_BACKEND'] == 'memory'
assert os.environ['DISPLAY'].split('.')[0] not in (':0', ':1')
assert Path(receipt).is_relative_to(Path(os.environ['XDG_DATA_HOME']) / 'termimochi/kitty-launchers')
output = Path(os.environ['XDG_CACHE_HOME']) / 'repair-route'
output.mkdir()
Atspi.init()


def descendants(node):
    if node is None:
        return
    yield node
    try:
        for index in range(node.get_child_count()):
            yield from descendants(node.get_child_at_index(index))
    except Exception:
        pass


def owned():
    for app in descendants(Atspi.get_desktop(0)):
        try:
            pid = app.get_process_id()
            if Path(f'/proc/{pid}/exe').resolve() == Path(runtime):
                yield app
        except Exception:
            pass


def wait(predicate, message):
    until = time.monotonic() + 25
    while time.monotonic() < until:
        result = predicate()
        if result:
            return result
        time.sleep(.2)
    raise AssertionError(message)


def names():
    return [n.get_name() for n in owned()]


env = dict(os.environ, GTK_A11Y='atspi')
with (output / 'runtime.log').open('w') as log:
    process = sp.Popen([runtime, '--open-kitty-launcher', receipt, checksum], env=env,
                       stdout=log, stderr=sp.STDOUT)
    try:
        repair = wait(lambda: next((n for n in owned() if n.get_name() == 'Repair This Launcher…'
                      and n.get_role_name() in ('button', 'push button')), None), 'Missing actual launcher error repair button')
        ImageGrab.grab(xdisplay=os.environ['DISPLAY']).save(output / '01-error.png')
        assert repair.get_action_iface().do_action(0)
        wait(lambda: any(name.startswith('Repair: Daily Launcher Native') for name in names()),
             'Error page did not route the same theme into the actual repair GUI')
        ImageGrab.grab(xdisplay=os.environ['DISPLAY']).save(output / '02-targeted-review.png')
        (output / 'result.json').write_text(json.dumps(dict(passed=True, names=names(), receipt=receipt), indent=2))
        print(output / 'result.json')
    finally:
        # Only the two private retained-runtime processes created by this route.
        for pid in {n.get_process_id() for n in owned()}:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        if process.poll() is None:
            process.terminate()
        process.wait(timeout=5)
