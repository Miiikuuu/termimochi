//! Review is a modal, read-only snapshot; Save continues to mean Save.
use super::*;
use crate::scheme_apply::{self, Action, Item, Plan, Report, Status};

pub(super) fn label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .max_width_chars(90)
        .xalign(0.0)
        .selectable(true)
        .build()
}

pub(super) fn versions(before: &str, after: &str) -> gtk::Expander {
    let panes = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    for (title, text) in [("Before", before), ("After", after)] {
        let pane = gtk::Box::new(gtk::Orientation::Vertical, 6);
        pane.set_hexpand(true);
        pane.append(&label(title));
        let view = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .build();
        view.buffer().set_text(text);
        pane.append(
            &gtk::ScrolledWindow::builder()
                .child(&view)
                .min_content_height(180)
                .hexpand(true)
                .build(),
        );
        panes.append(&pane);
    }
    gtk::Expander::builder()
        .label("Review file changes")
        .child(&panes)
        .build()
}

pub(super) fn dialog(
    parent: &adw::ApplicationWindow,
    title: &str,
    target: &str,
) -> (gtk::Window, gtk::Box, gtk::Box) {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(20);
    root.set_margin_bottom(20);
    root.set_margin_start(20);
    root.set_margin_end(20);
    let heading = label(title);
    heading.set_selectable(false);
    heading.add_css_class("title-2");
    root.append(&heading);
    // Keep the destination visible even while reviewing a long file diff.
    let destination = label(&target.lines().take(2).collect::<Vec<_>>().join("\n"));
    destination.set_selectable(false);
    root.append(&destination);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&body)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    root.append(&scroll);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    root.append(&buttons);
    let window = gtk::Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .destroy_with_parent(true)
        .default_width(780)
        .default_height(680)
        .child(&root)
        .build();
    (window, body, buttons)
}

