# Full Session design preview

Implemented against main `5a059342b8e35ba1b429ad074f599184c47b94b4`.
See [中文验收卡](full-session-acceptance-zh.md) and the local QA report below.

## What changes

The observation menu is **Full / Terminal / Prompt / Greeting**. Full is labelled
**Full Session** for accessibility and explicitly described as a design simulation.
All five left-side editors remain independent of this choice, including Inspect
navigation. Full combines the enabled Greeting, its information fields, the current
prompt, a non-executing `printf` specimen, and the existing prompt scenario/history
with its following prompt. Current Folder information is the existing safe local
snapshot, not a new shell running typed commands.
Full always observes current Prompt settings/output; the dedicated Prompt scene's
Original comparison and frozen starting prompt do not override that design.

Full does not introduce another settings document, terminal, or image decoder:

| Owner | Full consumes |
| --- | --- |
| Workspace/model | Palette variant, Typography, Layout and Prompt source/settings |
| GreetingSettings/Presentation | Enabled state, artwork/processed source, layout, fields, visual intent and Columns |
| Existing preview caches | Safe native Fastfetch fields, current folder and sanitized Starship output |
| Existing VTE/feed/Inspect map | All character art, fields, prompts, simulated output, cursor and semantic targets |
| Existing PresentationEditor | Prepared pixel source and frame textures; one shared play/pause state and playback timer |
| FullSession (view only) | Image cell rectangle, whole-canvas transform, Fit/Zoom and temporary transcript height |

Character Greeting follows the existing ANSI renderer. Pixel/GIF Greeting reserves
terminal cells through that same layout function, then positions the shared GTK
picture in those cells above the same VTE. The full transcript is collected through
the existing preview feed, measured for Unicode wrapping, sized, then sent to VTE
as one batch. This prevents resize-before-feed scrollback from separating the image
and text. It is a rendering projection, not another design model.

The existing local scroll canvas contains both the VTE and image. Fit scales both
axes to show the entire composition; 100%, 75% and 50% permit local scrolling.
Typography controls the actual VTE font and cell metrics used for pixel placement.
`Presentation.columns` still controls artwork occupancy (with the existing aspect
ratio/64-row bound). Greeting preview width, when explicitly set to 80/100/120,
selects the terminal grid; otherwise Full uses Layout Columns. Fit never changes
these values, font settings, editable source resolution or saved state. Full's
canvas grows beyond Layout Rows when needed to keep the opening transcript together.

Prepared textures are shared with Greeting. Navigation, Palette and Typography do
not re-decode unchanged image bytes; animation frames do not refeed the transcript.
Playback pauses when system animations are disabled. Hidden scenes do not advance
frames, and worker completions still use the existing generation checks. An image
preparation error has an explicit message and character fallback rather than a
blank supposedly verified picture. Minimal Card retains its existing no-art layout.

## State and safety

- New/reset sessions keep the existing Terminal default. Opening a saved workspace
  retains the existing initialization contract: enabled Greeting takes precedence,
  otherwise an included Prompt, otherwise Terminal. The previous Full selection is
  not serialized or restored. This avoids changing Open/Reset semantics.
- Fit and playback are session-only; they never enter Undo history or dirty Save.
  Within one running window, view preferences can remain available when revisiting
  Full; a new process begins with Fit and playback off.
- Full never writes protocol assessment or visual authorization. An unverified
  image can be designed here without being approved for real-terminal output.
- **Try Greeting** still means a temporary, real Greeting-only trial. GTK textures
  are not Kitty/Sixel protocol rendering and do not verify animation support.
- **Save** still saves the portable workspace only. Apply Scheme review, conflict
  checks, shared-pixel consent, backups and selective recovery are unchanged.
- No new apply path, startup hook, target adapter or external execution entrypoint.

## Phase 2: intentionally not implemented

`Try Full Scheme` would be inaccurate with the current adapters:

1. `pixel_trial::Terminal::command` launches the Greeting helper, and its Request
   contains protocol/ANSI/columns and asset hashes, not the whole Workspace.
2. The trial runner owns safe Fastfetch/protocol testing; it has no temporary
   Starship session integrating the chosen Designer/detached document, nor a
   no-startup-file interactive shell for a complete scheme.
3. Ptyxis palette activation targets a profile but appearance/font options include
   application-wide GSettings. Existing Apply transactions are durable/recoverable,
   not isolated temporary-window settings. Reusing them for a trial would change
   daily settings.
4. Kitty palette export exists, but the launcher does not build/pass a complete
   temporary Kitty configuration for palette, font, supported spacing/cursor/tab
   settings and Starship together. Xterm likewise lacks a complete scheme adapter.
5. Layout's preview-only properties need an explicit supported/unsupported map for
   each target; unsupported parts cannot be silently simulated and called real.

A future adapter must construct a complete isolated bundle and launch it without
loading daily configs/startup files, preserve existing capability/visual checks,
and report unsupported properties. No speculative adapter interface is added now.

## Local validation

Final run results and evidence paths are recorded in
[Full Session QA](qa-full-session-2026-09-11.md). These are local tests, not online CI
and not external-user usability feedback.
