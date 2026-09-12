//! One persistent output area, separate from scrolling properties and preview.
use super::*;

pub(super) struct OutputBar {
    pub root: gtk::Box,
    title: gtk::Label,
    primary: gtk::Button,
    trial: gtk::Button,
    more: gtk::MenuButton,
    module: Cell<Option<EditorModule>>,
    designer: Cell<bool>,
}

impl OutputBar {
    pub fn invalidate_context(&self) {
        self.module.set(None);
    }
    #[cfg(test)]
    pub fn more_button(&self) -> &gtk::MenuButton {
        &self.more
    }
    pub fn new(save: &gtk::MenuButton) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        root.add_css_class("editor-output-bar");
        let title = gtk::Label::builder()
            .label("Palette")
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["output-context"])
            .build();
        let primary = gtk::Button::builder()
            .label("Apply Scheme…")
            .css_classes(["output-primary"])
            .build();
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("Import, export and recovery")
            .css_classes(["tool-menu"])
            .build();
        let trial = gtk::Button::builder()
            .label("Try Greeting")
            .action_name("win.try-greeting")
            .tooltip_text(
                "Try only the greeting in the target terminal, not the whole scheme · no configuration changes",
            )
            .css_classes(["flat"])
            .build();
        trial.update_property(&[gtk::accessible::Property::Label("Try Greeting")]);
        actions.append(&title);
        actions.append(save);
        actions.append(&trial);
        actions.append(&primary);
        actions.append(&more);
        root.append(&actions);
        Self {
            root,
            title,
            primary,
            trial,
            more,
            module: Cell::new(None),
            designer: Cell::new(false),
        }
    }
}

impl Workbench {
    pub(super) fn output_module(&self) -> EditorModule {
        if self.greeting_module_button.is_active() {
            EditorModule::Greeting
        } else if self.prompt_module_button.is_active() {
            EditorModule::Prompt
        } else if self.layout_module_button.is_active() {
            EditorModule::Layout
        } else if self.typography_module_button.is_active() {
            EditorModule::Typography
        } else {
            EditorModule::Palette
        }
    }

