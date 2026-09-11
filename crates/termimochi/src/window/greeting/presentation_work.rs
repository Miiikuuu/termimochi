//! Bounded conversion drafts and advisory output checks. GTK stays on its thread.
use super::*;
use crate::greeting_output::{self, Presentation, VerificationKey, background::LatestTask};
use crate::pixel_trial::Terminal;
use std::time::Instant;

const EDIT_DELAY: Duration = Duration::from_millis(150);
const CHECK_TTL: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub(super) struct EditRequest {
    before: GreetingSettings,
    pub intent: Presentation,
    trial_after: bool,
}
pub(super) type EditTask = LatestTask<EditRequest, GreetingSettings>;
pub(super) fn edit_task() -> EditTask {
    LatestTask::new(|request: EditRequest| greeting_output::edit(&request.before, request.intent))
}

impl GreetingEditor {
    pub(in crate::window) fn connect_output_checked(&self, callback: impl Fn() + 'static) {
        *self.presentation.check_completed.borrow_mut() = Some(Box::new(callback));
    }
    pub(in crate::window) fn invalidate_output_checks(&self) {
        self.presentation.checks.borrow_mut().cancel();
    }
    pub(in crate::window) fn presentation_pending(&self) -> bool {
        self.presentation.edits.borrow().input().is_some()
    }
    pub(in crate::window) fn require_presentation_ready(&self) -> Result<(), String> {
        if self.presentation_pending() {
            Err("Artwork is still updating. Wait for the result or use Undo to cancel.".into())
        } else {
            Ok(())
        }
    }
    pub(super) fn queue_presentation_edit(&self, intent: Presentation) {
        self.queue_presentation_request(intent, false);
    }
    pub(super) fn queue_character_trial(&self) {
        let mut intent = self.settings().presentation;
        intent.visual = greeting_output::Visual::Character;
        intent.protocol = None;
        self.queue_presentation_request(intent, true);
    }
    fn queue_presentation_request(&self, intent: Presentation, trial_after: bool) {
        self.presentation.edits.borrow_mut().submit(
            EditRequest {
                before: self.settings(),
                intent,
                trial_after,
            },
            EDIT_DELAY,
        );
        self.presentation
            .note
            .set_label("Updating artwork… Undo cancels the pending edit.");
        self.notify();
    }
    pub(super) fn poll_presentation_work(&self) {
        if self.root.root().is_none_or(|root| !root.is_realized()) {
            self.presentation.edits.borrow_mut().cancel();
            self.presentation.checks.borrow_mut().cancel();
            return;
        }
        let completed = if self.invalid.get() {
            None
        } else {
            self.presentation.edits.borrow_mut().poll()
        };
        if let Some((request, result)) = completed {
            let current = self.settings();
            if current != request.before {
                if current.editable_artwork == request.before.editable_artwork
                    && current.presentation == request.before.presentation
                {
                    // A field changed during conversion. Rebase on it, never
                    // replace new fields with a worker's older full snapshot.
                    self.queue_presentation_request(request.intent, request.trial_after);
                } else {
                    self.refresh();
                    self.presentation
                        .note
                        .set_label("Artwork changed; the outdated conversion was discarded.");
                    self.notify();
                }
            } else {
                match result {
                    Ok(next) => {
                        self.replace(next, true);
                        if request.trial_after {
                            let _ = self.root.activate_action("win.try-greeting", None);
                        }
                    }
                    Err(error) => {
                        self.refresh();
                        self.presentation.note.set_label(&error);
                        self.notify();
                    }
                }
            }
        }
        let checked = self.presentation.checks.borrow_mut().poll();
        if checked && let Some(callback) = self.presentation.check_completed.borrow().as_ref() {
            // Advisory completion is not a document edit: do not bump its
            // revision, rebuild the preview or restart unrelated discovery.
            callback();
        }
    }
}

