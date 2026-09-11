# Unified scheme implementation — 2026-09-10

## Delivery and boundaries

Implemented on `workbench/unified-greeting-scheme`, based on
`8adb7288295261cfaa6918e9ae8af10692e524fc`. At the initial handoff, changes were uncommitted. No applicable
AGENTS.md was found. The supplied brief remains a separate user-owned file.
No daily terminal configuration, startup file or installed application was changed;
no commit, push, merge or publication was performed during that implementation run. Installer checks use temporary
HOME/prefix/data/state trees, not the user's installation.

Build: `cargo build --workspace --release --locked`.
Artifact: `target/release/termimochi` (plus `termimochi-cli`).
Safe visible launch: `bash scripts/run-isolated.sh`.
See the [Chinese acceptance card](acceptance-card-zh.md) for a task-based walkthrough.
The launcher isolates state, not arbitrary file access: open test copies only.

Initial handoff GUI SHA-256: `f59b5f218fcd89cbcb0af5839ef54a5139c4e951d4bb9bbc18e253c79dd93775`.
This is a native build for the tested host, not a universal self-contained Linux bundle.

## What changed

### Follow-up: palette synchronization — 2026-09-11

Corrected the inverted vertical hue scale: its values now increase downwards,
matching the CSS spectrum and saturation/value square. A native regression
failed on the old direction and now checks thumb geometry, rendered spectrum
and square pixels, HEX/RGB updates and external swatch selection at 1× and 2×.
Both runs pass, as do four related native shared-color/chrome checks. Workspace
tests remain 348 passed (53 native cases opt-in); formatting, Clippy with warnings
denied and the release build pass. Earlier whole-matrix/environment limitations
below remain unchanged; that matrix was not rerun for this fix.

Local evidence: `target/qa/picker-workspace-tests.log`,
`target/qa/termimochi-regression-hotv6nf8/results.json` (2/2), and
`target/qa/termimochi-regression-c5uexgsc/results.json` (4/4).
Updated GUI SHA-256: `7828df414e2a8c3cb6cb231f9baf1c20818979db8267e31107170332271effb6`.

### Unified scheme implementation

1. **Portable design:** `GreetingSettings.presentation` owns Auto / Image /
   Animation / Character, column occupancy, character style, optional protocol
   hint and explicit fallback preference. History, complete schemes and Greeting
   presets carry it with the retained original and recipe. Legacy documents remain
   Character and infer size/style from their existing recipe without re-rendering.
   The existing outer v1 document stays readable; new optional data is defaulted
   only on import. Typed decoding precedes migration so duplicate/unknown-field
   and size rejection are not weakened. Older app versions may reject new fields.
2. **Local state:** target choice is a local enum, not a portable authorization.
   Shared-pixel replacement consent is unchecked on each new window. A visual key
   binds source/recipe, geometry, protocol, target, executable, known target config
   contents and app cell/font geometry. Field-only edits preserve image evidence;
   a new trial always clears older approval, even when it fails. Reopening does
   not restore approval. Deployment comparison uses the reviewed destination bytes,
   separately from Save and visual approval.
3. **One Greeting output plan:** ordinary Greeting Apply and post-trial review
   delegate to Apply Scheme. Character updates the explicitly reviewed shared
   config. Pixels require matching approval, default to
   `data/termimochi/targets/<terminal>/config.jsonc`, and report installed/not
   enabled. Shared replacement has a visible cross-terminal warning. Current
   imported JSONC fields/commands are merged with verified assets, never executed
   by Apply. Reapply refreshes the shared snapshot instead of self-conflicting.
4. **Workbench:** main Save / Ctrl+S always saves a full scheme; named local presets
   and export copies remain under More. Greeting exposes display/occupancy/style
   in its normal editor; advanced compatibility stays expandable. Add image/GIF and
   Edit Artwork are near the top. Main Try opens the selected terminal and offers
   GUI feedback. The Greeting pixel canvas uses GTK composition/playback, with
   independent Fit/100% viewing state; other modules retain VTE test scenes.
