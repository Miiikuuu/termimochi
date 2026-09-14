#!/usr/bin/python3
"""Paste a fixed QA command into an owned native test window, never the desktop."""
import ctypes as C
import json
import os
from pathlib import Path
import sys
import time

display = os.environ['DISPLAY']
assert display.split('.')[0] not in (':0', ':1')
assert os.environ.get('GSETTINGS_BACKEND') == 'memory'
request = json.loads(Path(sys.argv[1]).read_text())
X = C.CDLL('libX11.so.6')
T = C.CDLL('libXtst.so.6')
X.XOpenDisplay.argtypes = [C.c_char_p]
X.XOpenDisplay.restype = C.c_void_p
X.XKeysymToKeycode.argtypes = [C.c_void_p, C.c_ulong]
X.XKeysymToKeycode.restype = C.c_uint
X.XFlush.argtypes = [C.c_void_p]
X.XSetInputFocus.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_ulong]
X.XDefaultRootWindow.argtypes = [C.c_void_p]
X.XDefaultRootWindow.restype = C.c_ulong
X.XTranslateCoordinates.argtypes = [C.c_void_p, C.c_ulong, C.c_ulong, C.c_int, C.c_int, C.POINTER(C.c_int), C.POINTER(C.c_int), C.POINTER(C.c_ulong)]
T.XTestFakeKeyEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
T.XTestFakeMotionEvent.argtypes = [C.c_void_p, C.c_int, C.c_int, C.c_int, C.c_ulong]
T.XTestFakeButtonEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
d = X.XOpenDisplay(display.encode())
assert d
X.XSetInputFocus(d, request['xid'], 1, 0)
x, y, child = C.c_int(), C.c_int(), C.c_ulong()
X.XTranslateCoordinates(d, request['xid'], X.XDefaultRootWindow(d), request['x'], request['y'], C.byref(x), C.byref(y), C.byref(child))
T.XTestFakeMotionEvent(d, -1, x.value, y.value, 0)
T.XTestFakeButtonEvent(d, 1, 1, 0)
T.XTestFakeButtonEvent(d, 1, 0, 0)
X.XFlush(d)
time.sleep(.3)
if len(sys.argv) > 2:
    assert sys.argv[2] in ('scroll-up', 'scroll-down')
    button, count = (4, 60) if sys.argv[2] == 'scroll-up' else (5, 3)
    for _ in range(count):
        T.XTestFakeButtonEvent(d, button, 1, 0)
        T.XTestFakeButtonEvent(d, button, 0, 0)
        X.XFlush(d)
        time.sleep(.02)
    raise SystemExit(0)
for sym in (0xffe3, 0xffe1, ord('v')):
    T.XTestFakeKeyEvent(d, X.XKeysymToKeycode(d, sym), 1, 0)
for sym in (ord('v'), 0xffe1, 0xffe3):
    T.XTestFakeKeyEvent(d, X.XKeysymToKeycode(d, sym), 0, 0)
X.XFlush(d)
time.sleep(.3)
for down in (1, 0):
    T.XTestFakeKeyEvent(d, X.XKeysymToKeycode(d, 0xff0d), down, 0)
X.XFlush(d)