#[derive(Clone, PartialEq)]
pub(in crate::window) struct CheckInput {
    settings: GreetingSettings,
    terminal: Terminal,
    context: String,
    deployed: Option<(PathBuf, Option<Vec<u8>>)>,
}
impl CheckInput {
    pub(super) fn verification_key(&self) -> Result<VerificationKey, String> {
        let executable = self
            .terminal
            .executable()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| (m.len(), m.modified().ok()));
        VerificationKey::new(
            &self.settings,
            self.terminal,
            format!(
                "{:?}|{}|{}",
                executable,
                self.context,
                greeting_output::environment(self.terminal)?
            ),
        )
    }
}
#[derive(Clone)]
pub(in crate::window) struct CheckResult {
    pub key: Result<VerificationKey, String>,
    pub deployment_matches: bool,
    pub profile: String,
}
pub(super) struct OutputChecks {
    task: LatestTask<CheckInput, CheckResult>,
    cached: Option<(CheckInput, CheckResult, Instant)>,
}
impl OutputChecks {
    pub fn new() -> Self {
        Self {
            task: LatestTask::new(|input: CheckInput| {
                Ok(CheckResult {
                    key: input.verification_key(),
                    deployment_matches: input.deployed.as_ref().is_some_and(|(path, expected)| {
                        crate::fastfetch_apply::Target {
                            path: path.clone(),
                            expected: expected.clone(),
                        }
                        .check()
                        .is_ok()
                    }),
                    profile: crate::scheme_apply::activation::profile()
                        .map(|(_, name)| name)
                        .unwrap_or_else(|_| "unavailable".into()),
                })
            }),
            cached: None,
        }
    }
    pub fn get(&mut self, input: CheckInput) -> Option<CheckResult> {
        if let Some((key, result, at)) = &self.cached
            && *key == input
            && at.elapsed() < CHECK_TTL
        {
            return Some(result.clone());
        }
        if self.task.input() != Some(&input) {
            self.task.submit(input, Duration::from_millis(80));
        }
        None
    }
    fn poll(&mut self) -> bool {
        if let Some((input, result)) = self.task.poll() {
            self.cached = Some((
                input,
                result.unwrap_or_else(|error| CheckResult {
                    key: Err(error),
                    deployment_matches: false,
                    profile: "unavailable".into(),
                }),
                Instant::now(),
            ));
            return true;
        }
        // Expiry triggers an advisory refresh even without further input.
        if self
            .cached
            .as_ref()
            .is_some_and(|(_, _, at)| at.elapsed() >= CHECK_TTL)
            && self.task.input().is_none()
        {
            self.cached = None;
            return true;
        }
        false
    }
    pub fn cancel(&mut self) {
        self.task.cancel();
        self.cached = None;
    }
}
impl Workbench {
    pub(in crate::window) fn greeting_check_input(&self) -> CheckInput {
        CheckInput {
            settings: self.greeting.settings(),
            terminal: self.greeting.presentation.binding.borrow().terminal,
            context: format!(
                "{:?}|{}x{}|{}",
                self.typography_settings(),
                self.preview_terminal.char_width(),
                self.preview_terminal.char_height(),
                self.preview_terminal.scale_factor()
            ),
            deployed: self
                .greeting
                .presentation
                .deployed
                .borrow()
                .as_ref()
                .map(|(_, _, file, _)| (file.path.clone(), file.expected.clone())),
        }
    }
    pub(in crate::window) fn cached_greeting_check(&self) -> Option<CheckResult> {
        self.greeting
            .presentation
            .checks
            .borrow_mut()
            .get(self.greeting_check_input())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::greeting::tests::{greeting_controller as controller, settle};

    fn ready(test: &str) -> (tempfile::TempDir, gtk::Window, Rc<Workbench>) {
        assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id(test)
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_advanced_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let this = controller(&window);
        let until = Instant::now() + Duration::from_secs(20);
        while this.preview_loading.get() {
            assert!(Instant::now() < until);
            settle();
        }
        (root, window.upcast(), this)
    }
    fn finish(this: &Workbench) {
        let until = Instant::now() + Duration::from_secs(10);
        while this.greeting.presentation_pending() {
            assert!(Instant::now() < until);
            settle();
        }
    }
    fn wait_check(this: &Workbench) -> CheckResult {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(result) = this.cached_greeting_check() {
                return result;
            }
            assert!(Instant::now() < until);
            settle();
        }
    }
    #[test]
    #[ignore = "isolated GTK/VTE: coalescing, heartbeat, stale results, undo, save guards and close"]
    fn presentation_edits_are_bounded_responsive_and_do_not_overwrite_new_state() {
        let (_root, window, this) = ready("io.github.miiikuuu.termimochi.AsyncEditTest");
        let (mut original, _) = crate::pixel_trial::tests::fixture(false);
        greeting_output::sync_recipe(&mut original).unwrap();
        this.greeting.replace(original.clone(), false);
        this.greeting_module_button.set_active(true);
        *this.greeting.presentation.edits.borrow_mut() = LatestTask::new(|request: EditRequest| {
            // Deliberately slow worker makes UI responsiveness deterministic;
            // the actual decoder/converter still runs after this test-only delay.
            std::thread::sleep(Duration::from_millis(350));
            greeting_output::edit(&request.before, request.intent)
        });
        let ticks = Rc::new(Cell::new(0));
        let counter = ticks.clone();
        let heartbeat = glib::timeout_add_local(Duration::from_millis(5), move || {
            counter.set(counter.get() + 1);
            glib::ControlFlow::Continue
        });
        for columns in 25..49 {
            this.greeting.presentation.width.set_value(columns.into());
        }
        assert_eq!(this.greeting.settings(), original);
        assert!(this.greeting.presentation_pending());
        assert!(!this.save_action.is_enabled());
        assert!(this.undo_action.is_enabled());
        assert!(
            this.committed_workspace()
                .unwrap_err()
                .contains("still updating")
        );
        assert!(
            this.greeting
                .persist()
                .unwrap_err()
                .contains("still updating")
        );
        assert!(this.prepare_greeting_action().is_err());
        settle();
        assert!(
            this.greeting.presentation_pending(),
            "slow worker must still be running"
        );
        assert!(
            ticks.get() >= 10,
            "main context must keep dispatching during conversion"
        );
        this.greeting.presentation.width.set_value(30.0);
        this.greeting.message.set_text("Keep this newer field");
        finish(&this);
        assert_eq!(this.greeting.settings().message, "Keep this newer field");
        assert_eq!(this.greeting.settings().presentation.columns, 30);
        assert_eq!(
            this.greeting
                .settings()
                .editable_artwork
                .unwrap()
                .options()
                .unwrap()
                .columns,
            30
        );
        this.greeting.undo();
        assert_eq!(
            this.greeting.settings().presentation.columns,
            original.presentation.columns
        );
        assert_eq!(this.greeting.settings().message, "Keep this newer field");
        this.greeting.redo();
        assert_eq!(this.greeting.settings().presentation.columns, 30);

        this.greeting.presentation.width.set_value(42.0);
        settle();
        this.greeting.undo(); // cancels the draft rather than undoing unrelated history
        let cancelled = this.greeting.settings();
        settle();
        settle();
        assert_eq!(this.greeting.settings(), cancelled);
        this.greeting.presentation.width.set_value(40.0);
        settle();
        this.greeting.replace(original.clone(), false); // source/workspace replacement
        settle();
        settle();
        assert_eq!(this.greeting.settings(), original);
        let mut invalid = original.presentation.clone();
        invalid.columns = 161;
        this.greeting.queue_presentation_edit(invalid);
        finish(&this);
        assert_eq!(this.greeting.settings(), original);
        assert!(this.greeting.presentation.note.text().contains("8–160"));
        this.greeting.presentation.width.set_value(44.0);
        settle();
        window.destroy();
        settle();
        settle();
        assert_eq!(this.greeting.settings(), original);
        assert!(!this.greeting.presentation_pending());
        heartbeat.remove();
    }

