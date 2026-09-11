use super::*;
use crate::pixel_trial::{Assessment, Terminal, Trial, Visual};

fn visual(value: Visual) -> &'static str {
    match value {
        Visual::Unverified => "Unverified",
        Visual::Confirmed => "Visually confirmed by you for this trial",
        Visual::Failed => "Visual check failed",
    }
}

impl PixelExport {
    pub(super) fn terminal(&self) -> Terminal {
        Terminal::ALL[self.target.selected().min(2) as usize]
    }

    pub(super) fn connect_trials(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.target.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                this.invalidate_trial();
                this.refresh();
            }
        });
        for (button, ansi) in [(&self.test, false), (&self.ansi_test, true)] {
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    if this.trial_running.get() {
                        this.invalidate_trial();
                        this.status
                            .set_label("Trial stopped. No daily configuration was changed.");
                    } else if ansi {
                        if let Some(workbench) = this.workbench.upgrade() {
                            workbench.greeting.queue_character_trial();
                            if let Some(window) = this.window.upgrade() {
                                window.close();
                            }
                        }
                    } else {
                        let ansi = this
                            .before
                            .presentation
                            .resolve(&this.before, this.terminal())
                            .is_ok_and(|s| s.protocol.is_none());
                        this.start_trial(ansi);
                    }
                }
            });
        }
        let weak = Rc::downgrade(self);
        self.install.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.review_install();
            }
        });
        for (button, key) in self.feedback.iter().zip(*b"ynnt") {
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    let result = this
                        .trial
                        .borrow()
                        .as_ref()
                        .ok_or("No active trial".to_owned())
                        .and_then(|trial| trial.respond(key));
                    if let Err(error) = result {
                        this.status.set_label(&error);
                    }
                    for button in &this.feedback {
                        button.set_sensitive(false);
                    }
                }
            });
        }
        self.invalidate_trial();
    }

    pub(super) fn invalidate_trial(&self) {
        self.trial_generation
            .set(self.trial_generation.get().wrapping_add(1));
        self.trial_running.set(false);
        self.trial.borrow_mut().take(); // Only deletes this newly created trial tree.
        self.show_assessment(&Assessment::default());
        self.refresh_trial_controls();
    }

    pub(super) fn refresh_trial_controls(&self) {
        let available = self.image.borrow().is_some() && !self.writing.get() && !self.closed.get();
        let installed = self.terminal().executable().is_some();
        self.test.set_sensitive(available && installed);
        self.test.set_label(if self.trial_running.get() {
            "Stop Trial"
        } else {
            "Test in Terminal"
        });
        self.ansi_test
            .set_sensitive(available && installed && !self.trial_running.get());
        self.target
            .set_sensitive(!self.trial_mode && !self.writing.get());
        self.protocol
            .set_sensitive(!self.trial_mode && !self.writing.get());
        self.columns
            .set_sensitive(!self.trial_mode && !self.writing.get());
        let verified = self
            .trial
            .borrow()
            .as_ref()
            .and_then(|t| t.assessment().ok())
            .is_some_and(|r| r.done && r.visual == Visual::Confirmed)
            && self.workbench.upgrade().is_some_and(|w| {
                self.key.is_some() && self.key == w.greeting_verification_key().ok()
            });
        self.install
            .set_sensitive(available && verified && !self.trial_running.get());
        if !installed {
            self.capabilities.set_label(&format!("{} is not installed. Kitty static: Unverified · Animation: Unverified · Sixel: Unverified", self.terminal().label()));
        }
    }

    fn show_assessment(&self, assessment: &Assessment) {
        let ready = !assessment.done
            && assessment.asset_hash.is_some()
            && (assessment.message.starts_with("Does the animation")
                || assessment.message.starts_with("Is the artwork"));
        for (index, button) in self.feedback.iter().enumerate() {
            button.set_sensitive(if index == 3 {
                !assessment.done && assessment.message.starts_with("No conclusive")
            } else {
                ready && (index != 2 || self.protocol() == Protocol::KittyAnimation)
            });
        }
        self.capabilities.set_label(&format!(
            "Kitty static: {}\nAnimation: {}\nSixel: {}\nCurrent output: {}{}",
            assessment.kitty.label(),
            visual(assessment.animation),
            assessment.sixel.label(),
            visual(assessment.visual),
            assessment
                .cells
                .map(|[w, h]| format!(" · target cell {w} × {h} px"))
                .unwrap_or_else(|| " · cell size unverified".into())
        ));
    }

    pub(super) fn start_trial(self: &Rc<Self>, ansi: bool) {
        if !self.trial_mode {
            return;
        }
        if self.writing.get() || self.closed.get() {
            return;
        }
        let Some(workbench) = self.workbench.upgrade() else {
            return;
        };
        if !workbench.art_draft_matches(&self.before) {
            return;
        }
        let Some(image) = self.image.borrow().clone() else {
            return;
        };
        self.invalidate_trial();
        // A new trial supersedes old positive evidence, including if launch or
        // the user's new visual assessment fails. Never fall back to old approval.
        workbench.greeting.presentation.verified.borrow_mut().take();
        workbench.refresh_output_bar();
        let generation = self.trial_generation.get();
        self.writing.set(true);
        self.export.set_sensitive(false);
        self.protocol.set_sensitive(false);
        self.columns.set_sensitive(false);
        self.refresh_trial_controls();
        self.status
            .set_label("Preparing a temporary terminal trial; daily configuration is untouched…");
        let settings = self.before.clone();
        let protocol = self.protocol();
        let terminal = self.terminal();
        let columns = self.columns.value_as_int() as u32;
        let cells = self.cell_size;
        let parent = glib::user_cache_dir().join("termimochi/pixel-trials");
        let (send, receive) = mpsc::channel();
        thread::spawn(move || {
            let _ = send.send(Trial::prepare(
                &parent,
                &settings,
                &image,
                crate::pixel_trial::TrialOptions {
                    protocol,
                    terminal,
                    ansi,
                    columns,
                    cells,
                },
            ));
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            let Some(this) = weak
                .upgrade()
                .filter(|t| !t.closed.get() && t.trial_generation.get() == generation)
            else {
                return glib::ControlFlow::Break;
            };
            let result = match receive.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("Trial preparation stopped.".into()),
            };
            this.writing.set(false);
            this.export.set_sensitive(true);
            this.protocol.set_sensitive(true);
            this.columns.set_sensitive(true);
            let result = result.and_then(|trial| {
                #[cfg(test)]
                let helper = std::env::var_os("TERMIMOCHI_SVG_WORKER_BIN")
                    .map(PathBuf::from)
                    .ok_or("Missing isolated trial worker.")?;
                #[cfg(not(test))]
                let helper = std::env::current_exe().map_err(|e| e.to_string())?;
                trial.launch(&helper)?;
                Ok(trial)
            });
            match result {
                Ok(trial) => {
                    *this.trial.borrow_mut() = Some(Rc::new(trial));
                    this.trial_running.set(true);
                    this.status.set_label("Test window opened. Follow its prompts; successful export or process exit is not proof of rendering.");
                    this.watch_trial(generation);
                }
                Err(error) => this
                    .status
                    .set_label(&format!("Trial not started: {error}")),
            }
            this.refresh_trial_controls();
            glib::ControlFlow::Break
        });
    }

    fn watch_trial(self: &Rc<Self>, generation: u64) {
        let started = std::time::Instant::now();
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(150), move || {
            let Some(this) = weak
                .upgrade()
                .filter(|t| !t.closed.get() && t.trial_generation.get() == generation)
            else {
                return glib::ControlFlow::Break;
            };
            if started.elapsed() > Duration::from_secs(210) {
                this.invalidate_trial();
                this.status.set_label("Trial timed out or its terminal was closed. Output remains unverified; retry or choose ANSI fallback.");
                return glib::ControlFlow::Break;
            }
            let result = this.trial.borrow().as_ref().map(|trial| trial.assessment());
            match result {
                Some(Ok(assessment)) => {
                    this.show_assessment(&assessment);
                    this.status.set_label(&assessment.message);
                    if assessment.done {
                        if assessment.visual == Visual::Confirmed
                            && let Some(workbench) = this.workbench.upgrade()
                            && this.key == workbench.greeting_verification_key().ok()
                            && let Some(key) = this.key.clone()
                            && let Some(trial) = this.trial.borrow().as_ref()
                        {
                            *workbench.greeting.presentation.verified.borrow_mut() =
                                Some((key, trial.clone()));
                            workbench.refresh_output_bar();
                        }
                        this.trial_running.set(false);
                        this.refresh_trial_controls();
                        return glib::ControlFlow::Break;
                    }
                }
                Some(Err(error)) => {
                    this.invalidate_trial();
                    this.status
                        .set_label(&format!("Cannot verify this trial: {error}"));
                    return glib::ControlFlow::Break;
                }
                None => return glib::ControlFlow::Break,
            }
            glib::ControlFlow::Continue
        });
    }

    fn review_install(self: &Rc<Self>) {
        if !self.trial_mode || self.writing.get() || self.trial_running.get() {
            return;
        }
        if let Some(workbench) = self.workbench.upgrade() {
            if let Some(window) = self.window.upgrade() {
                window.close();
            }
            workbench.request_scheme_apply();
        }
    }
}
