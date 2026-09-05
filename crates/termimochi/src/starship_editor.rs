//! Shared controls for reviewed modules in a lossless imported configuration copy.
use crate::starship_draft::{ModuleEdit, StarshipDraft};
use crate::starship_modules::{MODULES, ModuleSpec, module_index};
use crate::starship_scene::{Focus, SceneRequest};
use gtk::{gdk, pango, prelude::*};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
};

type Changed = Box<dyn Fn()>;

pub(crate) struct StarshipEditor {
    pub(crate) root: gtk::Box,
    pub(crate) module: gtk::DropDown,
    pub(crate) symbol_field: gtk::DropDown,
    pub(crate) style_field: gtk::DropDown,
    pub(crate) status: gtk::Label,
    pub(crate) symbol: gtk::Entry,
    pub(crate) style: gtk::Entry,
    pub(crate) format: gtk::Entry,
    pub(crate) color: gtk::ColorDialogButton,
    pub(crate) bold: gtk::Switch,
    pub(crate) layout: gtk::DropDown,
    pub(crate) version: gtk::DropDown,
    pub(crate) scenario: gtk::DropDown,
    pub(crate) enabled: gtk::Switch,
    pub(crate) reset: gtk::Button,
    pub(crate) symbols: Vec<gtk::Button>,
    nerd_preview: gtk::Label,
    nerd_preview_stack: gtk::Stack,
    nerd_font: RefCell<pango::FontDescription>,
    nerd_preview_key: RefCell<Option<(&'static str, String, u32)>>,
    pub(crate) draft: RefCell<Option<StarshipDraft>>,
    pub(crate) file: RefCell<Result<crate::starship_file::FileSnapshot, String>>,
    warning: gtk::Label,
    symbol_group: gtk::Box,
    style_group: gtk::Box,
    layout_row: gtk::Box,
    version_row: gtk::Box,
    style_advanced: gtk::Box,
    shown_module: Cell<usize>,
    invalid: Cell<bool>,
    updating: Cell<bool>,
    changed: RefCell<Option<Changed>>,
    document: Cell<u64>,
}

impl StarshipEditor {
    pub(crate) fn new() -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.set_visible(false);
        let module =
            gtk::DropDown::from_strings(&MODULES.iter().map(|m| m.label).collect::<Vec<_>>());
        module.set_hexpand(true);
        module.add_css_class("prompt-property-control");
        module.update_property(&[gtk::accessible::Property::Label("Module to edit")]);
        let enabled = gtk::Switch::builder().valign(gtk::Align::Center).build();
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        header.append(&module);
        header.append(&enabled);
        root.append(&header);
        enabled.update_property(&[gtk::accessible::Property::Label("Enable selected module")]);

        let symbol = entry("Module symbol", "Starship default (unchanged)");
        let symbol_group = gtk::Box::new(gtk::Orientation::Vertical, 6);
        symbol_group.append(&caption("Symbol"));
        let symbol_field = gtk::DropDown::from_strings(&[]);
        symbol_field.add_css_class("prompt-property-control");
        symbol_field.update_property(&[gtk::accessible::Property::Label("Symbol field")]);
        symbol_group.append(&symbol_field);
        symbol_group.append(&symbol);
        let choices = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let symbols: Vec<_> = [
            ("rs", "Plain text — works with basic terminal fonts"),
            ("🦀", "Emoji — appearance depends on your emoji font"),
            (
                "Nerd Font",
                "Module icon — requires a Nerd Font in the terminal",
            ),
        ]
        .into_iter()
        .map(|(label, tooltip)| {
            let button = gtk::Button::with_label(label);
            button.add_css_class("prompt-starter-button");
            button.set_tooltip_text(Some(tooltip));
            choices.append(&button);
            button
        })
        .collect();
        let nerd_preview = gtk::Label::new(None);
        nerd_preview.set_single_line_mode(true);
        let nerd_preview_stack = gtk::Stack::new();
        nerd_preview_stack.set_size_request(22, 22);
        nerd_preview_stack.set_valign(gtk::Align::Center);
        nerd_preview_stack.add_named(&nerd_preview, Some("glyph"));
        let missing = gtk::Image::from_icon_name("dialog-warning-symbolic");
        missing.set_pixel_size(16);
        missing.add_css_class("status-warning");
        nerd_preview_stack.add_named(&missing, Some("missing"));
        let nerd_choice = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        nerd_choice.append(&nerd_preview_stack);
        nerd_choice.append(&gtk::Label::new(Some("Nerd Font")));
        symbols[2].set_child(Some(&nerd_choice));
        symbol_group.append(&choices);
        root.append(&symbol_group);

