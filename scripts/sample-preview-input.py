#!/usr/bin/python3
"""Real XTest keys into an isolated GTK sample Entry; never a shell command."""
import ctypes as C
import json
import os
from pathlib import Path
import sys
import time

root = Path(sys.argv[1])
request = json.loads((root / 'input.json').read_text())
assert os.environ['DISPLAY'].split('.')[0] not in (':0', ':1')
X = C.CDLL('libX11.so.6')
T = C.CDLL('libXtst.so.6')
X.XOpenDisplay.argtypes = [C.c_char_p]
X.XOpenDisplay.restype = C.c_void_p
X.XKeysymToKeycode.argtypes = [C.c_void_p, C.c_ulong]
X.XKeysymToKeycode.restype = C.c_uint
X.XFlush.argtypes = [C.c_void_p]
X.XSetInputFocus.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_ulong]
T.XTestFakeKeyEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
d = X.XOpenDisplay(os.environ['DISPLAY'].encode())
assert d
X.XSetInputFocus(d, request['xid'], 1, 0)

def key(symbol, down):
    T.XTestFakeKeyEvent(d, X.XKeysymToKeycode(d, symbol), down, 0)
    X.XFlush(d)

def chord(*symbols):
    for symbol in symbols:
        key(symbol, True)
    for symbol in reversed(symbols):
        key(symbol, False)
    time.sleep(.12)

def text(value):
    for c in value:
        chord(0xff0d if c == '\n' else ord(c))

def checkpoint(name):
    (root / name).touch()
    deadline = time.monotonic() + 25
    while not (root / (name + '-ok')).exists():
        assert time.monotonic() < deadline, name
        time.sleep(.03)

time.sleep(.5)
text('help\n')
text('git dif')
chord(0xff09)  # finite completion
checkpoint('completed')
text('\n')
chord(0xff52)  # native history Up, not cursor escape bytes
checkpoint('history')
chord(0xffe3, ord('a'))
chord(0xffff)
if request['ime']:
    chord(0xffe3, ord(' '))
    text('nihao')
    checkpoint('preedit')
    chord(0xff0d)
    checkpoint('preedit-enter')
    chord(0xffe3, ord('a'))
    chord(0xffff)
    text('nihao')
    text(' ')
    checkpoint('committed')
    chord(0xffe3, ord(' '))
    chord(0xffe3, ord('a'))
    chord(0xffff)
    chord(0xffe3, ord(' '))
    text('nihao')
    chord(0xff1b)
    checkpoint('cancelled')
    chord(0xffe3, ord(' '))
chord(0xffe3, 0xffe1, ord('v'))
checkpoint('pasted')
chord(0xffe3, ord('a'))
chord(0xffe3, 0xffe1, ord('c'))
checkpoint('copied')
chord(0xffff)
chord(0xffe3, 0xffe1, ord('v'))
checkpoint('multiline')
text('draft')
chord(0xffe3, ord('z'))
checkpoint('undo-input')
chord(0xffe3, 0xffe1, ord('z'))
checkpoint('redo-input')
chord(0xffe3, ord('c'))
checkpoint('cancel-input')
chord(0xffe3, 0xffe1, ord('t'))
text('pwd\n')
checkpoint('new-tab')
chord(0xffe3, ord('s'))
checkpoint('save')
