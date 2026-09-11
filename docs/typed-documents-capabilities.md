# Implemented document / target capabilities

Historical v2 / Advanced-tool table. For the current default workflow use the
[theme workspace capability table](theme-workspace-architecture.md).

Availability is per operation, not one compatibility flag. The GUI's Document
Capabilities page distinguishes Available, Needs requirement, and Unsupported.
Local dependencies, visual evidence, selected target and write review are checked
again at use time; this table does not certify every machine/terminal version.

| Document | Owned editing / Save | Native output | Real use and reopen |
| --- | --- | --- | --- |
| Ptyxis Palette | Color fields and selected variant only | Palette and existing theme exporters | Scoped install + enable; exact profile open; existing recovery. Native Ptyxis desktop remains environment-dependent. |
| Native Kitty appearance | Source-preserving appearance properties; absent fields stay inherited | Reconciled `kitty.conf`; unknown/includes retained | Independent controlled Kitty entry; supported safe fields only, no implicit Prompt/Greeting. Component conversion required before exporting borrowed defaults as a different preset. |
| Starship Prompt | Current Designer or imported editable source | Current `starship.toml` | Choose an existing native file for reviewed update, or independent Kitty prompt session. Shared update does not install shell integration. |
| Fastfetch Greeting | Fields/order/styles/display/source and artwork | Current JSONC; existing TXT/ANSI/pixel bundles | Native selected-file update with same-flow trial, or independent Kitty entry. Full imported execution requires separate exact-source Run Once consent. |
| Typography | Font preset only | Existing font-preset format | Ptyxis's supported/global scope, or independent Kitty font-only session. |
| Layout | Layout preset only | Existing layout-preset format | Ptyxis supported subset or Kitty supported subset; remaining metrics preview-only. |
| Artwork | Source image/text, processing recipe, display intent | Existing dedicated artwork/pixel exporters | Independent Kitty display wrapper with no reference system fields; not a complete terminal project. |
| Explicit project | Only checked components | Component conversion/export, not a fictitious single native file | Kitty passes owned color/font/layout/current supported Prompt/Greeting to ONE Kitty; durable entry/library/recovery. Ptyxis uses only its actual supported effects and reviewed shared components. |
| Legacy Workspace v1 | Lossless import and typed saved copy | Explicit converted components | No direct mixed-target application; choose a target and components in Create Project / Convert Copy. |

## Deliberate unsupported / limited behavior

- Kitty controlled sessions support Bash only. They do not inherit the user's rc,
  aliases, Conda/ROS, login environment or old Greeting hook.
- Arbitrary custom commands, network/executable modules, includes/watchers and
  unsupported Starship modules are retained in inert original sources, **not run by
  the controlled session**. Dynamic omission notes appear before trial/publication.
  This is not a promise of lossless execution of every imported native file.
- Kitty uses the actual family/size and cell adjustments supported by its mapper.
  Default weight equivalence requires an exact font face/family; some VTE metrics,
  exact spacing, scrollbar behavior and right-prompt semantics are not equivalent.
- No automatic default-terminal change or daily startup-file change occurs in Use.
  The old explicit Bash startup tool remains under an owned Greeting's advanced
  actions. Native updates alone do not mean automatic activation.
- No optional `.desktop` entry generator, other-shell controlled session adapter,
  or automatic cleanup of referenced version assets is added.
- Sixel and existing theme formats retain their previous explicit export/trial
  capabilities; they are not falsely promoted into full independent project targets.
- A native export is a copy, not execution. Sharing preserved source may expose
  paths/private text present in that source; it is not advertised as fully sanitized.
- GTK Full is a design simulation. Real Kitty checks and user GIF confirmation are
  separate evidence; no Ptyxis or Sixel support is inferred from a successful export.

See the QA report for implemented/tested/blocked/not-run distinctions. An entry
library launch reports process startup, not visual success. Damaged dependencies,
conflicts or artifacts require re-preparation rather than silent overwrite.
