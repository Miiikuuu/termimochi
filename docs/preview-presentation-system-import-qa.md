# Preview presentation and saved-terminal import

Implementation date: 2026-09-12. Base HEAD: `076e2d0` on `main`.
Scope: `TermiMochi_Preview_Presentation_System_Import_Addendum.md`.
The working tree already contained the [local preview fixes](preview-local-fixes-qa.md);
those fixes and all user briefs were retained. No commit, push, installation into
the daily environment, terminal preference change or shell startup edit was made.

## Implementation and retained features

| Existing feature | Change / owner | Regression evidence |
| --- | --- | --- |
| One theme / one target / sparse fields | Unchanged `DesignDocument` and `ThemeIntent`; only bounded, optional import-report metadata added, defaulting empty for old files | Workspace tests; import Save/decode equality |
| Actual size / fixed grid / whole-window Fit | Existing `PreviewFrame` allocates a finite centered surface inside a canvas; Actual keeps font scale 1 and reflows the observation grid | `preview_geometry_same_theme`, four `preview_presentation_*` cases |
| Shadow and rounding | Existing non-Full CSS retained; Full gets an outer GSK shadow followed by a separate rounded content clip | Actual node bounds, GTK PNGs and JSON geometry |
| GTK input / finite commands / tab state | Same Session reducer, entry, IME and history. Initial short welcome is a projection of the initial block, not a new data source; clear removes it | Keyboard Actual/Fit, no-context, vertical workbench, long-history checks |
| Kitty / Ptyxis tops | Real GTK label replaces small painted title; one actual sample menu; target-specific fonts, tab shapes and thresholds; long labels ellipsize | Window-top tests, 1/2/6-tab canvas matrix, actual Kitty styles test |
| Full / local scenes / Inspect | Existing navigation remains independent of left editor selection; one GTK transform for paint, hit testing and input | Full composition and geometry pointer/selection/Inspect tests |
| Images / GIF / current Greeting / Prompt | Existing Presentation cache and anchored image projection reused; no command or protocol expansion | GIF anchors, Full composition, native Kitty regression |
| Read-only Ptyxis appearance | Existing reader refactored to accept explicit selected profile; automatic foreign-terminal fallback retained only for the old automatic path | Two memory-backend profiles with process `TERM_PROGRAM=Kitty` |
| Kitty native parser and sources | Bounded include expansion feeds the existing parser; original files retained inert. A later unreadable recognized value invalidates the earlier parsed value | Four `system_import` unit tests |
| New/Open/import | New Theme and footer menu offer **From My Terminal…**. Source review opens another theme window, never overwrites the current one | Actual GUI Create action, dirty old theme and Chinese input preserved |
| Save / native export / Apply / trial / library / restore | Existing adapters, source preservation, confirmation, backups and conflict checks retained; import creates no deployment authorization | Workspace tests; source-byte/GSettings equality checks; no Apply invoked by import |
| Native experimental preview | Existing feature and safety boundary retained, not replaced by samples | Enabled build plus actual Kitty/Ptyxis and Fcitx matrix |

## Geometry and target calibration

`PreviewFrame` owns a single child allocation transform: translate into the
canvas, then scale. Actual mode uses the smaller of suggested and available
surface size at scale 1; fixed/Fit modes retain their existing meaning. The
suggestion uses current theme columns and a 24–40-row observation-height bound
in Actual mode only. It never authors Layout rows/columns or Greeting Columns.
The canvas gutter is up to 36 logical pixels on each side. History size does not
participate in the window-height calculation; it scrolls inside the viewport.

