//! Layout persistence and data-only complete workspace documents.
use super::*;

impl Workbench {
    pub(super) fn layout_dirty(&self) -> bool {
        self.layout_settings() != self.layout_baseline.get()
    }

    pub(super) fn set_layout_settings(&self, settings: LayoutSettings, record: bool) {
        let updating = self.updating.replace(true);
        self.content_padding_input
            .set_value(settings.content_padding.into());
        self.column_count_input.set_value(settings.columns as f64);
        self.row_count_input.set_value(settings.rows as f64);
        self.cursor_shape_selector
            .set_selected(settings.cursor_shape.index());
        self.cursor_blink_selector
            .set_selected(settings.cursor_blink.index());
        self.tab_bar_switch.set_active(settings.tab_bar);
        self.scrollbar_switch.set_active(settings.scrollbar);
        self.window_spacing_input
            .set_value(settings.window_spacing.into());
        if !record {
            self.last_layout.set(settings);
        }
        self.updating.set(updating);
        self.preview_input.borrow_mut().reset();
        self.refresh_layout();
        self.refresh_preview();
    }

    fn committed_layout(&self) -> Result<LayoutSettings, String> {
        for input in [
            &self.content_padding_input,
            &self.column_count_input,
            &self.row_count_input,
            &self.window_spacing_input,
        ] {
            input.update();
        }
        let settings = self.layout_settings();
        settings.validate()?;
        Ok(settings)
    }

    pub(super) fn save_layout_preset(&self) {
        let result = self.committed_layout().and_then(|settings| {
            self.layout_store
                .borrow_mut()
                .as_mut()
                .ok_or("The preset location is unavailable. Export a preset instead.")?
                .save(&LayoutPreset::new(settings))?;
            self.layout_baseline.set(settings);
            self.layout_has_saved.set(true);
            Ok(())
        });
        match result {
            Ok(()) => {
                self.refresh_layout();
                self.toast("Layout preset saved for the next launch. Ptyxis was not changed.");
            }
            Err(error) => self.toast(&format!("Could not save layout: {error}")),
        }
    }

    pub(super) fn reload_layout_preset(self: &Rc<Self>) {
        let path = self
            .layout_store
            .borrow()
            .as_ref()
            .map(|store| store.path.clone())
            .unwrap_or_else(|| {
                typography_preset::state_directory().join("layout.termimochi-layout.json")
            });
        match DocumentStore::<LayoutPreset>::open(path).and_then(|store| {
            let preset = store
                .document()?
                .ok_or("No layout preset has been saved yet.")?;
            Ok((store, preset))
        }) {
            Ok((store, preset)) => self.confirm_layout_discard(move |this| {
                *this.layout_store.borrow_mut() = Some(store);
                this.layout_baseline.set(preset.layout);
                this.layout_has_saved.set(true);
                this.set_layout_settings(preset.layout, true);
            }),
            Err(error) => self.toast(&format!("Could not reload layout: {error}")),
        }
    }

