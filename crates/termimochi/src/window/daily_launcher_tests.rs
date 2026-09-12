use super::*;
use crate::window::greeting::tests::{controller, descendants, settle};

fn wait(mut predicate: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while !predicate() {
        assert!(
            std::time::Instant::now() < deadline,
            "GUI operation timed out"
        );
        settle();
    }
    settle();
}
fn dialog(title: &str) -> Option<gtk::Window> {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| {
            w.is_visible()
                && (w.title().as_deref() == Some(title)
                    || descendants(w.upcast_ref())
                        .into_iter()
                        .filter_map(|v| v.downcast::<gtk::Label>().ok())
                        .any(|l| l.text() == title))
        })
}
fn button(window: &gtk::Window, label: &str) -> gtk::Button {
    descendants(window.upcast_ref())
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|b| b.label().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("Missing {label}"))
}

#[test]
#[ignore = "isolated native GTK + Kitty; review gates, app-menu creation, result/library and deactivated recovery"]
fn launcher_gui_review_install_and_deactivated_recovery() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    assert!(!matches!(
        std::env::var("DISPLAY").as_deref(),
        Ok(":0" | ":1") | Err(_)
    ));
    assert!(std::env::var_os("XDG_DATA_HOME").is_some());
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.DailyLauncherTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let temp = tempfile::tempdir().unwrap();
    present_advanced_with_preset(&app, None, temp.path().join(typography_preset::PRESET_NAME));
    let main = app.active_window().unwrap();
    let this = controller(&main);
    wait(|| !this.preview_loading.get() && !this.copy_loading.get());
    let root = glib::user_data_dir().join("termimochi/kitty-sessions");
    let entry = crate::kitty_session::Plan::prepare(
        &root,
        "gui-daily",
        "GUI Daily",
        1,
        &this.workspace_snapshot(),
        crate::kitty_session::Ownership {
            palette: true,
            ..Default::default()
        },
        None,
        [8, 16],
    )
    .unwrap()
    .publish()
    .unwrap();
    this.choose_daily_launcher(entry.clone());
    wait(|| dialog("Choose the launcher's Bash environment").is_some());
    let chooser = dialog("Choose the launcher's Bash environment").unwrap();
    button(&chooser, "Isolated Bash").emit_clicked();
    wait(|| dialog("Add to Application Menu").is_some());
    let review = dialog("Add to Application Menu").unwrap();
    let confirmation = descendants(review.upcast_ref())
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::CheckButton>().ok())
        .next()
        .unwrap();
    let install = button(&review, "Create / Update App Launcher");
    assert!(!install.is_sensitive() && !confirmation.is_sensitive());
    // Even synthetic toggles/clicks may not bypass the successful trial gate.
    confirmation.set_active(true);
    install.emit_clicked();
    assert!(Installed::discover("gui-daily").unwrap().is_none());
    confirmation.set_active(false);
    crate::window::typed_tests::capture(&review, "daily-launcher-review");
    review.set_title(Some("Add to Application Menu"));
    button(&review, "Try Daily Session").emit_clicked();
    wait(|| confirmation.is_sensitive());
    assert!(!install.is_sensitive());
    // Automated acknowledgement tests the gate, NOT a human usability verdict.
    confirmation.set_active(true);
    assert!(install.is_sensitive());
    install.emit_clicked();
    wait(|| dialog("Theme App Launcher").is_some());
    let result = dialog("Theme App Launcher").unwrap();
    assert!(Installed::discover("gui-daily").unwrap().is_some());
    crate::window::typed_tests::capture(&result, "daily-launcher-installed");
    result.close();
    assert!(
        crate::kitty_session::restore(&root, "gui-daily", &entry.version)
            .unwrap()
            .is_none()
    );
    this.show_kitty_library();
    wait(|| dialog("Independent Kitty Schemes").is_some());
    let library = dialog("Independent Kitty Schemes").unwrap();
    crate::window::typed_tests::capture(&library, "daily-launcher-deactivated-library");
    button(&library, "Remove / Restore App Launcher…").emit_clicked();
    wait(|| dialog("Undo this app-menu launcher update?").is_some());
    let confirm = dialog("Undo this app-menu launcher update?").unwrap();
    button(&confirm, "Undo Launcher Update").emit_clicked();
    wait(|| Installed::discover("gui-daily").unwrap().is_none());
    assert!(
        root.join("gui-daily/versions")
            .join(&entry.version)
            .exists(),
        "removal must retain theme assets"
    );
    library.close();
    main.close();
    settle();
}
