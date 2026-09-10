# Image/GIF terminal trials and installation

Open **Bottom bar ⋮ → Export → Export Image Greeting…** after accepting an
editable PNG/JPG/WebP/SVG/GIF source and enabling Greeting.

1. Choose the output protocol, maximum columns and target terminal: Kitty,
   Ptyxis or xterm. Missing terminal executables are not installed automatically.
2. Click **Test in Terminal**. A new terminal opens with temporary files and
   absolute asset paths. Your active Fastfetch configuration and shell startup
   files are untouched. The test uses your artwork/layout with fixed safe sample
   fields, not arbitrary imported commands or network modules.
3. Follow the terminal prompt. A missing/ambiguous protocol response stays
   **Unverified**; press **T** only if you want to try that output anyway.
   Confirm actual appearance with **Y**, reject broken output with **N**, or
   leave unverified with **Q**. For GIF, check actual movement, placement and
   transparent-frame trails. A successful process exit does not confirm anything.
4. After visual confirmation, **Review Install…** shows the selected Fastfetch
   target, permanent asset directory, ANSI fallback path and complete Before/After
   configuration. Review imported commands here: the full configuration retains
   them, even though the trial did not run them. Cancel writes nothing.
5. **Install & Apply** writes private immutable assets under
   `$XDG_DATA_HOME/termimochi/image-greetings/` (normally
   `~/.local/share/termimochi/image-greetings/`), then backs up and replaces the
   reviewed Fastfetch configuration. External changes block replacement.

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
**Test in Terminal → Review Install… → Install & Apply** to adopt the guard.
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

**Test ANSI Fallback** follows the same temporary-test and reviewed-install flow
without requiring pixel support. It uses the accepted character artwork; GIF
fallback is a still. This remains available when the selected pixel output is
unavailable or visually broken. Managed pixel installations also keep
`config-ansi.jsonc` alongside their assets for an explicit manual fallback.
There is no silent terminal-name-based switching.

**Known limitation: the installation target is a shared Fastfetch file, not a
terminal-specific profile.** Selecting Kitty for a trial does not restrict the
installed configuration to Kitty. If Ptyxis or another incompatible terminal
reads the same file, the image area can be blank. To return to character output,
select **Ptyxis → Test ANSI Fallback**, confirm with **Y**, then choose
**Review Install… → Install & Apply**. Changing the target dropdown alone does
not change the daily configuration. Automatic per-terminal image/ANSI selection
is not implemented yet.

Sixel trials resize the generated raster to queried target cell dimensions. If
cell size is unverified, preview-derived sizing remains and must be checked
visually. Changing target, protocol or maximum columns discards the previous
confirmation. Test again after changing the artwork, target font or DPI.

## Recovery and boundaries

- **Restore Previous Configuration** uses the existing Fastfetch backup/conflict
  checks. Successful restoration returns the exact prior file, or removes only
  the configuration newly created by this installation.
- Managed assets are deliberately retained during restore and uncertain failures:
  a saved configuration/backup can still reference them. Do not delete directories
  still referenced by active files or backups. Automatic garbage collection is
  not included.
- Closing/stopping a trial removes only its own temporary tree. No running user
  terminal is reused or killed. Failed/unconfirmed trials cannot be installed.
- The GTK checkerboard and embedded VTE are unchanged: the main Live Preview,
  ordinary Greeting Apply and Apply Scheme's Greeting component still use ANSI.
  Applying those later can replace the image configuration; review Before/After.
- Save Preset/Workspace still only saves the editable workspace. Portable
  **Export Folder…** remains separate and retains its documented relative paths.
- The full configuration is not executed automatically after installation.
  Trials require the system `/usr/bin/fastfetch` and a selected installed terminal.
