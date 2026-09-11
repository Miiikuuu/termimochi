//! A single reusable field popover; the list stays quiet until a field is selected.
use super::*;
use crate::greeting_fields::{self, FieldStyle};

pub(super) struct FieldInspector {
    pub popover: gtk::Popover,
    buttons: RefCell<Vec<(usize, glib::WeakRef<gtk::MenuButton>)>>,
    anchor: glib::WeakRef<gtk::MenuButton>,
    selected: Cell<Option<usize>>,
    source: RefCell<String>,
    updating: Cell<bool>,
    draft: RefCell<Option<FieldStyle>>,
    title: gtk::Label,
    label: gtk::Entry,
    icon: gtk::Entry,
    key_color: gtk::DropDown,
    value_color: gtk::DropDown,
    format: gtk::DropDown,
    reset: gtk::Button,
    note: gtk::Label,
}

impl FieldInspector {
    pub fn new() -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        content.set_width_request(270);
        for side in [0, 1, 2, 3] {
            match side {
                0 => content.set_margin_top(14),
                1 => content.set_margin_bottom(14),
                2 => content.set_margin_start(14),
                _ => content.set_margin_end(14),
            }
        }
        let title = gtk::Label::builder()
            .xalign(0.0)
            .css_classes(["heading"])
            .build();
        let label = gtk::Entry::builder()
            .enable_undo(false)
            .max_length(65)
            .hexpand(true)
            .build();
        let icon = gtk::Entry::builder()
            .enable_undo(false)
            .max_length(9)
            .hexpand(true)
            .placeholder_text("Optional · ASCII or Nerd Font")
            .build();
        label.update_property(&[gtk::accessible::Property::Label("Greeting Field Name")]);
        icon.update_property(&[gtk::accessible::Property::Label("Greeting Field Icon")]);
        let mut colors = vec!["Inherit"];
        colors.extend(ANSI_NAMES);
        colors.push("Text");
        let key_color = layout_drop_down(
            &colors,
            0,
            "Field Name Color",
            "Inherit or choose a terminal palette slot",
        );
        let value_color = layout_drop_down(
            &colors,
            0,
            "Field Content Color",
            "Inherit or choose a terminal palette slot",
        );
        let format = layout_drop_down(
            &["Original"],
            0,
            "Field Display Format",
            "Original retains the source format, including formats not editable here",
        );
        let reset = gtk::Button::with_label("Reset Field");
        reset.add_css_class("flat");
        let note = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .max_width_chars(34)
            .css_classes(["dim-label"])
            .build();
        content.append(&title);
        for (name, child) in [
            ("Name", label.clone().upcast::<gtk::Widget>()),
            ("Icon", icon.clone().upcast()),
            ("Name color", key_color.clone().upcast()),
            ("Content color", value_color.clone().upcast()),
            ("Display", format.clone().upcast()),
        ] {
            content.append(&typography_field(name, &child));
        }
        content.append(&note);
        content.append(&reset);
        let popover = gtk::Popover::builder()
            .child(&content)
            .position(gtk::PositionType::Right)
            .has_arrow(false)
            .css_classes(["greeting-field-popover"])
            .build();
        Self {
            popover,
            buttons: RefCell::new(Vec::new()),
            anchor: glib::WeakRef::new(),
            selected: Cell::new(None),
            source: RefCell::new(String::new()),
            updating: Cell::new(false),
            draft: RefCell::new(None),
            title,
            label,
            icon,
            key_color,
            value_color,
            format,
            reset,
            note,
        }
    }
}

pub(super) fn field_button(label: &str) -> gtk::MenuButton {
    let title = gtk::Label::builder()
        .label(label)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .hexpand(true)
        .build();
    gtk::MenuButton::builder()
        .child(&title)
        .hexpand(true)
        .has_frame(false)
        .direction(gtk::ArrowType::Right)
        .always_show_arrow(false)
        .tooltip_text("Edit this field")
        .css_classes(["greeting-field-button"])
        .build()
}

