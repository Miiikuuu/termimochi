//! Current-document ownership. The existing editors remain the only mutable
//! settings; saved documents are immutable snapshots, references never write.
use super::*;
use crate::design_document::{DesignDocument, Kind, NativeSource, Scope, TargetHint};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

pub(super) struct TypedSession {
    pub kind: Cell<Kind>,
    pub scope: Cell<Scope>,
    pub target: Cell<Option<TargetHint>>,
    pub native: RefCell<Option<NativeSource>>,
    pub native_reference: RefCell<Option<Workspace>>,
    pub source_path: RefCell<Option<PathBuf>>,
    pub baseline: RefCell<Option<DesignDocument>>,
    pub store: RefCell<Option<DocumentStore<DesignDocument>>>,
    pub identity: Cell<u64>,
    pub document_id: RefCell<String>,
    pub deployment: RefCell<String>,
    pub busy: Cell<bool>,
    pub theme_origin: RefCell<Option<DesignDocument>>,
    pub theme_reference: RefCell<Option<Workspace>>,
    pub theme_history: RefCell<super::theme_workspace::ThemeHistory>,
    pub advanced: Cell<bool>,
    pub prompt_toggle: RefCell<Option<gtk::Button>>,
}

impl TypedSession {
    pub fn new() -> Self {
        Self {
            kind: Cell::new(Kind::Palette),
            scope: Cell::new(Scope::for_kind(Kind::Palette)),
            target: Cell::new(Some(TargetHint::Ptyxis)),
            native: RefCell::new(None),
            native_reference: RefCell::new(None),
            source_path: RefCell::new(None),
            baseline: RefCell::new(None),
            store: RefCell::new(None),
            identity: Cell::new(NEXT_SESSION.fetch_add(1, Ordering::Relaxed)),
            document_id: RefCell::new(glib::uuid_string_random().into()),
            deployment: RefCell::new(String::new()),
            busy: Cell::new(false),
            theme_origin: RefCell::new(None),
            theme_reference: RefCell::new(None),
            theme_history: RefCell::new(Default::default()),
            advanced: Cell::new(false),
            prompt_toggle: RefCell::new(None),
        }
    }
}

impl Workbench {
    pub(super) fn owns_module(&self, module: EditorModule) -> bool {
        if self.is_theme() {
            return true;
        }
        if self.typed.kind.get() == Kind::KittyAppearance {
            // These are addable appearance properties; unchanged inherited
            // fields remain absent in the source-only saved document.
            return matches!(
                module,
                EditorModule::Palette | EditorModule::Typography | EditorModule::Layout
            );
        }
        use crate::design_document::Action;
        let s = self.typed.scope.get();
        s.allows(match module {
            EditorModule::Palette => Action::Palette,
            EditorModule::Typography => Action::Typography,
            EditorModule::Layout => Action::Layout,
            EditorModule::Prompt => Action::Prompt,
            EditorModule::Greeting => Action::Artwork,
        })
    }

