# Window Top — local QA, 2026-09-12

Base: `main` at `8152343`. This is local evidence for the working-tree change,
not remote CI, a full native matrix rerun, or external-user certification.
The four untracked user briefs were preserved. No commit, push, installation to
daily directories, default-terminal change or shell-startup edit was performed.

**Ptyxis follow-up:** the initial Kitty-only record below is retained as history.
The following section supersedes its statement that Ptyxis header controls and
preview are absent.

## Ptyxis header editing and preview follow-up

Implemented **Layout → Window Top → Background / Text** and **Palette → Ptyxis
Window Top** as views of the same palette keys, with no duplicate Layout colors.
Both variants, all observation scenes, Inspect, Undo/Redo, invalid-draft rejection,
save/reopen and target/window isolation are covered. Ptyxis preview includes a
header and optional two-tab sample; actual tab shape/selection tint is governed
by Adwaita, not four imaginary editable Kitty-style colors. Missing optional
colors use a documented approximate design fallback. Menu dots are drawn as
shapes to avoid a missing-font square.

The test exposed that `select_color` previously required a main-grid swatch;
optional Ptyxis header keys were parsed/preserved but could not be selected in
that editor. Selection now accepts known palette keys without inventing a
swatch; the added Palette target exposes the two keys explicitly. Unknown keys
remain rejected.

- **408 ordinary tests passed**, 73 opt-in native tests ignored:
  `ptyxis-tests-final.log`.
- Format, Clippy all targets `-D warnings` and whitespace checks passed:
  `ptyxis-clippy-final.log`.
- **14/14 targeted native checks passed** at 1×/2×:
  `ptyxis-native-final.log`, `termimochi-regression-kwgw0o19/results.json`.
  Includes new Ptyxis header pixels/edit/save/apply/restore, prior Kitty header
  editing, nine actual Kitty tab configurations per scale, real Kitty GIF/Prompt
  reopening, shared source-color controls, Ptyxis workspace scope and multi-window
  rendered-color isolation. This is not the full native matrix.
- A final test-only extension selects **Ptyxis Window Top** through the actual
  Palette drop-down rather than directly selecting the color in code:
  **2/2 passed** at 1×/2× (`ptyxis-selector-final.log`,
  `termimochi-regression-1cmgmqgj/results.json`), with no intervening production
  behavior change. This repeats the Ptyxis case; it is not two new test functions.
- Ptyxis install testing uses private XDG and the existing Use Theme plan. A
  header-only theme must first explicitly include a full base palette; the test
  confirms this guard before exercising the same inclusion as the review dialog.
  It installs both variants, reads back their header colors, and restores the
  transaction. It does not activate a real user's profile.
- Release rebuilt: `ptyxis-release-final.log`. The isolated 10-second startup
  passed with expected timeout 124: `ptyxis-smoke.log`, `smoke-lOrT1Lve/gui.log`.
  SHA-256: `b5f6833c986f47c900a30e7d7c2c4064a7ca7f359354b177c4ab1beef66f2b24`.
- Actual GTK screenshot (not a mockup):
  `termimochi-regression-kwgw0o19/1x-window-window_top-tests-window_top_ptyxis_shared_palette_preview_and_save/cache/window-top-ptyxis-editor.png`,
  with an equivalent 2× capture.

**Environment blocked, not passed:** a separate installed-Ptyxis startup probe
used its own Xvfb :97, temporary HOME/XDG, keyfile GSettings and private D-Bus.
Its terminal displays `Failed to connect to user scope bus via local transport`.
Evidence: `ptyxis-terminal-color.log`, `ptyxis-terminal-windows.txt`,
`ptyxis-terminal-probe.png` and `ptyxis-probe-color.log`. The window can draw,
but shell startup and a complete real Ptyxis apply/open session are not verified.
This does not invalidate the App's isolated GTK rendering or private file
installation/restoration tests. GNOME Wayland decorations, exact Adwaita tab
shading, mixed-monitor scales and real-user operation remain unverified.

Earlier follow-up failures are preserved in `ptyxis-native.log` and the
`termimochi-regression-po1lrajy/` / `termimochi-regression-cicphr24/` directories.
The latter was an intentionally sparse theme correctly blocked by the existing
full-base requirement; the test now follows that requirement instead of bypassing
it or changing production safety policy.

## Results

Evidence root: `target/qa/window-top/` (ignored local artifacts).

| Check | Result | Evidence |
| --- | --- | --- |
| Rust 1.92.0 format | Passed | `cargo fmt --all -- --check` |
| Clippy, all workspace targets, `-D warnings` | Passed | `clippy-final.log` |
| Locked workspace tests | **408 passed, 0 failed**; 72 opt-in native tests ignored by this command | `tests-final.log` (372 App + 4 CLI unit + 5 CLI integration + 27 core) |
| Targeted isolated native matrix, 1× and 2× | **8/8 passed** | `native-final.log`, `termimochi-regression-fls94l86/results.json` |
| Release build | Passed | `release.log` |
| Isolated Release startup | Survived 10 seconds, expected timeout status 124, no fatal GTK/panic match | `smoke.log`, `smoke-gOJEJgWd/gui.log` |
| Whitespace check | Passed | `git diff --check` |

