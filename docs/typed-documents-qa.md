# Typed documents: verification record

Historical v2 baseline. This is not evidence for the theme-workspace correction;
see [current theme-workspace QA](theme-workspace-qa.md).

This is local implementation/verification evidence, not an online CI result or
external-user usability study. Ignored, blocked, unavailable, unimplemented and
not-run checks are not passes. Baseline results are separated from integrated
results below; repeated runs are not added together as independent coverage.

Companion handoff: [architecture](typed-documents-architecture.md),
[complete feature retention ledger](typed-documents-feature-map.md),
[actual document/target capabilities](typed-documents-capabilities.md), and
[Chinese manual acceptance card](typed-documents-acceptance-zh.md).

## Integrated local checks

Rust 1.92.0 (the repository CI version) was used, without changing the user's
default toolchain. Current logs and the runnable release are local files:

| Check | Result | Evidence under `target/qa/typed-final/` |
| --- | --- | --- |
| Formatting | Passed | `fmt.log` |
| Clippy, all targets, `-D warnings` | Passed | `clippy.log` |
| Workspace tests | 388 passed: GUI unit tests 352, CLI 4 + process contracts 5, core 27; 63 opt-in native tests ignored by this default command | `workspace-tests.log` |
| Release workspace build | Passed; runnable `target/release/termimochi` and CLI | `release-build.log`, `artifacts.sha256` |
| Disposable user installer/uninstaller | Passed against the new release; spaces, sentinels, backup, dry-run and idempotence covered | `installer.log` |
| Isolated native release smoke | Passed: 10 seconds, `G_DEBUG=fatal-criticals`, memory GSettings, private HOME/XDG/D-Bus, dedicated Xvfb; timeout 124 expected | `release-smoke.log`, `smoke.sh` |
| Desktop resource tests / metadata | 9/9 and exact 17 embedded resource paths/bytes passed | `desktop-unit.log`, `desktop-resources.log` |
| Cargo workspace packages + unpacked tests | All three packages verified; unpacked tests passed (GUI 352, CLI 4 + 5, core 27; GUI native 63 ignored by default) | `package.log`, `package-core-tests.log`, `termimochi-0.1.0-package-tests.log`, `termimochi-cli-0.1.0-package-tests.log` |
| RustSec | 149 locked dependencies, no reported vulnerabilities; unchanged lockfile matches the audited SHA below | Baseline audit JSON/log and final `artifacts.sha256` |
| Online CI / external-user study | Not run | No push or external tester |

The smoke's software-rendering/portal warnings are retained; surviving 10 seconds
is startup evidence, not proof of all visual behavior. The installer removed only
its own disposable test installation; no daily application/configuration was
installed, changed or removed.

Final GUI release SHA-256:
`34ef279f690f442bc0c706f54297e08a5d630fb1337a4bba2c3e3899e1a017de`.
Final source inventory (147 source/script/workflow files, including new untracked
implementation files), tracked diff and worktree status are retained as
`source-files.sha256`, `tracked-changes.patch` and `worktree-status.txt` in
`target/qa/typed-final/`. Native resource bytes are independently checked above.
All task-owned Xvfb displays/diagnostic processes were stopped after validation.

## Native evidence and limits

Final deduplicated native coverage: **126 test/scale slots = 122 passed,
4 environment-blocked, 0 other unresolved failures**. This is the latest executed
result per exact test name and scale **across pinned builds**, not one fresh
126-case run of a single final binary. The manifest preserves each case's actual
build hashes, source report, raw log and classification:
[final-coverage.json](../target/qa/typed-native/final-coverage.json).

The sequence is the original 124-case matrix → 82 migrated-window cases
(`a_ztbjba`, 68 passed/14 failures) → 22/22 affected-path retries (`qykxjcxi`) →
2/2 new Kitty GUI gates (`tokov9ad`) → 2/2 extra Palette mapped/bounds checks
(`tf0dyvf9`). Earlier failures are retained; the later runs replace those slots,
not inflate totals. The final real Kitty retest is included in `qykxjcxi`.

