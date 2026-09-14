#!/usr/bin/python3
"""Release GUI -> unmodified system Ptyxis -> reviewed profile -> restore.

Owns Xvfb, D-Bus, HOME and shared keyfile GSettings. A read-only bubblewrap
namespace masks systemd-run only in this test (no private user manager).
No terminal replacement, fake launch, Rust controller or daily config writes.
"""
import argparse
import configparser
import ctypes as C
import hashlib
import json
import os
import re
from pathlib import Path
import signal
import subprocess as sp
import tempfile
import time

REPO = Path(__file__).resolve().parent.parent
TARGET = '12345678-1234-4321-9876-0123456789ab'
DECOY = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee'
GLOBAL = 'org.gnome.Ptyxis'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def wait(fn, description, seconds=30):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        result = fn()
        if result:
            return result
        time.sleep(.15)
    raise AssertionError(description)


def settings(uuid=None):
    return GLOBAL if uuid is None else f'{GLOBAL}.Profile:/org/gnome/Ptyxis/Profiles/{uuid}/'


def gs(key, value=None, uuid=None):
    args = ['gsettings', 'get' if value is None else 'set', settings(uuid), key]
    if value is not None:
        args.append(value)
    return sp.check_output(args, text=True).strip()


def inside(root):
    Path(os.environ['XDG_RUNTIME_DIR']).mkdir(mode=0o700, exist_ok=True)
    sp.run(['gdbus', 'call', '--session', '--dest', 'org.freedesktop.DBus',
            '--object-path', '/org/freedesktop/DBus', '--method', 'org.freedesktop.DBus.StartServiceByName',
            'org.a11y.Bus', '0'], check=True)
    import gi
    gi.require_version('Atspi', '2.0')
    from gi.repository import Atspi
    from PIL import ImageGrab
    assert os.environ['HOME'] == str(root / 'home')
    assert os.environ['GSETTINGS_BACKEND'] == 'keyfile'
    assert os.environ['DISPLAY'] == ':96'
    assert sp.run(['/usr/bin/systemd-run', '--version'], capture_output=True).returncode != 0
    Atspi.init()
    x11 = C.CDLL('libX11.so.6')
    x11.XOpenDisplay.argtypes, x11.XOpenDisplay.restype = [C.c_char_p], C.c_void_p
    x11.XRaiseWindow.argtypes = [C.c_void_p, C.c_ulong]
    x11.XSetInputFocus.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_ulong]
    x11.XFlush.argtypes = [C.c_void_p]
    xtest = C.CDLL('libXtst.so.6')
    xtest.XTestFakeMotionEvent.argtypes = [C.c_void_p, C.c_int, C.c_int, C.c_int, C.c_ulong]
    xtest.XTestFakeButtonEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
    display = x11.XOpenDisplay(b':96')
    assert display

    def tree(node):
        if node is None:
            return
        yield node
        try:
            for i in range(node.get_child_count()):
                yield from tree(node.get_child_at_index(i))
        except Exception:
            return

    def nodes(app_only=True):
        for app in tree(Atspi.get_desktop(0)):
            try:
                if not app_only or app.get_process_id() == editor.pid:
                    yield app
            except Exception:
                continue

    def find(name):
        for node in nodes():
            try:
                if node.get_name() == name and node.get_state_set().contains(Atspi.StateType.SHOWING):
                    return node
            except Exception:
                continue

    def click(name, node=None):
        node = node or wait(lambda: find(name), f'Missing GUI control: {name}')
        # Xvfb has no window manager/Alt+Tab. Bring only this private editor's
        # known windows forward so screenshots also show the reviewed controls.
        listing = sp.check_output(['xwininfo', '-root', '-tree'], text=True)
        windows = re.findall(r'(0x[0-9a-f]+) "([^"\n]*)"', listing)
        for title in ('Ptyxis Apply QA · Ptyxis — TermiMochi', 'Apply This Scheme', 'Scheme Application Results'):
            for xid, caption in windows:
                if caption == title:
                    x11.XRaiseWindow(display, int(xid, 16))
        x11.XFlush(display)
        # Prefer the actual toggle child over GtkMenuButton's proxy container.
        actionable = next((n for n in tree(node) if n.get_action_iface().get_n_actions()), node)
        action = actionable.get_action_iface()
        if action.get_n_actions():
            assert action.do_action(0), name
        else:
            # GtkMenuButton's labelled container has no AT-SPI action. Click
            # its real screen bounds; do not invoke a hidden controller action.
            bounds = node.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
            assert bounds.width > 0 and bounds.height > 0
            for xid, caption in windows:
                if 'Ptyxis Apply QA' in caption:
                    x11.XRaiseWindow(display, int(xid, 16))
                    x11.XSetInputFocus(display, int(xid, 16), 1, 0)
            xtest.XTestFakeMotionEvent(display, -1, bounds.x + bounds.width // 2, bounds.y + bounds.height // 2, 0)
            xtest.XTestFakeButtonEvent(display, 1, 1, 0)
            xtest.XTestFakeButtonEvent(display, 1, 0, 0)
            x11.XFlush(display)
        time.sleep(.4)

    def shot(name):
        ImageGrab.grab(xdisplay=':96').save(root / (name + '.png'))
        result = []
        for node in nodes(False):
            try:
                result.append([node.get_process_id(), node.get_role_name(), node.get_name()])
            except Exception:
                pass
        (root / (name + '-tree.json')).write_text(json.dumps(result, ensure_ascii=False, indent=2))

    def probes():
        path = root / 'shell-probes.jsonl'
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def geometry():
        # GTK4 VTE accessibility reports terminal-local coordinates as screen
        # coordinates here. Locate its native background pixels, not the header.
        match = re.search(r'rgb:(\w+)/(\w+)/(\w+)', probes()[-1]['background'])
        if not match:
            return None
        bg = tuple(int(value, 16) >> 8 for value in match.groups())
        img = ImageGrab.grab(xdisplay=':96').convert('RGB')
        pixels = img.load()
        for top in range(40, 160):
            xs = [x for x in range(img.width) if pixels[x, top] == bg]
            if len(xs) > 400:
                left, right = min(xs), max(xs)
                break
        else:
            return None
        ink = []
        for y in range(top, top + 65):
            xs = [x for x in range(left + 6, min(right - 6, left + 700))
                  if (min(pixels[x, y]) > 150 if max(bg) < 110 else max(pixels[x, y]) < 110)]
            if xs:
                ink.extend((x, y) for x in xs)
            elif ink:
                break
        if ink:
            xs, ys = zip(*ink)
            return dict(x=min(xs), y=min(ys), width=max(xs) - min(xs) + 1, height=max(ys) - min(ys) + 1,
                        method='native first-row twenty-M ink bounds', background_top=top)

    def close_terminal():
        # Only private terminal PIDs recorded in this test's probe.
        for record in probes():
            try:
                os.kill(record['shell_pid'], signal.SIGHUP)
            except ProcessLookupError:
                pass
        time.sleep(1)

    def snapshot():
        return {settings(uuid): sp.check_output(['gsettings', 'list-recursively', settings(uuid)], text=True)
                for uuid in (None, TARGET, DECOY)}

    def sentinels():
        return {str(p.relative_to(root)): digest(p) for p in sorted((root / 'home').glob('.*')) if p.is_file()}

    # Configure two profiles; the deliberately different default detects wrong-target opens.
    gs('profile-uuids', repr([TARGET, DECOY]))
    gs('default-profile-uuid', repr(DECOY))
    gs('use-system-font', 'false')
    gs('font-name', repr('Liberation Mono 12'))
    gs('interface-style', repr('light'))
    gs('default-columns', '80')
    gs('default-rows', '24')
    for uuid in (TARGET, DECOY):
        gs('label', repr('Apply QA target' if uuid == TARGET else 'Wrong target decoy'), uuid)
        gs('palette', repr('qa-baseline' if uuid == TARGET else 'qa-decoy'), uuid)
        gs('use-custom-command', 'true', uuid)
        gs('custom-command', repr(f'/usr/bin/bash --noprofile --rcfile {root}/test.bashrc -i'), uuid)
    keyfile = root / 'config/glib-2.0/settings/keyfile'
    keyfile_before = keyfile.read_bytes()
    (root / 'keyfile-before.ini').write_bytes(keyfile_before)
    before = snapshot()
    protected = sentinels()
    (root / 'settings-before.json').write_text(json.dumps(before, indent=2))
    log = (root / 'app.log').open('w')
    editor = sp.Popen([str(root / 'termimochi-release'), str(root / 'theme.termimochi-design.json')], stdout=log, stderr=sp.STDOUT)
    measurements = {}
    partial = (root / 'partial-restore').exists()
    try:
        wait(lambda: find('Use Theme…'), 'Release theme did not open')
        with (root / 'baseline.log').open('w') as native_log:
            sp.Popen(['/usr/bin/ptyxis', f'--tab-with-profile={TARGET}'], stdout=native_log, stderr=sp.STDOUT)
        wait(lambda: len(probes()) == 1, 'Stock Ptyxis did not start its baseline shell')
        measurements['baseline'] = dict(probe=probes()[-1], glyph=wait(geometry, 'Baseline glyph geometry unavailable'))
        assert probes()[-1]['profile'] == TARGET
        shot('01-baseline')
        close_terminal()
        click('Use Theme…')
        wait(lambda: find('Apply & Open Profile'), 'Application review unavailable')
        shot('02-review')
        click('Cancel')
        assert snapshot() == before, 'Cancel changed configuration'
        assert len(probes()) == 1, 'Cancel opened a shell'
        click('Use Theme…')
        click('Apply & Open Profile')
        wait(lambda: len(probes()) == 2, 'Apply did not automatically open stock Ptyxis')
        applied = probes()[-1]
        assert applied['profile'] == TARGET, applied
        assert 'e7e7/e8e8/e5e5' in applied['background'].lower(), applied
        measurements['applied'] = dict(probe=applied, glyph=wait(geometry, 'Applied glyph geometry unavailable'))
        assert measurements['applied']['glyph']['height'] > measurements['baseline']['glyph']['height'] * 1.4
        assert measurements['applied']['glyph']['width'] > measurements['baseline']['glyph']['width'] * 1.4
        assert gs('font-name') == "'Liberation Mono 20'"
        assert gs('default-profile-uuid') == repr(DECOY)
        assert snapshot()[settings(DECOY)] == before[settings(DECOY)]
        reports = list((root / 'state/termimochi/scheme-applies').glob('*/report.json'))
        assert len(reports) == 1
        report = json.loads(reports[0].read_text())
        assert report['profile_uuid'] == TARGET and report['terminal'] == 'ptyxis'
        # Installation and activation are separate receipt items. The install
        # item alone is NotEnabled; actual activation must independently pass.
        assert {item['id']: item['status'] for item in report['items']} == {
            'palette': 'NotEnabled', 'activate': 'Applied', 'typography': 'Applied'}, report
        (root / 'settings-applied.json').write_text(json.dumps(snapshot(), indent=2))
        shot('03-applied')
        close_terminal()
        if partial:
            gs('cell-width-scale', '1.5', TARGET)
            gs('interface-style', repr('dark'))  # unchanged by this theme: not owned
        click('Restore This Application…')
        shot('04-restore-review')
        click('Restore Changes')
        wait(lambda: gs('font-name') == "'Liberation Mono 12'", 'Restore did not restore font')
        if partial:
            partial_report = json.loads(reports[0].read_text())
            row = next(i for i in partial_report['items'] if i['id'] == 'typography')
            assert row['status'] == 'RestoreBlocked' and 'cell-width-scale: kept' in row['detail']
            assert 'font-name: restored' in row['detail']
            assert gs('cell-width-scale', uuid=TARGET) == '1.5'
            assert gs('cell-height-scale', uuid=TARGET) == '1.0'
            assert gs('interface-style') == "'dark'", 'Unowned setting overwritten'
            assert gs('palette', uuid=TARGET) == "'qa-baseline'"
            shot('07-partial-restore')
            (root / 'partial-report.json').write_text(json.dumps(partial_report, indent=2))
            # Returning a conflicted field to its recorded applied value permits
            # its recovery. A later edit to a completed field remains untouched.
            gs('cell-width-scale', '1.2', TARGET)
            gs('font-name', repr('Liberation Mono 20'))
            editor.terminate()
            editor.wait(timeout=10)
            editor = sp.Popen([str(root / 'termimochi-release'), str(root / 'theme.termimochi-design.json')], stdout=log, stderr=sp.STDOUT)
            wait(lambda: find('Use Theme…'), 'Release app did not reopen for recovery')
            click('Import, export and recovery')
            shot('07b-recovery-menu-after-restart')
            recovery = find('Last Application & Recovery…')
            if recovery is None:
                # GTK's stock GMenu bridge exposes anonymous menu items here.
                # This fixture's Palette menu has 16 visible rows; the screenshot
                # records the actual twelfth row. Fail on menu-structure drift.
                items = [n for n in nodes() if n.get_role_name() == 'menu item'
                         and n.get_state_set().contains(Atspi.StateType.SHOWING)]
                assert len(items) == 16 and all(not n.get_name() for n in items), len(items)
                recovery = items[11]
            click('Last Application & Recovery…', recovery)
            click('Restore This Application…')
            click('Restore Changes')
            wait(lambda: gs('cell-width-scale', uuid=TARGET) == '1.0', 'Retry did not restore remaining spacing')
            assert gs('font-name') == "'Liberation Mono 20'", 'Retry rewrote completed font'
            assert gs('interface-style') == "'dark'"
            click('Open Profile Tab')
            wait(lambda: len(probes()) == 3, 'Partial recovery profile did not reopen')
            assert probes()[-1]['profile'] == TARGET
            # The user's unowned Dark choice was deliberately retained: the
            # restored palette must use its Dark colors, not the Light baseline.
            palette = configparser.ConfigParser(interpolation=None)
            palette.read(root / 'data/org.gnome.Ptyxis/palettes/qa-baseline.palette')
            color = palette['Dark']['Background'].lstrip('#')
            expected_color = '/'.join(color[i:i+2].lower() * 2 for i in (0, 2, 4))
            assert expected_color in probes()[-1]['background'].lower()
            measurements['partial_reopened'] = dict(probe=probes()[-1], glyph=wait(geometry, 'Recovered native glyph geometry unavailable'))
            assert measurements['partial_reopened']['glyph']['height'] > measurements['baseline']['glyph']['height'] * 1.4
            assert snapshot()[settings(DECOY)] == before[settings(DECOY)]
            assert sentinels() == protected
            assert not (root / 'data/applications').exists()
            shot('08-partial-retry-native')
            (root / 'result.json').write_text(json.dumps(dict(passed=True, binary_sha256=digest(root / 'termimochi-release'),
                scenario='partial recovery + persistent field progress + actual reviewed native profile',
                measurements=measurements, pending=['daily desktop', 'Conda/custom Bash', 'Wayland', 'multi-monitor']), indent=2))
            return
        assert snapshot() == before, 'Restoration changed the baseline or decoy profile'
        assert keyfile.read_bytes() == keyfile_before, 'Restore changed explicit/unset settings or original bytes'
        click('Open Profile Tab')
        wait(lambda: len(probes()) == 3, 'Restored profile did not reopen')
        restored = probes()[-1]
        assert restored['profile'] == TARGET
        assert restored['background'] == measurements['baseline']['probe']['background']
        measurements['restored'] = dict(probe=restored, glyph=wait(geometry, 'Restored glyph geometry unavailable'))
        assert measurements['restored']['glyph']['height'] == measurements['baseline']['glyph']['height']
        assert measurements['restored']['glyph']['width'] == measurements['baseline']['glyph']['width']
        shot('05-restored')
        assert sentinels() == protected, 'Private startup sentinels changed'
        assert not (root / 'data/applications').exists()
        assert not (root / 'data/org.gnome.Ptyxis/palettes/termimochi-bbbbaaaa-1234-4abc-8def-0123456789ab.palette').exists()
        # A removed reviewed profile must not silently open the default profile.
        gs('profile-uuids', repr([DECOY]))
        time.sleep(.5)
        click('Open Profile Tab')
        wait(lambda: any('reviewed profile no longer exists' in n.get_name() for n in nodes()),
             'Missing-profile refusal was not visible')
        shot('06-missing-profile-blocked')
        assert len(probes()) == 3, 'Missing target silently opened another shell'
        gs('profile-uuids', repr([TARGET, DECOY]))
        assert snapshot() == before
        assert keyfile.read_bytes() == keyfile_before
        (root / 'keyfile-restored.ini').write_bytes(keyfile.read_bytes())
        (root / 'settings-restored.json').write_text(json.dumps(snapshot(), indent=2))
        (root / 'result.json').write_text(json.dumps(dict(passed=True, binary_sha256=digest(root / 'termimochi-release'),
            ptyxis_sha256=digest(Path('/usr/bin/ptyxis')), measurements=measurements,
            isolation='private Xvfb/D-Bus/keyfile/HOME; read-only host; systemd-run masked in mount namespace',
            scope='actual Release AT-SPI application, stock Ptyxis and real shell; not daily systemd/Wayland certification'), indent=2))
    except Exception:
        shot('failure')
        raise
    finally:
        (root / 'measurements.json').write_text(json.dumps(measurements, indent=2))
        close_terminal()
        editor.terminate()
        editor.wait(timeout=10)
        log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=REPO / 'target/release/termimochi')
    parser.add_argument('--partial-restore', action='store_true')
    parser.add_argument('--inside', type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.inside:
        return inside(args.inside.resolve())
    assert not Path('/tmp/.X11-unix/X96').exists(), 'Do not reuse another display'
    base = REPO / 'target/qa/release-acceptance'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='ptyxis-apply-', dir=base))
    print('Evidence:', root, flush=True)
    for folder in ('home', 'config', 'data', 'state', 'cache', 'runtime'):
        (root / folder).mkdir(mode=0o700)
    binary = root / 'termimochi-release'
    binary.write_bytes(args.binary.read_bytes())
    binary.chmod(0o700)
    for name in ('.bashrc', '.bash_profile'):
        (root / 'home' / name).write_text('# private sentinel, not sourced\n')
    (root / 'test.bashrc').write_text(f'HISTFILE=/dev/null\nPS1="QA $ "\n/usr/bin/python3 {REPO}/scripts/ptyxis-apply-probe.py {root}\n')
    palette = (REPO / 'themes/fog-paper-codex.palette').read_text()
    palettes = root / 'data/org.gnome.Ptyxis/palettes'
    palettes.mkdir(parents=True)
    for name, color in [('qa-baseline', '#B9D8F2'), ('qa-decoy', '#FACACA')]:
        (palettes / (name + '.palette')).write_text(palette.replace('#E7E8E5', color))
    theme = dict(schema='termimochi-design', version=3, id='bbbbaaaa-1234-4abc-8def-0123456789ab', kind='project',
        target_hint='ptyxis', components=dict(palette=dict(source=palette, light=True)),
        theme=dict(name='Ptyxis Apply QA', light=True, typography=dict(family='Liberation Mono', size=20.0),
                   layout={}, colors={}, dark_colors={}, inherit=[], prompt_enabled=False, sources=[]))
    (root / 'theme.termimochi-design.json').write_text(json.dumps(theme, indent=2))
    if args.partial_restore:
        (root / 'partial-restore').touch()
        theme['theme']['typography'].update(line_height=1.3, cell_width=1.2)
        (root / 'theme.termimochi-design.json').write_text(json.dumps(theme, indent=2))
    env = os.environ.copy()
    for key in ('DBUS_SESSION_BUS_ADDRESS', 'DBUS_STARTER_ADDRESS', 'DBUS_STARTER_BUS_TYPE', 'AT_SPI_BUS_ADDRESS', 'WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'SSH_AUTH_SOCK', 'BASH_ENV', 'ENV', 'LD_LIBRARY_PATH', 'TERMIMOCHI_PRIVATE_SESSION'):
        env.pop(key, None)
    for key, folder in [('HOME', 'home'), ('XDG_CONFIG_HOME', 'config'), ('XDG_DATA_HOME', 'data'), ('XDG_STATE_HOME', 'state'), ('XDG_CACHE_HOME', 'cache'), ('XDG_RUNTIME_DIR', 'runtime')]:
        env[key] = str(root / folder)
    env.update(DISPLAY=':96', GDK_BACKEND='x11', GSK_RENDERER='cairo', GSETTINGS_BACKEND='keyfile',
               XDG_RUNTIME_DIR='/tmp/ptyxis-apply-runtime',
               GTK_A11Y='atspi', GTK_IM_MODULE='simple', PTYXIS_PROFILE=TARGET, LIBGL_ALWAYS_SOFTWARE='1', GDK_SCALE='1')
    xvfb_path = next((REPO / 'target/qa/native-tools').glob('*/runtime/usr/bin/Xvfb'))
    with (root / 'xvfb.log').open('w') as log:
        xvfb = sp.Popen([str(xvfb_path), ':96', '-screen', '0', '1800x1200x24', '-nolisten', 'tcp', '-noreset'], stdout=log, stderr=sp.STDOUT)
    try:
        wait(lambda: Path('/tmp/.X11-unix/X96').exists(), 'Xvfb unavailable')
        # All host files read-only. Only this evidence tree is writable.
        argv = ['bwrap', '--ro-bind', '/', '/', '--bind', str(root), str(root), '--dev', '/dev',
                '--tmpfs', '/tmp', '--ro-bind', '/tmp/.X11-unix', '/tmp/.X11-unix',
                '--tmpfs', '/sys', '--dir', '/tmp/ptyxis-apply-runtime',
                '--unshare-pid', '--proc', '/proc', '--die-with-parent',
                '--ro-bind', '/usr/bin/false', '/usr/bin/systemd-run',
                'dbus-run-session', '--', '/usr/bin/python3', str(Path(__file__).resolve()), '--inside', str(root)]
        with (root / 'driver.log').open('w') as log:
            driver = sp.Popen(argv, env=env, stdout=log, stderr=sp.STDOUT, start_new_session=True)
            try:
                code = driver.wait(timeout=180)
            except sp.TimeoutExpired:
                os.killpg(driver.pid, signal.SIGTERM)
                driver.wait(timeout=10)
                raise
        print((root / 'driver.log').read_text(), flush=True)
        raise SystemExit(code)
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=5)


if __name__ == '__main__':
    main()
