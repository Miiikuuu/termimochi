# Image background in the design preview

Background is now an optional, owned setting of the existing theme. Save embeds
the prepared image and its framing; it does not apply any terminal settings.
This does not add a new theme/document model or claim stock Ptyxis support.

## Use

```sh
bash scripts/run-background-trial.sh --workbench
```

Open a theme, select the separate **Background** editor page (Ctrl+6), and
click **Choose Background Image…**. The workbench mode does not require the
optional patched Ptyxis runtime. The local executable is
`target/background-trial-build/release/termimochi`.

- **Cover / Contain** selects fill/crop versus whole-image display.
- **Image zoom %** shrinks or enlarges the image (10–300%), independently of
  Full's Fit/Zoom. The slider scales around the image's current center.
- **Move Image** enables direct dragging in the right preview, with a hand
  cursor. It is enabled after loading an image and only captures drags while
  the Background page is selected. **Ctrl+wheel** zooms around the pointer;
  ordinary scrolling still scrolls the terminal sample. Turn Move Image off to
  select terminal text. Inspect and Move Image are mutually exclusive.
- **Horizontal / Vertical position** moves the image left/up (0), through the
  center (50), to right/down (100), including when image and viewport sizes
  match. A small part of the image stays visible at the limits.
- **Overlay %** mixes in the current theme background to soften the image.
- **Center** preserves zoom and centers the image. **Reset** also restores
  100% image zoom. **Remove** restores the ordinary opaque terminal background.

The VTE design surface, including Full's simulated commands, displays the
background. It stays anchored to the content viewport when text scrolls and
shares the existing whole-window Fit transform and rounded clipping. It does
not cover the terminal title/tab bar. Greeting's separate pixel-only artwork
inspector and the advanced native-terminal pane are not background previews.

Changing left-hand editor pages preserves the image. Palette changes update the
overlay color; font changes keep the background. The finite command input and
history are not reset by background controls. Opening another theme restores
only that theme's own image; another App window never inherits it. Editing and
saving do not launch a shell. Image changes, framing and removal are undoable.

## Save and use

- **Save / Ctrl+S** embeds a bounded prepared PNG and Cover/Contain, zoom,
  horizontal/vertical position and overlay. Reopening works even if the source
  JPG/PNG/WebP has moved or been deleted. The original file is never changed.
- **Kitty → Use Theme → Try in Kitty → Apply & Open in Kitty** includes a
  `background.png` in the existing versioned independent entry. The reviewed
  trial and publication use the same prepared image. Existing entry reopening,
  integrity/conflict checks and recovery also cover the background artifact.
- **Ptyxis → Background → Open in Enhanced Ptyxis…** uses the current theme
  image/framing, palette, font and grid in the optional patched terminal.
  **Open Real Terminal** is a temporary trial. **Apply & Open** explicitly saves
  the appearance snapshot and creates an independent app-menu shortcut. No extra
  consent checkbox or second image picker is required. Reopen it after an App
  restart from **⋮ → Enhanced Ptyxis — Saved Entries…** or the shortcut;
  an applied terminal keeps running when the editor closes. Applying again creates
  a version; the library restores the previous one or removes the initial entry.
  Version assets remain recoverable. This is NOT a complete theme application:
  Prompt/Greeting, personal Bash/Conda, other layout settings and live updates are
  not included. Clean Bash uses your actual home directory; terminal settings and
  its D-Bus remain private. The optional runtime at the reviewed path is still
  required; it is not copied into a standalone distribution.
- **System Ptyxis → Use Theme** still reviews its supported shared changes.
  An owned background is explicitly recorded as unsupported, shown in the main
  review and in the result, with an Enhanced Ptyxis entry in the review. It is
  never silently reported as applied to the stock terminal.

