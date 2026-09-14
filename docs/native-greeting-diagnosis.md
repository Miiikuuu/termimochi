# Complex native Kitty Greeting: diagnosed, not repaired

Follow-up: the diagnosis below is retained as historical evidence. New controlled
sessions now have a [width-aware native output repair and acceptance card](native-greeting-reflow-qa.md).

2026-09-13; base `38ad24d`, plus a diagnostic-only ignored GTK test and pointer
driver. No production behavior, theme values, daily configuration, default
terminal or startup files were changed. No commit/push/install was performed.

## Conclusion

The reported symptom is reproducible: after expanding native Kitty, system
information remains truncated even though the right side is now empty.

The cause in the tested environment is **output-time width plus disabled
automatic wrapping**, not missing machine information or the Full renderer:

1. The native session materializes a safe Fastfetch configuration and runs
   Fastfetch directly. It does not render through Full's text clipping/cache.
2. The original saved theme uses 16 pt and a left-hand 32-column GIF. The
   generated logo reserves 32 columns plus a 3-column gap. The ordinary native
   pane has only 46 actual terminal columns, leaving 11 for keys and values.
3. Installed Fastfetch **2.57.1** defaults `disableLinewrap` to true. Our generated
   native configuration sets `display.pipe=false` but does not set a wrapping
   policy. The captured byte stream contains `ESC[?7l`, complete CPU/GPU values,
   and later `ESC[?7h`. With wrapping disabled, overflow repeatedly writes the
   last terminal cell; it is not a hidden complete line waiting off-screen.
4. Expanding the host changes the actual grid to 80 columns but does not rerun
   Greeting. The old truncated rows persist. Re-running at 80 columns restores
   more text, but long CPU output still exceeds the remaining 45 columns.
5. At 142 columns, the same configuration renders the complete CPU/GPU values
   visibly. Same theme, no smaller font/GIF and no removed modules.
6. A one-option control at the original 46 columns (`--disable-linewrap false`)
   removes the wrap-disable control and retains the visible value tails, but
   continuations begin at the terminal's left edge instead of staying in the
   information column. This is **not an acceptable completed layout fix**.

This explains the “space is available, yet information is cut” report without
assuming a Casilda texture crop or a race. A delayed-start problem is not needed
to produce it: the controlled narrow rerun occurs after the surface has settled
and reproduces the same truncation. Other startup races are not globally ruled out.

Responsibility remains in TermiMochi's native output adaptation: a fixed left-logo
composition relies on an external default wrapping policy and does not account
for the available information width. It is not a claim that Kitty incorrectly
implements terminal wrapping or that the user's image is defective.

The original source's `display` options are filtered by the existing safe
projection (`fastfetch_document::preview_config_with_logo`); simply adding a
setting to the source is not sufficient unless the controlled adapter carries
an explicit safe policy through. The original fixture does not explicitly set
`disableLinewrap`, so this particular failure does not require a dropped override.