- `target/qa/termimochi-regression-1bwpe6qd/`: 10/10 focused cases at integration
  time (typed documents, Full, real controlled Kitty). The real Kitty test checks
  actual color settings, `LiberationMono` face and 15-point size, current Starship
  marker, Greeting once, interactive input, GIF frame differences and published
  reopening. The two GTK scales do **not** imply two Kitty DPI modes: Kitty is
  not GTK and does not consume `GDK_SCALE`.
- `target/qa/typed-gtk-final-hro0ovah/`: 8/8 native GTK cases (four tests × 1×/2×),
  including main Use → prepared Kitty review and stale-trial/security gates.
  The test deliberately does not treat fabricated checkbox clicks as visual
  confirmation and does not launch a successful terminal child.
- Broad original run `target/qa/termimochi-regression-r18kp3l2/`: 66/124 passed,
  58 failed. Four are separately confirmed Ptyxis environment blocks; many other
  failures exposed obsolete all-modules fixtures and old Save/Run labels. Raw
  failures remain on disk and are not relabeled as passes.
- Migrated fixtures explicitly own projects or standalone Greeting as appropriate.
  New typed tests still start as Palette-only. No assertions were removed to
  hide ownership, source preservation, cancel or conflict failures. A sensitivity
  regression discovered here was fixed in production: ownership must narrow,
  not overwrite, business validity/readiness; pending conversions and invalid
  owned drafts cannot be re-enabled by scope refresh.
- Intermediate focused failures were also retained: initial Kitty bootstrap did
  not initialize the first current prompt (fixed in production); the native font
  query returns the PostScript name `LiberationMono` rather than a spaced family
  label (assertion normalized, still checking the actual face/size). Compile-only
  failures during concurrent test integration ran no cases and are not passes.
- One obsolete legacy test invoked the new Save chooser and forcibly destroyed
  its parent, producing a native GTK crash. GDB evidence is retained under
  `target/qa/typed-documents-gdb-ZT9GTQa1/`. The legacy conflict test now uses its
  actual v1 store; typed GUI Save is tested separately. This does not claim to fix
  every upstream GTK forced-parent-destruction case.
- Full successful GUI **visual-confirm → publish → restart App → library launch**
  is not one automated end-to-end scenario. Its actual backend rendering/reopen
  path and GUI review/gates are separately tested. The combined human workflow
  remains on the acceptance card; it is not counted as externally validated.

### Screenshot evidence (native, not HTML)

Relative paths below point to retained local test evidence, not uploaded assets:

- [Palette-only at 1024×700](../target/qa/termimochi-regression-1bwpe6qd/1x-window-typed_tests-typed_palette_actions_and_save_exclude_reference_write_sets/cache/typed-palette-1024x700.png)
- [Kitty appearance at 1280×900](../target/qa/termimochi-regression-1bwpe6qd/1x-window-typed_tests-typed_plain_kitty_is_not_an_implicit_project/cache/typed-kitty-appearance-1280x900.png)
- [Full combined design](../target/qa/termimochi-regression-1bwpe6qd/1x-window-full_session-tests-full_session_live_composition_and_independent_navigation/cache/full-1280x900.png)
- [Real published Kitty session](../target/qa/termimochi-regression-1bwpe6qd/1x-kitty_session-tests-controlled_kitty_real_gif_prompt_and_reopen/cache/controlled-kitty-evidence/published-1.png)
- [Native stale-trial review gate](../target/qa/typed-gtk-final-hro0ovah/1x-window-typed_kitty-tests-typed_kitty_main_use_review_and_stale_trial_guards/cache/typed-kitty-review-stale-guard.png)

Static screenshots alone do not prove animation. Frame sequences, text/font queries
and the motion comparison assertions are retained with their native test logs.

