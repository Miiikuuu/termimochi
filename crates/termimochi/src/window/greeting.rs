//! Greeting editor and contextual file actions. Shares the mounted VTE.
use super::*;
use crate::greeting::{
    GreetingPreset, GreetingSettings, Info, Logo, Opening, PRESET_NAME, Position,
};
use crate::greeting_official::{OfficialItem, OfficialPreset};
mod art_preview;
mod artwork;
mod fastfetch;
mod fields;
mod image_import;
mod pixel_export;
mod presentation;
mod startup;
mod sync;

pub(super) type OfficialKey = (Option<OfficialPreset>, String);
type OfficialRow = (u16, gtk::Box, gtk::Switch, gtk::Button, gtk::Button);

const WIDTHS: [u16; 4] = [0, 80, 100, 120];

fn move_official_item(
    items: &mut Vec<OfficialItem>,
    source: u16,
    target: u16,
    after: bool,
) -> bool {
    if source == target {
        return false;
    }
    let Some(from) = items.iter().position(|i| i.id == source) else {
        return false;
    };
    let Some(to) = items.iter().position(|i| i.id == target) else {
        return false;
    };
    let insertion = to + usize::from(after);
    let destination = insertion - usize::from(from < insertion);
    if destination == from {
        return false;
    }
    let item = items.remove(from);
    items.insert(destination, item);
    true
}

impl HistorySnapshot for GreetingSettings {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn set_context(&mut self, (): Self::Context) {}
}

type Changed = Box<dyn Fn()>;
pub(super) struct GreetingEditor {
    pub presentation: presentation::PresentationEditor,
    pub root: gtk::ScrolledWindow,
    fields: fields::FieldInspector,
    appearance_group: gtk::Box,
    imported_list: gtk::Box,
    imported_signature: RefCell<Vec<String>>,
    compatibility: gtk::Expander,
    report_body: gtk::Box,
    last_report: RefCell<Vec<String>>,
    fastfetch_target: RefCell<Option<crate::fastfetch_apply::Target>>,
    fastfetch_state: PathBuf,
    sync: sync::SyncNotice,
    enabled: gtk::Switch,
    preset: gtk::DropDown,
    preset_note: gtk::Label,
    artwork_size: gtk::Label,
    edit_image: gtk::Button,
    official_list: gtk::Box,
    official_rows: RefCell<Vec<OfficialRow>>,
    shown_official: Cell<Option<OfficialPreset>>,
    official_dragging: Cell<Option<u16>>,
    weak: std::rc::Weak<Self>,
    logo: gtk::DropDown,
    position: gtk::DropDown,
    accent: gtk::DropDown,
    gap: gtk::SpinButton,
    columns: gtk::DropDown,
    opening: gtk::DropDown,
    replay: gtk::Button,
    message: gtk::Entry,
    artwork: gtk::TextView,
    artwork_scroll: gtk::ScrolledWindow,
    artwork_view: gtk::Stack,
    artwork_card: gtk::Box,
    pub artwork_preview: Rc<art_preview::ArtPreview>,
    image_import_window: glib::WeakRef<gtk::Window>,
    pixel_export_window: glib::WeakRef<gtk::Window>,
    list: gtk::Box,
    rows: Vec<(Info, gtk::Box, gtk::Switch, gtk::Button, gtk::Button)>,
    pub status: gtk::Label,
    settings: RefCell<GreetingSettings>,
    pub baseline: RefCell<GreetingSettings>,
    pub history: RefCell<EditHistory<GreetingSettings>>,
    store: RefCell<Option<DocumentStore<GreetingPreset>>>,
    saved: Cell<bool>,
    pub invalid: Cell<bool>,
    updating: Cell<bool>,
    changed: RefCell<Option<Changed>>,
    revision: Cell<u64>,
    dragging: Cell<Option<Info>>,
}