    #[test]
    #[ignore = "isolated GTK/VTE: cached status, expiry and uncached apply authorization"]
    fn output_cache_never_authorizes_apply_after_external_changes() {
        let (root, window, this) = ready("io.github.miiikuuu.termimochi.CheckCacheTest");
        let (mut settings, image) = crate::pixel_trial::tests::fixture(false);
        settings.presentation.visual = greeting_output::Visual::Image;
        this.greeting.replace(settings.clone(), false);
        this.greeting.presentation.target.set_selected(1);
        let daily = root.path().join("config.jsonc");
        std::fs::write(&daily, "// unchanged\n{}").unwrap();
        this.accept_scheme_fastfetch(crate::fastfetch_apply::Target::open(daily.clone()).unwrap());
        let trial = Rc::new(
            crate::pixel_trial::Trial::prepare(
                root.path(),
                &settings,
                &image,
                crate::pixel_trial::tests::options(
                    crate::greeting_image::pixel_export::Protocol::Kitty,
                    false,
                ),
            )
            .unwrap(),
        );
        crate::pixel_trial::tests::confirm(&trial); // transaction fixture, not a native rendering claim
        let key = this.greeting_verification_key().unwrap();
        *this.greeting.presentation.verified.borrow_mut() = Some((key.clone(), trial));
        let checked = wait_check(&this);
        assert!(checked.key.unwrap() == key);
        for _ in 0..50 {
            this.refresh_output_bar();
        }
        assert!(
            this.greeting
                .presentation
                .checks
                .borrow()
                .task
                .input()
                .is_none(),
            "fresh cache must not rescan per refresh"
        );
        assert!(this.prepare_greeting_action().is_ok());
        let kitty = glib::user_config_dir().join("kitty");
        std::fs::create_dir_all(&kitty).unwrap();
        std::fs::write(kitty.join("theme.conf"), "font_size 17\n").unwrap();
        assert!(
            this.cached_greeting_check().unwrap().key.unwrap() == key,
            "UI cache may briefly retain its last snapshot"
        );
        assert!(this.greeting_verification_key().unwrap() != key);
        assert!(
            this.prepare_greeting_action().is_err(),
            "Apply must re-read, not trust the cached success"
        );
        this.greeting
            .presentation
            .checks
            .borrow_mut()
            .cached
            .as_mut()
            .unwrap()
            .2 = Instant::now() - CHECK_TTL;
        assert!(this.cached_greeting_check().is_none());
        assert!(wait_check(&this).key.unwrap() != key);
        this.greeting.invalidate_output_checks();
        assert!(this.cached_greeting_check().is_none());
        wait_check(&this);
        this.greeting.presentation.target.set_selected(2);
        assert!(
            this.cached_greeting_check().is_none(),
            "target switch invalidates the cache"
        );
        wait_check(&this);
        assert_eq!(std::fs::read_to_string(daily).unwrap(), "// unchanged\n{}");
        window.destroy();
    }
}