### Environment-blocked / unimplemented / unverified

- **Environment-blocked:** native Ptyxis private-bus terminal opening. Independently
  reproduced in disposable state on a separate Xvfb with `/usr/bin/ptyxis
  --standalone`: “Failed to connect to user scope bus via local transport”. Evidence:
  `target/qa/typed-ptyxis-probe-JIWCarT4/probe.log` and `failed.png`. No real bus
  fallback was used. Recheck in an independent supported desktop/VM.
- **Unimplemented/limited:** non-Bash controlled sessions, optional desktop menu
  entries, arbitrary imported executable/include/watcher behavior in controlled
  sessions, unsupported Starship/layout equivalents. These are disclosed before
  trial/publication; originals are retained inert, not silently executed.
- **Unverified:** full human GUI journey above, external-user comprehension,
  additional OS/terminal versions, real desktop scaling/accessibility combinations,
  daily startup integration in the user's environment, and remote CI. No daily
  environment test was performed, by instruction.

## Scope and baseline

- Task: `TermiMochi_Typed_Documents_Codex_Task.md`, version 1.0.
- Baseline branch: `main`, commit
  `6e3e73da1751e9e1deeebc3cbd5dc8ca6d897214`.
- No applicable `AGENTS.md` found in repository or filesystem ancestors.
- Pre-existing/task worktree changes are preserved; no reset/clean/checkout,
  commit, push, merge, daily-environment installation or published release performed.
- Test files and unpacked tools are under ignored `target/qa/`. Local paths in
  this report are **not uploaded artifacts or remotely accessible links**.

## Toolchain and isolation

Current `.github/workflows/ci.yml` and `Cargo.toml` require Rust **1.92.0**.
The daily default toolchain is 1.98.0 and is not used as evidence of 1.92
compatibility. A separate rustup home was prepared under the repository:

```bash
RUSTUP_HOME="$PWD/target/qa/typed-toolchain/rustup" \
  rustup toolchain install 1.92.0 --profile minimal --component clippy,rustfmt
```

Confirmed tools: rustc 1.92.0 (`ded5c06cf`, 2025-12-08), Clippy 0.1.92,
rustfmt 1.8.0-stable. This changes only the project-local rustup home, not the
user's default toolchain. Use both environment variables on build/test commands:

```bash
export RUSTUP_HOME="$PWD/target/qa/typed-toolchain/rustup"
export RUSTUP_TOOLCHAIN=1.92.0
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
```

Native libraries on this machine: GTK 4.22.4, libadwaita 1.9.1, VTE 0.84.0;
Kitty 0.45.0, Fastfetch 2.57.1. This differs from CI's Ubuntu 24.04 environment;
local success does not imply online CI passed.

`scripts/test-regression.py` was read before execution. Each native case uses
private HOME, CONFIG/DATA/STATE/CACHE roots, a short private runtime directory,
private D-Bus, memory GSettings, dedicated X11 display, and a pinned test/worker
binary. It never uses the daily desktop display. Existing Ptyxis private-user-bus
failures must remain blocked, not be bypassed by connecting to the real bus.
Private state is **not a filesystem sandbox**: only fixture copies may be used.
The ordinary `scripts/run-isolated.sh` retains the compositor socket for a visible
window and is not a replacement for dedicated native regression isolation.

Unpacked Xvfb/Xterm from previous local testing remain in project-local paths;
neither was installed into the daily environment. Example native recipe, after
checking that display `:96` is free and building stable source:

```bash
target/qa/native-tools/termimochi-gif-qa.Zn3tVv/runtime/usr/bin/Xvfb \
  :96 -screen 0 2560x1800x24 -nolisten tcp
# In another shell, from the repository, with Rust variables above:
TMPDIR="$PWD/target/qa" \
TERMIMOCHI_XTERM="$PWD/target/qa/native-tools/termimochi-display-qa.qJeqLQ/xterm-runtime/usr/bin/xterm" \
LD_LIBRARY_PATH="$PWD/target/qa/native-tools/termimochi-display-qa.qJeqLQ/xterm-runtime/usr/lib/x86_64-linux-gnu" \
  python3 scripts/test-regression.py --display :96 --scale 1 --scale 2
```

