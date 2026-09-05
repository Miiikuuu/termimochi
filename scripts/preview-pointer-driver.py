"""X11 pointer driver for the opt-in Rust GTK test, never a user's app window.

Requires Python 3, libX11 and libXtst. The test creates the uniquely named
window below and passes surface-local coordinates. No files are modified.
"""
import ctypes as c
import os
import subprocess
import sys
import time

title = b"TermiMochi point-to-edit test"
x11 = c.CDLL("libX11.so.6")
xtst = c.CDLL("libXtst.so.6")
x11.XOpenDisplay.restype = c.c_void_p
x11.XOpenDisplay.argtypes = [c.c_char_p]
d = x11.XOpenDisplay(None)
assert d, "X11 display required"
window_t = c.c_ulong
x11.XDefaultRootWindow.restype = window_t
x11.XDefaultRootWindow.argtypes = [c.c_void_p]
x11.XQueryTree.argtypes = [c.c_void_p, window_t, c.POINTER(window_t), c.POINTER(window_t), c.POINTER(c.POINTER(window_t)), c.POINTER(c.c_uint)]
x11.XFetchName.argtypes = [c.c_void_p, window_t, c.POINTER(c.c_char_p)]
x11.XFree.argtypes = [c.c_void_p]
x11.XFlush.argtypes = [c.c_void_p]
x11.XRaiseWindow.argtypes = [c.c_void_p, window_t]
x11.XSetInputFocus.argtypes = [c.c_void_p, window_t, c.c_int, c.c_ulong]
x11.XTranslateCoordinates.argtypes = [c.c_void_p, window_t, window_t, c.c_int, c.c_int, c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(window_t)]
xtst.XTestFakeMotionEvent.argtypes = [c.c_void_p, c.c_int, c.c_int, c.c_int, c.c_ulong]
xtst.XTestFakeButtonEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
xtst.XTestFakeKeyEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
x11.XKeysymToKeycode.argtypes = [c.c_void_p, c.c_ulong]
x11.XKeysymToKeycode.restype = c.c_uint
root = x11.XDefaultRootWindow(d)


def find_window(w):
    name = c.c_char_p()
    if x11.XFetchName(d, w, c.byref(name)):
        matches = name.value == title
        x11.XFree(name)
        if matches:
            return w
    parent, out_root, children, count = window_t(), window_t(), c.POINTER(window_t)(), c.c_uint()
    if x11.XQueryTree(d, w, c.byref(out_root), c.byref(parent), c.byref(children), c.byref(count)):
        ids = list(children[:count.value])
        if children:
            x11.XFree(children)
        for child in ids:
            found = find_window(child)
            if found:
                return found
    return None


w = find_window(root)
assert w, "Only the named test window may receive pointer events"
ox, oy, child = c.c_int(), c.c_int(), window_t()
x11.XTranslateCoordinates(d, w, root, 0, 0, c.byref(ox), c.byref(oy), c.byref(child))
x11.XRaiseWindow(d, w)
x11.XSetInputFocus(d, w, 1, 0)
mode, px, py = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])


def move(x, y):
    xtst.XTestFakeMotionEvent(d, -1, ox.value + x, oy.value + y, 0)
    x11.XFlush(d)
    time.sleep(0.05)


def button(down):
    xtst.XTestFakeButtonEvent(d, 1, int(down), 0)
    x11.XFlush(d)
    time.sleep(0.05)


move(px, py)
if mode == "hover":
    time.sleep(0.15)
    if path := os.environ.get("TERMIMOCHI_INSPECT_SCREENSHOT"):
        subprocess.run(["gst-launch-1.0", "-q", "ximagesrc", f"xid={w}", "num-buffers=1", "!", "videoconvert", "!", "pngenc", "!", "filesink", f"location={path}"], check=True, timeout=5)
    sys.exit(0)
button(True)
if mode == "drag":
    for offset in (10, 20, 30, 40, 50):
        move(px + offset, py)
button(False)
if mode == "double":
    button(True)
    button(False)
if mode == "click_escape":
    keycode = x11.XKeysymToKeycode(d, 0xFF1B)
    xtst.XTestFakeKeyEvent(d, keycode, 1, 0)
    xtst.XTestFakeKeyEvent(d, keycode, 0, 0)
    x11.XFlush(d)
