# Native Greeting width adaptation — repair and acceptance

2026-09-13, base `38ad24d` plus the retained diagnostic and this repair.
The [original diagnosis](native-greeting-diagnosis.md) remains available. No
theme-model or visual redesign; no daily configuration, shell startup, default
terminal, commit, push or installation changes.

## What changed

New controlled Kitty themes, their independent sessions/daily launchers, and
the experimental native adapter use a one-shot bounded Greeting output helper.
Ptyxis's experimental character session uses the same existing controlled plan;
this does not add Kitty pixel support to Ptyxis.

1. The helper checks the managed manifest, artifact hashes and executable
   dependencies before reading the already-reviewed Fastfetch projection.
2. Fastfetch collects actual native information once, with its logo disabled.
   Its wrapper-aware `FFTS_IGNORE_PARENT=1` is scoped to that child so it detects
   the actual Bash/terminal rather than reporting TermiMochi as the shell.
3. The existing `NativeOutput` parser resolves safe ANSI positioning with a
   measured information extent. It no longer clips this path at Full's
   240-column observation bound. The existing SGR/grapheme wrapper splits rows
   to the real terminal's measured columns, leaving one spare cell at the edge.
4. With fewer than 20 information columns beside the unchanged logo/gap, the
   ephemeral output uses image-above-information. Otherwise it retains the
   original side placement, with every continuation inside the information area.
5. Fastfetch renders those escaped literal rows and the original native image
   protocol. This is actual Kitty output, not a GTK bitmap or fake fact snapshot.
   No imported custom/network/command module is newly granted execution.
6. Generated GIF streams use `C=1` (unchanged cursor). A temporary top-layout
   stream adds its exact reserved row advance so text cannot overlap the GIF.
   Static Kitty placement advances natively; it is not given a duplicate advance.
7. Explicit wrapping settings avoid reliance on Fastfetch's changing defaults.
   Shell startup runs the helper once; resize, hot color/font updates and TUI
   interaction never inject commands or re-execute Greeting.

The current theme, sparse ownership, original configuration/assets, shared
Fastfetch file and save/apply/restore semantics are unchanged. Transient output
files are private and removed when the helper exits. Native width measurement
uses read-only `stty -F /dev/tty size`; no global environment is altered.

Information capture is bounded to 128 KiB / 10 seconds, 4096 source columns and
256 source rows; wrapping has a 4096-row limit. Final graphics/text capture keeps
the existing 40 MiB cap with a 10-second deadline. Exceeding a limit or missing a
dependency reports a failure and leaves the shell usable instead of silently
discarding a tail. Artwork wider than the terminal is explicitly reported, never
silently shrunk. This is not unlimited scrollback.

Existing manifests default the new adapter flag to false: old published versions
keep their reviewed behavior until explicitly replaced. Retained daily runtimes
continue working independently of the old editor helper location; the currently
running, launcher-verified runtime never executes that old helper path. Artifact
and Fastfetch dependency checks remain in place.

## Evidence and test scope

Same private **小猫测试** design, unchanged SHA256
`283f0aaf3c14bfcb01826eedd834054fa3abcec0b91f75a167c474836522b13e`.
The native fixture retains 16 pt, its actual multi-field module list and 32×14
GIF reservation. Native measured grids: **46×14**, **80×14**, **142×26** (columns
× rows). The static control changes only the private in-memory output choice.
The saved source is checked byte-identical after each run.

| Check | Evidence / scope |
| --- | --- |
| Default + enabled builds | [10/10 gates](../target/qa/native-preview/build-checks-7c2edkgo/results.json): fmt, both strict Clippy runs, both workspace tests and Release builds, isolated installer and metadata checks. Each workspace run has **424 passed** (388 App + 4 CLI + 5 integration + 27 core); ignored native/GUI tests are not passes. |
| Actual default Release output | [3/3](../target/qa/release-acceptance/termimochi-regression-jfqx5t7m/results.json): controlled Kitty real GIF/Prompt/reopen, daily desktop entry open/update/restore, existing sample GIF anchors. The helper and retained launcher runtime are the actual optimized application. |
| Existing Full design | [4/4 Release matrix](../target/qa/release-acceptance/termimochi-regression-htoktq51/results.json): Full live composition and private-fixture Actual/Fit/fixed reachability, with real pointer/scroll coverage at 1×. |
| Actual enabled Release Kitty/Ptyxis interaction | [Release run](../target/qa/native-preview/termimochi-regression-native-r1n7x5i5/results.json): the four interaction cases pass, covering both targets at 1×/2×, native shell/tab, Fcitx IME, clipboard, hot-update and lifecycle. This run retains two failed static-image assertions, discussed below; it is **not** an 8/8 pass. |
| Final complex GIF + static control | [4/4 dedicated Release rerun](../target/qa/native-preview/termimochi-regression-native-x_g34tbn/results.json): both formats at 1×/2×, same real native output, physical grid limits, native row reservations, actual pointer scrolling and inspected screenshots. |

