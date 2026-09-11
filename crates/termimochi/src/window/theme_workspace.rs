//! Target workspace orchestration, using the existing editor values and Project.
use super::*;
use crate::design_document::{DesignDocument, TargetHint};

#[cfg(test)]
#[path = "theme_workspace_tests.rs"]
mod tests;

#[derive(Default)]
pub(super) struct ThemeHistory {
    pub undo: Vec<DesignDocument>,
    pub redo: Vec<DesignDocument>,
    current: Option<DesignDocument>,
    last: Option<(EditorModule, std::time::Instant)>,
}
pub(super) fn target_label(target: TargetHint) -> &'static str {
    match target {
        TargetHint::Kitty => "Kitty",
        TargetHint::Ptyxis => "Ptyxis",
    }
}

impl Workbench {
    pub(super) fn theme_prompt_disabled(&self) -> bool {
        self.is_theme()
            && self
                .design_snapshot()
                .ok()
                .and_then(|d| d.theme)
                .is_some_and(|t| !t.prompt_enabled)
    }
    pub(super) fn toggle_theme_prompt(self: &Rc<Self>) {
        let Ok(mut next) = self.committed_design() else {
            return;
        };
        let Some(t) = next.theme.as_mut() else {
            return;
        };
        t.prompt_enabled = !t.prompt_enabled;
        if t.prompt_enabled && next.components.prompt.is_none() {
            let w = self.workspace_snapshot();
            next.components.prompt = Some(crate::design_document::PromptComponent {
                designer: w.designer,
                use_designer: w.use_designer || w.starship.is_none(),
                starship: w.starship,
            });
        }
        self.replace_theme(next, true);
    }
    pub(super) fn is_theme(&self) -> bool {
        self.typed.theme_origin.borrow().is_some()
    }
    pub(super) fn enforce_theme_target(&self) {
        let terminal = match self.typed.target.get() {
            Some(TargetHint::Kitty) => crate::pixel_trial::Terminal::Kitty,
            _ => crate::pixel_trial::Terminal::Ptyxis,
        };
        let p = &self.greeting.presentation;
        p.binding.borrow_mut().terminal = terminal;
        p.target.set_visible(false);
        p.target.set_sensitive(false);
        if let Some(row) = p.target.parent() {
            row.set_visible(false);
        }
        if terminal == crate::pixel_trial::Terminal::Kitty {
            p.shared.set_active(false);
            p.shared.set_sensitive(false);
        }
        if let Ok(design) = self.design_snapshot() {
            let t = design.theme.as_ref().unwrap();
            let font = if design.scope().typography {
                "Specified font fields"
            } else {
                "Inherited font · preview reference"
            };
            self.typography_status
                .set_text(&format!("{font} · Theme Settings manages inheritance"));
            self.typography_status.set_tooltip_text(Some(if terminal==crate::pixel_trial::Terminal::Kitty{"Unspecified font uses Kitty controlled-session defaults, not your daily config. The App shows a preview reference. Kitty weight uses family matching and may differ."}else{"Unspecified font fields keep the reviewed target values. Font family/size/weight affect all Ptyxis windows. Cell spacing affects the selected profile."}));
            self.layout_status.set_text(if t.layout.is_empty() {
                "Inherited layout · preview reference"
            } else {
                "Specified layout · target mapping is reviewed in Use Theme"
            });
        }
    }
    pub(super) fn record_theme_edit(&self) {
        let Ok(now) = self.design_snapshot() else {
            return;
        };
        let mut h = self.typed.theme_history.borrow_mut();
        if h.current.as_ref() == Some(&now) {
            return;
        }
        if let Some(before) = h.current.take() {
            let module = self.output_module();
            if !h
                .last
                .is_some_and(|(m, t)| m == module && t.elapsed() < Duration::from_millis(250))
            {
                h.undo.push(before);
                if h.undo.len() > 100 {
                    h.undo.remove(0);
                }
            }
            h.redo.clear();
            h.last = Some((module, std::time::Instant::now()));
        }
        h.current = Some(now);
    }
    pub(super) fn theme_undo(self: &Rc<Self>, redo: bool) {
        let Ok(now) = self.committed_design() else {
            return;
        };
        let target = {
            let mut h = self.typed.theme_history.borrow_mut();
            let target = if redo { h.redo.pop() } else { h.undo.pop() };
            if target.is_some() {
                if redo {
                    h.undo.push(now)
                } else {
                    h.redo.push(now)
                }
            }
            target
        };
        if let Some(target) = target {
            self.replace_theme(target, false);
        }
    }
    /// Apply an immutable theme edit snapshot without changing ID/target, saved
    /// baseline, local path or view scene. A new session epoch rejects old work.
    pub(super) fn replace_theme(self: &Rc<Self>, design: DesignDocument, record: bool) {
        let baseline = self.typed.baseline.borrow().clone();
        let store = self.typed.store.borrow_mut().take();
        let source = self.typed.source_path.borrow().clone();
        let scene = self.preview_scene_selector.selected();
        let module = self.output_module();
        if record {
            self.record_theme_edit();
        }
        let mut history = std::mem::take(&mut *self.typed.theme_history.borrow_mut());
        if record && let Some(before) = history.current.take() {
            history.undo.push(before);
            history.redo.clear();
        }
        history.current = Some(design.clone());
        history.last = None;
        self.load_design(design);
        *self.typed.baseline.borrow_mut() = baseline;
        *self.typed.store.borrow_mut() = store;
        *self.typed.source_path.borrow_mut() = source;
        *self.typed.theme_history.borrow_mut() = history;
        match module {
            EditorModule::Palette => &self.palette_module_button,
            EditorModule::Typography => &self.typography_module_button,
            EditorModule::Layout => &self.layout_module_button,
            EditorModule::Prompt => &self.prompt_module_button,
            EditorModule::Greeting => &self.greeting_module_button,
        }
        .set_active(true);
        self.select_preview_scene(match scene {
            1 => preview_scene::PreviewScene::Terminal,
            2 => preview_scene::PreviewScene::Prompt,
            3 => preview_scene::PreviewScene::Greeting,
            _ => preview_scene::PreviewScene::Full,
        });
        self.refresh_history_actions();
    }
    pub(super) fn choose_new_theme(self: &Rc<Self>) {
        let dialog=gtk::AlertDialog::builder().message("New terminal theme")
            .detail("One theme, one target. Edit colors, fonts, layout, Prompt and Greeting in the same workspace. Unspecified settings inherit.")
            .buttons(["Cancel","Kitty Theme","Ptyxis Theme"]).cancel_button(0).default_button(0).modal(true).build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |answer| {
                if let Ok(index @ 1..=2) = answer
                    && let Some(this) = weak.upgrade()
                {
                    this.open_design_window(DesignDocument::new_theme(
                        if index == 1 {
                            TargetHint::Kitty
                        } else {
                            TargetHint::Ptyxis
                        },
                        "Untitled Theme",
                        true,
                    ));
                }
            },
        );
    }
    pub(super) fn choose_import_theme_target(self: &Rc<Self>, design: DesignDocument) {
        let dialog=gtk::AlertDialog::builder().message("Which terminal is this theme for?")
            .detail("This input has no terminal target. All content is retained in memory. Choosing a target does not apply or authorize anything.")
            .buttons(["Cancel","Kitty","Ptyxis"]).cancel_button(0).default_button(0).modal(true).build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |answer| {
                if let Ok(index @ 1..=2) = answer
                    && let Some(this) = weak.upgrade()
                {
                    match design.into_theme(
                        if index == 1 {
                            TargetHint::Kitty
                        } else {
                            TargetHint::Ptyxis
                        },
                        "Imported Theme",
                    ) {
                        Ok(doc) => this.load_design(doc),
                        Err(e) => this.toast(&e),
                    }
                }
            },
        );
    }
    pub(super) fn choose_artwork_theme(self: &Rc<Self>, path: PathBuf) {
        let dialog=gtk::AlertDialog::builder().message("Import artwork into a terminal theme")
            .detail("Artwork does not select a terminal. Choose its workspace; no terminal will be started.")
            .buttons(["Cancel","New Kitty Theme","New Ptyxis Theme"]).cancel_button(0).default_button(0).modal(true).build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |answer| {
                if let Ok(index @ 1..=2) = answer
                    && let Some(this) = weak.upgrade()
                {
                    this.load_design(DesignDocument::new_theme(
                        if index == 1 {
                            TargetHint::Kitty
                        } else {
                            TargetHint::Ptyxis
                        },
                        "Artwork Theme",
                        true,
                    ));
                    this.open_artwork_document_source(path);
                }
            },
        );
    }
    pub(super) fn choose_theme_import(self: &Rc<Self>) {
        let chooser = gtk::FileDialog::builder()
            .title("Import into Current Theme")
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        let identity = self.typed.identity.get();
        chooser.open(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if let Some(this) = weak
                    .upgrade()
                    .filter(|w| w.typed.identity.get() == identity)
                    && let Ok(file) = result
                    && let Some(path) = file.path()
                {
                    this.import_into_theme(&path);
                }
            },
        );
    }
    pub(super) fn import_into_theme(self: &Rc<Self>, path: &Path) {
        if !self.is_theme() {
            self.toast("Open a target theme first.");
            return;
        }
        if crate::greeting_image::is_image(path)
            || path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| matches!(s, "txt" | "ans"))
        {
            self.open_artwork_document_source(path.into());
            return;
        }
        let bytes =
            match typography_preset::read_private_with_limit(path, DesignDocument::MAX_BYTES) {
                Ok(Some(b)) => b,
                Ok(None) => {
                    self.toast("Input no longer exists");
                    return;
                }
                Err(e) => {
                    self.toast(&e);
                    return;
                }
            };
        // The subsequent confirmation explicitly names the candidate type.
        let parsed = match crate::design_document::detect(&bytes) {
            Ok(crate::design_document::Detected::Ambiguous(kind)) => {
                crate::design_document::import_as(&bytes, kind, &self.workspace_snapshot())
            }
            _ => crate::design_document::import(&bytes, &self.workspace_snapshot()),
        };
        let mut imported = match parsed {
            Ok(d) => d,
            Err(e) => {
                self.toast(&e);
                return;
            }
        };
        if let Some(greeting) = imported.components.greeting.as_mut()
            && greeting.source_logo.is_none()
            && let Some(source) = greeting.imported_source.as_deref()
        {
            match crate::greeting_art::snapshot_file(source, path) {
                Ok(logo) => greeting.source_logo = logo,
                Err(e) => self.toast(&format!("Source retained; logo needs attention: {e}")),
            }
        }
        if let Some(target) = imported.target_hint
            && Some(target) != self.typed.target.get()
        {
            let prompt=gtk::AlertDialog::builder().message("This file belongs to another terminal")
                .detail("Open it as a separate target theme. The current theme and its terminal will stay unchanged.")
                .buttons(["Cancel","Open Separate Theme"]).cancel_button(0).default_button(0).modal(true).build();
            let weak = Rc::downgrade(self);
            prompt.choose(Some(&self.window()), gio::Cancellable::NONE, move |r| {
                if r == Ok(1)
                    && let Some(this) = weak.upgrade()
                {
                    match imported.into_theme(target, "Imported Theme") {
                        Ok(d) => this.open_design_window(d),
                        Err(e) => this.toast(&e),
                    }
                }
            });
            return;
        }
        let prompt=gtk::AlertDialog::builder().message("Import into this theme?")
            .detail(format!("Replace only the imported {} settings. Theme identity and terminal stay unchanged; Undo restores the previous content.",imported.kind.label()))
            .buttons(["Cancel","Import Settings"]).cancel_button(0).default_button(0).modal(true).build();
        let weak = Rc::downgrade(self);
        let before = self.design_snapshot().ok();
        let identity = self.typed.identity.get();
        prompt.choose(Some(&self.window()), gio::Cancellable::NONE, move |r| {
            if r == Ok(1)
                && let Some(this) = weak.upgrade()
            {
                if this.typed.identity.get() != identity || this.design_snapshot().ok() != before {
                    this.toast("Theme changed. Import again.");
                    return;
                }
                if let Err(e) = this.merge_theme_input(imported.clone()) {
                    this.toast(&e);
                }
            }
        });
    }
    pub(super) fn merge_theme_input(
        self: &Rc<Self>,
        imported: DesignDocument,
    ) -> Result<(), String> {
        let mut current = self.committed_design()?;
        let target = current.target_hint.ok_or("Choose a target")?;
        if imported.target_hint.is_some_and(|t| t != target) {
            return Err("Cross-terminal input requires a separate theme".into());
        }
        let imported = imported.into_theme(target, "Import")?;
        let theme = current.theme.as_mut().ok_or("Not a theme")?;
        let incoming = imported.theme.as_ref().unwrap();
        theme.typography.extend(incoming.typography.clone());
        theme.layout.extend(incoming.layout.clone());
        theme.colors.extend(incoming.colors.clone());
        theme.dark_colors.extend(incoming.dark_colors.clone());
        for (group, fields) in [
            ("typography", &incoming.typography),
            ("layout", &incoming.layout),
        ] {
            for key in fields.keys() {
                theme.inherit.remove(&format!("{group}.{key}"));
            }
        }
        for source in &incoming.sources {
            theme.retain(source.clone());
        }
        if let Some(native) = imported.native {
            let incoming = crate::kitty_document::KittyDocument::parse(&native.text)?;
            let base = current
                .native
                .as_ref()
                .map(|n| n.text.as_str())
                .unwrap_or("# Theme native settings\n");
            let combined =
                crate::kitty_document::KittyDocument::parse(base)?.edited(incoming.properties())?;
            for (key, value) in incoming.properties() {
                if let Some(color) = crate::kitty_document::palette_key(key) {
                    theme.colors.insert(color.clone(), value.clone());
                    theme.dark_colors.insert(color, value.clone());
                }
            }
            for (group, patch) in [
                ("typography", &mut theme.typography),
                ("layout", &mut theme.layout),
            ] {
                let removed: Vec<_> = patch
                    .keys()
                    .filter(|f| {
                        crate::design_document::theme::kitty_field(group, f)
                            .is_some_and(|k| incoming.properties().contains_key(k))
                    })
                    .cloned()
                    .collect();
                for field in removed {
                    patch.remove(&field);
                }
                theme.inherit.retain(|f| {
                    !f.split_once('.')
                        .filter(|(g, _)| *g == group)
                        .and_then(|(g, f)| crate::design_document::theme::kitty_field(g, f))
                        .is_some_and(|k| incoming.properties().contains_key(k))
                });
            }
            current.native = Some(crate::design_document::NativeSource {
                format: crate::design_document::NativeFormat::Kitty,
                text: combined,
            });
        }
        if imported.components.palette.is_some() {
            current.components.palette = imported.components.palette;
        }
        if imported.components.prompt.is_some() {
            current.components.prompt = imported.components.prompt;
            theme.prompt_enabled = incoming.prompt_enabled;
        }
        if imported.components.greeting.is_some() {
            current.components.greeting = imported.components.greeting;
        }
        current.validate()?;
        self.replace_theme(current, true);
        Ok(())
    }
    pub(super) fn show_theme_settings(self: &Rc<Self>) {
        let Ok(before) = self.committed_design() else {
            return;
        };
        let Some(theme) = before.theme.as_ref() else {
            return;
        };
        let (window, body, buttons) = super::scheme::dialog(
            &self.window(),
            "Theme Settings & Inheritance",
            target_label(before.target_hint.unwrap()),
        );
        let name = gtk::Entry::builder()
            .text(&theme.name)
            .placeholder_text("Theme name")
            .build();
        body.append(&name);
        let prompt = gtk::CheckButton::with_label("Enable this theme's Prompt");
        prompt.set_active(theme.prompt_enabled);
        body.append(&prompt);
        body.append(&super::scheme::label("Unspecified fields are editable preview references, not output. Kitty inheritance means controlled-session defaults, not your daily kitty.conf. Ptyxis inheritance uses its reviewed current settings. Unchecking an override removes only this theme's intent; shared system values are never reset blindly."));
        let mut rows = Vec::new();
        for (group, base) in [
            (
                "typography",
                crate::design_document::theme::fields(&TypographySettings::default()),
            ),
            (
                "layout",
                crate::design_document::theme::fields(&LayoutSettings::default()),
            ),
        ] {
            for field in base.keys() {
                let explicit = if group == "typography" {
                    theme.typography.contains_key(field)
                } else {
                    theme.layout.contains_key(field)
                } || before
                    .theme_native_source()
                    .and_then(|s| crate::kitty_document::KittyDocument::parse(&s).ok())
                    .is_some_and(|d| {
                        crate::design_document::theme::kitty_field(group, field)
                            .is_some_and(|k| d.properties().contains_key(k))
                    });
                let check = gtk::CheckButton::with_label(&format!(
                    "{group} · {field} — {}",
                    if explicit {
                        "Specified"
                    } else {
                        "Inherited / preview reference"
                    }
                ));
                check.set_active(explicit);
                body.append(&check);
                rows.push((group.to_owned(), field.clone(), check, explicit));
            }
        }
        let apply = gtk::Button::with_label("Update Theme");
        buttons.append(&apply);
        let weak = Rc::downgrade(self);
        let win = window.downgrade();
        apply.connect_clicked(move |_| {
            let Some(this) = weak.upgrade() else {
                return;
            };
            if this.design_snapshot().ok().as_ref() != Some(&before) {
                this.toast("Theme changed. Review inheritance again.");
                return;
            }
            let mut next = before.clone();
            let t = next.theme.as_mut().unwrap();
            t.name = name.text().to_string();
            t.prompt_enabled = prompt.is_active();
            if t.prompt_enabled && next.components.prompt.is_none() {
                let w = this.workspace_snapshot();
                next.components.prompt = Some(crate::design_document::PromptComponent {
                    designer: w.designer,
                    starship: w.starship,
                    use_designer: w.use_designer,
                });
            }
            let current = this.workspace_snapshot();
            for (group, key, check, initial) in &rows {
                if check.is_active() == *initial {
                    continue;
                }
                let values = if group == "typography" {
                    crate::design_document::theme::fields(&current.typography)
                } else {
                    crate::design_document::theme::fields(&current.layout)
                };
                let patch = if group == "typography" {
                    &mut t.typography
                } else {
                    &mut t.layout
                };
                if check.is_active() {
                    if let Some(v) = values.get(key) {
                        patch.insert(key.clone(), v.clone());
                    }
                    t.inherit.remove(&format!("{group}.{key}"));
                } else {
                    patch.remove(key);
                    t.inherit.insert(format!("{group}.{key}"));
                }
            }
            match next.validate() {
                Ok(()) => {
                    if let Some(win) = win.upgrade() {
                        win.close();
                    }
                    this.replace_theme(next, true);
                }
                Err(e) => this.toast(&e),
            }
        });
        window.present();
    }
    pub(super) fn convert_theme_target(self: &Rc<Self>) {
        let Ok(before) = self.committed_design() else {
            return;
        };
        let target = if before.target_hint == Some(TargetHint::Kitty) {
            TargetHint::Ptyxis
        } else {
            TargetHint::Kitty
        };
        let dialog=gtk::AlertDialog::builder().message(format!("Create a {} theme copy?",target_label(target)))
            .detail("The original theme and published entry are unchanged. Source text and assets are retained. Native terminal-only directives are kept inert; font/layout equivalents may differ. Pixel Greeting is not supported by the Ptyxis adapter and must be explicitly changed or disabled there. No visual approval is copied.")
            .buttons(["Cancel","Create Theme Copy"]).cancel_button(0).default_button(0).modal(true).build();
        let weak = Rc::downgrade(self);
        dialog.choose(Some(&self.window()), gio::Cancellable::NONE, move |r| {
            if r == Ok(1)
                && let Some(this) = weak.upgrade()
            {
                match before.convert_theme_copy(target, &this.workspace_snapshot()) {
                    Ok(copy) => this.open_design_window(copy),
                    Err(e) => this.toast(&e),
                }
            }
        });
    }
}