impl Workbench {
    fn scheme_palette_name(&self) -> String {
        format!("termimochi-{}.palette", self.typed.document_id.borrow())
    }
    pub(super) fn scheme_plan(&self, workspace: &Workspace) -> Plan {
        let design = self.design_snapshot().ok();
        let theme = design.as_ref().and_then(|d| d.theme.as_ref());
        let mut scope = design
            .as_ref()
            .map(|d| d.scope())
            .unwrap_or(self.typed.scope.get());
        if let Some(t) = theme {
            scope.prompt &= t.prompt_enabled;
            scope.greeting &= workspace.greeting.enabled;
        }
        let profile = scheme_apply::activation::profile();
        let target = if !(scope.palette || scope.typography || scope.layout) {
            "Target: explicitly reviewed native configuration\nOnly owned Prompt / Greeting content; terminal appearance and shell startup are unchanged.".into()
        } else {
            match &profile {
                Ok((uuid, name)) => format!(
                    "Appearance target: Ptyxis\nProfile: {name} ({uuid})\nLaunching profile when available, otherwise the configured default. This is not another window's active tab. Typography can affect ALL Ptyxis windows. Greeting has a separate explicit destination below; shared files can affect other terminals."
                ),
                Err(error) => format!(
                    "Target: Ptyxis unavailable\n{error}\nStarship and Fastfetch file operations remain available independently."
                ),
            }
        };
        let mut plan = Plan::new(
            &typography_preset::state_directory(),
            target,
            (scope.palette || scope.typography || scope.layout)
                .then(|| profile.as_ref().ok().map(|p| p.0.clone()))
                .flatten(),
        );
        let palette = (|| {
            scope.require(crate::design_document::Action::Palette)?;
            if theme.is_some()
                && design
                    .as_ref()
                    .is_some_and(|d| d.components.palette.is_none())
            {
                return Err("This theme contains individual colors, not a complete Ptyxis palette. Explicitly include a base palette in Use Theme; preview colors are not silently installed.".into());
            }
            let installer = self
                .ptyxis_installer
                .clone()
                .ok_or("Ptyxis palette directory unavailable.")?;
            let name = self.scheme_palette_name();
            let path = installer.palette_dir().join(&name);
            let before = typography_preset::read_private_with_limit(&path, 256 * 1024)?;
            let errors = lint_palette(&workspace.palette()?, Target::Codex)
                .issues
                .iter()
                .filter(|issue| issue.severity == Severity::Error)
                .count();
            let detail = format!(
                "Install only: {}\nShared palette file; profiles already using this filename will see its new colors. Select the separate activation option to choose it for the reviewed profile.\nReadability errors: {errors}. Applying does not fix diagnostic findings.",
                path.display()
            );
            let versions = Some((
                before
                    .as_ref()
                    .map(|b| String::from_utf8_lossy(b).into_owned())
                    .unwrap_or_else(|| "No file".into()),
                workspace.palette.clone(),
            ));
            Ok((
                detail,
                versions,
                Action::Palette {
                    installer,
                    name,
                    before,
                    contents: workspace.palette.clone(),
                },
            ))
        })();
        add(&mut plan, "palette", "Palette · install", palette);
        let activation = (|| {
            scope.require(crate::design_document::Action::Palette)?;
            let (uuid, _) = profile.as_ref().map_err(Clone::clone)?;
            let request = scheme_apply::activation::Activation::prepare(
                uuid,
                self.scheme_palette_name().trim_end_matches(".palette"),
                workspace.light,
            )?;
            Ok((request.detail(), None, Action::Activate(request)))
        })();
        add(
            &mut plan,
            "activate",
            "Palette · enable & Light/Dark",
            activation,
        );
        let typography = (|| {
            scope.require(crate::design_document::Action::Typography)?;
            let (uuid, _) = profile.as_ref().map_err(Clone::clone)?;
            let settings = &workspace.typography;
            if theme.is_none_or(|t| t.typography.contains_key("family")) {
                let font = self
                    .preview_terminal
                    .pango_context()
                    .load_font(&settings.font_description())
                    .ok_or("The selected font is unavailable.")?;
                if !settings.family.eq_ignore_ascii_case(DEFAULT_FONT_FAMILY)
                    && font.face().is_none_or(|face| {
                        !face.family().name().eq_ignore_ascii_case(&settings.family)
                    })
                {
                    return Err("The selected font is missing. Install it before applying; preview currently uses a fallback.".into());
                }
            }
            let target = TypographyTarget::for_uuid(uuid)?;
            let request = if let Some(t) = theme {
                target.prepare_theme(settings, &t.typography)?
            } else {
                target.prepare(settings)?
            };
            Ok((request.detail(), None, Action::Typography(request)))
        })();
        add(&mut plan, "typography", "Typography", typography);
        add(
            &mut plan,
            "layout",
            "Layout · supported settings",
            scope
                .require(crate::design_document::Action::Layout)
                .and_then(|()| {
                    if let Some(t) = theme {
                        layout_apply::ApplyRequest::discover_theme(&workspace.layout, &t.layout)
                    } else {
                        layout_apply::ApplyRequest::discover(&workspace.layout)
                    }
                })
                .map(|request| (request.detail(), None, Action::Layout(request))),
        );
        plan.items.push(Item { id: "preview-only", title: "Layout · preview only".into(), detail: "Exact content padding, tab bar and window spacing are saved in the workspace, but cannot be applied to Ptyxis. Preview animation is not a terminal setting.".into(), versions: None, action: None });
        let prompt = (|| {
            scope.require(crate::design_document::Action::Prompt)?;
            let designer = self.prompt_source_selector.selected() == 1;
            let contents = if designer {
                workspace.designer.to_starship_toml()
            } else {
                workspace
                    .starship
                    .clone()
                    .ok_or("No editable Starship configuration is loaded.")?
            };
            if let Some(file) = self.document_use.starship.borrow().clone() {
                file.verify()?;
                Ok((
                    format!(
                        "Update reviewed Starship file: {}\nPrivate backup; only existing shells using this file are affected. No startup integration is added.",
                        file.path.display()
                    ),
                    Some((file.contents.clone(), contents.clone())),
                    Action::Starship { file, contents },
                ))
            } else if designer || self.starship_editor.detached.get() {
                let path = plan.directory.join("starship.toml");
                Ok((
                    format!(
                        "Export only: {}\nDesigner and workspace copies are not bound to a host configuration. This file will not be enabled automatically; shell startup is untouched.",
                        path.display()
                    ),
                    Some(("No exported file".into(), contents.clone())),
                    Action::ExportStarship(contents),
                ))
            } else {
                let file = self.starship_editor.file.borrow().clone()?;
                file.verify()?;
                Ok((
                    format!(
                        "Write: {}\nPrivate backup beside the file. Takes effect only in shells already using this Starship configuration; shell startup is unchanged.",
                        file.path.display()
                    ),
                    Some((file.contents.clone(), contents.clone())),
                    Action::Starship { file, contents },
                ))
            }
        })();
        add(&mut plan, "starship", "Prompt · Starship", prompt);
        add(
            &mut plan,
            "fastfetch",
            "Greeting · selected display effect",
            scope
                .require(crate::design_document::Action::Greeting)
                .and_then(|()| self.prepare_greeting_action()),
        );
        plan.items.retain(|item| match item.id {
            "palette" | "activate" => scope.palette,
            "typography" => scope.typography,
            "layout" | "preview-only" => scope.layout,
            "starship" => scope.prompt,
            "fastfetch" => scope.greeting,
            _ => false,
        });
        if self.typed.target.get() == Some(crate::design_document::TargetHint::Kitty) {
            // The legacy Ptyxis executor is never a Kitty adapter.
            plan.items.clear();
        }
        plan
    }

