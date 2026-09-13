# Isolated Release acceptance — 2026-09-13

Base: `main`, `13faee83b8001ef36a24d17d414ad0cdecfd1016`, plus this
uncommitted QA/fix patch. No commit, push, publication, daily installation,
default-terminal change or daily startup/configuration write was performed.
User task briefs and the original **小猫测试** design were preserved.

## Findings fixed in production code

1. **Starship sample under a non-`/tmp` TMPDIR failed.** The sandbox used the
   fixed `/tmp/termimochi-language-preview` destination without mounting its
   source when the source lived elsewhere. Synthetic samples now always get
   their read-only bind. Existing trust checks, sandbox and bounded execution
   stay intact; no unsandboxed fallback was added. A new test exercises a
   source outside `/tmp`, successful Rust projection and unchanged source bytes.
2. **Independent Greeting canvas missed per-window palette CSS.** It now gets
   the same window-local color scope as the terminal shell. Two simultaneously
   open GIF designs are sampled from actual GTK textures before/after editing
   the other window, including the separate Greeting scene.
3. **Greeting → Full could crash during VTE realization.** Mapping the stack
   synchronously refreshed the target while `refresh_output_bar` held a
   `RefCell` borrow of that target. It now takes a read-only snapshot before
   crossing GTK callbacks. The reproducing multi-window scene transition stays
   in the regression; the actual Release GUI also navigates Greeting → Full.

No theme schema, command set, Apply/restore contract or preview architecture was
changed. Full is still a design sample, not Kitty/Sixel protocol certification.

## Evidence levels and results

These are local results, not an online CI result. Ignored tests are not passes.
The selected GUI matrix does not claim every ignored test in the repository.

| Check | Evidence and scope |
| --- | --- |
| Default + `native-preview` boundaries | [10/10 build gates](../target/qa/native-preview/build-checks-n37t6lka/results.json): formatting, both strict Clippy checks, both workspace suites, both Release builds, isolated installer, metadata tests and validation |
| Workspace suites | 419 passed per feature mode; default 87 ignored, native-enabled 96 ignored. These are development test harnesses, not GUI execution evidence. |
| Optimized workspace suite | [419 passed, 0 failed, 87 ignored](../target/qa/release-acceptance/workspace-release-tests-delivery.log), counted separately from the development suites |
| Selected optimized GTK matrix | [46/46 passed, 0 failed, 0 missing-fixture cases](../target/qa/release-acceptance/termimochi-regression-r44q7qi1/results.json): 23 cases at both 1× and 2×. Separate optimized harness and actual production worker are [pinned and hashed](../target/qa/release-acceptance/termimochi-regression-r44q7qi1/build.json) by `test-regression.py --release`. This is one complete final matrix, not a union of partial reruns. |
| Actual standalone Release App | [Black-box result](../target/qa/release-acceptance/blackbox-wul4nzvq/result.json): real GUI navigation, font editing, Ctrl+S, exact document comparison, new-process reopen, visible external-conflict report and protected-byte comparison. No Rust controller hooks. |
| Native-enabled execution | [8/8](../target/qa/native-preview/termimochi-regression-native-ymj1mirg/results.json): actual Kitty/Ptyxis embedding and Fcitx/sample keyboard checks at 1×/2×. This is an enabled development harness, not claimed as an optimized native GUI matrix. |

Default production application SHA256:
`6071fe9bd95d498a661a754f0d1212ca5204226d0a44428625496a4375fdbbe1`.
The independent black-box process and the runtime retained by the Kitty desktop
launcher use this same production binary, not the optimized Rust test harness.
The black-box copies the executable before launching it, so subsequent builds
cannot silently change what is under test.

Optimized GTK harness SHA256:
`808e212554366bb6c8c60cd0dddd6ccbab33f207518e835468d1f294f33b5622`.
The default executable is left at `target/release/termimochi`, matches the final
black-box and retained-launcher runtime, and passes `--help` without the private
native library path. To launch state-isolated, use `bash scripts/run-isolated.sh`
with a private design copy. The original desktop theme's SHA256 remains
`283f0aaf3c14bfcb01826eedd834054fa3abcec0b91f75a167c474836522b13e`.

### Configuration and lifecycle coverage

