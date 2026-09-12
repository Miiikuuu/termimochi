#!/usr/bin/python3
"""Actual native processes on an owned Xvfb, with private settings and bus.
Snapshots are evidence only: Casilda itself renders live Wayland surfaces.
"""
import ctypes as C
import json
import os
from pathlib import Path
import signal
import subprocess as sp
import tempfile
import time
from PIL import ImageGrab

REPO = Path(__file__).resolve().parent.parent
PREFIX = REPO / 'target/native-preview/prefix'
OUT = Path(tempfile.mkdtemp(prefix='probe-', dir=REPO / 'target/qa/native-preview'))
DISPLAY = ':98'
assert not Path('/tmp/.X11-unix/X98').exists(), 'Never reuse another display'
X = C.CDLL('libX11.so.6')
T = C.CDLL('libXtst.so.6')
X.XOpenDisplay.argtypes = [C.c_char_p]; X.XOpenDisplay.restype = C.c_void_p
X.XDefaultRootWindow.argtypes = [C.c_void_p]; X.XDefaultRootWindow.restype = C.c_ulong
X.XQueryTree.argtypes = [C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)]
X.XFetchName.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_char_p)]
X.XFree.argtypes=[C.c_void_p]
X.XSetInputFocus.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_ulong]
X.XFlush.argtypes=[C.c_void_p]
X.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong]; X.XKeysymToKeycode.restype=C.c_uint
T.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
T.XTestFakeMotionEvent.argtypes=[C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_ulong]
T.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]

def wait_for(fn, seconds=10):
    until=time.monotonic()+seconds
    while time.monotonic()<until:
        value=fn()
        if value: return value
        time.sleep(.05)
    return None

def names(d, w):
    result=[]; name=C.c_char_p()
    if X.XFetchName(d,w,C.byref(name)):
        result.append((w,name.value.decode(errors='replace'))); X.XFree(name)
    root=C.c_ulong(); parent=C.c_ulong(); children=C.POINTER(C.c_ulong)(); n=C.c_uint()
    if X.XQueryTree(d,w,C.byref(root),C.byref(parent),C.byref(children),C.byref(n)):
        ids=list(children[:n.value]); X.XFree(children)
        for child in ids: result.extend(names(d,child))
    return result

def key(d, symbol, down):
    T.XTestFakeKeyEvent(d,X.XKeysymToKeycode(d,symbol),int(down),0); X.XFlush(d)

def type_text(d,text):
    for c in text:
        shift=c.isupper() or c in '~!@#$%^&*()_+{}|:"<>?'
        if shift: key(d,0xffe1,True)
        sym=0xff0d if c=='\n' else ord(c)
        key(d,sym,True); key(d,sym,False)
        if shift: key(d,0xffe1,False)
        time.sleep(.035)

def setup(case):
    root=Path(tempfile.mkdtemp(prefix='tm-np-'))
    for name in ['home','config','data','state','cache','runtime']:
        (root/name).mkdir(mode=0o700)
    env={k:v for k,v in os.environ.items() if k not in ['WAYLAND_DISPLAY','WAYLAND_SOCKET','DBUS_SESSION_BUS_ADDRESS','BASH_ENV','ENV','PTYXIS_PROFILE','STARSHIP_CONFIG','SSH_AUTH_SOCK']}
    env.update(HOME=str(root/'home'), XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'),XDG_STATE_HOME=str(root/'state'),XDG_CACHE_HOME=str(root/'cache'),XDG_RUNTIME_DIR=str(root/'runtime'),
        DISPLAY=DISPLAY,GDK_BACKEND='x11',GSK_RENDERER='cairo',GTK_A11Y='none',GIO_USE_VFS='local',GSETTINGS_BACKEND='keyfile',LIBGL_ALWAYS_SOFTWARE='1',GTK_IM_MODULE='wayland',
        LD_LIBRARY_PATH=f'{PREFIX}/lib:{PREFIX}/usr/lib/x86_64-linux-gnu',TERMIMOCHI_PRIVATE_SESSION='1')
    env['GTK_IM_MODULE']='fcitx'
    (root/'config/fcitx5').mkdir()
    (root/'config/fcitx5/profile').write_text('[Groups/0]\nName=Default\nDefault Layout=us\nDefaultIM=pinyin\n[Groups/0/Items/0]\nName=keyboard-us\nLayout=\n[Groups/0/Items/1]\nName=pinyin\nLayout=\n[GroupOrder]\n0=Default\n')
    (root/'bashrc').write_text('PS1="NP \\w $ "\nHISTFILE=/dev/null\nprintf "%s\\n" "$$" >> '+str(root/'shell-pids')+'\n')
    command=f'/usr/bin/bash --noprofile --rcfile {root}/bashrc -i'
    profile='12345678-1234-4321-9876-0123456789ab'
    for schema,k,v in [('org.gnome.Ptyxis','default-profile-uuid',profile),('org.gnome.Ptyxis','profile-uuids',f"['{profile}']"),('org.gnome.Ptyxis','use-system-font','false'),('org.gnome.Ptyxis','font-name','Monospace 13'),
        (f'org.gnome.Ptyxis.Profile:/org/gnome/Ptyxis/Profiles/{profile}/','use-custom-command','true'),(f'org.gnome.Ptyxis.Profile:/org/gnome/Ptyxis/Profiles/{profile}/','custom-command',command)]:
        sp.run(['gsettings','set',schema,k,v],env=env,check=True,capture_output=True)
    (root/'kitty.conf').write_text(f'font_size 13\nbackground #e8e5de\nforeground #31353c\nshell {command}\nallow_remote_control socket-only\nlisten_on unix:{root}/runtime/kitty.sock\ntab_bar_min_tabs 1\nshell_integration disabled\nlinux_display_server wayland\n')
    return root,env

