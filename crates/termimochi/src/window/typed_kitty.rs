//! One target flow: prepare → real controlled trial → review → publish → open.
//! Never invokes the legacy Ptyxis/shared-config executor for a Kitty document.
use super::*;
use crate::{
    design_document::{NativeFormat, TargetHint},
    kitty_session::{self, Ownership},
};
use std::{sync::Arc, thread};

fn root() -> PathBuf {
    glib::user_data_dir().join("termimochi/kitty-sessions")
}

#[cfg(test)]
#[path = "typed_kitty_tests.rs"]
mod tests;

impl Workbench {
    pub(super) fn use_kitty_design(self: &Rc<Self>) {
        if self.typed.target.get() != Some(TargetHint::Kitty) || self.typed.busy.replace(true) {
            return;
        }
        let design = match self.committed_design() {
            Ok(d) => d,
            Err(e) => {
                self.typed.busy.set(false);
                self.toast(&e);
                return;
            }
        };
        let mut snapshot = self.workspace_snapshot();
        let scope = design.scope();
        if let Some(greeting) = design.greeting_output() {
            snapshot.greeting = greeting;
        }
        let owned = Ownership {
            palette: scope.palette,
            typography: scope.typography,
            layout: scope.layout,
            prompt: scope.prompt && design.theme.as_ref().is_none_or(|t| t.prompt_enabled),
            greeting: (scope.greeting || scope.artwork) && snapshot.greeting.enabled,
        };
        let native = if design.theme.is_some() {
            match design.theme_kitty_configuration(&snapshot) {
                Ok(s) => Some(s),
                Err(e) => {
                    self.typed.busy.set(false);
                    self.toast(&e);
                    return;
                }
            }
        } else {
            design
                .native
                .as_ref()
                .filter(|s| s.format == NativeFormat::Kitty)
                .map(|s| s.text.clone())
        };
        let id = design.id.clone();
        let name = design
            .theme
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_else(|| {
                self.typed
                    .store
                    .borrow()
                    .as_ref()
                    .and_then(|s| s.path.file_stem().map(|s| s.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| format!("{} {}", design.kind.label(), &id[..8]))
            });
        let identity = self.typed.identity.get();
        let cells = [
            self.preview_terminal.char_width().max(1) as u32,
            self.preview_terminal.char_height().max(1) as u32,
        ];
        let target_root = root();
        let (send, receive) = mpsc::channel();
        self.toast("Preparing the owned components for Kitty… No daily settings are changed.");
        thread::spawn(move || {
            let _ = send.send(kitty_session::Plan::prepare(
                &target_root,
                &id,
                &name,
                identity,
                &snapshot,
                owned,
                native.as_deref(),
                cells,
            ));
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let result = match receive.try_recv() {
                Ok(r) => r,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("Session preparation stopped.".into()),
            };
            this.typed.busy.set(false);
            if this.typed.identity.get() != identity
                || this.design_snapshot().ok().as_ref() != Some(&design)
            {
                this.toast("Design changed during preparation. Use Design again; no configuration was published.");
            } else {
                match result {
                    Ok(mut plan) => {
                        if let Some(t) = &design.theme {
                            plan.notes.push("Unspecified appearance inherits controlled Kitty defaults, not your daily kitty.conf. App reference font and layout may differ.".into());
                            if t.typography.contains_key("weight") {
                                plan.notes.push("Typography weight is preview-only in this sparse adapter. Choose an exact weighted font family for Kitty; no weight setting was applied.".into());
                            }
                            if t.layout.contains_key("scrollbar") {
                                plan.notes.push("Scrollbar is preview-only; Kitty does not implement this scrollbar control.".into());
                            }
                        }
                        this.review_kitty_plan(Arc::new(plan), design.clone())
                    }
                    Err(e) => this.show_kitty_requirement(&e),
                }
            }
            glib::ControlFlow::Break
        });
    }

    fn show_kitty_requirement(self: &Rc<Self>, error: &str) {
        let dialog = gtk::AlertDialog::builder()
            .message("Kitty session needs attention")
            .detail(error)
            .buttons(["Close", "Retry"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(Some(&self.window()), gio::Cancellable::NONE, move |r| {
            if r == Ok(1)
                && let Some(this) = weak.upgrade()
            {
                this.use_kitty_design();
            }
        });
    }

    fn review_kitty_plan(
        self: &Rc<Self>,
        plan: Arc<kitty_session::Plan>,
        design: crate::design_document::DesignDocument,
    ) {
        let (window, body, buttons) = super::scheme::dialog(
            &self.window(),
            "Use Design in Kitty",
            "Independent Kitty + controlled Bash\nNo Ptyxis, default terminal, shared configuration or shell startup files are modified.",
        );
        body.append(&super::scheme::label(&plan.notes.join("\n")));
        body.append(&super::scheme::versions(
            "New independent version; daily files unchanged",
            &plan.review_text(),
        ));
        body.append(&super::scheme::label("Trial and publication use the same supported safe projection. Imported executable instructions are not authorized here. Original source is retained as inert source files. This is a real shell, not a filesystem sandbox."));
        let state = gtk::Label::builder()
            .label("First open the real target trial, then confirm what you actually see.")
            .wrap(true)
            .xalign(0.0)
            .build();
        body.append(&state);
        let visual = gtk::CheckButton::with_label(
            "The colors, font, current prompt and Greeting look correct for the included components",
        );
        visual.set_sensitive(false);
        body.append(&visual);
        let animation = gtk::CheckButton::with_label(
            "I can see the GIF moving in the Kitty trial (not just in the App preview)",
        );
        animation.set_visible(plan.animated);
        animation.set_sensitive(false);
        body.append(&animation);
        let trial = gtk::Button::with_label("Try in Kitty");
        let publish = gtk::Button::with_label("Create / Update Independent Entry");
        publish.set_sensitive(false);
        buttons.append(&trial);
        buttons.append(&publish);
        let held_trial = Rc::new(RefCell::new(None::<kitty_session::PreparedSession>));
        let running = Rc::new(Cell::new(false));
        for check in [&visual, &animation] {
            let visual = visual.clone();
            let animation = animation.clone();
            let publish = publish.clone();
            let animated = plan.animated;
            check.connect_toggled(move |_| {
                publish.set_sensitive(
                    visual.is_sensitive()
                        && visual.is_active()
                        && (!animated || animation.is_active()),
                )
            });
        }
        let weak = Rc::downgrade(self);
        let weak_window = window.downgrade();
        let plan_trial = plan.clone();
        let v = visual.clone();
        let a = animation.clone();
        let st = state.clone();
        let held = held_trial.clone();
        let active = running.clone();
        let trial_design = design.clone();
        trial.connect_clicked(move |button| {
            let Some(this) = weak.upgrade() else { return; };
            if this.typed.identity.get() != plan_trial.revision || this.committed_design().ok().as_ref() != Some(&trial_design) {
                st.set_text("Document or target changed. Close and use the design again before starting a trial.");
                return;
            }
            if active.replace(true) { return; }
            v.set_active(false); a.set_active(false); v.set_sensitive(false); a.set_sensitive(false); button.set_sensitive(false);
            let plan = plan_trial.clone(); let (send, receive) = mpsc::channel(); let parent = glib::user_cache_dir().join("termimochi/controlled-trials");
            thread::spawn(move || { let _ = send.send(plan.temporary_trial(&parent)); });
            let (weak, weak_window, v, a, st, held, active, button) = (weak.clone(), weak_window.clone(), v.clone(), a.clone(), st.clone(), held.clone(), active.clone(), button.clone());
            let expected_design = trial_design.clone();
            let expected_identity = plan_trial.revision;
            glib::timeout_add_local(Duration::from_millis(50), move || {
                let Some(this) = weak.upgrade().filter(|_| weak_window.upgrade().is_some_and(|w| w.is_visible())) else { return glib::ControlFlow::Break; };
                if this.typed.identity.get() != expected_identity || this.design_snapshot().ok().as_ref() != Some(&expected_design) {
                    active.set(false); button.set_sensitive(true);
                    st.set_text("Document or target changed during preparation. No trial was launched; close and review again.");
                    return glib::ControlFlow::Break;
                }
                let result = match receive.try_recv() { Ok(r) => r, Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue, Err(_) => Err("Trial preparation stopped.".into()) };
                active.set(false); button.set_sensitive(true);
                match result.and_then(|trial| { trial.launch()?; Ok(trial) }) {
                    Ok(trial) => { *held.borrow_mut() = Some(trial); v.set_sensitive(true); a.set_sensitive(true); st.set_text("Kitty process started. Confirm the actual window above; process launch alone does not verify appearance or animation."); }
                    Err(e) => st.set_text(&format!("Trial failed: {e}. Retry after resolving the requirement.")),
                }
                glib::ControlFlow::Break
            });
        });
        let weak = Rc::downgrade(self);
        let win = window.downgrade();
        let publishing = Rc::new(Cell::new(false));
        publish.connect_clicked(move |button| {
            let Some(this) = weak.upgrade() else { return; };
            if this.committed_design().ok().as_ref() != Some(&design) || this.typed.identity.get() != plan.revision {
                state.set_text("Document or target changed. Close and use the design again to review the new version."); button.set_sensitive(false); return;
            }
            if !visual.is_active() || (plan.animated && !animation.is_active()) || held_trial.borrow().is_none() { return; }
            if publishing.replace(true) { return; }
            button.set_sensitive(false);
            state.set_text("Publishing the confirmed snapshot… Versioned file writes are running in the background.");
            let (send, receive) = mpsc::channel();
            let plan = plan.clone();
            let expected_identity = plan.revision;
            thread::spawn(move || { let _ = send.send(plan.publish()); });
            let (weak, win, state, publishing) = (weak.clone(), win.clone(), state.clone(), publishing.clone());
            glib::timeout_add_local(Duration::from_millis(50), move || {
                let result = match receive.try_recv() { Ok(r) => r, Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue, Err(_) => Err("Publication worker stopped; inspect the independent entry library before retrying.".into()) };
                publishing.set(false);
                match result {
                    Ok(deployment) => {
                        if let Some(win) = win.upgrade() { win.close(); }
                        if let Some(this) = weak.upgrade() {
                            if this.typed.identity.get() == expected_identity { this.show_kitty_deployment(deployment); }
                            else { this.toast("The previously confirmed snapshot was published. Open it from the independent Kitty scheme library; the current document was not changed."); }
                        }
                    }
                    Err(e) => state.set_text(&format!("Publication failed: {e}. Existing entry kept. Reopen Use Design to review again.")),
                }
                glib::ControlFlow::Break
            });
        });
        window.present();
    }

    fn show_kitty_deployment(self: &Rc<Self>, deployment: kitty_session::Deployment) {
        let (window, body, buttons) = super::scheme::dialog(
            &self.window(),
            "Independent Kitty Entry",
            &format!(
                "{}\nVersion {} · ready to open",
                deployment.name, deployment.version
            ),
        );
        body.append(&super::scheme::label("Open in Kitty uses isolated Bash. For everyday use without opening the editor, choose Add / Update App Launcher and explicitly select your Bash environment. The normal Kitty icon and default terminal stay unchanged."));
        let state = gtk::Label::builder().wrap(true).xalign(0.0).build();
        body.append(&state);
        let open = gtk::Button::with_label("Open in Kitty");
        buttons.append(&open);
        let daily = gtk::Button::with_label("Add / Update App Launcher…");
        let weak = Rc::downgrade(self);
        let entry = deployment.clone();
        daily.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.choose_daily_launcher(entry.clone());
            }
        });
        buttons.append(&daily);
        self.add_daily_launcher_actions(&body, &deployment.id);
        let restore = gtk::Button::with_label("Restore / Deactivate Entry…");
        buttons.append(&restore);
        let entry = deployment.clone();
        let status = state.clone();
        open.connect_clicked(move |_| match entry.launch() {
            Ok(()) => {
                status.set_text("Kitty process started. Inspect its window to verify the result.")
            }
            Err(e) => status.set_text(&format!("Could not open: {e}")),
        });
        let weak = Rc::downgrade(self);
        restore.connect_clicked(move |_| {
            let Some(this) = weak.upgrade() else { return; };
            let has_previous = deployment.previous.is_some();
            let confirm = gtk::AlertDialog::builder()
                .message(if has_previous { "Restore the previous independent entry?" } else { "Deactivate this independent entry?" })
                .detail(if has_previous { "Only the current-version pointer changes. Version files remain. A pinned app launcher will require review if its version no longer matches. External changes block restoration." } else { "There is no previous version. This removes the entry from the active library and stops its app launcher from opening. Version files and artwork remain. Remove the app-menu launcher separately if it is no longer needed." })
                .buttons(["Cancel", if has_previous { "Restore Entry" } else { "Deactivate Entry" }]).cancel_button(0).default_button(0).modal(true).build();
            let entry = deployment.clone(); let state = state.clone();
            confirm.choose(Some(&this.window()), gio::Cancellable::NONE, move |result| { if result == Ok(1) {
                match kitty_session::restore(&root(), &entry.id, &entry.version) {
                    Ok(Some(_)) => state.set_text("Previous entry restored. Reopen it from the independent scheme library."),
                    Ok(None) => state.set_text("Entry deactivated. Version files and referenced artwork were retained."),
                    Err(e) => state.set_text(&e)
                }
            } });
        });
        window.present();
    }

