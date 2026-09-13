# Using a theme

One theme has one target and one workspace. Edit its Palette, Typography, Layout,
Prompt and Greeting together. **Save / Ctrl+S** keeps the complete design and its
assets; **Use Theme…** reviews external changes separately. You do not need to
assemble a Project to add a font or GIF. Unspecified settings stay inherited;
preview references are not part of the application write set.

## Kitty: try, apply and open

1. In the Kitty theme, click **Use Theme…**, then **Try in Kitty**. This opens a
   real temporary Kitty session with the supported owned settings, Prompt and
   Greeting, rather than changing your daily `kitty.conf`.
2. Inspect the actual window and confirm the included settings. If the theme
   includes animation, the confirmation also requires checking GIF motion.
   GTK design playback is not evidence of native support. Changes that invalidate
   the trial require another trial before application.
3. Click **Apply & Open in Kitty** to publish a versioned independent theme and
   open it. An opening failure is reported separately from publication; the
   result retains an **Open in Kitty** retry.
4. After restarting the app, use **⋮ → Open Independent Kitty Scheme…** to open
   the published theme again. No manual Fastfetch command is needed.

The ordinary Kitty icon and default terminal remain unchanged. For a separate
application-menu entry, expand **Advanced · app launcher & recovery** in the
result and choose **Add / Update App Launcher…**. This has its own environment
selection, real trial and confirmation. Updating a theme does not silently update
an existing desktop launcher. See the [daily launcher guide](kitty-daily-launcher.md).

Kitty uses a controlled Bash session and a safe projection of the theme, not
arbitrary imported Kitty commands. Unsupported directives remain preserved in
the design but are reported as inactive. No Ptyxis settings are written. Recovery
is available in the result's Advanced section and the independent-entry library;
a first publication can be deactivated, while an update can restore its prior
version. Desktop-launcher recovery is a separate action.

## Ptyxis: review the affected profile and shared settings

1. Click **Use Theme…** and read the actual profile name/UUID and scope. This is
   the reviewed destination, not a guess at another window's active tab.
2. Supported settings explicitly owned by the theme are preselected. The main
   summary shows selected changes and their impact; explicitly requested but
   unavailable settings stay prominent. Unowned modules do not clutter it with
   unrelated “Not applied” messages.
3. Expand **Advanced · components & files** to change selections or inspect
   exact destinations, before/after text and additional limits. Unsupported
   settings remain saved in the theme, but are not silently written as defaults.
4. **Apply & Open Profile** opens the reviewed profile after successful appearance
   application. With only file/export changes selected, the action is **Apply
   Changes**. Closing or cancelling review does not apply settings.

| Part | Destination and effect |
| --- | --- |
| Palette installation | The displayed Ptyxis `.palette` file. Profiles already using that filename see the replacement too. Installation alone does not select it. |
| Palette activation | Selects the installed palette for the displayed profile and applies the theme's Light/Dark choice **Ptyxis-wide**. Does not change the default profile. Requires successful installation in this plan. |
| Typography | Family, size and weight apply to **all Ptyxis profiles/windows**. Line height and character width apply to the displayed profile. Missing fonts block this item. |
| Layout | Owned global cursor, blink, scrollbar and default grid settings. Grid affects new windows, not existing ones; explicit grid application disables remembered sizing. Requested unsupported fields such as exact padding are called out, not applied. |
| Bound Starship source | Writes the displayed existing TOML file, keeping a private backup beside it. Only shells already configured to use that file pick it up. |
| Designer or detached theme prompt | **Exports only** to `starship.toml` inside the displayed application-record directory. Does not replace this machine's Starship file or activate a shell integration. |
| Character Greeting | Replaces the displayed shared Fastfetch configuration, retaining a private backup. Every terminal reading it is affected. Does not run it or add a startup hook. |

Ptyxis does not gain Kitty image/GIF protocol support from GTK playback. Use the
explicit **Display → Character** alternative for character output; the app does
not silently turn an image request into a successful native-image application.
**Try Greeting** tests only Greeting, not a complete Ptyxis theme.

