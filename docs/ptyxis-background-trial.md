# Experimental Ptyxis background trial

For the subsequently added **in-App design background**, see
[Image background in the design preview](background-design-preview.md). That
preview does not launch Ptyxis and does not require this optional runtime.

This is a local, optional enhanced terminal, not an update to system Ptyxis.
The normal release package does not contain the patched terminal. The workbench
now saves backgrounds inside themes and can open their image/framing directly in
this runtime; stock Ptyxis Apply still cannot enable an image background.

## Try it on the development machine

From the repository:

```sh
bash scripts/run-background-trial.sh
```

1. Choose **Choose Background Image…** and select a static PNG, JPG or WebP.
2. Select **Cover** (fill, may crop) or **Contain** (whole image, may leave margins).
3. Adjust **Background color overlay**: higher values obscure the image with the
   terminal's background color, improving text contrast.
4. Click **Open Real Terminal**. The adjacent notice explains that commands have
   your user permissions; this one click authorizes opening the temporary shell.
   There is no additional consent checkbox. Merely selecting an image does not
   execute anything; session mechanics are under **Details · temporary session**.
5. Close the terminal normally, or use **Stop Trial…** and confirm. Stop terminates
   the trial's programs. After it ends, choose another image or change settings
   and start again. Settings do not hot-update a running trial.

To carry the current theme's colors, font and grid into the trial:

```sh
bash scripts/run-background-trial.sh --workbench
```

Open/create a **Ptyxis** theme, choose its background image, then **Background →
Open in Enhanced Ptyxis… → Open Real Terminal**. This takes the saved image,
zoom, position, overlay, palette,
font-family/size/weight and grid snapshot (bounded
to 20–240 columns and 10–100 rows). Cell spacing, other layout controls, Prompt,
Greeting and subsequent theme edits are not included. The action is unavailable for Kitty and advanced documents that
do not own Layout. Opening the runtime does not convert or dirty the document.
Closing and reopening the theme restores the image without its source path.
For a persistent appearance, use **Apply & Open** in the same theme review.
This creates a versioned, independent app-menu entry. Reopen/update/restore through
**⋮ → Enhanced Ptyxis — Saved Entries…**; an applied terminal survives editor exit.
The launcher retains its own TermiMochi helper, but the optional enhanced runtime
must remain at its reviewed path. This is not complete Prompt/Greeting or personal
Bash/Conda application. The native prepared canvas scales with the window, and
changing aspect ratio may stretch it. See the [acceptance card](background-theme-acceptance.md).

## Build locally

The runtime script uses the existing project-local native dependency prefix,
Meson/Ninja and a C compiler. It pins Ptyxis 50.1 at
`0f045a653db114e22fe7bf4b409fcb30d7763fa9`, applies the private-scope and background
patches, and installs **only** into `target/background-runtime/prefix`.

```sh
bash scripts/build-background-runtime.sh
CARGO_TARGET_DIR=target/background-trial-build cargo build --release --locked -p termimochi
```

Use the repository's CI-compatible Rust toolchain. On this development machine
it is available as:

```sh
RUSTUP_HOME="$PWD/target/qa/typed-toolchain/rustup" RUSTUP_TOOLCHAIN=1.92.0 \
  CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/background-trial-build \
  cargo build --release --locked -p termimochi
```

The launcher accepts `TERMIMOCHI_BACKGROUND_APP` and
`TERMIMOCHI_BACKGROUND_PREFIX` for explicit local builds. It never falls back to
the ordinary system Ptyxis. A missing runtime is an error, not a successful trial.

## Isolation and boundaries

- Each trial has an owned temporary HOME/XDG tree, keyfile GSettings, unique
  application/schema namespace and private D-Bus without service activation.
- The supervisor reaps the terminal's descendants before assets are released.
  Normal exit and explicit Stop clean temporary files. Abruptly killing the App
  stops its supervised processes but may leave a temporary directory; this is
  not a crash-proof disk cleanup service.
- No `.bashrc`, login startup files, default terminal or daily profiles are
  modified. The shell starts with a controlled rc file and a private HOME.
- **This is not a security sandbox.** Commands have the user's permissions and
  can access other files. Isolation protects automatic configuration behavior,
  not against commands the user chooses to run.
- Input files must be regular, non-aliased files, at most 16 MiB. Existing image
  decode limits apply (8192 pixels per side, 16 million pixels, 64 MiB decoded).
  A bounded worker converts to a maximum 1920×1080 static PNG, preserving aspect
  ratio and alpha. The terminal reads only that owned PNG. Animated backgrounds
  and SVG are not supported by this trial; existing Greeting import is unchanged.
- GL rendering is selected because the earlier Cairo control and background
  runs showed text artifacts. This is not certification of all GPU drivers.
- A temporary trial never records native theme validation or Apply success.
  Explicit **Apply & Open** records only the independent appearance publication;
  successfully creating files or spawning a process is not a visual compatibility pass.