The final native helper SHA256 matches the separately built enabled Release:
`0f807f69422c6d7223628a51a162d96104605a855ea92499369857488f5cd588`.
Default `target/release/termimochi`:
`c04c8104f8431d5a7b9712907c0e32bb7da19186a06645617d17d0abe7b11c9f`.
Final native harness assertions were additionally checked by strict enabled
Clippy. The native runner now accepts `--release` and records its profile.

### Actual native screenshots

- [GIF at the top of narrow history](../target/qa/native-preview/termimochi-regression-native-x_g34tbn/1x-native_complex_greeting_reflow/cache/rerun-narrow-history-top.png).
- [Narrow information, below the GIF](../target/qa/native-preview/termimochi-regression-native-x_g34tbn/1x-native_complex_greeting_reflow/cache/rerun-narrow-history-middle.png).
- [Complete CPU/GPU tails and Prompt](../target/qa/native-preview/termimochi-regression-native-x_g34tbn/1x-native_complex_greeting_reflow/cache/rerun-narrow.png).
- [Side composition with aligned continuation](../target/qa/native-preview/termimochi-regression-native-x_g34tbn/1x-native_complex_greeting_reflow/cache/rerun-expanded.png).
- [Wide complete composition](../target/qa/native-preview/termimochi-regression-native-x_g34tbn/1x-native_complex_greeting_reflow/cache/rerun-wide.png).
- [Static top reservation](../target/qa/native-preview/termimochi-regression-native-x_g34tbn/1x-native_complex_greeting_static_reflow/cache/rerun-narrow-history-top.png),
  [following information](../target/qa/native-preview/termimochi-regression-native-x_g34tbn/1x-native_complex_greeting_static_reflow/cache/rerun-narrow-history-middle.png).

These are actual native textures on an isolated X11 host / nested Wayland SHM,
not stock screenshots or similarity scores. Raw `.typescript` streams and
`.grid` measurements sit alongside each frame. Assertions check complete CPU
tail bytes, physical text widths, top-row reservation and the absence of the
wrapper as Shell. Pointer scrolling covers top/middle/bottom views; screenshots
were visually inspected. This is not an automated all-pixel visibility proof or
external human acceptance. Existing native motion tests independently check GIF
movement over multiple actual frames.

## Failure history retained

- `jct8kfep`: native output failed because the temporary configuration lacked
  the `.jsonc` suffix Fastfetch expects. Fixed in production; not waived.
- `lqc_4ae8`: byte/width assertions passed, but image inspection found the GIF
  overlapping top-layout information. The raw `C=1` row advance was fixed and
  an explicit reservation assertion added. This run is not visual acceptance.
- `jn9wnyjf`: six native cases passed, but wide screenshots showed the helper
  name as Shell. Child-scoped wrapper-aware shell detection was added, then
  actual optimized runs verified `bash 5.3.9` and reject the wrapper name.
- `fl2gsd40`: enabled Clippy rejected a test's `len() > 0`; corrected without
  suppressing warnings. The complete dual-build gates were rerun as `7c2edkgo`.
- `r1n7x5i5`: static-image tests incorrectly treated native cursor advance as
  zero, as an ANSI-only parser ignores graphics. The test now verifies the actual
  static placement command and its `r-1` native advance; GIF's explicit advance
  remains separately required. Actual static top/middle screenshots supplement
  the corrected model. The failing logs remain, followed by the dedicated rerun.

## 中文手动验收卡

1. 用 `./target/release/termimochi` 打开原主题。新建的 **Use Theme → Try in Kitty**
   使用新输出；嵌入实验预览可用 `bash scripts/run-native-preview.sh`，先结束旧预览再启动。
2. 在窄窗口检查：图上文下；向上滚动看到完整图像，向下看到全部信息和 Prompt。
   CPU／GPU 尾部不丢失，信息不能压到 GIF 上；主题字号和图片占位不变。
3. 展开窗口：已有内容不能凭空丢失，也不会自动执行新命令。旧历史可能保留原来的
   换行；新会话按当前宽度输出。空间够时横排，长信息的续行仍留在信息区域。
4. 输入命令或运行 TUI 后改颜色／字号：原 shell、目录和内容应保留，不能自动重跑 Greeting。
5. 切换静态图、重开原生会话：图片后留出正确行数，不叠字。
6. 已发布的旧 Kitty 入口不是自动迁移对象。确认新试用后使用 **Apply & Open in Kitty**；
   若已有桌面入口，再在结果 Advanced 中明确更新入口。Save 本身不应用配置。

## Remaining boundaries

This repairs **new controlled sessions**, not arbitrary raw `fastfetch` commands,
plain export bundles, standalone advanced **Try Greeting** or shared Ptyxis
Fastfetch application. Those direct paths still rely on their own output layout.
It does not recreate characters already lost in an old terminal's history.

Ptyxis's character regression passes only with the existing private patched
native dependency; stock private-bus/systemd restrictions are not newly solved.
Daily GNOME Wayland/GPU/DMA-BUF, fractional/multi-monitor scaling, long idle/motion
soaks and external-user acceptance remain unverified. No online CI, RustSec,
package publication, automatic installation or push was performed this turn.