The outer canvas still has `Overflow::Hidden`, but is no longer tight to the
window. Full's GSK shadow is appended outside the 10-pixel rounded surface clip;
the child is then drawn inside that clip. Real shadow-node bounds are recorded
and asserted inside the canvas. Existing `.terminal-shell` CSS was not deleted:
only Full's `sample-window` class disables the former shell shadow/focus glow
because the frame now owns that rendering. GTK overflow clips at widget bounds;
this is why margins and clip ownership, not a larger CSS shadow, matter.
[GTK Overflow](https://docs.gtk.org/gtk4/enum.Overflow.html).

Same theme (`Geometry fixture`, Ptyxis, background `#FAEDDD`, font 13 pt), same
window requests, actual measured GTK allocation, at device scale 1:

| Measurement | Previous presentation | New presentation |
| --- | --- | --- |
| Requested App 2048×1126 / actual allocation | 2038×1116 | 2038×1116 |
| Text cell / font / observation scale | 10×22 / 13 pt / 1 | 10×22 / 13 pt / 1 |
| Observation columns | 150 | 80 |
| Theme columns / Greeting columns | 80 / 32 | 80 / 32 |
| New surface / canvas | Not recorded by prior test | 836×590 / 1561×739 |
| New surface origin inside canvas | Not recorded by prior test | 362.5, 36 |
| Actual GSK shadow bounds inside canvas | Not recorded by prior test | 337.5, 17, 886, 640 |
| Requested App 1130×830 / actual allocation | 1120×820 | 1120×820 |
| New small-window surface / canvas | — | 571×371 / 643×443 |
| Small-window observation / theme columns | — | 53 / 80, still 13 pt at 100% |

Do not interpret the VTE widget's row count as viewport rows: the VTE holds the
transcript and may be taller than the visible scroller. JSON records both.

Before evidence is from the preserved pre-addendum working-tree build, not a
claim that it is pristine HEAD. Both original GTK images were opened and inspected:

- [Before: large Actual](../target/qa/preview-geometry-runs/termimochi-regression-6kqve8y_/1x-window-preview_geometry_tests-preview_geometry_same_theme/cache/after-actual-2048.png), [before geometry](../target/qa/preview-geometry-runs/termimochi-regression-6kqve8y_/1x-window-preview_geometry_tests-preview_geometry_same_theme/cache/after-actual-2048.json).
- [After: large Actual](../target/qa/preview-geometry-runs/termimochi-regression-iqllhkqh/1x-window-preview_geometry_tests-preview_geometry_same_theme/cache/after-actual-2048.png), [after geometry](../target/qa/preview-geometry-runs/termimochi-regression-iqllhkqh/1x-window-preview_geometry_tests-preview_geometry_same_theme/cache/after-actual-2048.json).
- [After: small Actual](../target/qa/preview-geometry-runs/termimochi-regression-iqllhkqh/1x-window-preview_geometry_tests-preview_geometry_same_theme/cache/after-actual-1130.png), [geometry/clip](../target/qa/preview-geometry-runs/termimochi-regression-iqllhkqh/1x-window-preview_geometry_tests-preview_geometry_same_theme/cache/after-actual-1130.json).
- [Dark Kitty, six long tabs, Fit](../target/qa/preview-geometry-runs/termimochi-regression-iqllhkqh/1x-window-preview_geometry_tests-preview_presentation_kitty_dark/cache/canvas-Kitty-1130-dark-6tabs-mode0.png).
- [Light Ptyxis, two tabs, Actual](../target/qa/preview-geometry-runs/termimochi-regression-iqllhkqh/1x-window-preview_geometry_tests-preview_presentation_ptyxis_light/cache/canvas-Ptyxis-2048-light-2tabs-mode4.png).

These are native GTK render captures, not synthesized mockups. Canvas cases
cover two targets × two colors × two App sizes × three tab counts × three
observation modes × two device scales (144 screenshot/geometry pairs).

Local references: Kitty 0.45.0, private native-preview Ptyxis 50.1, GTK 4.22.4,
libadwaita 1.9.1, VTE 0.84.0. Installed desktop is GNOME Shell 50.1, but automated
GUI runs use Xvfb/X11, software rendering and private D-Bus; native clients use
the existing nested Wayland/Casilda host. They are normal windows, not a daily
GNOME maximization test. Samples use terminal font 13 pt; native hot-update
screenshots are after changing to 17 pt. The initial same-theme projection is
recorded separately in `sample-comparison.json` and `sample-same-theme-before-native.png`.

- [Native Ptyxis with actual Fcitx candidates](../target/qa/native-preview/termimochi-regression-native-ts667_n4/1x-native_interactive_ptyxis_enabled/cache/native-app-candidates.png).
- [Native Kitty after font/color hot update, two tabs](../target/qa/native-preview/termimochi-regression-native-ts667_n4/1x-native_interactive_kitty_enabled/cache/native-app-after-hot.png).

Ptyxis samples use GTK UI typography, a centered title and single-tab autohide;
the existing preview tab-visibility setting still applies. Kitty uses terminal
font metrics, native style choices and minimum-tab threshold. Samples deliberately
omit host close/maximize/drag controls and retain safe sample controls rather than
pretending to expose every native menu. They are target-specific **design
approximations**, not pixel-identical native chrome. The reference HeaderBar
includes window handling, so it is not blindly embedded into the host.
[AdwHeaderBar](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/class.HeaderBar.html).

## Saved-settings import contract

Entry: **New Theme… → From My Terminal…**, or footer **⋮ → From My Terminal…**.
Choose target; multiple profiles/files require source selection, one candidate
goes directly to compact review. No Kitty file opens a file chooser; unavailable
Ptyxis profiles produce one explanatory error without borrowing the daily bus.
Create opens a detached theme, with Full selected and original editor unchanged.
One bounded background read per modal review, then one opt-in asset job, updates
widgets only on the main thread through weak window/controller references.

| Target/source | Read into the new theme | Not claimed / not performed |
| --- | --- | --- |
| Ptyxis selected saved profile | Readable palette; global or desktop font; profile cell spacing; configured grid and cursor; field-source metadata | Capturing another window's active tab; authoring preview-only padding/tab/scrollbar defaults; any settings write |
| Kitty saved files | Existing parser's supported appearance keys; ordered include/globinclude expansion; inert original source snapshots | CLI overrides, runtime remote-control state, environment interpolation, geninclude/envinclude execution |
| Optional Starship/Fastfetch | Explicitly checked candidates; existing parser; relative Fastfetch artwork snapshotted by the bounded asset pipeline | Default-path presence proving association; running startup scripts or imported command modules |
| Save | Complete new theme and source/asset snapshots | Applying back to the imported profile or path; importing deployment receipts or shell authorization |

Kitty limits: 32 files, include depth 8, 512 KiB aggregate/expanded text, existing
256 KiB single-file snapshot limit; glob traversal depth 8, 2048 entries and 32
matches. Glob matches sort before expansion. Directory symlinks are not traversed
and are reported; ordinary file symlinks resolve through existing snapshot
identity/conflict checks. Cycles, missing includes, unknown fields and unresolved
environment sources appear in the report. Hard limits reject the snapshot.
Known credential/history/startup paths are rejected, including symlink aliases;
this is a defensive path check, not a universal content-classification sandbox.
Original config files may contain user comments, which are retained as source.
Later invalid recognized settings no longer leave an earlier value reported as
the effective setting. Unspecified settings stay inherited.
[Kitty config/include semantics](https://sw.kovidgoyal.net/kitty/conf/).

Source-path metadata is for local traceability and confers no write authorization.
Default Starship/Fastfetch candidates are not automatically associated with a
managed launcher. Dynamic startup paths and arbitrary managed-entry discovery
remain unimplemented in this import flow; existing native/asset import remains
available. No “merge into current theme” option was added to this read-only flow.

Import GUI evidence:

- [Compact review, unchecked candidates](../target/qa/preview-geometry-runs/termimochi-regression-2poel_yr/1x-window-system_import-tests-system_import_profiles_and_detached_gui/cache/system-import-review.png).
- [New Kitty theme window](../target/qa/preview-geometry-runs/termimochi-regression-2poel_yr/1x-window-system_import-tests-system_import_profiles_and_detached_gui/cache/system-import-new-theme.png).
- [Saved source report](../target/qa/preview-geometry-runs/termimochi-regression-2poel_yr/1x-window-system_import-tests-system_import_profiles_and_detached_gui/cache/system-import-report.json).
- [Original theme, Kitty bytes and every fixture GSettings key unchanged](../target/qa/preview-geometry-runs/termimochi-regression-2poel_yr/1x-window-system_import-tests-system_import_profiles_and_detached_gui/cache/no-write.json).
- [Explicit optional import, asset snapshot and no-write assertions](../target/qa/preview-geometry-runs/termimochi-regression-2poel_yr/1x-window-system_import-tests-system_import_profiles_and_detached_gui/cache/optional-no-write.json).

The no-write proof compares private fixtures, not the user's real settings.
Tests did not grant themselves access to the daily D-Bus session.

## Executed checks

Locked Rust 1.92.0; `cargo fmt --all -- --check`, both default and
`--features native-preview` strict `cargo clippy --workspace --all-targets
--locked -- -D warnings`, workspace tests and workspace release builds.

| Check | Result / evidence |
| --- | --- |
| Default workspace tests | 417 passed (381 App + 4 CLI + 5 integration + 27 core), 84 GUI/opt-in tests ignored by this command |
| Native-feature workspace tests | 417 passed, 93 GUI/opt-in tests ignored by this command |
| Both strict Clippy / fmt / release; isolated installer and desktop resources | [Final build gates](../target/qa/native-preview/build-checks-ooasc4eq/results.json), separate default/enabled binaries and SHA256 beside logs |
| Related default GUI, 1×/2× | [30/30](../target/qa/preview-geometry-runs/termimochi-regression-iqllhkqh/results.json): canvas, same-theme geometry, window tops including real Kitty styles, Full, GIF, keyboard, no-context and detached import |
| Expanded import GUI including optional candidates, 1×/2× | [2/2](../target/qa/preview-geometry-runs/termimochi-regression-2poel_yr/results.json), after the final import-only changes |
| Actual enabled native interaction and real Fcitx Actual/Fit, 1×/2× | [8/8](../target/qa/native-preview/termimochi-regression-native-ts667_n4/results.json); actual preedit/commit/cancel, not paste-only IME evidence |
| Native safety policy | `python3 scripts/test-native-safety.py`: 6/6 |

Ignored tests are not counted as passing. The selected GUI matrix is not a rerun
of every historical ignored test. Native interaction ran before the final
import-only symlink-report/test addition; both feature builds were checked again
after it. Default/native tests are not distinct coverage counts to add together.

Failure history is retained, not converted to passing evidence: initial canvas
capture had an unallocated GTK paintable; a bounded real-frame wait fixed it.
The original combined 72-frame test was stopped and split by target/color to fit
the per-case timeout. An old top test expected two decorative tabs with one
Session tab; it now creates two actual Session tabs. Strict Clippy caught a
needless GSK-node borrow, fixed without suppressing warnings. Their earlier
logs (`yfdze5ib`, `o714smv3`, `m0ci_6p1`, `build-checks-856xrxmx`) remain under QA.
Final results above are separate reruns. Private Fcitx logs can contain SIGTERM
stacks during runner cleanup after successful tests; these are not App crashes.

## Remaining boundaries

- **Unverified:** daily GNOME Wayland, real maximize/snap decorations, fractional
  scale, cross-monitor movement, IBus, extreme minimum-width usability, long
  animation/idle performance and external-user usability. Screenshots and model
  visual review are not human acceptance.
- **Not implemented:** arbitrary running-window capture, environment/command
  evaluation, automatic managed-entry association, optional same-target merge.
- **Environment-blocked / known native failures retained:** stock Ptyxis on a
  private bus needs the existing flag-gated local patch; Kitty DMA-BUF failed
  previously in Xvfb/llvmpipe. The successful tests use the existing private
  prefix and SHM path, not a claim those failures disappeared.
- **Known native defect retained:** complex Greeting truncation/spacing and
  crowded native controls are not solved by these design-preview changes.
  Primary selection, runtime drift, IME edge cases and the other open entries in
  [native QA](native-preview-qa.md) still apply.

Local evidence lives in ignored `target/qa`; deleting it removes screenshots and
raw logs. It is not remote CI evidence. For launch and hands-on checks use the
[Chinese acceptance card](preview-presentation-acceptance-zh.md).

The final `target/release/termimochi` was rebuilt without `native-preview` and
matches the pinned default artifact, SHA256
`26cfaa8eb541a9287d167d45c050a235c3d66f2e878153e8f2dd66ae023236a9`.
`ldd` resolves its GTK/adwaita/VTE dependencies. An additional 8-second isolated
release launch on the owned Xvfb display reached the running App without an App
error, then was deliberately stopped by `timeout` (exit 124, not an interaction
test). Its state is `target/qa/session-O6T1UTHl`. Private portal/GVFS and DRI3
warnings reflect this Xvfb environment; daily portal dialogs were not certified.
