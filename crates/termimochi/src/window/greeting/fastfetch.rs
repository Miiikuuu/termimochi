use super::*;
use crate::{fastfetch_apply, fastfetch_document};

fn plain_report_line(line: &str) -> String {
    let mut chars = line.chars();
    let mut plain = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.next() == Some('[') {
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        } else if !ch.is_control() {
            plain.push(ch);
        }
    }
    plain.chars().take(160).collect()
}

impl Workbench {
    pub(in crate::window) fn review_pixel_install(
        self: &Rc<Self>,
        plan: crate::pixel_trial::InstallPlan,
        snapshot: GreetingSettings,
        terminal: &str,
        ansi: bool,
    ) {
        let title = format!(
            "Install {} Greeting · Tested in {terminal}",
            if ansi { "ANSI" } else { "Image" }
        );
        let before = plan
            .target
            .expected
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).into_owned());
        let startup_note = if plan.config.contains(crate::pixel_trial::startup::ARG) {
            "Kitty startup protection: Fastfetch preRun calls TermiMochi to wait for stable terminal dimensions (normally about 300 ms, bounded to 1.5 s). Existing preRun commands are retained. Keep TermiMochi installed at this path.\n"
        } else {
            ""
        };
        let description = format!(
            "Configuration: {}\nManaged immutable assets: {}\nANSI fallback: {}\n{startup_note}Test used safe sample fields. The full configuration below may contain imported commands; they are not run by installation. Only the tested terminal/output is visually verified. Other terminals remain unverified. No shell startup hook is added. Use Restore Previous Configuration to roll back; managed assets remain for backup references.",
            plan.target.path.display(),
            plan.directory.display(),
            plan.directory.join("config-ansi.jsonc").display()
        );
        let weak = Rc::downgrade(self);
        self.review_fastfetch(&format!("{title}\n{description}"), &plan.target.path.clone(), before.as_deref(), Some(&plan.config.clone()), "Install & Apply", move |_| {
            let Some(this) = weak.upgrade() else { return; };
            if this.greeting.settings() != snapshot || this.greeting.invalid.get() { this.toast("Greeting changed during review. Test and review it again."); return; }
            match plan.apply(&this.greeting.fastfetch_state) {
                Ok(target) => {
                    *this.greeting.fastfetch_target.borrow_mut() = Some(target);
                    let saved = this.greeting.persist();
                    this.schedule_fastfetch_sync();
                    this.toast(&match saved { Ok(()) => "Greeting installed with absolute asset paths and a configuration backup. Startup is unchanged; other terminals may require ANSI fallback.".into(), Err(error) => format!("Image configuration applied, but the editable preset could not be saved: {error}") });
                }
                Err(error) => this.toast(&error),
            }
        });
    }

    pub(in crate::window) fn prepare_scheme_fastfetch(
        &self,
    ) -> Result<(fastfetch_apply::Target, String), String> {
        let mut settings = self.greeting.settings();
        if settings.imported_source.is_none() {
            settings.position =
                settings.position_at_width(self.preview_terminal.column_count().max(12) as usize);
        }
        let source = settings.fastfetch_config()?;
        let target = self
            .greeting
            .fastfetch_target
            .borrow()
            .clone()
            .map(Ok)
            .unwrap_or_else(|| {
                fastfetch_apply::last_path(&self.greeting.fastfetch_state).and_then(|path| {
                    fastfetch_apply::Target::open(
                        path.unwrap_or_else(fastfetch_apply::default_path),
                    )
                })
            })?;
        target.check()?;
        Ok((target, source))
    }

    pub(in crate::window) fn accept_scheme_fastfetch(
        self: &Rc<Self>,
        target: fastfetch_apply::Target,
    ) {
        *self.greeting.fastfetch_target.borrow_mut() = Some(target);
        self.schedule_fastfetch_sync();
    }

    pub(in crate::window) fn load_fastfetch_path(self: &Rc<Self>, path: PathBuf) {
        self.load_fastfetch(path, false);
    }

    pub(super) fn load_applied_fastfetch_path(self: &Rc<Self>, path: PathBuf) {
        self.load_fastfetch(path, true);
    }

    fn load_fastfetch(self: &Rc<Self>, path: PathBuf, save: bool) {
        let result = fastfetch_apply::Target::open(path).and_then(|target| {
            let source = target.source()?;
            fastfetch_document::parse(&source)?;
            Ok((target, source))
        });
        let snapshot = self.greeting.settings();
        let invalid = self.greeting.invalid.get();
        let revision = self.greeting.revision.get();
        match result {
            Ok((target, source)) => self.confirm_fastfetch_load(save, move |this| {
                if this.greeting.settings() != snapshot || this.greeting.invalid.get() != invalid || this.greeting.revision.get() != revision {
                    this.toast("Greeting changed while confirming. Load the configuration again.");
                    return;
                }
                if let Err(error) = target.check() {
                    this.toast(&error);
                    this.schedule_fastfetch_sync();
                    return;
                }
                let mut settings = this.greeting.settings();
                let same_logo = settings.fastfetch_config().ok().and_then(|s| fastfetch_document::value(&s).ok()).map(|v| v["logo"].clone())
                    == fastfetch_document::value(&source).ok().map(|v| v["logo"].clone());
                if !same_logo { settings.editable_artwork = None; }
                settings.enabled = true;
                settings.official_preset = None;
                settings.official_items.clear();
                settings.field_styles.retain(|key, _| !key.starts_with("imported:"));
                settings.source_logo=match crate::greeting_art::snapshot_file(&source, &target.path) {
                    Ok(snapshot)=>snapshot,
                    Err(error)=>{this.toast(&error);None},
                };
                settings.imported_source = Some(source);
                *this.greeting.fastfetch_target.borrow_mut() = Some(target);
                this.greeting.replace(settings, true);
                this.greeting_module_button.set_active(true);
                if save {
                    match this.greeting.persist() {
                        Ok(()) => this.toast("Applied configuration loaded and saved in TermiMochi. Fastfetch and shell startup are unchanged."),
                        Err(error) => {
                            this.greeting.sync.save_failed("Loaded; preset not saved.", &error);
                            this.toast(&format!("Loaded for preview, but preset could not be saved: {error}"));
                        }
                    }
                    this.refresh_history_actions();
                } else {
                    this.toast("Fastfetch loaded for editing; nothing was applied. Comments and unsupported settings are retained. Imported commands are never run in preview.");
                }
                this.schedule_fastfetch_sync();
            }),
            Err(error) => self.toast(&error),
        }
    }

    fn confirm_fastfetch_load(
        self: &Rc<Self>,
        save: bool,
        action: impl FnOnce(Rc<Self>) + 'static,
    ) {
        // A saved workspace is not permission to replace the current draft and
        // persist a different Greeting. Always confirm dirty / invalid edits.
        if !self.greeting.dirty() {
            action(self.clone());
            return;
        }
        let dialog = gtk::AlertDialog::builder()
            .message("Replace unsaved greeting changes?")
            .detail(if save {
                "Load Applied replaces this draft and saves the loaded version in TermiMochi. Save your current preset or workspace first to keep it. Fastfetch itself is not changed."
            } else {
                "Save Preset or Save Workspace first to keep this greeting. Loading does not apply the configuration."
            })
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
    pub(in crate::window) fn choose_fastfetch_import(self: &Rc<Self>) {
        let dialog = Self::greeting_dialog("Import Fastfetch Configuration", "*.json*");
        let weak = Rc::downgrade(self);
        dialog.open(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(file) => match file.path() {
                        Some(path) => this.load_fastfetch_path(path),
                        None => this.toast("Choose a local configuration file."),
                    },
                    Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                    Err(error) => this.toast(&error.to_string()),
                }
            },
        );
    }
    pub(in crate::window) fn request_fastfetch_apply(self: &Rc<Self>) {
        self.greeting.finish();
        if self.greeting.invalid.get() {
            self.toast("Fix or undo the invalid field before applying.");
            return;
        }
        let snapshot = self.greeting.settings();
        let mut export = snapshot.clone();
        if export.imported_source.is_none() {
            export.position =
                export.position_at_width(self.preview_terminal.column_count().max(12) as usize);
        }
        let source = match export.fastfetch_config() {
            Ok(source) => source,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let target = self
            .greeting
            .fastfetch_target
            .borrow()
            .clone()
            .map(Ok)
            .unwrap_or_else(|| {
                fastfetch_apply::last_path(&self.greeting.fastfetch_state).and_then(|path| {
                    fastfetch_apply::Target::open(
                        path.unwrap_or_else(fastfetch_apply::default_path),
                    )
                })
            });
        let target = match target.and_then(|target| target.check().map(|()| target)) {
            Ok(target) => target,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let before = target
            .expected
            .as_ref()
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned());
        let weak = Rc::downgrade(self);
        self.review_fastfetch(
            "Review Fastfetch Changes",
            &target.path.clone(),
            before.as_deref(),
            Some(&source.clone()),
            "Back Up & Apply",
            move |run_after| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                if this.greeting.invalid.get() || this.greeting.settings() != snapshot {
                    this.toast(
                        "Greeting changed while reviewing. Review it again before applying.",
                    );
                    return;
                }
                let mut target = target;
                match fastfetch_apply::apply(&mut target, &source, &this.greeting.fastfetch_state) {
                    Ok(_) => {
                        *this.greeting.fastfetch_target.borrow_mut() = Some(target.clone());
                        let saved = this.greeting.persist();
                        if let Err(error) = &saved {
                            this.greeting
                                .sync
                                .save_failed("Fastfetch applied; preset not saved.", error);
                        }
                        let mut message = match saved {
                            Ok(()) => "Fastfetch applied and Greeting preset saved in TermiMochi."
                                .to_owned(),
                            Err(error) => format!(
                                "Fastfetch applied, but Greeting preset could not be saved: {error}"
                            ),
                        };
                        if run_after {
                            match crate::fastfetch_run::launch(&target) {
                                Ok(()) => message.push_str(
                                    " Opening a new terminal; press Enter there to close.",
                                ),
                                Err(error) => {
                                    message.push_str(&format!(" Automatic launch failed: {error}"))
                                }
                            }
                        }
                        this.toast(&message);
                        this.refresh_history_actions();
                        this.schedule_fastfetch_sync();
                    }
                    Err(error) => this.toast(&error),
                }
            },
        );
    }
    pub(in crate::window) fn request_fastfetch_restore(self: &Rc<Self>) {
        let plan = match fastfetch_apply::prepare_restore(&self.greeting.fastfetch_state) {
            Ok(plan) => plan,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let current = plan
            .target
            .expected
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).into_owned());
        let previous = plan
            .before
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).into_owned());
        let weak = Rc::downgrade(self);
        self.review_fastfetch("Restore Fastfetch Configuration", &plan.target.path.clone(), current.as_deref(), previous.as_deref(), "Restore Configuration", move |_| {
            let Some(this)=weak.upgrade() else {return;};
            match fastfetch_apply::restore(&plan,&this.greeting.fastfetch_state) {
                Ok(())=>{
                    *this.greeting.fastfetch_target.borrow_mut()=Some(fastfetch_apply::Target{path:plan.target.path,expected:plan.before.clone()});
                    this.toast(if plan.before.is_some(){"Previous Fastfetch configuration restored. A recovery copy was retained; the preview draft and shell startup are unchanged."}else{"Removed only the Fastfetch config created by TermiMochi. A recovery copy was retained; shell startup is unchanged."});
                    this.schedule_fastfetch_sync();
                },
                Err(error)=>this.toast(&error),
            }
        });
    }
    fn review_fastfetch(
        &self,
        title: &str,
        path: &Path,
        before: Option<&str>,
        after: Option<&str>,
        action: &str,
        accept: impl FnOnce(bool) + 'static,
    ) {
        let (title, extra_detail) = title.split_once('\n').unwrap_or((title, ""));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        content.set_margin_top(20);
        content.set_margin_bottom(20);
        content.set_margin_start(20);
        content.set_margin_end(20);
        let heading = gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["title-3"])
            .build();
        content.append(&heading);
        let path_label = gtk::Label::builder()
            .label(path.to_string_lossy().as_ref())
            .xalign(0.0)
            .selectable(true)
            .wrap(true)
            .build();
        content.append(&path_label);
        if !extra_detail.is_empty() {
            content.append(
                &gtk::Label::builder()
                    .label(extra_detail)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .max_width_chars(95)
                    .xalign(0.0)
                    .selectable(true)
                    .build(),
            );
        }
        let note=gtk::Label::builder().label("Existing content is backed up; external changes block replacement. Imported settings may include commands or network modules. Running Fastfetch executes this configuration outside the preview sandbox. Shell startup files remain unchanged.").wrap(true).xalign(0.0).max_width_chars(95).css_classes(["dim-label"]).build();
        content.append(&note);
        if action == "Back Up & Apply" {
            let save_note = gtk::Label::builder()
                .label(
                    "Applying also saves this Greeting preset in TermiMochi for the next launch.",
                )
                .wrap(true)
                .xalign(0.0)
                .build();
            content.append(&save_note);
        }
        let versions = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        versions.set_vexpand(true);
        for (label, text) in [("Before", before), ("After", after)] {
            let pane = gtk::Box::new(gtk::Orientation::Vertical, 8);
            pane.set_hexpand(true);
            pane.append(
                &gtk::Label::builder()
                    .label(label)
                    .xalign(0.0)
                    .css_classes(["heading"])
                    .build(),
            );
            let view = gtk::TextView::builder()
                .editable(false)
                .cursor_visible(false)
                .monospace(true)
                .wrap_mode(gtk::WrapMode::None)
                .top_margin(10)
                .bottom_margin(10)
                .left_margin(10)
                .right_margin(10)
                .build();
            view.buffer().set_text(text.unwrap_or("No configuration file. If restoring, the file created by TermiMochi will be removed."));
            view.update_property(&[gtk::accessible::Property::Label(&format!(
                "Fastfetch {label}"
            ))]);
            pane.append(
                &gtk::ScrolledWindow::builder()
                    .child(&view)
                    .min_content_width(300)
                    .min_content_height(280)
                    .hexpand(true)
                    .vexpand(true)
                    .build(),
            );
            versions.append(&pane);
        }
        content.append(&versions);
        let run_after = if action == "Back Up & Apply" {
            let run =
                gtk::CheckButton::with_label("Run Fastfetch in a new terminal after applying");
            run.set_active(self.greeting.settings().imported_source.is_none());
            run.set_tooltip_text(Some("Uses a new Ptyxis window and runs this configuration once. Imported configurations default to off because they may contain commands or network modules. No shell startup hooks are added."));
            content.append(&run);
            Some(run)
        } else {
            None
        };
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_halign(gtk::Align::End);
        let cancel = gtk::Button::with_label("Cancel");
        let confirm = gtk::Button::with_label(action);
        confirm.add_css_class("suggested-action");
        buttons.append(&cancel);
        buttons.append(&confirm);
        content.append(&buttons);
        let dialog = gtk::Window::builder()
            .title(title)
            .transient_for(&self.window())
            .modal(true)
            .destroy_with_parent(true)
            .default_width(900)
            .default_height(if extra_detail.is_empty() { 560 } else { 760 })
            .child(&content)
            .build();
        let weak = dialog.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.destroy();
            }
        });
        let accept = RefCell::new(Some(accept));
        let weak = dialog.downgrade();
        confirm.connect_clicked(move |_| {
            if let Some(accept) = accept.borrow_mut().take() {
                if let Some(dialog) = weak.upgrade() {
                    dialog.destroy();
                }
                accept(run_after.as_ref().is_some_and(|run| run.is_active()));
            }
        });
        dialog.present();
        cancel.grab_focus();
    }

    pub(in crate::window) fn greeting_compatibility_issues(&self) -> Vec<Issue> {
        let settings = self.greeting.settings();
        let mut entries = Vec::<(Severity, &'static str, String)>::new();
        if settings.imported_source.is_none()
            && settings.logo == Logo::Custom
            && let Some(art) = &settings.custom_art
        {
            for note in art.notices() {
                entries.push((
                    Severity::Warning,
                    "greeting-art",
                    format!("Artwork compatibility\n{note}"),
                ));
            }
        }
        if settings.enabled && settings.needs_native() {
            entries.push((Severity::Note,"greeting-offline","Offline preview\nDesktop/terminal facts and disk read-only flags may differ from a real terminal. Refresh the snapshot or review the exported file in your terminal.".into()));
            match self.greeting_official_result.borrow().as_ref() {
                Some(Err(error)) => entries.push((
                    Severity::Warning,
                    "greeting-runtime",
                    format!("Preview compatibility\n{error}"),
                )),
                Some(Ok(output)) => {
                    let failures: Vec<_> = output
                        .render(240)
                        .lines()
                        .filter(|line| {
                            line.contains("Unknown ")
                                || line.contains("not supported")
                                || line.contains("Unavailable")
                        })
                        .take(4)
                        .map(plain_report_line)
                        .collect();
                    if !failures.is_empty() {
                        entries.push((Severity::Warning,"greeting-detection",format!("Detection unavailable\n{}\nThese are offline detection failures, not missing configuration fields.",failures.join("\n"))));
                    }
                }
                None => {}
            }
        }
        let configuration = settings.fastfetch_config();
        if settings.imported_source.is_some()
            && let Ok(source) = &configuration
            && let Ok((_, notes)) =
                fastfetch_document::preview_config_with_logo(source, settings.source_logo.as_ref())
        {
            let total = notes.len();
            for note in notes.into_iter().take(12) {
                entries.push((
                    Severity::Warning,
                    "greeting-preserved",
                    format!(
                        "Preserved · not simulated\n{}",
                        note.chars().take(240).collect::<String>()
                    ),
                ));
            }
            if total > 12 {
                entries.push((Severity::Note,"greeting-preserved",format!("More retained settings\n{} additional differences. Review Before / After before applying.",total-12)));
            }
        }
        // Inspect only non-ASCII glyphs actually present in active field text.
        // Normal CJK/emoji fallback is fine; missing/private-use icons need help.
        if settings.enabled
            && let Ok(source) = &configuration
            && let Ok(value) = fastfetch_document::value(source)
        {
            let mut glyphs = BTreeSet::new();
            if let Some(modules) = value["modules"].as_array() {
                for module in modules {
                    for key in ["key", "keyIcon"] {
                        if let Some(text) = module[key].as_str() {
                            glyphs.extend(text.chars().filter(|ch| {
                                !ch.is_ascii() && !ch.is_whitespace() && !ch.is_control()
                            }));
                        }
                    }
                }
            }
            let context = self.preview_terminal.pango_context();
            let font = self.typography_settings().font_description();
            if let Some(face) = context.load_font(&font) {
                let layout = gtk::pango::Layout::new(&context);
                layout.set_font_description(Some(&font));
                let mut missing = Vec::new();
                let mut fallback = Vec::new();
                for ch in glyphs.into_iter().take(64) {
                    layout.set_text(&ch.to_string());
                    if layout.unknown_glyphs_count() > 0 {
                        missing.push(format!("U+{:04X}", u32::from(ch)));
                    } else if matches!(ch,'\u{e000}'..='\u{f8ff}'|'\u{f0000}'..='\u{ffffd}'|'\u{100000}'..='\u{10fffd}')
                        && !face.has_char(ch)
                    {
                        fallback.push(format!("U+{:04X}", u32::from(ch)));
                    }
                }
                if !missing.is_empty() {
                    entries.push((Severity::Warning,"greeting-font",format!("Missing icon glyphs\n{}. Choose a matching font in Typography or replace the field icon with ASCII.",missing.join(", "))));
                }
                if !fallback.is_empty() {
                    entries.push((Severity::Warning,"greeting-font",format!("Icon font fallback\n{}. The selected font does not contain these private-use icons. Choose a compatible Nerd Font or use ASCII.",fallback.join(", "))));
                }
            }
        }
        let report: Vec<_> = entries
            .iter()
            .map(|(_, _, message)| message.clone())
            .collect();
        if report != *self.greeting.last_report.borrow() {
            while let Some(child) = self.greeting.report_body.first_child() {
                self.greeting.report_body.remove(&child);
            }
            for text in &report {
                let label = gtk::Label::builder()
                    .label(text)
                    .xalign(0.0)
                    .wrap(true)
                    .max_width_chars(40)
                    .selectable(true)
                    .css_classes(["dim-label"])
                    .build();
                self.greeting.report_body.append(&label);
            }
            if entries.iter().any(|(_, code, _)| *code == "greeting-font") {
                self.greeting.report_body.append(
                    &gtk::Button::builder()
                        .label("Open Typography")
                        .action_name("win.show-typography")
                        .css_classes(["flat"])
                        .build(),
                );
            }
            let count = entries
                .iter()
                .filter(|(severity, _, _)| *severity == Severity::Warning)
                .count();
            self.greeting.compatibility.set_label(Some(&if count == 0 {
                "Compatibility · no issues".into()
            } else {
                format!("Compatibility · {count} notices")
            }));
            *self.greeting.last_report.borrow_mut() = report;
        }
        // Keep detailed retained-field notes in the expandable report; the
        // existing preview log gets only actionable summary categories.
        entries
            .into_iter()
            .filter(|(_, code, _)| {
                matches!(
                    *code,
                    "greeting-runtime" | "greeting-detection" | "greeting-font"
                )
            })
            .map(|(severity, code, message)| Issue {
                severity,
                code,
                message: format!("Greeting · {message}"),
                variant: None,
                ratio: None,
            })
            .collect()
    }
}