    /// Called at action boundaries as well as to project menu sensitivity.
    pub(super) fn allows_document_action(&self, name: &str) -> bool {
        if self.is_theme() {
            if matches!(
                name,
                "install-ptyxis"
                    | "rollback-ptyxis"
                    | "apply-typography"
                    | "apply-layout"
                    | "save-starship"
                    | "apply-fastfetch"
                    | "reload-terminal"
            ) {
                return false;
            }
            if name.starts_with("show-")
                || matches!(
                    name,
                    "import-fastfetch"
                        | "load-current-fastfetch"
                        | "import-greeting-art"
                        | "edit-image-artwork"
                        | "try-greeting"
                        | "reload-starship"
                        | "diagnostic-module"
                )
            {
                return true;
            }
        }
        if self.typed.kind.get() == Kind::Legacy
            && matches!(
                name,
                "install-ptyxis"
                    | "rollback-ptyxis"
                    | "apply-typography"
                    | "restore-typography"
                    | "apply-layout"
                    | "restore-layout"
                    | "save-starship"
                    | "restore-starship"
                    | "apply-fastfetch"
                    | "restore-fastfetch"
                    | "greeting-startup"
            )
        {
            // Advanced compatibility actions cannot bypass explicit v1 conversion.
            return false;
        }
        if self.typed.kind.get() == Kind::KittyAppearance {
            if matches!(name, "show-palette" | "show-typography" | "show-layout") {
                return true;
            }
            if (name.starts_with("export-") && name != "export-native")
                || matches!(
                    name,
                    "save-theme"
                        | "save-typography"
                        | "save-font-preset"
                        | "save-layout"
                        | "reload-typography"
                        | "reload-layout"
                )
            {
                return false;
            }
        }
        let s = self.typed.scope.get();
        let kitty = self.typed.target.get() == Some(TargetHint::Kitty);
        match name {
            "show-greeting" => s.greeting || s.artwork,
            "install-ptyxis" | "rollback-ptyxis" => s.palette && !kitty,
            "apply-typography" | "restore-typography" => s.typography && !kitty,
            "apply-layout" | "restore-layout" => s.layout && !kitty,
            "save-starship" | "restore-starship" => s.prompt && !kitty,
            "apply-fastfetch" | "restore-fastfetch" | "greeting-startup" => s.greeting && !kitty,
            "save-theme" | "export-theme" | "show-palette" => s.palette,
            "show-typography" | "save-typography" | "save-font-preset" | "export-typography"
            | "reload-typography" => s.typography,
            "show-layout" | "save-layout" | "export-layout" | "reload-layout" => s.layout,
            "show-prompt" | "export-starship" | "reload-starship" | "diagnostic-module" => s.prompt,
            "save-greeting"
            | "export-greeting"
            | "reload-greeting"
            | "load-current-fastfetch"
            | "import-fastfetch"
            | "export-fastfetch" => s.greeting,
            "try-greeting"
            | "import-greeting-art"
            | "export-greeting-txt"
            | "export-greeting-ans"
            | "edit-greeting-art-text"
            | "edit-image-artwork"
            | "export-pixel-greeting" => s.greeting || s.artwork,
            "reload-terminal" => s.palette && s.typography && s.layout && !kitty,
            "export-native" => true,
            action if action.starts_with("export-") => s.palette,
            _ => true,
        }
    }

    pub(super) fn require_document_action(&self, name: &str) -> bool {
        if self.allows_document_action(name) {
            true
        } else {
            self.toast("This operation is outside the current document or target. Open that document, or explicitly create a project copy.");
            false
        }
    }

    pub(super) fn design_snapshot(&self) -> Result<DesignDocument, String> {
        if let Some(origin) = self.typed.theme_origin.borrow().as_ref() {
            let mut design = origin.capture_theme(
                self.typed
                    .native_reference
                    .borrow()
                    .as_ref()
                    .ok_or("Missing theme reference")?,
                &self.workspace_snapshot(),
            )?;
            design.id = self.typed.document_id.borrow().clone();
            return Ok(design);
        }
        let native = self.typed.native.borrow().clone();
        let mut design = if self.typed.kind.get() == Kind::KittyAppearance
            && let Some(source) = native.as_ref()
        {
            let reference = self.typed.native_reference.borrow();
            let current = self.workspace_snapshot();
            let text = crate::kitty_document::KittyDocument::parse(&source.text)?
                .reconcile_with_reference(&current, reference.as_ref().unwrap_or(&current))?;
            DesignDocument::from_kitty_source(text)?
        } else {
            DesignDocument::from_workspace(
                self.typed.kind.get(),
                self.typed.scope.get(),
                &self.workspace_snapshot(),
                native,
            )?
        };
        design.target_hint = self.typed.target.get();
        design.id = self.typed.document_id.borrow().clone();
        design.validate()?;
        Ok(design)
    }

    pub(super) fn design_clean(&self) -> bool {
        let s = self.editable_document_scope();
        !(s.palette && self.has_draft()
            || s.prompt && self.starship_editor.invalid()
            || (s.greeting || s.artwork)
                && (self.greeting.invalid.get() || self.greeting.presentation_pending()))
            && self
                .design_snapshot()
                .ok()
                .as_ref()
                .is_some_and(|now| self.typed.baseline.borrow().as_ref() == Some(now))
    }

    pub(super) fn committed_design(&self) -> Result<DesignDocument, String> {
        let s = self.editable_document_scope();
        self.settle_active_edit();
        if s.typography {
            self.committed_typography()?;
        }
        if s.layout {
            self.committed_layout()?;
        }
        if s.greeting || s.artwork {
            self.greeting.require_presentation_ready()?;
            self.greeting.finish();
            if self.greeting.invalid.get() {
                return Err("Fix the artwork or Greeting fields first.".into());
            }
        }
        if s.palette && self.has_draft() || s.prompt && self.starship_editor.invalid() {
            return Err("Fix the highlighted document fields first.".into());
        }
        self.design_snapshot()
    }

