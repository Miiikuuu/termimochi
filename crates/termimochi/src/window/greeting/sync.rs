//! Read-only comparison with the applied file. Never executes imported modules
//! or silently replaces a draft / grants a portable preset write authority.
use super::*;
use crate::{fastfetch_apply, fastfetch_document};

#[cfg(test)]
mod ui_tests;

pub(super) struct SyncNotice {
    pub root: gtk::Box,
    difference: gtk::Box,
    message: gtk::Label,
    load: gtk::Button,
    pub save_failure: gtk::Box,
    save_message: gtk::Label,
    observed: RefCell<Option<PathBuf>>,
    pending: Cell<bool>,
    generation: Cell<u64>,
}

impl SyncNotice {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.set_visible(false);
        let row = |text: &str, button: &gtk::Button| {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let label = gtk::Label::builder()
                .label(text)
                .xalign(0.0)
                .wrap(true)
                .max_width_chars(26)
                .hexpand(true)
                .build();
            row.append(&label);
            button.add_css_class("pill");
            button.set_valign(gtk::Align::Center);
            row.append(button);
            row.set_visible(false);
            root.append(&row);
            (row, label)
        };
        let load = gtk::Button::with_label("Load Applied");
        let (difference, message) = row("Preview differs from Fastfetch.", &load);
        let save = gtk::Button::builder()
            .label("Retry Save")
            .action_name("win.save-greeting")
            .build();
        let (save_failure, save_message) = row("Preset not saved.", &save);
        Self {
            root,
            difference,
            message,
            load,
            save_failure,
            save_message,
            observed: RefCell::new(None),
            pending: Cell::new(false),
            generation: Cell::new(0),
        }
    }

    pub fn save_failed(&self, context: &str, error: &str) {
        self.save_message.set_text(context);
        self.save_failure.set_tooltip_text(Some(error));
        self.save_failure.set_visible(true);
        self.refresh_visibility();
    }

    pub fn refresh_visibility(&self) {
        self.root
            .set_visible(self.difference.get_visible() || self.save_failure.get_visible());
    }
}

fn normalized(source: &str) -> Result<serde_json::Value, String> {
    let mut value = fastfetch_document::value(source)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("$schema");
    }
    Ok(value)
}

fn matches(settings: &GreetingSettings, source: &str) -> Result<bool, String> {
    let actual = normalized(source)?;
    if actual == normalized(&settings.fastfetch_config()?)? {
        return Ok(true);
    }
    // Applying a narrow designer preview stacks side-by-side artwork. The
    // saved design retains its adaptive layout; this is not a stale preset.
    if settings.imported_source.is_none()
        && matches!(settings.position, Position::Left | Position::Right)
    {
        let mut stacked = settings.clone();
        stacked.position = Position::Top;
        return Ok(actual == normalized(&stacked.fastfetch_config()?)?);
    }
    Ok(false)
}

fn observe(
    settings: &GreetingSettings,
    explicit: Option<PathBuf>,
    state: &Path,
) -> Result<Option<(PathBuf, bool)>, String> {
    if settings.presentation.visual != crate::greeting_output::Visual::Character
        && settings.editable_artwork.is_some()
    {
        // Pixel deployment uses its exact reviewed destination snapshot in the
        // output bar. Comparing its ANSI fallback to a shared file is incorrect.
        return Ok(None);
    }
    let known = match explicit {
        Some(path) => Some(path),
        None => fastfetch_apply::last_path(state)?,
    };
    let target =
        fastfetch_apply::Target::open(known.clone().unwrap_or_else(fastfetch_apply::default_path))?;
    if target.expected.is_none() && known.is_none() {
        return Ok(None);
    }
    let source = target
        .source()
        .map_err(|error| format!("{}: {error}", target.path.display()))?;
    Ok(Some((target.path, matches(settings, &source)?)))
}

