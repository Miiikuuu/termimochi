#!/usr/bin/python3
"""Real release UI + optional Ptyxis background runtime, read-only host in bwrap."""
import argparse
import base64
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess as sp
import tempfile
import time

REPO = Path(__file__).resolve().parent.parent


def wait(fn, label, seconds=25):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        result = fn()
        if result:
            return result
        time.sleep(.15)
    raise AssertionError(label)


def inside(root, workbench=False, persistent=False):
    sp.run(['gdbus', 'call', '--session', '--dest', 'org.freedesktop.DBus',
            '--object-path', '/org/freedesktop/DBus', '--method', 'org.freedesktop.DBus.StartServiceByName',
            'org.a11y.Bus', '0'], check=True)
    import gi
    gi.require_version('Atspi', '2.0')
    from gi.repository import Atspi
    from PIL import Image, ImageDraw, ImageGrab
    Atspi.init()
    assert os.environ['DISPLAY'] == ':93'
    assert os.environ['HOME'] == str(root / 'home')
    fixture = root / 'background.png'
    im = Image.new('RGB', (900, 600), '#253757')
    draw = ImageDraw.Draw(im)
    draw.rectangle((450, 0, 900, 600), fill='#b77851')
    draw.ellipse((250, 100, 650, 500), fill='#86ae96')
    im.save(fixture)
    (root / 'invalid.png').write_bytes(b'not an image')
    x11, xtest = C.CDLL('libX11.so.6'), C.CDLL('libXtst.so.6')
    x11.XOpenDisplay.argtypes, x11.XOpenDisplay.restype = [C.c_char_p], C.c_void_p
    for name in ('XRaiseWindow', 'XFlush', 'XSetInputFocus', 'XKeysymToKeycode'):
        getattr(x11, name).restype = C.c_int
    x11.XRaiseWindow.argtypes = [C.c_void_p, C.c_ulong]
    x11.XSetInputFocus.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_ulong]
    x11.XFlush.argtypes = [C.c_void_p]
    x11.XKeysymToKeycode.argtypes = [C.c_void_p, C.c_ulong]
    xtest.XTestFakeKeyEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
    xtest.XTestFakeMotionEvent.argtypes = [C.c_void_p, C.c_int, C.c_int, C.c_int, C.c_ulong]
    xtest.XTestFakeButtonEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
    display = x11.XOpenDisplay(b':93')
    assert display

    def tree(node):
        yield node
        try:
            for i in range(node.get_child_count()):
                yield from tree(node.get_child_at_index(i))
        except Exception:
            pass

    def find(text, exact=True):
        for n in tree(Atspi.get_desktop(0)):
            try:
                if n.get_state_set().contains(Atspi.StateType.SHOWING):
                    if n.get_name() == text if exact else text in n.get_name():
                        return n
            except Exception:
                pass

    def focus(title):
        listing = sp.check_output(['xwininfo', '-root', '-tree'], text=True)
        found = re.findall(r'(0x[0-9a-f]+) "' + re.escape(title) + '"', listing)
        if not found:
            return False
        info = sp.run(['xwininfo', '-id', found[0]], text=True, capture_output=True)
        if info.returncode or 'Map State: IsViewable' not in info.stdout:
            return False
        xid = int(found[0], 16)
        x11.XRaiseWindow(display, xid)
        x11.XSetInputFocus(display, xid, 1, 0)
        x11.XFlush(display)
        time.sleep(.2)
        return True

    def key(sym, down):
        xtest.XTestFakeKeyEvent(display, x11.XKeysymToKeycode(display, sym), down, 0)

    def chord(*syms):
        for sym in syms:
            key(sym, 1)
        for sym in reversed(syms):
            key(sym, 0)
        x11.XFlush(display)
        time.sleep(.3)

    def type_ascii(text):
        for char in text:
            assert char.islower() or char.isdigit() or char in ' /.-'
            chord(ord(char))

    def click(name, pointer=False):
        n = wait(lambda: find(name), 'Missing control ' + name)
        actionable = next((child for child in tree(n) if child.get_action_iface().get_n_actions()), n)
        action = actionable.get_action_iface()
        if action.get_n_actions() and not pointer:
            assert action.do_action(0)
        else:
            # GTK4 reports unsupported SCREEN positions as (0, 0). Use
            # window-relative geometry and the native frame origin instead.
            b = n.get_component_iface().get_extents(Atspi.CoordType.WINDOW)
            frame = n
            while frame and frame.get_role_name() != 'frame':
                frame = frame.get_parent()
            assert frame
            f = frame.get_component_iface().get_extents(Atspi.CoordType.WINDOW)
            listing = sp.check_output(['xwininfo', '-root', '-tree'], text=True)
            match = re.search(r'(0x[0-9a-f]+) "' + re.escape(frame.get_name()) + r'".*?\d+x\d+\+(-?\d+)\+(-?\d+)', listing)
            assert match, frame.get_name()
            b.x += int(match[2]) - f.x
            b.y += int(match[3]) - f.y
            print('Pointer:', name, (b.x, b.y, b.width, b.height), 'frame', (f.x, f.y), flush=True)
            xtest.XTestFakeMotionEvent(display, -1, b.x + b.width // 2, b.y + b.height // 2, 0)
            x11.XFlush(display)
            time.sleep(.15)
            xtest.XTestFakeButtonEvent(display, 1, 1, 0)
            x11.XFlush(display)
            time.sleep(.15)
            xtest.XTestFakeButtonEvent(display, 1, 0, 0)
            x11.XFlush(display)
        time.sleep(.4)

    def shot(name):
        time.sleep(1)
        ImageGrab.grab(xdisplay=':93').save(root / (name + '.png'))
        (root / (name + '-tree.json')).write_text(json.dumps([
            (n.get_role_name(), n.get_name()) for n in tree(Atspi.get_desktop(0))
        ], ensure_ascii=False, indent=2))

    def choose(path):
        focus('Ptyxis Background Trial · Experimental')
        click('Choose Background Image…')
        wait(lambda: focus('Choose a static background'), 'File chooser')
        chord(0xffe3, ord('l'))
        field = wait(lambda: next((n for n in tree(Atspi.get_desktop(0))
                                  if n.get_role_name() == 'text' and n.get_state_set().contains(Atspi.StateType.FOCUSED)), None), 'Location field')
        field.get_editable_text_iface().set_text_contents(str(path))
        chord(0xff0d)
        wait(lambda: find(str(path)), 'File selection')

    log = (root / 'app.log').open('w')
    binary = Path(os.environ.get('TERMIMOCHI_BACKGROUND_APP', REPO / 'target/background-trial-build/release/termimochi'))
    (root / 'build.json').write_text(json.dumps(dict(
        binary=str(binary), sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        workbench=workbench, persistent=persistent), indent=2))
    if workbench:
        theme = root / 'background.termimochi-design.json'
        theme.write_text(json.dumps(dict(
            schema='termimochi-design', version=3, id='12345678-1234-4321-9876-0123456789ab',
            kind='project', target_hint='ptyxis',
            components={'palette': {'source': (REPO / 'crates/termimochi/resources/themes/fog-paper.palette').read_text(), 'light': True}},
            theme=dict(name='Background Button QA', light=True, typography={'size': 14},
                       layout={}, colors={}, inherit=[], prompt_enabled=False, sources=[],
                       background=dict(image=base64.b64encode(fixture.read_bytes()).decode(),
                                       placement=dict(contain=True, zoom=.8, x=.5, y=.5, tint=.1)))), indent=2))
        command = [str(binary), str(theme)]
    else:
        command = ['bash', str(REPO / 'scripts/run-background-trial.sh')]
    app = sp.Popen(command, stdout=log, stderr=sp.STDOUT)
    try:
        if workbench:
            wait(lambda: find('Use Theme…'), 'Workbench loaded')
            time.sleep(2)
            frame = next(n for n in tree(Atspi.get_desktop(0)) if n.get_role_name() == 'frame' and 'Background Button QA' in n.get_name())
            assert focus(frame.get_name())
            click('Use Theme…', pointer=True)
            wait(lambda: focus('Apply This Scheme'), 'Apply review')
            click('Open in Enhanced Ptyxis…', pointer=True)
            wait(lambda: focus('Enhanced Ptyxis · Background'), 'Nested trial review')
            assert not find('Choose Background Image…'), 'Must consume the saved theme image'
            assert not list(Path('/tmp').glob('tm-bg-*')), 'Review is not execution consent'
            wait(lambda: find('Ready · no files changed yet.'), 'Frozen publication review')
            shot('00-workbench-review-before-click')
            if persistent:
                entry = root / 'data/applications/io.github.miiikuuu.termimochi.ptyxis-12345678-1234-4321-9876-0123456789ab.desktop'
                versions = root / 'data/termimochi/ptyxis-background-sessions/12345678-1234-4321-9876-0123456789ab/versions'
                assert not entry.exists(), 'Review must not publish a shortcut'
                title = 'TermiMochi · Background Button QA — Enhanced Ptyxis'
                sentinel = root / 'home/.bashrc'
                original = b'touch ~/personal-startup-ran\n'
                sentinel.write_bytes(original)

                def exit_terminal(native_title):
                    assert focus(native_title)
                    type_ascii('exit')
                    chord(0xff0d)
                    wait(lambda: not list(Path('/tmp').glob('tm-bg-*')), 'Published session runtime cleanup')

                def restart_editor(env=None):
                    nonlocal app
                    if app.poll() is None:
                        app.terminate()
                        app.wait(timeout=10)
                    app = sp.Popen(command, stdout=log, stderr=sp.STDOUT, env=env)
                    wait(lambda: find('Use Theme…'), 'Editor restart')
                    time.sleep(2)
                    frame = next(n for n in tree(Atspi.get_desktop(0)) if n.get_role_name() == 'frame' and '— TermiMochi' in n.get_name())
                    assert focus(frame.get_name())

                click('Apply & Open', pointer=True)
                wait(lambda: focus(title), 'Applied native window', seconds=40)
                first = entry.read_bytes()
                first_versions = set(versions.iterdir())
                assert len(first_versions) == 1
                type_ascii('pwd')
                chord(0xff0d)
                type_ascii('touch persistent-one')
                chord(0xff0d)
                wait(lambda: (root / 'home/persistent-one').exists(), 'Published shell has real caller HOME, not temporary HOME')
                shot('10-applied-native')
                app.terminate()
                app.wait(timeout=10)
                assert focus(title), 'Closing editor must not kill an applied terminal'
                type_ascii('touch editor-closed')
                chord(0xff0d)
                wait(lambda: (root / 'home/editor-closed').exists(), 'Applied terminal remains interactive after editor exit')
                shot('11-editor-closed-terminal-alive')
                exit_terminal(title)

                # Use the actual desktop Exec dispatcher without a running App
                # or the script's TERMIMOCHI_BACKGROUND_PREFIX. Runtime metadata
                # belongs to the managed entry, not the launching environment.
                env = os.environ.copy()
                env.pop('TERMIMOCHI_BACKGROUND_PREFIX', None)
                env['BASH_FUNC_pwd%%'] = '() { touch ~/inherited-function-ran; builtin pwd; }'
                sp.run(['gio', 'launch', str(entry)], check=True, env=env)
                wait(lambda: focus(title), 'Desktop shortcut reopen')
                type_ascii('pwd')
                chord(0xff0d)
                type_ascii('touch shortcut-open')
                chord(0xff0d)
                wait(lambda: (root / 'home/shortcut-open').exists(), 'Shortcut shell input')
                assert not (root / 'home/inherited-function-ran').exists()
                shot('12-shortcut-open')
                exit_terminal(title)

                restart_editor(env)
                click('Import, export and recovery')
                # This GTK version exposes unnamed menu-item accessibles.
                # Exercise real keyboard navigation to the visible entry (not
                # an injected window action); a wrong destination fails below.
                chord(0xff50)  # Home: Save Theme
                for _ in range(11):
                    chord(0xff54)
                chord(0xff0d)
                wait(lambda: focus('Enhanced Ptyxis · Saved Entries'), 'Persistent library after restart')
                click('Open', pointer=True)
                wait(lambda: focus(title), 'Library reopen without runtime-prefix environment')
                exit_terminal(title)
                app.terminate(); app.wait(timeout=10)
                updated = json.loads(theme.read_text())
                updated['theme']['name'] = 'Background Updated QA'
                updated['theme']['background']['placement']['zoom'] = .4
                theme.write_text(json.dumps(updated))
                restart_editor()
                click('Use Theme…', pointer=True)
                wait(lambda: focus('Apply This Scheme'), 'Update review')
                click('Open in Enhanced Ptyxis…', pointer=True)
                wait(lambda: focus('Enhanced Ptyxis · Background'), 'Update enhanced review')
                wait(lambda: find('Ready · no files changed yet.'), 'Update ready')
                click('Apply & Open', pointer=True)
                updated_title = 'TermiMochi · Background Updated QA — Enhanced Ptyxis'
                wait(lambda: focus(updated_title), 'Updated appearance')
                assert entry.read_bytes() != first
                assert len(set(versions.iterdir()) - first_versions) == 1
                type_ascii('touch updated-open')
                chord(0xff0d)
                wait(lambda: (root / 'home/updated-open').exists(), 'Updated shell ready')
                shot('13-updated-native')
                exit_terminal(updated_title)
                focus('Enhanced Ptyxis · Background')
                click('Saved Entries…', pointer=True)
                wait(lambda: focus('Enhanced Ptyxis · Saved Entries'), 'Library recovery')
                click('Restore Previous…', pointer=True)
                click('Restore Previous')
                wait(lambda: entry.read_bytes() == first, 'Exact shortcut/version restoration')
                click('Open', pointer=True)
                wait(lambda: focus(title), 'Restored version reopen')
                type_ascii('touch restored-open')
                chord(0xff0d)
                wait(lambda: (root / 'home/restored-open').exists(), 'Restored shell ready')
                shot('14-restored-native')
                exit_terminal(title)
                focus('Enhanced Ptyxis · Saved Entries')
                click('Remove Entry…', pointer=True)
                click('Remove Entry')
                wait(lambda: not entry.exists(), 'Only managed shortcut removed')
                assert len(list(versions.iterdir())) == 2, 'Retain both immutable versions'
                assert sentinel.read_bytes() == original
                assert not (root / 'home/personal-startup-ran').exists()
                assert not (root / 'config/ptyxis').exists()
                shot('15-entry-removed-assets-retained')
                (root / 'results.json').write_text(json.dumps(dict(
                    passed=['review-no-writes', 'pointer-apply-and-open', 'versioned-assets', 'real-shell-home',
                            'editor-exit-survival', 'shortcut-reopen', 'no-inherited-shell-function',
                            'library-after-restart', 'no-runtime-prefix-required-for-reopen',
                            'update-new-version', 'exact-restore', 'restored-version-open',
                            'remove-entry-retain-assets', 'private-settings', 'personal-startup-unchanged-and-not-run',
                            'session-cleanup'],
                    unverified=['real desktop menu discovery/indexing', 'Wayland', 'physical IME', 'hardware GPU', 'personal Bash/Conda (not included)'],
                    visual_review='Native screenshots require separate inspection; gio launch does not verify the desktop shell menu UI.'), indent=2))
                return
            # Accessibility do_action()/emit_clicked() can bypass GTK modal
            # event routing. Only actual pointer events prove this works.
            click('Open Real Terminal', pointer=True)
            wait(lambda: focus('Ptyxis Background Trial'), 'Pointer click must open native terminal from Apply review')
            time.sleep(4)
            shot('01-workbench-native-open')
            type_ascii('touch ../shell-probe')
            chord(0xff0d)
            private = wait(lambda: next((p for p in Path('/tmp').glob('tm-bg-*') if (p / 'shell-probe').exists()), None), 'Real theme shell input')
            type_ascii('exit')
            chord(0xff0d)
            wait(lambda: not private.exists(), 'Theme trial cleanup')
            wait(lambda: find('Terminal closed.', False), 'Theme trial completion')
            focus('Enhanced Ptyxis · Background')
            click('Open Real Terminal', pointer=True)
            wait(lambda: focus('Ptyxis Background Trial'), 'Pointer restart from theme review')
            type_ascii('exit')
            chord(0xff0d)
            wait(lambda: not list(Path('/tmp').glob('tm-bg-*')), 'Second theme cleanup')
            shot('02-workbench-cleaned')
            (root / 'results.json').write_text(json.dumps(dict(
                passed=['release-workbench-theme', 'use-theme-pointer', 'nested-review-pointer',
                        'review-does-not-execute', 'open-real-terminal-pointer', 'current-theme-background',
                        'real-shell-input', 'normal-exit-cleanup', 'pointer-restart'],
                unverified=['Wayland', 'hardware GPU', 'physical IME'],
                visual_review='Screenshots require separate inspection.'), indent=2))
            return
        wait(lambda: find('Open Real Terminal'), 'Trial UI')
        focus('Ptyxis Background Trial · Experimental')
        assert not find('I understand this opens a real shell')
        assert not find('Stop Trial…')
        assert find('Opens a real shell with your user permissions. Daily settings stay unchanged.')
        assert not list(Path('/tmp').glob('tm-bg-*')), 'Opening the review must not launch a terminal'
        shot('00-single-action-review')
        click('Open Real Terminal')
        assert find('Choose a background image first.')
        choose(root / 'invalid.png')
        click('Open Real Terminal')
        wait(lambda: find('static PNG', False), 'Invalid image rejection')
        shot('01-invalid-image')
        choose(fixture)
        assert not list(Path('/tmp').glob('tm-bg-*')), 'Selecting an image is not execution consent'
        click('Open Real Terminal')
        wait(lambda: focus('Ptyxis Background Trial'), 'Native Ptyxis window')
        time.sleep(4)
        shot('02-native-cover')
        # Real finite QA input to this explicitly authorized private shell.
        type_ascii('pwd')
        chord(0xff0d)
        type_ascii('touch ../shell-probe')
        chord(0xff0d)
        private = wait(lambda: next((p for p in Path('/tmp').glob('tm-bg-*') if (p / 'shell-probe').exists()), None), 'Real shell input')
        config = (private / 'config/glib-2.0/settings/keyfile').read_text()
        assert 'org/gnome/Ptyxis' not in config
        (root / 'private-settings.ini').write_text(config)
        shot('03-native-shell')
        type_ascii('exit')
        chord(0xff0d)
        wait(lambda: not private.exists(), 'Temporary files not cleaned')
        wait(lambda: find('Terminal closed.', False), 'Completion status')
        focus('Ptyxis Background Trial · Experimental')
        # Start again and exercise cancellation and explicit process cleanup.
        click('Cover · fill the window')
        chord(0xff54)  # Down: Contain
        chord(0xff0d)
        wait(lambda: find('Contain · show the whole image'), 'Contain selection')
        slider = next(n for n in tree(Atspi.get_desktop(0)) if n.get_role_name() == 'slider')
        assert slider.get_value_iface().set_current_value(.65)
        click('Open Real Terminal')
        wait(lambda: focus('Ptyxis Background Trial'), 'Second trial')
        time.sleep(4)
        shot('04-native-contain-tint')
        private = next(Path('/tmp').glob('tm-bg-*'))
        focus('Ptyxis Background Trial · Experimental')
        click('Stop Trial…')
        click('Keep Running')
        assert private.exists()
        click('Stop Trial…')
        click('Stop Trial')
        wait(lambda: not private.exists(), 'Stop cleanup')
        wait(lambda: find('Terminal closed.', False), 'Stop completion')
        shot('05-cleaned')
        (root / 'results.json').write_text(json.dumps(dict(
            passed=['single-explicit-open-action', 'review-and-picker-do-not-execute', 'no-image', 'invalid-image', 'file-picker', 'native-start',
                    'real-shell-input', 'private-settings', 'normal-exit-cleanup',
                    'contain-selection', 'tint-control', 'stop-cancel', 'stop-confirm-cleanup'],
            visual_review='Screenshots require separate human/model inspection; not inferred from commands.',
            unverified=['Wayland', 'hardware GPU', 'IME', 'theme persistence (covered by separate workbench regressions, not this standalone picker test)']
        ), indent=2))
    except Exception:
        shot('failure')
        raise
    finally:
        if app.poll() is None:
            app.terminate()
            app.wait(timeout=10)
        log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inside', type=Path)
    parser.add_argument('--workbench', action='store_true', help='Actual pointer path through Use Theme, not the standalone picker')
    parser.add_argument('--persistent', action='store_true', help='Apply/update/restore/restart/desktop dispatcher, in private data directories only')
    args = parser.parse_args()
    if args.inside:
        inside(args.inside, args.workbench or args.persistent, args.persistent)
        return
    assert not Path('/tmp/.X11-unix/X93').exists(), 'Display already owned'
    root = Path(tempfile.mkdtemp(prefix='background-trial-', dir=REPO / 'target/qa'))
    print('Evidence:', root, flush=True)
    env = os.environ.copy()
    for key in ['WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'DBUS_SESSION_BUS_ADDRESS', 'AT_SPI_BUS_ADDRESS', 'BASH_ENV', 'ENV']:
        env.pop(key, None)
    for key, folder in [('HOME', 'home'), ('XDG_CONFIG_HOME', 'config'), ('XDG_DATA_HOME', 'data'), ('XDG_CACHE_HOME', 'cache'), ('XDG_STATE_HOME', 'state')]:
        (root / folder).mkdir()
        env[key] = str(root / folder)
    # The host GNOME portal only exposes settings on this private X11 display.
    # Select the real GTK file-chooser backend in this test's config, not daily config.
    (root / 'config/xdg-desktop-portal').mkdir()
    (root / 'config/xdg-desktop-portal/portals.conf').write_text('[preferred]\ndefault=gtk\n')
    env.update(DISPLAY=':93', XDG_RUNTIME_DIR='/tmp/bg-ui-runtime', GDK_BACKEND='x11',
               GSK_RENDERER='cairo', LIBGL_ALWAYS_SOFTWARE='1', GTK_A11Y='atspi',
               GTK_IM_MODULE='simple', GSETTINGS_BACKEND='memory', GIO_USE_VFS='local',
               GTK_USE_PORTAL='0')
    env.setdefault('TERMIMOCHI_BACKGROUND_PREFIX', str(REPO / 'target/background-runtime/prefix'))
    env['LD_LIBRARY_PATH'] = ':'.join([str(REPO / 'target/native-preview/prefix/lib'),
                                     str(REPO / 'target/native-preview/prefix/usr/lib/x86_64-linux-gnu'),
                                     env.get('LD_LIBRARY_PATH', '')])
    xvfb = next((REPO / 'target/qa/native-tools').glob('*/runtime/usr/bin/Xvfb'))
    server = sp.Popen([str(xvfb), ':93', '-screen', '0', '1600x1100x24', '-nolisten', 'tcp', '-noreset'])
    try:
        wait(lambda: Path('/tmp/.X11-unix/X93').exists(), 'Xvfb start')
        sp.run(['bwrap', '--ro-bind', '/', '/', '--bind', str(root), str(root), '--dev', '/dev',
                '--tmpfs', '/tmp', '--tmpfs', '/sys', '--ro-bind', '/tmp/.X11-unix', '/tmp/.X11-unix',
                '--dir', '/tmp/bg-ui-runtime', '--unshare-pid', '--proc', '/proc', '--die-with-parent',
                'dbus-run-session', '--', '/usr/bin/python3', str(Path(__file__).resolve()),
                '--inside', str(root), *(['--workbench'] if args.workbench else []), *(['--persistent'] if args.persistent else [])], env=env, check=True, timeout=300)
    finally:
        server.terminate()
        server.wait(timeout=5)


if __name__ == '__main__':
    main()
