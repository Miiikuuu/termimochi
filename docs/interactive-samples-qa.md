# Interactive samples — QA ledger

Date: 2026-09-12. Base: `28e506ce304e334f0924e07aac77f9d55c530b96`
(`main`), plus the uncommitted implementation described in
[interactive-samples.md](interactive-samples.md). No commit, push, installation to
daily paths or changes to the user's terminal configuration were performed.
The six user-provided task briefs were retained.

## Build and environment

Rust 1.92.0 (same version as repository CI), Ubuntu 26.04.1, GTK 4.22.4,
libadwaita 1.9.1, VTE 0.84, GLib 2.88. The native tests use the existing local
Casilda 1.4 / wlroots 0.20.2 prefix, patched Ptyxis 50.1 and Kitty 0.45.
These are local checks, **not a GitHub CI result**.

GTK tests run on dedicated Xvfb displays, at 1× and 2×, with private HOME/XDG,
D-Bus and settings. Default tests use GTK's simple input method; enabled-feature
keyboard tests additionally use an isolated Fcitx Pinyin instance. Native rendering
uses the existing software/SHM test route, not a hardware Wayland validation.

Final release artifacts and all build logs:
`target/qa/native-preview/build-checks-kra2lchx/`.

| Gate | Default | `native-preview` |
| --- | --- | --- |
| Format | Passed | Same source |
| Clippy, all workspace targets, `-D warnings`, locked | Passed | Passed |
| Non-ignored workspace tests | 412 passed, 0 failed | 412 passed, 0 failed |
| Ignored App tests (not counted above) | 76 | 85 |
| Locked workspace release build | Passed | Passed |
| Temporary installer test | Passed | Not separately repeated |
| Desktop resource unit tests / validator | 9 passed / passed | Shared resources |

412 = 376 App + 4 CLI + 5 CLI integration + 27 core tests. Canned `cargo test`
sample output contributes **zero** to these totals. Ignored tests require explicit
runners; only the selected runs below were executed, not the entire historical
ignored suite. RustSec audit and online packaging CI were not rerun this turn;
Cargo manifests and lockfile are unchanged.

Pinned runnable binaries (relative to repository root):

- `target/qa/native-preview/build-checks-kra2lchx/termimochi-default`
  SHA-256 `3a2a2ed7b3dfac4e6699ffa52249695e72626e68c1cd4495a84b3c4a69244a99`
- `target/qa/native-preview/build-checks-kra2lchx/termimochi-enabled`
  SHA-256 `c9e33a498da3eb42d54f31c7b74579c633414f5ad4f34659964c95f1bdace902`
  (requires the local native prefix library environment).

Run the first path directly for the default App. To rebuild, use
`cargo run -p termimochi --locked`; this machine's private Rust toolchain can be
selected with `RUSTUP_HOME="$PWD/target/qa/typed-toolchain/rustup"`
and `RUSTUP_TOOLCHAIN=1.92.0`. For the advanced build use
`bash scripts/run-native-preview.sh` (sets the native library environment).

## Executed GUI and native checks

Each runner pins its test executable and helper; `build.json` in each report
records their hashes. These are different artifacts from the release binaries.

| Run | Result / exact scope |
| --- | --- |
| `/tmp/termimochi-regression-g2qijma3` | Earlier broad run: 18/20 passed; two Full Inspect checks failed because the picture had been scrolled offscreen. Retained, not relabeled as passing. |
| `/tmp/termimochi-regression-deu2211r` | Final Full composition rerun: 2/2 passed; current imported Prompt, reduced motion, fit, shared geometry, Inspect and scene independence at both scales. |
| `/tmp/termimochi-regression-da69nbgr` | Final broad default run: **20/20 passed** at 1×/2× on the final code, including all ten selected behaviors. Logs/screenshots/hash manifest also copied to `target/qa/interactive-samples/default-final/` without duplicating binaries. |
| `target/qa/native-preview/termimochi-regression-native-5ia6zhke` | 6/6 passed: Kitty and Ptyxis native interaction, plus sample keyboard with real Fcitx, all at both scales. Predates the final imported-Prompt projection correction. |
| `target/qa/native-preview/termimochi-regression-native-l4ifhthz` | Final native comparison rerun: 4/4 passed; both targets at both scales. Now waits for the imported `NATIVE_PROMPT` in the sample before recording the comparison. |
| `python3 scripts/test-native-safety.py` | 6 passed: private Ptyxis backend and immutable Kitty remote-control policy, including rejection of arbitrary commands. |
| `python3 -m py_compile scripts/sample-preview-input.py` / `git diff --check` | Passed |

