# Complex Greeting content reachability

Scope: the three user screenshots from 2026-09-12 at 19:38:13, 19:38:24 and
19:38:36, and the saved **小猫测试** Kitty theme. Base: `d44937a` on `main`.
No theme-model, window-decoration or system-import redesign; no daily config
writes, installs, commits or pushes.

## Reproduction and measured baseline

The three actual PNGs under `~/Pictures/Screenshots` were opened and inspected.
The theme was found at `~/Desktop/design.termimochi-design.json`; its SHA256 is
`283f0aaf3c14bfcb01826eedd834054fa3abcec0b91f75a167c474836522b13e`.
A byte-identical private copy is retained in
`target/qa/reachability/reproduction.termimochi-design.json`. It includes the
actual GIF and original Fastfetch module list, not a simple substitute Greeting.
The theme file and artwork are not added to the repository.

The saved theme explicitly owns font size 16 pt, background `#EDE8EF`, the
enabled Prompt and Greeting, and animation occupancy 32 columns. Its sparse
layout is empty. Read-only GSettings checks found CaskaydiaCove Nerd Font 11,
cell width/height scale 1/1, default grid 80×24, but restore-window-size enabled
with saved 132×41. Existing App limits project the latter to **120×36**. These
reference values, padding 6 and window spacing 18 were reproduced in a private
reference Workspace; they were not authored into the user's theme.

An initial isolated run used a fallback font (20×30 cells, Fit 21.8%), so it is
**not** screenshot-matched evidence. The exact regular font was copied into a
private Fontconfig directory under QA, without installing it. Font SHA256:
`fae37ce4fdc965ba5d3caea122b856418dc5dec2e4e638a024c49d681ffce5e9`.
The corrected baseline below uses 13×25 cells and reproduces the reported 33%.

| Measurement | Fit | 100% fixed grid | Actual size |
| --- | --- | --- | --- |
| Theme font / VTE font scale / cell scales | 16 pt / 1 / 1×1 | Same | Same |
| Character size before observation transform | 13×25 logical pixels | Same | Same |
| Target grid | 120×36 | 120×36 | 120×36, unchanged reference |
| Suggested target surface, including top/padding/rail | 1596×950 logical pixels | Same | Same |
| Observation scale | 0.3327067669 (33.27%) | 1 | 1 |
| Actual App allocation | 1080×780 | Same | Same |
| Preview canvas | 603×495 at (459,164) | Same | Same |
| Visible target surface | 531×316.07 | 531×423 | 531×423 |
| Body viewport | ~523.02×304.76 displayed; 1572×916 unscaled | 507×389 | 507×389 |
| Observation columns | 120 | 120 | 38 |
| GIF reservation | 32×15 cells = 416×375 unscaled | Same | Same |
| Initial information | Beside image, first column 35 | Same, mostly off the right edge | Below image (top fallback) |
| Initial vertical range | 0…0 | 0…323 | 0…723 |
| Initial horizontal range | 0…0 | 0…1065 | 0…0 |

Physical row positions (zero-based): Fit/Fixed information starts at (column
35, row 0), Prompt at rows 25/26. Actual information starts at (0,16), after
the unchanged 15-row GIF and its gap; pre-fix Prompt was at rows 41/42 and
the GTK input rectangle was (527,1290,468,25), below the initial viewport.

Ranges are `upper - page_size - lower`, not raw upper bounds. VTE transcript
height and absolute cursor row are distinct from the fixed target window size.
Measurements are logical GTK coordinates, not dimensions inferred from resized
images. The screenshot's collapsed **Details** state was reproduced; earlier
expanded-details probes had a smaller 389→293-pixel body viewport and are retained
separately, not mixed into this table.

Matched pre-fix GTK evidence and detailed geometry:

- [Fit](../target/qa/reachability/termimochi-regression-vlh0hc10/1x-window-preview_reachability_tests-preview_complex_greeting_reachability/cache/fit-initial.png), [full measurement](../target/qa/reachability/termimochi-regression-vlh0hc10/1x-window-preview_reachability_tests-preview_complex_greeting_reachability/cache/fit-initial-reach.json).
- [Fixed grid](../target/qa/reachability/termimochi-regression-vlh0hc10/1x-window-preview_reachability_tests-preview_complex_greeting_reachability/cache/fixed-initial.png), [full measurement](../target/qa/reachability/termimochi-regression-vlh0hc10/1x-window-preview_reachability_tests-preview_complex_greeting_reachability/cache/fixed-initial-reach.json).
- [Actual size](../target/qa/reachability/termimochi-regression-vlh0hc10/1x-window-preview_reachability_tests-preview_complex_greeting_reachability/cache/actual-initial.png), [full measurement](../target/qa/reachability/termimochi-regression-vlh0hc10/1x-window-preview_reachability_tests-preview_complex_greeting_reachability/cache/actual-initial-reach.json).

