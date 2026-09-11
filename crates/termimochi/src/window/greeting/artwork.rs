use super::*;
use crate::greeting_art::{self, Artwork};
use unicode_width::UnicodeWidthStr;

/// Runs on a worker: built-in logos are resolved by the same sandboxed native
/// renderer as preview. Only the logo, never system information, is exported.
fn exported_art(settings: &GreetingSettings) -> Result<Artwork, String> {
    settings.validate()?;
    let art = if let Some(source) = &settings.imported_source {
        let value = crate::fastfetch_document::value(source)?;
        let logo = &value["logo"];
        let (kind, name) = greeting_art::source_kind(logo);
        if matches!(kind, "builtin" | "small" | "auto") && greeting_art::builtin_name(name)
            || kind == "data"
            || greeting_art::is_file(logo) && kind != "file-raw"
        {
            let mut projected =
                greeting_art::preview_logo(logo, settings.source_logo.as_ref(), &mut Vec::new());
            if projected["type"] == "none" {
                return Err("There is no artwork to export.".into());
            }
            projected["position"] = serde_json::json!("left");
            projected["padding"] = serde_json::json!({"left":0,"right":0,"top":0});
            projected["printRemaining"] = serde_json::json!(true);
            let output = crate::greeting_official::render_config(serde_json::json!({
                "logo":projected,"modules":[{"type":"custom","format":""}]
            }))?;
            Artwork::parse(&output)?
        } else {
            greeting_art::text_logo(logo, settings.source_logo.as_ref(), &mut Vec::new())?
        }
    } else {
        Artwork::parse(&settings.artwork_ansi())?
    };
    if art.plain.trim().is_empty() {
        return Err("There is no artwork to export. Choose a logo first.".into());
    }
    Ok(art)
}