    fn document_dialog<T: Document>(title: &str) -> gtk::FileDialog {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(title));
        filter.add_pattern(&format!("*{}", T::SUFFIX));
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        gtk::FileDialog::builder()
            .title(title)
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build()
    }

    pub(super) fn choose_layout_export(self: &Rc<Self>) {
        let settings = match self.committed_layout() {
            Ok(settings) => settings,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let dialog = Self::document_dialog::<LayoutPreset>("Export Layout Preset");
        dialog.set_initial_name(Some("layout.termimochi-layout.json"));
        let weak = Rc::downgrade(self);
        dialog.save(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(file) => {
                        let result = file
                            .path()
                            .ok_or("Only local files can be saved.".into())
                            .and_then(DocumentStore::open)
                            .and_then(|mut store| store.save(&LayoutPreset::new(settings)));
                        match result {
                            Ok(()) => this.toast(
                                "Layout exported. Save Preset remembers it for the next launch.",
                            ),
                            Err(error) => this.toast(&format!("Could not export layout: {error}")),
                        }
                    }
                    Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                    Err(error) => this.toast(&error.to_string()),
                }
            },
        );
    }

    pub(super) fn choose_layout_open(self: &Rc<Self>) {
        let dialog = Self::document_dialog::<LayoutPreset>("Open Layout Preset");
        let weak = Rc::downgrade(self);
        dialog.open(Some(&self.window()), gio::Cancellable::NONE, move |result| {
            let Some(this) = weak.upgrade() else { return; };
            match result {
                Ok(file) => {
                    let result = file.path().ok_or("Only local files can be opened.".into())
                        .and_then(DocumentStore::<LayoutPreset>::open)
                        .and_then(|store| store.document())
                        .and_then(|preset| preset.ok_or("The layout file no longer exists.".into()));
                    match result {
                        Ok(preset) => this.confirm_layout_discard(move |this| {
                            this.set_layout_settings(preset.layout, true);
                            this.toast("Layout opened. Save Preset to remember it for the next launch.");
                        }),
                        Err(error) => this.toast(&format!("Could not open layout: {error}")),
                    }
                }
                Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                Err(error) => this.toast(&error.to_string()),
            }
        });
    }

    fn confirm_replace(
        self: &Rc<Self>,
        message: &str,
        detail: &str,
        action: impl FnOnce(Rc<Self>) + 'static,
    ) {
        let dialog = gtk::AlertDialog::builder()
            .message(message)
            .detail(detail)
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

    pub(super) fn confirm_layout_discard(self: &Rc<Self>, action: impl FnOnce(Rc<Self>) + 'static) {
        if !self.layout_dirty() || self.workspace_is_clean() {
            action(self.clone());
            return;
        }
        self.confirm_replace(
            "Replace unsaved layout changes?",
            "Save Preset or Save Workspace first to keep these settings.",
            action,
        );
    }

    pub(super) fn request_layout_apply(self: &Rc<Self>) {
        let request = match self
            .committed_layout()
            .and_then(|settings| layout_apply::ApplyRequest::discover(&settings))
        {
            Ok(request) => request,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let dialog = gtk::AlertDialog::builder()
            .message("Apply layout to Ptyxis?")
            .detail(request.detail())
            .buttons(["Cancel", "Back Up and Apply"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if result != Ok(1) {
                    return;
                }
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match request.apply(&typography_preset::state_directory()) {
                    Ok(Some(backup)) => this.toast(&format!(
                        "Layout applied. Window size affects new Ptyxis windows. Backup: {}",
                        backup.display()
                    )),
                    Ok(None) => {
                        this.toast("Ptyxis already uses this layout. Previous backup kept.")
                    }
                    Err(error) => this.toast(&format!("Could not apply layout: {error}")),
                }
            },
        );
    }

    pub(super) fn request_layout_restore(self: &Rc<Self>) {
        let request = match layout_apply::RestoreRequest::load(&typography_preset::state_directory())
        {
            Ok(request) => request,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let dialog = gtk::AlertDialog::builder().message("Restore previous Ptyxis layout?")
            .detail("Restore the six global layout settings from the last backup. Your preview, presets, fonts and shell files are unchanged.")
            .buttons(["Cancel", "Restore Layout"]).cancel_button(0).default_button(0).modal(true).build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if result != Ok(1) {
                    return;
                }
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match request.restore() {
                    Ok(()) => this.toast("Previous Ptyxis layout restored."),
                    Err(error) => this.toast(&format!("Could not restore layout: {error}")),
                }
            },
        );
    }

    pub(super) fn refresh_editor_menus(&self) {
        let (menu, label, tooltip) = if self.greeting_module_button.is_active() {
            (
                &self.greeting_save_menu,
                "Open Greeting Preset",
                "Open a greeting preset  Ctrl+O",
            )
        } else if self.layout_module_button.is_active() {
            (
                &self.layout_save_menu,
                "Open Layout Preset",
                "Open a layout preset  Ctrl+O",
            )
        } else if self.typography_module_button.is_active() {
            (
                &self.typography_save_menu,
                "Open Typography Preset",
                "Open a typography preset  Ctrl+O",
            )
        } else {
            (
                &self.save_menu,
                "Open Theme",
                "Open a Ptyxis .palette file  Ctrl+O",
            )
        };
        self.save_button.set_menu_model(Some(menu));
        self.open_button.set_tooltip_text(Some(tooltip));
        self.open_button
            .update_property(&[gtk::accessible::Property::Label(label)]);
        if let Some(popover) = self.save_button.popover() {
            popover.add_css_class("save-popover");
        }
        self.refresh_history_actions();
    }

    pub(super) fn workspace_snapshot(&self) -> Workspace {
        let model = self.model.borrow();
        let starship = self
            .starship_editor
            .draft
            .borrow()
            .as_ref()
            .map(|draft| draft.contents().to_owned());
        let mut snapshot = Workspace::new(
            &model.palette,
            model.active_variant,
            self.typography_settings(),
            self.layout_settings(),
            self.prompt_settings.borrow().clone(),
            starship,
            self.preview_prompt_source.get() == 1,
        );
        snapshot.greeting = self.greeting.settings();
        snapshot
    }

    pub(super) fn workspace_is_clean(&self) -> bool {
        !self.has_draft()
            && !self.greeting.invalid.get()
            && !self.starship_editor.invalid()
            && self
                .workspace_baseline
                .borrow()
                .as_ref()
                .is_some_and(|saved| *saved == self.workspace_snapshot())
    }

    pub(super) fn refresh_workspace_title(&self) {
        let path = self
            .workspace_store
            .borrow()
            .as_ref()
            .map(|store| store.path.clone());
        if let Some(path) = path {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let dirty = if self.workspace_is_clean() {
                ""
            } else {
                " • Modified"
            };
            self.window()
                .set_title(Some(&format!("{name} — TermiMochi{dirty}")));
            self.brand_title
                .set_tooltip_text(Some(&format!("{}{}", path.display(), dirty)));
        }
    }

    pub(super) fn has_unsaved_setup(&self) -> bool {
        if self.workspace_is_clean() {
            return false;
        }
        self.workspace_baseline.borrow().is_some()
            || self.model.borrow().dirty
            || self.has_draft()
            || self.prompt_has_unexported_changes()
            || self.typography_dirty()
            || self.layout_dirty()
            || self.greeting.dirty()
    }

    fn committed_workspace(&self) -> Result<Workspace, String> {
        self.greeting.finish();
        self.settle_active_edit();
        self.committed_typography()?;
        self.committed_layout()?;
        if self.has_draft() || self.starship_editor.invalid() || self.greeting.invalid.get() {
            return Err("Fix the highlighted fields before saving the workspace.".into());
        }
        // Capture the imported source once, then keep the portable document
        // independent of asynchronous discovery and host configuration changes.
        if !self.workspace_prompt_loaded.get() {
            if self.preview_loading.get() && self.starship_editor.draft.borrow().is_none() {
                return Err(
                    "The current prompt is still loading. Try saving again in a moment.".into(),
                );
            }
            let navigating = self.navigating_preview.replace(true);
            self.ensure_starship_copy();
            self.navigating_preview.set(navigating);
        }
        let snapshot = self.workspace_snapshot();
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub(super) fn save_workspace(self: &Rc<Self>) {
        if self.workspace_store.borrow().is_none() {
            self.choose_workspace_save_as();
            return;
        }
        let result = self.committed_workspace().and_then(|snapshot| {
            self.workspace_store
                .borrow_mut()
                .as_mut()
                .ok_or("No workspace is open.")?
                .save(&snapshot)?;
            *self.workspace_baseline.borrow_mut() = Some(snapshot);
            self.workspace_prompt_loaded.set(true);
            Ok(())
        });
        self.finish_workspace_save(result);
    }

    fn finish_workspace_save(&self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.refresh_all();
                self.toast("Workspace saved: colors, typography, layout, prompt and greeting. Terminal settings are unchanged.");
            }
            Err(error) => self.toast(&format!("Could not save workspace: {error}")),
        }
    }

    pub(super) fn save_workspace_path(
        &self,
        path: PathBuf,
        snapshot: Workspace,
    ) -> Result<(), String> {
        let mut store = DocumentStore::open(path)?;
        store.save(&snapshot)?;
        *self.workspace_store.borrow_mut() = Some(store);
        *self.workspace_baseline.borrow_mut() = Some(snapshot);
        self.workspace_prompt_loaded.set(true);
        Ok(())
    }

    pub(super) fn choose_workspace_save_as(self: &Rc<Self>) {
        let snapshot = match self.committed_workspace() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let dialog = Self::document_dialog::<Workspace>("Save Complete Workspace");
        dialog.set_initial_name(Some("my-setup.termimochi.json"));
        let weak = Rc::downgrade(self);
        dialog.save(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(file) => {
                        let result = file
                            .path()
                            .ok_or("Only local workspaces can be saved.".into())
                            .and_then(|path| this.save_workspace_path(path, snapshot));
                        this.finish_workspace_save(result);
                    }
                    Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                    Err(error) => this.toast(&error.to_string()),
                }
            },
        );
    }

    pub(super) fn choose_workspace_open(self: &Rc<Self>) {
        let dialog = Self::document_dialog::<Workspace>("Open Complete Workspace");
        let weak = Rc::downgrade(self);
        dialog.open(Some(&self.window()), gio::Cancellable::NONE, move |result| {
            let Some(this) = weak.upgrade() else { return; };
            match result {
                Ok(file) => {
                    let Some(path) = file.path() else { this.toast("Only local workspaces can be opened."); return; };
                    this.settle_active_edit();
                    if this.has_unsaved_setup() {
                        this.confirm_replace("Replace the current setup?", "Unsaved colors, typography, layout, prompt and greeting edits will be lost. Save Workspace first to keep the complete setup.", move |this| this.open_workspace_path(&path));
                    } else { this.open_workspace_path(&path); }
                }
                Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                Err(error) => this.toast(&error.to_string()),
            }
        });
    }

    pub(super) fn open_workspace_path(self: &Rc<Self>, path: &Path) {
        let result = DocumentStore::<Workspace>::open(path.to_owned()).and_then(|store| {
            let snapshot = store
                .document()?
                .ok_or("The workspace file no longer exists.")?;
            let palette = snapshot.palette()?;
            Ok((store, snapshot, palette))
        });
        let (store, snapshot, palette) = match result {
            Ok(result) => result,
            Err(error) => {
                self.toast(&format!("Could not open workspace: {error}"));
                return;
            }
        };
        // Entire document is valid before changing any control or binding.
        self.finish_active_edit();
        self.discard_drafts();
        self.reset_prompt_preview();
        self.workspace_prompt_loaded.set(true);
        let updating = self.updating.replace(true);
        self.starship_editor
            .begin_detached(snapshot.starship.clone())
            .expect("validated workspace prompt");
        *self.model.borrow_mut() =
            Model::new(palette, snapshot.variant(), None, "Workspace".into());
        *self.prompt_settings.borrow_mut() = snapshot.designer.clone();
        self.prompt_source_selector
            .set_selected(u32::from(snapshot.use_designer));
        self.preview_prompt_source
            .set(u32::from(snapshot.use_designer));
        self.set_typography_settings(&snapshot.typography, false);
        self.set_layout_settings(snapshot.layout, false);
        self.greeting.replace(snapshot.greeting.clone(), false);
        self.greeting.history.borrow_mut().clear();
        self.history.borrow_mut().clear();
        self.typography_history.borrow_mut().clear();
        self.layout_history.borrow_mut().clear();
        self.prompt_history.borrow_mut().clear();
        *self.selected_color_key.borrow_mut() = "Foreground".into();
        *self.workspace_store.borrow_mut() = Some(store);
        *self.workspace_baseline.borrow_mut() = Some(snapshot.clone());
        self.updating.set(updating);
        self.refresh_all();
        self.sync_prompt_page();
        if snapshot.use_designer || snapshot.starship.is_some() {
            self.activate_prompt_preview(u32::from(snapshot.use_designer));
            if !snapshot.use_designer {
                self.schedule_copy_preview();
            }
            self.redraw_preview_contents();
        }
        if snapshot.greeting.enabled {
            self.show_greeting_preview();
        }
        self.toast("Workspace opened. Terminal settings and shell files were not changed.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle() {
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_millis(250) {
            while glib::MainContext::default().pending() {
                glib::MainContext::default().iteration(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut result = vec![widget.clone()];
        let mut child = widget.first_child();
        while let Some(current) = child {
            result.extend(descendants(&current));
            child = current.next_sibling();
        }
        result
    }

    fn respond(label: &str) {
        settle();
        let button = gtk::Window::list_toplevels()
            .into_iter()
            .flat_map(|widget| descendants(&widget))
            .find_map(|widget| {
                widget
                    .downcast::<gtk::Button>()
                    .ok()
                    .filter(|button| button.label().as_deref() == Some(label))
            })
            .unwrap_or_else(|| panic!("dialog button missing: {label}"));
        button.emit_clicked();
        settle();
    }

    fn workbench(app: &adw::Application) -> Rc<Workbench> {
        let window = app.active_window().unwrap();
        let this = unsafe {
            window
                .data::<Rc<Workbench>>("termimochi-workbench")
                .unwrap()
                .as_ref()
                .clone()
        };
        settle();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while this.preview_loading.get() {
            assert!(
                std::time::Instant::now() < deadline,
                "folder discovery timed out"
            );
            settle();
        }
        this
    }

    #[test]
    #[ignore = "requires a graphical GTK/VTE session; run separately with --ignored --test-threads=1"]
    fn layout_and_workspace_save_restore_and_safety() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.WorkspaceTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        let preset_path = root.path().join(typography_preset::PRESET_NAME);
        let layout_path = root.path().join("layout.termimochi-layout.json");
        present_with_preset(&app, None, preset_path.clone());
        let this = workbench(&app);
        let palette = this.model.borrow().palette.clone();
        let typography = this.typography_settings();
        gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-layout", None);
        assert!(!this.layout_dirty());
        assert!(!this.undo_action.is_enabled());
        assert_eq!(
            this.save_button.menu_model(),
            Some(this.layout_save_menu.clone().upcast())
        );
        this.content_padding_input.set_value(17.0);
        this.column_count_input.set_value(93.0);
        this.row_count_input.set_value(27.0);
        this.cursor_shape_selector
            .set_selected(PreviewCursorShape::Underline.index());
        this.cursor_blink_selector
            .set_selected(PreviewCursorBlink::Off.index());
        this.tab_bar_switch.set_active(false);
        this.scrollbar_switch.set_active(true);
        this.window_spacing_input.set_value(13.0);
        assert!(this.layout_dirty());
        assert!(this.undo_action.is_enabled());
        let saved = this.layout_settings();
        this.save_action.activate(None);
        assert_eq!(
            DocumentStore::<LayoutPreset>::open(layout_path.clone())
                .unwrap()
                .document()
                .unwrap()
                .unwrap()
                .layout,
            saved
        );
        assert!(!this.layout_dirty());
        this.undo_action.activate(None);
        assert!(this.layout_dirty());
        this.redo_action.activate(None);
        assert!(!this.layout_dirty());
        assert_eq!(this.model.borrow().palette, palette);
        assert_eq!(this.typography_settings(), typography);
        assert_eq!(
            this.preview_terminal.cursor_shape(),
            saved.cursor_shape.vte_shape()
        );
        assert_eq!(
            this.preview_terminal.cursor_blink_mode(),
            saved.cursor_blink.vte_mode()
        );
        this.window_spacing_input.set_value(22.0);
        this.window().close();
        respond("Cancel");
        assert!(this.window().is_visible());
        this.reload_layout_preset();
        respond("Cancel");
        assert_eq!(this.layout_settings().window_spacing, 22);
        this.reload_layout_preset();
        respond("Replace Changes");
        assert_eq!(this.layout_settings(), saved);
        if layout_apply::ApplyRequest::discover(&saved).is_ok() {
            let before = import_current_appearance().layout;
            this.request_layout_apply();
            respond("Cancel");
            assert_eq!(import_current_appearance().layout, before);
        }
        let mut external = saved;
        external.columns = 99;
        DocumentStore::open(layout_path.clone())
            .unwrap()
            .save(&LayoutPreset::new(external))
            .unwrap();
        this.column_count_input.set_value(94.0);
        this.save_action.activate(None);
        assert!(this.layout_dirty());
        assert_eq!(
            DocumentStore::<LayoutPreset>::open(layout_path.clone())
                .unwrap()
                .document()
                .unwrap()
                .unwrap()
                .layout,
            external
        );
        this.window().destroy();
        present_with_preset(&app, None, preset_path.clone());
        let this = workbench(&app);
        assert_eq!(
            this.layout_settings(),
            external,
            "all eight fields survive restart"
        );
        assert!(!this.layout_dirty());

        // Create a different full setup. Portable Starship is data, not a file binding.
        let source = "# preserve me\nformat='$directory$rust$character'\n[rust]\nsymbol='rs '\n[custom.private]\ncommand='must-not-execute'\n";
        this.starship_editor
            .begin_detached(Some(source.into()))
            .unwrap();
        this.workspace_prompt_loaded.set(true);
        this.font_size_input.set_value(14.5);
        this.column_count_input.set_value(88.0);
        this.name_entry.set_text("Portable Setup");
        this.settle_active_edit();
        this.update_prompt_settings(|settings| settings.apply_preset(PromptPreset::Remote));
        this.activate_prompt_preview(0);
        let snapshot = this.committed_workspace().unwrap();
        let path = root.path().join("complete.termimochi.json");
        this.save_workspace_path(path.clone(), snapshot.clone())
            .unwrap();
        assert!(this.workspace_is_clean());
        assert!(!this.has_unsaved_setup());
        this.font_size_input.set_value(16.0);
        assert!(!this.workspace_is_clean());
        assert!(this.has_unsaved_setup());
        let invalid_path = root.path().join("invalid.termimochi.json");
        let mut invalid = snapshot.clone();
        invalid.layout.rows = 0;
        std::fs::write(&invalid_path, serde_json::to_vec(&invalid).unwrap()).unwrap();
        let before = this.workspace_snapshot();
        this.open_workspace_path(&invalid_path);
        assert_eq!(
            this.workspace_snapshot(),
            before,
            "invalid documents cannot partially replace a setup"
        );
        this.open_workspace_path(&path);
        assert_eq!(this.workspace_snapshot(), snapshot);
        assert!(this.workspace_is_clean());
        assert!(this.model.borrow().current_path.is_none());
        assert!(this.starship_editor.detached.get());
        assert!(this.starship_editor.file.borrow().is_err());
        assert_eq!(
            this.starship_editor
                .draft
                .borrow()
                .as_ref()
                .unwrap()
                .contents(),
            source
        );
        this.reload_starship_from_disk();
        assert_eq!(this.workspace_snapshot(), snapshot);
        assert_eq!(this.typography_status.text(), "Saved in workspace");
        assert_eq!(this.layout_status.text(), "Saved in workspace");
        for action in [
            "show-layout",
            "show-typography",
            "show-palette",
            "show-prompt",
        ] {
            gio::prelude::ActionGroupExt::activate_action(&this.window(), action, None);
            settle();
            assert_eq!(
                this.workspace_snapshot(),
                snapshot,
                "navigation must not edit saved setup"
            );
        }
        let mut external_workspace = snapshot.clone();
        external_workspace.layout.rows = 29;
        DocumentStore::open(path.clone())
            .unwrap()
            .save(&external_workspace)
            .unwrap();
        this.font_size_input.set_value(17.0);
        this.save_workspace();
        assert!(
            !this.workspace_is_clean(),
            "conflicted save cannot mark clean"
        );
        assert_eq!(
            DocumentStore::<Workspace>::open(path.clone())
                .unwrap()
                .document()
                .unwrap()
                .unwrap(),
            external_workspace
        );
        this.open_workspace_path(&path);
        assert!(this.workspace_is_clean());
        let saved_window = this.window();
        saved_window.close();
        settle();
        assert!(
            !saved_window.is_visible(),
            "saved workspace closes without a false unsaved warning"
        );

        // Initial file open must win over asynchronous host imports, including no imported prompt.
        external_workspace.starship = None;
        external_workspace.use_designer = true;
        DocumentStore::open(path.clone())
            .unwrap()
            .save(&external_workspace)
            .unwrap();
        present_with_preset(&app, Some(path), preset_path);
        let this = workbench(&app);
        assert_eq!(this.workspace_snapshot(), external_workspace);
        this.prompt_source_selector.set_selected(0);
        assert!(this.starship_editor.draft.borrow().is_none());
        assert!(
            !this.ensure_starship_copy(),
            "workspace absence must not import this machine's config"
        );
        this.window().destroy();
    }
}