    pub(super) fn refresh_output_bar(&self) {
        let presentation = &self.greeting.presentation;
        self.output_bar
            .trial
            .set_visible(self.typed.scope.get().greeting || self.typed.scope.get().artwork);
        presentation
            .target_bar
            .set_visible(self.typed.scope.get().greeting || self.typed.scope.get().artwork);
        if presentation.target_bar.parent().is_none() {
            self.output_bar.root.prepend(&presentation.target_bar);
        }
        let binding = presentation.binding.borrow();
        let settings = self.greeting.settings();
        let resolved = settings.presentation.resolve(&settings, binding.terminal);
        presentation.cells.set([
            self.preview_terminal.char_width().max(1) as u32,
            self.preview_terminal.char_height().max(1) as u32,
        ]);
        if presentation.canvas.parent().is_none()
            && let Some(terminal) = self.inspect_layer.child()
        {
            self.inspect_layer.set_child(None::<&gtk::Widget>);
            let stack = gtk::Stack::builder()
                .vhomogeneous(false)
                .hhomogeneous(false)
                .build();
            stack.add_named(&terminal, Some("terminal"));
            stack.add_named(&presentation.canvas, Some("pixels"));
            self.inspect_layer.set_child(Some(&stack));
        }
        if let Some(stack) = self.inspect_layer.child().and_downcast::<gtk::Stack>() {
            let pixels = self.greeting_preview.get()
                && !self.full_session.active.get()
                && settings.enabled
                && resolved.as_ref().is_ok_and(|s| s.protocol.is_some());
            stack.set_visible_child_name(if pixels { "pixels" } else { "terminal" });
            if pixels {
                let text: String = self
                    .greeting_parts_for_width(160)
                    .into_iter()
                    .filter(|(_, part)| *part != crate::greeting::GreetingPart::Artwork)
                    .map(|(text, _)| text)
                    .collect();
                let plain = crate::greeting_art::Artwork::parse(&text)
                    .map(|a| a.plain)
                    .unwrap_or_else(|_| "Field preview unavailable · Try uses safe samples".into());
                presentation.fields.set_label(
                    &plain
                        .lines()
                        .map(str::trim_start)
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
                presentation.fields.set_tooltip_text(Some("Design information from the existing safe preview. Imported commands are not executed; Try uses safe sample fields."));
            }
        }
        let checked = self.cached_greeting_check();
        let verified = !self.greeting.presentation_pending()
            && checked
                .as_ref()
                .and_then(|c| c.key.as_ref().ok())
                .is_some_and(|key| {
                    presentation
                        .verified
                        .borrow()
                        .as_ref()
                        .is_some_and(|(k, _)| k == key)
                });
        use crate::greeting_output::Destination;
        let destination = if self.greeting.presentation_pending() {
            "Updating artwork… Save, Try and Apply wait for the result.".to_owned()
        } else {
            match &resolved {
                Ok(spec) => match spec.destination(&binding) {
                    Destination::SharedCharacter | Destination::SharedImage => {
                        "Shared greeting · affects every reader".to_owned()
                    }
                    Destination::IndependentImage => {
                        "Independent image · shared greeting unchanged".to_owned()
                    }
                },
                Err(error) => format!("Output unavailable: {error}"),
            }
        };
        let deployment = presentation.deployed.borrow();
        let applied = match deployment.as_ref() {
            Some((design, target, _, independent))
                if *design == settings
                    && *target == *binding
                    && checked.as_ref().is_some_and(|c| c.deployment_matches) =>
            {
                if *independent {
                    "Installed · not enabled"
                } else {
                    "Greeting applied"
                }
            }
            Some(_) if checked.is_none() => "Checking applied configuration…",
            Some(_) => "Greeting has pending changes / target differs",
            None => "Application not compared · review before applying",
        };
        let profile = checked
            .as_ref()
            .map(|c| c.profile.as_str())
            .unwrap_or("checking…");
        presentation.state.set_label(&format!(
            "{} · {}\n{}\n{applied}\nPtyxis profile: {profile} · font is global",
            if self.workspace_is_clean() {
                "Scheme saved"
            } else {
                "Scheme unsaved"
            },
            if verified {
                "Visual check confirmed"
            } else if checked.is_none() {
                "Checking target environment…"
            } else {
                "Visual check unverified"
            },
            destination
        ));
        let module = self.output_module();
        let designer =
            self.prompt_source_selector.selected() == 1 || self.starship_editor.detached.get();
        let bar = &self.output_bar;
        if self.is_theme() {
            let kitty = self.typed.target.get() == Some(crate::design_document::TargetHint::Kitty);
            if kitty {
                presentation.shared.set_visible(false);
            }
            presentation.state.set_label(if kitty {"Kitty theme · independent session\nUse Theme reviews this entire theme; Try Greeting tests only Greeting."}else{"Ptyxis theme · reviewed shared settings\nUse Theme shows profile/global effects; no complete session trial."});
            presentation.state.set_tooltip_text(Some("Unspecified settings are inherited preview references. Theme Settings & Inheritance lists overrides. Save only saves the theme; Use Theme requires a separate review."));
        }
        bar.title.set_text(if self.is_theme() {
            "Theme"
        } else {
            self.typed.kind.get().label()
        });
        if bar.module.get() == Some(module) && bar.designer.get() == designer {
            return;
        }
        bar.module.set(Some(module));
        bar.designer.set(designer);
        let save = gio::Menu::new();
        let more = gio::Menu::new();
        let (save_label, apply_label, apply_action, detail) = match module {
            EditorModule::Palette => (
                "Save Theme",
                "Install…",
                "win.install-ptyxis",
                "Install this palette in Ptyxis; selecting it in the terminal is a separate step.",
            ),
            EditorModule::Typography => (
                "Save Typography Preset",
                "Apply…",
                "win.apply-typography",
                "Review font changes for Ptyxis before applying.",
            ),
            EditorModule::Layout => (
                "Save Layout Preset",
                "Apply…",
                "win.apply-layout",
                "Review supported Ptyxis settings. Exact padding, tab bar and window spacing remain preview-only.",
            ),
            EditorModule::Greeting => (
                "Save Greeting Preset",
                "Apply…",
                "win.apply-fastfetch",
                "Review and back up the external Fastfetch configuration before applying.",
            ),
            EditorModule::Prompt if designer => (
                "Export Starship…",
                "Export…",
                "win.export-starship",
                "Export this prompt to a chosen file; this is not automatic activation.",
            ),
            EditorModule::Prompt => (
                "Save Changes…",
                "Apply…",
                "win.save-starship",
                "Review and back up changes to your current Starship configuration. Shell startup is unchanged.",
            ),
        };
        // Prompt's main action already owns writing/exporting Starship. Save in
        // that module instead offers a workspace snapshot, without a duplicate.
        if module != EditorModule::Prompt {
            let item = gio::MenuItem::new(
                Some(save_label),
                Some(match module {
                    EditorModule::Palette => "win.save-theme",
                    EditorModule::Typography => "win.save-font-preset",
                    EditorModule::Layout => "win.save-layout",
                    _ => "win.save-greeting",
                }),
            );
            set_menu_verb_icon(&item, "termimochi-save-symbolic");
            more.append_item(&item);
        }
        save.append(
            Some(if self.is_theme() {
                "Save Theme"
            } else {
                "Save Design"
            }),
            Some(if module == EditorModule::Prompt {
                "win.save"
            } else {
                "win.save-workspace"
            }),
        );
        save.append(
            Some(if self.is_theme() {
                "Save Theme As…"
            } else {
                "Save Design As…"
            }),
            Some(if module == EditorModule::Prompt {
                "win.save-as"
            } else {
                "win.save-workspace-as"
            }),
        );
        more.append(Some("Open Theme…"), Some("win.open-workspace"));
        more.append(Some("New Theme…"), Some("win.new-document"));
        more.append(Some("From My Terminal…"), Some("win.theme-from-terminal"));
        if self.is_theme() {
            more.append(Some("Import into Current Theme…"), Some("win.import-theme"));
            more.append(
                Some("Theme Settings & Inheritance…"),
                Some("win.theme-settings"),
            );
        }
        let advanced = gio::Menu::new();
        advanced.append(
            Some("New Single-component Document…"),
            Some("win.advanced-document"),
        );
        advanced.append(
            Some("Open Single-component Document…"),
            Some("win.advanced-open"),
        );
        more.append_submenu(Some("Advanced"), &advanced);
        more.append(Some("Export Native Copy…"), Some("win.export-native"));
        more.append(
            Some("Document Capabilities…"),
            Some("win.document-capabilities"),
        );
        if !self.is_theme() {
            more.append(Some("Choose Use Target…"), Some("win.document-target"));
        }
        more.append(
            Some(if self.is_theme() {
                "Convert Terminal — Create Copy…"
            } else {
                "Create Project / Convert Copy…"
            }),
            Some("win.document-copy"),
        );
        more.append(
            Some("Open Independent Kitty Scheme…"),
            Some("win.kitty-library"),
        );
        more.append(
            Some("Last Application & Recovery…"),
            Some("win.last-scheme-application"),
        );
        more.append(
            Some(&format!("{} — {apply_label}", module.label())),
            Some(apply_action),
        );
        match module {
            EditorModule::Palette => {
                more.append(Some("Save Theme As…"), Some("win.export-theme"));
                let exports = gio::Menu::new();
                for format in ExportFormat::ALL {
                    exports.append(
                        Some(&format!("Export {}…", format.display_name())),
                        Some(&format!("win.export-{}", format.as_str())),
                    );
                }
                more.append_submenu(Some("Export Theme"), &exports);
                more.append(Some("Restore Previous Theme…"), Some("win.rollback-ptyxis"));
            }
            EditorModule::Typography => {
                more.append(
                    Some("Export Typography Preset…"),
                    Some("win.export-typography"),
                );
                more.append(
                    Some("Reload Saved Typography…"),
                    Some("win.reload-typography"),
                );
                more.append(
                    Some("Restore Previous Typography…"),
                    Some("win.restore-typography"),
                );
            }
            EditorModule::Layout => {
                more.append(Some("Export Layout Preset…"), Some("win.export-layout"));
                more.append(Some("Reload Saved Layout…"), Some("win.reload-layout"));
                more.append(Some("Restore Previous Layout…"), Some("win.restore-layout"));
            }
            EditorModule::Prompt => {
                more.append(Some("Save Starship As…"), Some("win.export-starship"));
                more.append(Some("Reload from Disk…"), Some("win.reload-starship"));
                more.append(
                    Some("Restore Previous Starship…"),
                    Some("win.restore-starship"),
                );
            }
            EditorModule::Greeting => {
                let exports = gio::Menu::new();
                for (label, action) in [
                    ("Export Greeting Preset…", "export-greeting"),
                    ("Export Fastfetch Configuration…", "export-fastfetch"),
                    ("Export Logo as TXT…", "export-greeting-txt"),
                    ("Export Logo as ANSI…", "export-greeting-ans"),
                    ("Export Image Greeting…", "export-pixel-greeting"),
                ] {
                    exports.append(Some(label), Some(&format!("win.{action}")));
                }
                more.append_submenu(Some("Export"), &exports);
                let sources = gio::Menu::new();
                for (label, action) in [
                    ("Load Current Configuration", "load-current-fastfetch"),
                    ("Import Fastfetch Configuration…", "import-fastfetch"),
                    ("Refresh System Snapshot", "refresh-preview-folder"),
                ] {
                    sources.append(Some(label), Some(&format!("win.{action}")));
                }
                more.append_submenu(Some("Sources"), &sources);
                let recovery = gio::Menu::new();
                for (label, action) in [
                    ("Reload Saved Greeting…", "reload-greeting"),
                    ("Restore Previous Configuration…", "restore-fastfetch"),
                ] {
                    recovery.append(Some(label), Some(&format!("win.{action}")));
                }
                more.append_submenu(Some("Recovery"), &recovery);
                more.append(Some("Terminal Startup…"), Some("win.greeting-startup"));
            }
        }
        bar.title.set_text(if self.is_theme() {
            "Theme"
        } else {
            self.typed.kind.get().label()
        });
        bar.primary.set_label(if self.is_theme() {
            "Use Theme…"
        } else {
            "Use Design…"
        });
        bar.primary.set_action_name(Some("win.apply-scheme"));
        bar.primary.set_tooltip_text(Some(
            "Use only this document's content in its explicit target. Review before any writes.",
        ));
        bar.more.set_tooltip_text(Some(detail));
        bar.primary
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "{} — Apply Scheme",
                module.label()
            ))]);
        self.save_button.set_menu_model(Some(&save));
        self.save_button
            .set_tooltip_text(Some("Save the current design document · Ctrl+S"));
        bar.more.set_menu_model(Some(&more));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::greeting::tests::{descendants, project_controller as controller, settle};

    #[test]
    #[ignore = "isolated GTK/VTE: resolved output status agrees with the reviewed action"]
    fn resolved_greeting_destination_matches_status_and_review() {
        use crate::{greeting_output::Visual, scheme_apply::Action};
        assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.OutputDestinationTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_advanced_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let this = crate::window::greeting::tests::greeting_controller(&window);
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        let daily = root.path().join("config.jsonc");
        let original = "// untouched\n{}";
        std::fs::write(&daily, original).unwrap();
        this.accept_scheme_fastfetch(crate::fastfetch_apply::Target::open(daily.clone()).unwrap());
        let mut settings = this.greeting.settings();
        settings.editable_artwork = None;
        for visual in [Visual::Auto, Visual::Character] {
            settings.presentation.visual = visual;
            this.greeting.replace(settings.clone(), false);
            for shared in [false, true] {
                this.greeting.presentation.shared.set_active(shared);
                this.refresh_output_bar();
                let status = this.greeting.presentation.state.text();
                assert!(
                    status.contains("Shared greeting · affects every reader"),
                    "{status}"
                );
                assert!(!status.contains("Independent image"));
                let (detail, _, action) = this.prepare_greeting_action().unwrap();
                assert!(detail.contains("Character greeting · shared configuration"));
                let Action::Fastfetch { target, .. } = action else {
                    panic!("resolved characters must prepare the shared character action");
                };
                assert_eq!(target.path, daily);
            }
        }
        settings.presentation.visual = Visual::Image;
        this.greeting.replace(settings, false);
        this.refresh_output_bar();
        let error = this.prepare_greeting_action().err().unwrap();
        let status = this.greeting.presentation.state.text();
        assert!(
            status.contains(&format!("Output unavailable: {error}")),
            "{status}"
        );
        assert!(!status.contains("shared greeting unchanged"));
        assert_eq!(std::fs::read_to_string(daily).unwrap(), original);
        window.destroy();
    }

    fn commands(menu: &gio::MenuModel) -> Vec<String> {
        let mut result = Vec::new();
        for index in 0..menu.n_items() {
            if let Some(action) = menu
                .item_attribute_value(index, "action", None)
                .and_then(|value| value.str().map(str::to_owned))
            {
                result.push(action);
            }
            for link in ["section", "submenu"] {
                if let Some(child) = menu.item_link(index, link) {
                    result.extend(commands(&child));
                }
            }
        }
        result
    }

    #[test]
    #[ignore = "requires GTK/VTE; fixed contextual output actions and narrow-window geometry"]
    fn consolidated_output_actions_are_reachable_and_stay_visible() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.OutputBarTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_advanced_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let this = controller(&window);
        for (module, action) in [
            (EditorModule::Palette, "win.install-ptyxis"),
            (EditorModule::Typography, "win.apply-typography"),
            (EditorModule::Layout, "win.apply-layout"),
            (EditorModule::Prompt, "win.save-starship"),
            (EditorModule::Greeting, "win.apply-fastfetch"),
        ] {
            gio::prelude::ActionGroupExt::activate_action(
                &this.window(),
                module.action_name(),
                None,
            );
            settle();
            assert_eq!(
                this.output_bar.primary.action_name().as_deref(),
                Some("win.apply-scheme")
            );
            let saved = commands(&this.save_button.menu_model().unwrap());
            assert!(
                saved.contains(
                    &if module == EditorModule::Prompt {
                        "win.save"
                    } else {
                        "win.save-workspace"
                    }
                    .into()
                )
            );
            let extra = commands(&this.output_bar.more.menu_model().unwrap());
            assert!(extra.contains(&"win.open-workspace".into()));
            assert!(extra.contains(&action.into()));
            assert!(extra.contains(&"win.last-scheme-application".into()));
            if module == EditorModule::Greeting {
                for expected in [
                    "win.export-fastfetch",
                    "win.import-fastfetch",
                    "win.restore-fastfetch",
                    "win.greeting-startup",
                    "win.export-greeting-ans",
                ] {
                    assert!(extra.iter().any(|s| s == expected), "missing {expected}");
                }
            }
            let before = this.output_bar.root.compute_bounds(&window).unwrap();
            for scroll in descendants(window.upcast_ref())
                .into_iter()
                .filter_map(|w| w.downcast::<gtk::ScrolledWindow>().ok())
            {
                if scroll.is_mapped() {
                    scroll.vadjustment().set_value(scroll.vadjustment().upper());
                }
            }
            settle();
            assert_eq!(
                this.output_bar.root.compute_bounds(&window).unwrap(),
                before
            );
        }
        this.prompt_module_button.set_active(true);
        this.prompt_source_selector.set_selected(1);
        settle();
        assert_eq!(
            this.output_bar.primary.label().as_deref(),
            Some("Use Design…")
        );
        assert_eq!(
            this.output_bar.primary.action_name().as_deref(),
            Some("win.apply-scheme")
        );
        window.set_default_size(800, 560);
        settle();
        let bounds = this.output_bar.primary.compute_bounds(&window).unwrap();
        assert!(bounds.x() >= 0.0 && bounds.y() >= 0.0);
        assert!(bounds.x() + bounds.width() <= window.width() as f32);
        assert!(bounds.y() + bounds.height() <= window.height() as f32);
        this.save_button.popup();
        settle();
        this.save_button.popdown();
        // Ctrl+S saves the explicitly owned project, not the active Starship file.
        let starship = root.path().join("starship.toml");
        let source = "[rust]\nsymbol = 'rs '\nstyle = 'bold red'\n";
        std::fs::write(&starship, source).unwrap();
        this.starship_editor
            .begin(starship.clone(), source.into())
            .unwrap();
        this.prompt_source_selector.set_selected(0);
        this.starship_editor.select_module("rust");
        this.starship_editor.symbol.set_text("rust ");
        let workspace = root.path().join("saved.termimochi-design.json");
        *this.typed.store.borrow_mut() = Some(DocumentStore::open(workspace.clone()).unwrap());
        this.save_design(false);
        this.starship_editor.symbol.set_text("crab ");
        this.save_action.activate(None);
        settle();
        assert_eq!(std::fs::read_to_string(&starship).unwrap(), source);
        assert!(
            this.starship_editor.dirty(),
            "workspace save must not mark external Starship applied"
        );
        let stored = DocumentStore::<crate::design_document::DesignDocument>::open(workspace)
            .unwrap()
            .document()
            .unwrap()
            .unwrap();
        assert_eq!(stored, this.design_snapshot().unwrap());
        assert_eq!(
            this.open_button.tooltip_text().as_deref(),
            Some("Open a complete workspace  Ctrl+O")
        );
        window.destroy();
    }
}
