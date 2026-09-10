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
x11.XGetGeometry.argtypes = [c.c_void_p, window_t, c.POINTER(window_t), c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(c.c_uint), c.POINTER(c.c_uint), c.POINTER(c.c_uint), c.POINTER(c.c_uint)]
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
if mode in ("key_y", "key_n", "key_q", "key_t"):
    assert os.environ.get("DISPLAY") not in (None, ":0", ":1"), "Trial approvals are test-display-only"
    keycode = x11.XKeysymToKeycode(d, ord(mode[-1]))
    xtst.XTestFakeKeyEvent(d, keycode, 1, 0)
    xtst.XTestFakeKeyEvent(d, keycode, 0, 0)
    x11.XFlush(d)
    sys.exit(0)
if mode in ("scroll_up", "scroll_down", "scroll_left", "scroll_right", "shift_scroll_down"):
    shift = mode == "shift_scroll_down"
    keycode = x11.XKeysymToKeycode(d, 0xFFE1)
    if shift:
        xtst.XTestFakeKeyEvent(d, keycode, 1, 0)
    wheel = {"scroll_up": 4, "scroll_down": 5, "scroll_left": 6, "scroll_right": 7, "shift_scroll_down": 5}[mode]
    for _ in range(min(100, max(1, int(sys.argv[4]) if len(sys.argv) > 4 else 6))):
        xtst.XTestFakeButtonEvent(d, wheel, 1, 0)
        xtst.XTestFakeButtonEvent(d, wheel, 0, 0)
        x11.XFlush(d)
        time.sleep(0.015)
    if shift:
        xtst.XTestFakeKeyEvent(d, keycode, 0, 0)
    x11.XFlush(d)
    sys.exit(0)
if mode == "jitter":
    # Stay inside the same glyph while exercising real motion/crossing events.
    for offset in (1, -1, 2, -2, 1, 0) * 4:
        move(px + offset, py + offset)
    sys.exit(0)
if mode in ("hover", "capture"):
    time.sleep(0.15)
    if path := os.environ.get("TERMIMOCHI_INSPECT_SCREENSHOT"):
        # Capture the named test window's on-screen bounds, including native
        # popover surfaces. Capturing its backing pixmap misses those menus.
        geom_root, gx, gy = window_t(), c.c_int(), c.c_int()
        width, height, border, depth = (c.c_uint() for _ in range(4))
        assert x11.XGetGeometry(d, w, c.byref(geom_root), c.byref(gx), c.byref(gy), c.byref(width), c.byref(height), c.byref(border), c.byref(depth))
        subprocess.run(["gst-launch-1.0", "-q", "ximagesrc", f"startx={ox.value}", f"starty={oy.value}", f"endx={ox.value + width.value - 1}", f"endy={oy.value + height.value - 1}", "show-pointer=false", "num-buffers=1", "!", "videoconvert", "!", "pngenc", "!", "filesink", f"location={path}"], check=True, timeout=5)
    sys.exit(0)
button(True)
if mode == "drag":
    for offset in (10, 20, 30, 40, 50):
        move(px + offset, py)
if mode == "drag_to":
    tx, ty = int(sys.argv[4]), int(sys.argv[5])
    for step in range(1, 17):
        move(round(px + (tx - px) * step / 16), round(py + (ty - py) * step / 16))
    time.sleep(0.15)
button(False)
if mode == "double":
    button(True)
    button(False)
if mode in ("click_escape", "click_enter"):
    keycode = x11.XKeysymToKeycode(d, 0xFF1B if mode == "click_escape" else 0xFF0D)
    xtst.XTestFakeKeyEvent(d, keycode, 1, 0)
    xtst.XTestFakeKeyEvent(d, keycode, 0, 0)
    x11.XFlush(d)
