# Full Session QA — 2026-09-11

## Scope and build

Baseline main: `5a059342b8e35ba1b429ad074f599184c47b94b4`. No applicable AGENTS.md
was found. The full-session brief and previous implementation documents were read;
the two user-owned brief files remain untouched and untracked. No commit, push,
merge, publication or installation into the user's system was performed.

Production changes are confined to the new `window/full_session.rs` controller,
`window.rs` scene/feed/geometry/Inspect integration, `window/preview_scene.rs`,
`window/preview_scroll.rs`, the shared Greeting layout and prompt render paths,
`window/greeting/presentation.rs`'s texture observer, and the output bar's scene
selection condition. Other Greeting/scroll/hint test edits update explicit scene
indices. The chrome A/B test now explicitly observes Prompt instead of assuming a
left editor switch changes the scene. Documentation is in `docs/full-session.md`,
the Chinese acceptance card, README and the architecture index.

Local versions: Rust 1.98.0; GTK 4.22.4; libadwaita 1.9.1; VTE 0.84.0;
Fastfetch 2.57.1; Kitty 0.45.0. Xvfb and Xterm were restored from the prior local
test archive into ignored `target/qa/native-tools`, not installed system-wide.

Runnable GUI: `target/release/termimochi`.
SHA-256: `1f65b7a665214c962a6837b898220fc379a82660609f83eadb13918b17bb9d58`.
Run without daily application/configuration state:

```bash
cd /home/xiaozhouchao/Projects/personal/TermiMochi
bash scripts/run-isolated.sh
```

The launcher prints its retained disposable session path and reopen command. It
isolates HOME/XDG, D-Bus and GSettings, not arbitrary filesystem access; use copies.

## Executed gates

| Command/check | Final result | Local evidence under `target/qa/` |
| --- | --- | --- |
| `cargo fmt --all -- --check`; `git diff --check` | Passed | Executed on final tree |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed; no checks disabled | `full-session-clippy.log` |
| `cargo test --workspace --locked` | 355 passed; 58 opt-in tests ignored, not counted as passing | `full-session-workspace.log` |
| Repeat of the full workspace command | 355 passed; 58 ignored | `full-session-workspace-repeat.log` |
| `cargo build --workspace --release --locked` | Passed | `full-session-release.log` |
| Isolated release GUI, fatal GTK criticals, 10-second timeout | Remained alive; expected exit 124; no panic/GTK critical | `full-session-release-smoke.log` |
| `python3 -m unittest discover -s scripts -p 'test_desktop_resources.py' -v` | 9/9 passed | `full-session-resource-tests.log` |
| `python3 scripts/validate_desktop_resources.py` | Metadata, 17 embedded resource paths/bytes and packaged copies passed | `full-session-resources.log` |

The release smoke used `DISPLAY=:94 GDK_BACKEND=x11 GSK_RENDERER=cairo
GTK_A11Y=none G_DEBUG=fatal-criticals timeout --kill-after=5s 10s bash
scripts/run-isolated.sh`, never the real desktop. Native cases independently use
private HOME/XDG/runtime, private D-Bus, memory GSettings and dedicated Xvfb.
Apply/restore/startup tests operate only on disposable fixtures. Real configuration
and shell startup files were not modified. Full's new GUI test additionally checks
unchanged Fastfetch and `.bashrc` sentinels in its isolated HOME.
The task-owned Xvfb servers/probe were stopped after verification; evidence and
unpacked test tools remain under ignored `target/qa/` for inspection.

## Native coverage

The opt-in suite was selected with:

```bash
TMPDIR="$PWD/target/qa" python3 scripts/test-regression.py --display :94 --scale 1 --scale 2
```

The primary runner was interrupted after 93 recorded cases. Its unfinished cases
were explicitly selected and run in a second 2× runner. This is not claimed to be
one uninterrupted green run. Final deduplicated result: **112/116 passed, 4
Ptyxis-dependent cases environment-blocked (not passed)**. The continuation passed
24/24 cases. `target/qa/full-session-evidence/native-summary.json` maps every unique
test/scale pair to its raw result and evidence. Repeated successful tests are not
added to the total again.

Reports (each directory contains a build fingerprint and per-case raw logs):

- `termimochi-regression-_2x2bx9f/results.json`: primary, 93 completed records.
- `termimochi-regression-p5vjmaaq/results.json`: remaining 2× cases and retry of
  real Kitty animation feedback. Run with `--scale 2` and repeated `--filter` for
  the 23 unrecorded names plus `native_gui_animation_feedback_uses_the_frozen_scheme`.
- `termimochi-regression-bomk3rme/results.json`: **2/2**, real Kitty/Xterm visual
  output rerun with `--filter pixel_bundles_visual_terminal_rendering` and restored
  `TERMIMOCHI_XTERM`/`LD_LIBRARY_PATH` pointing inside `target/qa/native-tools`.
- `termimochi-regression-61i2lxe8/results.json`: **0/2**, combined real terminal trial
  rerun with the restored Xterm runtime. Kitty/Xterm subcases passed, then Ptyxis
  could not run its helper. The combined test remains **not passed**.
- `termimochi-regression-b6akw7vq/results.json`: **6/6 on final code**, strengthened
  Full Session, independent scenes and corrected chrome A/B cases at both scales.
  Selection: `--scale 1 --scale 2 --filter full_session_live --filter independent_preview_scenes
  --filter light_chrome_pages_and_native_controls`.

The new Full Session case verifies:

