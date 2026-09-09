//! Select a color by its use, while editing the single authoritative palette.
use super::*;

pub(super) struct ColorTargets {
    pub root: gtk::Box,
    scope: gtk::DropDown,
    role: gtk::DropDown,
    note: gtk::Label,
    open: gtk::Button,
    syncing: Cell<bool>,
}

impl ColorTargets {
    pub fn reset_scope(&self) {
        self.scope.set_selected(0);
    }
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let scope = gtk::DropDown::from_strings(&["Terminal", "Prompt Designer", "Greeting"]);
        scope.set_hexpand(true);
        scope.update_property(&[gtk::accessible::Property::Label("Color Target")]);
        let role = gtk::DropDown::from_strings(&[]);
        role.set_hexpand(true);
        role.update_property(&[gtk::accessible::Property::Label("Color Role")]);
        let note = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .max_width_chars(36)
            .css_classes(["dim-label"])
            .build();
        let open = gtk::Button::builder()
            .label("Edit source…")
            .halign(gtk::Align::Start)
            .css_classes(["flat"])
            .build();
        root.append(&scope);
        root.append(&role);
        root.append(&note);
        root.append(&open);
        role.set_visible(false);
        note.set_visible(false);
        open.set_visible(false);
        Self {
            root,
            scope,
            role,
            note,
            open,
            syncing: Cell::new(false),
        }
    }
}

fn tone_slot(tone: PreviewTone, bright: bool) -> u8 {
    let slot = match tone {
        PreviewTone::Cyan => 6,
        PreviewTone::Blue => 4,
        PreviewTone::Magenta => 5,
        PreviewTone::Yellow => 3,
        PreviewTone::Green => 2,
        PreviewTone::Red => 1,
    };
    slot + if bright { 8 } else { 0 }
}

impl Workbench {
    pub(super) fn connect_color_targets(this: &Rc<Self>) {
        let weak = Rc::downgrade(this);
        this.color_targets.scope.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                this.refresh_color_targets(true);
            }
        });
        let weak = Rc::downgrade(this);
        this.color_targets.role.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade()
                && !this.color_targets.syncing.get()
            {
                this.refresh_color_targets(false);
            }
        });
        let weak = Rc::downgrade(this);
        this.color_targets.open.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                let target = if this.color_targets.scope.selected() == 1 {
                    PromptSegmentKind::ALL
                        .get(this.color_targets.role.selected() as usize)
                        .copied()
                        .map(PreviewTarget::PromptSegment)
                        .unwrap_or(PreviewTarget::PromptCopy)
                } else {
                    PreviewTarget::GreetingFields
                };
                this.inspect_preview_target(target);
            }
        });
    }

    pub(super) fn refresh_color_targets(&self, rebuild: bool) {
        let controls = &self.color_targets;
        if controls.syncing.replace(true) {
            return;
        }
        let scope = controls.scope.selected();
        controls.role.set_visible(scope != 0);
        controls.note.set_visible(scope != 0);
        controls.open.set_visible(scope != 0);
        if rebuild {
            let names: Vec<&str> = match scope {
                1 => PromptSegmentKind::ALL
                    .iter()
                    .map(|kind| kind.label())
                    .collect(),
                2 => vec!["Accent", "Content text"],
                _ => vec![],
            };
            controls.role.set_model(Some(&gtk::StringList::new(&names)));
            controls.role.set_selected(0);
        }
        let key = match scope {
            1 => PromptSegmentKind::ALL
                .get(controls.role.selected() as usize)
                .and_then(|kind| {
                    self.prompt_settings
                        .borrow()
                        .segments()
                        .iter()
                        .find(|segment| segment.kind == *kind)
                        .map(|segment| {
                            format!(
                                "Color{}",
                                tone_slot(segment.tone, self.preview_terminal.is_bold_is_bright())
                            )
                        })
                }),
            2 => {
                let settings = self.greeting.settings();
                if settings.imported_source.is_some() || !settings.field_styles.is_empty() {
                    None
                } else if controls.role.selected() == 1 || settings.accent == 16 {
                    Some("Foreground".into())
                } else {
                    Some(format!("Color{}", settings.accent))
                }
            }
            _ => None,
        };
        self.color_picker
            .widget()
            .set_sensitive(key.is_some() || scope == 0);
        if let Some(key) = key {
            self.select_color(&key);
            controls.note.set_text(&format!(
                "Shared theme color · {}",
                color_display_name(&key)
            ));
            controls.note.set_tooltip_text(Some("Changing this color affects every terminal, prompt or greeting element using the same palette slot. Edit source to change the binding instead."));
        } else if scope != 0 {
            controls
                .note
                .set_text("This source has independent styles. Edit them in the source editor.");
            controls.note.set_tooltip_text(Some("Imported RGB colors and per-field overrides are preserved; they are not replaced with a guessed theme slot."));
        }
        controls.syncing.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires GTK/VTE; shared color ownership and undo, imported-style fallback"]
    fn shared_color_targets_edit_palette_without_overwriting_sources() {
        use crate::window::greeting::tests::{controller, settle};
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.ColorTargetsTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let this = controller(&window);
        settle();
        let prompt = this.prompt_settings.borrow().clone();
        let greeting = this.greeting.settings();
        this.color_targets.scope.set_selected(1);
        this.color_targets.role.set_selected(2);
        settle();
        let key = this.selected_color_key.borrow().clone();
        assert!(key.starts_with("Color"));
        let before = this.model.borrow().palette.clone();
        this.apply_color(&key, Rgb::new(13, 87, 129));
        assert_ne!(this.model.borrow().palette, before);
        assert_eq!(*this.prompt_settings.borrow(), prompt);
        assert_eq!(this.greeting.settings(), greeting);
        this.undo_action.activate(None);
        assert_eq!(this.model.borrow().palette, before);
        let mut imported = greeting.clone();
        imported.imported_source = Some(
            r##"{"logo":{"type":"none"},"modules":[{"type":"os","keyColor":"#123456"}]}"##.into(),
        );
        this.greeting.replace(imported.clone(), true);
        this.color_targets.scope.set_selected(2);
        settle();
        assert!(!this.color_picker.widget().is_sensitive());
        assert_eq!(this.greeting.settings(), imported);
        this.controls["Foreground"].swatch.button().emit_clicked();
        assert_eq!(this.color_targets.scope.selected(), 0);
        assert!(this.color_picker.widget().is_sensitive());
        assert_eq!(this.greeting.settings(), imported);
        window.destroy();
    }
    #[test]
    fn prompt_binding_tracks_bold_bright_palette_slots() {
        for (tone, expected) in [
            (PreviewTone::Cyan, 6),
            (PreviewTone::Blue, 4),
            (PreviewTone::Magenta, 5),
            (PreviewTone::Yellow, 3),
            (PreviewTone::Green, 2),
            (PreviewTone::Red, 1),
        ] {
            assert_eq!(tone_slot(tone, false), expected);
            assert_eq!(tone_slot(tone, true), expected + 8);
        }
    }
}
