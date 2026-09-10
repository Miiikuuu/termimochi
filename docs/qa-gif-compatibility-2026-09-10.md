# GIF compatibility QA

## Scope

GIF import retains an embedded editable source and uses the first non-transparent
frame for ANSI artwork. Image Greeting export offers a paused-by-default animation
preview, Play/Pause, frame seeking and static Kitty/Sixel alternatives. Crop/trim
uses one union canvas; automatic background removal uses one inferred color across
frames. The preview re-decodes the processed GIF so palette and alpha match export.

Animated bundles contain `config.jsonc`, `logo.kitty`, `logo.gif`, `logo.png`,
`config-ansi.jsonc` and a README. Fastfetch's raw logo mode displays a generated
Kitty graphics animation with inline PNG frames and full-frame replacement (`X=1`).
This is generated output only, not permission to import arbitrary raw commands.
No kitten executable is required. The terminal stream loops continuously; the
standalone GIF retains its encoded repeat setting. Size changes require re-export.
Normal Live Preview and Apply remain static ANSI. No active Fastfetch configuration,
shell startup, default terminal or user source image is changed by this feature.

## Results

- Workspace tests: **320 passed**, 42 opt-in tests excluded (GUI binary 284,
  CLI 4, CLI integration 5, core 27).
- Formatting, diff checks, Clippy with warnings denied and release build passed.
- Desktop/resource checks: **9 tests passed**, metadata and **17 embedded
  resources** matched their packaged paths and bytes.
- Final native/GTK matrix: **10/10 passed**, five cases at 1x and 2x with the
  real pointer driver: GIF native Kitty rendering; GIF import/still notice,
  Undo/Redo and re-edit after deleting the original; existing image import,
  coalescing, cancel, export and restart; GIF Play/Pause/seek/static fallback;
  existing static pixel dialog guards, geometry and protocol exports.
  Report: `/tmp/termimochi-regression-lvkevdho/results.json`.
- Additional GIF-dialog-only matrix: **2/2 passed** after adding a chunked PNG
  transport unit test. Report:
  `/tmp/termimochi-regression-izwbpjfa/results.json`.
  The 2x screenshot was visually inspected at
  `/tmp/termimochi-gif-qa.Zn3tVv/gif-dialog.png`; controls and labels fit.
- Core tests cover offsets, previous/background disposal, fixed crop, stable
  removal, timing/catch-up, transparency, source persistence, safe ANSI fallback,
  static exports, malformed/truncated/trailing data and decode budgets. A noisy
  128x128 two-frame fixture verifies multi-packet 4096-byte base64 transfer,
  continuation headers, frame boundaries and exact decoded PNG pixels.

## Native rendering regression

The initial `kitty-icat` implementation passed a movement-only screenshot check
but manual review found transparent-frame trails in real **Kitty 0.45.0** with
**Fastfetch 2.57.1**. That initial result is not evidence of correct transparency.
The export now generates full-frame replacement commands directly, and the native
test additionally asserts that only one moving red rectangle is visible. Both
final scale runs passed the stronger check; consecutive 1x captures were also
inspected and showed no old rectangles.

The native test runs a trusted fixed custom field in its own Kitty window on
Xvfb, without user Kitty/shell configuration. Fastfetch output is forwarded to
the real terminal so geometry queries receive actual Kitty responses. Output,
runtime and subprocess lifetime are bounded. Python Pillow is a test-only
dependency for the residue check. User desktop windows are left untouched.

## Limits and unverified paths

GIF limits: 16 MiB source and encoded GIF; 1024x1024 canvas; 120 frames;
64 MiB aggregate decoded frame pixels; 60 seconds per cycle. Delays below 20 ms
are normalized. GIF decoding/preparation is serialized, with cooperative
frame-boundary deadlines, not a hard sandbox timeout. The Kitty stream is limited
to 32 MiB. GIF palette quantization and binary transparency remain visible format
limitations; all-transparent results are rejected. Imported settings, including
commands, are preserved during export but never executed by export itself.

Native Wayland graphics, other Kitty-compatible terminals, animated Sixel,
remote/SSH sessions and the desktop folder portal were not verified. The GTK
tests call the post-selection export method using temporary local folders.
APNG, animated WebP and arbitrary raw protocol import remain unsupported.

The release was installed with `scripts/install.sh install --no-build`; build
and installed executable SHA-256 both equal
`033c8deec017f8647a80d59e69d51c36ebe742d8076a0f03d58fe9366d1e67ed`.
No commit or push was made for this step.

## Follow-up functional verification

At the user's request, the current installed-version source was tested again
without changing application code or user terminal configuration:

- **320 ordinary workspace tests passed**, including PNG/JPEG/WebP conversion,
  GIF compositing and limits, SVG validation, background removal, source storage,
  pixel bundle generation and Apply/Restore safety.
- **34/34 opt-in native/GTK cases passed** (17 cases at both 1x and 2x).
  Report: `/tmp/termimochi-regression-vghbnc9b/results.json`.
  Coverage includes actual Kitty GIF playback and transparent-frame residue;
  native Fastfetch Kitty/Sixel protocol emission; SVG sandbox success/failure;
  large ANSI output; saved source recovery after deleting originals; GIF
  re-edit/Undo/Redo; background controls, invalid colors and empty-image refusal;
  cancel/coalescing/export/restart; wheel protection; animation Play/Pause/seek;
  export dialog guards and stale drafts; Apply/load/restart/conflicts and preview
  synchronization; Greeting Inspect navigation; output action reachability; and
  extreme geometry, long artwork and rapid cross-module navigation/recovery.
- **One additional real Fastfetch run-once test passed**, covering exactly-once
  execution, literal unusual paths, ignored shell startup hooks and visible
  failure status. The external Ptyxis launch UI was not re-tested in this run.
- Clippy with warnings denied, formatting, diff checks, **9 desktop tests** and
  **17 embedded resource** validations passed. Installed and release executable
  hashes still match the SHA-256 recorded above.
- Consecutive native Kitty screenshots from this run were visually inspected:
  the moving rectangle changed position without leaving previous copies.

No functional failure was observed in this coverage. Xvfb logged lack of DRI3
acceleration and the isolated session's optional secret-service portal timed out;
the affected tests still passed. These are test-environment warnings, not proof
of a desktop portal working. File/folder chooser portal interaction, native
Wayland rendering and visual Sixel rendering remain unverified. Sixel's check
proves protocol emission only. Normal Apply remains static ANSI by design;
animated export does not install itself into shell startup.

Only this QA record was updated; no application changes or push were made.