| Flow | What is actually checked |
| --- | --- |
| Save / reopen | Edit private theme from 16 to 18 pt in the standalone GUI; actual Ctrl+S; JSON must equal the original except that one owned value. Terminate after saving, start a different process, observe 18 pt. Identity, target, Prompt and embedded GIF remain unchanged. |
| External Save conflict | Independently change disk to 19 while the editor owns 20; Ctrl+S must visibly report “changed outside this window” and preserve the external bytes. |
| No accidental application | Hash private Kitty, Starship, Fastfetch, Bash startup and desktop sentinels before/after; enforce memory GSettings to prevent persistent dconf writes. This backend guard does not inspect the App's in-process settings (a separate memory-backend gsettings process cannot do that); optimized GTK adapter tests cover those settings. Save is not Apply. |
| GUI application gates | Existing optimized GTK cases exercise cancellation, review, validation/stale-trial guards, write scopes, success results, reopen and recovery. These use controller access, clearly separate from the standalone black-box. |
| Desktop entry update | Publish a new independent Kitty version; assert publication alone does not retarget the launcher. Explicitly update the entry; open through GIO twice and query real Kitty palette/font, shell/Prompt and GIF movement. |
| Recovery | Reject stale recovery of the old entry. Restore the newer deployment and launcher; `current.json`, `.desktop` and `kitty.conf` must exactly match the original bytes. Open the restored entry twice through GIO; verify the original appearance. |
| Damage and isolation | Retain startup tampering, deactivated-entry recovery, external-file conflicts, cancelled artwork import, unsaved draft protection and two-window async/undo/color/GIF isolation checks. |
| Complex preview | Use the private 小猫测试 copy and matched font fixture. Actual/Fit/Fixed tests perform real wheel/pointer/input operations and check reachable cells, clipping, caret geometry and history scroll preservation. String presence alone is not a visibility pass. |

The desktop test creates its own private Bash fixture, not a copy executed from
the user's daily `.bashrc`. Its remote-control directives are test-only settings
in the private Kitty configuration; production projection remains unchanged.
GIO activation tests the desktop entry's real Exec chain, not GNOME's daily
application-menu indexing or shell cache.

### Native screenshots

Actual standalone production GUI:

- [Greeting scene](../target/qa/release-acceptance/blackbox-wul4nzvq/greeting-scene.png).
- [Edited](../target/qa/release-acceptance/blackbox-wul4nzvq/edited.png),
  [reopened in a new process](../target/qa/release-acceptance/blackbox-wul4nzvq/reopened.png).
- [Visible save conflict](../target/qa/release-acceptance/blackbox-wul4nzvq/save-conflict.png),
  [protected bytes before](../target/qa/release-acceptance/blackbox-wul4nzvq/protected-before.json),
  [after](../target/qa/release-acceptance/blackbox-wul4nzvq/protected-after.json).

Supplementary actual native-enabled GTK/Fcitx evidence:

- [Kitty after hot update](../target/qa/native-preview/termimochi-regression-native-ymj1mirg/1x-native_interactive_kitty_enabled/cache/native-app-after-hot.png).
- [Ptyxis after hot update](../target/qa/native-preview/termimochi-regression-native-ymj1mirg/1x-native_interactive_ptyxis_enabled/cache/native-app-after-hot.png).
- [Sample IME candidates](../target/qa/native-preview/termimochi-regression-native-ymj1mirg/1x-interactive_samples_keyboard/cache/sample-ime-candidates.png).

