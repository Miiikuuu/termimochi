# Workbench consolidation

## Intent

Keep the white, quiet editor and persistent terminal canvas. Reduce competing
entry points, not capabilities. Module navigation, property editing and output
actions are separate responsibilities.

References:

- [VS Code sidebar guidelines](https://code.visualstudio.com/api/ux-guidelines/sidebars):
  related views together, descriptive names, no duplicate functionality or excess toolbars.
- [Penpot interface](https://help.penpot.app/user-guide/first-steps/the-interface/):
  stable canvas, selection-dependent properties and shared color resources.

## Structure

The header owns brand, opening files and undo/redo. The existing activity rail
owns module navigation. The left inspector scrolls independently; its compact
output bar stays visible below it. The right Design Preview does not move or reset
when navigating. Diagnostics remain local to the preview.

The right-hand Preview Scene selector owns observation: Terminal, Prompt or
Greeting. Left editor navigation (including Inspect) never selects a scene.
Greeting pixels and playback depend on the observed scene, not the active editor.
Editing a hidden scene does not bring it forward. Scene selection is session-only;
opening a workspace can still initialize its preview from the loaded design.

The output bar has four entry points:

- **Save / Ctrl+S**: always the complete scheme, regardless of the active module.
- **Try Greeting**: only current Greeting output and local target; GUI visual
  feedback. It does not trial the entire font/color/prompt scheme.
- **Apply Scheme…**: review all workspace destinations/scopes and select external
  changes. No checkbox is preselected; palette activation depends on installation.
  Existing review, backup and conflict checks remain mandatory.
- **More**: the former contextual Apply/Install/Export action, module-specific
  export/import/reload/restore, and persistent **Last Application & Recovery…**.

Remove duplicate output controls from module bodies and the global header.
Saving a workspace is not permission to modify terminal settings. Theme
installation must not be mislabeled as activating a theme. Prompt Designer
export must not be mislabeled as applying an active Starship file.

The scheme review names the launching/configured Ptyxis profile and UUID; it
does not claim to identify another application's active tab. Typeface is global,
cell spacing is profile-specific, and palette activation also changes global
Light/Dark appearance. Exact padding, tab bar and window spacing are preview-only.
File replacements expose before/after text. See [Applying a scheme](apply-scheme.md)
for outcome and recovery semantics.

Every module's Save menu and Ctrl+S save workspace snapshots. Ctrl+Shift+S saves
a workspace under another name; Ctrl+O opens a workspace. Starship writes are
explicit Apply / Export actions, never a side effect of the generic Save shortcut.

Greeting's normal editor owns Display, Columns and character style. Protocol and
fallback preferences are advanced options; preview zoom/playback are session-only.
The Greeting pixel canvas is a GTK composition, not a VTE graphics-protocol test.
Other modules retain their VTE testing scenes. The output bar separately reports
saved state, current-session visual approval and deployment comparison. Local target
selection is remembered, but shared-pixel replacement consent is never remembered
or embedded in a scheme. Image output defaults to an independent managed config.

## Shared color editing

Colors are a shared editing capability, not a copy of each module's settings.
Show the target before the editor: Terminal, Prompt, or Greeting. Terminal
foreground/background/cursor and ANSI slots remain directly editable. Module
color roles should expose their binding to these slots and their owning source.
Changing a role is different from changing a shared slot; make that distinction
visible. Preserve the original module's history and persistence behavior.

Imported advanced Starship styles and native Fastfetch fields must not silently
lose literal RGB colors, retained formats or unsupported settings. When a source
cannot be represented, reveal its existing editor rather than approximate it.
Greeting roles include individual names/contents, with repeated native modules
identified by their source indices. Only proven ANSI bindings enable the palette
picker. Your Starship lists reviewed style fields and opens the matching module
and field in its existing source editor. Artwork pixel adjustments stay in the
image editor unless explicitly requested.

## Verification

### Continuous Greeting edits and advisory checks

Display, width and character-style edits are drafts until conversion completes.
Inputs are debounced for 150 ms; each editor retains at most one running
conversion and one replaceable pending request. Decoding/conversion uses owned
data on a worker; GTK updates and history commits stay on the main thread.
Undo cancels a pending draft. Opening/replacing a design or destroying its window
invalidates late results; cancellation never starts a second concurrent decoder.
Concurrent field edits are retained by rebasing the latest conversion request.
Save, export, Try and Apply refuse pending drafts rather than use stale artwork.
The explicit character-fallback trial also waits for its conversion to complete.

The output bar uses a bounded background check and a two-second advisory cache
for target environment, deployed-file comparison and profile information. Target,
design and preview font/cell changes invalidate the matching snapshot; returning
to the window forces a refresh. Expired/missing results display a checking state.
Check completion updates status only, without changing the document revision or
restarting preview/discovery work. Explicit trial/review authorization still
performs a fresh check and does not trust a cached positive result. These fresh
safety-boundary checks remain synchronous; this is not a rewrite of every I/O path.

This follows GNOME's [main-context guidance](https://developer.gnome.org/documentation/tutorials/main-contexts.html):
keep blocking work out of recurring UI callbacks and return results to the UI
context. Tests cover a deliberately delayed worker plus a live GTK heartbeat,
coalescing/cancellation, save guards, undo/redo, stale-result rejection and an
external configuration change immediately after a positive cached check.
The injected delay verifies event-loop liveness, not a real-user latency benchmark.

### Workbench regression scope

Check all five modules, minimum window size and 1x/2x scaling. Every former
command remains reachable. Output actions route to the right module and cancel
without external writes. Navigation preserves preview/input and Inspect.
Color edits must have correct ownership, preview, undo/redo and saved round trips.
Wheel scrolling does not alter parameters.