        let color = gtk::ColorDialogButton::new(Some(
            gtk::ColorDialog::builder()
                .title("Module Color")
                .with_alpha(false)
                .build(),
        ));
        color.set_valign(gtk::Align::Center);
        let bold = gtk::Switch::builder().valign(gtk::Align::Center).build();
        let layout = gtk::DropDown::from_strings(&[
            "Imported format",
            "Symbol + Version",
            "Symbol only",
            "Version only",
        ]);
        let version = gtk::DropDown::from_strings(&[
            "Imported version",
            "Full version",
            "Major.Minor",
            "Major only",
        ]);
        for dropdown in [&layout, &version] {
            dropdown.add_css_class("prompt-property-control");
        }
        let style_group = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let style_field = gtk::DropDown::from_strings(&[]);
        style_field.add_css_class("prompt-property-control");
        style_field.update_property(&[gtk::accessible::Property::Label("Style field")]);
        style_group.append(&style_field);
        style_group.append(&row("Color", &color));
        style_group.append(&row("Bold", &bold));
        root.append(&style_group);
        let layout_row = row("Display", &layout);
        let version_row = row("Version", &version);
        root.append(&layout_row);
        root.append(&version_row);
        let scenario = gtk::DropDown::from_strings(&[
            "Current Folder",
            "Rust Sample",
            "Node.js Sample",
            "Python Sample",
            "Go Sample",
        ]);
        scenario.add_css_class("prompt-property-control");
        scenario.set_tooltip_text(Some("The initial prompt before the simulated commands. Selecting a module adds a command and its resulting full prompt."));
        root.append(&row("Starting Prompt", &scenario));
        let style = entry("Module Starship style", "bold red / fg:#eb6f92");
        let advanced = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let style_advanced = gtk::Box::new(gtk::Orientation::Vertical, 6);
        style_advanced.append(&caption("Starship Style"));
        style_advanced.append(&style);
        advanced.append(&style_advanced);
        let format = entry("Module Starship format", "Starship default (unchanged)");
        format.set_tooltip_text(Some("This module's Starship format. Variables and styles are preserved; this does not change the top-level prompt order."));
        advanced.append(&caption("Module Format"));
        advanced.append(&format);
        root.append(
            &gtk::Expander::builder()
                .label("Advanced")
                .child(&advanced)
                .build(),
        );

        let warning = gtk::Label::new(None);
        warning.set_xalign(0.0);
        warning.set_wrap(true);
        warning.set_max_width_chars(34);
        warning.add_css_class("preview-source-detail");
        root.append(&warning);
        let status = gtk::Label::new(Some("Changes stay in preview until you save."));
        status.set_xalign(0.0);
        status.set_wrap(true);
        status.set_max_width_chars(34);
        status.add_css_class("preview-source-detail");
        root.append(&status);
        let reset = gtk::Button::with_label("Reset Module to Loaded");
        reset.add_css_class("prompt-menu-item");
        root.append(&reset);
        let this = Rc::new(Self {
            root,
            module,
            symbol_field,
            style_field,
            symbol_group,
            style_group,
            style_advanced,
            layout_row,
            version_row,
            shown_module: Cell::new(usize::MAX),
            status,
            symbol,
            style,
            format,
            color,
            bold,
            layout,
            version,
            scenario,
            enabled,
            reset,
            symbols,
            nerd_preview,
            nerd_preview_stack,
            nerd_font: RefCell::new(
                crate::typography::TypographySettings::default().font_description(),
            ),
            nerd_preview_key: RefCell::new(None),
            warning,
            draft: RefCell::new(None),
            file: RefCell::new(Err("No configuration is loaded.".into())),
            updating: Cell::new(false),
            invalid: Cell::new(false),
            changed: RefCell::new(None),
            document: Cell::new(0),
        });
        Self::connect(&this);
        this
    }

