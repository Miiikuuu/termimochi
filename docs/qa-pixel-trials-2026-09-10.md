# Real-terminal image trials QA — 2026-09-10

Follow-up: the startup timing limitation recorded below is addressed for newly
managed Kitty configurations by the [startup guard QA](qa-kitty-startup-2026-09-10.md).
This report preserves the original unguarded test findings.

## Scope and isolation

Temporary real-TTY trials, capability queries, explicit visual approval, managed
installation and existing Fastfetch backup/recovery. Tests use a dedicated Xvfb
display, isolated XDG directories and in-memory GSettings. No user Fastfetch
configuration, shell startup file or terminal profile was changed. The xterm
test binary and its libraries are unpacked test tools, not a system installation.

Runtime: Fastfetch 2.57.1, Kitty 0.45.0, xterm 407, Ptyxis 50.1 / VTE 0.84.0.

## Automated checks

- Workspace tests: **340 passed**, 47 opt-in tests excluded from the ordinary run.
- Formatting and Clippy with `-D warnings`: passed.
- Desktop resource tests: **9 passed**; metadata and **17 embedded resources**
  plus packaged copies validated after installation.
- Release workspace build and local install: passed; built and installed app
  SHA-256: `c7d445c33fa5527089b3921701e8a68df18580f825589732afd369d5c43b321a`.
- Native/GTK regression matrix: **12/12 passed**; see final `results.json` from
  `termimochi-regression-kzl4hojx` under the isolated QA cache.

The matrix runs six opt-in cases at both GTK 1x and 2x: actual terminal trials;
GIF playback/pause/seeking; pixel export lifecycle/geometry; trial installation
gate/review/cancel/restore; Fastfetch synchronization/conflicts; whole-scheme
review/application/recovery. A separate pointer/screenshot run verified that
wheel events do not change output parameters and the new controls are visible.

## Real terminal cases

The driver captures actual terminal windows and checks colored fixture pixels
before sending Y. Animation requires multiple captures with moving bounds and
no accumulated trails. It does not fabricate capability replies. Native terminal
font sizes are fixed by their test launchers; the 1x/2x matrix changes GTK scale,
not a claim that every terminal DPI combination was tested.

| Case | Acceptance |
| --- | --- |
| Kitty static PNG | Matching query ACK, actual transparent image, explicit Y |
| Kitty rejected picture | Actual image under `NO_COLOR=1`, N marks failed and blocks installation |
| Kitty GIF | Static ACK leaves animation unverified; moving frames then Y confirm animation separately |
| xterm Sixel | Real feature-4 DA and cell-size reply; regenerated transparent raster visible |
| Kitty Sixel | Ambiguous legacy DA remains unverified; Q cancels without sending artwork |
| Ptyxis Kitty image | Negative protocol result; no artwork sent; installation blocked |
| Ptyxis ANSI fallback | Accepted character artwork renders; explicit Y |
| Managed Kitty configuration | Temporary trial removed; installed absolute paths render from `/`, including `NO_COLOR=1` |

Ordinary tests cover temporary cleanup without daily writes; safe projection
excluding imported commands/display flags/external logo files; OSC filtering;
layout-gap preservation; protocol-identity parsing; no automatic animation
verification; post-test config/asset edits blocking review; late target conflicts;
private managed files; exact config restore without deleting referenced assets;
ANSI fallback independent of Sixel generation; fixed argv without a shell.

GTK install-gate tests inject a test assessment to exercise UI state, not to prove
protocol support. They verify disabled install before approval, width changes
invalidating approval, closing the temporary dialog, cancelling Before/After
without creating managed assets, confirmed installation, and exact restoration.
The separate real-terminal case provides the rendering evidence.

## Findings and explicit limits

The first real-TTY run returned a Kitty ACK but Fastfetch displayed the builtin
Ubuntu logo under inherited `NO_COLOR`/pipe policy. The worker now explicitly
requests non-pipe output after verifying it owns a TTY. Managed pixel configs
also set `display.pipe=false`, visible during review. This is regression-tested;
successful process exit alone still never grants approval.

An immediate terminal-startup test exposed Kitty losing a PNG during its first
map/resize. This external startup timing is **not fixed or certified** here.
The managed-config test waits 750 ms before invoking Fastfetch, matching a user
running it in an opened window and the existing pixel-rendering regression.
Trials already spend time querying before rendering. Existing startup hooks are
not edited; users can rerun Fastfetch after the window opens if affected.

Other limits: tests use bounded synthetic artwork, not every imported image;
safe trial fields are not the complete imported configuration; the full reviewed
configuration is not executed by installation; tmux/SSH/Wayland and other terminal
builds require their own verification. Embedded VTE and ordinary Apply remain
ANSI. Unknown support is never promoted because an export succeeded. Managed
assets intentionally remain after restore because backups may reference them.
