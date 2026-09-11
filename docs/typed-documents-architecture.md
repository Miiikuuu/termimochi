# Current document and target boundaries

Historical v2 baseline / retained Advanced tools. Normal operation now follows
[one theme, one target workspace](theme-workspace-architecture.md).

This describes implemented native Rust/GTK code, not a web prototype.

## Data and editing

`design_document::DesignDocument` implements the existing `document_store::Document`
contract with schema `termimochi-design`, version 2, suffix
`.termimochi-design.json`. Components are optional owned values. A palette document
does not serialize fonts, layout, prompt or Greeting just because they appear in
Full. Project membership is explicit. Native Kitty documents are source-only:
absent properties remain absent, while an immutable opening projection lets the
shared editor distinguish a changed/additional property from a reference default.

`window/typed_documents.rs` holds session metadata, portable identity, immutable
saved baseline, immutable native reference baseline, local source path and checked
store. The pre-existing editors remain the **only mutable settings**. A Workspace
snapshot is a temporary preview/output bridge, never permission to write every
component. Native source is provenance, not execution authorization. Current
Starship drafts/Fastfetch edits use their existing source-preserving pipelines.

Open/Ctrl+O creates another native window/controller. Histories, render queues and
bindings stay window-local. Loading increments identity, invalidates preview
generations, cancels artwork edits and discards old target/visual approval. Save
compares owned content with the immutable baseline, not just a revision counter.
Native Kitty snapshots preserve identity while reconciling only explicit changes.
Save As forks a new portable identity after the checked save succeeds and discards
local approval/binding state, so the copy cannot update the original Kitty entry.

`Scope::allows/require`, `owns_module`, and `require_document_action` project editor
navigation and gate GActions plus compatibility export/apply endpoints. Inspect
rejects unowned references, including Greeting fields in an artwork-only document.
Native Kitty component exporters require an explicit converted component copy;
otherwise they would silently export inherited defaults. Its native exporter uses
the reconciled original source.

## Save, conversion and output

Save/Ctrl+S goes through `committed_design` → typed encode → existing checked store.
It never applies. Native imports start as drafts. Content/schema detection is
authoritative; an ambiguous valid candidate asks once. V1 Workspace and existing
font/layout/Greeting presets import without writes. A legacy mixed workspace cannot
directly apply as a falsely unified target: Create Project / Convert Copy shows
actual retained/added/omitted component notes and assigns a new identity.

The fixed Use action dispatches by document and target. `document_use.rs` selects
an explicit native file or independent Kitty entry; font/layout can choose Ptyxis.
Shared file selection captures existing file snapshots. Greeting trial is reachable
inside the same flow, followed by revalidation and review. Selection and visual
approval remain local; changing document/settings/target invalidates a frozen plan.
The same identity/content checks run before a Kitty trial and again when its
background preparation returns. An old document cannot launch a stale trial.

The Ptyxis plan is the existing `scheme_apply` executor with a restricted write set.
Palette documents use a unique managed palette filename, and ordinary review
selects installation plus activation (advanced install-only remains possible).
Global light/dark and font effects are still disclosed. Native updates, receipts,
backup and reverse-order restoration retain existing conflict checks. Full native
execution is a separate explicit source review; target-aware Run Once opens a new
terminal with no inherited shell startup, never types into an existing terminal.

## Independent Kitty sessions

`kitty_document.rs` handles bounded source retention/projection. `kitty_session.rs`
is the target adapter. It generates only owned appearance fields and supported
current Starship/Fastfetch content, retains original executable/unknown configuration
as inert source files, and lists projection omissions. Trial and publication use
the same safe projection; arbitrary imported execution is not silently enabled.

Preparation/conversion and version publication run off GTK in bounded jobs. The GUI holds a frozen plan, requires actual
visual confirmation and a separate GIF-motion confirmation where applicable,
then publishes a complete version before switching its checked current pointer.
Dependencies and artifact hashes are checked before opening. Two design IDs have
separate version directories. The independent-entry library persists across App
restarts and reports damaged entries without hiding healthy ones. Restore changes
only the checked pointer; first-version restore deactivates it. Version assets are
retained, including while referenced by trials or open sessions.
Review identifies the exact managed destination and any entry/version it replaces;
both current pointer and previous manifest participate in publication conflicts.
Ptyxis projects cannot mix a Kitty/Sixel image target into their write set: the
same flow offers an explicit project copy or a return to choose Character output.
Previous Fastfetch recovery receipts are never borrowed as a new document's target.

Kitty runs with `--config NONE` and explicit safe overrides. Bash uses
`--noprofile --norc -i`; a locally generated one-shot bootstrap runs Greeting once,
initializes the current supported Starship prompt, then leaves an interactive
shell. It does not rely on `--hold`. The child environment excludes `BASH_ENV`,
`ENV`, exported functions and inherited prompt hooks. Personal aliases, Conda/ROS
and other rc integrations are **not inherited**. This is not a filesystem sandbox:
commands the user subsequently types execute normally.

Managed entries live at `$XDG_DATA_HOME/termimochi/kitty-sessions/<design-id>/`.
This task's tests use only isolated project-local HOME/XDG trees, not daily paths.
No desktop menu entry is created; the GUI library is the persistent launch route.

## Reused preview and safety

Full/Terminal/Prompt/Greeting still consume the same VTE/render state and image/GIF
cache. Left editor selection remains independent of the observer scene. Columns
means terminal occupancy; Fit/Zoom is view-only. GTK imagery never grants protocol
approval. Existing bounded artwork queues, source recipes, background removal,
converters, Starship safe projection, journals, backups and CLI remain in place.

Technical references checked for the adapter:
[Kitty CLI](https://sw.kovidgoyal.net/kitty/invocation/),
[Bash startup rules](https://www.gnu.org/software/bash/manual/html_node/Bash-Startup-Files.html),
[Desktop Exec specification](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html).
Desktop Exec is not reused as shell quoting; optional desktop entries are not
implemented in this iteration.
