# Multi-packet Kitty animation QA — 2026-09-10

## Root cause

GIF frames are encoded as PNG and split into 4096-byte base64 packets. The
exporter supplied `a=f` on the first packet of an animation frame, but omitted
it on continuation packets. The [Kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/#animation)
requires `a=f` on every packet of an animation-frame transfer. Base-image
continuations still omit the action key.

The old simple moving-rectangle fixture compressed into one packet per frame,
so earlier motion/startup passes did not cover this bug. The existing chunked
payload unit test reconstructed PNG bytes successfully but incorrectly accepted
the missing action. It now checks the action as well as lossless reconstruction.

The fix is shared by Test in Terminal, managed installation and animated Kitty
bundle export. It does not change GIF processing, frame timing or static output.

## Reproduction and verification

A synthetic 128×128 GIF contains grayscale noise plus a moving red rectangle,
forcing continuation packets without including personal artwork in the repo.

- Before the fix, the corrected unit test failed and the real Kitty trial failed
  with **Animation did not move**. Native report: `termimochi-regression-2fiyq7k9`.
- After the fix, the same trial and managed installation both showed moving
  pixels without trails, at GTK scales 1x and 2x (**2/2 cases**, four terminal
  launches). Native report: `termimochi-regression-ppnulzna`.
- Installation requires the real trial's visual confirmation; no synthetic
  approval is used in this regression. Trial files are deleted before testing
  the installed configuration, and Fastfetch runs from `/`.
- Ordinary workspace tests: **342 passed**, 50 opt-in tests excluded. Formatting
  and Clippy with `-D warnings` passed.
- Additional native/GTK matrix: **6/6 passed** at 1x and 2x, covering transparent
  GIF bundle playback, real terminal trial/protocol/fallback/install checks and
  the GIF editor's play/pause/seek/static controls. Report:
  `termimochi-regression-6h1ol4fk`.
- Release build and local installation passed; desktop metadata, all 17 embedded
  resources and packaged copies validated. Built/installed executable SHA-256:
  `a65054670c8abb1ce6cd1aeb61a59804b77b56f024c29598c054e5b48c4ef3ca`.

Tests run on a dedicated Xvfb display using Kitty 0.45.0 and Fastfetch 2.57.1,
with isolated XDG paths. No daily terminal config or shell startup file is edited.
This is not a claim that every GIF or compositor has been tested.

## Updating existing output

Restart TermiMochi and regenerate the trial using **Kitty · Animated GIF**.
Existing exported or installed `logo.kitty` files retain the old bytes: repeat
export or **Test in Terminal → Review Install… → Install & Apply** to replace
them through the normal reviewed workflow. Reimporting a retained editable GIF
source is unnecessary. Ordinary ANSI Apply remains static.
