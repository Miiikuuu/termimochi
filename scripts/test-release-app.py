#!/usr/bin/python3
"""Black-box Release GUI acceptance on an owned Xvfb/private D-Bus/HOME.

Uses AT-SPI controls and XTest shortcuts, not Rust test-only controller hooks.
Only a copy of --theme is edited. No Use/Apply or real startup files are touched.
"""
import argparse
import ctypes as C
import hashlib
import json
import os
import re
import shutil
import signal
from pathlib import Path
import subprocess as sp
import sys
import tempfile
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def wait(predicate, label, seconds=30):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(.15)
    raise AssertionError(label)


def inside(root, binary):
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi
    from PIL import ImageGrab
    assert os.environ['HOME'] == str(root / 'home')
    assert os.environ['GSETTINGS_BACKEND'] == 'memory'
    Atspi.init()
    X, T = C.CDLL('libX11.so.6'), C.CDLL('libXtst.so.6')
    X.XOpenDisplay.argtypes, X.XOpenDisplay.restype = [C.c_char_p], C.c_void_p
    X.XKeysymToKeycode.argtypes, X.XKeysymToKeycode.restype = [C.c_void_p, C.c_ulong], C.c_uint
    X.XFlush.argtypes = [C.c_void_p]
    X.XSetInputFocus.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_ulong]
    T.XTestFakeKeyEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
    T.XTestFakeMotionEvent.argtypes = [C.c_void_p, C.c_int, C.c_int, C.c_int, C.c_ulong]
    T.XTestFakeButtonEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
    display = X.XOpenDisplay(os.environ['DISPLAY'].encode())
    assert display
    def click(node, focus=True):
        windows = sp.check_output(['xwininfo', '-root', '-tree'], text=True)
        xid = int(re.search(r'(0x[0-9a-f]+) "[^"\n]*TermiMochi"', windows).group(1), 16)
        if focus:
            X.XSetInputFocus(display, xid, 1, 0)
        bounds = node.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
        T.XTestFakeMotionEvent(display, -1, bounds.x + bounds.width // 2, bounds.y + bounds.height // 2, 0)
        T.XTestFakeButtonEvent(display, 1, 1, 0)
        T.XTestFakeButtonEvent(display, 1, 0, 0)
        X.XFlush(display)
        time.sleep(.5)
    def chord(*symbols):
        for symbol in symbols:
            T.XTestFakeKeyEvent(display, X.XKeysymToKeycode(display, symbol), 1, 0)
        for symbol in reversed(symbols):
            T.XTestFakeKeyEvent(display, X.XKeysymToKeycode(display, symbol), 0, 0)
        X.XFlush(display)
        time.sleep(.5)
    def descendants(node):
        # AT-SPI may drop a child while GTK replaces a toast or dialog.
        if node is None:
            return
        yield node
        try:
            for i in range(node.get_child_count()):
                yield from descendants(node.get_child_at_index(i))
        except Exception:
            return
    def nodes():
        desktop = Atspi.get_desktop(0)
        for index in range(desktop.get_child_count()):
            app = desktop.get_child_at_index(index)
            if process is not None and app.get_process_id() == process.pid:
                return list(descendants(app))
        return []
    def find(name, role=None):
        for node in nodes():
            try:
                if (node.get_name() == name and (role is None or node.get_role_name() == role)
                        and node.get_state_set().contains(Atspi.StateType.SHOWING)):
                    return node
            except Exception:
                continue
    def dump(name):
        tree = []
        for node in nodes():
            try:
                tree.append([node.get_role_name(), node.get_name()])
            except Exception:
                pass
        (root / (name + '-tree.json')).write_text(json.dumps(tree, ensure_ascii=False, indent=2))
        ImageGrab.grab(xdisplay=os.environ['DISPLAY']).save(root / (name + '.png'))
    theme = root / 'theme.termimochi-design.json'
    before = json.loads(theme.read_text())
    def protected():
        paths = [root / 'home/.bashrc', root / 'home/.bash_profile']
        for directory in ['config/kitty', 'config/fastfetch', 'data/applications']:
            paths.extend(p for p in (root / directory).rglob('*') if p.is_file())
        paths.append(root / 'config/starship.toml')
        return {str(p.relative_to(root)): digest(p) for p in sorted(paths)}
    original = protected()
    (root / 'protected-before.json').write_text(json.dumps(original, indent=2))
    # A separate gsettings process with the memory backend cannot inspect this
    # application's in-process settings. Isolation is a guard, not a dconf
    # before/after assertion. The GTK adapter tests cover in-process settings.
    process = None
    try:
        def launch(phase):
            nonlocal process
            with (root / (phase + '.log')).open('w') as log:
                process = sp.Popen([str(binary), str(theme)], stdout=log, stderr=sp.STDOUT)
            wait(lambda: find('Sample command input'), 'Actual Release Full input missing')
            nav = wait(lambda: find('Typography'), 'Typography navigation missing')
            click(nav)
            chord(0xffe3, ord('2'))
            return wait(lambda: find('Font Size', 'spin button'), 'Actual Typography editor missing')
        size = launch('first')
        # Real GTK dropdown navigation: Greeting -> Full can re-realize VTE.
        scene = wait(lambda: find('Full', 'toggle button'), 'Scene control missing')
        assert scene.get_action_iface().do_action(0)
        wait(lambda: find('Greeting', 'label'), 'Greeting option missing')
        dump('scene-menu')
        chord(0xff57)
        chord(0xff0d)
        wait(lambda: find('Greeting', 'combo box'), 'Greeting scene did not appear')
        dump('greeting-scene')
        scene = next(node for node in descendants(find('Greeting', 'combo box'))
                     if node.get_role_name() == 'toggle button')
        assert scene.get_action_iface().do_action(0)
        wait(lambda: find('Full', 'label'), 'Full option missing')
        chord(0xff50)
        chord(0xff0d)
        wait(lambda: find('Sample command input'), 'Full did not return after Greeting')
        assert process.poll() is None
        assert size.get_value_iface().get_current_value() == before['theme']['typography']['size']
        assert size.get_value_iface().set_current_value(18.0)
        time.sleep(1)
        dump('edited')
        chord(0xffe3, ord('s'))
        wait(lambda: json.loads(theme.read_text())['theme']['typography']['size'] == 18,
             'Ctrl+S did not save the current theme')
        saved = json.loads(theme.read_text())
        expected = json.loads(json.dumps(before))
        expected['theme']['typography']['size'] = 18.0
        assert saved == expected, 'Save changed unrelated theme ownership/content'
        (root / 'saved.json').write_text(json.dumps(saved, ensure_ascii=False, indent=2))
        assert protected() == original
        first_pid = process.pid
        process.terminate()
        process.wait(timeout=10)
        time.sleep(.5)
        size = launch('reopen')
        assert process.pid != first_pid
        assert size.get_value_iface().get_current_value() == 18.0
        dump('reopened')
        assert size.get_value_iface().set_current_value(20.0)
        # A genuine independent file change after this editor loaded its source.
        external = json.loads(theme.read_text())
        external['theme']['typography']['size'] = 19.0
        theme.write_text(json.dumps(external, ensure_ascii=False, indent=2))
        external_bytes = theme.read_bytes()
        chord(0xffe3, ord('s'))
        wait(lambda: any('changed outside this window' in node.get_name() for node in nodes()),
             'Release GUI did not report the external save conflict')
        assert theme.read_bytes() == external_bytes, 'Save overwrote an external edit'
        dump('save-conflict')
        assert protected() == original
        assert process.poll() is None
        for phase in ('first', 'reopen'):
            assert not re.search(r'CRITICAL|panicked at|segmentation fault', (root / (phase + '.log')).read_text())
        (root / 'protected-after.json').write_text(json.dumps(protected(), indent=2))
        (root / 'result.json').write_text(json.dumps({
            'passed': True, 'binary_sha256': digest(binary), 'pids': [first_pid, process.pid],
            'checks': ['real Release Greeting/Full navigation', 'real Release GUI edit', 'XTest Ctrl+S', 'sparse document equality',
                       'process restart/reopen', 'external save conflict preserves bytes',
                       'startup/config/desktop sentinels unchanged', 'memory GSettings backend enforced'],
            'scope': 'No Rust test controller hooks; no Apply; SIGTERM between saved sessions'
        }, indent=2))
    except Exception:
        dump('failure')
        raise
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--theme', type=Path)
    parser.add_argument('--xvfb', type=Path, help='Private Xvfb executable (defaults to PATH)')
    parser.add_argument('--inside', type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.inside:
        return inside(args.inside.resolve(), args.binary.resolve())
    assert args.theme, '--theme must be a design file; only a private copy will be edited'
    repo = Path(__file__).resolve().parent.parent
    evidence = repo / 'target/qa/release-acceptance'
    evidence.mkdir(parents=True, exist_ok=True)
    xvfb = args.xvfb or shutil.which('Xvfb')
    assert xvfb, 'Xvfb required: pass --xvfb; this script does not install dependencies'
    root = Path(tempfile.mkdtemp(prefix='blackbox-', dir=evidence))
    source_hash = digest(args.theme)
    binary = root / 'termimochi-release'
    binary.write_bytes(args.binary.read_bytes())
    binary.chmod(0o700)
    (root / 'theme.termimochi-design.json').write_bytes(args.theme.read_bytes())
    env = os.environ.copy()
    for key in ('DBUS_SESSION_BUS_ADDRESS', 'WAYLAND_DISPLAY', 'SSH_AUTH_SOCK', 'BASH_ENV', 'ENV', 'GTK_A11Y'):
        env.pop(key, None)
    for part, key in [('home', 'HOME'), ('config', 'XDG_CONFIG_HOME'), ('data', 'XDG_DATA_HOME'),
                      ('state', 'XDG_STATE_HOME'), ('cache', 'XDG_CACHE_HOME')]:
        (root / part).mkdir()
        env[key] = str(root / part)
    for path, text in {'home/.bashrc': '# private startup sentinel\n',
                       'home/.bash_profile': '# private login sentinel\n',
                       'config/kitty/kitty.conf': 'background #010203\n',
                       'config/starship.toml': "format='ORIGINAL'\n",
                       'config/fastfetch/config.jsonc': '{"modules":["os"]}\n',
                       'data/applications/sentinel.desktop': '# private menu sentinel\n'}.items():
        target = root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)
    with tempfile.TemporaryDirectory(prefix='tm-release-', dir='/tmp') as runtime:
        env.update(DISPLAY=':98', XDG_RUNTIME_DIR=runtime, GDK_BACKEND='x11', GSK_RENDERER='cairo',
                   GSETTINGS_BACKEND='memory', G_DEBUG='fatal-criticals', GTK_IM_MODULE='simple',
                   GTK_A11Y='atspi', GDK_SCALE='1')
        print('Evidence:', root, flush=True)
        with (root / 'xvfb.log').open('w') as log:
            display = sp.Popen([str(xvfb), ':98', '-screen', '0', '1280x900x24', '-nolisten', 'tcp', '-noreset'], stdout=log, stderr=sp.STDOUT)
        try:
            time.sleep(.5)
            assert display.poll() is None, 'Private display unavailable; do not reuse another display'
            with (root / 'driver.log').open('w') as log:
                driver = sp.Popen(['dbus-run-session', '--', '/usr/bin/python3', str(Path(__file__).resolve()),
                                 '--inside', str(root), '--binary', str(binary)], env=env,
                                  stdout=log, stderr=sp.STDOUT, start_new_session=True)
                try:
                    code = driver.wait(timeout=150)
                except sp.TimeoutExpired:
                    os.killpg(driver.pid, signal.SIGTERM)
                    driver.wait(timeout=10)
                    raise
            assert digest(args.theme) == source_hash, 'Source fixture changed'
            print((root / 'driver.log').read_text(), flush=True)
            raise SystemExit(code)
        finally:
            display.terminate()
            display.wait(timeout=5)


if __name__ == '__main__':
    main()