impl Workbench {
    pub(super) fn art_draft_matches(&self, before: &GreetingSettings) -> bool {
        if let Err(error) = self.greeting.require_presentation_ready() {
            self.toast(&error);
            return false;
        }
        if self.greeting.invalid.get() || &self.greeting.settings() != before {
            self.toast("Greeting changed while the dialog was open. Import again to keep your latest edits.");
            false
        } else {
            true
        }
    }
    pub(in crate::window) fn choose_greeting_art_import(self: &Rc<Self>) {
        self.choose_artwork_file(false);
    }
    pub(super) fn choose_original_image(self: &Rc<Self>) {
        self.choose_artwork_file(true);
    }
    fn choose_artwork_file(self: &Rc<Self>, image_only: bool) {
        if let Some(dialog) = self.greeting.image_import_window.upgrade() {
            dialog.present();
            return;
        }
        if self.greeting.invalid.get() {
            self.toast("Fix the invalid Greeting field before importing artwork.");
            return;
        }
        let before = self.greeting.settings();
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(if image_only {
            "Original Image"
        } else {
            "Images / Text / ANSI Artwork"
        }));
        let suffixes: &[&str] = if image_only {
            &["png", "jpg", "jpeg", "webp", "svg", "gif"]
        } else {
            &["png", "jpg", "jpeg", "webp", "svg", "gif", "txt", "ans"]
        };
        for suffix in suffixes {
            filter.add_suffix(suffix);
            filter.add_suffix(&suffix.to_ascii_uppercase());
        }
        for mime in [
            "image/png",
            "image/jpeg",
            "image/webp",
            "image/svg+xml",
            "image/gif",
        ] {
            filter.add_mime_type(mime);
        }
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title(if image_only {
                "Reimport Original Image"
            } else {
                "Import Artwork"
            })
            .filters(&filters)
            .default_filter(&filter)
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
                match result {
                    Ok(file) => {
                        if !this.art_draft_matches(&before) {
                            return;
                        }
                        let Some(path) = file.path() else {
                            this.toast("Select a local artwork file.");
                            return;
                        };
                        if crate::greeting_image::is_image(&path) {
                            this.open_image_artwork(before, path);
                            return;
                        }
                        if image_only {
                            this.toast("Choose the original PNG, JPG, WebP, SVG or GIF image, not a text/ANSI export.");
                            return;
                        }
                        match greeting_art::file_art(&path) {
                            Ok(art) => this.confirm_greeting_art(before, art),
                            Err(error) => this.toast(&error),
                        }
                    }
                    Err(error) if !error.matches(gtk::DialogError::Dismissed) => {
                        this.toast(&error.to_string())
                    }
                    _ => {}
                }
            },
        );
    }
    fn confirm_greeting_art(self: &Rc<Self>, before: GreetingSettings, art: Artwork) {
        let mut next = before.clone();
        if let Err(error) = next.import_artwork(art.clone()) {
            self.toast(&error);
            return;
        }
        crate::greeting_output::select_character(&mut next);
        let detail = format!(
            "{} lines · {} cells wide\n\nOnly the logo will change. Colors are preserved; unsupported controls are removed. Other fields and comments stay intact. Imported configurations embed a portable text copy. Nothing is applied to your terminal. Undo restores the previous logo.{}",
            art.plain.lines().count(),
            art.plain
                .lines()
                .map(UnicodeWidthStr::width)
                .max()
                .unwrap_or(0),
            if art.filtered {
                "\n\nThis file contains filtered controls. See Compatibility after import."
            } else {
                ""
            }
        );
        let dialog = gtk::AlertDialog::builder()
            .message("Replace greeting artwork?")
            .detail(detail)
            .buttons(["Cancel", "Replace Artwork"])
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
                    && this.art_draft_matches(&before)
                {
                    this.greeting.replace(next, true);
                    this.greeting_module_button.set_active(true);
                    this.toast(if art.filtered {
                        "Artwork imported. Unsupported controls were filtered; see Compatibility."
                    } else {
                        "Artwork imported. Save Preset to keep it for next launch."
                    });
                }
            },
        );
    }
    pub(in crate::window) fn edit_greeting_art_text(self: &Rc<Self>) {
        if self.greeting.invalid.get() {
            self.toast("Fix the invalid field first.");
            return;
        }
        let mut settings = self.greeting.settings();
        if settings.imported_source.is_some()
            || settings.logo != Logo::Custom
            || settings.custom_art.is_none()
        {
            self.toast("Select custom artwork in the designer to edit its plain text. For imported Fastfetch logos, export TXT and import that copy.");
            return;
        }
        settings.custom_art = None;
        settings.editable_artwork = None;
        crate::greeting_output::select_character(&mut settings);
        self.greeting.replace(settings, true);
        self.greeting.artwork.grab_focus();
        self.toast("Plain-text editing enabled. Undo restores the original colors.");
    }
    pub(in crate::window) fn choose_greeting_art_export(self: &Rc<Self>, ansi: bool) {
        if let Err(error) = self.greeting.require_presentation_ready() {
            self.toast(&error);
            return;
        }
        if self.greeting.invalid.get() {
            self.toast("Fix the invalid field before exporting.");
            return;
        }
        let snapshot = self.greeting.settings();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(exported_art(&snapshot));
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(40), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            match receiver.try_recv() {
                Ok(Ok(art)) => {
                    let contents = if ansi { art.ansi } else { art.plain };
                    this.save_greeting_art_dialog(contents, ansi);
                    glib::ControlFlow::Break
                }
                Ok(Err(error)) => {
                    this.toast(&error);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => {
                    this.toast("Artwork export worker stopped.");
                    glib::ControlFlow::Break
                }
            }
        });
    }
    fn save_greeting_art_dialog(self: &Rc<Self>, contents: String, ansi: bool) {
        let dialog = Self::greeting_dialog(
            if ansi {
                "Export ANSI Artwork"
            } else {
                "Export Plain Text Artwork"
            },
            if ansi { "*.ans" } else { "*.txt" },
        );
        dialog.set_initial_name(Some(if ansi {
            "termimochi-logo.ans"
        } else {
            "termimochi-logo.txt"
        }));
        let weak = Rc::downgrade(self);
        dialog.save(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(file) => match file.path() {
                        Some(path) => this.confirm_greeting_art_export(path, contents, ansi),
                        None => this.toast("Select a local file."),
                    },
                    Err(error) if !error.matches(gtk::DialogError::Dismissed) => {
                        this.toast(&error.to_string())
                    }
                    _ => {}
                }
            },
        );
    }
    fn confirm_greeting_art_export(self: &Rc<Self>, path: PathBuf, contents: String, ansi: bool) {
        let expected = match typography_preset::read_private_with_limit(
            &path,
            if ansi {
                crate::greeting::ART_MAX_ANSI_BYTES
            } else {
                crate::greeting::ART_MAX_BYTES
            } as u64,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let exists = expected.is_some();
        let detail = format!(
            "{}\n\nA backup will be kept in termimochi-backups beside this file. Only artwork is exported; no system information or shell startup files are changed.",
            path.display()
        );
        let finish =
            move |this: &Self| match greeting_art::export_file(&path, &contents, ansi, &expected) {
                Ok(()) => {
                    this.toast("Artwork exported. Existing content was backed up if replaced.")
                }
                Err(error) => this.toast(&error),
            };
        if !exists {
            finish(self);
            return;
        }
        let dialog = gtk::AlertDialog::builder()
            .message("Replace artwork file?")
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

#[cfg(test)]
mod tests {
    use super::super::tests::{controller, feed, respond, settle, wait_official};
    use super::*;
    use serde_json::json;

    #[test]
    #[ignore = "requires system Fastfetch and Bubblewrap"]
    fn native_text_logo_projection_colors_small_and_artwork_exports() {
        for colors in [json!({}), json!({"1":"red"}), json!({"2":"blue"})] {
            let logo = json!({"type":"data","source":"plain$1A$2B$3C\n\x1b[31mred$2blue\nblue\tend","color":colors,"printRemaining":true,"padding":{"left":0,"right":0,"top":0}});
            let render = |logo| {
                Artwork::parse(
                    &crate::greeting_official::render_config(
                        json!({"logo":logo,"modules":[{"type":"custom","format":""}]}),
                    )
                    .unwrap(),
                )
                .unwrap()
            };
            let native = render(logo.clone());
            let safe = render(greeting_art::preview_logo(&logo, None, &mut Vec::new()));
            assert_eq!(safe.plain, native.plain);
            assert_eq!(
                safe.ansi, native.ansi,
                "Host-default slots and mixed multiline colors must agree"
            );
        }
        for kind in ["data", "data-raw", "builtin", "small"] {
            let source = if matches!(kind, "data" | "data-raw") {
                "$1A$2B$$\n$1C"
            } else {
                "ubuntu"
            };
            let logo = json!({"type":kind,"source":source,"color":{"1":"#12abef","2":"@123"},"printRemaining":true,"padding":{"left":0,"right":0,"top":0}});
            let reference = crate::greeting_official::render_config(
                json!({"logo":logo,"modules":[{"type":"custom","format":""}]}),
            )
            .unwrap();
            let reference = Artwork::parse(&reference).unwrap();
            let safe = greeting_art::preview_logo(&logo, None, &mut Vec::new());
            let projected = crate::greeting_official::render_config(
                json!({"logo":safe,"modules":[{"type":"custom","format":""}]}),
            )
            .unwrap();
            let projected = Artwork::parse(&projected).unwrap();
            assert_eq!(projected.plain, reference.plain, "{kind}");
            assert!(!projected.filtered, "{kind}");
            if kind == "data" {
                assert!(projected.ansi.contains("38;2;18;171;239"));
                assert!(projected.ansi.contains("38;5;123"));
            }
            let settings = GreetingSettings {
                enabled: true,
                imported_source: Some(json!({"logo":logo,"modules":["cpu"]}).to_string()),
                ..Default::default()
            };
            let exported = exported_art(&settings).unwrap();
            assert_eq!(
                exported.plain.trim_end(),
                reference.plain.trim_end(),
                "{kind}"
            );
            assert!(!exported.plain.contains("CPU"));
        }
    }

    #[test]
    #[ignore = "requires GTK/VTE, system Fastfetch and Bubblewrap; run separately at 1x and 2x"]
    fn artwork_import_cancel_colors_undo_export_conflicts_and_restart() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.ArtworkTest")
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
        this.greeting_module_button.set_active(true);
        this.preview_scene_selector.set_selected(3);
        settle();
        let before = this.greeting.settings();
        let layout = this.layout_settings();
        let logo = root.path().join("logo.ans");
        std::fs::write(&logo,"\x1b[38;2;24;140;190m   .- Mochi -.\n\x1b[38;5;203m  (  >_     )\n   `-------'\x1b]52;c;SECRET\x07").unwrap();
        let art = greeting_art::file_art(&logo).unwrap();
        this.confirm_greeting_art(before.clone(), art.clone());
        respond("Cancel");
        assert_eq!(this.greeting.settings(), before);
        this.confirm_greeting_art(before.clone(), art.clone());
        respond("Replace Artwork");
        wait_official(&this);
        assert_eq!(
            this.greeting.settings().official_items,
            before.official_items
        );
        assert_eq!(this.layout_settings(), layout);
        assert!(!this.greeting.artwork.is_editable());
        assert!(feed(&this).contains("38;2;24;140;190"));
        assert!(!feed(&this).contains("SECRET"));
        this.greeting_compatibility_issues();
        assert!(
            this.greeting
                .last_report
                .borrow()
                .iter()
                .any(|note| note.contains("filtered"))
        );
        let colored = this.greeting.settings();
        this.greeting.accent.set_selected(3);
        settle();
        assert!(this.greeting.settings().custom_art.is_some());
        this.greeting.undo();
        settle();
        assert_eq!(this.greeting.settings(), colored);
        this.edit_greeting_art_text();
        settle();
        assert!(this.greeting.artwork.is_editable());
        assert!(this.greeting.settings().custom_art.is_none());
        this.greeting.undo();
        settle();
        assert_eq!(this.greeting.settings(), colored);
        assert!(!this.greeting.artwork.is_editable());
        this.greeting.redo();
        settle();
        assert!(this.greeting.artwork.is_editable());
        this.greeting.undo();
        settle();
        let txt = root.path().join("export.txt");
        this.confirm_greeting_art_export(txt.clone(), art.plain.clone(), false);
        assert_eq!(std::fs::read_to_string(&txt).unwrap(), art.plain);
        this.confirm_greeting_art_export(txt.clone(), "cancel".into(), false);
        respond("Cancel");
        assert_eq!(std::fs::read_to_string(&txt).unwrap(), art.plain);
        this.confirm_greeting_art_export(txt.clone(), "stale".into(), false);
        std::fs::write(&txt, "external edit").unwrap();
        respond("Back Up & Replace");
        assert_eq!(std::fs::read_to_string(&txt).unwrap(), "external edit");
        this.confirm_greeting_art_export(txt.clone(), art.plain.clone(), false);
        respond("Back Up & Replace");
        assert_eq!(std::fs::read_to_string(&txt).unwrap(), art.plain);
        let ans = root.path().join("export.ans");
        this.confirm_greeting_art_export(ans.clone(), art.ansi.clone(), true);
        assert_eq!(greeting_art::file_art(&ans).unwrap().ansi, art.ansi);
        // A late edit in a modal must not be overwritten by its captured draft.
        this.confirm_greeting_art(colored.clone(), Artwork::parse("LATE").unwrap());
        this.greeting.accent.set_selected(4);
        respond("Replace Artwork");
        assert_eq!(this.greeting.settings().custom_art, colored.custom_art);
        this.greeting.undo();
        settle();
        this.save_greeting_preset();
        let saved = this.greeting.settings();
        if let Some(file) = std::env::var_os("TERMIMOCHI_ARTWORK_SCREENSHOT") {
            window.set_title(Some("TermiMochi point-to-edit test"));
            let mut child = std::process::Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../scripts/preview-pointer-driver.py"
                ))
                .args(["capture", "0", "0"])
                .env("TERMIMOCHI_INSPECT_SCREENSHOT", file)
                .spawn()
                .unwrap();
            while child.try_wait().unwrap().is_none() {
                settle();
            }
            assert!(child.wait().unwrap().success());
        }
        window.close();
        drop(this);
        settle();
        present_with_preset(&app, None, path);
        let window = app.active_window().unwrap();
        let this = controller(&window);
        settle();
        assert_eq!(this.greeting.settings(), saved);
        assert!(!this.greeting.artwork.is_editable());
        // Imported paths become snapshots; embedding replaces only the logo.
        let config = root.path().join("config.jsonc");
        std::fs::write(root.path().join("source.txt"), "$1LOCAL").unwrap();
        std::fs::write(&config,"// keep\n{\"logo\":{\"type\":\"file\",\"source\":\"source.txt\",\"color\":{\"1\":\"red\"}},\"modules\":[\"cpu\"]}").unwrap();
        this.load_fastfetch_path(config);
        settle();
        wait_official(&this);
        assert!(this.greeting.settings().source_logo.is_some());
        assert!(feed(&this).contains("LOCAL"));
        let before = this.greeting.settings();
        this.confirm_greeting_art(before, art);
        respond("Replace Artwork");
        wait_official(&this);
        let source = this.greeting.settings().fastfetch_config().unwrap();
        assert!(source.contains("// keep"));
        assert!(!source.contains("source.txt"));
        assert!(feed(&this).contains("Mochi"));
        window.close();
        settle();
    }
}
