#!/usr/bin/python3
"""XTest driver for the enabled App test. Never drives the daily display.
Receives the actual Casilda bounds from the test, not guessed editor positions.
"""
import ctypes as C
import json
import os
from pathlib import Path
import sys
import time
from PIL import ImageGrab

root = Path(sys.argv[1])
request = json.loads((root/'input.json').read_text())
display = os.environ['DISPLAY']
assert display.split('.')[0] not in (':0', ':1')
X = C.CDLL('libX11.so.6'); T = C.CDLL('libXtst.so.6')
X.XOpenDisplay.argtypes=[C.c_char_p]; X.XOpenDisplay.restype=C.c_void_p
X.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong]; X.XKeysymToKeycode.restype=C.c_uint
X.XFlush.argtypes=[C.c_void_p]
X.XSetInputFocus.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_ulong]
X.XDefaultRootWindow.argtypes=[C.c_void_p]; X.XDefaultRootWindow.restype=C.c_ulong
X.XTranslateCoordinates.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_int,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_ulong)]
T.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
T.XTestFakeMotionEvent.argtypes=[C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_ulong]
T.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
d=X.XOpenDisplay(display.encode()); assert d
def key(sym, down):
    T.XTestFakeKeyEvent(d,X.XKeysymToKeycode(d,sym),int(down),0); X.XFlush(d)
def chord(*syms):
    for s in syms: key(s,True)
    for s in reversed(syms): key(s,False)
    time.sleep(.1)
def text(value):
    for c in value:
        shift=c.isupper() or c in '~!@#$%^&*()_+{}|:"<>?'
        if shift: key(0xffe1,True)
        sym=0xff0d if c=='\n' else ord(c)
        key(sym,True); key(sym,False)
        if shift: key(0xffe1,False)
        time.sleep(.025)
def wait(path):
    deadline=time.monotonic()+30
    while not path.exists():
        assert time.monotonic()<deadline, str(path)
        time.sleep(.03)
X.XSetInputFocus(d,request['xid'],1,0)
x=C.c_int(); y=C.c_int(); child=C.c_ulong()
X.XTranslateCoordinates(d,request['xid'],X.XDefaultRootWindow(d),request['x'],request['y'],C.byref(x),C.byref(y),C.byref(child))
T.XTestFakeMotionEvent(d,-1,x.value,y.value,0)
T.XTestFakeButtonEvent(d,1,1,0); T.XTestFakeButtonEvent(d,1,0,0); X.XFlush(d)
time.sleep(.3)
text(f'cd /tmp; printf "%s:%s" "$$" "$PWD" > {root}/shell-before\n')
wait(root/'shell-before')
chord(0xffe3,0xffe1,ord('t'))
time.sleep(.7)
text(f'printf "%s" "$$" > {root}/shell-tab-two\n')
wait(root/'shell-tab-two')
# Real input method composition, not paste or Unicode injection.
text('echo '); chord(0xffe3,ord(' ')); time.sleep(.7)
text('nihao'); time.sleep(.5)
ImageGrab.grab(xdisplay=display).save(Path(request['evidence_dir'])/'native-app-candidates.png')
(root/'preedit-ready').touch(); wait(root/'preedit-captured')
text(' '); chord(0xffe3,ord(' '))
text(f' > {root}/ime-commit\n'); wait(root/'ime-commit')
# Composition cancellation must leave no accidental partial Pinyin command.
text('echo '); chord(0xffe3,ord(' ')); time.sleep(.2); text('nihao'); chord(0xff1b)
chord(0xffe3,ord(' ')); text(f'OK > {root}/ime-cancel\n'); wait(root/'ime-cancel')
# The host sets a controlled clipboard marker; native handles actual paste.
text('printf %s "'); chord(0xffe3,0xffe1,ord('v'))
text(f'" > {root}/clipboard\n'); wait(root/'clipboard')
# Keep a foreground job alive while the App updates appearance.
text(f'stty size > {root}/grid-before; printf "%s:%s" "$$" "$PWD" > {root}/live-before; top -p $$ -d 1\n')
wait(root/'live-before'); time.sleep(.5); (root/'hot-ready').touch(); wait(root/'hot-done')
chord(0xffe3,ord('c'))
text(f'stty size > {root}/grid-after; printf "%s:%s" "$$" "$PWD" > {root}/live-after\n'); wait(root/'live-after')
text('sleep 40\n'); time.sleep(.2); chord(0xffe3,ord('z'))
text(f'jobs -p > {root}/stopped-job; kill %1\n'); wait(root/'stopped-job')
chord(0xffe3,ord('s')); chord(0xffe3,ord('q'))
text(f'printf %s "$STARSHIP_CONFIG" > {root}/prompt-config\n'); wait(root/'prompt-config')
text(f"printf '\\033[2J\\033[H\\033[48;2;20;90;180m'; cat {root}/clipboard; printf '\\033[0m\\n'\n")
time.sleep(.3); (root/'copy-ready').touch(); wait(root/'copy-coordinates.json')
copy=json.loads((root/'copy-coordinates.json').read_text())
def move_to(cx,cy):
    px=C.c_int(); py=C.c_int(); child=C.c_ulong()
    X.XTranslateCoordinates(d,request['xid'],X.XDefaultRootWindow(d),cx,cy,C.byref(px),C.byref(py),C.byref(child))
    T.XTestFakeMotionEvent(d,-1,px.value,py.value,0); X.XFlush(d)
move_to((copy['x1']+copy['x2'])//2,copy['y'])
# Native triple-click selects the real line, away from client resize borders.
for _ in range(3):
    T.XTestFakeButtonEvent(d,1,1,0); T.XTestFakeButtonEvent(d,1,0,0); X.XFlush(d); time.sleep(.075)
chord(0xffe3,0xffe1,ord('c'))
time.sleep(.3); (root/'copy-done').touch(); wait(root/'copy-checked')
chord(0xffe3,0xffe9,0xffe1,0xffc9)
(root/'input-done').touch()
