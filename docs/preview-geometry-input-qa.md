# Preview geometry and inline input

Scope: `TermiMochi_Preview_Geometry_Input_Fix.md`, base `c2082740ad79d7cef35abfe28d0f0ab86df5e042`.
This is a local change to the GTK preview, not a theme-model or native-terminal rewrite.
No daily configuration, shell startup, default terminal, installation, commit or push
was changed during implementation. User task briefs remain untracked and untouched.

## Changes

- Default **Actual size** keeps the selected font and derives observation columns
  from the unscaled content viewport and VTE cell width. This never writes Layout
  columns/rows, Greeting Columns or design dirty state.
- **100% · fixed grid** keeps the saved initial columns, with local horizontal
  scrolling when needed. **Fit window**, **75%**, **50%** use a single GTK allocation
  transform for the bounded simulated window. Its target height comes from initial
  rows, measured chrome and padding, not the number of history lines.
- `PreviewFrame` is a small GTK sizing/transform widget, with no design data.
  GTK performs snapshot, picking and IME coordinate translation through the same
  allocation. The existing VTE/image content scroller stays inside the window;
  its adjustment remains in logical coordinates.
- Existing GtkEntry/GtkText now follows the final VTE Prompt in that scroller.
  Padding and cursor cell position determine placement. Contents/cursor/adjustment
  updates coalesce to reposition after asynchronous VTE reflow. There is no second
  fake VTE input cursor and no fixed-bottom input form. Long commands retain GTK
  single-line internal scrolling, IME, selection and undo.
- Color-only updates do not refeed or restore rounded scroll anchors. Actual
  reflow restores the existing semantic anchors; typing/commands follow input.
  Entry text/preedit/selection is not reset by theme refreshes. Tab remounts still
  clear the GTK edit-undo stack, not per-tab drafts or command histories.
- Sample selection lives outside the simulated window; new/close/select tab
  controls share the existing reducer and live in its top area. No pixel animation
  means no Play GIF action. Small warning reports are expandable; errors are kept
  expanded and cannot be collapsed by that control.

Preserved: finite commands, per-window/tab state, Full/Terminal/Prompt/Greeting,
VTE, GIF/processing caches, Inspect, native experimental modes and their known
issues, Try/Use/Save/source-preservation/backup/conflict/recovery boundaries.

## Temporary-area cleanup

Before editing, `/tmp` (a 7.4 GiB tmpfs) held about 1.5 GiB. `lsof` showed no use
of the twelve previous `termimochi-regression-*` report directories. They were
**moved, not deleted**, into `target/qa/preview-geometry-tmp-archive/`, retaining
screenshots, logs and pinned binaries. `/tmp` then reported 244 MiB used, 7.2 GiB
available. Other programs' files and the user's running App were not touched.
Old `/tmp` references in the preceding QA ledger resolve beneath that archive now.
New default GUI reports use disk-backed `target/qa/preview-geometry-runs/` as TMPDIR.

## Before/after method

The three user screenshots (2026-09-12 16:08:23, 16:08:39, 16:08:46) were found
under `Pictures/Screenshots` and actually viewed. Their diagnosis was reproduced
with a deterministic **Ptyxis Geometry fixture**, same font size 13, background
`#FAEDDD`, foreground `#252B33`, same `help` / `git diff` history and `git status`
draft. This is an equivalent controlled fixture, not a claim to have recovered
the user's unsaved theme or exact command history.

Before code changes, the GTK test captured large Fit, small fixed 100%, small Fit
at requested 2048×1126 and 1130×830, at both device scales:
`target/qa/preview-geometry-runs/termimochi-regression-808mk2xz/` (2/2 capture cases).
Each per-case cache contains `before-{large-fit,small-100,small-fit}.png` and JSON.
Actual App allocations differ from requested CSD surface dimensions (for example
2038×1116 and 1120×820 at 1×); JSON records actual values rather than renaming them.

