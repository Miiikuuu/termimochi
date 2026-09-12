# Native preview implementation and acceptance — 2026-09-12

## Delivery status

**Runnable experimental App integration exists for both Ptyxis and Kitty. Full
acceptance is not complete.** Primary selection, comprehensive clipboard and
IME focus-edge coverage, hardware/desktop validation and some runtime-setting
capabilities remain open. No external user/usability testing was performed.

Baseline: main `3f3024216814ae79750db3e9fa37ac6afe6ecb66`, plus the local changes
delivered here. The five user-provided briefs were preserved. No commit, push,
merge, release, daily installation, terminal configuration, default terminal,
shell startup file or global session environment was changed.

See [architecture / preservation map](native-preview.md),
[locked dependencies](native-preview-dependencies.md), and the
[Chinese acceptance card](native-preview-acceptance-zh.md).

## Build evidence

The default and enabled builds were checked separately with CI's Rust 1.92.0:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`: **408 passed**, 73 GUI/opt-in tests ignored.
- `cargo build --workspace --release --locked`
- Enabled Clippy/tests/Release repeated with `--features native-preview`:
  **408 passed**, 82 GUI/opt-in tests ignored; ignored cases are not counted as passes.
- Isolated installer, desktop metadata validation and all **9 resource tests** passed.
- `python3 scripts/test-native-safety.py`: **6 passed** (owned-path RC policy,
  TTY/arbitrary-command/override rejection, private keyfile batch validation and
  A/B isolation, explicit reset, relative-backend rejection).
- Workspace crate packaging/verification completed; raw output is retained in
  the package log below. This is not a distribution package for the patched native dependencies.

Full build commands and results: [build log index](../target/qa/native-preview/build-checks-mced77ls/results.json).
Latest verification run: [final build checks](../target/qa/native-preview/build-checks-haqu3dhu/results.json).
Packaging: [package log](../target/qa/native-preview/build-checks-3ic9v_vo/package.log).

Release SHA-256:

| Artifact | SHA-256 |
| --- | --- |
| Default | `68e49ce2bc92d9ed637edb632c8ea41439e6e12b56519543b2af8654e2b3c9ba` |
| Enabled | `46ecf63a2773962d6802019d0163fedbd30ab696ca1689db21a45e7d6f33d3b5` |

The enabled normal App and test harness use the same production implementation;
tests are opt-in Rust test binaries, not the Release executable itself. Their
individual `build.json` records pin test/worker hashes, Casilda/Ptyxis hashes and
the input driver. Test-only fixes below changed harness hashes; this report is
a documented coverage union, **not a claim that all checks ran in one binary**.

## Native GUI evidence sources

All native matrices ran on owned Xvfb displays with private HOME/XDG/D-Bus and
private Fcitx5 Pinyin; outer GTK is X11, actual embedded clients are Wayland.
The tested renderer is Cairo/llvmpipe/native SHM. No daily display was driven.

| Evidence group | Scope / result |
| --- | --- |
| [Interaction matrix](../target/qa/native-preview/termimochi-regression-native-aueuje1m/results.json) | Kitty and Ptyxis at 1×/2×: real shells, native tabs, IME, Unicode clipboard in both directions, top TUI hot sync, return shortcut and expansion |
| [Lifecycle / isolation matrix](../target/qa/native-preview/termimochi-regression-native-ppzs4x5a/results.json) | 13/14 passed; the 2× mixed-target capture raced the first client frame, as detailed below |
| [Mixed-target rerun](../target/qa/native-preview/termimochi-regression-native-jmzjaf97/results.json) | 1× and 2× passed after waiting for an actual first frame |
| [Existing-feature GUI regressions](../target/qa/native-preview/enabled-c7d12fv8/results.json) | 12/13 passed; one runner isolation-name guard blocked the final case, not an App assertion |
| [Ptyxis window-top rerun](../target/qa/native-preview/termimochi-regression-native-21gcxujy/results.json) | Passed with the required isolated fixture-directory prefix; no guard disabled |

The existing-feature set covers the color picker, Full composition/navigation,
Inspect/VTE hit testing, workspace save/restore, theme-window color/GIF isolation,
Kitty vertical edit/save/reopen, reviewed use/stale-trial guards, daily launcher
review/recovery, scheme review/restore and both window-top editors. This is a
selected regression set, **not all 73 pre-existing opt-in tests**.

### Actual native screenshots

