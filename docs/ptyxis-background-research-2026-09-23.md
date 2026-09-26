# Ptyxis image-background research — 2026-09-23

## Decision

**Feasible as an optional patched Ptyxis runtime; not available as a setting in the tested stock Ptyxis, and not ready for production integration.** A real Ptyxis 50.1 process now draws a static image behind its own VTE content, including native tab overview thumbnails. This is neither a TermiMochi preview overlay nor an external wallpaper window. No Casilda, replacement terminal emulator or second theme model is required for this standalone experiment.

This turn is research plus an isolated proof of concept. TermiMochi's Rust application, theme schema, existing runtime, installer and release package were not changed. Nothing was committed, pushed or published.

## Source and build

- TermiMochi baseline: `a5755b1976492c194b619a309d412aa4f1da6627`.
- Ptyxis baseline: 50.1, `0f045a653db114e22fe7bf4b409fcb30d7763fa9`, from the already retained source checkout. Conclusions apply to this inspected baseline; the attempted upstream-main raw-file fetch was unavailable, so this is not a claim to have audited the latest upstream main.
- Existing `native/patches/ptyxis-private-scope.patch` is reused in a **new source clone only**. It permits the private bus without a daily systemd manager and retains existing palette/allocation fixes.
- Additional rendering patch: [`native/experiments/ptyxis-background.patch`](../native/experiments/ptyxis-background.patch), 56 added lines in `src/ptyxis-tab.c`. It is deliberately not added to `build-native-preview.sh`.
- GTK 4.22.4, libadwaita 1.9.1, VTE 0.84.0; Meson `debugoptimized`, Ubuntu 26.04.1 x86_64.
- Distinct application/schema ID: `io.github.Miiikuuu.Termimochi.PtyxisBackgroundResearch`; distinct schema path and install prefix. No daily desktop entry or service was installed.
- Final experiment binary SHA256: `af289fa37cf236640939476d77c14e7fd229e4696454edb36f13fd0c7863b197`.
- Build passed. The two C90 declaration-order warnings are from the reused private-scope patch, not hidden as a warning-free build. The first build was blocked by read-only ccache; the approved retry uses a project-local cache.

All generated files, test code, build logs and screenshots live under:

```text
target/qa/ptyxis-background-68JnBSJb/
  source/             independent Ptyxis source clone
  build.sh, build/    isolated build recipe/output
  prefix/            private binaries, schemas and resources
  fixture.png        synthetic quadrant image; no private artwork
  session.bash       controlled rc, never sources personal startup
  probe.py           native XTest/clipboard/shell/screenshot checks
```

The original `target/native-preview/src/ptyxis` modifications were preserved. Patch forward-check against that source and reverse-check against the experiment both passed. `/usr/bin/ptyxis` and the previous private Ptyxis binary retain their hashes.

## Why this insertion point

The inspected `ptyxis-window-dressing.c` explicitly clears CSS backgrounds on `vte-terminal`. `ptyxis-terminal.c` also rewrites VTE's snapshot to remove its large uniform background node. Ordinary terminal content is already composed over a separate background: injecting user CSS alone is not a dependable per-theme interface.

`ptyxis_tab_snapshot()` has **two** relevant paths:

1. Normal terminal content: draw the image and theme-color tint immediately before the existing child snapshot.
2. Active-tab animation/overview: draw the same image/tint into the cached texture before the transformed terminal snapshot.

The image uses tab coordinates rather than scrollback coordinates, so it stays fixed when history scrolls. A clipped aspect-preserving cover/contain calculation leaves terminal sizing to Ptyxis. VTE still handles the shell, selection, input, explicit ANSI cell backgrounds and cursor. The prototype does not reduce opacity of the entire terminal widget, which would also fade text.