impl Workbench {
    pub(in crate::window) fn connect_fastfetch_sync(this: &Rc<Self>) {
        let weak = Rc::downgrade(this);
        this.greeting.sync.load.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                let path = this.greeting.sync.observed.borrow().clone();
                if let Some(path) = path {
                    this.load_applied_fastfetch_path(path);
                }
            }
        });
    }

    /// Coalesced checks keep bounded file reads and JSONC parsing off GTK.
    /// At most one worker per window; stale results cannot change the notice.
    pub(in crate::window) fn schedule_fastfetch_sync(self: &Rc<Self>) {
        let sync = &self.greeting.sync;
        sync.generation.set(sync.generation.get().wrapping_add(1));
        let settings = self.greeting.settings();
        if settings.presentation.visual != crate::greeting_output::Visual::Character
            && settings.editable_artwork.is_some()
        {
            sync.difference.set_visible(false);
            sync.observed.borrow_mut().take();
            sync.refresh_visibility();
            return;
        }
        if sync.pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(200), move || {
            let Some(this) = weak.upgrade() else {
                return;
            };
            let generation = this.greeting.sync.generation.get();
            let settings = this.greeting.settings();
            let explicit = this
                .greeting
                .fastfetch_target
                .borrow()
                .as_ref()
                .map(|t| t.path.clone());
            let state = this.greeting.fastfetch_state.clone();
            let (send, receive) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = send.send(observe(&settings, explicit, &state));
            });
            let weak = Rc::downgrade(&this);
            glib::timeout_add_local(Duration::from_millis(40), move || {
                let Some(this) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let result = match receive.try_recv() {
                    Ok(result) => result,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        return glib::ControlFlow::Continue;
                    }
                    Err(_) => Err(
                        "Fastfetch comparison could not finish. Refocus the window to retry."
                            .into(),
                    ),
                };
                let sync = &this.greeting.sync;
                sync.pending.set(false);
                if generation != sync.generation.get() {
                    this.schedule_fastfetch_sync();
                    return glib::ControlFlow::Break;
                }
                *sync.observed.borrow_mut() = None;
                match result {
                    Ok(Some((path, matching))) => {
                        sync.message.set_text("Preview differs from Fastfetch.");
                        sync.difference.set_tooltip_text(Some(&format!(
                            "{}\nLoad this configuration into the preview and save it in TermiMochi. Unsaved edits require confirmation. Imported commands are never run.", path.display()
                        )));
                        *sync.observed.borrow_mut() = Some(path);
                        sync.load.set_visible(true);
                        sync.difference.set_visible(!matching);
                    }
                    Ok(None) => sync.difference.set_visible(false),
                    Err(error) => {
                        sync.message
                            .set_text("Cannot compare Fastfetch configuration.");
                        sync.difference.set_tooltip_text(Some(&error));
                        sync.load.set_visible(false);
                        sync.difference.set_visible(true);
                    }
                }
                sync.refresh_visibility();
                glib::ControlFlow::Break
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_compares_artwork_not_comments_or_schema_and_accepts_adaptive_layout() {
        let mut settings = GreetingSettings::starter();
        settings
            .import_artwork(
                crate::greeting_art::Artwork::parse("\x1b[38;2;255;255;255m██\x1b[0m\n██").unwrap(),
            )
            .unwrap();
        let old = settings.fastfetch_config().unwrap();
        assert!(matches(&settings, &format!("// comment\n{old}")).unwrap());
        let mut reformatted = normalized(&old).unwrap();
        reformatted["$schema"] = serde_json::json!("https://example.invalid/schema.json");
        assert!(matches(&settings, &reformatted.to_string()).unwrap());
        settings
            .import_artwork(
                crate::greeting_art::Artwork::parse("  \n\x1b[38;2;12;34;56m▀▀\x1b[0m").unwrap(),
            )
            .unwrap();
        assert!(!matches(&settings, &old).unwrap());
        let mut stacked = settings.clone();
        stacked.position = Position::Top;
        assert!(matches(&settings, &stacked.fastfetch_config().unwrap()).unwrap());
        stacked.gap += 1;
        assert!(!matches(&settings, &stacked.fastfetch_config().unwrap()).unwrap());
    }

    #[test]
    fn sync_is_read_only_and_preserves_unknown_and_command_differences() {
        let root = tempfile::tempdir().unwrap();
        let sentinel = root.path().join("MUST_NOT_RUN");
        let source = serde_json::json!({"modules":[{"type":"command", "text":format!("touch {}", sentinel.display())}], "future":42}).to_string();
        let mut settings = GreetingSettings::starter();
        settings.imported_source = Some(source.clone());
        settings.official_preset = None;
        settings.official_items.clear();
        let path = root.path().join("config.jsonc");
        std::fs::write(&path, &source).unwrap();
        let state = root.path().join("state");
        assert_eq!(
            observe(&settings, Some(path.clone()), &state).unwrap(),
            Some((path.clone(), true))
        );
        assert!(!matches(&settings, &source.replace("42", "43")).unwrap());
        assert!(!matches(&settings, &source.replace("touch", "echo")).unwrap());
        assert!(matches(&settings, "{broken").is_err());
        assert!(!sentinel.exists());
        assert!(!state.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        std::fs::remove_file(&path).unwrap();
        assert!(observe(&settings, Some(path), &state).is_err());
    }
}