## Findings and changes

1. **Fit 33% is correct in this reproduction.** Available width 531 divided by
   target width 1596 determines the scale; height is not the limiting dimension.
   Font, grid and artwork occupancy were not changed to enlarge the picture.
2. **Fixed-grid content existed and wheel panning worked before the fix.** The
   problem was poor discoverability with the theme scrollbar off, plus an actual
   input-focus fault found by pointer testing: focusing the 1534-pixel-wide Entry
   could align its far edge and leave its start at x=-532, outside the viewport
   x=495…1002. A string-presence assertion would have missed this failure.
3. **Actual size switched to image-above-information as designed.** Its existing
   vertical range reached the Prompt, but long information values had already
   been clipped during projection (for example CPU ended at `i7-13`). No amount
   of scrolling could recover those omitted characters.
4. **Full now wraps complete bounded information.** The existing native-output
   parser first projects up to its 240-column safety bound, then the existing
   Greeting renderer wraps Full's reserved-image information column, preserving
   SGR and grapheme clusters. Native exports and non-reserved output keep their
   original behavior. No imported module is hidden to pass this check.
5. **Editor-owned overflow navigation.** Horizontal/vertical scrollbars live in
   the existing canvas gutters, outside the rounded/scaled terminal surface.
   The rails appear only on overflow, independently of theme Scrollbar, with a
   **Jump to sample input** toolbar action (disabled without overflow, with a
   stable toolbar allocation). Full's duplicate internal horizontal
   rail is not also drawn. This navigation never enters the saved theme.
6. **One input-following policy.** Full disables GtkViewport's whole-widget
   focus scrolling and follows GTK's actual insertion rectangle in the existing
   unscaled scroll coordinates. Typing/submission opts into following; deliberate
   browsing preserves horizontal and vertical position across color changes.
   Tab horizontal position joins the existing transient Session state, not the
   theme. The existing GTK entry, IME, VTE and finite command reducer remain.
7. **Oversized image occupancy contributes to the scrollable canvas width.**
   Actual size can reflow text to fewer columns than the fixed image reservation;
   it must not silently clip the image's right side. This is a view-only extent,
   not a change in Greeting Columns.

