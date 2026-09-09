# Inspect and artwork-editor wheel regression — 2026-09-09

- Ordinary workspace tests: 310 passed; 35 opt-in tests excluded.
- Formatting, Clippy with warnings denied, release build and diff whitespace checks passed.
- Image-editor real-wheel tests passed at 1x/2x: focused numeric inputs,
  dropdowns, background removal, tone/detail and crop controls retain their
  values while the sidebar scrolls. A direct slider click still edits.
- Greeting navigation passed at 1x/2x across horizontal, right-aligned, stacked
  and minimal-card layouts: actual VTE cells resolve to artwork/message/basic
  field controls; controls are revealed/highlighted; settings and transcript
  remain unchanged. Designer prompt navigation keeps the designer source.
- Original Inspect test passed at 1x/2x with `TERMIMOCHI_POINTER_TEST=1`:
  hover feedback, navigation, drag/double-click selection and disabling Inspect.
  Its first-row fixture now explicitly positions the independently scrolling
  canvas before sending pointer events.
- Resource mutation tests: 9 passed; metadata and all 17 embedded resources
  passed byte/path validation. Installed release hash matches the built binary.

Local logs: `/tmp/termimochi-regression-nb0v068p`,
`/tmp/termimochi-regression-3jj0l0s6`,
`/tmp/termimochi-regression-8tn_06xi`,
`/tmp/termimochi-regression-tl1v3kks`.

Native Fastfetch output without module provenance targets the field list.
Imported logos require a unique full-rectangle match against the retained source;
ambiguous, clipped or excessively repetitive matches fall back to the list.
These are intentional navigation limits, not inferred field identities.

Tests ran on a dedicated Xvfb display with temporary XDG state. Native Wayland
and hardware touchpad testing were not performed. No commit or push was made.
