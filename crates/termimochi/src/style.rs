pub const BASE_CSS: &str = r#"
.termimochi-window,
.workbench-toolbar {
  color: #1e2529;
  background: #ffffff;
}

.workbench-header {
  min-height: 50px;
  padding: 0 8px;
  color: #1e2529;
  background: #ffffff;
  border-bottom: none;
  box-shadow: none;
}

.brand-title-name {
  color: #1e2529;
  font-size: 1rem;
  font-weight: 700;
}

.brand-mark {
  padding: 0;
  color: #242521;
  background: transparent;
  box-shadow: none;
}

.header-actions {
  margin-right: 2px;
}

.workbench-header button.tool-button,
.workbench-header menubutton.tool-menu > button {
  min-height: 28px;
  min-width: 28px;
  padding: 3px 7px;
  border: 1px solid transparent;
  border-radius: 4px;
  color: #30383e;
  background: transparent;
  box-shadow: none;
}

.workbench-header button.open-button {
  min-width: 30px;
  min-height: 28px;
  padding: 3px 6px;
  border-radius: 5px;
  color: #515b5e;
}

.workbench-header button.tool-button:hover,
.workbench-header menubutton.tool-menu > button:hover {
  background: #eef1f3;
  border-color: transparent;
}

.workbench-header button.tool-button:active,
.workbench-header menubutton.tool-menu > button:active {
  background: #e5e9ec;
}

.workbench-header button.tool-button:focus-visible,
.workbench-header menubutton.tool-menu > button:focus-visible,
.variant-switch button:focus-visible {
  outline: 2px solid #505b63;
  outline-offset: 1px;
}

.workbench-header menubutton.overflow-menu > button {
  min-width: 30px;
  padding: 3px 6px;
}

.workbench-header menubutton.save-menu > button {
  min-width: 30px;
  padding: 3px 6px;
  border-radius: 5px;
  color: #515b5e;
}

.workbench-header menubutton.save-menu > button:checked {
  color: #1e2529;
  background: #e9edef;
}

.workbench-header menubutton.save-menu.save-ready > button {
  color: #303940;
}

