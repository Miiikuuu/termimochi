# Image conversion workbench

TermiMochi converts a bounded local PNG, JPEG, WebP or static SVG into portable text and ANSI
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
not enable original-image terminal protocols or make character output lossless.

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
TXT and ANSI exports include only final artwork. Legacy/imported ANSI with no
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