impl GreetingEditor {
    pub(in crate::window) fn open_color_field(&self, index: usize) {
        let weak = self.weak.clone();
        // Wait for the newly selected module to be allocated. Custom and
        // native lists may share indices; only use the currently mapped row.
        self.root.add_tick_callback(move |_, _| {
            if let Some(this) = weak.upgrade() {
                let button = this
                    .fields
                    .buttons
                    .borrow()
                    .iter()
                    .filter(|(id, _)| *id == index)
                    .filter_map(|(_, button)| button.upgrade())
                    .find(|button| button.is_mapped());
                if let Some(button) = button {
                    button.grab_focus();
                    button.popup();
                }
            }
            glib::ControlFlow::Break
        });
    }
    pub(super) fn bind_field(&self, button: &gtk::MenuButton, index: usize) {
        let mut buttons = self.fields.buttons.borrow_mut();
        buttons.retain(|(_, button)| button.upgrade().is_some());
        buttons.push((index, button.downgrade()));
        drop(buttons);
        let weak = self.weak.clone();
        button.set_create_popup_func(move |button| {
            if let Some(this) = weak.upgrade() {
                this.select_field(index, button);
            }
        });
    }
    fn source_key(&self) -> String {
        let settings = self.settings.borrow();
        if settings.imported_source.is_some() {
            "imported".into()
        } else {
            format!("{:?}", settings.official_preset)
        }
    }
    fn select_field(&self, index: usize, anchor: &gtk::MenuButton) {
        if self.invalid.get()
            && (self.fields.selected.get() != Some(index) || !self.invalid_field_draft())
        {
            return;
        }
        if let Some(previous) = self.fields.anchor.upgrade() {
            previous.set_popover(None::<&gtk::Popover>);
        }
        self.finish();
        self.fields.selected.set(Some(index));
        *self.fields.source.borrow_mut() = self.source_key();
        self.fields.anchor.set(Some(anchor));
        if !self.invalid.get() {
            self.refresh_field_inspector();
        }
        anchor.set_popover(Some(&self.fields.popover));
    }
    pub(super) fn refresh_field_inspector(&self) {
        let inspector = &self.fields;
        if *inspector.source.borrow() != self.source_key() {
            inspector.popover.popdown();
            if let Some(anchor) = inspector.anchor.upgrade() {
                anchor.set_popover(None::<&gtk::Popover>);
            }
            inspector.anchor.set(None::<&gtk::MenuButton>);
            inspector.selected.set(None);
            *inspector.draft.borrow_mut() = None;
            return;
        }
        let Some(index) = inspector.selected.get() else {
            return;
        };
        let settings = self.settings();
        let id = settings.style_id(index);
        let Some(module) = settings.module_for_field_id(&id) else {
            inspector.popover.popdown();
            return;
        };
        let kind = greeting_fields::module_kind(&module);
        let style = settings.field_styles.get(&id).cloned().unwrap_or_default();
        *inspector.draft.borrow_mut() = None;
        inspector.updating.set(true);
        inspector
            .title
            .set_text(&format!("{} · Field", kind.to_uppercase()));
        inspector.label.set_text(
            style
                .label
                .as_deref()
                .or_else(|| module["key"].as_str())
                .unwrap_or(kind),
        );
        inspector.icon.set_text(style.icon.as_deref().unwrap_or(""));
        inspector
            .key_color
            .set_selected(style.key_color.map_or(0, |s| u32::from(s) + 1));
        inspector
            .value_color
            .set_selected(style.value_color.map_or(0, |s| u32::from(s) + 1));
        let options = greeting_fields::formats(kind);
        let mut labels = vec!["Original"];
        labels.extend(options.iter().map(|(label, _)| *label));
        inspector
            .format
            .set_model(Some(&gtk::StringList::new(&labels)));
        inspector.format.set_selected(
            style
                .format
                .as_ref()
                .and_then(|f| options.iter().position(|(_, v)| f == v))
                .map_or(0, |i| i as u32 + 1),
        );
        let editable = greeting_fields::editable(kind);
        for child in [
            inspector.label.clone().upcast::<gtk::Widget>(),
            inspector.icon.clone().upcast(),
            inspector.key_color.clone().upcast(),
            inspector.value_color.clone().upcast(),
            inspector.format.clone().upcast(),
            inspector.reset.clone().upcast(),
        ] {
            child.set_sensitive(editable);
        }
        inspector.note.set_text(if editable {
            "Live preview · Reset restores the source field"
        } else {
            "Preserved read-only. This module is not supported by the field editor."
        });
        inspector.updating.set(false);
    }
    fn edit_field_style(&self, text_edit: bool, edit: impl FnOnce(&mut FieldStyle, &str)) {
        if self.updating.get() || self.fields.updating.get() {
            return;
        }
        if self.invalid.get() && !self.invalid_field_draft() {
            return;
        }
        let Some(index) = self.fields.selected.get() else {
            return;
        };
        let mut settings = self.settings();
        let id = settings.style_id(index);
        let Some(module) = settings.module_for_field_id(&id) else {
            return;
        };
        let mut style = self
            .fields
            .draft
            .borrow()
            .clone()
            .unwrap_or_else(|| settings.field_styles.get(&id).cloned().unwrap_or_default());
        edit(&mut style, greeting_fields::module_kind(&module));
        settings.field_styles.insert(id.clone(), style.clone());
        if style == FieldStyle::default() {
            settings.field_styles.remove(&id);
        }
        if let Err(error) = settings.validate() {
            *self.fields.draft.borrow_mut() = Some(style);
            self.invalid.set(true);
            self.fields.note.set_text(&error);
            self.status.set_text(&error);
            self.notify();
            return;
        }
        *self.fields.draft.borrow_mut() = None;
        self.invalid.set(false);
        if settings != self.settings() {
            let mut history = self.history.borrow_mut();
            if !text_edit {
                history.commit(self.settings());
            }
            history.begin(self.settings());
            history.mark_changed();
            if !text_edit {
                history.commit(settings.clone());
            }
            *self.settings.borrow_mut() = settings;
        }
        self.fields
            .note
            .set_text("Live preview · Reset restores the source field");
        self.refresh_status();
        self.notify();
    }
    pub(super) fn invalid_field_draft(&self) -> bool {
        self.fields.draft.borrow().is_some()
    }
    pub(super) fn connect_field_inspector(this: &Rc<Self>) {
        for (entry, icon) in [(&this.fields.label, false), (&this.fields.icon, true)] {
            let weak = this.weak.clone();
            // notify::text is emitted after an atomic set_text replacement;
            // changed also exposes its intermediate deletion as an empty name.
            entry.connect_text_notify(move |entry| {
                if let Some(this) = weak.upgrade() {
                    this.edit_field_style(true, |style, _| {
                        if icon {
                            style.icon = Some(entry.text().into());
                        } else {
                            style.label = Some(entry.text().into());
                        }
                    });
                }
            });
            let focus = gtk::EventControllerFocus::new();
            let weak = this.weak.clone();
            focus.connect_leave(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.finish();
                }
            });
            entry.add_controller(focus);
        }
        for (dropdown, output) in [
            (&this.fields.key_color, false),
            (&this.fields.value_color, true),
        ] {
            let weak = this.weak.clone();
            dropdown.connect_selected_notify(move |dropdown| {
                if let Some(this) = weak.upgrade() {
                    this.edit_field_style(false, |style, _| {
                        let color = dropdown.selected().checked_sub(1).map(|s| s.min(16) as u8);
                        if output {
                            style.value_color = color;
                        } else {
                            style.key_color = color;
                        }
                    });
                }
            });
        }
        let weak = this.weak.clone();
        this.fields.format.connect_selected_notify(move |dropdown| {
            if let Some(this) = weak.upgrade() {
                this.edit_field_style(false, |style, kind| {
                    style.format = dropdown.selected().checked_sub(1).and_then(|i| {
                        greeting_fields::formats(kind)
                            .get(i as usize)
                            .map(|(_, s)| s.to_string())
                    });
                });
            }
        });
        let weak = this.weak.clone();
        this.fields.reset.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade()
                && let Some(index) = this.fields.selected.get()
            {
                let mut settings = this.settings();
                settings.field_styles.remove(&settings.style_id(index));
                this.replace(settings, true);
            }
        });
    }
    pub(super) fn refresh_imported_fields(&self) {
        let settings = self.settings();
        let imported = settings.imported_source.is_some();
        self.imported_list.set_visible(imported);
        self.appearance_group.set_visible(!imported);
        if imported {
            self.preset_note
                .set_text("Imported JSONC · original layout preserved");
        }
        self.message.set_visible(!imported);
        self.enabled.set_sensitive(!imported);
        if !imported {
            return;
        }
        let modules = settings
            .imported_source
            .as_ref()
            .and_then(|s| crate::fastfetch_document::value(s).ok())
            .and_then(|v| v["modules"].as_array().cloned())
            .unwrap_or_default();
        let signature: Vec<_> = modules
            .iter()
            .map(|m| greeting_fields::module_kind(m).to_owned())
            .collect();
        if signature != *self.imported_signature.borrow() {
            self.fields.popover.popdown();
            while let Some(child) = self.imported_list.first_child() {
                self.imported_list.remove(&child);
            }
            for (index, module) in modules.iter().enumerate() {
                let kind = greeting_fields::module_kind(module);
                let label = format!(
                    "{}{}",
                    kind,
                    if greeting_fields::editable(kind) {
                        ""
                    } else {
                        " · read-only"
                    }
                );
                let button = field_button(&label);
                self.bind_field(&button, index);
                self.imported_list.append(&button);
            }
            *self.imported_signature.borrow_mut() = signature;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::greeting::tests::{
        controller, descendants, feed, respond, settle, wait_official,
    };

    fn open_custom(this: &Workbench, kind: Info) {
        let row = &this
            .greeting
            .rows
            .iter()
            .find(|(k, ..)| *k == kind)
            .unwrap()
            .1;
        let content = this.greeting.root.child().unwrap();
        let bounds = row.compute_bounds(&content).unwrap();
        let scroll = this.greeting.root.vadjustment();
        scroll.set_value(f64::from(bounds.y()).min(scroll.upper() - scroll.page_size()));
        settle();
        descendants(&row.clone().upcast())
            .into_iter()
            .find_map(|w| w.downcast::<gtk::MenuButton>().ok())
            .unwrap()
            .popup();
        settle();
        assert!(this.greeting.fields.popover.is_visible());
    }

    fn capture(window: &gtk::Window) {
        if let Some(path) = std::env::var_os("TERMIMOCHI_FIELDS_SCREENSHOT") {
            window.set_title(Some("TermiMochi point-to-edit test"));
            let mut child = std::process::Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../scripts/preview-pointer-driver.py"
                ))
                .args(["capture", "0", "0"])
                .env("TERMIMOCHI_INSPECT_SCREENSHOT", path)
                .spawn()
                .unwrap();
            while child.try_wait().unwrap().is_none() {
                settle();
            }
            assert!(child.wait().unwrap().success());
        }
    }

    #[test]
    #[ignore = "requires GTK/VTE, Fastfetch and Bubblewrap; run separately at 1x and 2x"]
    fn greeting_field_editor_import_review_apply_restore_and_invalid_input() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.GreetingFieldsTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        let preset = root.path().join(typography_preset::PRESET_NAME);
        present_with_preset(&app, None, preset.clone());
        let window = app.active_window().unwrap();
        window.set_default_size(1320, 850);
        let this = controller(&window);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-greeting", None);
        this.preview_scene_selector.set_selected(3);
        this.greeting.preset.set_selected(0); // retained basic/custom editor
        this.greeting.enabled.set_active(true);
        settle();
        let original_layout = this.layout_settings();
        open_custom(&this, Info::Cpu);
        let inspector = &this.greeting.fields;
        inspector.label.set_text("Processor");
        inspector.icon.set_text("+");
        inspector.key_color.set_selected(5);
        inspector.value_color.set_selected(3);
        inspector.format.set_selected(2);
        wait_official(&this);
        let styled = this.greeting.settings();
        assert!(feed(&this).contains("+ Processor"));
        assert!(
            feed(&this).contains("C / ") && feed(&this).contains("T @ "),
            "the selected detail format must remain visible at the default preview width"
        );
        assert!(
            styled
                .fastfetch_config()
                .unwrap()
                .contains("cores-physical")
        );
        assert_eq!(this.layout_settings(), original_layout);
        capture(&window);
        this.greeting.undo();
        assert_ne!(this.greeting.settings(), styled);
        this.greeting.redo();
        assert_eq!(this.greeting.settings(), styled);

        inspector.label.set_text("bad\u{202e}name");
        assert!(this.greeting.invalid.get());
        inspector.value_color.set_selected(4);
        assert!(
            this.greeting.invalid.get(),
            "color changes cannot discard invalid drafts"
        );
        this.greeting.columns.set_selected(2);
        assert!(
            this.greeting.invalid.get(),
            "other controls cannot discard invalid fields"
        );
        assert_eq!(this.greeting.settings(), styled);
        this.greeting.undo();
        assert!(!this.greeting.invalid.get());
        assert_eq!(this.greeting.settings(), styled);
        inspector.icon.set_text("\u{10fffd}");
        assert!(
            this.greeting_compatibility_issues()
                .iter()
                .any(|i| i.code == "greeting-font")
        );
        inspector.icon.set_text("+");
        assert!(
            !this
                .greeting_compatibility_issues()
                .iter()
                .any(|i| i.code == "greeting-font")
        );
        inspector.popover.popdown();

        // Changing source invalidates the old popup anchor; returning must open
        // a freshly bound inspector, including for the same CPU button.
        this.greeting.preset.set_selected(1);
        settle();
        this.greeting.preset.set_selected(0);
        settle();
        open_custom(&this, Info::Cpu);
        assert_eq!(inspector.label.text().as_str(), "Processor");
        inspector.reset.emit_clicked();
        assert!(
            !this
                .greeting
                .settings()
                .field_styles
                .contains_key("custom:4")
        );
        inspector.popover.popdown();

        let target = root.path().join("config.jsonc");
        let sentinel = root.path().join("MUST_NOT_RUN");
        let source = format!(
            "// keep this comment\n{{\n  \"future\": {{\"revision\":42}},\n  \"modules\": [{{\"type\":\"cpu\",\"future\":7}}, /* duplicate */ {{\"type\":\"cpu\",\"key\":\"Second CPU\"}}, {{\"type\":\"command\",\"text\":\"touch {}\"}}, \"memory\"],\n}}\n",
            sentinel.display()
        );
        std::fs::write(&target, &source).unwrap();
        this.load_fastfetch_path(target.clone());
        respond("Replace Changes");
        wait_official(&this);
        assert!(this.greeting.settings().imported_source.is_some());
        assert!(!this.greeting.appearance_group.is_visible());
        assert!(!this.greeting.enabled.is_sensitive());
        let first = this
            .greeting
            .imported_list
            .first_child()
            .unwrap()
            .downcast::<gtk::MenuButton>()
            .unwrap();
        this.greeting.select_field(0, &first);
        first.popup();
        settle();
        inspector.label.set_text("My CPU");
        wait_official(&this);
        let edited = this.greeting.settings().fastfetch_config().unwrap();
        assert!(edited.contains("// keep this comment"));
        assert!(edited.contains("/* duplicate */"));
        assert!(edited.contains("\"future\":7"));
        assert!(edited.contains("\"key\":\"Second CPU\""));
        assert!(feed(&this).contains("My CPU"));
        this.greeting_compatibility_issues();
        assert!(
            this.greeting
                .last_report
                .borrow()
                .iter()
                .any(|s| s.contains("command"))
        );
        assert!(!sentinel.exists());
        inspector.popover.popdown();

        this.request_fastfetch_apply();
        crate::fastfetch_run::LAUNCHES.with(|runs| runs.borrow_mut().clear());
        super::super::tests::select_scheme_greeting();
        respond("Cancel");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), source);
        assert!(!this.greeting.fastfetch_state.exists());
        this.request_fastfetch_apply();
        std::fs::write(&target, "// external\n{}").unwrap();
        super::super::tests::select_scheme_greeting();
        respond("Back Up & Apply Selected");
        respond("Close");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "// external\n{}");
        std::fs::write(&target, &source).unwrap();
        this.request_fastfetch_apply();
        super::super::tests::select_scheme_greeting();
        respond("Back Up & Apply Selected");
        respond("Close");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), edited);
        assert!(!sentinel.exists());
        assert!(crate::fastfetch_run::LAUNCHES.with(|runs| runs.borrow().is_empty()));
        this.save_greeting_preset();
        let saved = this.greeting.settings();
        let workspace = root.path().join("fields.termimochi.json");
        this.save_workspace_path(workspace.clone(), this.workspace_snapshot())
            .unwrap();
        window.destroy();
        present_with_preset(&app, None, preset);
        let window = app.active_window().unwrap();
        let this = controller(&window);
        assert_eq!(this.greeting.settings(), saved);
        assert!(
            this.greeting.fastfetch_target.borrow().is_none(),
            "portable presets must not retain write authority"
        );
        this.open_workspace_path(&workspace);
        assert_eq!(this.greeting.settings(), saved);
        this.show_last_scheme_application();
        respond("Restore This Application…");
        respond("Restore Changes");
        respond("Close");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), source);
        assert!(crate::fastfetch_apply::prepare_restore(&this.greeting.fastfetch_state).is_err());
        assert!(!sentinel.exists());
        window.destroy();
    }
}