.workbench-header menubutton.save-menu.save-ready > button:hover {
  background: alpha(#505b63, 0.10);
}

.workbench-header menubutton.save-menu.save-ready > button:active {
  background: alpha(#505b63, 0.17);
}

.workbench-header menubutton.save-menu.save-ready > button:checked {
  color: #303940;
  background: #e9edef;
}

popover.save-popover > contents {
  padding: 5px;
  border: 1px solid alpha(#1e2529, 0.07);
  border-radius: 10px;
  background: #ffffff;
  box-shadow: 0 12px 32px alpha(#0f1720, 0.14);
}

popover.save-popover modelbutton {
  min-height: 30px;
  padding: 5px 9px;
  border-radius: 6px;
}

popover.save-popover modelbutton:hover {
  background: #f1f3f5;
}

.variant-switch {
  padding: 2px;
  border-radius: 999px;
  background: #eef1f3;
}

.variant-switch button {
  min-width: 38px;
  min-height: 24px;
  padding: 0 8px;
  border: none;
  border-radius: 999px;
  color: #515b5e;
  background: transparent;
  box-shadow: none;
  font-size: 0.78rem;
  font-weight: 650;
}

.variant-switch button:not(:checked):hover {
  background: alpha(#1e2529, 0.07);
}

.variant-switch button:not(:checked):active {
  background: alpha(#1e2529, 0.13);
}

.variant-switch button:checked {
  color: white;
  background: #1e2529;
  box-shadow: 0 1px 2px alpha(#141a20, 0.14);
}

.variant-switch button:checked:hover {
  background: #30383e;
}

.variant-switch button:checked:active {
  background: #141a20;
}

.variant-switch button:disabled {
  color: #929896;
  background: transparent;
  box-shadow: none;
}

.workbench-split > separator {
  min-width: 4px;
  border: none;
  background: transparent;
  box-shadow: none;
}

.editor-workspace,
.editor-module-stack,
.editor-scroll,
.typography-scroll,
.layout-scroll,
.prompt-scroll,
.termimochi-editor,
.termimochi-typography-pane,
.termimochi-layout-pane,
.termimochi-prompt-pane {
  background: #f5f6f7;
}

.activity-rail {
  min-width: 48px;
  padding: 12px 0;
  background: #eef1f3;
}

.activity-item {
  min-width: 48px;
  min-height: 36px;
}

button.activity-button {
  min-width: 36px;
  min-height: 36px;
  padding: 0;
  color: #697174;
  border: none;
  border-radius: 9px;
  background: transparent;
  box-shadow: none;
}

button.activity-button:hover {
  color: #414b50;
  background: alpha(#1e2529, 0.06);
}

button.activity-button:checked {
  color: #1e2529;
  background: transparent;
  box-shadow: none;
}

button.activity-button:checked:hover {
  background: alpha(#1e2529, 0.06);
}

button.activity-button:focus-visible {
  outline: 2px solid #505b63;
  outline-offset: -2px;
}

.activity-indicator {
  min-width: 2px;
  min-height: 20px;
  margin-left: 2px;
  border-radius: 999px;
  background: #505b63;
}

.preview-scroll,
.termimochi-preview-pane {
  background: #ffffff;
}

.preview-heading {
  color: #1e2529;
  font-size: 1.08rem;
  font-weight: 700;
}

.preview-title-row {
  min-height: 28px;
}

.preview-source-button > button {
  min-width: 28px;
  min-height: 28px;
  padding: 2px;
  color: #56616a;
  background: transparent;
  border: none;
  box-shadow: none;
  border-radius: 999px;
}

.preview-source-button > button:hover {
  color: #20272d;
  background: #f0f2f3;
}

.preview-source-detail {
  font-size: 0.88rem;
  color: #59636a;
}

.section-heading {
  color: #30383e;
  font-size: 0.92rem;
  font-weight: 700;
}

.palette-identity {
  padding: 0 0 4px;
  border-bottom: none;
}

.palette-identity .preview-heading {
  min-width: 58px;
}

.identity-label {
  color: #515b5e;
  font-size: 0.84rem;
  font-weight: 600;
}

.ansi-palette,
.professional-picker {
  padding: 0;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.palette-label {
  color: #414b50;
  font-size: 0.82rem;
  font-weight: 600;
}

.palette-row-label {
  min-width: 32px;
  color: #586163;
  font-size: 0.78rem;
  font-weight: 600;
}

button.palette-swatch {
  padding: 2px;
  border: 1px solid transparent;
  border-radius: 4px;
  background: transparent;
  box-shadow: none;
}

button.palette-swatch:hover {
  border-color: transparent;
  background: transparent;
}

button.palette-swatch:focus-visible {
  outline: 2px solid #505b63;
  outline-offset: 1px;
}

button.palette-swatch:checked,
button.palette-swatch:active {
  border-color: transparent;
  background: transparent;
  box-shadow: none;
}

button.basic-swatch-button {
  min-width: 50px;
  min-height: 38px;
}

button.ansi-swatch-button {
  min-width: 28px;
  min-height: 28px;
  border-radius: 3px;
}

.swatch-pin {
  opacity: 0;
}

button.palette-swatch:hover .swatch-pin {
  opacity: 0.28;
}

button.palette-swatch:checked .swatch-pin {
  opacity: 0.92;
}

.professional-picker entry,
.palette-name-entry {
  min-height: 30px;
  padding: 2px 7px;
  color: #1e2529;
  background: #ffffff;
  border: 1px solid transparent;
  border-radius: 3px;
  box-shadow: none;
}

.professional-picker entry:focus,
.palette-name-entry:focus {
  border-color: #505b63;
  box-shadow: 0 0 0 1px #505b63;
}

.professional-picker entry.error,
.palette-name-entry.error {
  border-color: #a33c47;
  box-shadow: 0 0 0 1px #a33c47;
}

.picker-field-label {
  color: #515b5e;
  font-size: 0.76rem;
  font-weight: 700;
}

.professional-picker entry {
  font-feature-settings: "tnum";
}

.rgb-entry {
  min-width: 40px;
}

.picker-square:focus {
  outline: 2px solid #505b63;
  outline-offset: 2px;
}

scale.picker-hue-scale:focus {
  outline: 2px solid #505b63;
  outline-offset: 1px;
  border-radius: 4px;
}

scale.picker-hue-scale {
  padding: 0 5px;
}

scale.picker-hue-scale trough {
  min-width: 14px;
  border: none;
  border-radius: 3px;
  background: linear-gradient(to bottom,
    #ff0000 0%, #ffff00 16.67%, #00ff00 33.33%,
    #00ffff 50%, #0000ff 66.67%, #ff00ff 83.33%, #ff0000 100%);
}

scale.picker-hue-scale highlight,
scale.picker-hue-scale fill {
  background: transparent;
}

scale.picker-hue-scale slider {
  min-width: 22px;
  min-height: 4px;
  margin: -3px -4px;
  border: 2px solid white;
  border-radius: 2px;
  background: #ffffff;
  box-shadow: 0 0 0 1px alpha(#1e2529, 0.45),
              0 1px 3px alpha(black, 0.24);
}

.terminal-shell {
  padding: 10px 11px 11px;
  border: none;
  border-radius: 7px;
  box-shadow: 0 2px 6px alpha(#0f1720, 0.16),
              0 14px 34px alpha(#0f1720, 0.12);
}

.terminal-shell.preview-active {
  box-shadow: 0 3px 8px alpha(#0f1720, 0.18),
              0 18px 42px alpha(#0f1720, 0.15),
              0 0 18px alpha(#505b63, 0.16);
}

/* Full's canvas owns the shadow and rounded clipping. No focus glow is
 * allowed to masquerade as a change in the target terminal's appearance. */
.terminal-shell.sample-window, .terminal-shell.sample-window.preview-active {
  padding: 0;
  border-radius: 10px;
  box-shadow: none;
}

.vte-preview {
  padding: 0;
}

.sample-command, .sample-command:focus-within {
  min-height: 0;
  min-width: 0;
  padding: 0;
  margin: 0;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
  outline: none;
}
.sample-command text { min-height: 0; padding: 0; margin: 0; }

.preview-inspected {
  box-shadow: 0 0 0 2px alpha(#526b73, 0.32);
  transition: box-shadow 180ms ease-out;
}

.preview-inspect-toggle {
  font-family: sans-serif;
  min-height: 26px;
  padding: 2px 11px;
  border: none;
  border-radius: 999px;
  box-shadow: none;
  background: #edf0f1;
  color: #556166;
  font-size: 0.82rem;
  font-weight: 600;
}

.preview-inspect-toggle:hover {
  background: #e2e7e9;
}

.preview-inspect-toggle:checked {
  background: #20292d;
  color: white;
}

.preview-inspect-hint {
  font-family: sans-serif;
  padding: 6px 10px;
  border-radius: 7px;
  background: #20292d;
  color: #ffffff;
  font-size: 0.8rem;
  font-weight: 500;
  box-shadow: 0 3px 9px alpha(black, 0.16);
}

.preview-fit-hint {
  font-family: sans-serif;
  font-size: 0.8rem;
  font-weight: 400;
  color: #ffffff;
  background: alpha(#20292d, 0.94);
  padding: 7px 11px;
  border-radius: 7px;
  box-shadow: 0 2px 8px alpha(black, 0.12);
}

.greeting-field-button > button {
  padding: 5px 6px;
  border-radius: 6px;
  background: transparent;
  box-shadow: none;
  border: none;
}

.greeting-field-button > button:hover {
  background: alpha(#20292d, 0.06);
}

.vte-preview:focus-visible {
  outline: 2px solid #505b63;
  outline-offset: -2px;
}

.terminal-header {
  min-height: 28px;
}

.terminal-chrome {
  font-family: monospace;
  font-size: 0.86rem;
  font-weight: 700;
  opacity: 0.86;
}

.terminal-tab {
  min-height: 24px;
}

dropdown.preview-scenario > button {
  min-height: 26px;
  padding: 1px 7px;
  border: 1px solid transparent;
  border-radius: 3px;
  color: inherit;
  background: transparent;
  box-shadow: none;
}

dropdown.preview-scenario > button:hover {
  background: alpha(currentColor, 0.05);
}

dropdown.preview-scenario > button:active,
dropdown.preview-scenario > button:checked {
  background: alpha(currentColor, 0.08);
}

dropdown.preview-scenario > button:focus-visible {
  outline: 2px solid #505b63;
  outline-offset: 1px;
}

.preview-scenario {
  font-family: sans-serif;
  font-size: 0.82rem;
  font-weight: 600;
}

.terminal-viewport {
  border: none;
  background: transparent;
  box-shadow: none;
}

/* AdwToolbarView's undershoot must not divide the terminal's own surface. */
.terminal-viewport > undershoot,
.terminal-viewport > overshoot {
  background: none;
  border: none;
  box-shadow: none;
}

.typography-header {
  min-height: 34px;
  margin-bottom: 2px;
}

.typography-title {
  padding: 3px 0 2px;
}

.layout-title {
  min-height: 34px;
  padding: 3px 0 2px;
}

.prompt-header {
  min-height: 34px;
}

.prompt-title {
  padding: 3px 0 2px;
}

menubutton.prompt-export > button {
  min-height: 28px;
  padding: 2px 10px;
  border: none;
  border-radius: 999px;
  color: #ffffff;
  background: #1e2529;
  box-shadow: 0 1px 2px alpha(#141a20, 0.14);
  font-size: 0.78rem;
  font-weight: 700;
}

menubutton.prompt-export > button:hover {
  background: #30383e;
}

menubutton.prompt-export > button:active {
  background: #141a20;
}

menubutton.prompt-export > button:focus-visible {
  outline: 2px solid #505b63;
  outline-offset: 2px;
}

.prompt-starter-row {
  min-height: 38px;
  padding: 2px 6px 2px 10px;
  border: none;
  border-radius: 8px;
  background: #ffffff;
}

.prompt-starter-title {
  color: #30383e;
  font-size: 0.82rem;
  font-weight: 650;
}

menubutton.prompt-starter-button > button {
  min-height: 28px;
  min-width: 104px;
  padding: 1px 8px;
  border: none;
  border-radius: 6px;
  color: #30383e;
  background: #eef1f3;
  box-shadow: none;
  font-size: 0.78rem;
  font-weight: 650;
}

menubutton.prompt-starter-button > button:hover,
menubutton.prompt-starter-button > button:checked {
  background: #e4e8ea;
}

menubutton.prompt-starter-button > button:focus-visible,
menubutton.prompt-add-button > button:focus-visible,
menubutton.prompt-module-more > button:focus-visible,
button.prompt-module-select:focus-visible,
button.prompt-menu-item:focus-visible,
button.prompt-catalog-item:focus-visible {
  outline: 2px solid #505b63;
  outline-offset: 1px;
}

popover.prompt-menu-popover > contents,
popover.prompt-catalog-popover > contents,
popover.greeting-field-popover > contents {
  padding: 0;
  border: none;
  border-radius: 10px;
  background: #ffffff;
  box-shadow: 0 8px 28px alpha(#141a20, 0.18);
}

button.prompt-menu-item,
button.prompt-catalog-item {
  min-height: 32px;
  padding: 2px 8px;
  border: none;
  border-radius: 6px;
  color: #30383e;
  background: transparent;
  box-shadow: none;
  font-size: 0.8rem;
  font-weight: 600;
}

button.prompt-menu-item:hover,
button.prompt-catalog-item:hover {
  background: #eef1f3;
}

button.prompt-menu-item.destructive-action {
  color: #94343d;
}

.prompt-catalog-category {
  margin-left: 7px;
  color: #737b7e;
  font-size: 0.7rem;
  font-weight: 750;
}

.prompt-catalog-sample {
  min-width: 82px;
  color: #697174;
  font-family: monospace;
  font-size: 0.72rem;
  font-weight: 600;
  font-feature-settings: "tnum";
}

.prompt-module-count {
  min-width: 20px;
  min-height: 20px;
  border-radius: 999px;
  color: #697174;
  background: #e8ebed;
  font-size: 0.72rem;
  font-weight: 750;
  font-feature-settings: "tnum";
}

menubutton.prompt-add-button > button {
  min-height: 28px;
  padding: 1px 8px;
  border: none;
  border-radius: 999px;
  color: #ffffff;
  background: #1e2529;
  box-shadow: none;
  font-size: 0.76rem;
  font-weight: 700;
}

menubutton.prompt-add-button > button:hover,
menubutton.prompt-add-button > button:checked {
  background: #30383e;
}

menubutton.prompt-add-button > button:disabled {
  color: #9aa0a2;
  background: #e8ebed;
}

.prompt-module-list {
  padding: 3px 4px;
  border: none;
  border-radius: 8px;
  background: #ffffff;
}

.prompt-module-row {
  min-height: 42px;
  border: none;
  border-radius: 6px;
  background: transparent;
}

.prompt-module-row:hover {
  background: #f4f6f7;
}

.prompt-module-row.selected {
  background: #e9eef1;
}

button.prompt-module-select {
  min-height: 42px;
  padding: 1px 4px 1px 7px;
  border: none;
  border-radius: 6px;
  color: #30383e;
  background: transparent;
  box-shadow: none;
}

button.prompt-module-select:hover,
button.prompt-module-select:active,
button.prompt-module-select:checked {
  background: transparent;
  box-shadow: none;
}

menubutton.prompt-module-more > button {
  min-width: 28px;
  min-height: 28px;
  padding: 0;
  margin-right: 3px;
  border: none;
  border-radius: 999px;
  color: #697174;
  background: transparent;
  box-shadow: none;
}

menubutton.prompt-module-more > button:hover,
menubutton.prompt-module-more > button:checked {
  color: #30383e;
  background: alpha(#1e2529, 0.07);
}

.prompt-module-sample {
  color: #697174;
  font-family: monospace;
  font-size: 0.74rem;
  font-weight: 650;
  font-feature-settings: "tnum";
}

.prompt-empty-state {
  min-height: 104px;
  border: none;
  border-radius: 8px;
  background: #ffffff;
}

.prompt-empty-mark {
  color: #697174;
  font-family: monospace;
  font-size: 1.05rem;
  font-weight: 750;
}

.prompt-empty-label {
  color: #515b5e;
  font-size: 0.8rem;
  font-weight: 650;
}

.prompt-properties {
  padding: 3px 5px;
}

.prompt-property-row {
  min-height: 38px;
}

dropdown.prompt-property-control > button {
  min-height: 30px;
  padding: 1px 7px;
  border: none;
  border-radius: 6px;
  color: #30383e;
  background: #eef1f3;
  box-shadow: none;
  font-size: 0.78rem;
  font-weight: 650;
}

dropdown.prompt-property-control > button:hover {
  background: #e4e8ea;
}

dropdown.prompt-property-control > button:focus-visible {
  outline: 2px solid #505b63;
  outline-offset: 1px;
}

.prompt-token-cyan {
  color: #276c78;
}

.prompt-token-blue {
  color: #315f89;
}

.prompt-token-magenta {
  color: #744383;
}

.prompt-token-yellow {
  color: #76560a;
}

.prompt-token-green {
  color: #2b6b4f;
}

.prompt-token-red {
  color: #9b3943;
}

.layout-group {
  min-width: 0;
}

.layout-group-title {
  margin-left: 2px;
  color: #515b5e;
  font-size: 0.82rem;
  font-weight: 700;
}

.layout-card {
  padding: 3px 5px;
  border: none;
  border-radius: 8px;
  background: #ffffff;
  box-shadow: none;
}

.layout-row {
  min-height: 36px;
  padding: 1px 5px;
}

.greeting-info-row {
  padding: 1px 4px;
  border-top: 2px solid transparent;
  border-bottom: 2px solid transparent;
}
.greeting-info-row:hover { background: #f6f7f8; }
.greeting-drop-before { border-top-color: #30383e; }
.greeting-drop-after { border-bottom-color: #30383e; }

.layout-row-label {
  color: #30383e;
  font-size: 0.82rem;
  font-weight: 600;
}

.typography-fields {
  margin-top: 2px;
}

.typography-field {
  min-width: 0;
}

.typography-field-label {
  margin-left: 2px;
  color: #697174;
  font-size: 0.76rem;
  font-weight: 650;
}

.nerd-status-label {
  color: #515b5e;
  font-size: 0.76rem;
  font-weight: 650;
  font-feature-settings: "tnum";
}

dropdown.typography-control > button,
spinbutton.typography-control {
  min-height: 32px;
  padding: 0;
  color: #30383e;
  background: #ffffff;
  border: 1px solid transparent;
  border-radius: 6px;
  box-shadow: none;
  font-size: 0.82rem;
  font-weight: 600;
}

dropdown.typography-control > button {
  padding: 1px 7px;
}

dropdown.layout-control > button,
spinbutton.layout-control {
  min-height: 30px;
  border-radius: 5px;
  background: #f5f6f7;
  font-feature-settings: "tnum";
}

dropdown.layout-control > button:hover,
spinbutton.layout-control:hover {
  background: #eceff1;
}

switch.layout-switch {
  color: #ffffff;
  border: none;
  border-radius: 999px;
  background: #dfe3e5;
  box-shadow: none;
}

switch.layout-switch:hover {
  background: #d5dade;
}

switch.layout-switch slider {
  background: #ffffff;
  box-shadow: 0 1px 3px alpha(#141a20, 0.18);
}

switch.layout-switch:checked {
  background: #1e2529;
}

switch.layout-switch:checked:hover {
  background: #30383e;
}

switch.layout-switch:focus-visible {
  outline: 2px solid #505b63;
  outline-offset: 1px;
}

.font-family-selection,
.font-family-option {
  font-size: 0.86rem;
  font-weight: 400;
}

.font-family-selection {
  padding: 0 1px;
}

.font-family-option {
  min-height: 28px;
  padding: 4px 8px;
}

dropdown.typography-control > button:hover,
spinbutton.typography-control:hover {
  background: #eef1f3;
}

dropdown.typography-control > button:focus-visible,
spinbutton.typography-control:focus-within {
  outline: 2px solid #505b63;
  outline-offset: 1px;
}

spinbutton.typography-control text {
  min-width: 34px;
  padding: 2px 4px 2px 7px;
  color: inherit;
  background: transparent;
}

spinbutton.typography-control button {
  min-width: 22px;
  min-height: 22px;
  padding: 0;
  color: #515b5e;
  background: transparent;
  border: none;
  border-radius: 4px;
  box-shadow: none;
}

spinbutton.typography-control button:hover {
  color: #1e2529;
  background: alpha(#1e2529, 0.08);
}

.quality-section {
  padding: 2px 0 0;
}

.quality-row {
  min-height: 28px;
  padding: 1px 0;
}

.quality-heading {
  color: #30383e;
  font-size: 0.9rem;
  font-weight: 700;
}

.inline-metric-label {
  color: #697174;
  font-size: 0.78rem;
  font-weight: 600;
}

.metric-value {
  color: #30383e;
  font-size: 0.9rem;
  font-weight: 700;
  font-feature-settings: "tnum";
}

.diagnostic-header {
  min-height: 24px;
}

.diagnostic-title {
  color: #30383e;
  font-size: 0.84rem;
  font-weight: 650;
}

.status-good {
  color: #28684e;
}

.status-warning {
  color: #7b5512;
}

.status-error {
  color: #a33c47;
}

.status-dot {
  border-radius: 999px;
}

.status-dot.status-good {
  background: #28684e;
}

.status-dot.status-warning {
  background: #7b5512;
}

.status-dot.status-error {
  background: #a33c47;
}

.metric-value.status-good {
  color: #30383e;
}

.diagnostic-log-surface {
  padding: 4px 8px;
  border: none;
  border-radius: 10px;
  background: #f7f8f9;
  box-shadow: none;
}

.diagnostic-scroll {
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.diagnostic-list {
  padding: 0;
  background: transparent;
}

.diagnostic-list > row {
  margin: 0;
  border: none;
  border-radius: 0;
  background: transparent;
}

.diagnostic-list > row:hover {
  background: transparent;
}

.diagnostic-row {
  min-height: 26px;
  padding: 3px 0;
}

.diagnostic-state {
  min-width: 16px;
  min-height: 16px;
  padding: 0;
  border-radius: 0;
  background: transparent;
}

.diagnostic-state.status-good {
  color: #1f5d43;
}

.diagnostic-state.status-warning {
  color: #6b470b;
}

.diagnostic-state.status-error {
  color: #8f303e;
}

.diagnostic-row-title {
  color: #4a5357;
  font-size: 0.82rem;
  font-weight: 600;
}

.diagnostic-action {
  min-height: 22px;
  padding: 2px 8px;
  border: none;
  border-radius: 8px;
  box-shadow: none;
  background: alpha(#526b73, 0.09);
  color: #34464e;
  font-size: 0.78rem;
}

.diagnostic-action:hover {
  background: alpha(#526b73, 0.17);
}

.status-note {
  color: #526b73;
}

.diagnostic-value {
  font-size: 0.82rem;
  font-weight: 700;
  font-feature-settings: "tnum";
}

.hex-entry {
  font-family: monospace;
  font-feature-settings: "tnum";
}

.palette-name-entry {
  min-width: 170px;
}

"#;

// GTK 4.16+ / libadwaita 1.6+ use variables for native control accents.
// Older supported GTK versions receive named colors and explicit widget CSS.
const MODERN_CHROME: &str = r#"
:root {
  --accent-bg-color: #343b41;
  --accent-fg-color: #ffffff;
  --accent-color: #343b41;
}
.editor-workspace {
  --accent-bg-color: #343b41;
  --accent-fg-color: #ffffff;
  --accent-color: #343b41;
  --window-bg-color: #ffffff;
  --window-fg-color: #252b31;
  --view-bg-color: #f4f5f6;
  --view-fg-color: #252b31;
  --popover-bg-color: #ffffff;
  --popover-fg-color: #252b31;
  --card-bg-color: #f4f5f6;
  --card-fg-color: #252b31;
  --scrollbar-outline-color: #ffffff;
  --error-color: #a33c47;
}
"#;

pub fn chrome_css(modern: bool) -> String {
    [
        BASE_CSS,
        include_str!("chrome.css"),
        if modern { MODERN_CHROME } else { "" },
    ]
    .concat()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::{gdk, glib, prelude::*};
    use std::str::FromStr;
    use termimochi_core::{Rgb, contrast_ratio};

    fn editor_color(name: &str) -> Rgb {
        let prefix = format!("@define-color {name} ");
        let value = include_str!("chrome.css")
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .unwrap()
            .trim_end_matches(';');
        Rgb::from_str(value).unwrap()
    }

    #[test]
    fn chrome_text_has_contrast_on_light_surfaces() {
        for surface in ["editor_bg", "editor_rail", "editor_field", "editor_hover"] {
            for text in ["editor_fg", "editor_muted", "editor_dim"] {
                let ratio = contrast_ratio(editor_color(text), editor_color(surface));
                assert!(ratio >= 4.5, "{text} on {surface}: {ratio}");
            }
        }
        assert_eq!(editor_color("editor_bg"), Rgb::from_str("#ffffff").unwrap());
    }

    #[test]
    fn modern_accents_have_an_explicit_legacy_fallback() {
        assert!(chrome_css(true).contains("--accent-bg-color: #343b41"));
        assert!(chrome_css(false).contains("@define-color accent_bg_color #343b41"));
        assert!(!chrome_css(false).contains("--accent-bg-color"));
        assert!(!include_str!("chrome.css").contains(".vte-preview"));
    }

    #[test]
    #[ignore = "requires a GTK display; run separately at 1x and 2x"]
    fn terminal_scroll_edges_and_idle_controls_blend_into_both_themes() {
        fn settle() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(180);
            let context = glib::MainContext::default();
            while std::time::Instant::now() < deadline {
                while context.pending() {
                    context.iteration(false);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        fn pixels(widget: &impl IsA<gtk::Widget>) -> (usize, usize, Vec<u8>) {
            let widget = widget.as_ref();
            let snapshot = gtk::Snapshot::new();
            gtk::WidgetPaintable::new(Some(widget)).snapshot(
                &snapshot,
                f64::from(widget.width()),
                f64::from(widget.height()),
            );
            let texture = widget.native().unwrap().renderer().unwrap().render_texture(
                snapshot.to_node().unwrap(),
                Some(&gtk::graphene::Rect::new(
                    0.0,
                    0.0,
                    widget.width() as f32,
                    widget.height() as f32,
                )),
            );
            let (width, height) = (texture.width() as usize, texture.height() as usize);
            let mut data = vec![0; width * height * 4];
            texture.download(&mut data, width * 4);
            (width, height, data)
        }

        adw::init().unwrap();
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
        let display = gdk::Display::default().unwrap();
        let provider = gtk::CssProvider::new();
        provider.load_from_data(&chrome_css(gtk::check_version(4, 16, 0).is_none()));
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let colors = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &display,
            &colors,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
        let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
        shell.set_css_classes(&["terminal-shell", "scroll-style-test"]);
        let selector = gtk::DropDown::from_strings(&["Edited", "Original"]);
        selector.add_css_class("preview-scenario");
        selector.set_halign(gtk::Align::End);
        shell.append(&selector);
        let canvas = gtk::DrawingArea::new();
        canvas.set_size_request(1000, 1000);
        canvas.add_css_class("scroll-style-canvas");
        let viewport = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::External)
            .vexpand(true)
            .child(&canvas)
            .build();
        viewport.add_css_class("terminal-viewport");
        shell.append(&viewport);
        // Match the workbench ancestor: AdwToolbarView's flat header enables
        // undershoot styles on descendant scrollers, including the terminal.
        let toolbar = adw::ToolbarView::new();
        toolbar.set_content(Some(&shell));
        toolbar.add_top_bar(&gtk::Label::new(Some("Live Preview")));
        let window = gtk::Window::builder()
            .title("TermiMochi scroll style test")
            .default_width(420)
            .default_height(320)
            .child(&toolbar)
            .build();
        window.present();
        gtk::prelude::GtkWindowExt::set_focus(&window, None::<&gtk::Widget>);
        for (background, foreground) in [("#f8f7f5", "#242424"), ("#17191c", "#eeeeee")] {
            colors.load_from_data(&format!(
                ".scroll-style-test, .scroll-style-canvas {{ background: {background}; color: {foreground}; }}"
            ));
            settle();
            assert!(viewport.hadjustment().upper() > viewport.hadjustment().page_size() + 100.0);
            assert!(viewport.vadjustment().upper() > viewport.vadjustment().page_size() + 100.0);
            for position in [0.0, 160.0, 1000.0] {
                viewport.hadjustment().set_value(position);
                viewport.vadjustment().set_value(position);
                settle();
                let (width, height, data) = pixels(&viewport);
                let pixel =
                    |x: usize, y: usize| &data[(y * width + x) * 4..(y * width + x + 1) * 4];
                let center = pixel(width / 2, height / 2);
                // GTK's undershoot/overshoot nodes must not add seams at any edge.
                for offset in [0, 1, 2, 4, 8] {
                    for (x, y) in [
                        (width / 2, offset),
                        (width / 2, height - 1 - offset),
                        (offset, height / 2),
                        (width - 1 - offset, height / 2),
                    ] {
                        assert_eq!(
                            pixel(x, y),
                            center,
                            "{background}, scroll {position}, edge ({x}, {y})"
                        );
                    }
                }
            }
            let (width, height, data) = pixels(&selector);
            // Padding beside the label/chevron should be entirely transparent.
            // Download uses native ARGB32 byte order; locate alpha portably.
            let alpha = if cfg!(target_endian = "little") { 3 } else { 0 };
            for x in [4, width - 5] {
                assert_eq!(
                    data[(height / 2 * width + x) * 4 + alpha],
                    0,
                    "idle dropdown has a solid background in {background}"
                );
            }
        }
        window.close();
        settle();
        gtk::style_context_remove_provider_for_display(&display, &colors);
        gtk::style_context_remove_provider_for_display(&display, &provider);
    }
}
