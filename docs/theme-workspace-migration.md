# Theme workspace correction

Baseline: main `5d7e083`. Four user task briefs are preserved, no tracked user
changes at start; no applicable AGENTS.md. CI toolchain: Rust 1.92.0.

| Previous behavior | Theme behavior | Reused implementation | Required regression |
| --- | --- | --- | --- |
| Palette/Kitty appearance limits pages to owned components | Target theme opens all supported pages; unset fields inherit | DesignDocument Project, shared GTK editors | Browse/Inspect changes no intent; size-only edit/undo |
| Create Project to add font/Prompt/Greeting | Edit/import in the same named target theme | Existing preset/Starship/Greeting/image editors | Stable ID/target, source retention, undo, Save/reopen |
| Save current component | Save entire theme, sparse appearance and assets | DocumentStore snapshots/conflicts | v3 roundtrip, v1/v2 read-only migration |
| Separate Greeting target | Theme target controls trials/use/open | Kitty sessions; Ptyxis scheme executor | No cross-terminal write/launch |
| Per-type initial preview | New/reopened themes observe Full | Full/VTE/image caches/Inspect | Independent left navigation, fit and animation |
| Advanced standalone documents | Secondary explicitly named tool | Typed document handlers and native exporters | Old formats/export/CLI remain available |
| Complete typography/layout application | Only explicitly set fields reach target | Existing transaction/journal machinery | Exact Ptyxis/Kitty write sets, conflict/restore |

Implementation order: Kitty vertical flow, Ptyxis/import/migration, automated and
isolated native verification. This ledger is scope, not a claim of test success.

## Feature retention / new entry points

| Existing feature | New normal entry / retained advanced entry | Regression family |
| --- | --- | --- |
| Palette controls, professional picker, contrast/Codex diagnostics, Light/Dark | Palette page of target theme; existing format exports | color_picker, color_targets, core exports/lint |
| Installed fonts, font-face previews, Nerd coverage/alignment | Typography page; Import into Current Theme; Save Typography Preset | typography, typography_apply, native font selection |
| Grid, padding, cursor/blink, tab bar, scrollbar, margins | Layout page; preset import/export retained | layout_apply sparse test; documents; extreme-settings native |
| Designer segments/presets, editable Starship, diagnostics and source retention | Prompt page with theme enable switch; in-place import; Advanced single Prompt | prompt, starship_draft/scene, typed native document tests |
| Official Greeting presets, fields/order/styles, source formatting, commands retained inert | Greeting page; Fastfetch import/current-config tools | greeting_official, fields, greeting sync/source tests |
| PNG/JPG/WebP character conversion, background removal and full recipe re-edit | Greeting Import Artwork / Edit Artwork | greeting_image; image_import native restart/source deletion |
| SVG sandbox, limits, inherited styles, editable source | Existing artwork import and bounded worker | SVG worker/sandbox and native re-edit tests |
| GIF, original pixels, ANSI/TXT, Kitty/Sixel exports, static fallback | Greeting display settings and existing export-copy tools | animation, pixel_export, pixel_trial native |
| Real Greeting trial, capability evidence, conflict checks | Try Greeting locked to theme target; same-flow Use Theme routing | presentation_work, pixel trial/review tests |
| Full/Terminal/Prompt/Greeting, GIF playback, Fit/Zoom/columns | Independent right-side scene control, Full default for themes | full_session, preview_scene, theme workspace native |
| Inspect, pointer feedback, independent scrolling | Existing preview toolbar | point_to_edit, preview_scroll, stress tests |
| Undo/Redo and invalid-draft recovery | Theme snapshots across modules; component history retained in Advanced | theme sparse undo, existing draft/coalescing regressions |
| Save/reopen, private backups, conflicts, source copies | One v3 theme; v1/v2 in-memory migration; Advanced v2 tools | design_document, document_store, native theme roundtrip |
| Ptyxis palette install + enable, supported settings, journals | Use Theme exact profile/global review; Last Application & Recovery | scheme/activation, sparse typography/layout, native scheme GUI |
| Kitty trial + controlled Bash + Prompt/Greeting + durable entries | Use Theme → Try in Kitty → confirm → publish → Open in Kitty; library after restart | theme GUI vertical; controlled Kitty pixel/input/GIF backend test |
| Legacy single docs, mixed v1 projects, presets, CLI and packaging | Advanced new/open tools, target-selection migration, unchanged CLI | typed advanced, v1/v2 unit migration, CLI/package checks |

No image algorithm, safe worker, terminal protocol, CLI command, backup journal
or library format was removed. Historical native tests now explicitly request
the retained advanced startup where their precondition is a standalone preset;
normal-theme initialization is tested separately, not hidden behind test-only
default behavior. Actual runs and limitations are in the QA report.
