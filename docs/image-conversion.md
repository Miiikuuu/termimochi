# Image conversion workbench

TermiMochi converts a bounded local PNG, JPEG, WebP, static SVG or GIF still into portable text and ANSI
colors. The conversion editor has an independently scrolling adjustment sidebar
and a fixed preview area. No cloud service, GPU renderer or additional converter
installation is required.

## SVG import

Use **Import Artwork…** with an `.svg` file. Static vector shapes, gradients,
local `#id` reuse, clipping, masks, filters and text go through the same Original /
Converted editor, crop, background removal and character modes as raster images.
The original SVG bytes and recipe are embedded in editable presets/workspaces;
deleting or moving the file does not prevent **Edit Artwork…**. Text uses fonts
from `/usr/share/fonts` and `/usr/local/share/fonts`, with a DejaVu Sans default.
Convert text to paths in your SVG editor for consistent results on other machines.

The [resvg renderer](https://docs.rs/resvg/0.48.1/resvg/) is built into TermiMochi.
SVG is rasterized onto a transparent canvas with a 1024-pixel longest side,
including upscaling small vector viewports, preserving premultiplied alpha and
aspect ratio. The existing character-grid limits still apply: SVG import does
not make character output lossless. Pixel-image export is a separate operation
described below.

Sources must be UTF-8, at most 2 MiB, use the standard SVG namespace, and stay
within 10,000 XML nodes / 64 nesting levels. Natural dimensions use the raster
decoder's existing 8192-pixel / 16-megapixel bound. SVGZ, embedded/linked raster
images, scripts, animation, foreign HTML, DTD/entity declarations, external
references, CSS escapes/at-rules and stylesheet processing instructions are
rejected with an error; they are not silently fetched or executed.

Rendering happens in a separate copy of the application executable, before GTK
initialization, under Bubblewrap and `prlimit`: no network, no home/project
mounts, read-only runtime/fonts/input, private temporary storage, 1 GiB address
space, 3 CPU seconds, a 5-second parent deadline and a 6 MiB response limit.
Both [usvg image resolvers](https://docs.rs/usvg/0.48.1/usvg/struct.ImageHrefResolver.html)
are disabled as a second barrier to external and embedded resources. A failed,
oversized or timed-out render leaves Apply disabled; no unsandboxed fallback is
used. Raster imports do not require this subprocess.

## Using the editor

Mouse-wheel and touchpad scrolling over adjustment controls moves the sidebar,
including when a numeric input has focus. It never changes a parameter. Use
clicks, slider dragging or the keyboard to edit values.

Start at **48 or 64 columns**, then choose a character mode:

- **ANSI Detail** uses fine blocks and diagonal shapes with foreground/background
  colors. This is Unicode character art, not strictly ASCII, and is usually the
  better starting point for compact illustrated logos.
- **ASCII** uses printable ASCII glyphs with optional ANSI foreground colors.
  Balanced combines shading and contours; Contours only removes shading; Tone
  only disables the edge stage. Pure text export contains no color escapes.
- **Half blocks** retains the predictable two-vertical-samples-per-cell style.

**Recipe** selects adjustment starting points, not saved artwork:

- Balanced: neutral tone and moderate edge sensitivity.
- Illustration: trim empty margins, mild contrast/saturation and noise smoothing.
- Line Art: switch to ASCII contours, trim margins and use terminal-theme ink.
- Photo: smooth noise and use tone-based shading when ASCII is selected.

Manual edits mark the recipe **Custom**. Changing a recipe keeps background
removal settings. **Reset adjustments** also turns background removal off and
resets crop, tone, color and structure, keeping character mode and width.
ASCII-only controls are shown only in ASCII mode; saturation is enabled only
for source-color output. Changes remain a draft until **Use Artwork**.

Exposure (-2 to +2 EV), contrast, saturation, smoothing and edge sensitivity
operate on a decoded snapshot. **Crop & margins** provides independent 0–45%
left/top/right/bottom crops, followed by optional outer-margin trimming. Trim
recognizes transparency or a near-uniform edge-connected border; it is not
semantic subject detection or a background-removal tool. It keeps a small margin
and does not erase enclosed light details.

**Background → Remove background** removes a solid-color backdrop before
conversion, leaving empty terminal cells transparent. It is off by default:

- **Automatic** detects a near-uniform opaque border. If it cannot identify one,
  the editor asks you to pick a color and disables Use Artwork until resolved.
- **Custom color** accepts a HEX color. **Pick** opens Original with a crosshair;
  click the background, or press Esc / Pick again to cancel picking. Empty
  letterbox space and transparent pixels cannot be sampled.
- **Tolerance %** controls how different a color may be from the selected key.
  Start low; increasing it can remove similar foreground colors.
- **Edge softness %** fades near-matching colors into transparency and reduces
  matte-colored fringes. It is a color-distance transition band, not a spatial
  blur radius.
- **Connected to edges only**, on by default, protects enclosed same-color
  details. Gaps in an outline or a manual crop through the subject can let the
  removal reach inside. Turn it off only to remove matching colors everywhere.

This is color-key removal, not AI subject segmentation. Complex photographic
backgrounds still need external editing. Original always retains the source
background for comparison/picking; only Converted and the accepted ANSI artwork
receive removal. No-match, invalid-color and entirely erased results cannot be
accepted. Disable removal or Reset adjustments to recover. Removal does not
automatically shrink the grid; use Trim empty margins for compact framing.

**Original / Converted** compare the same crop and canvas size. Original retains
source colors; Converted receives adjustments. **Fit preview** changes display
size only, not exported resolution. ASCII defaults to 64 columns; high-resolution
160 × 96 output remains available. Terminal font size and the width needed for
information fields still matter when displaying the exported greeting.

The final text/ANSI artwork, original image bytes and versioned conversion recipe
are saved in TermiMochi presets/workspaces. **Edit Artwork…** restores all controls
and uses the embedded image even if its original file was moved or deleted.
Original metadata may be present in those bytes; sharing a preset/workspace also
shares its source image. The filesystem path is never persisted. Fastfetch,
TXT and ANSI exports include only final artwork (pixel bundles are described
below). Legacy/imported ANSI with no
source offers **Reimport Original…**; it cannot reconstruct lost conversion settings.
Cancel changes nothing; Use Artwork creates one existing Greeting Undo/Redo step,
including both the source and recipe. Image bytes are shared across history
snapshots rather than copied per slider edit. Greeting/workspace documents have
a 24 MiB bound, including a maximum 16 MiB source encoded as base64. Decoding
and conversion remain bounded background tasks. Save Preset stays
local-only; a successful explicit Fastfetch apply also saves the Greeting preset,
so a restart cannot silently bring back artwork from before background removal.
When the saved preview differs from Fastfetch, Load Applied offers an explicit,
undoable load and local save. Dirty drafts require confirmation. Local save
failures remain visible and never bypass preset conflict checks.
After accepting, the Greeting sidebar shows a fitted color thumbnail. Click it
to enlarge; Fit off gives full-size scrolling without altering saved artwork.
Review & Apply can run Fastfetch once in a new Ptyxis window after a successful
apply. This does not install a shell startup hook or apply an unsaved color theme.
An independent **Terminal Startup…** opt-in can install a reviewed, removable
Bash block; neither image import nor preset/workspace loading enables it.

## Pixel-image bundles

After accepting an imported image with **Use Artwork**, enable Greeting and use
**Bottom bar ⋮ → Export → Export Image Greeting…**. This export uses the embedded
PNG/JPG/WebP/SVG source and accepted recipe, even if the original file was moved.
Text, ANSI and built-in character logos do not contain recoverable source pixels;
import an original image before using this export.

The dialog shows the processed image on a transparency checkerboard. Choose
**Kitty · Direct PNG** or **Sixel**, adjust **Max columns**, then **Export Folder…**.
It creates a fresh private `termimochi-image-*` folder containing:

- `logo.png`: re-encoded static pixels, preserving crop, margin trimming,
  background removal, exposure, contrast, saturation, color mode and smoothing.
  No original metadata, file path or editable source is included. The existing
  bounded decoder limits the longest side to 1024 pixels.
- `config.jsonc`: the current Greeting fields/colors with its logo replaced by
  `kitty-direct` with relative `logo.png`, or `raw` with relative `logo.sixel`,
  explicit dimensions and position.
  Maximum logo width is 8–120 columns; aspect-ratio sizing may reduce the actual
  width further to keep height within 64 rows. Dimensions use the current preview
  font's cell ratio. Card layout uses a top-positioned image.
- `README.md`: compatibility requirements and the launch command.
- Sixel also includes `logo.sixel` (generated transparent raster commands) and
  `config-ansi.jsonc` (the accepted portable character-art fallback).

Open a terminal **in that generated folder** and run:

```sh
fastfetch --config config.jsonc
```

The relative logo path uses the working directory. Keep all bundle files together;
moving the entire folder is supported. Review imported configuration commands or
network modules before running: these are retained, not executed by this export.
Cancel writes nothing. Every export uses a new directory; existing files, active
Fastfetch configuration, shell startup and the local Greeting draft are unchanged.

According to [Fastfetch's logo options](https://github.com/fastfetch-cli/fastfetch/wiki/Logo-options),
`kitty-direct` needs a supporting terminal and explicit width/height. TermiMochi's
Sixel bundle uses Fastfetch's `raw` path, not its ImageMagick conversion: the latter
produced black backgrounds in the native display regression. No ImageMagick is
required for the generated bundle. `logo.sixel` selects transparent background
mode (P2=1) and never paints transparent pixels; opaque black remains black.

Sixel has fixed pixel dimensions, calculated from the captured preview's physical
cell width/height (including GTK scale) and requested columns/rows. Changing only
Fastfetch's logo width/height does not resize this raster. **Re-export for a
different terminal font, DPI or logo size**. Limits are 2048×2048 output pixels,
8 MiB encoded data and a cooperative 12-second worker deadline; excessive sizes
are refused before creating an export folder. Resampling uses premultiplied alpha.
Alpha below 128 becomes transparent; the rest is opaque. Images with at most 256
colors retain an exact palette before Sixel's percent-RGB conversion; complex
images use bounded NeuQuant sampling. Sixel's binary alpha and palette may differ
from the full-color, soft-alpha PNG checkerboard preview, particularly at edges.
The original image, accepted recipe and reference PNG remain unchanged.

TermiMochi does not automatically detect or certify these capabilities. A saved
bundle is not proof that the receiving terminal can render it.

The checkerboard is a GTK pixel preview, **not Kitty/Sixel rendering inside VTE**.
Live Preview, normal Fastfetch Apply and startup integration remain on the ANSI
path. Character density, ASCII contour/inversion effects and opening animations
do not apply to pixel export. GIF animation is an explicit separate output,
described below. Animated PNG/WebP and importing raw terminal graphics payloads
remain unsupported.

## GIF animation

**Import Artwork…** accepts GIF87a/GIF89a. The image-to-text editor uses the first
nontransparent composited frame for its Original/Converted views and the ANSI
fallback. The complete original GIF and accepted recipe are embedded in the
existing editable source; moving/deleting the original does not break re-editing.
The import status explicitly distinguishes this still from animation playback.

After **Use Artwork**, enable Greeting and open **Export Image Greeting…**. A GIF
source adds **Kitty · Animated GIF**, selected by default. **Play / Pause** controls
a looping pixel preview; the frame scrubber selects a frame without changing the
source or exported animation. Playback does not start automatically. Wheel motion
cannot change the scrubber or output parameters. Selecting **Kitty · Still PNG**
or **Sixel · Transparent still** exports only a representative still, not motion.

Animation preparation composites frame offsets and disposal operations first.
The union of all per-frame crop/trim bounds defines one fixed canvas, preventing
trim from chasing the moving subject. Automatic background removal uses one
inferred color across all frames; use a manually picked key for changing or
ambiguous backgrounds. Empty individual frames are allowed, but an entirely
erased animation is rejected. Tone, ink and smoothing apply to every frame.

The processed GIF has palette colors and binary transparency: alpha below 128 is
cleared, the rest becomes opaque. This is not soft PNG transparency. The preview
re-decodes the exported GIF, so it shows the same quantized colors and transparency.
Delays shorter than 20 ms are raised to 20 ms. The preview accounts for delayed
callbacks rather than accumulating one timer tick of drift per frame.

The animation bundle contains `config.jsonc`, `logo.kitty`, `logo.gif`, `logo.png`,
`config-ansi.jsonc` and `README.md`. Launch from that folder with:

```sh
fastfetch --config config.jsonc
```

Fastfetch's **raw** logo type reads the generated `logo.kitty` stream. It contains
only bounded Kitty graphics commands and inline, locally re-encoded PNG frames;
there are no external image paths, shell commands or clipboard/title sequences.
Frame replacement (`X=1`) and independent full canvases prevent transparent pixels
from accumulating earlier frames. This avoids the ghosting reproduced with
Fastfetch 2.57.1 / Kitty 0.45.0's `kitty-icat` path; no `kitten` helper is required
for the exported stream. The protocol implementation follows the
[official Kitty animation specification](https://sw.kovidgoyal.net/kitty/graphics-protocol/#animation)
and [Fastfetch raw logo interface](https://github.com/fastfetch-cli/fastfetch/wiki/Logo-options#raw).

The terminal stream loops continuously. `logo.gif` is a shareable processed GIF,
not the file Fastfetch uses for animation. `logo.png` is a still image. Use
`fastfetch --config config-ansi.jsonc` for a terminal without Kitty animation.
Re-export to change animation dimensions: they are embedded in both the stream
and configuration. Keep the bundle together. Repeated runs allocate fresh image
IDs through image numbers rather than deleting unrelated terminal images.

GIF sources are bounded to 16 MiB, a 1024×1024 canvas, 120 frames, 64 MiB of
aggregate decoded frame pixels and 60 seconds per cycle. A container preflight
rejects out-of-canvas frame rectangles, excess frame budgets, malformed/truncated
blocks and trailing data before decompression. Decoder allocation limits also
apply. GIF workers are serialized to avoid concurrent large decode buffers, with
cooperative elapsed-time checks between frames (not hard subprocess deadlines).
Processed GIF output is capped at 16 MiB and the inline animation stream at 32 MiB.
These are data limits, not a total process-memory guarantee.

Native Kitty animation is not a universal terminal feature. Raw protocol imports
remain blocked; do not assume arbitrary `.kitty`/binary files are safe. The main
VTE preview, normal Apply and startup integration continue to use accepted ANSI.
No animation bundle is installed or executed automatically.

## Independent implementation and references

This implementation references the public design ideas of
[Chafa](https://hpjansson.org/chafa/) (selectable symbol shapes and per-cell color
fitting) and [Typographer](https://github.com/user-simon/typographer#algorithm)
(separate tonal fill, connected edges and directional ASCII strokes).
No source code, character tables, assets or binaries from either project are
copied, linked or bundled. TermiMochi's implementation remains independently
authored Rust under the repository's MIT license. This is not a port, an
equivalence claim, or a claim to outperform either project.

The pipeline is:

1. Validate edits; crop the premultiplied-alpha snapshot without modifying it.
   Optional background removal infers a per-channel border median only when at
   least 90% of the uncropped border is opaque and within 18 RGB units of it, or
   uses the picked key. An iterative four-connected flood fill selects eligible
   edge-connected pixels (or all pixels when disabled). The maximum absolute RGB
   channel difference controls tolerance and a smoothstep alpha transition;
   subtracting the estimated matte contribution limits fringes. Alpha can only
   decrease, and RGB remains premultiplied. Detection precedes crop in coordinate
   space; removal precedes tone changes and resampling.
2. Apply exposure using an approximate gamma-2.2 transfer, contrast about midtone,
   and luminance-relative saturation. Preserve alpha and premultiplication.
3. Resample to the bounded character grid, with optional Gaussian smoothing.
4. For ASCII, compute Sobel gradients, non-maximum suppression, percentile-based
   high/low thresholds and iterative eight-connected hysteresis. Aggregate edge
   direction inside each 4 × 4 cell. Combine with tonal fill according to the
   selected structure mode. This is a Canny-style pipeline, not an exact Canny
   or Otsu implementation. There is no semantic simplification or real-font
   glyph matching in this version.
5. For ANSI Detail, sample 8 × 8 subcells and fit independently generated masks:
   quadrant blocks, eighth-height/width blocks and four triangular halves.
   Minimize RGB squared error with channel weights 2:4:1 and a transparency
   mismatch penalty. Choose deterministic ties favoring common block glyphs.
   Never represent transparency using a default foreground, which would paint
   unintended terminal text color. Half blocks remain a separate simple path.
6. Map color mode, emit SGR/text and revalidate through the shared Artwork
   sanitizer and geometry/byte limits. Reject all-transparent or blank output.

The worker decodes once and coalesces requests. Stale results cannot update
either reference or converted views. A bounded result channel, fixed image/grid
limits and iterative flood fills bound memory. Tests cover edits, alpha, tiny
inputs, connected contours, crop preservation, background-key detection, enclosed
detail protection, alpha transitions, HiDPI color picking, exact shape fixtures, portable
round trips, real GTK/VTE controls, cancellation and temporary Fastfetch apply.
Each VTE frame includes its own clear/home sequence so queued older feeds cannot
leave a partial previous frame behind. GUI tests read the actual terminal text
back and compare it with the generated artwork, in addition to screenshot review.

More characters do not guarantee a better small logo. A complex image still
loses information at 40–64 columns; automatic results need visual review at the
actual font and background. Triangular glyph appearance can vary by font.
