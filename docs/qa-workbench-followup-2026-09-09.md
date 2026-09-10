# Cross-module color navigation and save workflow QA

## Changes

- Greeting color roles include individual field names and contents, in source
  order. Repeated native modules keep distinct indices. Proven palette bindings
  reuse the existing picker and palette undo; literal RGB, complex SGR and
  ambiguous native inherited colors remain source-owned.
- Your Starship lists reviewed module/style fields. Edit source opens the exact
  existing editor without changing source text or the terminal palette.
- Greeting source navigation opens the matching mapped field popover. Weak
  button references distinguish hidden custom/native lists with shared indices.
- Prompt Ctrl+S and Ctrl+Shift+S save workspaces; Ctrl+O opens a workspace.
  Explicit Apply / Export retain the backed-up Starship write workflow. The
  Open tooltip now refreshes on every module switch, including Palette/Prompt.
- Native JSONC is parsed once per color-role list, not once per field.

## Verification

- 313 ordinary workspace tests passed; 37 opt-in tests excluded.
- Formatting, Clippy with warnings denied, release build and diff checks passed.
- Desktop/resource mutation tests: 9 passed. Metadata and all 17 embedded
  resource paths/bytes and packaged copies validated.
- Four targeted GUI/native regressions passed at 1x and 2x (8/8), with both
  real-pointer and Starship-copy scenarios enabled. They cover shared-color
  ownership, imported RGB preservation, exact source navigation, field editing,
  bottom-bar routing, minimum-window geometry, local workspace saving, real
  Starship rendering, cancel/save/restore/conflict handling and Inspect.
- Color navigation was rerun at both scales after the JSONC parsing optimization
  (2/2). Source selection and navigation leave the original documents unchanged.
- The initial 6/8 run exposed a stale Open tooltip; the production signal wiring
  was corrected and the complete targeted matrix passed on rerun.

Final matrix: `/tmp/termimochi-regression-3qgiya71/results.json`.
Final color rerun: `/tmp/termimochi-regression-h8jmf3wj/results.json`.
Initial failure evidence: `/tmp/termimochi-regression-8g9rphem/results.json`.

Dedicated Xvfb displays, temporary XDG state and an in-memory settings backend
isolate all write tests from the user's terminal configuration. This was a
targeted follow-up, not a rerun of every opt-in regression. Native Wayland and
hardware touchpads were not tested. Arbitrary imported RGB styles are not edited
by the shared palette picker; they remain in the existing source editors.
No commit or push was made during this follow-up.