Actual GIO → retained Release runtime → Kitty (private synthetic moving-GIF
fixture, **not** the user's complex image):

- [Original entry reopened](../target/qa/release-acceptance/termimochi-regression-r44q7qi1/1x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/daily-launcher-evidence/reopen-0.png).
- [Updated entry reopened](../target/qa/release-acceptance/termimochi-regression-r44q7qi1/1x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/daily-launcher-evidence/updated/reopen-0.png).
- [Restored entry reopened](../target/qa/release-acceptance/termimochi-regression-r44q7qi1/1x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/daily-launcher-evidence/restored/reopen-0.png).
- [Before/update/restore hashes](../target/qa/release-acceptance/termimochi-regression-r44q7qi1/1x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/launcher-update-recovery.json).

Each phase has eight actual captures per open; movement is asserted across
frames. The selected PNG alone does not prove animation. Recovery compares
the full bytes, not only the displayed background or file existence.

Raw PNGs, logs, exact executables and private fixtures stay in ignored `target/qa`;
they are local evidence, not automatically uploaded public assets. Black-box
screenshots use isolated preview defaults, so they are not claimed to reproduce
the user's daily font/profile. The complex reachability cases separately seed
the documented exact reference fixture.

## Failure ledger (not erased by successful retests)

- Initial Release workspace run: two Starship sample failures. Serial replay
  reproduced them; the alternate-TMPDIR production bug above was fixed.
- Initial selected matrix `termimochi-regression-3m5o8cur`: 38/46. It exposed
  missing Greeting canvas CSS and stale test assumptions: sampling the separately
  styled titlebar instead of terminal body; checking before the 220 ms coalescer
  started; expecting frozen two-command history after a new sample session.
  Tests now check actual body pixels, await coalescing, and explicitly generate
  finite sample history where two Prompts are required.
- Extended targeted run `termimochi-regression-83in91ne`: 6/7. The new
  Greeting → Full transition aborted on a `RefCell` borrow; fixed in production.
- `termimochi-regression-p4ab38pv`: 45/46; one 1× animated texture capture initially
  returned no GTK render node. Capture now waits at most five seconds for a real
  frame; it does not retry a wrong color or weaken pixel equality.
- Early black-box driver attempts failed on AT-SPI focus, duplicate labels and
  dropdown navigation. The final driver uses role-specific values, process-ID
  filtering, the GTK toggle accessibility action and actual popup keyboard
  navigation. Its conflict check awaits the visible error, not just unchanged
  bytes. Earlier passing runs with older binary hashes are not final evidence.

## Reproduction

CI pins Rust **1.92.0**. This machine uses the already-provisioned private Rust
toolchain and native prefix; this task did not install tools into the daily
environment. From the repository, use the existing QA environment:

```bash
export RUSTUP_HOME="$PWD/target/qa/typed-toolchain/rustup" RUSTUP_TOOLCHAIN=1.92.0
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export PKG_CONFIG_PATH="$PWD/target/native-preview/prefix/lib/pkgconfig:$PWD/target/native-preview/prefix/usr/lib/x86_64-linux-gnu/pkgconfig"
export LD_LIBRARY_PATH="$PWD/target/native-preview/prefix/lib:$PWD/target/native-preview/prefix/usr/lib/x86_64-linux-gnu"
export TMPDIR="$PWD/target/qa/release-acceptance"
export FONTCONFIG_FILE="$PWD/target/qa/reachability/fonts.conf"
export TERMIMOCHI_REACHABILITY_THEME="$PWD/target/qa/reachability/reproduction.termimochi-design.json"
cargo test --workspace --release --locked
```

With a dedicated owned Xvfb `:96` already running (never the daily display):

```bash
python3 scripts/test-regression.py --release --display :96 --scale 1 --scale 2 \
  --filter theme_workspace --filter scheme_ --filter launcher_ --filter typed_ \
  --filter controlled_kitty --filter pixel_trial_review_gate --filter greeting_sync_load \
  --filter artwork_import_cancel --filter preview_complex_greeting --filter interactive_samples_keyboard
```

The standalone black-box owns `:98`, private D-Bus, HOME/XDG and memory GSettings:

```bash
python3 scripts/test-release-app.py --binary target/release/termimochi \
  --theme target/qa/reachability/reproduction.termimochi-design.json \
  --xvfb target/qa/native-tools/termimochi-gif-qa.Zn3tVv/runtime/usr/bin/Xvfb
```

`--xvfb` can instead point to another already installed Xvfb; without it the
script checks PATH. Requires system Python GI/AT-SPI, Pillow, XTest, xwininfo and
the Ptyxis schema. Missing dependencies are failures, not silent passes. The
script edits only a private design copy and retains evidence; it is state
isolation, not a filesystem-access sandbox. Display collision is a hard stop.

## Open / unverified boundaries

- **Known unresolved native issue:** the earlier complex-Greeting native Kitty
  clipping/placement limitation is not declared fixed by Full GTK reachability
  or the simpler native launcher fixture. See the previous reachability report.
- **Not run in this task:** actual daily GNOME/Wayland, fractional compositor
  scaling, IBus, logout/login menu refresh, power-loss recovery and arbitrary
  personal shell startup scripts. No external usability testers are available.
- **Not equivalent evidence:** memory-GSettings Apply checks do not certify
  daily Ptyxis profile application. Isolated embedded Ptyxis execution is a
  separate check. Neither GTK GIF playback nor export grants real-protocol
  verification.
- **Environment-blocked / existing native gaps:** stock Ptyxis's private-bus
  systemd-scope behavior remains blocked; the passing embedded tests use the
  existing private patched Ptyxis build. SHM tests do not certify DMA-BUF.
  Primary selection/non-text clipboard and complete Kitty drift inspection
  remain unimplemented; see the [native ledger](native-preview-qa.md#open-implementation--environment-items).
- **Not implemented by this patch:** new terminal adapters, Zsh/Fish launcher
  support or repairs to the pre-existing native limitation above.
- No new online CI, RustSec audit, package publication or daily installation was
  performed; local gates do not stand in for these. Any release decision must
  account for the known limitation and unverified environments.

For a non-source-reading checklist, use the
[Chinese acceptance card](release-acceptance-zh.md).
