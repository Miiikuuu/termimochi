# Workbench consolidation QA — 2026-09-09

## Scope

Persistent contextual output bar, removal of duplicate module output controls,
preview-local toasts, and shared palette targets for Prompt Designer / Greeting.
Existing SVG and Inspect work remains included; no terminal configuration was
applied to the user's profile during verification.

## Checks

- Ordinary workspace tests: 311 passed; 37 opt-in tests excluded.
- Formatting, Clippy with warnings denied, release build and diff whitespace
  checks passed.
- Desktop/resource mutation checks: 9 passed. Metadata, all 17 embedded
  resources (paths and bytes), and packaged resource copies validated.
- Isolated install/uninstall test passed, including paths with spaces.
- All 37 native/GTK regressions are covered at both 1x and 2x: the pinned full
  matrix passed 70/74 cases; its four stale-fixture failures passed in the
  corrected 2/2 hint and 2/2 Inspect reruns. No production fix was needed for
  those failures. The matrix includes artwork, SVG, wheel input, source
  persistence, conflict handling, terminal scrolling and extreme settings.
- Output bar regression passed at 1x/2x: all five module action routes, secondary
  imports/exports/recovery, scrolling, workspace Save and 800 × 560 geometry.
- Shared color regression passed at 1x/2x: palette ownership, undo, unchanged
  Prompt/Greeting documents, independent imported RGB fallback, and returning
  to terminal swatches. A unit test checks all six normal/bright prompt bindings.
- Preview-hint and real-pointer Inspect regressions passed at 1x/2x after
  updating stale test assumptions: ToastOverlay no longer contains the main
  divider, and the output bar stays visible even without a Starship document.
  Starship's write action, rather than the entire bar, is disabled in that case.

## Evidence and limits

Main native/GTK matrix: `/tmp/termimochi-regression-fhtchg9d/results.json`.
Corrected hint fixture: `/tmp/termimochi-regression-5ev6t4q9/results.json`.
Corrected Inspect fixture: `/tmp/termimochi-regression-bnrz7lgd/results.json`.
Screenshots: `/tmp/termimochi-consolidated-final` (latest scale).

Installed release matches the built binary:
`da3a019497674d953684dbbf50ffbd3def171acd04dc365fbc4fe7152aa16a28`.

Tests use dedicated Xvfb displays, temporary XDG directories and an in-memory
settings backend. Native Wayland, physical touchpads and real desktop theme
activation were not tested. Source-specific RGB editing remains in the source
editors; this iteration centralizes shared palette colors, not artwork pixels
or arbitrary imported styles. No commit or push was made.