    pub(super) fn request_scheme_apply(self: &Rc<Self>) {
        if self.typed.target.get().is_none()
            && !self.document_use.shared_review.get()
            && self.typed.kind.get() != crate::design_document::Kind::Legacy
        {
            self.choose_document_use_target();
            return;
        }
        if self.typed.target.get() == Some(crate::design_document::TargetHint::Kitty) {
            self.use_kitty_design();
            return;
        }
        if self.typed.kind.get() == crate::design_document::Kind::Legacy {
            self.choose_document_copy();
            return;
        }
        let design = match self.committed_design() {
            Ok(d) => d,
            Err(e) => {
                self.toast(&e);
                return;
            }
        };
        let document_identity = self.typed.identity.get();
        if design.theme.is_some() && design.scope().palette && design.components.palette.is_none() {
            let dialog=gtk::AlertDialog::builder().message("Choose a complete Ptyxis palette base")
                .detail("Ptyxis installs palette files as a whole. This theme currently specifies only individual colors. You can explicitly include ALL currently previewed colors as the theme's base, or go back and import a .palette. No font, layout, Prompt or Greeting reference is included. Nothing is applied by this step.")
                .buttons(["Back — import a palette","Include Preview Palette in Theme"]).cancel_button(0).default_button(0).modal(true).build();
            let weak = Rc::downgrade(self);
            dialog.choose(Some(&self.window()), gio::Cancellable::NONE, move |r| {
                if r == Ok(1)
                    && let Some(this) = weak.upgrade().filter(|w| {
                        w.typed.identity.get() == document_identity
                            && w.design_snapshot().ok().as_ref() == Some(&design)
                    })
                {
                    let mut next = design.clone();
                    let w = this.workspace_snapshot();
                    next.components.palette = Some(crate::design_document::PaletteComponent {
                        source: w.palette,
                        light: w.light,
                    });
                    this.replace_theme(next, true);
                    this.request_scheme_apply();
                }
            });
            return;
        }
        if design.scope().greeting && self.greeting.settings().enabled {
            let settings = self.greeting.settings();
            let protocol = settings.presentation.resolve(
                &settings,
                self.greeting.presentation.binding.borrow().terminal,
            );
            if let Ok(spec) = protocol
                && let Err(error) = design.require_greeting_target(spec.protocol)
            {
                let prompt = gtk::AlertDialog::builder()
                    .message("This Greeting is not supported by this target")
                    .detail(error)
                    .buttons([
                        "Back — change or disable Greeting",
                        "Convert Terminal Copy…",
                    ])
                    .cancel_button(0)
                    .default_button(0)
                    .modal(true)
                    .build();
                let weak = Rc::downgrade(self);
                prompt.choose(
                    Some(&self.window()),
                    gio::Cancellable::NONE,
                    move |answer| {
                        if answer == Ok(1)
                            && let Some(this) = weak
                                .upgrade()
                                .filter(|w| w.typed.identity.get() == document_identity)
                        {
                            this.choose_document_copy();
                        }
                    },
                );
                return;
            }
        }
        let workspace = match self.committed_workspace() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let plan = self.scheme_plan(&workspace);
        let (window, body, buttons) = dialog(&self.window(), "Apply This Scheme", &plan.target);
        body.append(&label(
            &plan.target.lines().skip(2).collect::<Vec<_>>().join("\n"),
        ));
        body.append(&label("Choose the parts to apply. Nothing changes until you confirm. Save never changes external settings. Successful changes have independent backups; a failed item does not undo the others."));
        let checks: Vec<_> = plan
            .items
            .iter()
            .map(|item| {
                let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
                let check = gtk::CheckButton::with_label(&item.title);
                check.set_sensitive(item.action.is_some() && item.id != "activate");
                check.set_widget_name(&format!("scheme-{}", item.id));
                row.append(&check);
                row.append(&label(&item.detail));
                if let Some((before, after)) = &item.versions {
                    row.append(&versions(before, after));
                }
                body.append(&row);
                check
            })
            .collect();
        if let (Some(palette_index), Some(activate_index)) = (
            plan.items.iter().position(|i| i.id == "palette"),
            plan.items.iter().position(|i| i.id == "activate"),
        ) {
            let activate_available = plan.items[activate_index].action.is_some();
            let activate = checks[activate_index].clone();
            let theme = self.is_theme();
            checks[palette_index].connect_toggled(move |palette| {
                activate.set_sensitive(palette.is_active() && activate_available);
                if theme {
                    activate.set_active(palette.is_active() && activate_available);
                }
                if !palette.is_active() {
                    activate.set_active(false);
                }
            });
        }
        let cancel = gtk::Button::with_label("Cancel");
        let confirm = gtk::Button::with_label("Back Up & Apply Selected");
        confirm.set_sensitive(false);
        confirm.add_css_class("suggested-action");
        confirm.set_widget_name("scheme-confirm");
        for check in &checks {
            let all: Vec<_> = checks.iter().map(|c| c.downgrade()).collect();
            let button = confirm.downgrade();
            check.connect_toggled(move |_| {
                if let Some(button) = button.upgrade() {
                    button.set_sensitive(
                        all.iter()
                            .filter_map(|c| c.upgrade())
                            .any(|c| c.is_sensitive() && c.is_active()),
                    );
                }
            });
        }
        if self.typed.kind.get() == crate::design_document::Kind::Palette {
            // The ordinary palette path includes installation and activation;
            // either can still be explicitly unchecked for advanced install-only.
            for check in &checks {
                if check.is_sensitive() {
                    check.set_active(true);
                }
            }
        }
        buttons.append(&cancel);
        buttons.append(&confirm);
        let weak = window.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.destroy();
            }
        });
        let weak_window = window.downgrade();
        let reviewed_target = self.greeting.presentation.binding.borrow().clone();
        let reviewed_key = self.greeting_verification_key();
        let reviewed_starship = self
            .document_use
            .starship
            .borrow()
            .as_ref()
            .map(|f| (f.path.clone(), f.contents.clone()));
        let reviewed_fastfetch = self
            .greeting
            .fastfetch_target
            .borrow()
            .as_ref()
            .map(|f| (f.path.clone(), f.expected.clone()));
        let weak = Rc::downgrade(self);
        let plan = RefCell::new(Some(plan));
        confirm.connect_clicked(move |_| {
            let Some(this) = weak.upgrade() else { return; };
            let Some(plan) = plan.borrow_mut().take() else { return; };
            if let Some(window) = weak_window.upgrade() { window.destroy(); }
            if this.typed.identity.get() != document_identity || this.committed_design().ok().as_ref() != Some(&design)
                || (design.scope().prompt && this.document_use.starship.borrow().as_ref().map(|f| (f.path.clone(), f.contents.clone())) != reviewed_starship)
                || (design.scope().greeting && (this.greeting.fastfetch_target.borrow().as_ref().map(|f| (f.path.clone(), f.expected.clone())) != reviewed_fastfetch || *this.greeting.presentation.binding.borrow() != reviewed_target || this.greeting_verification_key().ok() != reviewed_key.clone().ok())) {
                this.toast("Workspace changed during review. Review the scheme again; nothing was applied.");
                return;
            }
            let selected: Vec<_> = checks.iter().map(|c| c.is_sensitive() && c.is_active()).collect();
            let document = this.starship_editor.document();
            let binding = plan.items.iter().find_map(|item| match item.action.as_ref() { Some(Action::Starship { file, contents }) => Some((file.path.clone(), contents.clone())), _ => None });
            let fastfetch = plan.items.iter().find_map(|item| match item.action.as_ref() {
                Some(Action::Fastfetch { target, source }) => Some(crate::fastfetch_apply::Target { path: target.path.clone(), expected: Some(source.as_bytes().to_vec()) }),
                Some(Action::ImageGreeting { plan, independent: false }) => Some(crate::fastfetch_apply::Target { path: plan.target.path.clone(), expected: Some(plan.config.as_bytes().to_vec()) }),
                _ => None
            });
            match plan.apply(&selected) {
                Ok((directory, report)) => {
                    let succeeded = |id| report.items.iter().any(|row| row.id == id && matches!(row.status, Status::Applied | Status::Unchanged));
                    if succeeded("starship") && let Some((path, contents)) = binding && let Ok(file) = crate::starship_file::FileSnapshot::bind(&path, &contents) {
                        if this.document_use.starship.borrow().is_some() { *this.document_use.starship.borrow_mut() = Some(file.clone()); }
                        this.starship_editor.accept_saved(document, file);
                    }
                    if succeeded("fastfetch") && let Some(target) = fastfetch { this.accept_scheme_fastfetch(target); }
                    if let Some(row)=report.items.iter().find(|r|r.id=="fastfetch" && matches!(r.status,Status::Applied|Status::Unchanged|Status::NotEnabled)) && let Some(path)=&row.path && let Ok(target)=crate::fastfetch_apply::Target::open(path.clone()) {
                        *this.greeting.presentation.deployed.borrow_mut()=Some((this.greeting.settings(),this.greeting.presentation.binding.borrow().clone(),target,row.status==Status::NotEnabled));
                        // Retain the existing local recovery draft after a
                        // successful Greeting apply. This does not mark the
                        // complete scheme saved or borrow permission to run it.
                        if let Err(error) = this.greeting.retain_applied_preset() {
                            this.toast(&format!("Greeting applied, but local preset was not saved: {error}"));
                        }
                    }
                    this.refresh_deployment();
                    this.refresh_output_bar();
                    this.show_scheme_report(directory, report);
                }
                Err(error) => { gtk::AlertDialog::builder().message("Scheme application needs attention").detail(&error).buttons(["Close"]).modal(true).build().choose(Some(&this.window()), gio::Cancellable::NONE, |_| {}); },
            }
        });
        gtk::prelude::GtkWindowExt::set_focus(&window, Some(&cancel));
        window.present();
        cancel.grab_focus();
    }

    pub(super) fn show_last_scheme_application(self: &Rc<Self>) {
        match Report::latest(&typography_preset::state_directory()) {
            Ok((directory, report)) => self.show_scheme_report(directory, report),
            Err(error) => self.toast(&error),
        }
    }

    fn show_scheme_report(self: &Rc<Self>, directory: PathBuf, report: Report) {
        let (window, body, buttons) =
            dialog(&self.window(), "Scheme Application Results", &report.target);
        body.append(&label(
            &report.target.lines().skip(2).collect::<Vec<_>>().join("\n"),
        ));
        for item in &report.items {
            let heading = label(&format!("{} — {}", item.title, item.status.label()));
            heading.set_selectable(false);
            heading.add_css_class("heading");
            body.append(&heading);
            let details = gtk::Expander::builder()
                .label("Details & destination")
                .child(&label(&item.detail))
                .build();
            details.set_expanded(matches!(
                item.status,
                Status::Failed | Status::Pending | Status::RestoreBlocked
            ));
            body.append(&details);
        }
        body.append(&label(&format!("Application record: {}\nOnly the reviewed items were changed. A native update does not add a shell startup hook. Independent Kitty sessions are opened from their own entry library.", directory.display())));
        if report.profile_uuid.is_some() {
            body.append(&label("Ptyxis appearance: use Open Profile Tab for the exact reviewed profile. Font settings are global; grid size affects new windows."));
        }
        for row in &report.items {
            if row.id != "fastfetch"
                || !matches!(
                    row.status,
                    Status::Applied | Status::Unchanged | Status::NotEnabled
                )
            {
                continue;
            }
            let Some(path) = &row.path else {
                continue;
            };
            let Ok(target) = crate::fastfetch_apply::Target::open(path.clone()) else {
                continue;
            };
            let selection =
                gtk::DropDown::from_strings(&["Choose terminal…", "Kitty", "Ptyxis", "Xterm"]);
            selection
                .update_property(&[gtk::accessible::Property::Label("Open applied Greeting in")]);
            body.append(&selection);
            let run = gtk::Button::with_label("Review & Run Applied Greeting…");
            body.append(&run);
            let weak = Rc::downgrade(self);
            run.connect_clicked(move |_| {
                let Some(this) = weak.upgrade() else { return; };
                let terminal = match selection.selected() {
                    1 => crate::pixel_trial::Terminal::Kitty,
                    2 => crate::pixel_trial::Terminal::Ptyxis,
                    3 => crate::pixel_trial::Terminal::Xterm,
                    _ => { this.toast("Choose the real target terminal first. A configuration path does not select a terminal."); return; }
                };
                let source = match target.check().and_then(|()| target.source()) { Ok(s) => s, Err(e) => { this.toast(&e); return; } };
                let (review, body, buttons) = dialog(&this.window(), "Run Applied Native Configuration", &format!("Target: {}\n{}", terminal.label(), target.path.display()));
                body.append(&label("This executes the exact full configuration below, including its command/network modules and preRun. It is NOT the safe trial projection. Original shell startup is not loaded; it runs once in a new terminal."));
                body.append(&versions("Not executed yet", &source));
                let cancel = gtk::Button::with_label("Cancel");
                let dismissed = review.downgrade();
                cancel.connect_clicked(move |_| { if let Some(window) = dismissed.upgrade() { window.destroy(); } });
                buttons.append(&cancel);
                let confirm = gtk::Button::with_label("Run This Configuration Once");
                buttons.append(&confirm);
                let target = target.clone(); let weak = Rc::downgrade(&this); let win = review.downgrade();
                confirm.connect_clicked(move |_| {
                    if let Some(this) = weak.upgrade() {
                        match crate::fastfetch_run::launch_in(&target, terminal) {
                            Ok(()) => { if let Some(win) = win.upgrade() { win.close(); } this.toast("Target process started. Check that terminal for rendering or runtime errors."); }
                            Err(e) => this.toast(&e),
                        }
                    }
                });
                review.present();
            });
        }
        let close = gtk::Button::with_label("Close");
        let restore = gtk::Button::with_label("Restore This Application…");
        restore.set_sensitive(report.can_restore());
        restore.set_widget_name("scheme-restore");
        buttons.append(&restore);
        if let Some(uuid) = &report.profile_uuid {
            let verify = gtk::Button::with_label("Open Profile Tab");
            verify.set_tooltip_text(Some("Open a new Ptyxis tab with this exact profile and its normal shell. Existing shell startup may run your commands. Use a new window to verify the default grid size."));
            let uuid = uuid.clone();
            let weak = Rc::downgrade(self);
            verify.connect_clicked(move |_| {
                if let Err(error) = open_profile(&uuid)
                    && let Some(this) = weak.upgrade()
                {
                    this.toast(&error);
                }
            });
            buttons.append(&verify);
        }
        buttons.append(&close);
        let weak = window.downgrade();
        close.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.destroy();
            }
        });
        let weak_window = window.downgrade();
        let weak = Rc::downgrade(self);
        restore.connect_clicked(move |_| {
            let Some(this) = weak.upgrade() else { return; };
            let Some(window) = weak_window.upgrade() else { return; };
            let confirm = gtk::AlertDialog::builder().message("Restore this application's external changes?")
                .detail("Only this application's recorded changes will be restored. External edits block restoration for that item; other items can still be restored. Workspace edits and exported files are kept.")
                .buttons(["Cancel", "Restore Changes"]).cancel_button(0).default_button(0).modal(true).build();
            let weak = Rc::downgrade(&this);
            let directory = directory.clone();
            let weak_window = window.downgrade();
            confirm.choose(Some(&window), gio::Cancellable::NONE, move |response| {
                if response != Ok(1) { return; }
                let Some(this) = weak.upgrade() else { return; };
                if let Some(window) = weak_window.upgrade() { window.destroy(); }
                match Report::load(&directory).and_then(|mut report| { report.restore(&directory)?; Ok(report) }) {
                    Ok(report) => { this.accept_scheme_restore(&report); this.refresh_deployment(); this.refresh_output_bar(); this.show_scheme_report(directory, report); }
                    Err(error) => this.toast(&error),
                }
            });
        });
        gtk::prelude::GtkWindowExt::set_focus(&window, Some(&close));
        window.present();
        close.grab_focus();
    }
}