After captures use the **same fixture and requests**, plus Actual size, clear,
Fit selection/tabs and long-history assertions. JSON records App allocation,
viewport/title/tabs/input/VTE/toolbar bounds, cell metrics, grid, cursor,
observation scale, scroll range, theme columns and Greeting columns. Assertions
compare input origin with VTE's actual cursor to within 1.5 logical pixels and
check the shared window transform to within 2 pixels (allocation rounding).
This is a measured GTK alignment check, not an arbitrary image-similarity score.

Early measured corrected run: at Actual size, large→small changed 150→58
observation columns while cells stayed 10×22, font stayed 13, saved columns stayed
80 and Greeting artwork columns stayed 32. Later toolbar/rail measurements can
change the available grid; final JSON is authoritative.

Final 1× measurements (same fixture; viewport bounds are displayed pixels):

| Mode / actual App | Viewport | Cell before transform | Observation grid (columns × full visible rows) | Scale |
| --- | --- | --- | --- | --- |
| Actual size / 2038×1116 | 1515×672 | 10×22 | 150×30 | 1.000 |
| Actual size / 1120×820 | 597×376 | 10×22 | 58×16 | 1.000 |
| Fixed 100% / 1120×820 | 597×376, horizontally scrollable | 10×22 | 80×16 | 1.000 |
| Fit window / 1120×820 | 584.904×394.738 | 10×22 | 80×24 | 0.720325 |

Full visible rows exclude content padding and are computed in logical viewport
coordinates. JSON `grid` is VTE's **transcript canvas** allocation, not the finite
viewport: it may grow with history while the observation window does not.
For small Fit the input origin is `(528.872, 471.180)`; the VTE origin is
`(471.246, 201.779)` and its cursor is column 8, visible row 17. Applying the
same 0.720325 transform to 10×22 cells yields the input origin. The unscaled
34-pixel top row measures 24.491 pixels after Fit; the 22-pixel input line
measures 15.847 pixels. External toolbar height stays 30 pixels.

## Checks and evidence

Build gates passed in `target/qa/native-preview/build-checks-svvw1jue/`:
fmt, locked strict workspace Clippy/tests/release for **both** feature sets,
temporary installer, 9 resource unit tests and resource validation. Each workspace
run has **413 passed**, 0 failed (377 App + 4 CLI + 5 CLI integration + 27 core).
Default has 78 ignored App tests and enabled has 87; ignored is not passed.
`scripts/test-native-safety.py` separately passed all 6 checks.

Pinned default runnable binary:
`target/qa/native-preview/build-checks-svvw1jue/termimochi-default`, SHA-256
`47d4af3b17718c2b5bfd3e76f881bc916dd810bb2687595e5d73486f5e61e86f`.
Enabled artifact in that directory: `termimochi-enabled`, SHA-256
`45ed340585542763e2c57cebca2645384d28f255281cc54db3b3d868f47e2892`.
Run the default path directly from the repository, or rebuild with
`cargo run -p termimochi --locked`. The local toolchain lives at
`target/qa/typed-toolchain/rustup` (`RUSTUP_TOOLCHAIN=1.92.0`).
For the optional build, `bash scripts/run-native-preview.sh` supplies its prefix.

GUI reports (each contains build hashes and raw logs, not just this summary):

- Default broad matrix: `target/qa/preview-geometry-runs/termimochi-regression-oksa244r/`:
  **24/24 passed**, twelve selected behaviors at each scale. Includes original
  Full, sample/GIF/keyboard, scene, multi-window and both target-top regressions.
- Final stronger geometry/pointer run: `target/qa/preview-geometry-runs/termimochi-regression-8mphtt8a/`:
  **2/2 passed**, same geometry case with stronger assertions at both scales
  (not two additional independent features).
  It adds real wheel, transformed tab clicks and VTE drag selection. Both use the
  same worker SHA-256 `bd522c829d59808116894710865f7f58c23ec32b88934d3a8a6d39f337b85394`;
  their test hashes differ because the extra pointer assertions were added later.