impl GreetingEditor {
    pub(super) fn inspection_control(&self, target: PreviewTarget) -> Option<gtk::Widget> {
        fn first_control(root: &gtk::Widget) -> Option<gtk::Widget> {
            if !root.is_visible() || !root.is_sensitive() {
                return None;
            }
            if root.is_focusable() {
                return Some(root.clone());
            }
            let mut child = root.first_child();
            while let Some(widget) = child {
                if let Some(control) = first_control(&widget) {
                    return Some(control);
                }
                child = widget.next_sibling();
            }
            None
        }
        match target {
            PreviewTarget::GreetingArtwork => {
                if self.edit_image.is_visible() && self.edit_image.is_sensitive() {
                    Some(self.edit_image.clone().upcast())
                } else if self.artwork.is_editable()
                    && self.artwork_view.visible_child_name().as_deref() == Some("text")
                {
                    Some(self.artwork.clone().upcast())
                } else {
                    Some(self.logo.clone().upcast())
                }
            }
            PreviewTarget::GreetingMessage => Some(self.message.clone().upcast()),
            PreviewTarget::GreetingField(kind) => self
                .rows
                .iter()
                .find(|(k, ..)| *k == kind)
                .and_then(|(_, row, ..)| first_control(row.upcast_ref())),
            PreviewTarget::GreetingFields => [&self.imported_list, &self.official_list, &self.list]
                .into_iter()
                .find_map(|list| first_control(list.upcast_ref()))
                .or_else(|| Some(self.preset.clone().upcast())),
            _ => Some(self.preset.clone().upcast()),
        }
    }
    pub fn new(path: PathBuf) -> Rc<Self> {
        let fastfetch_state = path
            .parent()
            .unwrap_or(Path::new("."))
            .join("fastfetch-state");
        let loaded = DocumentStore::<GreetingPreset>::open(path).and_then(|store| {
            let preset = store.document()?;
            Ok((store, preset))
        });
        let (store, settings, saved, notice) = match loaded {
            Ok((store, preset)) => {
                let saved = preset.is_some();
                (
                    Some(store),
                    preset
                        .map(|p| p.greeting)
                        .unwrap_or_else(GreetingSettings::starter),
                    saved,
                    None,
                )
            }
            Err(error) => (None, GreetingSettings::starter(), false, Some(error)),
        };
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(15);
        content.set_margin_end(15);
        let heading = gtk::Label::new(Some("Greeting"));
        heading.set_xalign(0.0);
        heading.set_hexpand(true);
        heading.add_css_class("preview-heading");
        let enabled = layout_switch(
            settings.enabled,
            "Enable Greeting",
            "Include the greeting in a fresh-terminal preview and exports. Does not install a startup hook.",
        );
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        header.append(&heading);
        header.append(&enabled);
        content.append(&header);
        let mut presets = vec!["Custom design"];
        presets.extend(OfficialPreset::CHOICES.map(OfficialPreset::label));
        let preset = layout_drop_down(
            &presets,
            settings
                .official_preset
                .map_or(0, OfficialPreset::selector_index),
            "Greeting Starting Point",
            "TermiMochi and five reviewed Fastfetch presets. Full native fields, editable formats and undoable switching.",
        );
        content.append(&layout_row("Preset", &preset));
        // Configuration commands are collected in the fixed output bar.
        let sync = sync::SyncNotice::new();
        content.append(&sync.root);
        let preset_note = gtk::Label::new(None);
        preset_note.set_xalign(0.0);
        preset_note.set_wrap(true);
        preset_note.add_css_class("dim-label");
        content.append(&preset_note);
        let message = gtk::Entry::builder()
            .hexpand(true)
            .enable_undo(false)
            .max_length(161)
            .placeholder_text("Welcome text (optional)")
            .text(&settings.message)
            .build();
        message.add_css_class("typography-control");
        message.update_property(&[gtk::accessible::Property::Label("Welcome Text")]);
        content.append(&message);
        let presentation = presentation::PresentationEditor::new(
            fastfetch_state
                .parent()
                .unwrap()
                .join("greeting-target.json"),
        );
        content.append(&presentation.root);
        let logo = layout_drop_down(
            &Logo::ALL.map(Logo::label),
            settings.logo.index(),
            "Greeting Artwork",
            "Built-in artwork, text, or an image converted to colored characters. No commands are run.",
        );
        let position = layout_drop_down(
            &Position::ALL.map(Position::label),
            settings.position.index(),
            "Greeting Layout",
            "Horizontal, stacked or a borderless minimal card. Narrow terminals stack artwork automatically.",
        );
        let mut colors = ANSI_NAMES.to_vec();
        colors.push("Text");
        let accent = layout_drop_down(
            &colors,
            settings.accent.into(),
            "Greeting Accent",
            "Uses the matching color from your terminal palette.",
        );
        let gap = typography_spin_button(
            settings.gap.into(),
            0.0,
            8.0,
            1.0,
            0,
            "Artwork Gap",
            "Spacing in terminal cells",
        );
        let appearance_group = layout_group(
            "Appearance",
            [
                layout_row("Artwork", &logo),
                layout_row("Layout", &position),
                layout_row("Accent", &accent),
                layout_row("Gap", &gap),
            ],
        );
        content.append(&appearance_group);
        let artwork_size = gtk::Label::new(None);
        artwork_size.set_xalign(0.0);
        artwork_size.add_css_class("dim-label");
        artwork_size.set_tooltip_text(Some("Exported character dimensions. The thumbnail fits automatically; click to inspect at a larger size."));
        let art_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let import_content = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        import_content.append(&gtk::Image::from_icon_name("document-open-symbolic"));
        let import_label = gtk::Label::new(Some("Add image / GIF…"));
        import_label.set_hexpand(true);
        import_label.set_xalign(0.0);
        import_content.append(&import_label);
        let import_art = gtk::Button::builder()
            .child(&import_content)
            .action_name("win.import-greeting-art")
            .css_classes(["artwork-import"])
            .hexpand(true)
            .build();
        import_art.update_property(&[gtk::accessible::Property::Label("Add image or GIF")]);
        import_art.set_tooltip_text(Some(
            "Choose PNG, JPG, WebP, SVG, GIF, TXT or ANSI artwork.",
        ));
        art_actions.append(&import_art);
        let art_menu = gio::Menu::new();
        for (label, action) in [
            ("Export Plain Text…", "win.export-greeting-txt"),
            ("Export ANSI…", "win.export-greeting-ans"),
            ("Edit as Plain Text", "win.edit-greeting-art-text"),
        ] {
            art_menu.append(Some(label), Some(action));
        }
        let art_files = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .menu_model(&art_menu)
            .halign(gtk::Align::End)
            .build();
        art_files.add_css_class("flat");
        art_files.add_css_class("artwork-more");
        art_files.update_property(&[gtk::accessible::Property::Label(
            "Artwork Export and Editing Options",
        )]);
        art_files.set_tooltip_text(Some(
            "Export ANSI or plain text, or remove colors for manual text editing.",
        ));
        art_actions.append(&art_files);
        content.append(&art_actions);
        let edit_image = gtk::Button::builder()
            .label("Edit Artwork…")
            .action_name("win.edit-image-artwork")
            .css_classes(["flat"])
            .build();
        content.append(&edit_image);
        content.reorder_child_after(&art_actions, Some(&message));
        content.reorder_child_after(&edit_image, Some(&art_actions));
        let artwork = gtk::TextView::builder()
            .monospace(true)
            .wrap_mode(gtk::WrapMode::None)
            .top_margin(8)
            .bottom_margin(8)
            .left_margin(8)
            .right_margin(8)
            .build();
        artwork.buffer().set_enable_undo(false);
        artwork.buffer().set_text(&settings.custom_logo);
        artwork.update_property(&[gtk::accessible::Property::Label("Custom Greeting Artwork")]);
        artwork.set_tooltip_text(Some("Up to 96 lines, 160 cells per line and 16 KiB. Plain text and emoji; terminal control sequences are rejected."));
        let artwork_scroll = gtk::ScrolledWindow::builder()
            .min_content_height(160)
            .max_content_height(240)
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&artwork)
            .build();
        let artwork_preview = art_preview::ArtPreview::new(false);
        let artwork_view = gtk::Stack::builder()
            .hhomogeneous(false)
            .vhomogeneous(false)
            .build();
        artwork_view.add_named(&artwork_scroll, Some("text"));
        artwork_view.add_named(&artwork_preview.viewport, Some("preview"));
        let artwork_card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let art_caption = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        artwork_size.set_hexpand(true);
        art_caption.append(&artwork_size);
        let enlarge = gtk::Button::from_icon_name("view-fullscreen-symbolic");
        enlarge.add_css_class("flat");
        enlarge.set_tooltip_text(Some("Enlarge artwork preview"));
        enlarge.update_property(&[gtk::accessible::Property::Label("Enlarge Artwork Preview")]);
        let weak = Rc::downgrade(&artwork_preview);
        enlarge.connect_clicked(move |_| {
            if let Some(preview) = weak.upgrade() {
                preview.expand();
            }
        });
        art_caption.append(&enlarge);
        artwork_card.append(&art_caption);
        artwork_card.append(&artwork_view);
        content.append(&artwork_card);
        let columns = layout_drop_down(
            &["Follow Layout", "80 columns", "100 columns", "120 columns"],
            WIDTHS
                .iter()
                .position(|w| *w == settings.preview_columns)
                .unwrap_or(0) as u32,
            "Greeting Preview Width",
            "Exact terminal columns. Pan with Shift+wheel if needed; does not change your Layout settings.",
        );
        let opening = layout_drop_down(
            &Opening::ALL.map(Opening::label),
            settings.opening.index(),
            "Greeting Opening",
            "Preview-only motion. Fastfetch config.jsonc exports are static. Respects reduced motion.",
        );
        let replay = gtk::Button::from_icon_name("media-playback-start-symbolic");
        replay.add_css_class("flat");
        replay.set_tooltip_text(Some("Replay opening · preview only"));
        replay.update_property(&[gtk::accessible::Property::Label("Replay Greeting Opening")]);
        let motion = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        motion.append(&opening);
        motion.append(&replay);
        content.append(&layout_group(
            "Demo settings · preview only",
            [
                layout_row("Width", &columns),
                layout_row("Opening¹", &motion),
            ],
        ));
        let motion_note = gtk::Label::new(Some("¹ Motion is preview only · exports are static"));
        motion_note.set_xalign(0.0);
        motion_note.add_css_class("dim-label");
        motion_note.set_wrap(true);
        content.append(&motion_note);
        let title = gtk::Label::new(Some("System Information"));
        title.set_xalign(0.0);
        title.add_css_class("layout-group-title");
        content.append(&title);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 1);
        let mut rows = Vec::new();
        for kind in Info::ALL {
            let toggle = layout_switch(
                false,
                &format!("Show {}", kind.label()),
                "Include this information item",
            );
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("greeting-info-row");
            let handle = gtk::Image::from_icon_name("list-drag-handle-symbolic");
            handle.set_pixel_size(16);
            handle.set_cursor_from_name(Some("grab"));
            handle.set_tooltip_text(Some("Drag to reorder · or use the arrow buttons"));
            row.append(&handle);
            let label = fields::field_button(kind.label());
            let up = gtk::Button::from_icon_name("go-up-symbolic");
            let down = gtk::Button::from_icon_name("go-down-symbolic");
            for (button, direction) in [(&up, "up"), (&down, "down")] {
                button.add_css_class("flat");
                button.set_tooltip_text(Some(&format!("Move {} {direction}", kind.label())));
                button.update_property(&[gtk::accessible::Property::Label(&format!(
                    "Move {} {direction}",
                    kind.label()
                ))]);
            }
            row.append(&label);
            row.append(&up);
            row.append(&down);
            row.append(&toggle);
            list.append(&row);
            rows.push((kind, row, toggle, up, down));
        }
        content.append(&list);
        let official_list = gtk::Box::new(gtk::Orientation::Vertical, 1);
        content.append(&official_list);
        let imported_list = gtk::Box::new(gtk::Orientation::Vertical, 1);
        content.append(&imported_list);
        let fields = fields::FieldInspector::new();
        let report_body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let compatibility = gtk::Expander::builder()
            .label("Compatibility")
            .child(&report_body)
            .expanded(false)
            .build();
        content.append(&compatibility);
        // Save/export actions are collected in the fixed output bar.
        let status = gtk::Label::new(None);
        status.set_xalign(0.0);
        status.set_wrap(true);
        status.add_css_class("dim-label");
        content.append(&status);
        // Snapshot refresh is available in the output bar's More menu.
        let root = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_width(330)
            .css_classes(["editor-scroll"])
            .child(&content)
            .build();
        let this = Rc::new_cyclic(|weak| Self {
            presentation,
            root,
            fields,
            appearance_group,
            imported_list,
            imported_signature: RefCell::new(Vec::new()),
            compatibility,
            report_body,
            last_report: RefCell::new(Vec::new()),
            fastfetch_target: RefCell::new(None),
            fastfetch_state,
            sync,
            enabled,
            preset,
            preset_note,
            artwork_size,
            edit_image,
            official_list,
            official_rows: RefCell::new(Vec::new()),
            shown_official: Cell::new(None),
            official_dragging: Cell::new(None),
            weak: weak.clone(),
            logo,
            position,
            accent,
            gap,
            columns,
            opening,
            replay,
            message,
            artwork,
            artwork_scroll,
            artwork_view,
            artwork_card,
            artwork_preview,
            image_import_window: glib::WeakRef::new(),
            pixel_export_window: glib::WeakRef::new(),
            list,
            rows,
            status,
            settings: RefCell::new(settings.clone()),
            baseline: RefCell::new(settings),
            history: RefCell::new(EditHistory::new(EDIT_HISTORY_LIMIT)),
            store: RefCell::new(store),
            saved: Cell::new(saved),
            invalid: Cell::new(false),
            updating: Cell::new(false),
            changed: RefCell::new(None),
            revision: Cell::new(0),
            dragging: Cell::new(None),
        });
        this.refresh();
        Self::connect(&this);
        Self::connect_field_inspector(&this);
        Self::connect_presentation(&this);
        if let Some(notice) = notice {
            this.status
                .set_text(&format!("Saved greeting unavailable: {notice}"));
        }
        this
    }
    pub fn settings(&self) -> GreetingSettings {
        self.settings.borrow().clone()
    }
    pub fn dirty(&self) -> bool {
        self.invalid.get() || *self.settings.borrow() != *self.baseline.borrow()
    }
    pub fn finish(&self) {
        if !self.invalid.get() {
            self.history.borrow_mut().commit(self.settings());
        }
    }
    pub(super) fn retain_applied_preset(&self) -> Result<(), String> {
        self.persist().inspect_err(|error| {
            self.sync.save_failed("Greeting applied; local preset could not be saved. Save the complete scheme to keep the design.", error);
        })
    }
    fn persist(&self) -> Result<(), String> {
        self.finish();
        if self.invalid.get() {
            return Err("Fix the greeting fields before saving.".into());
        }
        let settings = self.settings();
        self.store
            .borrow_mut()
            .as_mut()
            .ok_or("The saved preset is unavailable. Export a separate preset instead.")?
            .save(&GreetingPreset::new(settings.clone()))?;
        *self.baseline.borrow_mut() = settings;
        self.saved.set(true);
        self.sync.save_failure.set_visible(false);
        self.sync.refresh_visibility();
        self.refresh_status();
        Ok(())
    }
    fn notify(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
        if let Some(changed) = self.changed.borrow().as_ref() {
            changed();
        }
    }
    pub fn connect_changed(&self, changed: impl Fn() + 'static) {
        *self.changed.borrow_mut() = Some(Box::new(changed));
    }
    pub fn replace(&self, settings: GreetingSettings, record: bool) {
        self.finish();
        if record && settings != self.settings() {
            let mut history = self.history.borrow_mut();
            history.begin(self.settings());
            history.mark_changed();
            history.commit(settings.clone());
        }
        *self.settings.borrow_mut() = settings;
        self.invalid.set(false);
        self.refresh();
        self.notify();
    }
    pub fn undo(&self) {
        if self.invalid.replace(false) {
            // TextBuffer replacement first deletes the old contents, then
            // inserts the paste. Do not expose that intermediate empty value
            // when rejecting an invalid paste and undoing its transaction.
            let current = self.settings();
            let target = self
                .history
                .borrow_mut()
                .undo_pending(current.clone())
                .unwrap_or(current);
            self.replace(target, false);
            return;
        }
        let target = self.history.borrow_mut().undo(self.settings());
        if let Some(target) = target {
            self.replace(target, false);
        }
    }
    pub fn redo(&self) {
        if self.invalid.get() {
            return;
        }
        let target = self.history.borrow_mut().redo(self.settings());
        if let Some(target) = target {
            self.replace(target, false);
        }
    }
    fn refresh(&self) {
        self.updating.set(true);
        let settings = self.settings();
        self.presentation.refresh(&settings);
        self.refresh_pixel_design();
        self.enabled.set_active(settings.enabled);
        let mut labels = vec!["Custom design"];
        labels.extend(OfficialPreset::CHOICES.map(OfficialPreset::label));
        if settings.imported_source.is_some() {
            labels.push("Imported Fastfetch");
        }
        if self
            .preset
            .model()
            .is_none_or(|m| m.n_items() as usize != labels.len())
        {
            self.preset.set_model(Some(&gtk::StringList::new(&labels)));
        }
        self.preset
            .set_selected(if settings.imported_source.is_some() {
                OfficialPreset::IMPORTED_INDEX
            } else {
                settings
                    .official_preset
                    .map_or(0, OfficialPreset::selector_index)
            });
        self.logo.set_selected(settings.logo.index());
        self.position.set_selected(settings.position.index());
        self.accent.set_selected(settings.accent.into());
        self.gap.set_value(settings.gap.into());
        self.columns.set_selected(
            WIDTHS
                .iter()
                .position(|w| *w == settings.preview_columns)
                .unwrap_or(0) as u32,
        );
        self.opening.set_selected(settings.opening.index());
        self.replay
            .set_sensitive(settings.enabled && settings.opening != Opening::None);
        self.logo.set_sensitive(settings.position != Position::Card);
        self.gap.set_sensitive(matches!(
            settings.position,
            Position::Left | Position::Right
        ));
        self.message.set_text(&settings.message);
        self.artwork.buffer().set_text(&settings.custom_logo);
        let colored = settings
            .custom_art
            .as_ref()
            .is_some_and(|art| art.ansi.contains('\x1b'));
        self.artwork.set_editable(!colored);
        self.artwork.set_tooltip_text(Some(if colored {"Colors are preserved. Artwork menu → Edit as Plain Text removes colors and enables editing; Undo restores them."}else{"UTF-8 artwork: up to 96 lines, 160 cells per line and 16 KiB."}));
        self.message.remove_css_class("error");
        self.artwork.remove_css_class("error");
        let mut previous: Option<&gtk::Box> = None;
        for (index, item) in settings.items.iter().enumerate() {
            let (_, row, toggle, up, down) = self
                .rows
                .iter()
                .find(|(kind, ..)| *kind == item.kind)
                .unwrap();
            self.list.reorder_child_after(row, previous);
            previous = Some(row);
            toggle.set_active(item.enabled);
            up.set_sensitive(index > 0);
            down.set_sensitive(index + 1 < settings.items.len());
        }
        self.refresh_status();
        self.refresh_artwork_info();
        self.refresh_official_fields();
        self.refresh_imported_fields();
        self.refresh_field_inspector();
        self.updating.set(false);
    }
    pub fn refresh_status(&self) {
        if self.invalid.get() {
            return;
        }
        self.status.set_text(if self.dirty() {
            "Unsaved greeting changes"
        } else if self.saved.get() {
            "Preset saved in TermiMochi"
        } else if !self.settings.borrow().enabled {
            "Greeting is off · enable above to preview"
        } else {
            "Preview only · not saved"
        });
        self.status.set_tooltip_text(Some("Save remembers this design inside TermiMochi. Apply in the bottom bar reviews and updates external Fastfetch and can run it once in a new terminal. Shell startup stays unchanged."));
    }
    fn refresh_artwork_info(&self) {
        use unicode_width::UnicodeWidthStr;
        let settings = self.settings();
        self.edit_image
            .set_label(if settings.editable_artwork.is_some() {
                "Edit Artwork…"
            } else {
                "Reimport Original…"
            });
        self.edit_image.set_tooltip_text(Some(if settings.editable_artwork.is_some() {
            "Continue editing the embedded original with your saved conversion settings."
        } else {
            "No editable source is saved. Choose the original image to create one; old conversion settings cannot be recovered."
        }));
        self.edit_image.set_visible(
            settings.editable_artwork.is_some()
                || (settings.logo == Logo::Custom && settings.custom_art.is_some())
                || settings.imported_source.is_some(),
        );
        let rendered = if let Some(source) = &settings.imported_source {
            crate::fastfetch_document::value(source)
                .ok()
                .and_then(|value| {
                    crate::greeting_art::text_logo(
                        &value["logo"],
                        settings.source_logo.as_ref(),
                        &mut Vec::new(),
                    )
                    .ok()
                })
        } else {
            crate::greeting_art::Artwork::parse(&settings.artwork_ansi()).ok()
        };
        let art = rendered
            .as_ref()
            .map(|art| art.plain.as_str())
            .unwrap_or("");
        let columns = art.lines().map(str::width).max().unwrap_or(0);
        self.artwork_size.set_text(&format!(
            "{} × {} cells{}",
            columns,
            art.lines().count(),
            if matches!(
                settings.logo,
                Logo::Ubuntu | Logo::Arch | Logo::Debian | Logo::Fedora | Logo::LinuxMint
            ) {
                " · Fastfetch original"
            } else {
                ""
            }
        ));
        self.artwork_size.set_visible(!art.is_empty());
        let editing = settings.imported_source.is_none()
            && settings.logo == Logo::Custom
            && self.artwork.is_editable();
        self.artwork_card.set_visible(
            !art.trim().is_empty() || (editing && settings.position != Position::Card),
        );
        self.artwork_scroll.set_visible(true);
        self.artwork_view
            .set_visible_child_name(if editing { "text" } else { "preview" });
        self.artwork_preview
            .set_art(rendered.filter(|art| !art.plain.trim().is_empty()));
    }
    fn refresh_official_fields(&self) {
        let settings = self.settings();
        self.list
            .set_visible(settings.official_preset.is_none() && settings.imported_source.is_none());
        self.official_list
            .set_visible(settings.official_preset.is_some());
        self.preset_note.set_text(match settings.official_preset {
            Some(OfficialPreset::TermiMochi) => "TermiMochi · full native system profile",
            Some(OfficialPreset::Icons) => "Official Example 8 · uses Nerd Font icons",
            Some(_) => "Fastfetch 2.57.1 · original fields and formats",
            None => "Basic custom fields · choose TermiMochi for a complete preset",
        });
        self.preset_note.set_tooltip_text(Some("TermiMochi's own full preset and five bundled upstream presets. Formats are retained when toggling or dragging fields. Desktop-specific facts may be unavailable in the offline sandbox; exports use live Fastfetch detection."));
        if self.shown_official.get() != settings.official_preset {
            self.shown_official.set(settings.official_preset);
            self.official_dragging.set(None);
            while let Some(child) = self.official_list.first_child() {
                self.official_list.remove(&child);
            }
            self.official_rows.borrow_mut().clear();
            if let Some(preset) = settings.official_preset {
                for item in preset.items() {
                    let id = item.id;
                    let label = preset.module_label(id);
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    row.add_css_class("greeting-info-row");
                    let handle = gtk::Image::from_icon_name("list-drag-handle-symbolic");
                    handle.set_cursor_from_name(Some("grab"));
                    handle.set_tooltip_text(Some("Drag to reorder this official field"));
                    let name = fields::field_button(&label);
                    self.bind_field(&name, usize::from(id));
                    let toggle = layout_switch(
                        true,
                        &format!("Show preset {label}"),
                        "Retains the original Fastfetch module and format",
                    );
                    let up = gtk::Button::from_icon_name("go-up-symbolic");
                    let down = gtk::Button::from_icon_name("go-down-symbolic");
                    row.append(&handle);
                    row.append(&name);
                    row.append(&up);
                    row.append(&down);
                    row.append(&toggle);
                    self.official_list.append(&row);
                    let weak = self.weak.clone();
                    toggle.connect_active_notify(move |toggle| {
                        let Some(this) = weak.upgrade() else {
                            return;
                        };
                        if this.updating.get() || this.invalid.get() {
                            return;
                        }
                        let mut settings = this.settings();
                        if let Some(item) = settings.official_items.iter_mut().find(|i| i.id == id)
                        {
                            item.enabled = toggle.is_active();
                            this.replace(settings, true);
                        }
                    });
                    for (button, direction) in [(&up, -1_isize), (&down, 1)] {
                        button.add_css_class("flat");
                        button.set_tooltip_text(Some(&format!(
                            "Move {label} {}",
                            if direction < 0 { "up" } else { "down" }
                        )));
                        let weak = self.weak.clone();
                        button.connect_clicked(move |_| {
                            let Some(this) = weak.upgrade() else {
                                return;
                            };
                            if this.invalid.get() {
                                return;
                            }
                            let mut settings = this.settings();
                            let Some(from) =
                                settings.official_items.iter().position(|i| i.id == id)
                            else {
                                return;
                            };
                            if let Some(to) = from
                                .checked_add_signed(direction)
                                .filter(|to| *to < settings.official_items.len())
                            {
                                settings.official_items.swap(from, to);
                                this.replace(settings, true);
                            }
                        });
                    }
                    self.connect_official_drag(id, &row, &handle);
                    self.official_rows
                        .borrow_mut()
                        .push((id, row, toggle, up, down));
                }
            }
        }
        let rows = self.official_rows.borrow();
        let mut previous: Option<&gtk::Box> = None;
        for (index, item) in settings.official_items.iter().enumerate() {
            let (_, row, toggle, up, down) = rows.iter().find(|r| r.0 == item.id).unwrap();
            self.official_list.reorder_child_after(row, previous);
            previous = Some(row);
            toggle.set_active(item.enabled);
            up.set_sensitive(index > 0);
            down.set_sensitive(index + 1 < settings.official_items.len());
        }
    }
    fn connect_official_drag(&self, id: u16, row: &gtk::Box, handle: &gtk::Image) {
        let source = gtk::DragSource::builder()
            .actions(gdk::DragAction::MOVE)
            .build();
        let weak = self.weak.clone();
        source.connect_prepare(move |_, _, _| {
            let this = weak.upgrade()?;
            if this.invalid.get() {
                return None;
            }
            this.official_dragging.set(Some(id));
            Some(gdk::ContentProvider::for_value(
                &format!("official:{id}").to_value(),
            ))
        });
        let row_weak = row.downgrade();
        source.connect_drag_begin(move |source, _| {
            if let Some(row) = row_weak.upgrade() {
                source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&row))), 0, 0);
                row.set_opacity(0.45);
            }
        });
        let weak = self.weak.clone();
        source.connect_drag_end(move |_, _, _| {
            if let Some(this) = weak.upgrade() {
                this.official_dragging.set(None);
                for (_, row, ..) in this.official_rows.borrow().iter() {
                    row.set_opacity(1.0);
                    row.remove_css_class("greeting-drop-before");
                    row.remove_css_class("greeting-drop-after");
                }
            }
        });
        handle.add_controller(source);
        let target = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
        for enter in [true, false] {
            let weak = self.weak.clone();
            let row_weak = row.downgrade();
            let motion = move |_: &gtk::DropTarget, _: f64, y: f64| {
                let (Some(this), Some(row)) = (weak.upgrade(), row_weak.upgrade()) else {
                    return gdk::DragAction::empty();
                };
                if this.invalid.get()
                    || this
                        .official_dragging
                        .get()
                        .is_none_or(|source| source == id)
                {
                    return gdk::DragAction::empty();
                }
                row.remove_css_class("greeting-drop-before");
                row.remove_css_class("greeting-drop-after");
                row.add_css_class(if y < f64::from(row.height()) / 2.0 {
                    "greeting-drop-before"
                } else {
                    "greeting-drop-after"
                });
                gdk::DragAction::MOVE
            };
            if enter {
                target.connect_enter(motion);
            } else {
                target.connect_motion(motion);
            }
        }
        let row_weak = row.downgrade();
        target.connect_leave(move |_| {
            if let Some(row) = row_weak.upgrade() {
                row.remove_css_class("greeting-drop-before");
                row.remove_css_class("greeting-drop-after");
            }
        });
        let weak = self.weak.clone();
        let row_weak = row.downgrade();
        target.connect_drop(move |_, value, _, y| {
            let (Some(this), Some(row)) = (weak.upgrade(), row_weak.upgrade()) else {
                return false;
            };
            row.remove_css_class("greeting-drop-before");
            row.remove_css_class("greeting-drop-after");
            let Some(source) = this.official_dragging.get() else {
                return false;
            };
            if this.invalid.get()
                || value.get::<String>().ok() != Some(format!("official:{source}"))
            {
                return false;
            }
            let mut settings = this.settings();
            if !move_official_item(
                &mut settings.official_items,
                source,
                id,
                y >= f64::from(row.height()) / 2.0,
            ) {
                return false;
            }
            this.replace(settings, true);
            true
        });
        row.add_controller(target);
    }
    fn read_controls(&self, text_edit: bool) {
        if self.updating.get() || self.invalid_field_draft() {
            return;
        }
        let mut settings = self.settings();
        settings.enabled = self.enabled.is_active();
        let logo = Logo::ALL[(self.logo.selected() as usize).min(Logo::ALL.len() - 1)];
        if settings.logo != logo {
            settings.editable_artwork = None;
            crate::greeting_output::select_character(&mut settings);
        }
        settings.logo = logo;
        settings.position = Position::ALL[self.position.selected().min(3) as usize];
        settings.preview_columns = WIDTHS[self.columns.selected().min(3) as usize];
        settings.opening = Opening::ALL[self.opening.selected().min(3) as usize];
        settings.accent = self.accent.selected().min(16) as u8;
        settings.gap = self.gap.value_as_int().clamp(0, 8) as u8;
        settings.message = self.message.text().into();
        let buffer = self.artwork.buffer();
        let text: String = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .into();
        if text != settings.custom_logo {
            settings.editable_artwork = None;
            crate::greeting_output::select_character(&mut settings);
            settings.custom_art = None;
            settings.custom_logo = text;
        }
        for item in &mut settings.items {
            item.enabled = self
                .rows
                .iter()
                .find(|(kind, ..)| *kind == item.kind)
                .unwrap()
                .2
                .is_active();
        }
        if let Err(error) = settings.validate() {
            self.invalid.set(true);
            self.status.set_text(&error);
            self.message.add_css_class("error");
            self.artwork.add_css_class("error");
            self.notify();
            return;
        }
        self.invalid.set(false);
        self.message.remove_css_class("error");
        self.artwork.remove_css_class("error");
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
        self.replay.set_sensitive(
            self.settings.borrow().enabled && self.settings.borrow().opening != Opening::None,
        );
        self.logo
            .set_sensitive(self.settings.borrow().position != Position::Card);
        self.gap.set_sensitive(matches!(
            self.settings.borrow().position,
            Position::Left | Position::Right
        ));
        self.refresh_status();
        self.refresh_artwork_info();
        self.notify();
    }
    fn connect(this: &Rc<Self>) {
        let weak = Rc::downgrade(this);
        this.preset.connect_selected_notify(move |selector| {
            let Some(this) = weak.upgrade() else {
                return;
            };
            if this.updating.get() {
                return;
            }
            if this.invalid.get() {
                this.status
                    .set_text("Fix or undo invalid input before switching presets.");
                this.updating.set(true);
                let settings = this.settings();
                selector.set_selected(if settings.imported_source.is_some() {
                    OfficialPreset::IMPORTED_INDEX
                } else {
                    settings
                        .official_preset
                        .map_or(0, OfficialPreset::selector_index)
                });
                this.updating.set(false);
                return;
            }
            let mut settings = this.settings();
            if selector.selected() == 0 {
                // Custom controls remain intact while exploring official
                // presets. Returning to them is an explicit, undoable choice.
                settings.official_preset = None;
                settings.official_items.clear();
                settings.imported_source = None;
                settings.source_logo = None;
                settings
                    .field_styles
                    .retain(|key, _| !key.starts_with("imported:"));
            } else if selector.selected() == OfficialPreset::IMPORTED_INDEX
                && settings.imported_source.is_some()
            {
                return;
            } else {
                let Some(preset) = selector
                    .selected()
                    .checked_sub(1)
                    .and_then(|i| OfficialPreset::CHOICES.get(i as usize))
                else {
                    return;
                };
                settings.use_official(*preset);
            }
            this.replace(settings, true);
        });
        for dropdown in [
            &this.logo,
            &this.position,
            &this.accent,
            &this.columns,
            &this.opening,
        ] {
            let weak = Rc::downgrade(this);
            dropdown.connect_selected_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.read_controls(false);
                }
            });
        }
        for switch in std::iter::once(&this.enabled).chain(this.rows.iter().map(|row| &row.2)) {
            let weak = Rc::downgrade(this);
            switch.connect_active_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.read_controls(false);
                }
            });
        }
        let weak = Rc::downgrade(this);
        this.gap.connect_value_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.read_controls(false);
            }
        });
        let weak = Rc::downgrade(this);
        this.message.connect_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.read_controls(true);
            }
        });
        let weak = Rc::downgrade(this);
        this.artwork.buffer().connect_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.read_controls(true);
            }
        });
        for widget in [
            this.message.clone().upcast::<gtk::Widget>(),
            this.artwork.clone().upcast(),
        ] {
            let focus = gtk::EventControllerFocus::new();
            let weak = Rc::downgrade(this);
            focus.connect_leave(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.finish();
                }
            });
            widget.add_controller(focus);
        }
        for (kind, row, _, up, down) in &this.rows {
            let kind = *kind;
            if let Some(button) = row
                .first_child()
                .and_then(|handle| handle.next_sibling())
                .and_then(|w| w.downcast::<gtk::MenuButton>().ok())
            {
                this.bind_field(&button, Info::ALL.iter().position(|k| *k == kind).unwrap());
            }
            let source = gtk::DragSource::builder()
                .actions(gdk::DragAction::MOVE)
                .build();
            let weak = Rc::downgrade(this);
            source.connect_prepare(move |_, _, _| {
                let this = weak.upgrade()?;
                if this.invalid.get() {
                    return None;
                }
                this.dragging.set(Some(kind));
                Some(gdk::ContentProvider::for_value(&kind.module().to_value()))
            });
            let row_weak = row.downgrade();
            source.connect_drag_begin(move |source, _| {
                if let Some(row) = row_weak.upgrade() {
                    source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&row))), 0, 0);
                    row.set_opacity(0.45);
                }
            });
            let weak = Rc::downgrade(this);
            source.connect_drag_end(move |_, _, _| {
                if let Some(this) = weak.upgrade() {
                    this.dragging.set(None);
                    for (_, row, ..) in &this.rows {
                        row.set_opacity(1.0);
                        row.remove_css_class("greeting-drop-before");
                        row.remove_css_class("greeting-drop-after");
                    }
                }
            });
            row.first_child().unwrap().add_controller(source);
            let target = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
            for enter in [true, false] {
                let weak = Rc::downgrade(this);
                let row_weak = row.downgrade();
                let motion = move |_: &gtk::DropTarget, _: f64, y: f64| {
                    let Some(this) = weak.upgrade() else {
                        return gdk::DragAction::empty();
                    };
                    if this.invalid.get() || this.dragging.get().is_none_or(|source| source == kind)
                    {
                        return gdk::DragAction::empty();
                    }
                    if let Some(row) = row_weak.upgrade() {
                        row.remove_css_class("greeting-drop-before");
                        row.remove_css_class("greeting-drop-after");
                        row.add_css_class(if y < f64::from(row.height()) / 2.0 {
                            "greeting-drop-before"
                        } else {
                            "greeting-drop-after"
                        });
                    }
                    gdk::DragAction::MOVE
                };
                if enter {
                    target.connect_enter(motion);
                } else {
                    target.connect_motion(motion);
                }
            }
            let row_weak = row.downgrade();
            target.connect_leave(move |_| {
                if let Some(row) = row_weak.upgrade() {
                    row.remove_css_class("greeting-drop-before");
                    row.remove_css_class("greeting-drop-after");
                }
            });
            let weak = Rc::downgrade(this);
            let row_weak = row.downgrade();
            target.connect_drop(move |_, value, _, y| {
                let (Some(this), Some(row)) = (weak.upgrade(), row_weak.upgrade()) else {
                    return false;
                };
                row.remove_css_class("greeting-drop-before");
                row.remove_css_class("greeting-drop-after");
                let Some(source) = this.dragging.get() else {
                    return false;
                };
                if this.invalid.get()
                    || value.get::<String>().ok().as_deref() != Some(source.module())
                {
                    return false;
                }
                let mut settings = this.settings();
                if !settings.move_item(source, kind, y >= f64::from(row.height()) / 2.0) {
                    return false;
                }
                this.replace(settings, true);
                true
            });
            row.add_controller(target);
            for (button, direction) in [(up, -1_isize), (down, 1)] {
                let weak = Rc::downgrade(this);
                button.connect_clicked(move |_| {
                    if let Some(this) = weak.upgrade() {
                        if this.invalid.get() {
                            return;
                        }
                        let mut settings = this.settings();
                        let index = settings
                            .items
                            .iter()
                            .position(|item| item.kind == kind)
                            .unwrap();
                        if let Some(target) = index
                            .checked_add_signed(direction)
                            .filter(|target| *target < settings.items.len())
                        {
                            settings.items.swap(index, target);
                            this.replace(settings, true);
                        }
                    }
                });
            }
        }
    }
}

