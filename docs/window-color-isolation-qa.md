# Preview color isolation — 2026-09-12

Follow-up to `e3d12dd`, prompted by the user's two-file comparison. This record
is local evidence, not a remote CI result or a complete rerun of the earlier matrix.

## Cause and fix

Each workbench registered a display-wide mutable CSS provider with the same
`#termimochi-terminal` selector. The most recently registered window's rule
therefore colored older preview shells too. Workspace/DesignDocument values and
VTE's independently assigned colors did not have to change for this to happen.

Each preview shell now has a unique, window-lifetime CSS class; its provider
matches only that shell. Destroying a window unregisters its providers. The
scope does not depend on document IDs, so opening the same document twice or
changing identity through Save As cannot create a selector collision.

## Verification

All paths below are relative to `target/qa/theme-workspace/`.

- Before the fix, `window-colors-before.log` / `termimochi-regression-esiz48uk/`
  reproduced the error: window A's rendered background became `#EDE8EF` after
  opening B, instead of its own `#FBEEDF`, while A's document stayed unchanged.
- Final regression: `window-colors-final.log` /
  `termimochi-regression-u5cjyb74/`: **2/2 passed**, at 1× and 2×. Checks actual
  rendered shell pixels and inherited foreground color, separate file windows,
  edits in B, all four scenes, refresh of A, provider release after closing B,
  reopening B, and preservation of A's document snapshot.
- Related native checks: `window-colors-related.log` /
  `termimochi-regression-tusnkzed/`: **8/8 passed** (Full composition, Inspect,
  scrollbar styling, existing two-window target/scope checks, each at 1×/2×).
  Each run retains its pinned `build.json`; the final regression adds foreground
  assertions to the test build, with no intervening production-code change.
- Rust 1.92.0: format and Clippy `-D warnings` passed
  (`window-colors-clippy-final.log`); **394 ordinary tests passed**, 66 native
  tests ignored by that command (`window-colors-tests-final.log`).
- Release rebuilt (`window-colors-release.log`). Native tests use a dedicated
  Xvfb, private HOME/XDG/D-Bus and memory GSettings; no daily configuration writes.
- The rebuilt release survived the isolated 10-second startup check (expected
  timeout status 124; `window-colors-smoke.log`, `smoke-hrgD8cE2/gui.log`).

Before/after PNGs live in each regression case's `cache/window-colors-*.png`.
These are actual GTK render captures, not design mockups. The final captures
include `window-colors-a-after-open.png` and `window-colors-a-after-reopen.png`.

## Manual check

Save unfinished work and close all old App windows. Run
`./target/release/termimochi`, open a pale theme, then open a differently colored
file in another window. Edit the second window's background/foreground; switch
Full / Terminal / Prompt / Greeting; close and reopen it. The first window must
keep its own colors and document. Real-desktop user confirmation remains pending.

## Expanded multi-window audit

Result: **16/16 targeted native cases passed** (6 multi-window + 10 related,
at 1×/2×); no additional cross-window defect was observed. Format, Clippy with
`-D warnings`, and **394 ordinary tests** passed; the ordinary command ignored
68 native tests (`multiwindow-clippy.log`, `multiwindow-tests.log`). This audit
adds tests, not further production changes beyond the CSS isolation fix above.

Two additional native tests extend the original rendered-color regression:

- **Same saved file in two windows:** shared document ID but distinct live-window
  identity and CSS scope; one window switches Light/Dark, changes font size,
  padding, columns and cursor shape, imports a marked Prompt, then renames,
  undoes and redoes. The other window's document, VTE font, layout, scene and
  rendered colors stay unchanged. Save from A succeeds; B's stale Save is
  rejected without overwriting A or discarding B's unsaved edits. Reopening A
  restores its saved state.
- **Two GIF windows:** pausing A does not pause B's frame updates. Continuous
  width changes coalesce independently. Replacing A's document while an edit is
  queued clears its old artwork and preserves B. Closing B with a queued edit
  leaves A's replacement document and preview intact. This checks GTK design
  playback, not Kitty/Sixel protocol compatibility.

The related run also exercises the existing deliberately slow conversion worker
(main-loop heartbeat, stale running results, cancellation and close), import
editor cancellation/reopen, external-file verification cache invalidation,
48 rounds of extreme settings/rapid navigation, and stale Kitty trial approval
guards. The latter uses private test state, not a user's published entry.

Evidence is under `target/qa/theme-workspace/`: `multiwindow-native.log`,
`termimochi-regression-0yznz1wa/`, `multiwindow-related.log`,
`termimochi-regression-y8c5rz2s/`. Each run retains `results.json`, pinned
`build.json`, per-case logs and captures. The first run's `cache/same-file-*.png`
captures the two-window rendered-state checks.

These are targeted checks, not a complete 68-case native matrix or proof of
absence of bugs. Actual Wayland/GPU rendering, fractional/mixed-monitor scaling,
long-duration use and arbitrary third-party configurations are not certified by
these Xvfb 1×/2× results. Physical shortcut/focus behavior is not covered merely
by invoking the corresponding GTK controls or Save method in these tests.
