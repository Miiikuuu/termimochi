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
output bar stays visible below it. The right Live Preview does not move or reset
when navigating. Diagnostics remain local to the preview.

The output bar has three entry points:

- **Save**: current document/preset and complete workspace options, explicitly named.
- **Apply / Install / Export**: contextual primary action, accurately named for
  the destination. Existing review, backup and conflict checks remain mandatory.
- **More**: secondary export/import/reload/restore operations for this module.

Remove duplicate output controls from module bodies and the global header.
Saving a workspace is not permission to modify terminal settings. Theme
installation must not be mislabeled as activating a theme. Prompt Designer
export must not be mislabeled as applying an active Starship file.

Existing keyboard behavior is preserved: Prompt's Ctrl+S reviews Starship
changes or exports Designer; its Save menu offers workspace snapshots. The
header's Open remains context-sensitive for presets, and opens a palette in
Prompt; Starship reload and Save As remain explicit secondary actions.

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
Artwork pixel adjustments stay in the image editor unless explicitly requested.

## Verification

Check all five modules, minimum window size and 1x/2x scaling. Every former
command remains reachable. Output actions route to the right module and cancel
without external writes. Navigation preserves preview/input and Inspect.
Color edits must have correct ownership, preview, undo/redo and saved round trips.
Wheel scrolling does not alter parameters.