The prototype uses [GTK snapshot textures](https://docs.gtk.org/gtk4/method.Snapshot.append_texture.html); [texture loading](https://docs.gtk.org/gdk4/ctor.Texture.new_from_filename.html) is performed once at tab construction. **Synchronous per-tab decoding is acceptable only for this small trusted fixture, not the intended production importer.** No hot-reload or bounded hostile-input handling is claimed.

## Native evidence and limitations

Tests use an owned Xvfb `:94`, a private D-Bus/keyfile/HOME/XDG tree, and Bubblewrap with the host mounted read-only. Only the current evidence directory is writable. No personal shell initialization is loaded. The driver pastes finite test commands through the actual clipboard shortcut and keyboard, scrolls with actual XTest wheel events, and opens native tabs using the native shortcut. Shell PID/current-directory files provide execution evidence independent of screenshots.

| Check | Observed result / boundary |
| --- | --- |
| Static background behind real Ptyxis text | Demonstrated with both Cairo and GL; PNG quadrant fixture and screenshots, not a simulated terminal |
| Cover/contain sizing and 0.35 theme-color tint | Demonstrated; contain retains side margins; not a complete option/boundary-value matrix |
| Actual shell input / clipboard paste | Commands execute and write expected private markers; not a full bidirectional clipboard test |
| Scroll history / clear | Wheel changes the rendered content; three right-side background anchors remain unchanged; clear retains the image |
| Two native tabs | Independent shell PIDs/directories; first PID/directory survives switching back |
| Overview / return | Image present in native thumbnails and after return; selected screenshots inspected, not every animation frame |
| Resize and font zoom | Image retained and first shell PID/directory unchanged; geometry saved per screenshot |
| Explicit ANSI backgrounds | The colored test line remains visible over the picture; no claim that every TUI becomes transparent |
| Cairo text rendering | **Visual failure:** after clear/tab return, text becomes pink blocks. Reproduced with background disabled in the same binary; therefore not unique to image drawing. Precise upstream GTK/VTE root cause not established. |
| GL control and image path | Same operation sequence shows readable text, no detected pink-block signature; software rendering in isolated X11, not hardware-GPU/Wayland certification |
| Missing image | Logs an explicit fallback, shows ordinary background and accepts a real shell command; passed in the final GL rerun |
| First-frame presentation | Early GL runs captured empty frames despite mapped windows/running shells. Those failures are retained; the final driver waits for visible chrome/content with a bounded timeout. This is not proof that real desktop startup is flicker-free. |
| Chinese display | Chinese fixture text visible; **actual IME preedit/commit not tested** |
| Isolation | Completed-run before/after hashes match for system/private Ptyxis, personal `.bashrc` and dconf file; host mount is read-only during native tests |

Evidence directories, relative to the experiment root:

- `cover/`: initial clipboard/focus driver failure; background screenshot retained. Fixed driver focus and waited for remote clipboard-format notification before pasting.
- `run-dexi5vr1/`: initial Cairo cover/contain/missing behavior checks passed, but later screenshot review found pink text. Do **not** treat its early `passed: true` records as whole-visual acceptance. A subsequent source audit also moved texture cleanup from an allocation callback to disposal; all final evidence uses the rebuilt binary above.
- `run-gtcsk2zb/`: final-binary Cairo plain/cover controls; behavior checks pass but both have recorded visual artifacts. Protected-file hashes unchanged.
- `run-m4q0lr5q/`: GL plain control passed; initial cover capture was empty and failed.
- `run-jfyzyke2/`: final-binary software-GL plain and cover behavior checks pass, no pink signature; cover native screenshots visually inspected. Contain's initial capture was empty and failed, so the matrix stopped before missing-image testing.
- `run-g6bzu30k/`: final software-GL contain/missing rerun **2/2 behavior cases passed**, no pink signature; contain required one additional 250 ms first-frame wait, with the preceding empty frame retained. Contain's returned-tab screenshot was visually inspected. Protected-file hashes unchanged. Together with the successful plain/cover cases in `run-jfyzyke2`, four GL cases have completed; this is not a claim that all attempts passed in one uninterrupted matrix.

The numerical screenshot checks only establish fixture presence and detect this particular pink-block failure. They are **not a general visual-similarity score or high-fidelity certification**. Logs retain DRI3/GL environment warnings and Pillow API deprecation warnings.

The GL results describe the explicit `GSK_RENDERER=gl` / `LIBGL_ALWAYS_SOFTWARE=1` test environment and observed output, not a GPU-driver capability probe. No real desktop was opened for this research. The owned Xvfb `:94` was closed at completion; roughly 75 MiB of local source/build/evidence is retained. The Rust workspace suite was not rerun because no production Rust sources or build wiring changed.

## What production integration would require

| Area | Required work, not implemented here |
| --- | --- |
| Theme ownership | Add sparse background intent/assets to the existing workspace. Unset inherits; reference previews are not an application write. Background is separate from Greeting and consumes no terminal rows. |
| Runtime capability | Identify stock vs enhanced Ptyxis explicitly. Stock Ptyxis cannot silently accept this setting or be marked applied. Do not substitute Kitty or claim a generic VTE preview is real Ptyxis. |
| Safe import | Reuse existing bounded artwork workers for validated static output; limits on source bytes, decoded dimensions, texture memory and stale results. Missing/invalid replacements retain the last valid image. |
| Hot update | Add a per-profile runtime property/control path, coalesce updates, retain immutable generation/theme identifiers, redraw and invalidate overview caches on the GTK thread. Do not restart the shell or inject commands. |
| Memory | Share immutable textures across compatible tabs; the prototype duplicates per-tab textures. A 3840×2160 RGBA bitmap alone is about 31.6 MiB before GPU/cache copies. |
| Real use | Publish managed image/config assets and a versioned launcher selecting this exact optional runtime. Close/reopen must work without the editor and without modifying the ordinary Ptyxis entry. Existing Kitty publication code cannot be treated as an already implemented Ptyxis adapter. |
| Isolation/recovery | Keep separate schema/backend/bus scope, retained runtime/assets, conflict checks and reversible launcher updates. Stock Ptyxis global font settings must not leak into the daily backend. |
| Compatibility | Test Wayland/hardware renderers, IME, clipboard, 1×/2×/fractional scale, tab detach/overview, search/selection, alternate-screen TUIs, suspend/restart and source deletion. Rebase and rerun when Ptyxis/GTK/VTE changes. |
| Distribution | Package the optional runtime and its agent/resources/schemas, lock dependencies, preserve upstream provenance and license information; review distribution obligations separately before shipping. Ptyxis source is marked GPL-3.0-or-later. |

The additional drawing code is small; safe integration and maintaining a terminal runtime are the substantial commitments. Prefer an upstreamable background API/patch long term, with this pinned optional runtime as an explicitly experimental route. No upstream submission or acceptance is claimed. Static images first; GIF/video backgrounds are a separate performance/lifecycle design and unimplemented here.

## Local reproduction and Chinese review card

From the main repository, on this prepared machine:

```sh
# Independent prefix only; never /usr or ~/.local.
CCACHE_DIR="$PWD/target/qa/ptyxis-background-68JnBSJb/ccache" \
  bash target/qa/ptyxis-background-68JnBSJb/build.sh

# Automated native windows appear only on private Xvfb, not the daily desktop.
/usr/bin/python3 target/qa/ptyxis-background-68JnBSJb/probe.py --gl
```

Requires the retained private dependency prefix/Xvfb plus host Meson, C compiler, GTK/VTE development files, Bubblewrap, Python GI/cairo/Pillow, XTest and `xwininfo`. The experiment launcher is not a distributable end-user installer. Each test rerun creates a new evidence directory; old failures are retained. Do not run its terminal binary directly against daily settings as a shortcut.

中文验收卡：

以下 cover 截图位于 `run-jfyzyke2/cover/`；contain/missing 位于 `run-g6bzu30k/`，不要误用最早的失败截图作最终结果。

1. 看 `cover/01-native.png`：图片是在终端文字后面，不是 Greeting 输出，也不是桌面壁纸。
2. 对照 `02-output.png`、`03-scrolled.png`、`04-clear.png`：历史滚动/清屏，背景保持；不能只看执行成功。
3. 看 `05-two-tabs.png`、`06-overview.png`、`07-return.png`：原生双标签和缩略图都带背景，文字必须可读。
4. 看 `08-resize.png`、`09-font-zoom.png` 和 result.json：记录窗口尺寸、各 shell PID/目录，确认不靠重启实现。
5. Cairo 的粉色色块是失败，不可拿命令成功替代视觉通过。GL 通过仅限记录的隔离环境。
6. 真正接入 App 前，另测中文输入法、Wayland、跨屏、热更新、主题保存重开及独立入口恢复。本轮没有宣称这些完成。