Task-owned displays/processes must be stopped after tests; do not close or send
input to the user's terminals. Runtime directories must be short enough for the
108-byte Unix socket path limit.

## Preparatory checks (not final-code acceptance)

| Check | Observed result | Local evidence |
| --- | --- | --- |
| Rust 1.92.0, Clippy, rustfmt availability | Confirmed project-local | `target/qa/typed-toolchain/rustup/` |
| Desktop resource unit tests | 9 passed | `target/qa/typed-baseline/desktop-unit.log` |
| Desktop metadata, embedded-resource bytes, packaged source copies | Passed, 17 resources | `target/qa/typed-baseline/desktop-resources.log` |
| Disposable installer/uninstaller, dry-run, sentinels and idempotency | Passed against **baseline** release, not new implementation | `target/qa/typed-baseline/installer.log` |
| RustSec audit with cargo-audit 0.22.2 | No reported vulnerabilities in 149 dependencies; final lockfile identity must still match | `target/qa/typed-baseline/rustsec-audit.log`, `rustsec-audit.json` |
| Typed-document acceptance | Not inferred from baseline | Integrated evidence is recorded above |
| Online CI | Not run; no push requested | None |
| External user usability study | Not performed; no external testers | None |

The baseline installer script was inspected: explicit disposable HOME,
prefix/data/state roots, sentinel preservation, dry-run, installation,
uninstallation and idempotent second uninstall. It removes only its validated
temporary test root. Installer runs must use an already-built binary and be
rerun after the final release build; a baseline artifact is not final evidence.

Baseline release SHA-256: GUI
`1f65b7a665214c962a6837b898220fc379a82660609f83eadb13918b17bb9d58`, CLI
`dd77146ed69aac3668e65c169f114f39c181d93bd2fc46c622bd76392e8920c8`.
Baseline audited `Cargo.lock` SHA-256:
`08389efd9d11a7434e1c6486e7ea31f14d873853f436096dbb414d37465223f0`.
RustSec database fetched successfully, 1,243 advisories, revision
`b50980aad8b8f14f77e25a97b32dd94bf008b0af` (2026-09-09T12:49:52+02:00).
Audit binary and database both reside under `target/qa/typed-toolchain/`; no
global cargo-audit installation was performed.

## Packaging and audit reproduction commands

```bash
TMPDIR="$PWD/target/qa" bash scripts/test-install.sh
python3 -m unittest discover -s scripts -p test_desktop_resources.py -v
python3 scripts/validate_desktop_resources.py
cargo package --workspace --allow-dirty --locked
```

Run unpacked core tests, then CLI/GUI tests with the unpacked local core patch as
in CI. Package verification and unpacked tests are separate checks. Run the
available RustSec audit with CI's cargo-audit version 0.22.2 and record advisory
database freshness; unavailable audit is not a pass.

```bash
target/qa/typed-toolchain/audit/bin/cargo-audit audit --file Cargo.lock \
  --db "$PWD/target/qa/typed-toolchain/advisory-db"
```

## Technical reference checked

The official [Desktop Entry Exec specification](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html)
was checked for future optional launcher output. Desktop escaping has a string
layer as well as argument quoting: four backslashes represent one literal
backslash inside a quoted argument; dollar signs require the corresponding
two-layer escape, literal percent uses `%%`, and executable paths cannot contain
`=`. Shell quoting alone is not sufficient. This is specification evidence, not a
claim that arbitrary-path desktop launching has been tested. The optional menu
entry and GUI managed-entry opening must be assessed separately.