- [Kitty App and real IME candidates](../target/qa/native-preview/termimochi-regression-native-aueuje1m/1x-native_interactive_kitty_enabled/cache/native-app-candidates.png)
- [Ptyxis App and real IME candidates](../target/qa/native-preview/termimochi-regression-native-aueuje1m/1x-native_interactive_ptyxis_enabled/cache/native-app-candidates.png)
- [Kitty native Greeting + Prompt](../target/qa/native-preview/termimochi-regression-native-aueuje1m/1x-native_interactive_kitty_enabled/cache/native-greeting-prompt.png)
- [Ptyxis native Greeting + Prompt](../target/qa/native-preview/termimochi-regression-native-aueuje1m/1x-native_interactive_ptyxis_enabled/cache/native-greeting-prompt.png)
- [Kitty expanded real terminal](../target/qa/native-preview/termimochi-regression-native-aueuje1m/1x-native_interactive_kitty_enabled/cache/native-expanded.png)
- [Actual Ptyxis titlebar color](../target/qa/native-preview/termimochi-regression-native-ppzs4x5a/1x-native_isolation_ptyxis_three_instances/cache/native-titlebar-color.png)
- [Kitty animation motion samples](../target/qa/native-preview/termimochi-regression-native-aueuje1m/1x-native_interactive_kitty_enabled/cache/native-gif-motion.json): centroid movement of the fixture's red block over 12 actual client textures, excluding cursor blinking as evidence. Matching PNG frames are alongside it.

These are local raw artifacts under ignored `target/qa`, not automatically
uploaded evidence. Commands and fixture text appear in some screenshots; no
real clipboard contents or shell history were used.

## NP01–NP24 acceptance ledger

“Partial” means the implemented/tested subset is explicit; it is not a passed
whole row. “Unverified” is different from “not implemented” and “failed”.

| ID | Status | Evidence / remaining boundary |
| --- | --- | --- |
| NP01 | Pass in tested environment | Actual project Ptyxis 50.1, private Wayland client inside App; real CSD and tabs |
| NP02 | Pass in tested environment | Actual Kitty 0.45.0 inside the same Casilda host abstraction |
| NP03 | Partial | `cd`/cwd, shell output, Ctrl+C and actual `top -p $$` tested; completion and broader TUI matrix not separately exercised |
| NP04 | Partial | Two native tabs have different shell PIDs and the second uses current controlled Starship; comprehensive mouse switching/history checks remain |
| NP05 | Pass in tested environment | Ptyxis A/B/mock-daily private roots; A body/font/titlebar changed, B/C pixels and keyfile bytes unchanged before and after A stopped; helper has no dconf fallback |
| NP06 | Pass in tested environment | Kitty A/B/mock-daily appearance/endpoint isolation; immutable policy restricts RC to owned config reload |
| NP07 | Partial | Ctrl+C/Z/S/Q/Shift+T and reserved return shortcut tested; editor/native focus and modifier edge cases need fuller coverage |
| NP08 | Partial | Real Fcitx5 Pinyin preedit, candidates, commit “你好”, Esc cancellation in both targets at 1×/2×; IME focus-loss/candidate-menu edge cases and IBus remain unverified |
| NP09 | Partial | Single-line Unicode host→native paste and native selection→host copy tested; long/multiline/bracketed-paste edge coverage unverified; primary selection/non-text formats not implemented |
| NP10 | Pass in tested environment | During hot color/font updates, native top PID, shell PID and cwd remain; PTY grid changes with font size |
| NP11 | Partial | Real tabs/CSD; Ptyxis titlebar pixels hot-update; complete native menu/decoration and all Kitty top-style combinations not revalidated in nested environment |
| NP12 | Partial | Actual Kitty GIF frame motion tested; Ptyxis pixel mode is explicitly rejected, not replaced by fake GTK graphics; more image/protocol combinations unverified |
| NP13 | Pass in tested environment | Removing explicit colors/font returns Kitty black / Ptyxis GNOME white controlled baselines without replacing shell; theme remains sparse |
| NP14 | Partial | Theme/target rejection, per-session bounded worker and stale-result guards implemented; adversarial rapid close/convert/update interleavings not exhaustively tested |
| NP15 | Partial | Ptyxis managed-key temporary differences detected and explicit Resync tested without design mutation; Kitty font-shortcut warning exists, comprehensive Kitty runtime-query comparison not implemented |
| NP16 | Partial | Current Prompt/Greeting startup, second-tab Prompt and Kitty GIF tested; changes report Stop/Start and do not inject commands; full interactive editing matrix while TUI runs remains unverified |
| NP17 | Partial | 1×/2×, actual expansion and font-driven PTY reflow tested; fraction scaling, popup placement and cross-monitor cases unverified |
| NP18 | Partial | Normal Stop/reap, killed native-client recovery, restarting and asset release tested; App SIGKILL, supervisor SIGKILL and all cancellation/close-dialog cases unverified |
| NP19 | Partial | Four 30-cycle runs show flat FDs and bounded observed host RSS; long idle/GIF CPU and soak testing not performed |
| NP20 | Partial | Existing save/reopen/undo regressions pass; runtime does not serialize into theme; exhaustive execution-consent/reopen combinations not tested |
| NP21 | Pass for local builds | Separate default/enabled Clippy/tests/Release and default installer/resources; optional runtime library is required only by the enabled binary; other distributions not built |
| NP22 | Pass in tested environment | Two Ptyxis + one Kitty simultaneously; private settings/pixels; stopping one leaves others alive |
| NP23 | Partial | 408 default and enabled non-opt-in tests, selected 13 GUI regressions after guarded rerun; entire old opt-in matrix not rerun |
| NP24 | Explicit limitation | Xvfb/X11 outer + Wayland inner is proven; daily GNOME Wayland, GPU hardware, fractional scale, IBus and human usability remain unverified |

