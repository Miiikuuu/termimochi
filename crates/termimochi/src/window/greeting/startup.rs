use super::*;
use crate::{fastfetch_apply, greeting_startup};

impl Workbench {
    pub(in crate::window) fn show_greeting_startup(self: &Rc<Self>) {
        self.show_greeting_startup_at(glib::home_dir().join(".bashrc"));
    }

    fn show_greeting_startup_at(self: &Rc<Self>, path: PathBuf) {
        let current = greeting_startup::current(&path);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_top(20);
        content.set_margin_bottom(20);
        content.set_margin_start(20);
        content.set_margin_end(20);
        let heading = gtk::Label::builder()
            .label("Show Greeting in New Terminals")
            .xalign(0.0)
            .css_classes(["title-3"])
            .build();
        content.append(&heading);
        let toggle = gtk::Switch::builder()
            .active(current.as_ref().is_ok_and(Option::is_some))
            .valign(gtk::Align::Center)
            .build();
        toggle.update_property(&[gtk::accessible::Property::Label(
            "Show Greeting in New Bash Terminals",
        )]);
        content.append(&layout_row("Bash startup", &toggle));
        let note = gtk::Label::builder().label("Runs the applied Fastfetch configuration in new local, top-level Bash terminals. SSH, nested shells, tmux and non-interactive commands are skipped. Turning this off removes only TermiMochi's managed block.").wrap(true).max_width_chars(52).xalign(0.0).css_classes(["dim-label"]).build();
        content.append(&note);
        let status = gtk::Label::builder()
            .wrap(true)
            .max_width_chars(52)
            .xalign(0.0)
            .selectable(true)
            .build();
        match &current {
            Ok(Some(config)) => status.set_text(&format!("Enabled · {}", config.display())),
            Ok(None) => status.set_text("Off · no startup changes made"),
            Err(error) => {
                status.set_text(error);
                toggle.set_sensitive(false);
            }
        }
        content.append(&status);
        let window = gtk::Window::builder()
            .title("Greeting Startup")
            .transient_for(&self.window())
            .modal(true)
            .destroy_with_parent(true)
            .default_width(470)
            .child(&content)
            .build();
        let weak = Rc::downgrade(self);
        let parent = window.downgrade();
        let updating = Rc::new(Cell::new(false));
        toggle.connect_state_set(move |toggle, enabled| {
            if updating.get() { return glib::Propagation::Proceed; }
            let Some(this) = weak.upgrade() else { return glib::Propagation::Stop; };
            let target = if enabled {
                this.greeting.fastfetch_target.borrow().clone().map(Ok).unwrap_or_else(|| {
                    fastfetch_apply::last_path(&this.greeting.fastfetch_state).and_then(|path| fastfetch_apply::Target::open(path.unwrap_or_else(fastfetch_apply::default_path)))
                }).map(Some)
            } else { Ok(None) };
            let plan = match target.and_then(|target| greeting_startup::prepare(path.clone(), target)) {
                Ok(plan) => plan,
                Err(error) => { status.set_text(&error); updating.set(true); toggle.set_active(toggle.state()); updating.set(false); return glib::Propagation::Stop; }
            };
            toggle.set_sensitive(false);
            let detail = format!("{}\n\n{}\n\n{}\n\nA backup is kept. Changes made outside this window block replacement.", plan.path.display(), if enabled {
                "This configuration may contain commands or network modules. Enabling startup runs them automatically in eligible new terminals. Only the applied configuration is used, not unsaved preview edits. The following block will be added:"
            } else { "Only this exact managed block will be removed; the rest of .bashrc is retained:" }, plan.snippet);
            let review = gtk::AlertDialog::builder().message(if enabled { "Enable Greeting at Bash Startup?" } else { "Disable Greeting at Bash Startup?" }).detail(detail).buttons(["Cancel", if enabled { "Enable Startup" } else { "Disable Startup" }]).cancel_button(0).default_button(0).modal(true).build();
            let toggle = toggle.clone();
            let status = status.clone();
            let weak = Rc::downgrade(&this);
            let updating = updating.clone();
            review.choose(parent.upgrade().as_ref(), gio::Cancellable::NONE, move |result| {
                updating.set(true);
                if result == Ok(1) && let Some(this) = weak.upgrade() {
                    match greeting_startup::apply(&plan, &this.greeting.fastfetch_state) {
                        Ok(_) => {
                            toggle.set_state(enabled);
                            status.set_text(if enabled { "Enabled · open a new Bash terminal to see your Greeting." } else { "Off · TermiMochi's startup block was removed. Other settings were kept." });
                        }
                        Err(error) => status.set_text(&error),
                    }
                }
                toggle.set_active(toggle.state());
                toggle.set_sensitive(true);
                updating.set(false);
            });
            glib::Propagation::Stop
        });
        window.present();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::greeting::tests::{controller, descendants, respond, settle};
    #[test]
    #[ignore = "requires GTK; temporary .bashrc only, never opens a user terminal"]
    fn startup_switch_review_cancel_enable_restart_disable_and_conflict() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.StartupTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        let rc = root.path().join(".bashrc");
        let original = "# existing setup\nalias hi='echo hi'\n";
        std::fs::write(&rc, original).unwrap();
        let config = root.path().join("config.jsonc");
        std::fs::write(&config, "{\"modules\":[\"os\"]}").unwrap();
        present_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let this = controller(&window);
        *this.greeting.fastfetch_target.borrow_mut() =
            Some(fastfetch_apply::Target::open(config).unwrap());
        let open = || {
            this.show_greeting_startup_at(rc.clone());
            settle();
            let dialog = gtk::Window::list_toplevels()
                .into_iter()
                .find_map(|w| {
                    w.downcast::<gtk::Window>()
                        .ok()
                        .filter(|w| w.title().as_deref() == Some("Greeting Startup"))
                })
                .unwrap();
            let toggle = descendants(dialog.upcast_ref())
                .into_iter()
                .find_map(|w| w.downcast::<gtk::Switch>().ok())
                .unwrap();
            (dialog, toggle)
        };
        let (dialog, toggle) = open();
        assert!(!toggle.is_active());
        toggle.set_active(true);
        respond("Cancel");
        assert!(!toggle.is_active() && !toggle.state());
        assert_eq!(std::fs::read_to_string(&rc).unwrap(), original);
        toggle.set_active(true);
        respond("Enable Startup");
        assert!(toggle.is_active() && toggle.state());
        assert!(greeting_startup::current(&rc).unwrap().is_some());
        dialog.destroy();
        settle();
        let (dialog, toggle) = open();
        assert!(toggle.is_active());
        toggle.set_active(false);
        respond("Cancel");
        assert!(toggle.is_active());
        toggle.set_active(false);
        respond("Disable Startup");
        assert!(!toggle.is_active());
        assert_eq!(std::fs::read_to_string(&rc).unwrap(), original);
        toggle.set_active(true);
        std::fs::write(&rc, "# external change\n").unwrap();
        respond("Enable Startup");
        assert!(!toggle.is_active());
        assert_eq!(std::fs::read_to_string(&rc).unwrap(), "# external change\n");
        dialog.destroy();
        window.destroy();
    }
}