5. **Recovery:** image application is an item in the existing per-run journal and
   Fastfetch backup/conflict system. Assets remain immutable and are retained on
   restore. Restoring an independent config never binds it as the shared target.
   Output state refreshes after restore. Pixel designs no longer trigger the old
   ANSI-only “Load Applied” discrepancy notice. Successful Greeting application
   still retains the existing local recovery preset; a failure to save that preset
   is shown separately and never marks the complete scheme saved. Full-config
   Run Once remains available from character results with a separate confirmation.

No new image format, adapter, icon system, dependency, frontend or shell installer
was introduced. Existing processing, sandboxing, original-source editing, JSONC/TOML
preservation, checked writes and per-module recovery are reused.

## Verification environment

Rust/Cargo 1.98.0; GTK 4.22.4; libadwaita 1.9.1; VTE 0.84.0;
Kitty 0.45.0; Fastfetch 2.57.1; Ptyxis 50.1; unpacked xterm/Xvfb test tools.
Native tests run on dedicated Xvfb `:94`, 3000×2200 physical pixels, at GDK_SCALE
1 and 2. Each case has separate HOME, XDG config/data/state/cache/runtime, a
private D-Bus session and memory GSettings. Wayland is removed from their env.
The harness refuses desktop `:0` / `:1`. Pointer capture targets only named test
windows. Test/worker binaries are pinned and hashed in each report.

Release smoke uses a separate Xvfb `:95` and the delivered isolated launcher.
These are actual GTK/VTE/Kitty/Xterm captures, not web approximations.

## Automated results

The authoritative run logs are under `target/qa/` (ignored, local artifacts):

| Check | Result / evidence |
| --- | --- |
| Formatting and diff whitespace | `cargo fmt --all -- --check`; `git diff --check` |
| Workspace tests | 348 passed; 52 display/native cases are opt-in, not silently counted as passed. `workspace-tests.log` |
| Clippy, all targets, warnings denied | Passed. `clippy.log` |
| Release build | Passed. `release-build.log` |
| Desktop validator tests | Passed, 9 cases. `desktop-tests.log` |
| Installed/embedded resource bytes | Passed, 17 resources plus desktop metadata and packaged copies. `desktop-validation.log` |
| Temporary install/uninstall | Passed; paths containing spaces, prior-icon backup and unrelated sentinels retained. `isolated-install.log` |
| Cargo packaging | Passed assembly and verification of all three crates. `packages.log` |
| Unpacked package tests | Passed: core 27, CLI 4 + 5 process contracts, GUI 312. See `package-core-tests.log`, `package-cli-tests.log`, `package-gui-tests.log`; local packaged core is patched into the other unpacked test copies, following CI. |
| Release GTK smoke | Passed 12-second observation with fatal GTK criticals enabled; expected timeout 124, no critical/panic. `release-gui-smoke.log`. This is liveness, not every function. |
| Native 1× / 2× matrix | **100/104 passed**: `termimochi-regression-v1m8ws03/results.json`. Four unsuccessful attempts (two Ptyxis-dependent tests × two scales) are environment-blocked below, never counted as passed. |

The initial whole matrix `termimochi-regression-m5ho8l95` was 86/104. It exposed
old Save/Apply UI assumptions, a target-config empty-directory fingerprint issue,
and the isolated Ptyxis environment failure. Its failures remain in the logs.
Tests were updated to exercise the new *explicit* preset, review and Run Once
actions, preserving conflict/restart/cancel assertions rather than deleting them.

After inspecting the minimum-size captures, Fit was corrected to measure the
information column and margins rather than subtract a fixed estimated width.
`termimochi-regression-p435ftdx` passes the scheme/save/zoom/geometry case at both
1× and 2×, including an explicit no-horizontal-overflow assertion. Its screenshots
are the final geometry evidence: `cache/scheme-1024x700.png` and
`cache/scheme-1280x900.png` beneath each case directory. Test palette diagnostics
may intentionally show a contrast issue; that is not a GTK crash or a claim that
all user-selected palettes pass contrast checks.