## Failures found and handling

- **Stock Ptyxis private scope: environment-blocked.** The native process
  tried a systemd user scope on a private bus without a user manager. The local
  flag-gated source patch fixes this path without borrowing the daily bus.
  [Decomposed preflight results](../target/qa/native-preview/probe-lev8ujg3/results.json)
  retain stock failure, native private startup and embedded cases separately.
- **Casilda initial keymap/combined modifiers: fixed.** Kitty initially received
  an invalid keymap; combined Ctrl+Shift was dropped. Both native tab tests now pass.
- **IME context resets/reference cycle: fixed.** Resetting the context on each
  protocol commit destroyed composition; binding the client widget too early
  retained the compositor. Context lifetime now follows realize/unrealize.
- **Cursor listener lifetime: fixed.** Client exit could leave a stale cursor
  listener and crash the host. Destroy-listener removal is now paired with reset.
- **FD leak: fixed.** Initial 30-cycle runs leaked five FDs per host. Current
  runs destroy the actual GObject, keep FDs flat and reap recorded shells.
- **Kitty DMA-BUF: failed in Xvfb/llvmpipe.** Corrupt glyphs were observed. Native
  SHM/software is the default tested fallback. Hardware DMA-BUF remains unverified,
  not certified by successful SHM rendering.
- **Copy-test targeting: corrected, not a bridge bypass.** The early test used
  stale geometry / a client edge as its drag anchor and kept reading a sentinel.
  It now uses current widget/surface geometry and native line selection away
  from resize borders, then reads the host clipboard. It does not directly set
  the expected result or issue a remote-control copy command.
- **Mixed-target first-frame race: corrected in test.** Shell existence did not
  imply the first native buffer had been presented. Capture now waits for a
  real render node with a bounded deadline, rather than replacing it with a fake image.
- **Old Ptyxis regression runner guard: corrected.** The test required its
  known isolated `termimochi-regression-` directory prefix. The new runner now
  provides that prefix; the safety assertion remains intact.

## Measured lifecycle performance

From the complete four 30-cycle runs in `termimochi-regression-native-ppzs4x5a`:

| Target / scale | Host FDs after every stop | Observed host RSS MiB | Mean start-shell + stop/reap cycle |
| --- | --- | --- | --- |
| Kitty 1× | 16–16 | 226.4–227.1 | 1.879 s |
| Ptyxis 1× | 17–17 | 226.5–227.1 | 1.537 s |
| Kitty 2× | 16–16 | 249.2–249.4 | 1.871 s |
| Ptyxis 2× | 17–17 | 248.1–249.6 | 1.538 s |

RSS here is the App/test host after stopping, not total peak client-tree memory.
These are warm repeated-cycle observations, not cold-start benchmarks, latency
percentiles, zero-copy claims or a long-duration leak proof. Input latency,
CPU utilization and long idle/animation soak remain unmeasured.

## Open implementation / environment items

- **User-observed display defect, not yet reproduced/fixed:** a full-window
  native Kitty preview with a multi-field Greeting showed truncated system
  information despite abundant horizontal space. The existing simple Greeting
  fixture did not cover this composition. Initial sizing, terminal reflow and
  output geometry need investigation. The native controls/titlebar integration
  also remains visually crowded. This snapshot preserves that known issue;
  passing the smaller fixtures is not proof that this layout is correct.

- Primary selection and non-text clipboard formats: **not implemented**.
- Complete Kitty runtime-setting drift inspection: **not implemented**; current
  font-shortcut notice is conservative, not a full readback of all native preferences.
- Ptyxis Prompt/Greeting still shares a planner that requires installed Kitty:
  **implementation debt**, even though Ptyxis renders/runs the actual session itself.
- Pixel Greeting in Ptyxis: **not supported by this adapter**; explicit rejection
  preserves the artwork and guides Character selection.
- Hardware GPU/DMA-BUF, host Wayland, fractional scale, cross-monitor, IBus,
  long/multiline clipboard and IME focus-edge matrix: **unverified**.
- Human visual approval and real-user usability: **not performed**.
- Default CI's Ubuntu 24.04 is below the locked Casilda/GTK requirement. The
  enabled local matrix does not imply that CI ran the native feature remotely.

Start locally with `bash scripts/run-native-preview.sh`. Do not run the ordinary
installer to obtain this experimental backend. Save and the existing reviewed
Use/Apply flows remain separate from starting native preview.