    pub(super) fn show_kitty_library(self: &Rc<Self>) {
        let (entries, issues) = match kitty_session::list_with_issues(&root()) {
            Ok(e) => e,
            Err(e) => {
                self.toast(&e);
                return;
            }
        };
        let (window, body, _) = super::scheme::dialog(
            &self.window(),
            "Independent Kitty Schemes",
            "Locally published entries · no shared terminal configuration",
        );
        if entries.is_empty() {
            body.append(&super::scheme::label(
                "No independent entries yet. Open or create a Kitty theme, then choose Use Theme.",
            ));
        }
        for issue in issues {
            body.append(&super::scheme::label(&format!(
                "Entry needs repair: {issue}"
            )));
        }
        for entry in entries {
            let button = gtk::Button::with_label(&format!("{} — {}", entry.name, entry.version));
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.show_kitty_deployment(entry.clone());
                }
            });
            body.append(&button);
        }
        body.append(&super::scheme::label(
            "Application-menu launchers (including deactivated themes)",
        ));
        match kitty_session::launcher::Installed::list() {
            Ok(entries) => {
                for (name, result) in entries {
                    body.append(&super::scheme::label(&name));
                    match result {
                        Ok(entry) => self.daily_launcher_buttons(&body, entry),
                        Err(e) => body.append(&super::scheme::label(&format!(
                            "Launcher needs repair: {e}"
                        ))),
                    }
                }
            }
            Err(e) => body.append(&super::scheme::label(&format!(
                "Cannot read launchers: {e}"
            ))),
        }
        window.present();
    }
}
