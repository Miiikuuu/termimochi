# Applying a scheme

Save keeps your design. **Apply Scheme…** is a separate, explicit external-change
workflow available from every module's fixed output bar. Opening or saving a
workspace never grants permission to change terminal configuration.

## Review and confirm

1. Open or edit a workspace, then click **Apply Scheme…**.
2. Read the target and profile. The current direct settings target is **Ptyxis**,
   using the launching profile when valid, otherwise its configured profile.
   The name and UUID are shown. This does not identify another window's active
   tab and does not modify Kitty or another terminal's settings.
3. Select the parts you want. All checkboxes initially start off. Expand **Review
   file changes** to compare replacement text. Unavailable items explain why;
   unsupported settings remain saved in the workspace.
4. Click **Back Up & Apply Selected**. Closing or cancelling the review writes
   no application record and changes no external settings.

| Part | Destination and effect |
| --- | --- |
| Palette installation | The displayed Ptyxis `.palette` file. Profiles already using that filename see the replacement too. Installation alone does not select it. |
| Palette activation | Selects the installed palette for the displayed profile and applies the workspace's Light/Dark choice **Ptyxis-wide**. Does not change the default profile. Requires successful installation in this plan. |
| Typography | Family, size and weight apply to **all Ptyxis profiles/windows**. Line height and character width apply to the displayed profile. Missing fonts block this item. |
| Layout | Global cursor, blink, scrollbar and default grid settings. Grid affects new windows, not existing ones; remembered window sizing is disabled. Exact padding, tab bar and outer spacing are preview-only. |
| Bound Starship source | Writes the displayed existing TOML file, keeping a private backup beside it. Only shells already configured to use that file pick it up. |
| Designer or detached workspace prompt | **Exports only** to `starship.toml` inside the displayed application-record directory. Does not replace this machine's Starship file or activate a shell integration. |
| Fastfetch | Replaces the displayed configuration, retaining a private backup. Does not run it or add a startup hook. Dedicated Kitty/Sixel/GIF bundles still use Image Greeting export. |

Application uses a frozen review snapshot. Changes to the workspace or reviewed
external values require another review. File operations retain the existing
regular-file, alias, size, TOML/JSON validation and conflict protections; the
reviewed palette installer adds pre-write snapshot checks. This is a sequence of
independent module transactions, not an all-or-nothing system transaction.

## Read the result and verify

Each item reports **Applied**, **Exported only**, **Installed · not enabled by
this action**, **Already matches**, **Not selected**, **Unsupported / preview
only**, or **Failed**. Palette activation has its own result. A failed item does
not hide or revert successful items. Expand **Details & destination** for the
exact path, scope and error.

**Open Profile Tab** opens a new Ptyxis tab using the reviewed UUID and the
profile's normal shell. Existing shell startup commands may run, just as when
you open a tab yourself. Use a new window with that profile to verify default
rows/columns. Check font, spacing, colors and cursor there. For Fastfetch the
result page includes a quoted `fastfetch --config ...` command for the exact
written file. Running an imported configuration can execute its command/network
modules, so inspect it first.

An applied config is not proof that shell integration is enabled. TermiMochi
does not add Starship initialization or Fastfetch startup hooks in this workflow.
Exported prompts and preview-only layout options do not automatically appear in
a new terminal.

## Restore this application

Use **Restore This Application…** in the result page, or reopen it through
**⋮ → Last Application & Recovery…** after restarting TermiMochi. Restoration
requires a second confirmation and works in reverse dependency order.

- Only this run's recorded changes are eligible; unchanged and unselected items
  do not borrow another operation's backup.
- External edits block that item and are preserved. Other eligible items can
  still be restored; results say **Restored** or **Restore blocked**.
- Palette activation is restored before its file. If activation is blocked, or
  another profile still uses a newly installed palette, that file is retained.
- Unset settings are restored as unset, preserving system defaults/inheritance.
- Workspace edits and exported artifacts are kept. Restoration does not undo
  commands that a user subsequently ran in their terminal.

Per-run records live under `$XDG_STATE_HOME/termimochi/scheme-applies/` (normally
`~/.local/state/termimochi/scheme-applies/`). Module receipts are isolated there
so later standalone Apply operations do not overwrite this run's recovery data.
The report is journaled before each attempted module; interrupted operations
remain visible for recovery. Each record directory is private. Keep these local
records/backups private; unlike portable workspaces, they contain local paths
and may contain previous Starship configuration text.
