# One theme, one terminal workspace

Current correction of the v2 document-first UI. Rust/GTK4/libadwaita, existing
editors and adapters remain in place. The older typed-document reports describe
the historical baseline and the retained Advanced tools, not the normal workflow.

## State and output boundaries

| Layer | Implementation | What it means |
| --- | --- | --- |
| Theme | Existing `DesignDocument`, `Kind::Project`, schema v3 | One stable ID, name, required Kitty/Ptyxis target, owned components/assets and sparse appearance intent |
| Editing | Existing GTK editors | Target pages remain accessible even when no corresponding override exists |
| Intent | Immutable opening document + sparse field patches in its `theme` metadata | Font size is independent of family/weight/cell spacing; light/dark colors are separate; Prompt enablement and Greeting settings are retained |
| Reference | Per-window immutable `Workspace` reference and disposable projection | Values needed to render are not ownership. Kitty references are not a claim to have read daily `kitty.conf` |
| Save/history | Existing DocumentStore + immutable theme checkpoints | Save all theme content; undo can return to inheritance. View scenes/zoom and local approvals do not serialize |
| Execution | Existing Kitty plan or Ptyxis scheme plan | Frozen target, intent, dependencies, local verification and conflicts determine actual writes |

No second mutable theme/Full data source was added. Existing bounded image work,
epoch/frozen-content guards, Full/VTE rendering, source snapshots and recovery
remain. Page navigation does not switch Full/Terminal/Prompt/Greeting scenes.
Normal new/reopened themes default to Full; Advanced tools retain local scenes.

## Actual target capabilities

| Operation | Kitty theme | Ptyxis theme |
| --- | --- | --- |
| Colors | Safe native color directives plus explicit edits; unknown source retained inert | Whole palette install + profile activation; partial colors require an explicit complete base, not an implicit preview dump |
| Typography | Explicit family, size, cell adjustments; weight equivalence is not implemented (choose exact weighted family) | Composite font preserves actual target's unedited fields; font affects all windows, cell scales affect selected profile |
| Layout | Explicit mapped grid/cursor/padding/margins/tab bar; scrollbar preview-only; Kitty points differ from VTE pixels | Explicit supported cursor, blink, scrollbar, columns/rows; exact padding/tab bar/window spacing preview-only |
| Prompt | Current supported source/Designer in the same controlled Bash, independently enabled | Reviewed Starship update or export-only; no implicit shell integration |
| Greeting/GIF | Same controlled session; actual Kitty trial with separate animation confirmation; original image and recipe retained | Character/shared Fastfetch path; pixel incompatibility is reported with return-to-edit/convert-copy choices, never silently sent to Kitty |
| Try/Use/Open | Existing temporary trial → GUI confirmation → versioned independent entry → Open in Kitty → durable library | Existing scope review, backups/conflicts, per-item result and exact profile opening; no complete isolated appearance/shell trial |
| Inheritance removal | Fresh independent version omits removed theme overrides, original source retained; previous version recoverable | Removes design intent only. Automatic per-field withdrawal of a prior shared application is **not implemented**; use Last Application & Recovery, which checks its actual receipts |

Kitty trial/publication omit unsafe/executable imported directives and disclose
omissions; they do not source daily rc/profile files. Original source is not
execution permission. GTK Full/GIF playback is design simulation, not protocol
verification. A process launch is not a human visual approval.

## Migration and safety

Normal v2 Palette/KittyAppearance/target Project opens as a target theme with the
same identity. Targetless v1/v2/preset input asks for a target and preserves all
content. Migration is in memory; v1/v2 do not acquire an in-place Save binding.
The v3 format uses the existing design suffix and strict codec. The user can
choose a new file; original source text is retained in the theme.

In-place import is reviewed and preserves ID/name/target. Inputs belonging to
another terminal offer a separate theme. Explicit terminal conversion and Save
As create a new ID and do not reuse visual approval or the original Kitty entry.
Font/layout preset import owns the preset's explicit fields, not unrelated modules.

Shared Ptyxis removal cannot infer ownership from old journals (they predate theme
identity). It is deliberately not implemented as a blind reset. Existing explicit
recovery remains available; this limitation must not be described as complete
automatic field withdrawal.