The final formatting, unit tests, Clippy, release build and unpacked package tests
were rerun after the Fit correction. Only this presentation geometry changed after
the full matrix was pinned; its dedicated 2/2 rerun uses the final source. Test Xvfb
servers and the diagnostic Ptyxis instance were stopped after verification; logs,
screenshots, user-test sessions and release artifacts are retained locally.

### Acceptance contract

| Brief item | Executable coverage / boundary |
| --- | --- |
| 1. Animation survives Save/reopen | `scheme_presentation_save_reopen_zoom_and_main_actions`; original + Animation + 48 columns persist, approval does not. |
| 2. Reapply cannot silently turn pixels into ANSI | `pixel_trial_review_gate_cancel_apply_and_restore`; independent apply, field edits, repeated shared image apply and explicit Character switch. |
| 3. Save means full scheme everywhere | Same scheme test dispatches main Save from all five modules; existing Ctrl+S binding still targets that action. Explicit preset tests retained. |
| 4. Target/environment invalidation | Portable-key unit tests, native GUI feedback test, config-font change and retry rejection. Dynamic compositor/remote state remains unverified. |
| 5. Cancel/open/save/close do not apply | Native sentinel assertions, temporary trial tests, workspace and reviewed-write cancellation tests. |
| 6. Scope is visible before confirmation | Unified scheme review GUI, global/profile GSettings tests, shared versus independent image review assertions. |
| 7. Unknown is not supported | Actual terminal protocol tests plus staged GUI response unit test; static ACK never approves GIF; explicit Character alternative. Isolated Ptyxis path blocked below. |
| 8. Conflicts and partial recovery | Existing scheme/file tests and native field/sync tests; late conflict preserves external contents, independent items retain results, restore is checked. |
| 9. Occupancy versus zoom | Scheme GUI test verifies zoom/play/scroll do not dirty; width edit/Undo and legacy 144-column ASCII migration test retain intent/recipe. |
| 10. Safe trial versus full config | Safe-config tests and `verified_pixels_merge_current_jsonc_fields_and_commands_without_running_them`; imported commands retained only for review/full output. |
| 11. Existing safety | Full workspace suite, original-source restart tests, SVG worker limits, real ANSI/PNG/Sixel/GIF rendering, managed assets and repeated immediate Kitty starts. |
| 12. Geometry, keyboard, Inspect | 1024×700 and 1280×900 GTK screenshots at 1×/2×; pointer/wheel/Inspect/scroll/stress cases. Wayland fractional scaling is not covered by Xvfb. |

## Explicitly pending, not passed

- **Ptyxis 50.1 under fully private runtime/user bus:** the GUI opens a “Failed”
  tab before running the command: “Failed to connect to user scope bus via local
  transport”. This build's agent invokes `systemd-run --user --scope`; no user
  manager exists in this isolated session. A short private runtime path did not
  resolve it. The one-shot Ptyxis test and the Ptyxis branch of the combined
  protocol test remain environment-blocked. Kitty/Xterm subcases are individually
  recorded, not used to declare that combined test passed. The real user's bus was
  not connected to bypass the boundary. Recheck in a disposable logged-in desktop/VM.
- **Wayland and fractional scaling**, real compositor DPI changes, multiple
  monitors and temporary external-terminal zoom. Xvfb 2× is not fractional-scale
  evidence. Known config fingerprints do not cover runtime zoom, external includes,
  reloaded X resources, SSH or multiplexers. Retest after those changes.
- **Usability with 3–5 new people:** unavailable; no completion-rate, time-on-task
  or “users understand it” claim is made. The acceptance card is a handoff, not a
  substitute for observing new users.
- **RustSec fresh audit:** cargo-audit is not installed; no tool was installed to
  claim this check passed. Existing CI audit remains separate.
- **Whole imported commands/network modules in actual daily shells:** intentionally
  excluded from safe trials and this run. Apply reviews/preserves them but does not
  certify their execution. Original project privacy/sandbox guarantees remain.

GTK composition is not pixel-identical to native terminal output: text styling,
font cell metrics, wrapping and Sixel alpha/quantization can differ. Approval is
current-session evidence for a specific output, not universal compatibility.
Independent image installation intentionally does not enable startup; automatic
per-terminal routing and embedded VTE graphics protocols remain out of scope.