The default matrix selects ten behaviors at each scale: sample vertical flow,
GIF anchors, real keyboard actions, Full composition, independent scenes,
multi-window GIF/pending-job isolation, multi-window colors, Kitty editor
save/reopen/top scope, Ptyxis titlebar palette/save and native Kitty top styles.
No result from the sample reducer is counted as target protocol verification.

Reproduction (use a dedicated Xvfb, never the user's display):

```bash
python3 scripts/test-regression.py --display :96 --scale 1 --scale 2 \
  --filter interactive_samples_ --filter full_session_live_ \
  --filter independent_preview_scenes_ --filter theme_windows_keep_ \
  --filter window_top_native_edit_ --filter window_top_ptyxis_ \
  --filter window_top_real_
python3 scripts/test-native-preview.py --display :97 --scale 1 --scale 2 \
  --filter native_interactive_ --filter interactive_samples_keyboard
python3 scripts/check-native-preview-builds.py
python3 scripts/test-native-safety.py
```

On this machine builds use `CARGO_INCREMENTAL=0`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` and the local Rust
variables above. The default runner was also given the same `PKG_CONFIG_PATH`
and `LD_LIBRARY_PATH` as `scripts/run-native-preview.sh`, to reuse the build
cache; the default binary does not depend on Casilda or the native prefix.

## Behavioral coverage and boundaries

| Brief IDs | Evidence and limits |
| --- | --- |
| IS01–04 | Default Full input mapped, no VTE PTY; two independent tabs; reducer rejects shell syntax/controls/unknown commands without mutating output. Core reducer has no execution dependency. GUI sends rejected input and checks design/output remain unchanged. No exhaustive syscall/exec trace or dependency-injected OS-denial harness was added. |
| IS05–06 | Palette does not refeed unchanged transcript; font changes preserve records/history/draft/cwd. Module changes keep Full. 1024×700 and 1280×900 captures at both scales; fit/scroll/cell geometry assertions. Does not prove every font/string/zoom combination. |
| IS07–09 | Current Greeting projection, repeated fastfetch block anchors, live paintable changes, clear/tab switch/font changes, generation stability and reduced motion. Target-specific top styles and Ptyxis character fallback. GIF frame checks, not one static image, establish animation. |
| IS10–11 | Actual Ctrl+S with App accelerators writes the design store; runtime input/history do not dirty/save the theme. Existing selected top save/reopen regressions pass. Entire historical apply/recovery GUI suite was not rerun. |
| IS12–14 | XTest keys plus real isolated Fcitx preedit/Enter/Chinese commit/Esc cancel; Unicode clipboard both ways; multi-line rejection; history/completion; Ctrl+Z/redo focus routing; Ctrl+C cancel; Ctrl+Shift+T. Full grapheme-by-grapheme movement/Delete matrix remains unverified. |
| IS15–16 | Pure tests exercise 2,000 commands and budget eviction; GUI runs 50 new/close cycles. Final run: FD 14→14 both scales; RSS 245832→246424 KiB at 1×, 260760→264408 KiB at 2×. This is a bounded smoke check, not a long-term texture/task leak profile. |
| IS17–18 | Existing two-window color and pending-image regressions, sample state owned per Workbench; Full Inspect mapping including clipped-image exclusion. Not every multi-window input/IME/undo permutation was exercised. |
| IS19–20 | Default/enabled gates, actual enabled native interaction, private backend/RC policy. Sample state is separate from verification; authorization/use/recovery retained. Native known issues below remain open. |

The basic path is: open theme, click input, type a supported command and Enter.
It requires **zero Start/Resync actions and zero execution confirmations**.
Creating another tab takes one `+` click; left editor changes keep the chosen
scene. Real application and native startup still require their existing review.

## Screenshot evidence and visual comparison

Final native captures are under
`target/qa/native-preview/termimochi-regression-native-l4ifhthz/`.
For each `1x-native_interactive_{kitty,ptyxis}_enabled/cache/` (also `2x-…`):

- `sample-same-theme-before-native.png`: actual GTK Full with imported Prompt.
- `native-greeting-prompt.png`: actual embedded target with the same author values.
- `sample-comparison.json`: recorded target, sample font/cell metrics and columns.
- `native-client-hot.png`, `native-app-after-hot.png`: live supported-setting updates.
- `native-app-ime-preedit.png`, `native-app-candidates.png`: actual IME interaction.
- Kitty `native-gif-00.png` through `native-gif-11.png`: changing native GIF frames.

The same-theme fixture uses background `#E8E5DE`, foreground `#31353C`, font size
13 and sample width 80 columns. At 1× the sample VTE reports 10×22 cells.
The native viewport can have a **different grid**: these captures prove use of
the same theme and content, not equal wrapping at an equal grid. Background
values are checked by the existing native assertions; no arbitrary image
similarity threshold was used.

Observed differences, not hidden as passes:

- Kitty/VTE font rasterization and native decorations differ. The sample title
  bar is a design projection, not a copied native widget.
- Current sample Greeting includes its designed welcome message; the imported
  native Fastfetch fixture emits only its configured modules. Some native
  runtime SGR key/prompt colors differ from the theme-indexed sample colors.
- Native field lines can clip at the narrower native viewport. Equal-grid
  wrapping, all two-tab geometry and every configured top style have not been
  jointly matched in one end-to-end comparison.
- Ptyxis uses its real Adwaita controls in native mode and an approximation in
  samples. Its Full character fallback is intentional, not pixel support.
- At narrow-window Fit, the transcript/image canvas shrinks but GTK input and
  clickable top controls keep their interactive size. The 1024-pixel screenshot
  therefore shows larger tab/input text than transcript text. This keeps controls
  usable, but does **not** satisfy a claim of identical top/body visual scale;
  tighter scaled-chrome fidelity remains incomplete.

Default screenshots include `samples-kitty-1024.png`, `samples-kitty-1280.png`,
`samples-gif-anchored.png` and `samples-ptyxis-characters.png` in the selected
matrix's per-case `cache/` directories. They are actual GTK snapshots, not mockups.

## Failures fixed during implementation

- DropDown model replacement inside selection notification caused repeated
  refresh and GObject criticals: retain an unchanged model and ignore redundant
  selection notifications.
- Multiline paste bypassed the outer Entry callback: guard its GtkText delegate
  too, and leave preedit key handling to GTK.
- Full scale request during allocation left an undersized scroll range: defer
  reflow and explicitly size the transformed content.
- Imported Starship sample incorrectly used `/` as sandbox cwd, excluding all
  PATH entries under the existing safety rule: render from an empty private
  temporary directory; do not weaken the sandbox or execute imported commands.
- Menu actions lacked canonical echoed text, and screenshot capture could race
  the first frame after resizing: use the same labeled actions as typed input
  and await a rendered GTK frame. Full Inspect now scrolls the target into view
  before asserting its hit region; offscreen hits remain rejected.

Earlier failed/timeout reports in `/tmp/termimochi-regression-*` and
`target/qa/native-preview/termimochi-regression-native-*` were not deleted.

## Remaining items, separated by status

**Known limitations / not implemented:** canned `cargo test` output is immediate,
not a timed progress animation. GTK input-edit undo resets on tab remount (drafts
and command history persist). Full native-fidelity/equal-grid verification is
incomplete as detailed above. No arbitrary command execution is intended.

**Known native issue, still open:** complex Greeting truncation/spacing from the
previous native-preview work, including the user's 2026-09-12 14:28:53 screenshot,
is not fixed merely by moving native mode into Advanced. The complete earlier
[native QA ledger](native-preview-qa.md) remains authoritative for its additional
gaps; it has not been erased or relabeled as passed.

**Environment not exercised:** hardware Wayland/DMA-BUF, fractional scaling,
IBus and other input methods, other target/library versions and desktops.
The available software/Xvfb native route was executable and passed selected
checks; this is not a claim that the hardware route was attempted and failed.

**Unverified:** external user usability (no testers), prolonged soak/resource
profiling, exhaustive input/Unicode navigation, complete syscall/write observation,
all historical ignored GUI regressions, and every image/format/layout combination.
These do not count as passed. See the
[Chinese manual card](interactive-samples-acceptance-zh.md) for a non-source-code
walkthrough of the delivered behavior.