impl Workbench {
    fn accept_scheme_restore(self: &Rc<Self>, report: &Report) {
        for row in &report.items {
            if row.status != Status::Restored {
                continue;
            }
            if let Some(path) = &row.path {
                if row.id == "fastfetch"
                    && !path.starts_with(glib::user_data_dir().join("termimochi/targets"))
                    && let Ok(target) = crate::fastfetch_apply::Target::open(path.clone())
                {
                    self.accept_scheme_fastfetch(target);
                }
                if row.id == "starship"
                    && let Ok(file) = crate::starship_file::FileSnapshot::read(path)
                {
                    let same_file = self
                        .starship_editor
                        .file
                        .borrow()
                        .as_ref()
                        .is_ok_and(|old| old.path == *path);
                    if same_file {
                        self.starship_editor
                            .accept_saved(self.starship_editor.document(), file);
                    }
                }
            }
        }
    }
}

fn profile_command(uuid: &str) -> Result<std::process::Command, String> {
    if !crate::ptyxis::valid_profile_uuid(uuid) {
        return Err("Invalid profile UUID.".into());
    }
    let mut command = std::process::Command::new("/usr/bin/ptyxis");
    command.arg(format!("--tab-with-profile={uuid}"));
    Ok(command)
}

#[cfg(test)]
fn shell_argument(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn open_profile(uuid: &str) -> Result<(), String> {
    let global = crate::ptyxis::find_settings("org.gnome.Ptyxis", None)
        .ok_or("Ptyxis settings are unavailable.")?;
    if !global
        .settings_schema()
        .is_some_and(|schema| schema.has_key("profile-uuids"))
        || !global.strv("profile-uuids").iter().any(|id| id == uuid)
    {
        return Err("The reviewed profile no longer exists. No terminal was opened.".into());
    }
    let command = profile_command(uuid)?;
    #[cfg(not(test))]
    {
        let mut child = {
            let mut command = command;
            command.spawn().map_err(|e| e.to_string())?
        };
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
    #[cfg(test)]
    let _ = command; // Tests never open the user's terminal.
    Ok(())
}

type Prepared = Result<(String, Option<(String, String)>, Action), String>;
fn add(plan: &mut Plan, id: &'static str, title: &str, prepared: Prepared) {
    let (detail, versions, action) = match prepared {
        Ok((detail, versions, action)) => (detail, versions, Some(action)),
        Err(error) => (error, None, None),
    };
    plan.items.push(Item {
        id,
        title: title.into(),
        detail,
        versions,
        action,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::greeting::tests::{descendants, project_controller as controller, settle};

    fn memory_profile() -> (gio::Settings, gio::Settings) {
        assert_eq!(
            std::env::var("GSETTINGS_BACKEND").as_deref(),
            Ok("memory"),
            "Never exercise external writes against real settings"
        );
        let global = crate::ptyxis::find_settings("org.gnome.Ptyxis", None).unwrap();
        global.set_strv("profile-uuids", ["scheme-test"]).unwrap();
        global
            .set_string("default-profile-uuid", "scheme-test")
            .unwrap();
        let profile = crate::ptyxis::find_settings(
            "org.gnome.Ptyxis.Profile",
            Some("/org/gnome/Ptyxis/Profiles/scheme-test/"),
        )
        .unwrap();
        profile.set_string("label", "Scheme Test Profile").unwrap();
        (global, profile)
    }

    fn top(title: &str) -> gtk::Window {
        gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.title().as_deref() == Some(title))
            .unwrap_or_else(|| panic!("Missing window {title}"))
    }

    fn check(window: &gtk::Window, id: &str) -> gtk::CheckButton {
        descendants(window.upcast_ref())
            .into_iter()
            .find(|w| w.widget_name() == format!("scheme-{id}"))
            .unwrap()
            .downcast()
            .unwrap()
    }

    fn button(window: &gtk::Window, name: &str) -> gtk::Button {
        descendants(window.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Button>().ok())
            .find(|w| w.label().as_deref() == Some(name))
            .unwrap_or_else(|| panic!("Missing button {name}"))
    }

    fn capture(window: &gtk::Window, name: &str) {
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(window)).snapshot(
            &snapshot,
            f64::from(window.width()),
            f64::from(window.height()),
        );
        let texture = window
            .renderer()
            .unwrap()
            .render_texture(snapshot.to_node().unwrap(), None);
        let path = glib::user_cache_dir().join(name);
        texture.save_to_png(&path).unwrap();
        println!("CAPTURE {}", path.display());
    }

    #[test]
    fn verification_profile_is_an_argument_not_shell_code() {
        assert_eq!(shell_argument("a 'b' $()"), "'a '\\''b'\\'' $()'");
        assert!(profile_command("a'; touch /tmp/nope").is_err());
        let command = profile_command("scheme-test").unwrap();
        assert_eq!(command.get_program(), "/usr/bin/ptyxis");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["--tab-with-profile=scheme-test"]
        );
    }

    #[test]
    #[ignore = "requires isolated memory GSettings and system Ptyxis schema; settings transaction matrix"]
    fn scheme_settings_recheck_conflicts_restore_unset_and_preserve_other_keys() {
        let (global, profile) = memory_profile();
        let root = tempfile::tempdir().unwrap();
        let initial = profile.user_value("palette");
        let style = global.user_value("interface-style");
        let request =
            scheme_apply::activation::Activation::prepare("scheme-test", "workspace-test", true)
                .unwrap();
        assert!(request.apply(root.path()).unwrap());
        assert_eq!(profile.string("palette"), "workspace-test");
        assert_eq!(global.string("interface-style"), "light");
        profile.set_double("opacity", 0.85).unwrap();
        scheme_apply::activation::restore(root.path()).unwrap();
        assert_eq!(profile.user_value("palette"), initial);
        assert_eq!(global.user_value("interface-style"), style);
        assert_eq!(profile.double("opacity"), 0.85);
        let request =
            scheme_apply::activation::Activation::prepare("scheme-test", "workspace-test", false)
                .unwrap();
        profile.set_string("palette", "external").unwrap();
        assert!(request.apply(&root.path().join("conflict")).is_err());
        assert!(!root.path().join("conflict").exists());
        assert!(scheme_apply::activation::restore(root.path()).is_err());
        assert_eq!(profile.string("palette"), "external");

        let typography = TypographyTarget::for_uuid("scheme-test")
            .unwrap()
            .prepare(&TypographySettings::default())
            .unwrap();
        let layout = layout_apply::ApplyRequest::discover(&LayoutSettings::default()).unwrap();
        let mut plan = Plan::new(root.path(), "Ptyxis".into(), Some("scheme-test".into()));
        add(
            &mut plan,
            "typography",
            "Typography",
            Ok((typography.detail(), None, Action::Typography(typography))),
        );
        add(
            &mut plan,
            "layout",
            "Layout",
            Ok((layout.detail(), None, Action::Layout(layout))),
        );
        let (directory, mut report) = plan.apply(&[true, true]).unwrap();
        assert!(report.items.iter().all(|r| r.status == Status::Applied));
        global.set_string("font-name", "Monospace 22").unwrap();
        report.restore(&directory).unwrap();
        assert_eq!(report.items[0].status, Status::RestoreBlocked);
        assert_eq!(report.items[1].status, Status::Restored);
        assert_eq!(global.string("font-name"), "Monospace 22");

        // Failed installation blocks activation, not independent file exports.
        let palettes = root.path().join("palettes");
        std::fs::create_dir(&palettes).unwrap();
        let mut plan = Plan::new(root.path(), "Ptyxis".into(), Some("scheme-test".into()));
        let activation =
            scheme_apply::activation::Activation::prepare("scheme-test", "new", true).unwrap();
        let installer = PtyxisInstaller::new(palettes.clone(), root.path().join("unused"));
        add(
            &mut plan,
            "palette",
            "Palette",
            Ok((
                "Reviewed empty target".into(),
                None,
                Action::Palette {
                    installer,
                    name: "new.palette".into(),
                    before: None,
                    contents: "new".into(),
                },
            )),
        );
        add(
            &mut plan,
            "activate",
            "Activate",
            Ok((activation.detail(), None, Action::Activate(activation))),
        );
        add(
            &mut plan,
            "starship",
            "Export",
            Ok((
                "Export only".into(),
                None,
                Action::ExportStarship("[rust]\nsymbol='rs '\n".into()),
            )),
        );
        std::fs::write(palettes.join("new.palette"), "external").unwrap();
        let (_, report) = plan.apply(&[true, true, true]).unwrap();
        assert_eq!(
            report
                .items
                .iter()
                .map(|row| row.status)
                .collect::<Vec<_>>(),
            [Status::Failed, Status::Failed, Status::Exported]
        );
        assert_eq!(profile.string("palette"), "external");
        assert!(!report.can_restore());
    }

    #[test]
    #[ignore = "requires isolated GTK/VTE, memory GSettings and XDG dirs; complete scheme review/apply/recovery"]
    fn scheme_review_cancel_apply_results_reopen_and_restore() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let (global, profile) = memory_profile();
        let old_palette = profile.user_value("palette");
        let old_style = global.user_value("interface-style");
        let old_font = global.user_value("font-name");
        let old_columns = global.user_value("default-columns");
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.SchemeTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_advanced_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let main = app.active_window().unwrap();
        let this = controller(&main);
        settle();
        this.starship_editor
            .begin_detached(Some("[rust]\nsymbol = 'rs '\n".into()))
            .unwrap();
        this.workspace_prompt_loaded.set(true);
        this.prompt_source_selector.set_selected(1);
        this.set_typography_settings(&TypographySettings::default(), false);
        settle();
        let state = typography_preset::state_directory();
        assert!(Report::latest(&state).is_err());
        this.request_scheme_apply();
        settle();
        let review = top("Apply This Scheme");
        assert!(!button(&review, "Back Up & Apply Selected").is_sensitive());
        assert!(!check(&review, "activate").is_sensitive());
        assert!(!check(&review, "preview-only").is_sensitive());
        assert!(!state.join("scheme-applies").exists());
        button(&review, "Cancel").emit_clicked();
        settle();
        assert!(!state.join("scheme-applies").exists());
        assert_eq!(global.user_value("font-name"), old_font);

        this.request_scheme_apply();
        settle();
        let review = top("Apply This Scheme");
        for id in [
            "palette",
            "activate",
            "typography",
            "layout",
            "starship",
            "fastfetch",
        ] {
            let control = check(&review, id);
            assert!(control.is_sensitive(), "{id} unavailable");
            control.set_active(true);
        }
        settle();
        capture(&review, "scheme-review.png");
        let confirm = button(&review, "Back Up & Apply Selected");
        let bounds = confirm.compute_bounds(&review).unwrap();
        assert!(bounds.y() + bounds.height() <= review.height() as f32);
        confirm.emit_clicked();
        settle();
        let result = top("Scheme Application Results");
        capture(&result, "scheme-results.png");
        let (directory, report) = Report::latest(&state).unwrap();
        for row in &report.items {
            assert!(
                !matches!(
                    row.status,
                    Status::Failed | Status::Pending | Status::Skipped
                ),
                "{}: {}",
                row.title,
                row.detail
            );
        }
        assert!(
            report
                .items
                .iter()
                .any(|r| r.id == "starship" && r.status == Status::Exported)
        );
        let applied_name = this
            .scheme_plan(&this.workspace_snapshot())
            .items
            .into_iter()
            .find_map(|item| match item.action {
                Some(Action::Palette { name, .. }) => Some(name),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            profile.string("palette"),
            applied_name.trim_end_matches(".palette")
        );
        assert!(directory.join("starship.toml").is_file());
        button(&result, "Close").emit_clicked();
        this.show_last_scheme_application();
        settle();
        let result = top("Scheme Application Results");
        button(&result, "Restore This Application…").emit_clicked();
        settle();
        let confirm = gtk::Window::list_toplevels()
            .into_iter()
            .flat_map(|w| descendants(&w))
            .filter_map(|w| w.downcast::<gtk::Button>().ok())
            .find(|b| b.label().as_deref() == Some("Restore Changes"))
            .unwrap();
        confirm.emit_clicked();
        settle();
        let report = Report::load(&directory).unwrap();
        assert!(
            !report.can_restore(),
            "{}",
            serde_json::to_string(&report).unwrap()
        );
        assert_eq!(profile.user_value("palette"), old_palette);
        assert_eq!(global.user_value("interface-style"), old_style);
        assert_eq!(global.user_value("font-name"), old_font);
        assert_eq!(global.user_value("default-columns"), old_columns);
        // Recovery refreshed the editor's conflict baseline, so a new review works.
        assert!(this.prepare_scheme_fastfetch().is_ok());
        top("Scheme Application Results").destroy();
        this.request_scheme_apply();
        settle();
        let review = top("Apply This Scheme");
        assert!(check(&review, "fastfetch").is_sensitive());
        button(&review, "Cancel").emit_clicked();
        main.destroy();
        settle();
    }
}
