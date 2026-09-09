//! One persistent output area, separate from scrolling properties and preview.
use super::*;

pub(super) struct OutputBar {
    pub root: gtk::Box,
    title: gtk::Label,
    primary: gtk::Button,
    more: gtk::MenuButton,
    module: Cell<Option<EditorModule>>,
    designer: Cell<bool>,
}

impl OutputBar {
    #[cfg(test)]
    pub fn more_button(&self) -> &gtk::MenuButton {
        &self.more
    }
    pub fn new(save: &gtk::MenuButton) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        root.add_css_class("editor-output-bar");
        let title = gtk::Label::builder()
            .label("Palette")
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["output-context"])
            .build();
        let primary = gtk::Button::builder()
            .label("Install…")
            .css_classes(["output-primary"])
            .build();
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("Import, export and recovery")
            .css_classes(["tool-menu"])
            .build();
        root.append(&title);
        root.append(save);
        root.append(&primary);
        root.append(&more);
        Self {
            root,
            title,
            primary,
            more,
            module: Cell::new(None),
            designer: Cell::new(false),
        }
    }
}

impl Workbench {
    fn output_module(&self) -> EditorModule {
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
        let module = self.output_module();
        let designer =
            self.prompt_source_selector.selected() == 1 || self.starship_editor.detached.get();
        let bar = &self.output_bar;
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
            let item = gio::MenuItem::new(Some(save_label), Some("win.save"));
            set_menu_verb_icon(&item, "termimochi-save-symbolic");
            save.append_item(&item);
        }
        save.append(Some("Save Workspace"), Some("win.save-workspace"));
        save.append(Some("Save Workspace As…"), Some("win.save-workspace-as"));
        more.append(Some("Open Workspace…"), Some("win.open-workspace"));
        match module {
            EditorModule::Palette => {
                more.append(Some("Save Theme As…"), Some("win.save-as"));
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
        bar.title.set_text(module.label());
        bar.primary.set_label(apply_label);
        bar.primary.set_action_name(Some(apply_action));
        bar.primary.set_tooltip_text(Some(detail));
        bar.primary
            .update_property(&[gtk::accessible::Property::Label(&format!(
                "{} {apply_label}",
                module.label()
            ))]);
        self.save_button.set_menu_model(Some(&save));
        self.save_button
            .set_tooltip_text(Some("Save the current preset or a complete workspace"));
        bar.more.set_menu_model(Some(&more));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::greeting::tests::{controller, descendants, settle};

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
        present_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
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
                Some(action)
            );
            let saved = commands(&this.save_button.menu_model().unwrap());
            assert!(saved.contains(&"win.save-workspace".into()));
            let extra = commands(&this.output_bar.more.menu_model().unwrap());
            assert!(extra.contains(&"win.open-workspace".into()));
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
        assert_eq!(this.output_bar.primary.label().as_deref(), Some("Export…"));
        assert_eq!(
            this.output_bar.primary.action_name().as_deref(),
            Some("win.export-starship")
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
        window.destroy();
    }
}