GTK documents whole-widget focus scrolling as enabled by default and provides
character-indexed cursor rectangles in widget coordinates; the fix uses these
existing mechanisms, not a second input renderer.
[GtkViewport focus scrolling](https://docs.gtk.org/gtk4/property.Viewport.scroll-to-focus.html),
[GtkText insertion geometry](https://docs.gtk.org/gtk4/method.Text.compute_cursor_extents.html).

## Evidence levels and automated acceptance

- **Source/unit checks:** finite parsing and native sanitization retained;
  Unicode/combining/emoji and SGR wrap-tail tests; no sparse-theme field or export
  change for observation settings.
- **Actual isolated GTK:** uses the saved complex theme and copied font, requested
  1090×790 window yielding 1080×780 allocation. Real XTest wheel events traverse
  a grid of view positions. Every nonblank text cell and every image reservation
  cell must be fully inside the allocated body viewport in at least one captured
  frame. The test records coverage and missing cells, not merely `contains(OS)`.
  Per-row VTE extraction preserves physical soft-wrap rows rather than treating
  a soft-wrapped Prompt as one long logical line outside the terminal grid.
  Native information-tail preservation is a separate string assertion in addition
  to this geometry/pointer/screenshot evidence.
- **Actual input:** a real pointer activates the jump action, then clicks the
  mapped Entry and types `help` through XTest. The sample history, actual input
  rectangle, scroll ranges and screenshot are checked. Scrollbar browsing and
  subsequent background-color changes must retain both adjustments.
  A separate long CJK/ASCII draft checks GTK's actual insertion rectangle at
  both ends; this geometry check uses GTK text APIs, not simulated IME input.
- **Additional stress:** Actual narrows only the editor divider until the
  observation grid is narrower than the original 32-column image; the same
  pointer/coverage scan must reach its right edge without editing font or artwork.
- **Not real terminal certification:** Full remains a GTK design composition.
  Native Kitty/Ptyxis and real Fcitx regressions are run separately and do not
  turn simulated output into a protocol-support receipt.

The reproduction tests require `TERMIMOCHI_REACHABILITY_THEME` pointing to the
private theme copy and `FONTCONFIG_FILE` pointing to its private font config.
Run with the repository's locked Rust 1.92 toolchain and local native prefix:
`python3 scripts/test-regression.py --display :96 --scale 1 --scale 2 --filter preview_complex_greeting`.
Never target the daily `:0`/`:1` display. Other GUI regressions do not require
the user theme fixture. Raw evidence remains in ignored `target/qa/reachability`.
The runner records private-fixture cases as **unverified** in `unverified.json`
when that fixture is not supplied; they never count as passes. An explicit
complex-only run without the fixture exits nonzero with no verified tests.
This guard was exercised in `termimochi-regression-l2iom4gd`; both changed Python
scripts also passed `py_compile`. The actual acceptance matrix supplies the fixture.

## Final build checks

[Dual-build results](../target/qa/native-preview/build-checks-htay1j63/results.json):
10/10 gates passed — fmt, default/enabled Clippy with `-D warnings`,
default/enabled workspace tests, default/enabled Release, isolated installer,
desktop-resource unit tests and desktop/embedded-resource validation.
Workspace tests: **418 passed, 0 failed** in each build (382 App + 4 CLI +
5 integration + 27 core). Ignored GUI/native cases: 87 default / 96 enabled;
these counts are not passes. Selected ignored cases are actually executed in
the separate matrices below. No dependency, toolchain or CI workflow was changed.

## Final default GTK matrix and screenshots

[Results](../target/qa/reachability/termimochi-regression-f6moauf8/results.json):
**30/30 passed** (15 cases at 1× and 2×), zero test failures or fatal GTK
criticals. This includes three complex-theme modes; the same-theme geometry
regression; five interactive sample regressions (GIF anchors, Actual/Fit input,
Ptyxis without context and workbench state); Full live composition; independent
terminal scrolling; read-only system import; and three target-top regressions,
including real Kitty styles. Binary hashes for the pinned test/worker executables
are in this run's `build.json`.

| Complex theme traversal | 1× frames | 2× frames | Unreachable cells |
| --- | --- | --- | --- |
| Fit | 1 | 1 | 0 / 0 |
| Fixed 100% | 15 | 15 | 0 / 0 |
| Actual, 38 columns | 5 | 5 | 0 / 0 |
| Extra narrow Actual, 21 columns | 16 | 16 | 0 / 0 |

Each scan covers 1,094 cells at 1× and 1,095 at 2× (current machine-fact digits
can change between runs). These are nonblank physical text cells plus every
cell of the 32×15 image reservation, not a screenshot similarity score.
The initial viewport and target dimensions remain identical across both scales
in logical coordinates. Every scan frame has a PNG plus widget/scroll JSON.

Selected final **actual GTK** evidence, matched to the pre-fix links above:

- [Fit whole window](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_fit/cache/fit-initial.png), [geometry](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_fit/cache/fit-initial-reach.json).
- [Fixed: pan to information](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_fixed/cache/fixed-scan-0-1.png), [next horizontal page](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_fixed/cache/fixed-scan-0-2.png), [bottom Prompt](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_fixed/cache/fixed-bottom-left.png), [coverage](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_fixed/cache/fixed-coverage.json).
- [Actual: complete wrapped tails and Prompt](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_actual/cache/actual-bottom-left.png), [initial geometry](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_actual/cache/actual-initial-reach.json).
- [Actual: long CJK draft at end](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_actual/cache/actual-long-draft-end.png), [start](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_actual/cache/actual-long-draft-start.png).
- [Narrower than artwork: pan to right edge](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_actual/cache/actual-extra-narrow-scan-0-1.png), [full traversal coverage](../target/qa/reachability/termimochi-regression-f6moauf8/1x-window-preview_reachability_tests-preview_complex_greeting_actual/cache/actual-extra-narrow-coverage.json).

The agent inspected rendered PNGs, including information tails, the input row,
the image's panned edge and native examples; this is separate from automated
geometry assertions and is not external human acceptance.

## Iteration failures retained

- `termimochi-regression-5_b1b_2g`: Fixed failed actual pointer-input reachability;
  this exposed GTK whole-Entry focus scrolling. Actual/Fit passed then-limited
  checks. The failure was repaired in production, not waived.
- `termimochi-regression-jti8vagi`: 22/28 passed, 6 failed. Scrollbar gestures
  claimed by GtkRange cancelled the observer's release; use a non-consuming
  raw-event observer instead. Showing the jump button only on overflow changed
  the toolbar requisition and Fit; keep its allocation stable instead.
- `termimochi-regression-2qkvukge`: 4/5 passed. Additional 21-column Actual test
  exposed a coverage-harness error: `text_format` joins soft-wrapped Prompt rows.
  The physical Prompt was present; the harness now extracts physical VTE rows
  before testing allocated cell visibility. It still traverses with real wheel
  input and requires zero unreachable cells; no missing cells are excluded.

## Enabled native / IME regression

[Results](../target/qa/native-preview/termimochi-regression-native-gur4514d/results.json):
**8/8 passed**, separately from the default GUI matrix. At both 1× and 2×,
run actual GTK sample keyboard/IME in Actual and Fit, and enabled native Kitty
and Ptyxis sessions. Existing assertions cover real Fcitx preedit/candidates,
commit/cancel, clipboard, two native shells, hot updates and stop/reap.
This is the isolated X11 host / nested Wayland SHM environment, not the daily
Wayland desktop and not the user's complex GIF native-protocol acceptance.

- [Kitty, native hot-update/clipboard](../target/qa/native-preview/termimochi-regression-native-gur4514d/1x-native_interactive_kitty_enabled/cache/native-app-after-hot.png).
- [Ptyxis, two native tabs and real preedit](../target/qa/native-preview/termimochi-regression-native-gur4514d/1x-native_interactive_ptyxis_enabled/cache/native-app-ime-preedit.png).

The native screenshots retain the old experimental layout and the fixture's
contrast warning; passing input/lifecycle assertions is not visual approval.

## Post-fix measured geometry (same window and original theme)

Fit remains **33.27067669%**, 120 observed columns, 32×15-cell GIF and no
initial overflow. Fixed remains 100%, 120 columns and horizontal range 0…1065.
Actual remains 100%, 38 columns, 507×389 body viewport. Its complete wrapped
information now yields 49 transcript rows and a vertical range **0…873**
(upper 1262, page 389), instead of the clipped baseline's 0…723.
Actual's Prompt occupies physical rows 47/48 (zero-based); initial GTK input
y=1440 is intentionally below the viewport, and is reachable by scrolling or
the jump action. After entering `help`, the long-draft insertion row is visible
at y=597…622 inside viewport y=234…623.

Additional editor-divider-only stress: 21 observation columns; original GIF
still 32×15 cells, scroll canvas width 428, page 294, horizontal range 0…134.
No theme font/grid/artwork mutation is used to reach the image's right edge.

The source user's file was rehashed unchanged after testing. Default executable
is restored at `target/release/termimochi` (not the native-feature build), SHA256
`79b943d33af69a599b41c859539714ef4f662fc13aceca683bf91de512df2951`.
It matches the pinned default Release and runs `--help` without the private
native library path. Start with `./target/release/termimochi`; state-isolated
launch and its inherited-value caveat are in the acceptance card.
The actual default Release also survived a 10-second isolated X11 GUI smoke
using the private theme copy (expected timeout status 124, no fatal/critical
log entries): [smoke log](../target/qa/reachability/release-smoke.log).
This launch check does not replace the screenshot-matched geometry fixture.

## Remaining boundaries

- The saved complex GIF is used for geometry/pointer/input scans. Separate
  existing GIF playback/anchor regressions exercise animation, clear, scrolling,
  font changes and tabs; a screenshot alone is not a playback test.
- Information comes from the existing isolated Fastfetch projection. Machine
  facts such as memory/uptime and terminal identity can differ from the daily
  screenshot; the original module list and artwork remain intact.
- Full information projection keeps the existing 240-column native-output
  safety bound and 900-row transcript budget. This is not unlimited scrollback
  or a promise to preserve arbitrarily large imported output without a limit.
- Daily Wayland, fractional scale, multiple monitors, IBus and human usability
  acceptance remain **unverified**, not passed by Xvfb / integer-scale checks.
- Native SHM regression does not fix/certify hardware DMA-BUF. Stock Ptyxis's
  private-bus/systemd scope limitation remains environment-blocked; the native
  runner uses the existing private patched build. Primary selection/non-text
  clipboard and complete Kitty runtime drift inspection remain unimplemented.
- The earlier complex-Greeting **native Kitty** truncation report is still open;
  this task fixes the **design Full** projection, not the native protocol path.
  Keep the [native limitations ledger](native-preview-qa.md#open-implementation--environment-items).
- No online CI, package matrix or RustSec audit was run this turn. No push means
  there is no new online CI result to equate with these local logs.

Chinese operations: [manual acceptance card](preview-reachability-acceptance-zh.md).
