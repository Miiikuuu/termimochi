# Static pixel-image export QA

Follow-up: [native display QA](qa-native-image-display-2026-09-10.md) now verifies
Kitty's actual pixels and finds a Sixel transparency failure. The byte-contract
success recorded below must not be interpreted as complete visual compatibility.

## Scope

Greeting's existing bottom-bar Export submenu now opens **Export Image Greeting…**.
It creates explicit Kitty-direct PNG / Sixel Fastfetch bundles from an accepted
editable image. A worker prepares bounded straight-alpha pixels; a second worker
writes a fresh owner-only directory. Crop, trimming, background removal, tone,
ink and smoothing are retained. Character-only effects are intentionally omitted.

No active configuration, shell startup, source image, Greeting document or Undo
history is changed. Normal Apply and Live Preview remain ANSI. The checkerboard
is a GTK pixel preview, not a terminal capability test. GIF/APNG/animated WebP and
raw protocol import are not implemented in this step.

## Results

- Ordinary workspace tests: **315 passed**, 39 opt-in tests excluded (GUI binary
  279, CLI 4, CLI integration 5, core 27).
- Formatting, diff check, Clippy with warnings denied and release build passed.
- Desktop/resource tests: **9 passed**; metadata and all **17 embedded resources**
  matched packaged paths and bytes.
- Four targeted native/GTK regressions at 1x and 2x: **8/8 passed**, with the real
  X11 pointer driver enabled. Matrix:
  `/tmp/termimochi-regression-acj_amtz/results.json`.
- Pixel dialog coverage: missing-source/disabled guards, cancel during preparation,
  one-dialog routing, readiness and geometry, wheel motion over focused numeric
  input and protocol dropdown, both export protocols, write-time close guard,
  unchanged Greeting settings and refusal after a stale draft change.
- Core coverage: PNG transparency and exact opaque color, source immutability,
  layout bounds, private directory/file permissions, preserved imported comments,
  fields and unknown settings, existing-file protection, disabled-export refusal.
- Existing image conversion/cancel/Undo/export/restart and shared-color editing
  regressions passed at both scales. The 2x pixel dialog capture was inspected at
  `/tmp/termimochi-pixel-export.png`.

Native Fastfetch 2.57.1 emitted both expected graphics protocols and the trusted
test field using each generated bundle's relative logo path. `test-pixel-protocol.py`
runs a controlled PTY with explicit 120x40 cells / 960x640 pixels, a 10-second
deadline and bounded captured output. This is a protocol-byte contract, **not a
Kitty/Sixel visual rendering test**. The initial plain-pipe Sixel experiment fell
back to a built-in logo because character pixel geometry was unavailable; the
PTY test supplies the required geometry rather than accepting that fallback.
Only the test's fixed custom field runs; imported commands are retained in a
separate serialization-only test and never executed.

Tests use temporary XDG state, memory GSettings and a dedicated Xvfb display.
Native Wayland, real target-terminal rendering, protocol-specific transparency,
remote/SSH file transfer and the desktop folder portal were not verified. The
GUI test calls the post-selection export method with a temporary local folder.

An initial matrix launch hit the temporary-storage quota before any test ran.
Only regenerable binaries from completed runs and that partial copy were removed;
their logs and all project/user files were retained. The final matrix completed.

Release SHA-256:
`3a0e6d5ffa9dc721aeced7405e6a3c0b3474841158cdd9f079178639e8c0bb56`.
No commit or push was made for this step.
