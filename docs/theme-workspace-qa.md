# Theme workspace: verification record

Local, isolated evidence for the correction based on `main@5d7e083`; not online
CI, a merge, a release publication or an external-user usability result. The four
user task books are unchanged. There were no tracked user edits at start.

## Automated checks

CI's Rust **1.92.0**, using the existing project-local toolchain:

```sh
RUSTUP_HOME="$PWD/target/qa/typed-toolchain/rustup" RUSTUP_TOOLCHAIN=1.92.0 cargo test --workspace --locked
```

| Check | Result / local log |
| --- | --- |
| Format; Clippy with `-D warnings` | Passed; `target/qa/theme-workspace/fmt.log`, `clippy-final.log` |
| Workspace tests | 394 passed (GUI 358, CLI 4 + 5, core 27); 65 native tests ignored in this command; `tests-final.log` |
| Release build | Passed; `target/release/termimochi`, `termimochi-cli`; `release-final.log` |
| Isolated install/uninstall | Passed under temporary prefix/HOME, never daily installation; `installer.log` |
| Metadata/resources | Validator and Python regressions passed; desktop/AppStream, embedded bytes and packaged copies; `resources-final.log`, `resource-tests.log` |
| Crate packaging + unpacked tests | All three packages verified; unpacked core 27, CLI 9, GUI 358 passed; 65 native ignored. `packages-final.log`, `package-tests-final.log` (core in `package-tests.log`) |
| Release smoke | 10 seconds under dedicated X11, private HOME/XDG/D-Bus and memory GSettings, expected timeout 124; `smoke-final.log` |

The first unpacked CLI attempt correctly rejected a stale patched lockfile. The
retry follows CI's offline `cargo update --package termimochi-core` in the
**unpacked target/package directory only**, then tests with `--locked`. The
repository lockfile was not changed.

## Native evidence and run lineage

`scripts/test-regression.py` pins test/worker binaries by SHA-256 and allocates a
fresh HOME/XDG/runtime and private D-Bus per case. Dedicated Xvfb :96/:97, X11,
1×/2×; no real desktop bus fallback. Raw paths below preserve earlier failures,
not just their successful retry. They are local QA artifacts, not portable docs.

After verification, the 11 completed regression directories and two Ptyxis
diagnostic directories were archived from `/tmp` to
`target/qa/theme-workspace/tmp-archive-6du4lRAX/`, preserving each directory's
basename and complete contents (including pinned binaries). All copies were
compared before removing their `/tmp` originals. The original paths below and
inside JSON/log records describe the run-time locations; resolve them using the
same basename under this archive. See `target/qa/theme-workspace/tmp-archive.log`.
Active application runtime directories were not removed.

- Baseline ordinary tests: `baseline-tests.log`.
- Early matrix: `/tmp/termimochi-regression-vkzs6hqd/results.json`, **90/128**.
  Old startup/scene assumptions failed; Xterm was absent from the QA PATH;
  Ptyxis failed in the isolated environment. These are not reported as passes.
- Corrected full matrix: `/tmp/termimochi-regression-0rxc0mh0/results.json`;
  **124/130**: 4 isolated Ptyxis blocks, 2 Inspect fixture failures (the test
  requested a Terminal sample while the new normal startup showed Full).
- Final targeted matrix: `/tmp/termimochi-regression-8umau456/results.json`,
  **8/8**, including 1×/2× normal Kitty/Ptyxis themes, Full and Inspect's explicitly
  selected Terminal scene. It additionally covers native-fragment merging,
  Prompt enable/disable and the saved Prompt marker after reopening.
- After replacing only matching test-name/scale slots with the final targeted
  results: **126 passed, 4 environment blocked, 0 unresolved code-test failures,
  130 distinct slots**. This is a **multi-build coverage union**, not a claim that
  all 130 were run on one final binary. `combined-native-results.json` records each
  slot's build; `full-matrix-build.json` and `final-targeted-build.json` contain
  pinned test/worker hashes. Original failed runs remain available.
- New theme checks cover sparse font undo, page/scene independence, in-place
  Prompt/GIF, one Save/reopen, current Prompt in Full, real GUI trial gating,
  isolated publication/open/library/recovery and two-window target isolation.
  Automated clicks through visual checkboxes prove GUI plumbing, **not a human
  visual approval**. Closing/recreating the controller is not claimed to be an
  external user's complete App-restart exercise.
- Independent `controlled_kitty_real_gif_prompt_and_reopen` checks real Kitty
  colors, font, current Starship, interactive input, one Greeting and changing
  GIF frames for temporary and durable entries. Original startup sentinels do
  not run. Kitty backend evidence is distinct from GTK's design simulation.

Screenshots: `target/qa/theme-workspace/screenshots/{1x,2x}/` contains native
1024×700 / 1280×900 workspace and Kitty result captures. GTK inspector assertions
and scene checks accompany them. `final-targeted-build.json` identifies the
workspace capture build; `release-sha256.txt` identifies the runnable release.
`screenshots/real-kitty-1x/` preserves the actual Kitty backend's captured frames,
font/color/text probes and logs from the corrected full matrix (build identified
by `full-matrix-build.json`). Capture file names alone do not certify protocol
compatibility.

## Not passed / not implemented / not verified

- **Environment blocked:** two Ptyxis real-window cases at each scale. The private
  terminal window fails/disappears, preventing its Enter/visual checks. An
  independent direct `/usr/bin/ptyxis --standalone` + minimal Bash diagnostic
  (without TermiMochi) also produced a `(Failed)` window: `ptyxis-diagnostic.log`.
  This does not certify Ptyxis's behavior in a real desktop session and is not
  counted as passing. No host session bus or daily configuration was used.
- **Not implemented:** automatic per-field withdrawal of a previously applied
  shared Ptyxis theme override. Inheritance edits remove theme intent; existing
  Last Application & Recovery handles reviewed receipts and external conflicts.
- **Existing adapter limits:** no full isolated Ptyxis session trial, no Kitty
  scrollbar control, no exact arbitrary font-weight/spacing/right-prompt or
  arbitrary imported executable-module equivalence. UI/review disclose these.
- **Not externally verified:** real-user usability, Wayland, fractional/mixed
  scaling, other OS/terminal versions, accessibility combinations, and user's
  actual desktop/profile/rc integrations. The Chinese card is not evidence that
  those checks were performed.
- RustSec and remote CI were not rerun here; dependencies were not changed.

## Run it

From the repository: `./target/release/termimochi`, optionally followed by a theme
or native file. No installation is required. See the Chinese acceptance card for
the GUI-only Kitty workflow and the precise Ptyxis/shared-setting limits.
