//! Explicit daily-use handoff for a published Kitty version.
use super::*;
use crate::kitty_session::{
    Deployment,
    launcher::{Installed, LauncherPlan, ShellMode},
};
use std::{sync::Arc, thread};

#[cfg(test)]
#[path = "daily_launcher_tests.rs"]
mod tests;

impl Workbench {
    pub(super) fn choose_daily_launcher(self: &Rc<Self>, deployment: Deployment) {
        let chooser = gtk::AlertDialog::builder()
            .message("Choose the launcher's Bash environment")
            .detail("My Bash Environment executes your local /etc/bash.bashrc and ~/.bashrc, preserving aliases, PATH and history. Startup stdout is hidden to avoid duplicate greetings; errors remain visible. It does not write these files. Login profiles and other shells are not included. Isolated Bash loads neither file. You will test and confirm before an application-menu entry is created.")
            .buttons(["Cancel", "Isolated Bash", "My Bash Environment"])
            .cancel_button(0).default_button(0).modal(true).build();
        let weak = Rc::downgrade(self);
        chooser.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let mode = match result {
                    Ok(1) => ShellMode::Controlled,
                    Ok(2) => ShellMode::PersonalBash,
                    _ => return,
                };
                if let Some(this) = weak.upgrade() {
                    this.prepare_daily_launcher(deployment.clone(), mode);
                }
            },
        );
    }

    fn prepare_daily_launcher(self: &Rc<Self>, deployment: Deployment, mode: ShellMode) {
        let (send, receive) = mpsc::channel();
        self.toast("Preparing the app launcher; no daily files have been changed.");
        thread::spawn(move || {
            let _ = send.send(LauncherPlan::prepare(deployment, mode));
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let result = match receive.try_recv() {
                Ok(v) => v,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("Launcher preparation stopped.".into()),
            };
            match result {
                Ok(plan) => this.review_daily_launcher(Arc::new(plan)),
                Err(error) => this.toast(&format!("Launcher needs attention: {error}")),
            }
            glib::ControlFlow::Break
        });
    }

    fn review_daily_launcher(self: &Rc<Self>, plan: Arc<LauncherPlan>) {
        let (window, body, buttons) = scheme::dialog(
            &self.window(),
            "Add to Application Menu",
            "Open this theme directly from the system application menu — no editor or copied command required.",
        );
        body.append(&scheme::label(&plan.summary()));
        body.append(&scheme::versions(
            "Review launcher and shell startup",
            &plan.review(),
        ));
        let state = gtk::Label::builder().wrap(true).xalign(0.0)
            .label("Test this exact daily session first. Launching a process does not prove that its appearance, GIF or local shell setup works.").build();
        let confirmed = gtk::CheckButton::with_label(
            "I checked the theme, GIF (if included), and my shell environment in this trial",
        );
        confirmed.set_sensitive(false);
        let trial = gtk::Button::with_label("Try Daily Session");
        let install = gtk::Button::with_label("Create / Update App Launcher");
        install.set_sensitive(false);
        body.append(&state);
        body.append(&confirmed);
        buttons.append(&trial);
        buttons.append(&install);
        let ready = Rc::new(Cell::new(false));
        let busy = Rc::new(Cell::new(false));
        let enabled = install.clone();
        let tried = ready.clone();
        let active = busy.clone();
        confirmed.connect_toggled(move |check| {
            enabled.set_sensitive(tried.get() && check.is_active() && !active.get())
        });
        let (p, status, check, publish, tried, active, win) = (
            plan.clone(),
            state.clone(),
            confirmed.clone(),
            install.clone(),
            ready.clone(),
            busy.clone(),
            window.downgrade(),
        );
        trial.connect_clicked(move |button| {
            if active.replace(true) { return; }
            tried.set(false); check.set_active(false); check.set_sensitive(false); publish.set_sensitive(false); button.set_sensitive(false);
            status.set_text("Starting the reviewed daily trial…");
            let (send, receive) = mpsc::channel(); let p = p.clone();
            thread::spawn(move || { let _ = send.send(p.trial()); });
            let (status, check, tried, active, button, win) = (status.clone(), check.clone(), tried.clone(), active.clone(), button.clone(), win.clone());
            glib::timeout_add_local(Duration::from_millis(50), move || {
                if !win.upgrade().is_some_and(|w| w.is_visible()) { return glib::ControlFlow::Break; }
                let result = match receive.try_recv() { Ok(v) => v, Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue, Err(_) => Err("Trial worker stopped.".into()) };
                active.set(false); button.set_sensitive(true);
                match result {
                    Ok(()) => { tried.set(true); check.set_sensitive(true); status.set_text("Kitty process started. Check its actual window, then confirm above. No app-menu entry has been written."); }
                    Err(e) => status.set_text(&format!("Trial failed: {e}. Close this review and prepare it again.")),
                }
                glib::ControlFlow::Break
            });
        });
        let weak = Rc::downgrade(self);
        let win = window.downgrade();
        install.connect_clicked(move |button| {
            if !ready.get() || !confirmed.is_active() || busy.replace(true) {
                return;
            }
            button.set_sensitive(false);
            trial.set_sensitive(false);
            confirmed.set_sensitive(false);
            state.set_text("Publishing the reviewed app-menu launcher…");
            let (send, receive) = mpsc::channel();
            let plan = plan.clone();
            thread::spawn(move || {
                let _ = send.send(plan.install());
            });
            let (weak, win, state) = (weak.clone(), win.clone(), state.clone());
            glib::timeout_add_local(Duration::from_millis(50), move || {
                let result = match receive.try_recv() {
                    Ok(v) => v,
                    Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                    Err(_) => Err(
                        "Launcher publication stopped. Inspect the app-menu entry before retrying."
                            .into(),
                    ),
                };
                match result {
                    Ok(installed) => {
                        if let Some(win) = win.upgrade() {
                            win.close();
                        }
                        if let Some(this) = weak.upgrade() {
                            this.show_daily_launcher_result(installed);
                        }
                    }
                    Err(e) => state.set_text(&format!(
                        "Launcher was not confirmed as installed: {e}. Close and review again."
                    )),
                }
                glib::ControlFlow::Break
            });
        });
        window.present();
    }

    fn show_daily_launcher_result(self: &Rc<Self>, installed: Installed) {
        let (window, body, _) = scheme::dialog(
            &self.window(),
            "Theme App Launcher",
            "Added to the system application menu. Search for your theme name followed by Kitty. You can close TermiMochi and open it there next time. The ordinary Kitty icon is unchanged.",
        );
        if let Ok(name) = installed.name() {
            body.append(&scheme::label(&format!(
                "Search in the application menu: {name}"
            )));
        }
        self.daily_launcher_buttons(&body, installed);
        window.present();
    }

    pub(super) fn add_daily_launcher_actions(self: &Rc<Self>, body: &gtk::Box, id: &str) {
        match Installed::discover(id) {
            Ok(Some(installed)) => self.daily_launcher_buttons(body, installed),
            Ok(None) => {}
            Err(error) => body.append(&scheme::label(&format!(
                "Existing app launcher needs attention: {error}"
            ))),
        }
    }

    pub(super) fn daily_launcher_buttons(self: &Rc<Self>, body: &gtk::Box, installed: Installed) {
        let state = gtk::Label::builder().wrap(true).xalign(0.0).build();
        let open = gtk::Button::with_label("Open App Launcher");
        let restore = gtk::Button::with_label("Remove / Restore App Launcher…");
        let entry = installed.clone();
        let status = state.clone();
        open.connect_clicked(move |_| {
            let (send, receive) = mpsc::channel();
            let entry = entry.clone();
            thread::spawn(move || {
                let _ = send.send(entry.open());
            });
            let status = status.clone();
            glib::timeout_add_local(Duration::from_millis(50), move || {
                match receive.try_recv() {
                    Ok(Ok(())) => status
                        .set_text("Kitty process started with the installed launcher settings."),
                    Ok(Err(e)) => status.set_text(&e),
                    Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                    Err(_) => status.set_text("Launcher stopped before reporting its result."),
                }
                glib::ControlFlow::Break
            });
        });
        let weak = Rc::downgrade(self);
        let status = state.clone();
        let open_for_restore = open.clone();
        restore.connect_clicked(move |button| {
            let Some(this) = weak.upgrade() else { return; };
            let confirm = gtk::AlertDialog::builder().message("Undo this app-menu launcher update?")
                .detail("The previous app-menu entry will be restored, or this entry removed if it was newly created. Theme versions, artwork and retained runtime remain recoverable. External changes block removal. No shell or default-terminal settings change.")
                .buttons(["Cancel", "Undo Launcher Update"]).cancel_button(0).default_button(0).modal(true).build();
            let (entry, status, button, open) = (installed.clone(), status.clone(), button.clone(), open_for_restore.clone());
            confirm.choose(Some(&this.window()), gio::Cancellable::NONE, move |r| { if r == Ok(1) {
                match entry.restore() {
                    Ok(()) => { status.set_text("App-menu update undone. Managed versions and artwork were retained."); button.set_sensitive(false); open.set_sensitive(false); }
                    Err(e) => status.set_text(&e),
                }
            } });
        });
        body.append(&open);
        body.append(&restore);
        body.append(&state);
    }
}