    pub(super) fn document_inputs_valid(&self) -> bool {
        let scope = self.editable_document_scope();
        !(scope.palette && self.has_draft()
            || scope.prompt && self.starship_editor.invalid()
            || (scope.greeting || scope.artwork)
                && (self.greeting.invalid.get() || self.greeting.presentation_pending()))
    }

    fn editable_document_scope(&self) -> Scope {
        if self.is_theme() {
            return Scope::for_kind(Kind::Legacy);
        }
        if self.typed.kind.get() == Kind::KittyAppearance {
            Scope::for_kind(Kind::KittyAppearance)
        } else {
            self.typed.scope.get()
        }
    }

    pub(super) fn initialize_design(self: &Rc<Self>) {
        if !self.typed.advanced.get() {
            self.load_design(DesignDocument::new_theme(
                TargetHint::Ptyxis,
                "Untitled Theme",
                self.workspace_snapshot().light,
            ));
            return;
        }
        *self.typed.baseline.borrow_mut() = self.design_snapshot().ok();
        self.refresh_document_scope();
    }

    pub(super) fn refresh_document_scope(&self) {
        if let Some(button) = self.typed.prompt_toggle.borrow().as_ref() {
            button.set_visible(self.is_theme());
            let enabled = self
                .design_snapshot()
                .ok()
                .and_then(|d| d.theme)
                .is_some_and(|t| t.prompt_enabled);
            button.set_label(if enabled {
                "Prompt enabled — Turn Off"
            } else {
                "Enable Prompt in This Theme"
            });
        }
        if self.is_theme() && !self.updating.get() {
            if let Ok(design) = self.design_snapshot() {
                self.typed.scope.set(design.scope());
            }
            self.enforce_theme_target();
        }
        self.greeting
            .restrict_to_artwork(self.typed.scope.get().artwork);
        self.refresh_window_top();
        let pairs = [
            (EditorModule::Palette, &self.palette_module_button),
            (EditorModule::Typography, &self.typography_module_button),
            (EditorModule::Layout, &self.layout_module_button),
            (EditorModule::Prompt, &self.prompt_module_button),
            (EditorModule::Greeting, &self.greeting_module_button),
        ];
        for (module, button) in pairs {
            let owned = self.owns_module(module);
            button.set_visible(owned);
            button.set_sensitive(owned);
        }
        if !self.owns_module(self.output_module())
            && let Some((_, button)) = pairs.iter().find(|(m, _)| self.owns_module(*m))
        {
            button.set_active(true);
        }
        for name in self.window().list_actions() {
            if let Some(action) = self
                .window()
                .lookup_action(&name)
                .and_downcast::<gio::SimpleAction>()
                && !matches!(
                    name.as_str(),
                    "undo"
                        | "redo"
                        | "save"
                        | "install-ptyxis"
                        | "rollback-ptyxis"
                        | "save-starship"
                        | "restore-starship"
                        | "reload-starship"
                        | "export-starship"
                )
            {
                action.set_enabled(
                    self.allows_document_action(&name)
                        && (!matches!(name.as_str(), "save-workspace" | "save-workspace-as")
                            || self.document_inputs_valid()),
                );
            }
        }
        // Type ownership narrows business readiness; it must not re-enable an
        // invalid draft or override dependency/conflict-specific controls.
        self.refresh_prompt_save_actions();
        self.refresh_deployment();
        self.refresh_design_title();
    }

    pub(super) fn refresh_design_title(&self) {
        if let Ok(design) = self.design_snapshot()
            && let Some(theme) = &design.theme
        {
            let title = format!(
                "{} · {}",
                theme.name,
                super::theme_workspace::target_label(design.target_hint.unwrap())
            );
            self.window().set_title(Some(&format!(
                "{title} — TermiMochi{}",
                if self.design_clean() {
                    ""
                } else {
                    " • Modified"
                }
            )));
            self.brand_title.set_text(&title);
            return;
        }
        let name = self
            .typed
            .store
            .borrow()
            .as_ref()
            .and_then(|s| s.path.file_name().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "Untitled design".into());
        self.window().set_title(Some(&format!(
            "{name} · {} — TermiMochi{}",
            self.typed.kind.get().label(),
            if self.design_clean() {
                ""
            } else {
                " • Modified"
            }
        )));
    }