/// An overlay mask animates pixels, never VTE input or geometry. Reflow, typing
/// and navigation can cancel immediately without losing the final transcript.
pub(super) struct GreetingMotion {
    layer: gtk::DrawingArea,
    progress: Cell<f64>,
    mode: Cell<Opening>,
    generation: Cell<u64>,
    rows: Cell<usize>,
}
impl GreetingMotion {
    pub fn new() -> Self {
        let layer = gtk::DrawingArea::new();
        layer.set_can_target(false);
        layer.set_focusable(false);
        layer.set_visible(false);
        Self {
            layer,
            progress: Cell::new(1.0),
            mode: Cell::new(Opening::None),
            generation: Cell::new(0),
            rows: Cell::new(0),
        }
    }
    pub fn stop(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.progress.set(1.0);
        self.layer.set_visible(false);
    }
}

impl Workbench {
    pub(super) fn ensure_official_greeting_preview(self: &Rc<Self>) {
        let settings = self.greeting.settings();
        let key = if settings.enabled && settings.needs_native() {
            let config = if settings.imported_source.is_some() {
                settings
                    .fastfetch_config()
                    .and_then(|source| {
                        crate::fastfetch_document::preview_config_with_logo(
                            &source,
                            settings.source_logo.as_ref(),
                        )
                    })
                    .map(|(config, _)| config)
            } else {
                let mut snapshot = settings.clone();
                snapshot.message.clear();
                snapshot
                    .fastfetch_config()
                    .and_then(|text| {
                        serde_json::from_str::<serde_json::Value>(&text).map_err(|e| e.to_string())
                    })
                    .map(|mut config| {
                        config["logo"] = serde_json::json!({"type":"none"});
                        config
                    })
            };
            match config {
                Ok(config) => Some((settings.official_preset, config.to_string())),
                Err(error) => {
                    self.greeting.preset_note.set_text(&error);
                    None
                }
            }
        } else {
            None
        };
        if key != *self.greeting_official_key.borrow() {
            *self.greeting_official_key.borrow_mut() = key.clone();
            self.greeting_official_result.borrow_mut().take();
        }
        let Some(key) = key else {
            return;
        };
        if self.greeting_official_result.borrow().is_some()
            || self.greeting_official_loading.replace(true)
        {
            return;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let requested = key.clone();
        std::thread::spawn(move || {
            let _ = sender.send(
                serde_json::from_str(&requested.1)
                    .map_err(|e| e.to_string())
                    .and_then(crate::greeting_official::render_native),
            );
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(40), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let result = match receiver.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err(
                    "Native preview worker stopped. Refresh the system snapshot to retry.".into(),
                ),
            };
            this.greeting_official_loading.set(false);
            if this.greeting_official_key.borrow().as_ref() != Some(&key) {
                this.ensure_official_greeting_preview();
                return glib::ControlFlow::Break;
            }
            if let Err(error) = &result {
                this.greeting.preset_note.set_text(error);
            } else {
                this.greeting
                    .preset_note
                    .set_text(if key.0 == Some(OfficialPreset::Icons) {
                        "Official Example 8 · uses Nerd Font icons"
                    } else {
                        "Offline preview · desktop detection and disk flags may differ"
                    });
            }
            *this.greeting_official_result.borrow_mut() = Some(result);
            this.schedule_diagnostics();
            if this.greeting_preview.get() {
                this.redraw_preview_contents();
            }
            glib::ControlFlow::Break
        });
    }
    pub(super) fn greeting_text_for_width(&self, columns: usize) -> String {
        self.greeting_parts_for_width(columns)
            .into_iter()
            .map(|(text, _)| text)
            .collect()
    }
    pub(super) fn greeting_parts_for_width(
        &self,
        columns: usize,
    ) -> Vec<(String, crate::greeting::GreetingPart)> {
        let context = self.current_preview_context.borrow();
        let fallback = crate::greeting::GreetingContext::default();
        let facts = context.as_ref().map(|c| &c.greeting).unwrap_or(&fallback);
        let result = self.greeting_official_result.borrow();
        let text = match result.as_ref() {
            Some(Ok(output)) => Some(output.render(columns.clamp(12, 240) - 1)),
            Some(Err(_)) => Some(
                "Native preview unavailable\r\nOpen Greeting > Compatibility for details.".into(),
            ),
            None => None,
        };
        let settings = self.greeting.settings();
        settings.render_parts(facts, columns, text.as_deref())
    }
    fn greeting_preview_text(&self) -> String {
        self.greeting_text_for_width(self.preview_terminal.column_count().max(12) as usize)
    }
    pub(super) fn connect_greeting_motion(this: &Rc<Self>) {
        this.inspect_layer.add_overlay(&this.greeting_motion.layer);
        this.inspect_layer
            .set_measure_overlay(&this.greeting_motion.layer, false);
        this.inspect_layer
            .set_clip_overlay(&this.greeting_motion.layer, true);
        let weak = Rc::downgrade(this);
        this.greeting_motion
            .layer
            .set_draw_func(move |layer, cr, _, _| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                let motion = &this.greeting_motion;
                let p = motion.progress.get();
                if p >= 1.0 {
                    return;
                }
                let Some(bounds) = this.preview_terminal.compute_bounds(layer) else {
                    return;
                };
                let Some(viewport) = this.preview_terminal_viewport.compute_bounds(layer) else {
                    return;
                };
                cr.rectangle(
                    viewport.x().into(),
                    viewport.y().into(),
                    viewport.width().into(),
                    viewport.height().into(),
                );
                cr.clip();
                let x = f64::from(bounds.x());
                let y = f64::from(bounds.y());
                let width = f64::from(bounds.width());
                let cell = this.preview_terminal.char_height() as f64;
                let height = (motion.rows.get() as f64 * cell).min(f64::from(bounds.height()));
                let model = this.model.borrow();
                let bg = model
                    .palette
                    .variant(model.active_variant)
                    .and_then(|v| v.get("Background"))
                    .unwrap_or(Rgb::new(0, 0, 0));
                let fg = model
                    .palette
                    .variant(model.active_variant)
                    .and_then(|v| v.get("Foreground"))
                    .unwrap_or(Rgb::new(255, 255, 255));
                let alpha = match motion.mode.get() {
                    Opening::Lines => 1.0,
                    Opening::Shimmer => (1.0 - p * 2.0).max(0.0),
                    _ => (1.0 - p).powi(2),
                };
                cr.set_source_rgba(
                    f64::from(bg.red()) / 255.0,
                    f64::from(bg.green()) / 255.0,
                    f64::from(bg.blue()) / 255.0,
                    alpha,
                );
                let revealed = if motion.mode.get() == Opening::Lines {
                    (p * motion.rows.get() as f64).floor() * cell
                } else {
                    0.0
                };
                cr.rectangle(x, y + revealed, width, (height - revealed).max(0.0));
                let _ = cr.fill();
                if motion.mode.get() == Opening::Shimmer {
                    let center = x - 100.0 + p * (width + 200.0);
                    let gradient =
                        gtk::cairo::LinearGradient::new(center - 100.0, 0.0, center + 100.0, 0.0);
                    for (stop, alpha) in [
                        (0.0, 0.0),
                        (0.5, 0.12 * (std::f64::consts::PI * p).sin()),
                        (1.0, 0.0),
                    ] {
                        gradient.add_color_stop_rgba(
                            stop,
                            f64::from(fg.red()) / 255.0,
                            f64::from(fg.green()) / 255.0,
                            f64::from(fg.blue()) / 255.0,
                            alpha,
                        );
                    }
                    let _ = cr.set_source(&gradient);
                    cr.rectangle(x, y, width, height);
                    let _ = cr.fill();
                }
            });
        let weak = Rc::downgrade(this);
        this.greeting.replay.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.play_greeting_opening();
            }
        });
        // A mode change previews once after the coalesced text/geometry update.
        let weak = Rc::downgrade(this);
        this.greeting.opening.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade()
                && !this.greeting.updating.get()
            {
                this.greeting_motion.stop();
                let weak = Rc::downgrade(&this);
                glib::timeout_add_local_once(Duration::from_millis(180), move || {
                    if let Some(this) = weak.upgrade() {
                        this.play_greeting_opening();
                    }
                });
            }
        });
        let click = gtk::GestureClick::new();
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(this);
        click.connect_pressed(move |_, _, _, _| {
            if let Some(this) = weak.upgrade() {
                this.greeting_motion.stop();
            }
        });
        this.preview_terminal.add_controller(click);
    }
    fn play_greeting_opening(self: &Rc<Self>) {
        self.greeting_motion.stop();
        let settings = self.greeting.settings();
        if !self.greeting_preview.get()
            || !self.greeting_module_button.is_active()
            || !settings.enabled
            || self.greeting.invalid.get()
            || settings.opening == Opening::None
        {
            return;
        }
        if !self.preview_terminal.settings().is_gtk_enable_animations() {
            self.toast("Opening motion is disabled by your system's reduced-motion setting.");
            return;
        }
        let text = self.greeting_preview_text();
        let rows = text.trim_end_matches(['\r', '\n']).lines().count();
        if rows == 0 {
            return;
        }
        let motion = &self.greeting_motion;
        motion.rows.set(rows);
        motion.mode.set(settings.opening);
        motion.progress.set(0.0);
        motion.layer.set_visible(true);
        let generation = motion.generation.get();
        let start = std::time::Instant::now();
        let weak = Rc::downgrade(self);
        motion.layer.add_tick_callback(move |_, _| {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let motion = &this.greeting_motion;
            if motion.generation.get() != generation {
                return glib::ControlFlow::Break;
            }
            if !this.preview_terminal.is_mapped()
                || !this.preview_terminal.settings().is_gtk_enable_animations()
            {
                motion.stop();
                return glib::ControlFlow::Break;
            }
            let p = (start.elapsed().as_secs_f64() / 0.85).min(1.0);
            motion.progress.set(p);
            motion.layer.queue_draw();
            if p >= 1.0 {
                motion.stop();
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }
    pub(super) fn show_greeting_preview(self: &Rc<Self>) {
        let navigating = self.navigating_preview.replace(true);
        self.ensure_starship_copy();
        self.navigating_preview.set(navigating);
        if !self.greeting_preview.replace(true) {
            self.preview_scroll.start();
        }
        self.ensure_official_greeting_preview();
        let previous_columns = self.preview_terminal.column_count();
        self.refresh_terminal_geometry(&self.layout_settings());
        if self.preview_prompt_source.get() == 0 && self.starship_editor.draft.borrow().is_some() {
            if !self.preview_uses_prompt.get() {
                self.activate_prompt_preview(0);
            }
            let contents = self
                .starship_editor
                .draft
                .borrow()
                .as_ref()
                .map(|d| d.contents().to_owned());
            if previous_columns != self.preview_terminal.column_count()
                || (!self.copy_loading.get() && contents != *self.copy_rendered_source.borrow())
            {
                self.schedule_copy_preview();
            }
        }
        self.redraw_preview_contents();
    }
    pub(super) fn redraw_greeting(&self) {
        self.terminal_title.set_text(&format!(
            "greeting · {} cols",
            self.preview_terminal.column_count()
        ));
        self.preview_selector.set_visible(false);
        self.prompt_preview_selector.set_visible(false);
        self.prompt_compare_selector.set_visible(false);
        for (text, part) in
            self.greeting_parts_for_width(self.preview_terminal.column_count().max(12) as usize)
        {
            use crate::greeting::GreetingPart;
            let target = match part {
                GreetingPart::Artwork => PreviewTarget::GreetingArtwork,
                GreetingPart::Message => PreviewTarget::GreetingMessage,
                GreetingPart::Fields => PreviewTarget::GreetingFields,
                GreetingPart::Field(kind) => PreviewTarget::GreetingField(kind),
            };
            self.feed_scoped_preview(&text, Some(target));
        }
        let context = self.current_preview_context.borrow();
        let designed = self.preview_prompt_source.get() == 1;
        let prompt = if designed {
            self.prompt_settings.borrow().preview_ansi(
                &context
                    .as_ref()
                    .map(CurrentPreviewContext::as_prompt_context)
                    .unwrap_or(prompt_preview_contexts()[0]),
            )
        } else {
            let contents = self
                .starship_editor
                .draft
                .borrow()
                .as_ref()
                .map(|d| d.contents().to_owned());
            let edited = (contents.is_some() && contents == *self.copy_rendered_source.borrow())
                .then(|| {
                    self.copy_preview
                        .borrow()
                        .as_ref()
                        .and_then(|p| p.as_ref().ok())
                        .cloned()
                })
                .flatten();
            edited
                .or_else(|| {
                    // A portable prompt must never briefly display this
                    // machine's unrelated configuration while rendering.
                    if contents.is_none() && !self.workspace_prompt_loaded.get() {
                        context
                            .as_ref()
                            .and_then(|c| c.imported_prompt.ansi.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "$ ".into())
        };
        let input = self.preview_input.borrow();
        for line in input.submitted() {
            if designed {
                self.feed_designed_prompt(
                    &self.prompt_settings.borrow(),
                    &context
                        .as_ref()
                        .map(CurrentPreviewContext::as_prompt_context)
                        .unwrap_or(prompt_preview_contexts()[0]),
                );
            } else {
                self.feed_scoped_preview(&prompt, Some(PreviewTarget::PromptCopy));
            }
            self.feed_preview(line.as_bytes());
            self.feed_preview(b"\r\n");
        }
        if designed {
            self.feed_designed_prompt(
                &self.prompt_settings.borrow(),
                &context
                    .as_ref()
                    .map(CurrentPreviewContext::as_prompt_context)
                    .unwrap_or(prompt_preview_contexts()[0]),
            );
        } else {
            self.feed_scoped_preview(&prompt, Some(PreviewTarget::PromptCopy));
        }
        self.feed_preview(input.text().as_bytes());
        self.feed_preview(PREVIEW_SHOW_CURSOR);
    }
    pub(super) fn save_greeting_preset(&self) {
        let result = self.greeting.persist();
        match result {
            Ok(()) => {
                let toast = adw::Toast::builder()
                    .title("Saved in TermiMochi. External Fastfetch is unchanged.")
                    .button_label("Review & Apply…")
                    .action_name("win.apply-fastfetch")
                    .priority(adw::ToastPriority::High)
                    .timeout(8)
                    .build();
                self.toast_overlay.add_toast(toast);
            }
            Err(error) => self.toast(&error),
        }
        self.greeting.refresh_status();
        self.refresh_history_actions();
    }
    fn confirm_greeting_replace(self: &Rc<Self>, action: impl FnOnce(Rc<Self>) + 'static) {
        if !self.greeting.dirty() || self.workspace_is_clean() {
            action(self.clone());
            return;
        }
        let dialog = gtk::AlertDialog::builder()
            .message("Replace unsaved greeting changes?")
            .detail("Save Preset or Save Workspace first to keep this greeting.")
            .buttons(["Cancel", "Replace Changes"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if result == Ok(1)
                    && let Some(this) = weak.upgrade()
                {
                    action(this);
                }
            },
        );
    }
    pub(super) fn reload_greeting_preset(self: &Rc<Self>) {
        let path = self
            .greeting
            .store
            .borrow()
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| typography_preset::state_directory().join(PRESET_NAME));
        match DocumentStore::<GreetingPreset>::open(path).and_then(|store| {
            let preset = store
                .document()?
                .ok_or("No greeting preset has been saved.")?;
            Ok((store, preset))
        }) {
            Ok((store, preset)) => self.confirm_greeting_replace(move |this| {
                *this.greeting.store.borrow_mut() = Some(store);
                *this.greeting.baseline.borrow_mut() = preset.greeting.clone();
                this.greeting.saved.set(true);
                this.greeting.replace(preset.greeting, true);
            }),
            Err(error) => self.toast(&error),
        }
    }
    fn greeting_dialog(title: &str, pattern: &str) -> gtk::FileDialog {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(title));
        filter.add_pattern(pattern);
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        gtk::FileDialog::builder()
            .title(title)
            .filters(&filters)
            .default_filter(&filter)
            .modal(true)
            .build()
    }
    pub(super) fn choose_greeting_open(self: &Rc<Self>) {
        let dialog = Self::greeting_dialog("Open Greeting Preset", "*.termimochi-greeting.json");
        let weak = Rc::downgrade(self);
        dialog.open(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(file) => match file
                        .path()
                        .ok_or("Choose a local preset.".into())
                        .and_then(DocumentStore::<GreetingPreset>::open)
                        .and_then(|store| store.document())
                        .and_then(|p| p.ok_or("Preset no longer exists.".into()))
                    {
                        Ok(preset) => this.confirm_greeting_replace(move |this| {
                            this.greeting.replace(preset.greeting, true);
                            this.toast(
                                "Greeting opened. Save Preset to remember it for next launch.",
                            );
                        }),
                        Err(error) => this.toast(&error),
                    },
                    Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                    Err(error) => this.toast(&error.to_string()),
                }
            },
        );
    }
    pub(super) fn choose_greeting_export(self: &Rc<Self>, fastfetch: bool) {
        let settings = self.greeting.settings();
        if fastfetch
            && settings
                .presentation
                .resolve(
                    &settings,
                    self.greeting.presentation.binding.borrow().terminal,
                )
                .is_ok_and(|s| s.protocol.is_some())
        {
            self.show_pixel_export();
            return;
        }
        self.greeting.finish();
        if self.greeting.invalid.get() {
            self.toast("Fix the greeting fields before exporting.");
            return;
        }
        let mut settings = self.greeting.settings();
        if fastfetch {
            // Export the actual stacked/side-by-side composition at the current
            // preview width. JSONC is static; it does not resize the user's terminal.
            settings.position =
                settings.position_at_width(self.preview_terminal.column_count().max(12) as usize);
        }
        let dialog = if fastfetch {
            Self::greeting_dialog("Export Fastfetch Configuration", "*.jsonc")
        } else {
            Self::greeting_dialog("Export Greeting Preset", "*.termimochi-greeting.json")
        };
        dialog.set_initial_name(Some(if fastfetch {
            "config.jsonc"
        } else {
            PRESET_NAME
        }));
        let weak = Rc::downgrade(self);
        dialog.save(Some(&self.window()), gio::Cancellable::NONE, move |result| {
            let Some(this) = weak.upgrade() else { return; };
            match result {
                Ok(file) => {
                    if fastfetch {
                        match file.path() {
                            Some(path) => this.confirm_fastfetch_export(path, settings),
                            None => this.toast("Choose a local file."),
                        }
                        return;
                    }
                    let outcome = file.path().ok_or("Choose a local file.".into()).and_then(|path| {
                        DocumentStore::open(path)?.save(&GreetingPreset::new(settings.clone()))
                    });
                    match outcome {
                        Ok(()) => this.toast(if fastfetch { "Fastfetch configuration exported. Run fastfetch --config with this file to try it; shell startup is unchanged." } else { "Greeting preset exported. Save Preset remembers it for next launch." }),
                        Err(error) => this.toast(&error),
                    }
                }
                Err(error) if error.matches(gtk::DialogError::Dismissed) => {}, Err(error) => this.toast(&error.to_string()),
            }
        });
    }

    fn confirm_fastfetch_export(self: &Rc<Self>, path: PathBuf, settings: GreetingSettings) {
        let expected = match validate_fastfetch_path(&path).and_then(|()| {
            typography_preset::read_private_with_limit(&path, crate::fastfetch_document::LIMIT)
        }) {
            Ok(expected) => expected,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let replacing = expected.is_some();
        let detail = format!(
            "{}\n\nThe existing file will be backed up in termimochi-backups beside it. If this is your active Fastfetch configuration, future runs will use the new static greeting. Opening animation is not exported. Shell startup files are unchanged.",
            path.display()
        );
        let finish = move |this: &Self| {
            match export_fastfetch(&path, &settings, &expected) {
                Ok(()) => this.toast("Static Fastfetch configuration exported. Existing content was backed up if replaced. Shell startup is unchanged."),
                Err(error) => this.toast(&error),
            }
        };
        // Choosing a path is not permission to silently replace an active
        // greeting. Confirm with a snapshot, then recheck it before writing.
        if !replacing {
            finish(self);
            return;
        }
        let dialog = gtk::AlertDialog::builder()
            .message("Replace Fastfetch configuration?")
            .detail(detail)
            .buttons(["Cancel", "Back Up & Replace"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if result == Ok(1)
                    && let Some(this) = weak.upgrade()
                {
                    finish(&this);
                }
            },
        );
    }
}

fn validate_fastfetch_path(path: &Path) -> Result<(), String> {
    if !path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "config.jsonc" || n.ends_with(".fastfetch.jsonc"))
    {
        return Err("Use config.jsonc or a filename ending in .fastfetch.jsonc. Shell startup files cannot be exported here.".into());
    }
    Ok(())
}

fn export_fastfetch(
    path: &Path,
    settings: &GreetingSettings,
    expected: &Option<Vec<u8>>,
) -> Result<(), String> {
    use crate::fastfetch_document::LIMIT;
    use crate::typography_preset::{
        read_private_with_limit, retain_backup, write_checked_with_limit,
    };
    validate_fastfetch_path(path)?;
    let bytes = settings.fastfetch_config()?.into_bytes();
    if &read_private_with_limit(path, LIMIT)? != expected {
        return Err("The destination changed after confirmation. Nothing was overwritten.".into());
    }
    if let Some(before) = expected {
        retain_backup(
            &path
                .parent()
                .ok_or("Missing export directory")?
                .join("termimochi-backups"),
            before,
        )?;
    }
    write_checked_with_limit(path, &bytes, expected, LIMIT)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GTK/VTE; semantic Greeting navigation across layouts without changing the scene"]
    fn greeting_inspect_artwork_message_fields_and_prompt_preserve_scene() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.GreetingInspectTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let this = controller(&window);
        settle();
        while this.preview_loading.get() {
            settle();
        }
        let terminal = &this.preview_terminal;
        for position in Position::ALL {
            let mut settings = GreetingSettings {
                enabled: true,
                logo: Logo::Custom,
                custom_logo: "LOGO\n中🙂".into(),
                message: "Hello Inspect".into(),
                position,
                preview_columns: 80,
                ..Default::default()
            };
            settings
                .items
                .iter_mut()
                .for_each(|i| i.enabled = i.kind == Info::Os);
            this.greeting.replace(settings.clone(), true);
            this.greeting_module_button.set_active(true);
            settle();
            this.preview_scroll.start();
            settle();
            let transcript = feed(&this);
            for target in [
                PreviewTarget::GreetingArtwork,
                PreviewTarget::GreetingMessage,
                PreviewTarget::GreetingField(Info::Os),
            ] {
                if position == Position::Card && target == PreviewTarget::GreetingArtwork {
                    continue; // Minimal card deliberately omits the logo.
                }
                this.preview_scroll.start();
                settle();
                assert!(
                    this.preview_feed
                        .borrow()
                        .iter()
                        .any(|c| c.scope == Some(target)),
                    "missing scope {target:?}"
                );
                // Resolve actual VTE cells, including Unicode and stacked layouts.
                let (top, fraction) = preview_visible_origin(terminal).unwrap();
                let native = terminal.native().unwrap();
                let (dx, dy) = native.surface_transform();
                let native = native.dynamic_cast::<gtk::Widget>().unwrap();
                let mut found = false;
                for row in top..=terminal.cursor_position().1 {
                    for col in 0..terminal.column_count() {
                        let point = terminal
                            .compute_point(
                                &native,
                                &gtk::graphene::Point::new(
                                    (col as f32 + 0.5) * terminal.char_width() as f32,
                                    ((row - top) as f32 - fraction as f32 + 0.5)
                                        * terminal.char_height() as f32,
                                ),
                            )
                            .unwrap();
                        if this
                            .preview_target_at(f64::from(point.x()) + dx, f64::from(point.y()) + dy)
                            == Some(target)
                        {
                            found = true;
                            break;
                        }
                    }
                    if found {
                        break;
                    }
                }
                assert!(
                    found,
                    "no real VTE hit for {position:?} {target:?}; top={top}, cursor={:?}, canvas={:?}, text={:?}",
                    terminal.cursor_position(),
                    terminal.compute_bounds(&this.preview_terminal_viewport),
                    terminal.text_format(vte::Format::Text)
                );
                this.typography_module_button.set_active(true);
                settle();
                this.inspect_preview_target(target);
                settle();
                assert!(this.greeting_module_button.is_active());
                let widget = this.greeting.inspection_control(target).unwrap();
                assert!(widget.has_css_class("preview-inspected"));
                let bounds = widget.compute_bounds(&this.greeting.root).unwrap();
                assert!(
                    bounds.y() >= -1.0
                        && bounds.y() + bounds.height() <= this.greeting.root.height() as f32 + 1.0,
                    "target not revealed: {target:?} {bounds:?}"
                );
                assert_eq!(this.greeting.settings(), settings);
                assert_eq!(feed(&this), transcript);
            }
        }
        // Designer prompt in Greeting must not navigate into Your Starship.
        this.preview_prompt_source.set(1);
        this.redraw_preview_contents();
        settle();
        assert!(this.preview_feed.borrow().iter().any(|c| matches!(
            c.scope,
            Some(PreviewTarget::PromptSegment(_) | PreviewTarget::PromptCharacter)
        )));
        let transcript = feed(&this);
        this.inspect_preview_target(PreviewTarget::PromptCharacter);
        settle();
        assert_eq!(this.prompt_source_selector.selected(), 1);
        assert_eq!(feed(&this), transcript);
        window.destroy();
    }

    #[test]
    fn export_standard_config_backup_conflicts_and_unsafe_destinations() {
        let root = tempfile::tempdir().unwrap();
        let settings = GreetingSettings {
            enabled: true,
            ..Default::default()
        };
        for name in [".bashrc", "starship.toml", "unrelated.jsonc"] {
            let path = root.path().join(name);
            std::fs::write(&path, "keep").unwrap();
            assert!(export_fastfetch(&path, &settings, &Some(b"keep".to_vec())).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep");
        }
        let path = root.path().join("config.jsonc");
        export_fastfetch(&path, &settings, &None).unwrap();
        let old = std::fs::read(&path).unwrap();
        export_fastfetch(&path, &settings, &Some(old.clone())).unwrap();
        let backup = std::fs::read_dir(root.path().join("termimochi-backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read(backup).unwrap(), old);
        let alias = root.path().join("alias.fastfetch.jsonc");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        assert!(export_fastfetch(&alias, &settings, &None).is_err());
        std::fs::write(&path, "not a config").unwrap();
        assert!(export_fastfetch(&path, &settings, &Some(old)).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not a config");
        // JSONC comments in an explicitly confirmed replacement are preserved
        // byte-for-byte in the backup, never parsed or executed.
        let before = b"// user's comments\n{\"modules\":[\"os\"]}\n".to_vec();
        std::fs::write(&path, &before).unwrap();
        export_fastfetch(&path, &settings, &Some(before.clone())).unwrap();
        assert!(
            std::fs::read_dir(root.path().join("termimochi-backups"))
                .unwrap()
                .any(|entry| std::fs::read(entry.unwrap().path()).unwrap() == before)
        );
        let alias = root.path().join("hard.fastfetch.jsonc");
        std::fs::hard_link(&path, &alias).unwrap();
        assert!(export_fastfetch(&path, &settings, &None).is_err());
        assert!(export_fastfetch(&alias, &settings, &None).is_err());
    }

    pub(in crate::window) fn settle() {
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_millis(300) {
            while glib::MainContext::default().pending() {
                glib::MainContext::default().iteration(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub(in crate::window) fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut result = vec![widget.clone()];
        let mut child = widget.first_child();
        while let Some(current) = child {
            result.extend(descendants(&current));
            child = current.next_sibling();
        }
        result
    }
    pub(super) fn respond(label: &str) {
        settle();
        gtk::Window::list_toplevels()
            .into_iter()
            .flat_map(|w| descendants(&w))
            .find_map(|w| {
                w.downcast::<gtk::Button>()
                    .ok()
                    .filter(|b| b.label().as_deref() == Some(label))
            })
            .unwrap_or_else(|| panic!("Missing button {label}"))
            .emit_clicked();
        settle();
    }
    pub(super) fn select_scheme_greeting() {
        settle();
        let check = gtk::Window::list_toplevels()
            .into_iter()
            .flat_map(|w| descendants(&w))
            .find(|w| w.widget_name() == "scheme-fastfetch")
            .unwrap()
            .downcast::<gtk::CheckButton>()
            .unwrap();
        assert!(check.is_sensitive(), "Greeting plan must be reviewable");
        check.set_active(true);
    }
    pub(in crate::window) fn controller(window: &gtk::Window) -> Rc<Workbench> {
        unsafe {
            window
                .data::<Rc<Workbench>>("termimochi-workbench")
                .unwrap()
                .as_ref()
                .clone()
        }
    }
    pub(super) fn feed(this: &Workbench) -> String {
        this.preview_feed
            .borrow()
            .iter()
            .map(|c| c.text.as_str())
            .collect()
    }

    pub(super) fn pointer(window: &gtk::Window, widget: &gtk::Widget, mode: &str) {
        let rect = widget.compute_bounds(window).unwrap();
        let (dx, dy) = window.surface_transform();
        let scale = f64::from(window.scale_factor());
        let x = (f64::from(rect.x() + rect.width() * 0.5) + dx) * scale;
        let y = (f64::from(rect.y() + rect.height() * 0.5) + dy) * scale;
        let mut child = std::process::Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/preview-pointer-driver.py"
            ))
            .args([mode, &(x as i32).to_string(), &(y as i32).to_string(), "2"])
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("pointer timeout");
            }
            settle();
        }
        assert!(child.wait().unwrap().success());
        settle();
        settle();
    }

    pub(super) fn wait_official(this: &Workbench) {
        settle();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while this.greeting_official_loading.get()
            || this.greeting_official_result.borrow().is_none()
        {
            assert!(
                std::time::Instant::now() < deadline,
                "official preview did not settle"
            );
            settle();
        }
        assert!(
            this.greeting_official_result
                .borrow()
                .as_ref()
                .unwrap()
                .is_ok(),
            "{:?}",
            this.greeting_official_result.borrow()
        );
    }

    #[test]
    #[ignore = "requires GTK/VTE, system Fastfetch and Bubblewrap; run at 1x and 2x"]
    fn termimochi_brand_starter_preview_and_saved_custom_preservation() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.BrandGreetingTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(typography_preset::PRESET_NAME);
        present_with_preset(&app, None, path.clone());
        let window = app.active_window().unwrap();
        window.set_default_size(1320, 850);
        let this = controller(&window);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        assert_eq!(this.greeting.settings(), GreetingSettings::starter());
        assert!(!this.greeting.dirty());
        gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-greeting", None);
        this.greeting.enabled.set_active(true);
        wait_official(&this);
        assert_eq!(this.greeting.preset.selected(), 1);
        assert_eq!(this.greeting.official_rows.borrow().len(), 19);
        assert!(!this.greeting.list.is_visible());
        for width in [1, 2, 3] {
            this.greeting.columns.set_selected(width);
            settle();
            assert_eq!(
                this.preview_terminal.column_count(),
                i64::from(WIDTHS[width as usize])
            );
            assert_eq!(
                this.greeting
                    .settings()
                    .position_at_width(WIDTHS[width as usize] as usize),
                Position::Left
            );
            assert!(feed(&this).contains("oooooooo"));
            assert!(feed(&this).contains("Packages"));
            assert!(feed(&this).contains("Memory"));
            assert!(!feed(&this).contains("Welcome back"));
        }
        this.greeting.columns.set_selected(2);
        settle();
        if let Some(path) = std::env::var_os("TERMIMOCHI_BRAND_SCREENSHOT") {
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
        this.save_greeting_preset();
        let brand = this.greeting.settings();
        this.greeting
            .preset
            .set_selected(OfficialPreset::Neofetch.selector_index());
        this.greeting.undo();
        wait_official(&this);
        assert_eq!(this.greeting.settings(), brand);
        window.destroy();
        present_with_preset(&app, None, path.clone());
        let window = app.active_window().unwrap();
        let this = controller(&window);
        assert_eq!(this.greeting.settings(), brand);
        this.greeting.preset.set_selected(0);
        this.greeting
            .message
            .set_text("My existing custom greeting");
        this.save_greeting_preset();
        let custom = this.greeting.settings();
        window.destroy();
        present_with_preset(&app, None, path);
        let window = app.active_window().unwrap();
        assert_eq!(controller(&window).greeting.settings(), custom);
        assert!(custom.official_preset.is_none());
        window.destroy();
    }

    #[test]
    #[ignore = "requires GTK/VTE, system Fastfetch and Bubblewrap; run separately"]
    fn imported_greeting_fields_stay_on_their_rows_after_resize_and_scroll() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.GreetingLayoutTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(typography_preset::PRESET_NAME);
        let art = (0..28)
            .map(|row| format!("\x1b[38;2;12;134;56mART{row:02}{}\x1b[0m", ".".repeat(51)))
            .collect::<Vec<_>>()
            .join("\n");
        let modules = (0..20).map(|row| serde_json::json!({"type":"os", "key":format!("FIELD{row:02}"), "format":format!("value{row:02}")})).collect::<Vec<_>>();
        let mut source = serde_json::json!({"logo":{"type":"data-raw","source":art,"padding":{"left":0,"right":3,"top":0},"printRemaining":true},"modules":modules});
        let mut settings = GreetingSettings::starter();
        settings.imported_source = Some(source.to_string());
        settings.enabled = true;
        settings.official_preset = None;
        settings.official_items.clear();
        settings.preview_columns = 80;
        DocumentStore::<GreetingPreset>::open(root.path().join(PRESET_NAME))
            .unwrap()
            .save(&GreetingPreset::new(settings.clone()))
            .unwrap();
        present_with_preset(&app, None, path);
        let window = app.active_window().unwrap();
        window.set_default_size(1320, 850);
        let this = controller(&window);
        gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-greeting", None);
        wait_official(&this);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        for position in ["left", "right", "top", "left"] {
            source["logo"]["position"] = serde_json::json!(position);
            settings.imported_source = Some(source.to_string());
            this.greeting.replace(settings.clone(), true);
            wait_official(&this);
            for width_index in [1, 2, 3, 1] {
                this.greeting.columns.set_selected(width_index);
                settle();
                let terminal = &this.preview_terminal;
                let columns = i64::from(WIDTHS[width_index as usize]);
                assert_eq!(terminal.column_count(), columns);
                let snapshot = || terminal.text_format(vte::Format::Text).unwrap().to_string();
                let text = snapshot();
                let lines: Vec<_> = text.lines().collect();
                let first = lines
                    .iter()
                    .position(|l| l.contains("FIELD00"))
                    .unwrap_or_else(|| {
                        panic!(
                            "fields visible: text={text:?}, cursor={:?}",
                            terminal.cursor_position()
                        )
                    });
                for row in 0..20 {
                    assert!(
                        lines[first + row].contains(&format!("FIELD{row:02}: value{row:02}")),
                        "position={position}, cols={columns}, row={row}, text={text:?}"
                    );
                    if position != "top" {
                        assert_eq!(first, 0);
                        assert!(lines[row].contains(&format!("ART{row:02}")), "{text:?}");
                        assert_eq!(
                            lines[row].find(&format!("FIELD{row:02}")),
                            Some(if position == "left" { 59 } else { 0 })
                        );
                    }
                }
                if position == "top" {
                    assert!(first >= 28);
                }
                let scroll = this.preview_terminal_viewport.vadjustment();
                scroll.set_value((scroll.upper() - scroll.page_size()).max(0.0));
                settle();
                scroll.set_value(0.0);
                window.set_default_size(if width_index == 2 { 1180 } else { 1320 }, 790);
                settle();
                assert_eq!(snapshot(), text, "scroll/resize must not relocate fields");
            }
        }
        window.destroy();
    }

    #[test]
    #[ignore = "requires GTK/VTE, system Fastfetch and Bubblewrap; run separately"]
    fn official_greeting_presets_preserve_fields_history_workspace_and_full_art() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.OfficialGreetingTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(typography_preset::PRESET_NAME);
        present_with_preset(&app, None, path.clone());
        let window = app.active_window().unwrap();
        window.set_default_size(1320, 850);
        let this = controller(&window);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-greeting", None);
        let original = this.greeting.settings();
        let layout = this.layout_settings();
        for preset in OfficialPreset::ALL {
            this.greeting.preset.set_selected(preset.selector_index());
            wait_official(&this);
            assert_eq!(this.greeting.settings().official_preset, Some(preset));
            assert_eq!(
                this.greeting.official_rows.borrow().len(),
                preset.items().len()
            );
            assert!(!this.greeting.list.is_visible());
            assert!(this.greeting.official_list.is_visible());
            assert!(
                feed(&this).contains(if preset == OfficialPreset::TermiMochi {
                    "oooooooo"
                } else {
                    "cooooo"
                })
            );
            assert!(!feed(&this).contains("Reading official preset"));
            assert_eq!(
                this.greeting.settings().artwork().lines().count(),
                if preset == OfficialPreset::TermiMochi {
                    12
                } else {
                    20
                }
            );
            let toggle = this.greeting.official_rows.borrow()[0].2.clone();
            toggle.set_active(false);
            wait_official(&this);
            let config: serde_json::Value =
                serde_json::from_str(&this.greeting.settings().fastfetch_config().unwrap())
                    .unwrap();
            assert_eq!(
                config["modules"].as_array().unwrap().len(),
                preset.items().len() - 1
            );
            this.greeting.undo();
            wait_official(&this);
            assert!(this.greeting.settings().official_items[0].enabled);
            let down = this.greeting.official_rows.borrow()[0].4.clone();
            down.emit_clicked();
            wait_official(&this);
            assert_eq!(this.greeting.settings().official_items[1].id, 0);
            this.greeting.undo();
            wait_official(&this);
            assert_eq!(this.greeting.settings().official_items[0].id, 0);
        }
        // Rapid selection must not let an older worker replace the final preset.
        this.greeting
            .preset
            .set_selected(OfficialPreset::Neofetch.selector_index());
        this.greeting
            .preset
            .set_selected(OfficialPreset::Paleofetch.selector_index());
        this.greeting
            .preset
            .set_selected(OfficialPreset::Screenfetch.selector_index());
        wait_official(&this);
        assert_eq!(
            this.greeting_official_key.borrow().as_ref().unwrap().0,
            Some(OfficialPreset::Screenfetch)
        );
        for (index, columns) in WIDTHS.into_iter().enumerate().skip(1) {
            this.greeting.columns.set_selected(index as u32);
            settle();
            assert_eq!(this.preview_terminal.column_count(), i64::from(columns));
            assert_eq!(this.layout_settings(), layout);
            if columns == 80 {
                assert!(
                    this.preview_terminal.row_count()
                        >= this.greeting_preview_text().lines().count() as i64
                );
            }
        }
        this.greeting.columns.set_selected(2);
        settle();
        if std::env::var_os("TERMIMOCHI_GREETING_DRAG_TEST").is_some() {
            window.set_title(Some("TermiMochi point-to-edit test"));
            let scroll = this.greeting.root.vadjustment();
            let content = this.greeting.root.child().unwrap();
            let bounds = this
                .greeting
                .official_list
                .compute_bounds(&content)
                .unwrap();
            scroll.set_value(f64::from(bounds.y()).min(scroll.upper() - scroll.page_size()));
            settle();
            let before = this.greeting.settings();
            let rows = this.greeting.official_rows.borrow();
            let a = rows[0]
                .1
                .first_child()
                .unwrap()
                .compute_bounds(&window)
                .unwrap();
            let b = rows[3].1.compute_bounds(&window).unwrap();
            drop(rows);
            let scale = window.scale_factor() as f32;
            let coords = [
                (a.x() + a.width() / 2.0) * scale,
                (a.y() + a.height() / 2.0) * scale,
                (b.x() + b.width() / 2.0) * scale,
                (b.y() + b.height() * 0.75) * scale,
            ];
            let mut child = std::process::Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../scripts/preview-pointer-driver.py"
                ))
                .arg("drag_to")
                .args(coords.map(|v| (v.round() as i32).to_string()))
                .spawn()
                .unwrap();
            while child.try_wait().unwrap().is_none() {
                settle();
            }
            assert!(child.wait().unwrap().success());
            wait_official(&this);
            let mut expected = before.clone();
            assert!(move_official_item(&mut expected.official_items, 0, 3, true));
            assert_eq!(this.greeting.settings(), expected);
            this.greeting.undo();
            wait_official(&this);
            assert_eq!(this.greeting.settings(), before);
            this.greeting.redo();
            wait_official(&this);
            assert_eq!(this.greeting.settings(), expected);
            scroll.set_value(0.0);
        }
        this.save_greeting_preset();
        let saved = this.greeting.settings();
        let workspace = root.path().join("official.termimochi.json");
        this.save_workspace_path(workspace.clone(), this.workspace_snapshot())
            .unwrap();
        this.greeting.preset.set_selected(0);
        settle();
        assert!(this.greeting.settings().official_preset.is_none());
        assert!(this.greeting.list.is_visible());
        assert_eq!(this.greeting.settings().items, original.items);
        this.open_workspace_path(&workspace);
        wait_official(&this);
        assert_eq!(this.greeting.settings(), saved);
        assert!(this.workspace_is_clean());
        if let Some(path) = std::env::var_os("TERMIMOCHI_OFFICIAL_SCREENSHOT") {
            if std::env::var_os("TERMIMOCHI_CAPTURE_MOCHI").is_some() {
                this.greeting.logo.set_selected(Logo::Mochi.index());
                settle();
                assert_eq!(this.greeting.artwork_size.text().as_str(), "38 × 12 cells");
                assert_eq!(
                    this.greeting.settings().position_at_width(80),
                    Position::Left
                );
            }
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
        window.destroy();
        present_with_preset(&app, None, path);
        let restored_window = app.active_window().unwrap();
        let restored = controller(&restored_window);
        assert_eq!(restored.greeting.settings(), saved);
        assert!(!restored.greeting.dirty());
        restored_window.destroy();
    }

    #[test]
    #[ignore = "requires GTK/VTE display; run separately with --ignored --test-threads=1"]
    fn greeting_editor_preview_persistence_and_workspace_safety() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.GreetingTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        let preset = root.path().join(typography_preset::PRESET_NAME);
        present_with_preset(&app, None, preset.clone());
        let window = app.active_window().unwrap();
        let this = controller(&window);
        // Exercise the retained basic editor, independent of the new starter.
        this.greeting.replace(GreetingSettings::default(), false);
        *this.greeting.baseline.borrow_mut() = this.greeting.settings();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        let before = (
            this.model.borrow().palette.clone(),
            this.typography_settings(),
            this.layout_settings(),
            this.prompt_settings.borrow().clone(),
        );
        gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-greeting", None);
        settle();
        assert!(this.greeting_module_button.is_active());
        assert!(!this.greeting.dirty());
        assert_eq!(
            this.save_button
                .menu_model()
                .unwrap()
                .item_attribute_value(0, "label", None),
            Some("Save Workspace".to_variant())
        );
        this.greeting.enabled.set_active(true);
        this.greeting.message.set_text("Hello 你好 🦀");
        settle();
        assert!(this.greeting_preview.get());
        assert!(feed(&this).contains("Hello 你好 🦀"));
        this.greeting.logo.set_selected(Logo::Custom.index());
        this.greeting
            .artwork
            .buffer()
            .set_text("(づ｡◕‿‿◕｡)づ\n  TermiMochi");
        this.greeting.position.set_selected(Position::Top.index());
        this.greeting.accent.set_selected(4);
        this.greeting.gap.set_value(5.0);
        this.greeting.rows[7].3.emit_clicked();
        settle();
        assert_eq!(this.greeting.settings().items[6].kind, Info::Date);
        assert!(feed(&this).contains("TermiMochi"));
        assert!(feed(&this).contains("\x1b[34m"));
        // Exact preview widths are isolated from the Layout document, and
        // cancelling the greeting scene restores that document's sizing.
        let layout_before = this.layout_settings();
        for (index, columns) in WIDTHS.into_iter().enumerate().skip(1) {
            this.greeting.columns.set_selected(index as u32);
            settle();
            assert_eq!(this.preview_terminal.column_count(), i64::from(columns));
            assert_eq!(this.layout_settings(), layout_before);
        }
        this.greeting.columns.set_selected(0);
        this.greeting.logo.set_selected(Logo::Ubuntu.index());
        this.greeting.position.set_selected(Position::Card.index());
        settle();
        assert!(!this.greeting.logo.is_sensitive());
        assert!(feed(&this).contains('─'));
        this.greeting.position.set_selected(Position::Top.index());
        this.greeting.logo.set_selected(Logo::Custom.index());
        settle();
        assert!(this.greeting.logo.is_sensitive());

        // Test all openings without changing the transcript or terminal grid.
        let gtk_settings = gtk::Settings::default().unwrap();
        let animations = gtk_settings.is_gtk_enable_animations();
        gtk_settings.set_gtk_enable_animations(true);
        for opening in [Opening::Fade, Opening::Lines, Opening::Shimmer] {
            this.greeting.opening.set_selected(opening.index());
            settle();
            let transcript = feed(&this);
            let grid = this.preview_terminal.column_count();
            this.greeting.replay.emit_clicked();
            assert!(this.greeting_motion.layer.is_visible());
            settle();
            assert!(this.greeting_motion.progress.get() > 0.0);
            assert_eq!(feed(&this), transcript);
            assert_eq!(this.preview_terminal.column_count(), grid);
            settle();
            settle();
            assert!(!this.greeting_motion.layer.is_visible());
            assert_eq!(feed(&this), transcript);
        }
        this.greeting.replay.emit_clicked();
        this.redraw_preview_contents();
        assert!(!this.greeting_motion.layer.is_visible());
        gtk_settings.set_gtk_enable_animations(false);
        this.greeting.replay.emit_clicked();
        assert!(!this.greeting_motion.layer.is_visible());
        gtk_settings.set_gtk_enable_animations(animations);
        this.greeting.opening.set_selected(Opening::None.index());
        settle();

        if std::env::var_os("TERMIMOCHI_GREETING_DRAG_TEST").is_some() {
            window.set_title(Some("TermiMochi point-to-edit test"));
            let scroll = this.greeting.root.vadjustment();
            let content = this.greeting.root.child().unwrap();
            let bounds = this.greeting.list.compute_bounds(&content).unwrap();
            scroll.set_value(f64::from(bounds.y()).min(scroll.upper() - scroll.page_size()));
            settle();
            let original = this.greeting.settings();
            let source = this
                .greeting
                .rows
                .iter()
                .find(|r| r.0 == Info::Os)
                .unwrap()
                .1
                .first_child()
                .unwrap();
            let destination = &this
                .greeting
                .rows
                .iter()
                .find(|r| r.0 == Info::Cpu)
                .unwrap()
                .1;
            let a = source.compute_bounds(&window).unwrap();
            let b = destination.compute_bounds(&window).unwrap();
            let scale = window.scale_factor() as f32;
            let coords = [
                (a.x() + a.width() / 2.0) * scale,
                (a.y() + a.height() / 2.0) * scale,
                (b.x() + b.width() / 2.0) * scale,
                (b.y() + b.height() * 0.75) * scale,
            ];
            let mut child = std::process::Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../scripts/preview-pointer-driver.py"
                ))
                .arg("drag_to")
                .args(coords.map(|v| (v.round() as i32).to_string()))
                .spawn()
                .unwrap();
            while child.try_wait().unwrap().is_none() {
                settle();
            }
            assert!(child.wait().unwrap().success());
            settle();
            let mut expected = original.clone();
            assert!(expected.move_item(Info::Os, Info::Cpu, true));
            assert_eq!(this.greeting.settings(), expected);
            this.undo_action.activate(None);
            assert_eq!(this.greeting.settings(), original);
            this.redo_action.activate(None);
            assert_eq!(this.greeting.settings(), expected);
            this.undo_action.activate(None);
            scroll.set_value(0.0);
            settle();
        }

        this.save_greeting_preset(); // Explicit secondary preset action.
        assert!(!this.greeting.dirty());
        let saved = this.greeting.settings();
        let path = root.path().join(PRESET_NAME);
        assert_eq!(
            DocumentStore::<GreetingPreset>::open(path.clone())
                .unwrap()
                .document()
                .unwrap()
                .unwrap()
                .greeting,
            saved
        );
        this.undo_action.activate(None);
        assert!(this.greeting.dirty());
        this.redo_action.activate(None);
        assert_eq!(this.greeting.settings(), saved);
        this.greeting.artwork.buffer().set_text("\x1b[2Junsafe");
        settle();
        assert!(this.greeting.invalid.get());
        assert!(!this.save_action.is_enabled());
        this.save_greeting_preset();
        assert_eq!(
            DocumentStore::<GreetingPreset>::open(path.clone())
                .unwrap()
                .document()
                .unwrap()
                .unwrap()
                .greeting,
            saved
        );
        this.undo_action.activate(None);
        assert!(!this.greeting.invalid.get());
        assert_eq!(this.greeting.settings(), saved);
        assert_eq!(
            (
                this.model.borrow().palette.clone(),
                this.typography_settings(),
                this.layout_settings(),
                this.prompt_settings.borrow().clone()
            ),
            before
        );
        let terminal = this.preview_terminal.clone();
        for action in [
            "show-typography",
            "show-layout",
            "show-palette",
            "show-prompt",
        ] {
            gio::prelude::ActionGroupExt::activate_action(&this.window(), action, None);
            settle();
            assert_eq!(this.preview_terminal, terminal);
            assert!(feed(&this).contains("Hello 你好 🦀"));
        }
        this.inspect_preview_target(PreviewTarget::GreetingFields);
        assert!(this.greeting_module_button.is_active());
        this.greeting.message.set_text("Do not lose me");
        window.close();
        respond("Cancel");
        assert!(window.is_visible());
        this.reload_greeting_preset();
        respond("Cancel");
        assert_eq!(this.greeting.settings().message, "Do not lose me");
        this.reload_greeting_preset();
        respond("Replace Changes");
        assert_eq!(this.greeting.settings(), saved);
        let workspace = root.path().join("complete.termimochi.json");
        this.save_workspace_path(workspace.clone(), this.workspace_snapshot())
            .unwrap();
        assert!(this.workspace_is_clean());
        this.greeting.message.set_text("Edited");
        assert!(!this.workspace_is_clean());
        this.open_workspace_path(&workspace);
        settle();
        assert_eq!(this.greeting.settings(), saved);
        assert!(this.workspace_is_clean());

        let export = root.path().join("config.jsonc");
        let original = b"// keep my formatting\n{\"modules\":[\"os\"]}\n";
        std::fs::write(&export, original).unwrap();
        this.confirm_fastfetch_export(export.clone(), this.greeting.settings());
        respond("Cancel");
        assert_eq!(std::fs::read(&export).unwrap(), original);
        assert!(!root.path().join("termimochi-backups").exists());
        this.confirm_fastfetch_export(export.clone(), this.greeting.settings());
        respond("Back Up & Replace");
        assert_eq!(
            std::fs::read_to_string(&export).unwrap(),
            this.greeting.settings().fastfetch_config().unwrap()
        );
        assert!(
            std::fs::read_dir(root.path().join("termimochi-backups"))
                .unwrap()
                .any(|entry| std::fs::read(entry.unwrap().path()).unwrap() == original)
        );
        this.confirm_fastfetch_export(export.clone(), this.greeting.settings());
        std::fs::write(&export, "external edit while dialog is open").unwrap();
        respond("Back Up & Replace");
        assert_eq!(
            std::fs::read_to_string(&export).unwrap(),
            "external edit while dialog is open"
        );

        if let Some(path) = std::env::var_os("TERMIMOCHI_GREETING_SCREENSHOT") {
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
        // Conflicting preset saves stay dirty and preserve the external file.
        let mut external = saved.clone();
        external.message = "External".into();
        DocumentStore::open(path.clone())
            .unwrap()
            .save(&GreetingPreset::new(external.clone()))
            .unwrap();
        this.greeting.message.set_text("My changes");
        this.save_greeting_preset();
        assert!(this.greeting.dirty());
        assert_eq!(
            DocumentStore::<GreetingPreset>::open(path)
                .unwrap()
                .document()
                .unwrap()
                .unwrap()
                .greeting,
            external
        );
        window.destroy();
        present_with_preset(&app, None, preset);
        let restored_window = app.active_window().unwrap();
        let restored = controller(&restored_window);
        settle();
        assert_eq!(restored.greeting.settings(), external);
        assert!(restored.greeting_preview.get());
        assert!(!restored.greeting.dirty());
        restored_window.destroy();
    }
}