Release SHA-256:
`7e104d0bbb5ba9ce301b90e654271d27914ae4ef1198955bdf1ac6ef73d8b5b4`.

The native matrix pins its test and worker binaries with `build.json`. It uses
a dedicated Xvfb :96, private HOME/XDG/runtime/D-Bus and memory GSettings.
Release smoke uses :97. Neither attaches to the user's desktop session.
Development/test debug information and incremental compilation were disabled
to limit rebuilt cache growth after the recent cleanup.

## What was checked

Seven new ordinary tests cover old preset deserialization, validated native
import and source preservation, sparse theme ownership and saved identity,
style/hidden round trips, conservative titlebar capability rules, malformed
configuration rejection, and cross-target reference visibility leakage.

Four native test functions run once at each scale:

- **Window Top GUI:** Kitty color-only import; no unedited layout ownership;
  Top/Bottom edit and Undo/Redo; Powerline and all four color controls;
  invalid color draft blocks commit; design/native save and same-ID reopen;
  left-module navigation preserves scene; repeat refresh does not reorder;
  a second window's edge edit does not change the first; Ptyxis advanced controls
  and X11 titlebar recoloring are disabled. GTK screenshots are retained.
- **Real Kitty tabs:** four styles × two edges, plus hidden, i.e. nine isolated
  Kitty configurations per scale. Actual remote color values match all four
  configured colors. Captured pixels verify the minimum-one/single-tab and
  minimum-two/two-tab visibility behavior and top/bottom location. X11 output
  omits requested titlebar color and explicitly reports it as not applied.
  Test-only remote control is confined to a private socket; the second shell
  uses `bash --noprofile --norc`.
- **Existing controlled Kitty GIF + Prompt + reopen:** real animation/session
  regression still passes.
- **Existing multi-window rendered-color isolation:** opening/editing/reopening
  another file and switching observation scenes do not recolor the first window.

Useful native captures under `termimochi-regression-fls94l86/`:

- `1x-window-window_top-tests-window_top_native_edit_save_reopen_and_scope/cache/window-top-tab-colors.png`
- `1x-window-window_top-tests-window_top_native_edit_save_reopen_and_scope/cache/window-top-reopened-bottom.png`
- Equivalent `2x-…` paths for scaled GTK captures.
- `1x-kitty_session-tests-window_top_real_kitty_styles_edges_and_colors/cache/window-top-kitty/`
  and its `2x-…` counterpart hold actual Kitty one/two-tab captures for all cases.

## Failures found and resolved

The first real-Kitty harness incorrectly serialized an `OsStr` program as a JSON
object instead of a string; fixed in the harness. More importantly, the GUI
test exposed a production reference bug: a Kitty theme without explicit tab
visibility inherited the current Ptyxis reference's `false`, so selecting
Powerline emitted `hidden`. Kitty preview projection now starts with its own
chrome defaults before explicit native/component/theme fields are overlaid.
These defaults remain unowned; explicit imported hidden still works. Both pure
and native regression checks now pass. Earlier logs (`native.log`,
`native-rerun.log` and preceding regression directories) retain failed evidence.

The first restricted workspace-test run could not create the existing Starship
renderer sandbox. Rerunning with permission to create that sandbox passed; no
unsandboxed renderer fallback was added. This is separate from native X11 results.

## Not verified / not implemented

- **Not verified:** real GNOME Wayland client-side titlebar appearance. The
  capability/serialization/gating paths are tested; Xvfb cannot verify this
  environment's decorations. No actual titlebar-color success is claimed.
- **Not verified:** other compositors, physical input/focus on the user's desktop,
  fractional or mixed-monitor scales, custom fonts' exact tab metrics, long runs
  and arbitrary third-party configurations. No external usability tester.
- **Not implemented:** Ptyxis equivalents of these new background modes and
  native tab-style/color controls, newer Kitty vertical tab bars or executable
  custom tab renderers. Existing Ptyxis palette titlebar colors are retained.
- **Approximation only:** App-drawn tab/title chrome. Real Kitty tests above do
  not imply the GTK drawing is pixel-identical or certify Kitty/Sixel image
  support for every terminal.

## Repeat

Use the CI Rust 1.92.0 toolchain for `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked` and `cargo build --release --locked`.
With a **dedicated test Xvfb**, run:

```sh
python3 scripts/test-regression.py --display :96 --scale 1 --scale 2 \
  --filter window_top \
  --filter controlled_kitty_real_gif_prompt_and_reopen \
  --filter theme_windows_keep_rendered_preview_colors_local
```

Do not use the user's display. Runtime prerequisites include installed Kitty,
Fastfetch, Starship, Python/Pillow and the existing sandbox tools; no packages
were automatically installed this turn. Startup and user-facing steps are in
the [Chinese acceptance card](window-top.md#中文验收卡).