    pub(super) fn save_design(self: &Rc<Self>, save_as: bool) {
        let mut snapshot = match self.committed_design() {
            Ok(s) => s,
            Err(e) => {
                self.toast(&e);
                return;
            }
        };
        if !save_as && self.typed.store.borrow().is_some() {
            let result = self
                .typed
                .store
                .borrow_mut()
                .as_mut()
                .unwrap()
                .save(&snapshot);
            match result {
                Ok(()) => {
                    *self.typed.baseline.borrow_mut() = Some(snapshot);
                    self.refresh_history_actions();
                    self.toast("Design saved. No terminal configuration was changed.");
                }
                Err(e) => self.toast(&e),
            }
            return;
        }
        if save_as {
            // An explicitly named copy must not update the original Kitty entry.
            snapshot.id = glib::uuid_string_random().into();
        }
        let identity = self.typed.identity.get();
        let dialog = gtk::FileDialog::builder()
            .title("Save Current Design")
            .initial_name("design.termimochi-design.json")
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.save(Some(&self.window()), gio::Cancellable::NONE, move |result| {
            let Some(this) = weak.upgrade().filter(|w| w.typed.identity.get() == identity) else { return; };
            let Ok(file) = result else { return; };
            let result = file.path().ok_or("Choose a local file.".to_owned()).and_then(DocumentStore::<DesignDocument>::open).and_then(|mut store| {
                store.save(&snapshot)?;
                if save_as {
                    *this.typed.document_id.borrow_mut() = snapshot.id.clone();
                    if let Some(origin)=this.typed.theme_origin.borrow_mut().as_mut(){origin.id=snapshot.id.clone();}
                    *this.typed.theme_history.borrow_mut()=Default::default();
                    this.typed.identity.set(NEXT_SESSION.fetch_add(1, Ordering::Relaxed));
                    this.document_use.reset();
                    this.greeting.presentation.verified.borrow_mut().take();
                }
                *this.typed.store.borrow_mut() = Some(store);
                *this.typed.baseline.borrow_mut() = Some(snapshot);
                Ok(())
            });
            match result { Ok(()) => { this.refresh_history_actions(); this.toast("Current design saved. Preview references and local permissions were not exported."); }, Err(e) => this.toast(&e) }
        });
    }