Both native paths use the same bounded framing projection as the GTK surface.
The native background is a prepared theme-grid canvas (maximum 1920 per side)
scaled to the native window. Resizing that window to a different aspect ratio
can stretch it; it is not a live background layout engine. Kitty's native
[`background_image_layout scaled`](https://sw.kovidgoyal.net/kitty/conf/#opt-kitty.background_image_layout)
is used for this canvas. GTK Full/Fit remains a design preview, not a native
compatibility stamp.

## Explicit limits

- Background belongs to a **theme**, not a standalone palette/font/layout file.
  Component-copy conversion reports its omission. Older themes without the
  optional field still open without creating an image override. Cross-target
  theme copies retain the image but use the destination's actual capabilities.
- Static PNG/JPG/WebP only, reusing the bounded background decoder (16 MiB source,
  existing dimension/pixel limits, maximum 1920×1080 prepared texture). Original
  files are not modified. This is not the Greeting artwork importer.
- No brush editing, blur or animated backgrounds yet.
- Existing contrast diagnostics still measure **solid theme colors**, not every
  image pixel. They do not certify readability over the background.
- The standalone experimental picker remains available for research. It is
  separate from a saved theme. Ordinary system Ptyxis remains unchanged.

## Implementation and regression

`window/preview_background.rs` owns the per-Workbench image editor and controls.
`background.rs` holds immutable image/framing snapshots and the shared geometry.
`ThemeIntent.background` is the optional serialized owner; Workspace is only the
existing projection bridge. History shares image bytes, not copies of the file.
Its GTK surface paints the prepared texture and theme-color overlay behind the
existing VTE viewport, within the existing Full frame. It neither reparents the
Full transcript/GIF nor changes image cell occupancy. VTE background clearing and
its GTK CSS background are made transparent only while a background is present.

Image preparation has one worker in flight per window. File-picker and worker
results are checked against the document identity and request generation. Remove
invalidates an outstanding result. The worker does not update GTK or the theme.
The GUI thread installs the bounded prepared texture and records the edit; slider
changes neither decode images nor refeed the transcript.

Pointer gestures use GTK's local coordinates after Full's allocation transform.
Only Move Image's primary-button drags and Ctrl+wheel are claimed; other events
reach the existing VTE/input/scroll path. A document, image or geometry change
invalidates an active drag. Translation stays bounded and pointer zoom uses the
same geometry as the sliders and the renderer.

New tests cover cover/contain/position geometry and an opt-in native GTK flow:
real composited pixels through VTE, before/after captures, scroll and pending
input preservation, Fit, editor navigation, palette/font edits, separate-window
isolation, and rejecting a late image result after a document change. Existing
Full/GIF, independent-scene and geometry regressions are run separately. These
are GTK design-preview checks, not native target compatibility certification.

The following results are historical. Current persistence/native-use acceptance
is recorded in [background-theme-acceptance.md](background-theme-acceptance.md).

### Initial preview acceptance — 2026-09-24 (before separate sidebar)

- Default Release workspace tests: **436 passed, 0 failed, 91 ignored**. Ignored
  GUI cases are not counted as passes by this command.
- Default workspace Clippy and enabled `native-preview` App Clippy, all targets,
  warnings denied: passed. Formatting and diff whitespace checks passed.
- Four distinct isolated GTK cases passed at scale 1: background composition and
  isolation, existing Full/GIF composition, same-theme geometry/input, and
  independent preview scenes. The background case was rerun after the final
  wording/formatting change and passed again; this is not a fifth distinct case.
- Screenshots were inspected: image visible behind actual VTE glyphs, controls
  reachable in the Background page, Fit preserving the full frame, and theme changes retaining
  the image. A pixel assertion checks the known fixture through the composite
  VTE surface, not just a loaded texture. The initial assertion failed because
  the GTK control's white background obscured the image; that failure is fixed,
  not omitted from the history.

Initial preview executable SHA-256 (not the later sidebar build):
`f5e9c7d5a3508590aac98ab4dc847c6c3d16cc572f5d576cb1cc51aff8f1c328`.
It matches the Release worker pinned by the final GTK runner. GTK interaction
checks themselves use the optimized test executable with production widgets and
callbacks, not an externally driven release-file-chooser test.

Evidence copied into `target/qa/background-preview-acceptance/`: before/after,
Fit, edited-theme and cleared screenshots; geometry JSON; GTK log; build hashes;
background and regression results. Full build/test logs are
`target/qa/background-preview-final-checks.log`,
`target/qa/background-preview-feature-clippy.log`,
`target/qa/background-preview-final-gui.log` and
`target/qa/background-preview-handoff-gui.log`.

Not verified in this pass: Wayland, hardware-GPU differences, scale 2, the new
workbench file chooser via external pointer automation, and advanced native
terminal interactions. Native terminal trials were not rerun or declared newly
certified by these design-preview tests. Persistence/Apply and the other limits
listed above are **not implemented**, rather than passed or environment-blocked.
No installation, real terminal configuration changes, commit or push was done.

### Separate Background page acceptance — 2026-09-24

Background controls and the optional Ptyxis trial now have their own left editor
page, reachable with Ctrl+6. Switching editor pages does not change the right
preview scene. The fixed Save/Use bar explicitly says the preview image is not
saved or applied; it offers no misleading background-specific Apply action.

The Release workspace suite passed **436 tests** (91 existing ignored cases).
The final optimized GTK run passed the background composition/isolation and
independent-scene navigation cases **2/2** at scale 1. Default Clippy passed;
`native-preview` Clippy passed using the existing local Casilda prefix. The
final Release executable at `target/background-trial-build/release/termimochi`
has SHA-256 `7a67ebab8f1c112d3636b5a666efb83026d9ee833f5fcd0035e683ecf24531e9`.
The isolated runner's pinned worker has the same hash. Its test report and GTK
captures are under `/tmp/termimochi-regression-oeamm9bg/`; the App run log is
`target/qa/background-section-release-gui.log`. The screenshots show the
separate page and the background behind the Full VTE sample, not native target
background support. This later check did not rerun native terminal trials.

### Shrink and drag acceptance — 2026-09-26

- Release workspace tests: **438 passed, 0 failed, 91 ignored**. The two new
  geometry cases cover small/exact-fit movement, reachable limits and anchored
  zoom. Default and `native-preview` Release Clippy passed with warnings denied;
  the optional feature used the existing local Casilda prefix.
- **Five distinct isolated GTK cases, seven executions, all passed** on the same
  optimized test build: background composition/real pointer editing and Inspect
  at both 1× and 2×; Full/GIF composition, independent scenes and Fit keyboard/
  Unicode clipboard at 1×. The background test drives real XTest dragging and
  Ctrl+wheel through GTK in Actual, Fit and 100% modes, checks the image's actual
  geometry, verifies scroll/input/history preservation, then types `help` after
  leaving Move Image. It also exercises 10%, Center, Reset and document isolation.
- Screenshots and pointer geometry records are in
  `target/qa/background-edit-2026-09-26/1x/` and `2x/`. A local 70×40 drag moves
  the image by 70×40 in Actual/100%; Fit's device-pixel rounding remains within
  the recorded tolerance. See `background-drag-geometry.json`, `background-moved-*`
  and `background-shrunk-10.png`. Runner results and build hashes are alongside.
- The Release App SHA-256 is
  `05e54641575ad0ca29d231af09956cce72c14b279754997b430ad79ef6f25728`, matching
  both runners' pinned production worker. No installation or publication was done.

The raw logs also retain GTK scrollbar allocation warnings during the transition
to fixed-grid mode. Those warnings were observed in both scale runs; navigation
and pointer assertions passed, but this change does not claim to fix that
separate allocation issue. Desktop-portal/PipeWire warnings are also present in
the isolated X11 session. Wayland, physical touchpad smooth scrolling, hardware
GPU differences and real Chinese IME preedit were not verified in this pass.
The keyboard test used GTK's simple input method and Unicode clipboard; optional
native terminal trials were not rerun. The image remains temporary preview state.

## 中文验收卡

1. 启动新版工作台 → 打开主题 → 左侧 Background（Ctrl+6）。
2. 点 Choose Preview Background，选静态图片；右侧 Full 应显示文字在图片上。
3. 将 Image zoom 调到 10%–50%；图片应缩小，空白处使用当前主题背景色。
4. 开启 Move Image，在右侧拖动；Ctrl＋滚轮围绕鼠标缩放。Center 只居中，Reset 居中并恢复 100%。
5. 关闭 Move Image，选择文字或输入 help；普通滚轮仍可查看历史。
6. 切 Palette 或 Typography；背景仍在，鼠标恢复终端操作，可继续对照颜色和字号。
7. 切 Full 的 Fit / 100% / Actual size；拖动距离跟随鼠标，滚动文字时背景固定。
8. Remove 恢复纯色。换主题、新开窗口不会带入上一份背景。
9. 本轮不验收保存/应用背景：它明确是临时设计预览，不改变日常终端。
