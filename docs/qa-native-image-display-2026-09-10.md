# Native image display QA — transparency failure found

Follow-up: [the Sixel fix and unchanged-assertion retest](qa-sixel-fix-2026-09-10.md)
now pass. The findings below describe the original ImageMagick-based exporter,
not the new generated transparent Sixel bundle.

## Outcome

Actual terminal screenshots, not just emitted protocol bytes, were tested.
**Kitty static PNG and GIF rendering passed the tested cases. Sixel rendering
failed transparency checks: transparent pixels became black.** The image export
compatibility work is therefore not fully green. No production fix was made in
this diagnostic turn.

| Path | Actual display result |
| --- | --- |
| Kitty-direct PNG | Four colors, transparent outer margin and center hole pass on light/dark backgrounds. Left/80-column, right/100-column and top/120-column cases pass. |
| Kitty-direct brand PNG | Real TermiMochi icon appears without a rectangular background on both themes; screenshots manually reviewed. |
| Kitty GIF | Existing real animation test passed again; checks changing frames and absence of previous-frame residue. |
| Sixel in xterm | Four colored quadrants appear, but transparent hole and margin are `(0, 0, 0)` on both themes, for all three placements. |
| Sixel brand PNG | Icon is recognizable but enclosed in a black rectangle on both themes; screenshots manually reviewed. |

## Environment and method

- Kitty 0.45.0, Fastfetch 2.57.1, Ubuntu xterm 407-1ubuntu1.
- xterm and its missing libutempter dependency were downloaded from the configured
  Ubuntu archive and unpacked only under `/tmp/termimochi-display-qa.qJeqLQ`.
  Nothing was installed system-wide and no terminal defaults were changed.
- Local VTE 0.84.0 reports `+BIDI +GNUTLS +ICU +SYSTEMD`, without SIXEL support.
  It was not used as a Sixel renderer. xterm used explicit VT340 graphics settings,
  256 color registers and real terminal geometry responses.
- Dedicated Xvfb displays, private per-run XDG state, fixed trusted test fields,
  no user shell startup/configuration. Native Wayland, SSH and other terminal
  implementations remain unverified.
- Rust calls the actual image preparation and bundle exporter. The chart starts
  with white borders and a white center hole; existing background removal produces
  the exported transparent PNG. The second fixture is the packaged 256px app icon.
- `scripts/test-pixel-rendering.py` opens real terminals and launches Fastfetch
  with stdout connected directly to the terminal. Pillow checks rendered quadrant
  colors (tolerance 5), orientation, bounds and transparent pixel samples.
  GStreamer captures the named X11 test window. Brand captures require manual
  review; they are deliberately labeled CAPTURE, not an automated PASS.
- The helper allows 750ms for initial window mapping. These tests check steady
  state output, not instant shell-startup races, interactive resize or scrolling.

## Evidence

Primary direct-TTY, unchanged exported configuration run:
`/tmp/termimochi-regression-zap4hhjj/results.json`.

Its `1x-greeting_image-pixel_export-tests-pixel_bundles_visual_terminal_rendering/cache/pixel-visual/`
directory contains the 16 terminal captures, processed source PNGs and logs.
Six chart cases pass for Kitty; six fail for Sixel. The four brand captures were
manually inspected. The opt-in Rust test correctly remains **failed**, rather
than relaxing the transparency assertions to accept the black background.

Absolute-logo-path direct-TTY control:
`/tmp/termimochi-regression-zc7on3oh/results.json`.
It gives the same results; the application's relative bundle path is not the
cause of the observed Sixel issue. Configs were not rewritten for this control;
Fastfetch's command-line logo override was used only inside the private test.

GIF real rendering rerun:
`/tmp/termimochi-regression-e9296s_k/results.json` — animation passed; the early
static test in that same run used the superseded forwarding harness below.

## Rejected harness result

An initial helper captured Fastfetch stdout through a PIPE and forwarded it to
Kitty. This produced mostly blank static-image screenshots even with absolute
paths. Those results are **not application failures**: when Fastfetch directly
owned the terminal, all six unchanged-config Kitty chart cases passed and both
brand images appeared. The harness now uses direct TTY output by default;
`TERMIMOCHI_VISUAL_PIPE_OUTPUT=1` retains the faulty setup only for diagnosis.
The exact upstream behavior causing the forwarding discrepancy was not isolated.

## Sixel finding

The exported source PNG retains zero alpha in the tested pixels; the terminal
receives a Sixel image with an opaque black area and displays it accordingly.
The problem is on the Fastfetch/Sixel rendering path, not loss of transparency
in TermiMochi's editable source. This also matches an existing
[Fastfetch transparency report](https://github.com/fastfetch-cli/fastfetch/issues/656).
That report alone does not establish the exact current implementation cause;
the local source-to-render comparison is the evidence for this installation.

[xterm's graphics documentation](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
documents VT340 Sixel support and its background controls. A follow-up fix should
preserve transparency or explicitly offer a chosen matte, then rerun these exact
pixel assertions. A generated transparent Sixel stream is another possible route;
no such production change was attempted here.

## Other checks and reproduction

All 320 ordinary workspace tests still pass (43 opt-in tests excluded). Clippy
with warnings denied, formatting and diff checks passed. This turn added only
the opt-in visual test, its helper and QA documentation; no application rebuild
was installed and no commit/push was made.

With a dedicated Xvfb display and xterm available:

```sh
TERMIMOCHI_XTERM=/absolute/path/to/xterm \
  python3 scripts/test-regression.py --display :91 --scale 1 \
  --filter pixel_bundles_visual_terminal_rendering
```

For an unpacked xterm, point `LD_LIBRARY_PATH` at its unpacked dependency directory.
The test currently exits nonzero due to the confirmed Sixel transparency failure.