    pub(super) fn choose_design_open(self: &Rc<Self>) {
        self.choose_design_open_mode(false);
    }
    fn choose_design_open_mode(self: &Rc<Self>, advanced: bool) {
        let dialog = gtk::FileDialog::builder()
            .title("Open Design or Native Configuration")
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.open(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                if let Ok(file) = result
                    && let Some(path) = file.path()
                    && let Some(app) = this
                        .window()
                        .application()
                        .and_downcast::<adw::Application>()
                {
                    // Separate controller means existing dialogs, history, workers and
                    // visual approvals can never become authority over the new file.
                    present_with_mode(
                        &app,
                        Some(path),
                        typography_preset::state_directory().join(typography_preset::PRESET_NAME),
                        advanced,
                    );
                }
            },
        );
    }

    pub(super) fn open_design_path(self: &Rc<Self>, path: &Path) {
        if crate::greeting_image::is_image(path)
            || path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "txt" | "ans"))
        {
            if !self.typed.advanced.get() {
                self.choose_artwork_theme(path.into());
                return;
            }
            match DesignDocument::from_workspace(
                Kind::Artwork,
                Scope::for_kind(Kind::Artwork),
                &self.workspace_snapshot(),
                None,
            ) {
                Ok(doc) => {
                    self.load_design(doc);
                    self.open_artwork_document_source(path.into());
                }
                Err(e) => self.toast(&e),
            }
            return;
        }
        let result = typography_preset::read_private_with_limit(path, DesignDocument::MAX_BYTES)
            .and_then(|b| b.ok_or("The document no longer exists.".into()))
            .and_then(|b| {
                if let Ok(crate::design_document::Detected::Ambiguous(kind)) =
                    crate::design_document::detect(&b)
                {
                    self.confirm_document_type(b, kind);
                    return Ok(None);
                }
                let native_fastfetch = matches!(
                    crate::design_document::detect(&b),
                    Ok(crate::design_document::Detected::Native(Kind::Greeting))
                );
                crate::design_document::import(&b, &self.workspace_snapshot())
                    .map(|doc| Some((doc, native_fastfetch)))
            });
        match result {
            Ok(Some((mut design, native_fastfetch))) => {
                if native_fastfetch
                    && let Some(greeting) = design.components.greeting.as_mut()
                    && greeting.source_logo.is_none()
                    && let Some(source) = greeting.imported_source.as_deref()
                {
                    match crate::greeting_art::snapshot_file(source, path) {
                        Ok(logo) => greeting.source_logo = logo,
                        Err(e) => self.toast(&format!(
                            "Source retained; referenced logo preview needs attention: {e}"
                        )),
                    }
                }
                if !self.typed.advanced.get() && design.theme.is_none() {
                    let target = design.target_hint.or(match design.kind {
                        Kind::Palette => Some(TargetHint::Ptyxis),
                        Kind::KittyAppearance => Some(TargetHint::Kitty),
                        _ => None,
                    });
                    if let Some(target) = target {
                        match design.into_theme(
                            target,
                            path.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Imported Theme"),
                        ) {
                            Ok(d) => design = d,
                            Err(e) => {
                                self.toast(&e);
                                return;
                            }
                        }
                    } else {
                        self.choose_import_theme_target(design);
                        return;
                    }
                }
                let is_v3 =
                    typography_preset::read_private_with_limit(path, DesignDocument::MAX_BYTES)
                        .ok()
                        .flatten()
                        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                        .is_some_and(|v| v["version"] == 3);
                self.load_design(design);
                *self.typed.source_path.borrow_mut() = Some(path.to_owned());
                if document_store::matches_path::<DesignDocument>(path)
                    && (is_v3 || self.typed.advanced.get())
                {
                    match DocumentStore::open(path.to_owned()) {
                        Ok(store) => *self.typed.store.borrow_mut() = Some(store),
                        Err(e) => self.toast(&e),
                    }
                }
                self.refresh_history_actions();
                self.toast(
                    "Opened as a design. Native sources and terminal settings were not modified.",
                );
            }
            Ok(None) => {}
            Err(e) => {
                self.initialize_design();
                self.toast(&format!("Could not open design: {e}"));
            }
        }
    }

    fn confirm_document_type(self: &Rc<Self>, bytes: Vec<u8>, kind: Kind) {
        let dialog = gtk::AlertDialog::builder().message("Confirm document type")
            .detail(format!("This file is valid but has no unambiguous type marker. Open as {}? Nothing will be executed or applied.", kind.label()))
            .buttons(["Cancel", "Open as Design"]).cancel_button(0).default_button(0).modal(true).build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if result == Ok(1)
                    && let Some(this) = weak.upgrade()
                {
                    match crate::design_document::import_as(
                        &bytes,
                        kind,
                        &this.workspace_snapshot(),
                    ) {
                        Ok(doc) => {
                            if this.typed.advanced.get() {
                                this.load_design(doc)
                            } else if let Some(target) = doc.target_hint {
                                match doc.into_theme(target, "Imported Theme") {
                                    Ok(theme) => this.load_design(theme),
                                    Err(e) => this.toast(&e),
                                }
                            } else {
                                this.choose_import_theme_target(doc)
                            }
                        }
                        Err(e) => this.toast(&e),
                    }
                }
            },
        );
    }

    pub(super) fn load_design(self: &Rc<Self>, design: DesignDocument) {
        if *self.typed.document_id.borrow() != design.id {
            *self.full_session.samples.state.borrow_mut() = Default::default();
            self.full_session.rendered.borrow_mut().clear();
            self.sync_sample_controls();
        }
        let references = if design.theme.is_some() {
            self.typed
                .theme_reference
                .borrow()
                .clone()
                .unwrap_or_else(|| self.workspace_snapshot())
        } else {
            self.workspace_snapshot()
        };
        if design.theme.is_some() && self.typed.theme_reference.borrow().is_none() {
            *self.typed.theme_reference.borrow_mut() = Some(references.clone());
        }
        let snapshot = design.project_preview(&references);
        *self.typed.native_reference.borrow_mut() = Some(snapshot.clone());
        self.finish_active_edit();
        self.discard_drafts();
        self.reset_prompt_preview();
        self.preview_generation
            .set(self.preview_generation.get().wrapping_add(1));
        self.typed
            .identity
            .set(NEXT_SESSION.fetch_add(1, Ordering::Relaxed));
        *self.typed.document_id.borrow_mut() = design.id.clone();
        self.typed.kind.set(design.kind);
        self.typed.scope.set(design.scope());
        self.typed.target.set(design.target_hint);
        self.typed.advanced.set(design.theme.is_none());
        if design.theme.is_none() {
            let p = &self.greeting.presentation;
            p.target.set_visible(true);
            p.target.set_sensitive(true);
            p.shared.set_sensitive(true);
            if let Some(row) = p.target.parent() {
                row.set_visible(true);
            }
        }
        self.output_bar.invalidate_context();
        *self.typed.native.borrow_mut() = design.native.clone();
        self.typed.store.borrow_mut().take();
        self.typed.source_path.borrow_mut().take();
        self.typed.deployment.borrow_mut().clear();
        self.document_use.reset();
        self.greeting.presentation.verified.borrow_mut().take();
        self.greeting.presentation.deployed.borrow_mut().take();
        self.greeting.invalidate_output_checks();
        self.detach_greeting_target();
        let updating = self.updating.replace(true);
        *self.typed.theme_origin.borrow_mut() = design.theme.as_ref().map(|_| design.clone());
        *self.typed.theme_history.borrow_mut() = Default::default();
        self.workspace_prompt_loaded.set(true);
        self.starship_editor
            .begin_detached(snapshot.starship.clone())
            .expect("validated design source");
        *self.model.borrow_mut() = Model::new(
            snapshot.palette().expect("validated palette"),
            snapshot.variant(),
            None,
            "Design".into(),
        );
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
        self.workspace_store.borrow_mut().take();
        self.workspace_baseline.borrow_mut().take();
        self.updating.set(updating);
        *self.typed.baseline.borrow_mut() = Some(design);
        self.refresh_all();
        self.refresh_document_scope();
        self.sync_prompt_page();
        let scene = match self.typed.kind.get() {
            Kind::Project | Kind::Legacy => preview_scene::PreviewScene::Full,
            Kind::Greeting | Kind::Artwork => preview_scene::PreviewScene::Greeting,
            Kind::Prompt => preview_scene::PreviewScene::Prompt,
            _ => preview_scene::PreviewScene::Terminal,
        };
        self.select_preview_scene(scene);
    }

    pub(super) fn install_document_actions(this: &Rc<Self>) {
        let toggle = gtk::Button::builder()
            .label("Enable Prompt in This Theme")
            .action_name("win.theme-toggle-prompt")
            .halign(gtk::Align::Start)
            .build();
        if let Some(content) = this
            .prompt_source_selector
            .parent()
            .and_then(|p| p.parent())
            .and_downcast::<gtk::Box>()
        {
            content.insert_child_after(&toggle, content.first_child().as_ref());
        }
        *this.typed.prompt_toggle.borrow_mut() = Some(toggle);
        for (name, operation) in [
            ("new-document", 0),
            ("document-copy", 1),
            ("kitty-library", 2),
            ("export-native", 3),
            ("document-capabilities", 4),
            ("document-target", 5),
            ("import-theme", 6),
            ("theme-settings", 7),
            ("advanced-document", 8),
            ("advanced-open", 9),
            ("theme-toggle-prompt", 10),
            ("theme-from-terminal", 11),
        ] {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(this);
            action.connect_activate(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    match operation {
                        0 => this.choose_new_document(),
                        1 => this.choose_document_copy(),
                        2 => this.show_kitty_library(),
                        3 => this.export_native_design(),
                        4 => this.show_document_capabilities(),
                        5 => {
                            if this.is_theme() {
                                this.convert_theme_target()
                            } else {
                                this.choose_document_use_target()
                            }
                        }
                        6 => this.choose_theme_import(),
                        7 => this.show_theme_settings(),
                        8 => this.choose_advanced_document(),
                        9 => this.choose_design_open_mode(true),
                        11 => this.choose_system_theme(),
                        _ => this.toggle_theme_prompt(),
                    }
                }
            });
            this.window().add_action(&action);
        }
    }

    fn show_document_capabilities(&self) {
        let design = match self.design_snapshot() {
            Ok(d) => d,
            Err(e) => {
                self.toast(&e);
                return;
            }
        };
        let (window, body, _) =
            super::scheme::dialog(&self.window(), "Document Capabilities", design.kind.label());
        body.append(&super::scheme::label("This table describes implemented operations. Needs requirement is not verified compatibility or write approval. Use Design checks local dependencies, the current target and its exact write scope."));
        for (operation, capability) in design.capabilities() {
            body.append(&super::scheme::label(&format!(
                "{} — {}\n{}",
                operation.label(),
                capability.label(),
                capability.detail()
            )));
        }
        window.present();
    }

    pub(super) fn export_native_design(self: &Rc<Self>) {
        let design = match self.committed_design() {
            Ok(d) => d,
            Err(e) => {
                self.toast(&e);
                return;
            }
        };
        let (name, text) = match design.native_export() {
            Ok(r) => r,
            Err(e) => {
                self.toast(&e);
                return;
            }
        };
        let dialog = gtk::FileDialog::builder()
            .title("Export Native Copy — not Apply")
            .initial_name(name)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        let identity = self.typed.identity.get();
        dialog.save(Some(&self.window()), gio::Cancellable::NONE, move |result| {
            let Some(this) = weak.upgrade().filter(|w| w.typed.identity.get() == identity) else { return; };
            let Ok(file) = result else { return; };
            let result = file.path().ok_or("Choose a local export destination.".to_owned()).and_then(|path| {
                if path.extension() != Path::new(name).extension() {
                    return Err(format!("Use the native file extension from {name}; shell startup files are never export targets."));
                }
                this.check_design_export_path(&path)?;
                let before = typography_preset::read_private_with_limit(&path, crate::design_document::DesignDocument::MAX_BYTES)?;
                if let Some(bytes) = &before {
                    typography_preset::retain_backup(&path.parent().ok_or("No export directory")?.join("termimochi-backups"), bytes)?;
                }
                typography_preset::write_checked_with_limit(&path, text.as_bytes(), &before, crate::design_document::DesignDocument::MAX_BYTES)
            });
            match result { Ok(()) => this.toast("Native copy exported. It is not enabled or authorized to execute."), Err(e) => this.toast(&e) }
        });
    }

    pub(super) fn check_design_export_path(&self, path: &Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or("Missing export directory")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let resolved = parent.join(path.file_name().ok_or("Missing export filename")?);
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| glib::home_dir().join(".local/state"));
        for protected in [glib::user_config_dir(), glib::user_data_dir(), state] {
            let protected = protected.canonicalize().unwrap_or(protected);
            if resolved.starts_with(protected) {
                return Err("Export a separate copy outside active configuration/data directories. Use Design to review native updates.".into());
            }
        }
        for source in [
            self.typed.source_path.borrow().clone(),
            self.typed.store.borrow().as_ref().map(|s| s.path.clone()),
        ]
        .into_iter()
        .flatten()
        {
            if paths_refer_to_same_file(&source, &resolved).map_err(|e| e.to_string())? {
                return Err("A native export cannot overwrite its source or design. Use an explicit native update.".into());
            }
        }
        // Opening applies the existing no-symlink/hard-link and size checks.
        typography_preset::read_private_with_limit(
            path,
            crate::design_document::DesignDocument::MAX_BYTES,
        )?;
        Ok(())
    }

    pub(super) fn open_design_window(&self, design: DesignDocument) {
        let Some(app) = self
            .window()
            .application()
            .and_downcast::<adw::Application>()
        else {
            return;
        };
        let old = app.windows();
        present(&app, None);
        if let Some(window) = app.windows().into_iter().find(|w| !old.contains(w)) {
            let controller = unsafe {
                window
                    .data::<Rc<Workbench>>("termimochi-workbench")
                    .map(|p| p.as_ref().clone())
            };
            if let Some(controller) = controller {
                controller.load_design(design);
            }
        }
    }

    fn choose_new_document(self: &Rc<Self>) {
        self.choose_new_theme();
    }

    pub(super) fn choose_advanced_document(self: &Rc<Self>) {
        let kinds = [
            Kind::Palette,
            Kind::Typography,
            Kind::Layout,
            Kind::Prompt,
            Kind::Greeting,
            Kind::Artwork,
            Kind::KittyAppearance,
        ];
        let labels: Vec<_> = std::iter::once("Cancel")
            .chain(kinds.iter().map(|k| k.label()))
            .collect();
        let dialog = gtk::AlertDialog::builder().message("New design document")
            .detail("Choose what this document owns. Other content is only a preview reference. For multiple components, use Create Project / Convert Copy.")
            .buttons(labels).cancel_button(0).default_button(0).modal(true).build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |choice| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                let Ok(index) = choice else {
                    return;
                };
                let Some(kind) = index
                    .checked_sub(1)
                    .and_then(|i| kinds.get(i as usize))
                    .copied()
                else {
                    return;
                };
                let mut reference = Workspace::new(
                    &PtyxisPalette::from_text(SAMPLE_PALETTE).unwrap(),
                    Variant::Light,
                    TypographySettings::default(),
                    LayoutSettings::default(),
                    PromptSettings::default(),
                    None,
                    true,
                );
                reference.greeting = crate::greeting::GreetingSettings::starter();
                match DesignDocument::from_workspace(kind, Scope::for_kind(kind), &reference, None)
                {
                    Ok(doc) => this.open_design_window(doc),
                    Err(e) => this.toast(&e),
                }
            },
        );
    }

    pub(super) fn choose_document_copy(self: &Rc<Self>) {
        if self.is_theme() {
            self.convert_theme_target();
            return;
        }
        let before = match self.committed_design() {
            Ok(d) => d,
            Err(e) => {
                self.toast(&e);
                return;
            }
        };
        let references = self.workspace_snapshot();
        let (window, body, buttons) = super::scheme::dialog(
            &self.window(),
            "Create a Separate Design Copy",
            "The current document stays unchanged.\nOnly checked components become owned by the copy.",
        );
        let kinds = [
            Kind::Project,
            Kind::Palette,
            Kind::Typography,
            Kind::Layout,
            Kind::Prompt,
            Kind::Greeting,
            Kind::Artwork,
        ];
        let kind =
            gtk::DropDown::from_strings(&kinds.iter().map(|k| k.label()).collect::<Vec<_>>());
        body.append(&kind);
        let target = gtk::DropDown::from_strings(&[
            "Kitty independent session",
            "Ptyxis — supported appearance and explicit shared components",
        ]);
        body.append(&target);
        let checks: Vec<_> = [
            "Palette",
            "Typography",
            "Layout",
            "Current Prompt",
            "Greeting",
        ]
        .into_iter()
        .map(|name| {
            let check = gtk::CheckButton::with_label(name);
            body.append(&check);
            check
        })
        .collect();
        let note = gtk::Label::builder().wrap(true).xalign(0.0).label("Select the components you want to include. Adding a preview reference is explicit here. Kitty uses a controlled Bash: personal aliases and startup integrations are not inherited.").build();
        body.append(&note);
        let create = gtk::Button::with_label("Review Conversion");
        buttons.append(&create);
        let weak = Rc::downgrade(self);
        let win = window.downgrade();
        create.connect_clicked(move |_| {
            let Some(this) = weak.upgrade() else { return; };
            let selected = kinds[kind.selected().min(6) as usize];
            let scope = if selected == Kind::Project { Scope { palette: checks[0].is_active(), typography: checks[1].is_active(), layout: checks[2].is_active(), prompt: checks[3].is_active(), greeting: checks[4].is_active(), artwork: false } } else { Scope::for_kind(selected) };
            let result = before.convert_copy(selected, scope, &references);
            let (mut copy, changes) = match result { Ok(r) => r, Err(e) => { note.set_text(&e); return; } };
            if selected == Kind::Project { copy.target_hint = Some(if target.selected() == 0 { TargetHint::Kitty } else { TargetHint::Ptyxis }); }
            let detail = if changes.is_empty() { "Owned content is retained. A new document identity is created; local bindings and visual approvals are not copied.".into() } else { changes.join("\n") };
            let confirm = gtk::AlertDialog::builder().message("Create this copy?").detail(detail).buttons(["Cancel", "Create Copy"]).cancel_button(0).default_button(0).modal(true).build();
            let weak = Rc::downgrade(&this); let win = win.clone();
            confirm.choose(Some(&this.window()), gio::Cancellable::NONE, move |result| {
                if result == Ok(1) && let Some(this) = weak.upgrade() {
                    if let Some(win) = win.upgrade() { win.close(); }
                    this.open_design_window(copy);
                }
            });
        });
        window.present();
    }
}
