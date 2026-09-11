# Typed documents: feature retention and implementation ledger

Baseline: main `6e3e73d`; three untracked user briefs are preserved. No applicable
AGENTS.md was found in the repository or its ancestors. Implementation status is
separate from test status; exact outcomes and environment blocks are in
[the QA record](typed-documents-qa.md).

Order: typed ownership/storage and palette routing → Kitty isolated deployment →
remaining native types and compatibility routes → integration, native QA and handoff.
Existing Workspace remains an editor/preview bridge, not authority to write reference
components. Existing document storage, backups, converters and VTE stay in place.

| Old entry / implementation | Actual retained capability | New owner / entry | Retention / regression | Migration status |
| --- | --- | --- | --- | --- |
| Palette editor; core Ptyxis parser/export | Light/Dark, HSV/HEX/RGB, contrast and Codex diagnostics | Open Palette → Palette; Use Design → Ptyxis review; ⋮ native/theme exports | `typed_palette_actions_and_save_exclude_reference_write_sets`, palette core and shared-color native regressions | Implemented; Ptyxis desktop opening has an isolated environment block |
| Typography editor/preset/apply | Installed font discovery, glyphs, size/weight/cell metrics | Open/New Typography, or explicitly owned appearance; Use → target; ⋮ preset/apply/restore | Typed independent preset review; typography/apply/glyph native regressions; Kitty real font query | Implemented; mapper limitations disclosed |
| Layout editor/preset/apply | Padding, grid, cursor, blink, spacing | Open/New Layout; target-specific Use; ⋮ preset/apply/restore | Layout preset/source tests, native widget/apply regressions, Kitty mapper | Implemented; exact unsupported metrics remain preview-only |
| Starship draft/file/import, Prompt editor | Designer, source modules, safe scenarios, A/B, native export | Open/New Prompt or checked project component; Use → existing native file / Kitty | `typed_native_documents_save_reopen_without_cross_document_writes`, draft roundtrips, controlled current prompt | Implemented; arbitrary executable modules remain inert in controlled sessions |
| Greeting/Fastfetch document/editor | Fields, order, styles, native JSONC including unknown/repeated modules | Open/New Greeting or checked project component; Use → native target / Kitty | Typed source sentinels; Fastfetch/source/editor/recovery regressions | Implemented; no implicit shell activation |
| Greeting artwork editor and source recipes | PNG/JPG/WebP/SVG/GIF, TXT/ANSI, crop, background removal, conversion | Open Artwork; or Greeting → Import Artwork; existing artwork editor and ⋮ exports | Image/source recipe tests; native image/SVG/GIF re-edit regressions; minimal artwork wrapper tests | Implemented; artwork owns no system fields/commands |
| Pixel export/trial | Kitty static/animation, Sixel, character bundles | Greeting/Artwork display intent → Try Greeting; ⋮ image export | Existing pixel trial, animation and protocol native checks; actual controlled Kitty GIF | Implemented; not every target capability verified on every host |
| Full/VTE/Inspect/scene/scroll | Shared composed preview, animation, Fit/Zoom | Independent right-hand scenes; Inspect only owned editors | `full_session_live_composition_and_independent_navigation`, typed scope, Inspect/scroll native tests | Reused, not a second renderer/data source |
| Presentation work and render caches | Latest-task queues, stale result rejection | Same document-scoped editors; Kitty prep/trial/publication background jobs | Presentation worker/coalescing/revision tests; typed load invalidation and trial checks | Reused; no cross-document approval import |
| Documents, scheme_apply, recovery | Save/export, backups, conflicts, journal restore | Save Design; Export Native Copy; Use Design; recovery / Kitty library | Typed stores and source sentinels, existing conflict/recovery tests, independent entry versions | Implemented; Save As forks identity, native use stays explicit |
| Bash integration, Run Once, Try Greeting | Explicit external execution | Owned Greeting advanced actions; controlled Kitty Use | RC/env sentinels; real once-only Greeting, Run Once guards, source-consent checks | Retained; controlled Bash excludes personal rc and unsupported imported execution |
| Theme exports, CLI, installer/resources | Existing native exports and packaging | Owned Palette secondary exports; CLI unchanged | Core/CLI tests, desktop resources, isolated installer and package checks | Retained; final packaging evidence in QA record |
| Legacy Workspace v1 | Portable complete design with detached sources | Open Legacy → Save typed copy / Create Project / Convert Copy | Model migration/conversion tests; typed native save/reopen test | Implemented, no writes/approvals on migration; mixed-target direct Apply prohibited |

New independent Kitty appearance/project capabilities, including safe projection
omissions, durable GUI opening and version recovery, are specified in
[the capability table](typed-documents-capabilities.md). Tests are not a substitute
for external-user usability assessment; that remains explicitly unverified.