## Task behavior coverage

“Covered” below identifies executed automated evidence, not universal platform
support. Native exact run outcomes and source/build identities are recorded with
the local matrix; these rows do not add extra test counts.

| ID | Implemented behavior / evidence | Remaining qualification |
| --- | --- | --- |
| T01 | Type detector/import model tests, strict schema/duplicate rejection, native GTK independent opening | Ambiguous confirmation uses one GUI decision; unfamiliar-user understanding unverified |
| T02 | Typed Palette action/write-set sentinels; native memory-GSettings apply/restore | Real Ptyxis profile opening blocked in private session |
| T03 | Native Kitty source-only parser/reconciler + GTK test; absent fields stay absent | Unsupported directives retained, not executed |
| T04 | Independent Prompt/Greeting native Save/reopen/source sentinel test | Shared activation depends on existing user's shell, not tested daily |
| T05 | Old preset decoding/export + GUI target-specific font/layout review | Unsupported target metrics disclosed |
| T06 | Model/source roundtrip, native JSONC/Starship parser tests, image/SVG/GIF re-edit | Converted copies report losses; not every native directive editable |
| T07 | Typed reference exclusion and Inspect tests; Full scene/zoom tests | References are read-only projections, not independently saved components |
| T08 | GAction scope assertions, old advanced Apply checks, legacy conversion guard | Same handler validation still runs even if UI state is fabricated |
| T09 | Typed separate windows/load identities and histories; stale Kitty GUI trial test | Exhaustive compositor/window destruction interleavings not proven |
| T10 | Typed store Save tests and existing alias/conflict tests | Save As creates a fresh identity; no daily file overwrite tested |
| T11 | GUI native-file/terminal selection, inline trial access, Kitty review/requirement path | Missing dependencies remain explicit; never auto-installed |
| T12 | Frozen scheme/Kitty plan checks, pointer and manifest conflicts, GUI stale gates | Caches are not final authorization |
| T13 | Kitty-owned artifacts / no Ptyxis actions; model rejects Ptyxis project pixels | Standalone explicit native updates are a separate authorized use intent |
| T14 | Real Kitty color/font/current Prompt/Greeting-once/input/GIF assertions | Backend visual trial tested separately from GUI confirmation sequence |
| T15 | Persistent deployment discovery/current-version reopening; GUI library implemented | Restart-App → GUI library launch combined human journey still unverified |
| T16 | Two independent backend IDs/version paths; update and restore preserve sibling | Portable duplicate files with same identity address same design; Save As/conversion fork identity |
| T17 | Controlled Bash rc/env sentinel tests + real Greeting-once | Does not inherit personal rc/aliases/environment integrations |
| T18 | Safe Starship/Fastfetch projection, inert imports; exact-source Run Once consent/cancel test | Arbitrary imported execution is not covered by safe visual trial |
| T19 | Operation capability model tests + real-use review notes | No success claim from export/process startup alone |
| T20 | V1 migration/copy tests and GUI legacy action rejection | Conversion explicitly chooses components/target |
| T21 | Dependency/hash/tamper/partial-entry tests; broken library entries reported | Process startup is not visual success |
| T22 | Pointer restoration/deactivation; original/sibling assets and external-change sentinels | Referenced assets intentionally retained, not garbage-collected |
| T23 | Native Full/scene/typography/GIF/Inspect/scroll regressions | GTK composition remains a design simulation |
| T24 | Native 1024×700/1280×900, 1×/2× screenshots; pointer/keyboard/scroll tests | Additional desktop/accessibility/user combinations unverified |
| T25 | Bounded worker heartbeat/coalescing/undo/stale tests; new Kitty stale-result gates | Synthetic obsolete GTK chooser crash retained as evidence, not universal lifecycle guarantee |
| T26 | Feature retention ledger + capability table + native/regression/package evidence | All limited/unimplemented behaviors are listed above and in the capability table |