1. Full reaches one shared VTE/image canvas; Greeting fields, simulated command,
   output and following prompt are present for character, static PNG and GIF.
2. All five editors preserve Full; GIF frame textures advance while the saved
   workspace and terminal transcript remain unchanged by navigation/playback.
3. Palette/zoom/font metric changes retain the decoded-source generation; native
   transparent-area samples change exactly from RGB 22/28/36 to 248/249/250.
4. Typography changes actual VTE cell metrics and pixel cell occupancy together;
   padding and Prompt one/two-line changes update the composition.
5. Columns changes image occupancy; all four zoom options leave occupancy, terminal
   columns and the saved workspace untouched. Fit checks both dimensions at
   1024×700 and 1280×900, with no split-off VTE scrollback.
6. System reduced motion stops playback. Static pictures do not offer GIF play.
7. Scene switching/Save/reopen preserve design but do not restore Full selection or
   protocol verification. No protocol confirmation is granted by the GTK picture.
8. Left/top/right pixel placement, transformed artwork Inspect targets and shared
   image/text scrolling work. Invalid pixel Columns gives an explicit character
   fallback with fields/transcript; disabling Greeting keeps Full's command scene.
9. Full ignores Prompt's hidden Original comparison and uses the current detached
   Starship output before/after its simulated command, not the frozen A/B starting
   prompt. These paths share the Greeting prompt resolver and current cached data.

Two new pure tests check Unicode/ANSI transcript measurement and shared cell
reservation layout, including narrow stacking and unchanged saved Greeting state.
Existing tests cover all three prior scenes, font selection reaching VTE, actual
pointer navigation, extreme edits, background queues, apply conflicts/recovery,
workspace source retention, ANSI/image/GIF export and real terminal protocols.

## Screenshot evidence

Unmodified native X11 captures, not mockups, in:

`target/qa/termimochi-regression-b6akw7vq/`

Under each of `1x-window-full_session-tests-full_session_live_composition_and_independent_navigation/cache/`
and `2x-window-full_session-tests-full_session_live_composition_and_independent_navigation/cache/`:

- `full-1024x700.png`, `full-1280x900.png`: complete scene after Palette/Typography/Prompt edits.
- `full-character.png`, `full-static.png`: character and image compositions.
- `full-gif-before.png`, `full-gif-playing.png`: native GIF views; automated texture
  progression assertions, not the still screenshots alone, establish playback.

Convenient copies are in `target/qa/full-session-evidence/`, including `1x`/`2x`
size-labelled images and the Ptyxis blocker screenshot.

Minimum/common-size captures at 1× and the 2× common-size capture were visually
inspected. The deliberately simple moving color-bar GIF is a deterministic test
fixture, not a new TermiMochi brand preset. Small-window Fit prioritizes the whole
composition; use 100% with scrolling or widen the divider for readable detail.

## Failures retained and explicit limits

- A first Fit implementation allowed one row of VTE scrollback when the Prompt
  changed height. Fixed by collecting/sizing before feeding the same VTE; the final
  Full test asserts no separate scrollback. Failed intermediate runs remain in QA.
- The old chrome A/B test relied on editor navigation selecting Prompt. Corrected
  its setup, not application behavior; both-scale reruns pass.
- Final current-state audit fixed hidden Original/frozen starting-prompt state
  leaking from the dedicated Prompt scene into Full. The final six-case native
  rerun covers this change and retains original-scene A/B behavior.
- The initial visual-output and combined-protocol attempts lacked Xterm after
  temporary-tool cleanup. Restored only the unpacked test runtime and reran.
- **Ptyxis isolation remains blocked:** a fresh standalone probe using only
  `/usr/bin/sleep` displayed “Failed to connect to user scope bus via local
  transport”. Evidence: `full-session-ptyxis-rK4eSD4y/failed.png` and `output.log`.
  This occurs before running Fastfetch. The one-shot Ptyxis test and combined
  terminal trial cannot pass in this private user-bus environment. No real user
  bus or daily configuration was used to bypass isolation.
- One intermediate workspace run failed in the existing Starship sandbox renderer
  (`full-session-workspace-intermediate-failure.log`). Two subsequent complete
  runs passed without changing/disabling the sandbox. Its transient root cause is
  not established; sustained-load stability is not certified.
- One 2× real Kitty GUI-feedback attempt sampled no moving frames. The separately
  rerun case passed without changing the protocol pipeline; the original failure
  remains in the primary report. Full's GTK animation evidence is separate.
- An attempted release smoke on a stopped test display is retained as
  `full-session-smoke-missing-display.log`; final smoke used the live dedicated
  display and passed its survival/error checks.
- **Not verified:** real-user usability/discoverability (no external testers),
  Wayland/fractional/mixed-display scaling, accessibility assistive-technology use,
  other GTK/terminal versions, remote/multiplexer sessions and daily desktop
  integration. Native terminal cases use fixed test font settings; GTK 2× does not
  certify every external-terminal DPI combination.
- No new online CI/RustSec run, Rust 1.92 toolchain run, system installer run or
  publishing workflow was performed. No dependencies were changed.

## Phase 2

No **Try Full Scheme** action is added. The exact adapter gaps are documented in
[Full Session architecture](full-session.md#phase-2-intentionally-not-implemented).
Full remains the complete design simulation; Try Greeting remains the real,
temporary Greeting-only verification. Save/Apply Scheme safety semantics remain
separate. See the [Chinese acceptance card](full-session-acceptance-zh.md).
