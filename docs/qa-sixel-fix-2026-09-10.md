# Transparent Sixel export fix

## Diagnosis and change

The [native display failure](qa-native-image-display-2026-09-10.md) was downstream
of the editable image: the processed PNG had zero-alpha borders and holes, but
Fastfetch 2.57.1's ImageMagick/Sixel path produced black pixels there. Its
[`printImageSixel` implementation](https://github.com/fastfetch-cli/fastfetch/blob/2.57.1/src/logo/image/image.c)
passes the resized image to ImageMagick's SIXEL blob encoder. This identifies the
affected conversion path, not a claim that every ImageMagick version behaves alike.

TermiMochi now generates `logo.sixel` itself and exports it through Fastfetch's
raw logo mode. The [Sixel background control](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
is P2=1; transparent pixels never enter a color plane. Opaque black remains ink.
No system converter, Fastfetch binary, user configuration or default terminal was
modified. Kitty-direct PNG, Kitty animation and ordinary ANSI Apply retain their
existing rendering paths.

The encoder resamples premultiplied alpha, then uses binary transparency (alpha
below 128 is clear). Flat images use an exact palette; complex images use the
existing color_quant/NeuQuant library with at most 65,536 visible training pixels
and 256 palette entries. Sixel percent-RGB rounding remains a format limitation.
Only generated raster controls, integer parameters and color planes are emitted;
no imported terminal commands or external file references are forwarded.

Output limits: 2048x2048 pixels, 8 MiB stream including terminator, cooperative
12-second deadline. Input is an already bounded 1024px-side processed image.
Pixel cell dimensions are validated before multiplication. Encoding and fallback
validation happen before creating the owner-only bundle directory; failed or
oversized exports leave existing files untouched.

Sixel bundles contain five files: `config.jsonc`, `logo.sixel`, `logo.png`,
`config-ansi.jsonc` and `README.md`. The pixel raster follows the preview's captured
physical cell size, including GTK scale. Raw Sixel cannot automatically resize to
a new target font/DPI; the user must re-export. The interface and bundle guide
explain this, palette/binary-alpha differences from the PNG reference preview,
and the portable ANSI fallback. Existing export bundles are not rewritten.

## Verification

- **323 ordinary tests passed**: GUI binary 287, CLI 4, integration 5, core 27;
  43 opt-in tests excluded. New unit coverage includes black ink versus transparent
  pixels, six-row boundaries, run-length encoding, deterministic complex palettes,
  alpha 127/128 behavior, dimension rejection and no-directory-on-failure.
  Bundle tests verify raw source selection, private file modes and exact unchanged
  ANSI fallback content.
- **18/18 native/GTK regressions passed** at 1x and 2x:
  `/tmp/termimochi-regression-rf9ltw5j/results.json`.
  These cover real Kitty GIF, native Fastfetch protocol output, actual static
  Kitty/xterm pixels, GIF source re-edit/Undo, existing image import/background
  controls/cancel/coalescing/export/restart, both pixel dialogs, and Apply/load/
  conflict/preview synchronization.
- After the final sparse-image palette sampling and stream-limit adjustments,
  the static visual matrix was rerun: **2/2 passed** at 1x and 2x:
  `/tmp/termimochi-regression-1p0my8mw/results.json`.
  Each run checks six chart scenes per protocol (left/80 columns, right/100,
  top/120; light/dark) plus captures the real brand icon on both themes. All
  transparent hole/margin assertions now pass unchanged. The brand captures in
  the broad run and final 2x run were manually inspected: no black rectangle.
- An initial focused post-fix matrix also passed **3/3**:
  `/tmp/termimochi-regression-ue3lyon4/results.json`.
- Clippy with warnings denied, formatting, diff checks and release build passed.
  Desktop checks: **9 tests passed**, metadata and **17 embedded resources** valid.

Real renderers: Kitty 0.45.0 and xterm 407 with Fastfetch 2.57.1, direct terminal
stdout (not PIPE forwarding), dedicated Xvfb, isolated XDG state, no user shell
startup. GTK scaling was exercised at 1x/2x; native Kitty/xterm font geometry was
explicitly fixed in their fixtures. This is not certification of every terminal's
DPI behavior. Native Wayland, SSH, other Sixel implementations and the desktop
folder portal remain unverified. No transparency assertion was weakened.

A matrix launch ran out of temporary quota while pinning its helper, before any
tests ran. About 2 GiB of regenerable program copies from completed/aborted test
runs were removed. All reports, screenshots, project and user files were retained.
The subsequent matrices completed successfully.

## Delivery

Installed with `scripts/install.sh install --no-build`. Build and installed binary
SHA-256 both match:
`613d52b605537811a6fe0900953c2ab907a01c376cd8298bb6453df34b6423be`.

Restart TermiMochi and export a new Sixel bundle. Older black-background bundles
continue using their original conversion path until replaced by a new export.
No commit or push was made.