Application uses a frozen review snapshot. Changes to the theme or reviewed
external values require another review. File operations retain the existing
regular-file, alias, size, TOML/JSON validation and conflict protections; the
reviewed palette installer adds pre-write snapshot checks. This is a sequence of
independent module transactions, not an all-or-nothing system transaction.

## Read the Ptyxis result and verify

Each item reports **Applied**, **Exported only**, **Installed · not enabled by
this action**, **Already matches**, **Not selected**, **Unsupported / preview
only**, or **Failed**. Palette activation has its own result. A failed item does
not hide or revert successful items. Expand **Details & destination** for the
exact path, scope and error.

**Open Profile Tab** opens a new Ptyxis tab using the reviewed UUID and the
profile's normal shell. Existing shell startup commands may run, just as when
you open a tab yourself. Use a new window with that profile to verify default
rows/columns. Check font, spacing, colors and cursor there. If opening fails, retry
from the result; a successful configuration write is not rolled back merely
because launching failed. A missing reviewed profile is not replaced by another
profile. This opens a normal shell, not an isolated full-theme trial.

To run the exact applied Greeting, expand **Advanced · run exact Greeting
configuration → Review & Run Applied Greeting…**, review the destination and
execution effects, then choose **Run This Configuration Once**. The run retains
the reviewed terminal; it does not guess from the app's current theme or claim
to reproduce the complete theme's appearance.
Imported command/network modules and `preRun` can execute, so this is a separate
confirmation, not a default post-apply action or a command you must copy manually.

An applied config is not proof that shell integration is enabled. TermiMochi
does not add Starship initialization or Fastfetch startup hooks in this workflow.
Exported prompts and preview-only layout options do not automatically appear in
a new terminal.

For continuity, a successful Greeting item also retains the existing local recovery
preset. Failure to save that preset is reported separately and leaves the draft
intact. This does not mark the complete theme saved; use main Save to keep all
modules together.

## Restore a Ptyxis application

Use **Restore This Application…** in the result page, or reopen it through
**⋮ → Last Application & Recovery…** after restarting TermiMochi. Restoration
requires **Restore Changes** confirmation and works in reverse dependency order.

- Only this run's recorded changes are eligible; unchanged and unselected items
  do not borrow another operation's backup.
- External edits block that item and are preserved. Other eligible items can
  still be restored; results say **Restored** or **Restore blocked**.
- Palette activation is restored before its file. If activation is blocked, or
  another profile still uses a newly installed palette, that file is retained.
- Unset settings are restored as unset, preserving system defaults/inheritance.
- Theme edits and exported artifacts are kept. Restoration does not undo
  commands that a user subsequently ran in their terminal.

Per-run records live under `$XDG_STATE_HOME/termimochi/scheme-applies/` (normally
`~/.local/state/termimochi/scheme-applies/`). Module receipts are isolated there
so later standalone Apply operations do not overwrite this run's recovery data.
The report is journaled before each attempted module; interrupted operations
remain visible for recovery. Each record directory is private. Keep these local
records/backups private; unlike portable workspaces, they contain local paths
and may contain previous Starship configuration text.

## Advanced single-component workflows

Single-component documents, native exports and reusable presets remain available
under **⋮**. Their **Use Design…** action reviews only their owned contents; they
are not required steps in normal theme editing. Opening/saving a native source
does not automatically authorize replacing it.

Standalone image installs can use an independent managed configuration at
`$XDG_DATA_HOME/termimochi/targets/<terminal>/config.jsonc`, leaving shared Fastfetch
untouched and reporting **Installed · not enabled**. Explicit shared replacement
has a separate opt-in and cross-terminal warning. Required real-terminal checks
still apply. This advanced configuration-only output is not the normal Kitty
theme workflow above, which publishes and opens a complete independent session.

Portable image export bundles also remain an advanced manual output. Neither
export nor installation automatically adds per-terminal startup routing. The
separate **Terminal Startup…** feature requires its own explicit review before
changing a Bash startup file; Save and Use Theme do not enable it.