## Checks and evidence

`scripts/test-background-trial.py` drives the actual optimized App and patched
Ptyxis in Xvfb, private D-Bus and a bubblewrap namespace with a read-only host.
It checks file selection, explicit button authorization, invalid input, real shell input, normal exit,
Stop cancellation and cleanup. Screenshots and private settings are retained in
the printed `target/qa/background-trial-*` directory. Screenshot appearance must
be inspected separately; a successful command is not visual proof.

Rust tests cover bounded conversion, private files, cleanup, invalid inputs and
the App action's target/document scope. Existing enabled-feature native tests
remain separate from this standalone background trial.

Wayland, hardware GPU rendering, real desktop window management and Chinese IME
in the background trial remain unverified. The underlying prototype's previous
tab/resize/control comparisons and known Cairo failure are in the
[research report](ptyxis-background-research-2026-09-23.md).

### Local acceptance — 2026-09-24

Built from `a5755b1976492c194b619a309d412aa4f1da6627` plus the uncommitted background
trial changes. This is **not** the published RC1 archive and no release was made.

| Check | Actual result |
| --- | --- |
| Default optimized workspace tests | 435 passed, 0 failed, 90 explicitly ignored |
| Formatting / diff whitespace | Passed |
| Default workspace Clippy, all targets, warnings denied | Passed |
| Enabled `native-preview` Clippy, all App targets, warnings denied | Passed |
| Native safety checks | 8 passed, including missing embedded FD and bus-start failure |
| Isolated GTK target/action/document regression | 1 passed |
| Enabled native Kitty + Ptyxis regression using the modified supervisor | 2 passed at scale 1; not a full native matrix |
| Final default Release standalone trial | 12 interaction/lifecycle checks passed; real Cover, Contain/tint, shell and cleanup screenshots inspected |

Final executable: `target/background-trial-build/release/termimochi`.
SHA-256: `7ace1d7a198e8fe2f93c0d83e389cdb9ea57d23b1459a76e0e668189fa125d75`.
Optional terminal: `target/background-runtime/prefix/bin/ptyxis`.
SHA-256: `8f23bef3856089f7bccac19f9c1b9a48eb35e32eda5e311da94b271566e9bef4`.
No rebuild was performed between that trial test and its handoff. These hashes
record the standalone-trial milestone; later workbench-preview changes rebuild
the same local App path and have their own evidence.

Local evidence (ignored by Git, not shipped):

- Final checks: `target/qa/background-trial-final-checks.log`.
- Enabled Clippy: `target/qa/background-trial-feature-clippy.log`.
- Safety: `target/qa/background-trial-native-safety.log`.
- Action regression: `target/qa/native-preview/termimochi-regression-native-vvhr9ip4`.
- New supervisor, actual enabled native tests:
  `target/qa/native-preview/termimochi-regression-native-0bh1wfap`.
  A project-local alternate prefix links the new supervisor; the existing native
  runtime prefix was not overwritten.
- Final trial: `target/qa/background-trial-weybo6dh/results.json`,
  `02-native-cover.png`, `03-native-shell.png`, `04-native-contain-tint.png`,
  `05-cleaned.png` and the private keyfile in that directory.

Earlier driver failures were retained rather than counted as passes: missing
AT-SPI activation, GNOME portal without an X11 file chooser, GTK4 checkbox-local
coordinates and trying to focus an unmapped X11 window. The final driver uses an
owned GTK portal configuration and waits for a viewable native window. These
test-harness corrections do not establish Wayland or desktop-menu compatibility.
The broad ignored GUI suite was not rerun. The App-to-trial theme snapshot has
data/action regression coverage, not a full real-desktop visual matrix.

At that milestone the native trial did not implement background persistence/Apply,
live background updates, animated backgrounds or production runtime packaging.
Subsequent theme persistence and current use boundaries are documented above.
Not verified: the environments listed above and forced-App-crash disk cleanup.

## 中文验收卡

| 操作 | 应看到的结果 |
| --- | --- |
| 运行试用脚本，选一张静态图 | 显示文件名；不会立即启动 Shell |
| 点 Open Real Terminal | 不必额外勾选；直接打开独立 Ptyxis，文字背后显示图片 |
| 输入 `pwd` | 显示临时 HOME，不是原终端的工作目录 |
| 关闭后改 Cover/Contain、遮罩再启动 | 裁剪/留白和图片明暗随设置变化 |
| Stop Trial → Keep Running | 原试用继续 |
| Stop Trial → Stop Trial | 结束试用；临时配置和图片被清理 |
| 从 Ptyxis 主题的 Background 入口启动 | 使用当前背景构图/颜色/字体/行列快照，主题不变脏 |
| 重新打开普通系统 Ptyxis | 没有被替换，日常主题及启动文件不变 |

Do not distribute the patched runtime without a separate dependency, GPL source
and packaging/provenance review. No system installation or release is performed
by this workflow.
