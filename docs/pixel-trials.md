# Image/GIF terminal trials and installation

Import an editable PNG/JPG/WebP/SVG/GIF source in **Greeting**, enable it, and
choose **Display** and **Columns** in the normal editor. Save preserves these
choices and the original source in the complete `.termimochi.json` scheme.

1. Choose Image, Animation (GIF only), Character, or Recommended. Protocol hints
   live in **Advanced · output compatibility**. Choose the persistent **Greeting
   target** in the workbench: Kitty, Ptyxis or xterm. This is a local selection,
   not portable approval or proof of support. Missing executables are not installed.
2. Click the main **Try in Terminal** button. A new terminal opens with temporary files and
   absolute asset paths. Your active Fastfetch configuration and shell startup
   files are untouched. The test uses your artwork/layout with fixed safe sample
   fields, not arbitrary imported commands or network modules.
3. Use the GUI feedback: **Looks correct**, **Nothing displayed**, or **Animation
   broken**. A missing/ambiguous response stays **Unverified**; **Try unverified
   output** explicitly attempts it anyway. Terminal T/Y/N/Q remain fallback keys.
   For GIF, check actual movement, placement and
   transparent-frame trails. A successful process exit does not confirm anything.
4. After visual confirmation, **Review Scheme & Apply…** (or main **Apply Scheme…**) shows the selected Fastfetch
   target, permanent asset directory, ANSI fallback path and complete Before/After
   configuration. Review imported commands here: the full configuration retains
   them, even though the trial did not run them. Cancel writes nothing.
5. Select Greeting, then **Back Up & Apply Selected** writes private immutable assets under
   `$XDG_DATA_HOME/termimochi/image-greetings/` (normally
   `~/.local/share/termimochi/image-greetings/`). By default the config is independent:
   `$XDG_DATA_HOME/termimochi/targets/<terminal>/config.jsonc`. The shared Fastfetch
   file is untouched, and the result says **Installed · not enabled**. External
   changes block replacement. Reapplying uses the same plan and backup mechanisms.

Pixel configurations explicitly set `display.pipe` to `false`; otherwise
Fastfetch can silently substitute a builtin logo under a pipe/`NO_COLOR` policy.
This change is visible in Before/After. It does not change your shell environment
or the ordinary ANSI configuration.

Run `fastfetch` in the tested terminal if the target is its default configuration.
For a custom target, use `fastfetch --config /absolute/path/to/config.jsonc`.
No `cd` into an export folder is needed. Installation does not add or enable a
startup hook; an existing hook pointing to that same target sees the updated
configuration. Other terminals opening it may not support its pixel protocol.
Use the existing separate startup review only when you explicitly want that change.

Managed Kitty PNG/GIF configurations now include a reviewed `general.preRun`
startup guard. It waits for the terminal's row/column dimensions to remain stable
for 300 ms (up to 1.5 seconds if resizing continues), then Fastfetch renders once.
It neither clears the screen nor reads keystrokes or changes terminal input mode.
Non-Kitty or redirected runs skip the guard. Existing `preRun` commands are
preserved after it. This prevents the cold-start blank-image race reproduced in
the tested Kitty version; it is not a promise about every compositor or later resize.

The guard references the installed TermiMochi executable by absolute path. Keep
that executable available; moving/uninstalling it removes this protection.
Previously installed configurations are not rewritten automatically: repeat
**Try in Terminal → Review Scheme & Apply…** to adopt the guard.
Portable export bundles and manually authored configurations are unchanged.

## Capability states and fallback

| Result | Meaning |
| --- | --- |
| Kitty static: protocol response received | Matching graphics query ACK; still needs visual confirmation |
| Animation: unverified | Static ACK never proves motion; confirm this GIF independently |
| Sixel: protocol response received | VT220-or-later Device Attributes advertise feature 4 |
| Unavailable | The session's response rules out the selected protocol; no pixel payload is sent |
| Unverified | No conclusive reply, cancellation or timeout; not a compatibility claim |
| Current output: visually confirmed | You confirmed this artwork and output in this session only |

Kitty graphics are queried using its
[official query plus primary Device Attributes sequence](https://sw.kovidgoyal.net/kitty/graphics-protocol/#querying-support-and-available-transmission-mediums).
Sixel feature flags and cell-size reports follow
[xterm control sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html).
Terminal identity is not a Sixel feature bit. Legacy/ambiguous DA replies stay
unverified for Sixel. Animation confirmation is independent of both static states.
Multiplexers, remote sessions, fonts, DPI and different builds can change results.

**Use Character & Try** explicitly edits the scheme's display intent and starts
the same temporary-test flow without requiring pixel support. It uses the accepted character artwork; GIF
fallback is a still. This remains available when the selected pixel output is
unavailable or visually broken. Managed pixel installations also keep
`config-ansi.jsonc` alongside their assets for an explicit manual fallback.
There is no silent terminal-name-based switching.

**Shared replacement is an explicit opt-in.** Checking **Replace shared greeting
with pixels** changes the proposed destination, not the file itself. Review warns
that **all terminals reading that file** are affected: incompatible terminals can
show a blank image. It is never checked automatically or remembered on reopening.
To return a shared file to characters, select **Display → Character**, choose
Greeting in **Apply Scheme…**, and confirm. To undo an application exactly, use
**Last Application & Recovery…**. Changing the terminal dropdown alone writes no
daily configuration. Automatic per-terminal startup routing is not implemented.

Sixel trials resize the generated raster to queried target cell dimensions. If
cell size is unverified, preview-derived sizing remains and must be checked
visually. Changing target, protocol or maximum columns discards the previous
confirmation. Approval is tied to artwork, occupancy, layout, protocol, executable,
known Kitty/Xterm config files or Ptyxis profile/global settings, and preview cell
geometry. A fresh trial always clears older approval, even if it fails. Known
configuration changes invalidate it; imported field-only edits retain image
evidence but their commands are still untested. Restarting/reopening never restores
approval. Retest after runtime zoom, external include changes, X resource reloads,
remote/multiplexer changes or compositor DPI changes: those cannot all be observed.

## Recovery and boundaries

- **Last Application & Recovery… → Restore This Application…** uses the existing Fastfetch backup/conflict
  checks. Successful restoration returns the exact prior file, or removes only
  the configuration newly created by this installation.
- Managed assets are deliberately retained during restore and uncertain failures:
  a saved configuration/backup can still reference them. Do not delete directories
  still referenced by active files or backups. Automatic garbage collection is
  not included.
- Closing/stopping an unconfirmed trial removes only its own temporary tree.
  A confirmed artifact may be retained for this app session so unified Apply can
  reuse it; it is not portable approval. No running user terminal is reused or
  killed. Failed/unconfirmed trials cannot be installed.
- The Greeting **Design Preview** shows processed pixels/GIF playback in a GTK
  composition. It is not a native Kitty/Sixel renderer. Other modules keep VTE
  testing scenes. All Greeting application paths now honor the saved display
  intent; none silently substitute ANSI for an unverified image.
- Main Save / Ctrl+S always saves the complete scheme. Module presets are explicit
  secondary commands. Legacy documents stay character output and retain source
  recipes and size. Portable
  **Export Folder…** remains separate and retains its documented relative paths.
- The full configuration is not executed automatically after installation.
  Trials require the system `/usr/bin/fastfetch` and a selected installed terminal.