print('Evidence:',OUT,flush=True)
xvfb=sp.Popen([str(REPO/'target/qa/native-tools/termimochi-gif-qa.Zn3tVv/runtime/usr/bin/Xvfb'),DISPLAY,'-screen','0','1500x1000x24','-nolisten','tcp','-noreset'],stdout=sp.DEVNULL,stderr=sp.DEVNULL)
results=[]
try:
    assert wait_for(lambda:Path('/tmp/.X11-unix/X98').exists())
    d=X.XOpenDisplay(DISPLAY.encode()); assert d
    for case in os.environ.get('NP_CASES', 'ptyxis-stock,ptyxis-private,ptyxis-embedded,kitty-embedded').split(','):
        root,env=setup(case)
        if case=='ptyxis-stock': env.pop('TERMIMOCHI_PRIVATE_SESSION')
        if case.startswith('ptyxis'):
            executable='/usr/bin/ptyxis' if case=='ptyxis-stock' else str(PREFIX/'bin/ptyxis')
            argv=[executable,'--standalone','--new-window','--maximize','--title','NP native client']
        else: argv=['/usr/bin/kitty','--config',str(root/'kitty.conf'),'--start-as=maximized']
        if case.endswith('embedded'): argv=[str(REPO/'target/native-preview/probe-host'),*argv]
        with (OUT/f'{case}.log').open('w') as log:
            child=sp.Popen(['dbus-run-session','--','bash','--noprofile','--norc','-c',
                'fcitx5 -D --disable=wayland,waylandim,notificationitem,notifications,cloudpinyin & exec "$@"','native-probe',*argv],cwd=root/'home',env=env,stdout=log,stderr=sp.STDOUT,start_new_session=True)
            try:
                started=bool(wait_for(lambda:(root/'shell-pids').exists(),6))
                windows=names(d,X.XDefaultRootWindow(d))
                if started and case.endswith('embedded'):
                    w=next(w for w,name in windows if name=='TermiMochi Native Host Probe')
                    X.XSetInputFocus(d,w,1,0)
                    T.XTestFakeMotionEvent(d,-1,400,400,0); T.XTestFakeButtonEvent(d,1,1,0); T.XTestFakeButtonEvent(d,1,0,0); X.XFlush(d)
                    type_text(d,f'pwd > {root}/typed-pwd\n')
                    wait_for(lambda:(root/'typed-pwd').exists(),3)
                    key(d,0xffe3,True); key(d,0xffe1,True); key(d,ord('t'),True); key(d,ord('t'),False); key(d,0xffe1,False); key(d,0xffe3,False)
                    wait_for(lambda:len((root/'shell-pids').read_text().splitlines())>=2,3)
                    type_text(d,'echo ')
                    key(d,0xffe3,True); key(d,ord(' '),True); key(d,ord(' '),False); key(d,0xffe3,False)
                    time.sleep(.7)
                    type_text(d,'nihao')
                    time.sleep(.5)
                    ImageGrab.grab(xdisplay=DISPLAY).save(OUT/f'{case}-ime-preedit.png')
                    type_text(d,' ')
                    key(d,0xffe3,True); key(d,ord(' '),True); key(d,ord(' '),False); key(d,0xffe3,False)
                    type_text(d,f' > {root}/ime-text\n')
                    wait_for(lambda:(root/'ime-text').exists(),3)
                time.sleep(.25)
                ImageGrab.grab(xdisplay=DISPLAY).save(OUT/f'{case}.png')
                item={'case':case,'root':str(root),'argv':argv,'shell_started':started,'typed_command':(root/'typed-pwd').exists(),'ime_text':(root/'ime-text').read_text() if (root/'ime-text').exists() else None,'shell_pids':(root/'shell-pids').read_text().splitlines() if started else [],'x11_windows':windows}
                results.append(item); print(json.dumps(item),flush=True)
            finally:
                os.killpg(child.pid,signal.SIGTERM)
                try: child.wait(timeout=4)
                except sp.TimeoutExpired: os.killpg(child.pid,signal.SIGKILL); child.wait()
                # Test shells are known by their recorded IDs, never by program name.
                if (root/'shell-pids').exists():
                    for pid in (root/'shell-pids').read_text().splitlines():
                        try: os.kill(int(pid),signal.SIGHUP)
                        except ProcessLookupError: pass
        (OUT/'results.json').write_text(json.dumps(results,indent=2))
finally:
    xvfb.terminate(); xvfb.wait(timeout=4)
