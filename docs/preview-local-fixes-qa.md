# Full preview: folder residue, fallback notice, input caret

Base: `076e2d0`, 2026-09-12. This is a local correction, not a preview or theme-model redesign.

## Changes and cause

- **Folder residue:** reproduced in a newly opened, ordinary Ptyxis theme with both Prompt and Greeting disabled. The Full projection contained only `/demo $`, while VTE's actual text contained `Reading current folder...` above it. VTE consumes feeds asynchronously; erasing the display can move the old visible content into scrollback. Full now queues a history erase **after** the display erase, before its transcript. The existing content cache, geometry, reflow, and input ownership remain unchanged.
- **Callbacks:** folder and copied-Prompt completion already use the current-scene redraw dispatcher, not a direct legacy renderer. The folder-result receiver was extracted without changing its generation/coalescing policy so tests can deterministically deliver delayed and stale results through the production callback. No new thread or design state was added.
- **Character reference:** the Greeting projection carries whether artwork actually fell back to characters. Only a Greeting block included in the current bounded transcript can set the notice. Disabled Greeting, no editable image, explicit Character output, and `clear` do not claim image fallback. Saved presentation and native verification are unchanged.
- **Caret:** GTK input and VTE now resolve the same theme `Cursor` color, with Foreground only as the missing-value fallback. Input text still uses Foreground. The VTE cursor remains hidden in Full; GTK owns the single caret and IME.

Actual size, whole-window Fit, inline input, finite commands, independent tabs, Full/VTE, GIF preparation/anchors, native preview, and application safety are retained. No daily terminal configuration, shell startup file, installation, commit, or push was performed.

## Regressions and evidence

All paths below are relative to the repository. The raw evidence is local under ignored `target/qa`, not an assertion about online CI.

| Check | Result / evidence |
| --- | --- |
| Original bug reproduction | Failed as expected before the fix; `target/qa/preview-geometry-runs/termimochi-regression-f9yt00l2` includes actual VTE text and screenshot. |
| Focused new-Ptyxis and GIF regressions | 4/4 at 1× and 2×; `target/qa/preview-geometry-runs/termimochi-regression-cqx56qab`. |
| No render tools | Empty child PATH and private HOME/XDG, no Starship file; 1/1 passed in `target/qa/preview-geometry-runs/no-render-tools-Vz2Pe1/output.log`. |
| Default GTK matrix | Initial run 13/14 passed in `target/qa/preview-geometry-runs/termimochi-regression-6kqve8y_`. The 1× Fit keyboard case failed while the empty-PATH GUI test overlapped on the same display. Both Fit keyboard scales passed on an exclusive-display rerun, 2/2 in `target/qa/preview-geometry-runs/termimochi-regression-m2yxmzta`; no code or assertion changes. The initial failure remains recorded, not relabelled as a pass. |
| Default and native-enabled builds | Both strict Clippy, workspace tests and Release builds passed. Each test suite: 413 passed; default 79 ignored, native-enabled 88 ignored. Ignored tests are not counted as passes. |
| Packaging | Installer checks, 9 desktop-resource unit tests and desktop-resource validation passed. |
| Native safety policy | 6/6 passed via `python3 scripts/test-native-safety.py`. |
| Enabled native / IME regression | 8/8 passed at 1× and 2× in `target/qa/native-preview/termimochi-regression-native-6qdbs3c6`: Kitty, Ptyxis, Actual-size input and Fit input. Keyboard cases verify real private Fcitx Pinyin preedit/commit/cancel as well as computed GTK caret colors. |

Build logs and separate runnable binaries: `target/qa/native-preview/build-checks-391uzjqg/` (`termimochi-default`, `termimochi-enabled`).

`target/release/termimochi` was restored to the default build after checking both build variants, without installation. Its SHA-256 matches the pinned default artifact: `794ce3ec0144553320f06eba3f269a2b392f95da0e16fb153cca9bba022934fe`.

Run from the repository with `./target/release/termimochi`. For a disposable application session instead, use `bash scripts/run-isolated.sh` (state isolation, not a filesystem sandbox; open only test copies).

The new GTK regression checks **actual VTE text**, not merely the cached feed. It submits `help`/`clear` before waiting for context; holds a worker receiver while typing; supplies late real-folder and stale other-theme Prompt results; switches editor pages, all zoom options and tabs; queues an old directory scene before returning to Full. Prompt/Greeting remain disabled and the design snapshot is unchanged. Existing keyboard regressions additionally rasterize GTK's insertion cursor using the real input delegate's computed style and check two distinct Cursor colors, alongside the single-caret geometry assertions.

Before screenshot:
`target/qa/preview-geometry-runs/termimochi-regression-f9yt00l2/1x-window-interactive_samples-tests-interactive_samples_ptyxis_without_context/cache/ptyxis-no-context-initial.png`

After screenshot (same theme and window size):
`target/qa/preview-geometry-runs/termimochi-regression-cqx56qab/1x-window-interactive_samples-tests-interactive_samples_ptyxis_without_context/cache/ptyxis-no-context-initial.png`

Geometry screenshots and JSON records are under each `preview_geometry_same_theme/cache` case in the default matrix. Real IME screenshots and the GTK-rendered green/magenta cursor images are under each keyboard case's `cache` in the enabled matrix. Native target screenshots remain in the corresponding Kitty/Ptyxis case evidence directories.

## 中文手动验收卡

1. 新建 Ptyxis 主题，保持 Prompt 和 Greeting 关闭。Full / Shell 应立即显示 `/demo $`，没有目录加载残留，也没有 character reference 提示。
2. 输入 `help`、`clear`，再输入尚未提交的中文。切左侧设置页、切 Fit / Actual size、创建并切回标签：内容、输入和目录各自保留，加载结束也不能覆盖。
3. 在调色页修改 **Cursor**，不要修改 Text：输入光标应跟随 Cursor，文字颜色不变，没有第二个光标。测试中文组合、候选确认及取消。
4. 启用带图片的 Greeting：只有确实显示字符回退时才出现 character reference。`clear` 后提示消失；`fastfetch` 显示当前 Greeting 时再恢复。关闭 Greeting 或改为字符输出后不应保留该提示。
5. 这些操作只验证设计样板，不等于 Kitty/Sixel 协议验证；不需要 Apply、安装或修改终端启动文件。

## Boundaries

No external user usability study or daily-desktop configuration changes were performed. Automated X11/GTK and private Fcitx evidence does not certify every Wayland compositor, IME, font, or terminal version. Existing experimental native-preview limitations are not removed or reclassified by this fix.