Upstream's [2.57.1 display options](https://github.com/fastfetch-cli/fastfetch/blob/2.57.1/src/options/display.c)
define the setting; the installed `fastfetch --help disable-linewrap` reports
`Default: true`. Later [2.65.1 release notes](https://github.com/fastfetch-cli/fastfetch/releases/tag/2.65.1)
change the default and explicitly note potential overlap with image logos.
No dependency was upgraded; upgrading alone is not certified as a repair.

## Actual evidence

Fixture: the private byte-identical **小猫测试** design at
`target/qa/reachability/reproduction.termimochi-design.json`, SHA256
`283f0aaf3c14bfcb01826eedd834054fa3abcec0b91f75a167c474836522b13e`.
The test rechecks its unchanged bytes. Its actual imported multi-field module
list and GIF are used, not the simpler moving-block native fixture.

Actual Kitty **0.45.0**, private native configuration and D-Bus; Xvfb `:97` outer
GTK/X11, Casilda nested Wayland, SHM/software, 1×. The saved theme owns size but
not family; the native controlled baseline is **DejaVuSansMono 16**. This is
not the Caskaydia/font-reference fixture used for the
earlier Full screenshots. Native grid counts below come from `stty size`, not
Full's reference 120×36. Generated logo occupancy is 32×14 cells.

All artifacts below are local ignored QA material, not uploaded CI evidence.
Root: `target/qa/native-preview/termimochi-regression-native-74facc5v/`.

| Native observation | Actual rows × columns | Result |
| --- | --- | --- |
| Stable narrow rerun | 14 × 46 | Truncated values; raw stream contains full tails and wrap-disable control |
| Expand without rerun | Host expanded after the 46-column run | Old truncated rows persist beside empty right-hand space |
| Expanded rerun | 14 × 80 | More characters visible; CPU line still overflows |
| Wide rerun | 26 × 142 | Complete CPU/GPU values visible |
| Narrow rerun, wrap enabled | 14 × 46 | Tails survive, but continuations spill into the logo column |

- [Startup](../target/qa/native-preview/termimochi-regression-native-74facc5v/1x-native_complex_greeting_diagnosis/cache/startup-narrow.png).
- [Expanded, old truncated rows](../target/qa/native-preview/termimochi-regression-native-74facc5v/1x-native_complex_greeting_diagnosis/cache/expanded-without-rerun.png).
- [Same configuration rerun wide](../target/qa/native-preview/termimochi-regression-native-74facc5v/1x-native_complex_greeting_diagnosis/cache/rerun-wide.png).
- [Wrapping-only control, poor alignment](../target/qa/native-preview/termimochi-regression-native-74facc5v/1x-native_complex_greeting_diagnosis/cache/narrow-wrap-enabled.png).
- [Narrow raw PTY output](../target/qa/native-preview/termimochi-regression-native-74facc5v/1x-native_complex_greeting_diagnosis/cache/rerun-narrow.typescript),
  [wide raw output](../target/qa/native-preview/termimochi-regression-native-74facc5v/1x-native_complex_greeting_diagnosis/cache/rerun-wide.typescript),
  [wrap-enabled raw output](../target/qa/native-preview/termimochi-regression-native-74facc5v/1x-native_complex_greeting_diagnosis/cache/narrow-wrap-enabled.typescript).
- [Generated Fastfetch configuration](../target/qa/native-preview/termimochi-regression-native-74facc5v/1x-native_complex_greeting_diagnosis/cache/fastfetch.jsonc),
  [capture-run result](../target/qa/native-preview/termimochi-regression-native-74facc5v/results.json).

The original startup capture uses the production native entry unchanged. For
diagnostic reruns, a real pointer focuses only the isolated native window and
pastes a fixed QA command. `script` captures the real PTY stream while forwarding
native output and terminal responses; its recorded grid agrees with `stty`.
The existing remote-control allowlist is not widened or bypassed. Screenshots
were visually inspected separately from raw byte assertions.

Two runs completed successfully: the initial width-only probe (`4rmj9_mr`)
and the final probe including the wrapping control (`74facc5v`). The runner's
`passed: true` means the **diagnostic capture/guards completed**, not that the
known visual defect has passed acceptance. The raw-output assertion checks the
expected wrap-control difference for installed Fastfetch 2.57.1.

Diagnostic code checks: enabled-feature workspace/all-targets Clippy with
`-D warnings` passed; Rust formatting, Python driver syntax, local evidence links
and `git diff --check` passed. Both owned native sessions/displays were stopped
by the runner. These checks are separate from visual correctness.

## Reproduce / next repair boundary

With the existing private dependencies and fixture:

```bash
TERMIMOCHI_REACHABILITY_THEME="$PWD/target/qa/reachability/reproduction.termimochi-design.json" \
FONTCONFIG_FILE="$PWD/target/qa/reachability/fonts.conf" \
python3 scripts/test-native-preview.py --scale 1 \
  --filter native_complex_greeting_diagnosis --display :97
```

The runner owns a new display and private HOME/XDG/D-Bus. Do not reuse a daily
display. The probe changes only QA window size and a one-run command-line flag;
it never saves those changes into the original design.

Next repair needs an explicit width-aware native Greeting layout/wrapping
policy, with same-theme narrow/wide and resize verification. Merely delaying
startup, enabling wrapping, shrinking the user's font/image, or hiding fields
does not meet that requirement. Do not inject a refresh command into an
arbitrary running native program on resize.

**Still unresolved:** production fix; broader GIF placement/scroll acceptance;
Ptyxis comparison; standalone daily Kitty versus nested-host comparison; Wayland
desktop/GPU/fractional scaling, 2×, and external human acceptance. The current
probe diagnoses the reproduced horizontal information loss, not every item in
the native limitations ledger. No release-wide regression or new Release build
was performed for this diagnostic-only change.
