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
  min-width: 0;
  min-height: 30px;
  padding: 3px 9px;
  border-radius: 5px;
  font-weight: 650;
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
  outline: 2px solid #42657a;
  outline-offset: 1px;
}

.workbench-header menubutton.overflow-menu > button {
  min-width: 30px;
  padding: 3px 6px;
}

.workbench-header splitbutton.save-split {
  min-height: 30px;
  color: #30383e;
  background: #f1f3f5;
  border: 1px solid alpha(#1e2529, 0.07);
  border-radius: 7px;
  box-shadow: 0 1px 2px alpha(#141a20, 0.10);
}

.workbench-header splitbutton.save-split > button,
.workbench-header splitbutton.save-split > menubutton > button {
  min-height: 28px;
  color: inherit;
  background: transparent;
  border: none;
  box-shadow: none;
}

.workbench-header splitbutton.save-split > button {
  padding: 3px 10px;
  border-radius: 6px 0 0 6px;
  font-weight: 700;
}

.workbench-header splitbutton.save-split > menubutton > button {
  min-width: 28px;
  padding: 3px 6px;
  border-radius: 0 6px 6px 0;
}

.workbench-header splitbutton.save-split > separator {
  min-width: 1px;
  margin: 6px 0;
  background: alpha(currentColor, 0.15);
}

.workbench-header splitbutton.save-split > button:not(:disabled):hover,
.workbench-header splitbutton.save-split > menubutton > button:not(:disabled):hover {
  background: alpha(#1e2529, 0.07);
}

.workbench-header splitbutton.save-split > button:not(:disabled):active,
.workbench-header splitbutton.save-split > menubutton > button:not(:disabled):active {
  background: alpha(#1e2529, 0.13);
}

.workbench-header splitbutton.save-split > button:disabled {
  color: #8f9694;
  background: transparent;
  opacity: 1;
}

.workbench-header splitbutton.save-split.save-ready {
  color: white;
  background: #42657a;
  border-color: alpha(#264654, 0.24);
  box-shadow: 0 1px 2px alpha(#141a20, 0.14);
}

.workbench-header splitbutton.save-split.save-ready > separator {
  background: alpha(white, 0.22);
}

.workbench-header splitbutton.save-split.save-ready > button:hover,
.workbench-header splitbutton.save-split.save-ready > menubutton > button:hover {
  background: alpha(white, 0.10);
}

.workbench-header splitbutton.save-split.save-ready > button:active,
.workbench-header splitbutton.save-split.save-ready > menubutton > button:active {
  background: alpha(#142630, 0.16);
}

.workbench-header splitbutton.save-split:disabled {
  color: #969c9b;
  background: #f5f6f7;
  border-color: transparent;
  box-shadow: none;
}

.workbench-header splitbutton.save-split > button:focus-visible,
.workbench-header splitbutton.save-split > menubutton > button:focus-visible {
  outline: none;
}

.workbench-header splitbutton.save-split:focus-within {
  outline: 2px solid #42657a;
  outline-offset: 2px;
}

.workbench-header splitbutton.save-split.save-ready:focus-within {
  outline-color: #274c61;
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

.editor-scroll,
.termimochi-editor {
  background: #f5f6f7;
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
  background: #eef1f3;
}

button.palette-swatch:focus {
  outline: 2px solid #42657a;
  outline-offset: 1px;
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

button.selected-swatch {
  border-color: white;
  box-shadow: 0 0 0 2px #42657a,
              inset 0 0 0 1px alpha(#1e2529, 0.18);
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
  border-color: #42657a;
  box-shadow: 0 0 0 1px #42657a;
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
  outline: 2px solid #42657a;
  outline-offset: 2px;
}

scale.picker-hue-scale:focus {
  outline: 2px solid #42657a;
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

.terminal-header {
  min-height: 28px;
}

.terminal-chrome {
  font-family: monospace;
  font-size: 0.86rem;
  font-weight: 700;
  opacity: 0.86;
}

dropdown.preview-scenario button {
  min-height: 26px;
  padding: 1px 7px;
  border: 1px solid transparent;
  border-radius: 3px;
  color: inherit;
  background: alpha(currentColor, 0.10);
  box-shadow: none;
}

dropdown.preview-scenario button:hover {
  background: alpha(currentColor, 0.16);
}

dropdown.preview-scenario button:focus-visible {
  outline: 2px solid #42657a;
  outline-offset: 1px;
}

.preview-scenario {
  font-family: sans-serif;
  font-size: 0.82rem;
  font-weight: 600;
}

.vte-preview {
  padding: 5px 1px 2px;
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