- Enabled native + IME: `target/qa/native-preview/termimochi-regression-native-xz6kmr_2/`:
  **8/8 passed**, Kitty/Ptyxis native interaction plus sample Actual/Fit real Fcitx,
  each at 1×/2×. This was pinned before a title-label-only cleanup and the stronger
  test requiring a click to restore input focus; its precise build is in `build.json`.

The standard gates are fmt, locked strict workspace Clippy/tests/release, default
installer/resource checks and enabled-feature build checks via
`scripts/check-native-preview-builds.py`. The default GUI matrix runs on dedicated
Xvfb :96; native/Fcitx checks use the existing runner on :97, never the daily display.
Rust 1.92.0 matches CI. Local GTK 4.22.4 / libadwaita 1.9.1 / VTE 0.84 and the
existing Casilda/Ptyxis/Kitty prefix are reused without dependency upgrades.

Actual keyboard tests exercise both Actual size and Fit: physical XTest click
back into the input, finite completion/history, clipboard, Ctrl+Z/redo and Save.
The native runner additionally uses real isolated Fcitx Pinyin preedit, Enter,
Chinese commit and Esc cancellation. `sample-ime-candidates.png` captures the
actual X display, including the candidate popup (not just a GTK backing snapshot).
Sample actions do not count as native protocol verification.

The final geometry run's per-case cache contains all `after-*.png`/JSON files;
compare them with `before-*.png` in the baseline cache. Files were actually
viewed: same-size Fit now scales the top and input with the text; Actual size
retains legible original font dimensions and scrolls history. The Fcitx display
capture shows candidates adjacent to the transformed input. This is agent visual
inspection, **not external user usability validation**.

Earlier failures retained in the intermediate report directories were fixed:
top-row reparenting required its actual new sibling for reorder; cursor movement
alone was insufficient for asynchronous VTE reflow; window width needed measured
scrollbar width rather than a guessed rail; unchanged color refreshes must not
restore a rounded history anchor. No gate was disabled or reclassified as ignored.

## Remaining boundaries

- Hardware Wayland/DMA-BUF, fractional scaling, other IMEs/desktops/fonts and
  external user usability are not established by software Xvfb tests.
- Exact target-native decoration/font fidelity and the prior complex native
  Greeting truncation issue remain in [native-preview-qa.md](native-preview-qa.md).
  This patch does not claim to fix those by changing the design preview.
- Fit intentionally makes a fixed large target window smaller; Actual size is
  the readable default. Empty space is not filled by changing the theme font.
- Text input is still a bounded single-line sample, not a shell or multiline
  terminal editor. Per-tab GTK editing undo is reset when remounted, as before.

## 中文验收卡

1. 打开一个主题：默认是 **Full → Actual size**。直接点末尾 Prompt 后输入
   `help`、`git diff`；不需要 Start 或授权，不能执行本机命令。
2. 缩窄窗口：字不自动缩小，内容按观察列数重排；主题的初始行列及 Greeting
   Columns 不应跟着改变。再选 **100% · fixed grid** 检查固定列数。
3. 选择 **Fit window**：标题、标签、正文、图片、输入一起缩小，外部工具栏
   不缩放；连续输入多次 `help`，只是增加内部滚动，不应越来越小。
4. 中文输入法组合/提交/Esc、复制粘贴、Ctrl+Z。输入应紧接 Prompt，只有一个
   输入光标；点击正文仍能选字，点击标签仍命中对应标签。
5. 新建两个标签，分别留下目录和草稿。改字体、改颜色、切回标签，不丢设计
   或会话。看旧历史时改颜色不应跳到尾部。
6. Kitty 主题加入 GIF；播放、滚动、缩放，再 `clear`。不能留下悬浮图片；
   Ptyxis 字符参考不提供误导的 Play GIF。保存仍只保存主题。

截图和自动断言不是“真实用户易用性通过”；需要用户实际走完这张卡。