    pub(crate) fn connect_changed(&self, changed: impl Fn() + 'static) {
        *self.changed.borrow_mut() = Some(Box::new(changed));
    }
    pub(crate) fn set_preview_font(&self, font: &pango::FontDescription) {
        self.nerd_font.replace(font.clone());
        if let Some(draft) = self.draft.borrow().as_ref() {
            self.refresh_nerd_preview(draft.spec());
        }
    }
    fn refresh_nerd_preview(&self, spec: &ModuleSpec) {
        let Some(presets) = spec.presets else {
            self.nerd_preview.set_text("");
            self.nerd_preview_key.borrow_mut().take();
            return;
        };
        let font = self.nerd_font.borrow();
        let context = self.nerd_preview.pango_context();
        let key = (
            spec.id,
            font.to_string().to_string(),
            context.font_map().map_or(0, |map| map.serial()),
        );
        if self.nerd_preview_key.borrow().as_ref() == Some(&key) {
            return;
        }
        // The preview and click handler share the exact same preset. Only trim
        // padding for the small specimen; applying it retains the original spaces.
        let glyph = presets[2].trim();
        let mut specimen_font = font.clone();
        specimen_font.set_absolute_size(18.0 * f64::from(pango::SCALE));
        let attributes = pango::AttrList::new();
        attributes.insert(pango::AttrFontDesc::new(&specimen_font));
        self.nerd_preview.set_attributes(Some(&attributes));
        self.nerd_preview.set_text(glyph);
        let layout = pango::Layout::new(&context);
        layout.set_font_description(Some(&specimen_font));
        layout.set_text(glyph);
        let missing = layout.unknown_glyphs_count() > 0;
        let primary = context
            .load_font(&specimen_font)
            .is_some_and(|face| glyph.chars().all(|ch| face.has_char(ch)));
        self.nerd_preview_stack
            .set_visible_child_name(if missing { "missing" } else { "glyph" });
        let coverage = if missing {
            "No available font can display this icon. Choose a Nerd Font in Typography."
        } else if primary {
            "Available in the selected preview font."
        } else {
            "Shown using font fallback. The selected terminal font does not contain this icon."
        };
        let codepoints = glyph
            .chars()
            .map(|ch| format!("U+{:04X}", u32::from(ch)))
            .collect::<Vec<_>>()
            .join(" ");
        let family = font.family().unwrap_or_else(|| "Monospace".into());
        let detail = format!(
            "{} icon · {}\nPreview font: {} (enlarged specimen).\n{}",
            spec.label, codepoints, family, coverage
        );
        self.symbols[2].set_tooltip_text(Some(&detail));
        self.symbols[2].update_property(&[
            gtk::accessible::Property::Label(&format!("Use {} Nerd Font icon", spec.label)),
            gtk::accessible::Property::Description(&detail),
        ]);
        self.nerd_preview_key.replace(Some(key));
    }
    pub(crate) fn scene(&self) -> Option<SceneRequest> {
        self.draft
            .borrow()
            .as_ref()
            .map(|draft| SceneRequest::capture(draft, self.document.get()))
    }
    pub(crate) fn select_module(&self, id: &str) {
        if let Some(index) = module_index(id) {
            self.module.set_selected(index as u32);
        }
    }
    pub(crate) fn open_finding(&self, id: &str, message: &str) {
        self.select_module(id);
        if self.invalid() {
            return;
        }
        let characters: Vec<char> = message
            .split('\n')
            .next()
            .unwrap_or_default()
            .split("U+")
            .skip(1)
            .filter_map(|part| {
                let hex: String = part.chars().take_while(char::is_ascii_hexdigit).collect();
                u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
            })
            .collect();
        let mut symbol_match = false;
        if let Some(draft) = self.draft.borrow_mut().as_mut() {
            if draft.spec().id != id {
                return;
            }
            if let Some(index) = draft.spec().symbols.iter().position(|field| {
                let text = draft
                    .field(field.key)
                    .unwrap_or_else(|| field.default.into());
                characters.iter().any(|ch| text.contains(*ch))
            }) {
                draft.finish_text_edit();
                draft.symbol = index;
                draft.sample_focus = Focus::Symbol;
                symbol_match = true;
            }
        }
        self.refresh();
        self.notify();
        if symbol_match {
            self.symbol.grab_focus();
        } else {
            self.module.grab_focus();
        }
    }
    fn notify(&self) {
        if let Some(changed) = self.changed.borrow().as_ref() {
            changed();
        }
    }
    pub(crate) fn begin(&self, path: PathBuf, source: String) -> Result<(), String> {
        let draft = StarshipDraft::new(path.clone(), source.clone())?;
        *self.file.borrow_mut() = crate::starship_file::FileSnapshot::bind(&path, &source);
        self.document.set(self.document.get().wrapping_add(1));
        *self.draft.borrow_mut() = Some(draft);
        self.invalid.set(false);
        self.refresh();
        self.notify();
        Ok(())
    }
    pub(crate) fn document(&self) -> u64 {
        self.document.get()
    }
    pub(crate) fn accept_saved(&self, document: u64, saved: crate::starship_file::FileSnapshot) {
        if document != self.document.get() {
            return;
        }
        if let Some(draft) = self.draft.borrow_mut().as_mut() {
            draft.mark_exported(saved.contents.clone());
        }
        *self.file.borrow_mut() = Ok(saved);
        self.notify();
    }
    pub(crate) fn accept_restored(&self, document: u64, saved: crate::starship_file::FileSnapshot) {
        if document != self.document.get() {
            return;
        }
        if let Some(draft) = self.draft.borrow_mut().as_mut() {
            draft
                .replace_contents(saved.contents.clone())
                .expect("validated backup");
        }
        self.invalid.set(false);
        self.refresh();
        self.accept_saved(document, saved);
    }
    pub(crate) fn invalid(&self) -> bool {
        self.invalid.get()
    }
    pub(crate) fn dirty(&self) -> bool {
        self.invalid()
            || self
                .draft
                .borrow()
                .as_ref()
                .is_some_and(StarshipDraft::dirty)
    }
    pub(crate) fn can_undo(&self) -> bool {
        self.invalid()
            || self
                .draft
                .borrow()
                .as_ref()
                .is_some_and(StarshipDraft::can_undo)
    }
    pub(crate) fn can_redo(&self) -> bool {
        !self.invalid()
            && self
                .draft
                .borrow()
                .as_ref()
                .is_some_and(StarshipDraft::can_redo)
    }
    pub(crate) fn undo(&self) {
        if !self.invalid.replace(false)
            && let Some(draft) = self.draft.borrow_mut().as_mut()
        {
            draft.undo();
        }
        self.refresh();
        self.notify();
    }
    pub(crate) fn redo(&self) {
        if self.invalid() {
            return;
        }
        if let Some(draft) = self.draft.borrow_mut().as_mut() {
            draft.redo();
        }
        self.refresh();
        self.notify();
    }
    pub(crate) fn discard_warning(&self) {
        self.invalid.set(false);
        if let Some(draft) = self.draft.borrow_mut().as_mut() {
            draft.mark_exported(draft.contents().to_owned());
        }
    }
    fn apply(&self, edit: ModuleEdit, entry: Option<&gtk::Entry>) {
        if self.updating.get() {
            return;
        }
        let result = self.draft.borrow_mut().as_mut().map(|draft| {
            if let Some(entry) = entry {
                draft.edit_text(
                    edit,
                    if entry == &self.symbol {
                        "symbol"
                    } else if entry == &self.format {
                        "format"
                    } else {
                        "style"
                    },
                )
            } else {
                draft.edit(edit)
            }
        });
        match result {
            Some(Err(error)) => {
                if let Some(entry) = entry {
                    entry.add_css_class("error");
                }
                self.invalid.set(true);
                self.status.set_text(&error);
                self.notify();
            }
            Some(Ok(changed)) => {
                if let Some(entry) = entry {
                    entry.remove_css_class("error");
                }
                // Keep an invalid value in another entry visible and block export.
                self.invalid.set(
                    self.symbol.has_css_class("error")
                        || self.style.has_css_class("error")
                        || self.format.has_css_class("error"),
                );
                if !self.invalid() {
                    // Avoid resetting the insertion point while the user types.
                    self.refresh_except(entry);
                }
                if changed || !self.invalid() {
                    self.notify();
                }
            }
            None => {}
        }
    }
    fn refresh(&self) {
        self.symbol.remove_css_class("error");
        self.style.remove_css_class("error");
        self.format.remove_css_class("error");
        self.refresh_except(None);
    }
    fn refresh_except(&self, editing: Option<&gtk::Entry>) {
        let draft = self.draft.borrow();
        let Some(draft) = draft.as_ref() else {
            return;
        };
        self.updating.set(true);
        let spec = draft.spec();
        self.module.set_selected(draft.module as u32);
        if self.shown_module.replace(draft.module) != draft.module {
            self.symbol_field.set_model(Some(&gtk::StringList::new(
                &spec.symbols.iter().map(|f| f.label).collect::<Vec<_>>(),
            )));
            self.style_field.set_model(Some(&gtk::StringList::new(
                &spec.styles.iter().map(|f| f.label).collect::<Vec<_>>(),
            )));
        }
        self.symbol_field.set_selected(draft.symbol as u32);
        self.style_field.set_selected(draft.style as u32);
        self.symbol_group.set_visible(!spec.symbols.is_empty());
        self.symbol_field.set_visible(spec.symbols.len() > 1);
        self.style_group.set_visible(!spec.styles.is_empty());
        self.style_field.set_visible(spec.styles.len() > 1);
        self.style_advanced.set_visible(!spec.styles.is_empty());
        self.layout_row.set_visible(spec.version);
        self.version_row.set_visible(spec.version);
        self.reset
            .set_label(&format!("Reset {} to Loaded", spec.label));
        self.enabled
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "Enable {} module",
                spec.label
            ))]);
        self.symbol.set_tooltip_text(Some(if draft.symbol_spec().is_some_and(|f| f.formatted) {
            "Starship format: keep styles such as [>](bold green) and variables such as ${count}, or enter plain text."
        } else { "Literal text; special characters are escaped automatically." }));
        for (index, button) in self.symbols.iter().enumerate() {
            button.set_visible(spec.presets.is_some());
            if let Some(presets) = spec.presets
                && index < 2
            {
                button.set_label(presets[index].trim());
                button.set_tooltip_text(Some(match index {
                    0 => "Plain text — works with basic terminal fonts",
                    _ => "Unicode symbol — appearance depends on available fonts",
                }));
            }
        }
        self.refresh_nerd_preview(spec);
        if editing != Some(&self.symbol) {
            self.symbol.set_text(&draft.literal_symbol());
        }
        if editing != Some(&self.style) {
            self.style.set_text(&draft.style());
        }
        if editing != Some(&self.format) {
            self.format
                .set_text(&draft.field("format").unwrap_or_default());
        }
        if let Some(color) = draft.color().and_then(|s| gdk::RGBA::parse(s).ok()) {
            self.color.set_rgba(&color);
        } else {
            self.color.set_rgba(&gdk::RGBA::TRANSPARENT);
        }
        self.bold
            .set_active(draft.style().split_whitespace().any(|t| t == "bold"));
        self.layout.set_selected(draft.layout_index());
        self.version.set_selected(draft.version_index());
        self.enabled.set_active(draft.enabled());
        // Font compatibility is reported centrally using actual glyph coverage,
        // not an unconditional warning for every private-use character.
        let warning = if spec.version
            && draft
                .field("format")
                .is_some_and(|f| !f.contains("$symbol") && !f.contains("${symbol}"))
        {
            "This format hides the symbol. Choose Symbol + Version to show it."
        } else {
            ""
        };
        self.warning.set_text(warning);
        self.warning.set_visible(!warning.is_empty());
        self.updating.set(false);
    }
    fn connect(this: &Rc<Self>) {
        for (dropdown, kind) in [
            (&this.module, 0),
            (&this.symbol_field, 1),
            (&this.style_field, 2),
        ] {
            let weak = Rc::downgrade(this);
            dropdown.connect_selected_notify(move |dropdown| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                if this.updating.get() {
                    return;
                }
                if this.invalid() {
                    this.updating.set(true);
                    if let Some(draft) = this.draft.borrow().as_ref() {
                        dropdown.set_selected(match kind {
                            0 => draft.module,
                            1 => draft.symbol,
                            _ => draft.style,
                        } as u32);
                    }
                    this.updating.set(false);
                    this.status
                        .set_text("Fix the highlighted field or Undo before switching.");
                    return;
                }
                if let Some(draft) = this.draft.borrow_mut().as_mut() {
                    draft.finish_text_edit();
                    let index = dropdown.selected() as usize;
                    match kind {
                        0 => {
                            let _ = draft.select_module(index);
                        }
                        1 if index < draft.spec().symbols.len() => {
                            draft.symbol = index;
                            draft.sample_focus = Focus::Symbol;
                        }
                        2 if index < draft.spec().styles.len() => {
                            draft.style = index;
                            draft.sample_focus = Focus::Style;
                        }
                        _ => (),
                    }
                }
                // Navigation does not mutate the draft, but its scene must
                // follow the selected module/field even outside a matching project.
                this.refresh();
                this.notify();
            });
        }
        for entry in [&this.symbol, &this.style, &this.format] {
            let focus = gtk::EventControllerFocus::new();
            let weak = Rc::downgrade(this);
            focus.connect_leave(move |_| {
                if let Some(this) = weak.upgrade()
                    && let Some(draft) = this.draft.borrow_mut().as_mut()
                {
                    draft.finish_text_edit();
                }
            });
            entry.add_controller(focus);
        }
        let weak = Rc::downgrade(this);
        this.scenario.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                this.notify();
            }
        });
        let weak = Rc::downgrade(this);
        this.symbol.connect_changed(move |entry| {
            if let Some(this) = weak.upgrade() {
                this.apply(ModuleEdit::Symbol(entry.text().to_string()), Some(entry));
            }
        });
        let weak = Rc::downgrade(this);
        this.style.connect_changed(move |entry| {
            if let Some(this) = weak.upgrade() {
                this.apply(ModuleEdit::Style(entry.text().to_string()), Some(entry));
            }
        });
        let weak = Rc::downgrade(this);
        this.format.connect_changed(move |entry| {
            if let Some(this) = weak.upgrade() {
                this.apply(ModuleEdit::Format(entry.text().to_string()), Some(entry));
            }
        });
        for (index, button) in this.symbols.iter().enumerate() {
            let weak = Rc::downgrade(this);
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    let symbol = this
                        .draft
                        .borrow()
                        .as_ref()
                        .and_then(|d| d.spec().presets)
                        .map(|p| p[index]);
                    if let Some(symbol) = symbol {
                        this.symbol.remove_css_class("error");
                        this.apply(ModuleEdit::Symbol(symbol.into()), None);
                    }
                }
            });
        }
        let weak = Rc::downgrade(this);
        this.color.connect_rgba_notify(move |button| {
            if let Some(this) = weak.upgrade() {
                let c = button.rgba();
                this.apply(
                    ModuleEdit::Color(format!(
                        "#{:02x}{:02x}{:02x}",
                        (c.red() * 255.0).round() as u8,
                        (c.green() * 255.0).round() as u8,
                        (c.blue() * 255.0).round() as u8
                    )),
                    None,
                );
            }
        });
        let weak = Rc::downgrade(this);
        this.bold.connect_active_notify(move |switch| {
            if let Some(this) = weak.upgrade() {
                this.apply(ModuleEdit::Bold(switch.is_active()), None);
            }
        });
        let weak = Rc::downgrade(this);
        this.enabled.connect_active_notify(move |switch| {
            if let Some(this) = weak.upgrade() {
                this.apply(ModuleEdit::Enabled(switch.is_active()), None);
            }
        });
        let weak = Rc::downgrade(this);
        this.layout.connect_selected_notify(move |dropdown| {
            if let Some(this) = weak.upgrade() {
                this.apply(ModuleEdit::Layout(dropdown.selected()), None);
            }
        });
        let weak = Rc::downgrade(this);
        this.version.connect_selected_notify(move |dropdown| {
            if let Some(this) = weak.upgrade() {
                this.apply(ModuleEdit::Version(dropdown.selected()), None);
            }
        });
        let weak = Rc::downgrade(this);
        this.reset.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.invalid.set(false);
                this.symbol.remove_css_class("error");
                this.style.remove_css_class("error");
                this.format.remove_css_class("error");
                this.apply(ModuleEdit::Reset, None);
            }
        });
    }
}
fn caption(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.add_css_class("layout-row-label");
    label
}
fn entry(label: &str, placeholder: &str) -> gtk::Entry {
    let entry = gtk::Entry::builder()
        .placeholder_text(placeholder)
        .hexpand(true)
        .build();
    entry.update_property(&[gtk::accessible::Property::Label(label)]);
    entry
}
fn row(label: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("layout-row");
    let label_widget = caption(label);
    label_widget.set_hexpand(true);
    row.append(&label_widget);
    row.append(widget);
    widget
        .as_ref()
        .update_property(&[gtk::accessible::Property::Label(label)]);
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a graphical GTK session; run with --ignored --test-threads=1"]
    fn nerd_font_preset_preview_tracks_module_and_font() {
        adw::init().unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("starship.toml");
        let source = "# Keep my settings\n[rust]\nsymbol = ' rs '\n";
        std::fs::write(&path, source).unwrap();
        let editor = StarshipEditor::new();
        editor.begin(path.clone(), source.into()).unwrap();
        editor.root.set_visible(true);
        let contents = || {
            editor
                .draft
                .borrow()
                .as_ref()
                .unwrap()
                .contents()
                .to_owned()
        };
        for spec in MODULES.iter().filter(|spec| spec.presets.is_some()) {
            editor.select_module(spec.id);
            let preset = spec.presets.unwrap()[2];
            assert!(editor.symbols[2].is_visible());
            assert_eq!(editor.nerd_preview.text(), preset.trim());
            assert!(
                editor.symbols[2]
                    .tooltip_text()
                    .unwrap()
                    .contains(spec.label)
            );
            assert_eq!(contents(), source, "previewing must not edit the draft");
            editor.symbols[2].emit_clicked();
            assert_eq!(
                editor.draft.borrow().as_ref().unwrap().literal_symbol(),
                preset,
                "click must apply exactly the pictured preset, including spaces"
            );
            editor.undo();
            assert_eq!(contents(), source);
        }
        editor.select_module("cmd_duration");
        assert!(!editor.symbols[2].is_visible());
        assert!(editor.nerd_preview.text().is_empty());
        editor.select_module("rust");
        let font = pango::FontDescription::from_string("DejaVu Sans Mono 24");
        editor.set_preview_font(&font);
        assert!(
            editor.symbols[2]
                .tooltip_text()
                .unwrap()
                .contains("DejaVu Sans Mono")
        );
        assert_eq!(contents(), source, "font changes must not edit the draft");
        let cached = editor.nerd_preview_key.borrow().clone();
        let detail = editor.symbols[2].tooltip_text();
        editor.refresh();
        assert_eq!(*editor.nerd_preview_key.borrow(), cached);
        assert_eq!(editor.symbols[2].tooltip_text(), detail);

        // Exercise loaded-face support and genuine missing glyphs without
        // requiring a particular Nerd Font to be installed on the test host.
        let specimen = |id, glyph| ModuleSpec {
            id,
            label: "Specimen",
            symbols: &[],
            styles: &[],
            version: false,
            disabled: false,
            presets: Some(["text", "emoji", glyph]),
        };
        editor.refresh_nerd_preview(&specimen("ascii-test", "A"));
        assert_eq!(
            editor.nerd_preview_stack.visible_child_name().as_deref(),
            Some("glyph")
        );
        assert!(
            editor.symbols[2]
                .tooltip_text()
                .unwrap()
                .contains("Available in the selected")
        );
        editor.refresh_nerd_preview(&specimen("missing-test", "\u{10fffd}"));
        assert_eq!(
            editor.nerd_preview_stack.visible_child_name().as_deref(),
            Some("missing")
        );
        assert!(
            editor.symbols[2]
                .tooltip_text()
                .unwrap()
                .contains("No available font")
        );
        editor.refresh();
        assert_eq!(editor.nerd_preview.text(), "\u{e7a8}");
        // With a patched fallback installed the glyph is visible, but it must
        // not be advertised as coverage by this unpatched primary face.
        if editor.nerd_preview_stack.visible_child_name().as_deref() == Some("glyph") {
            assert!(
                editor.symbols[2]
                    .tooltip_text()
                    .unwrap()
                    .contains("font fallback")
            );
        }
        assert!(!editor.dirty());
        assert_eq!(std::fs::read_to_string(path).unwrap(), source);
    }
}
