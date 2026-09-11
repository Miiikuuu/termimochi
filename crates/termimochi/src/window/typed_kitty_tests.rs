//! GUI gate coverage only. This test deliberately never authorizes a successful
//! terminal launch, and is not evidence that a user saw correct colors or GIF
//! motion. Real protocol/session rendering has separate native backend tests.
use super::*;
use crate::design_document::{DesignDocument, Kind, Scope};
use crate::window::greeting::tests::{controller, descendants, settle};

fn wait_until(mut ready: impl FnMut() -> bool, description: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while !ready() {
        assert!(
            std::time::Instant::now() < deadline,
            "Timed out: {description}"
        );
        settle();
    }
    settle();
}

fn review_window() -> Option<gtk::Window> {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Window>().ok())
        .find(|window| {
            window.is_visible() && window.title().as_deref() == Some("Use Design in Kitty")
        })
}

fn button(window: &gtk::Window, text: &str) -> gtk::Button {
    descendants(window.upcast_ref())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .find(|button| button.label().as_deref() == Some(text))
        .unwrap_or_else(|| panic!("Missing button: {text}"))
}

fn visual_check(window: &gtk::Window) -> gtk::CheckButton {
    descendants(window.upcast_ref())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::CheckButton>().ok())
        .find(|check| {
            check
                .label()
                .is_some_and(|text| text.starts_with("The colors, font"))
        })
        .unwrap()
}

fn contains_label(window: &gtk::Window, fragment: &str) -> bool {
    descendants(window.upcast_ref())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .any(|label| label.text().contains(fragment))
}

fn assert_no_entries() {
    let (entries, issues) = kitty_session::list_with_issues(&root()).unwrap();
    assert!(issues.is_empty(), "unexpected entry errors: {issues:?}");
    assert!(
        entries.is_empty(),
        "unverified or stale UI must not publish an entry"
    );
}

#[test]
#[ignore = "isolated GTK: main Use reaches Kitty review; unverified publication and stale trials are blocked (no successful terminal launch)"]
fn typed_kitty_main_use_review_and_stale_trial_guards() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    for key in [
        "HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "XDG_CACHE_HOME",
        "XDG_RUNTIME_DIR",
    ] {
        assert!(std::env::var_os(key).is_some(), "missing isolated {key}");
    }
    let display = std::env::var("DISPLAY").unwrap();
    assert!(
        !matches!(display.split('.').next(), Some(":0" | ":1")),
        "never use the user's desktop"
    );
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.TypedKittyReviewTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    crate::window::present_advanced_with_preset(
        &app,
        None,
        temporary.path().join(typography_preset::PRESET_NAME),
    );
    let main = app.active_window().unwrap();
    let this = controller(&main);
    wait_until(
        || !this.preview_loading.get() && !this.copy_loading.get(),
        "initial preview",
    );
    let mut document = DesignDocument::from_workspace(
        Kind::Project,
        Scope {
            palette: true,
            typography: true,
            ..Scope::default()
        },
        &this.workspace_snapshot(),
        None,
    )
    .unwrap();
    document.target_hint = Some(TargetHint::Kitty);
    this.load_design(document);
    wait_until(
        || !this.preview_loading.get() && !this.copy_loading.get(),
        "Kitty design preview",
    );
    assert_no_entries();

    // Click the actual main toolbar button, not a directly constructed plan.
    button(&main, "Use Design…").emit_clicked();
    wait_until(
        || review_window().is_some(),
        "main Use button preparing Kitty review (requires Kitty/Bash installed)",
    );
    let review = review_window().unwrap();
    let visual = visual_check(&review);
    let publish = button(&review, "Create / Update Independent Entry");
    assert!(!visual.is_sensitive());
    assert!(!publish.is_sensitive());
    assert!(contains_label(
        &review,
        "Imported executable instructions are not authorized"
    ));
    publish.emit_clicked();
    assert_no_entries();
    // A forged UI checked state is not an actual trial. held_trial remains None.
    visual.set_active(true);
    publish.emit_clicked();
    assert!(!publish.is_sensitive());
    assert_no_entries();
    visual.set_active(false);

    let trial_root = glib::user_cache_dir().join("termimochi/controlled-trials");
    assert!(
        !trial_root.exists(),
        "no real trial has been prepared or launched"
    );
    this.typed.identity.set(this.typed.identity.get() + 1);
    button(&review, "Try in Kitty").emit_clicked();
    assert!(contains_label(&review, "before starting a trial"));
    assert!(
        !trial_root.exists(),
        "a stale review must not even prepare a trial"
    );
    publish.emit_clicked();
    assert_no_entries();
    review.destroy();

    // Also cover the asynchronous boundary: edit identity immediately after the
    // preparation thread starts, before the GTK result callback can launch it.
    button(&main, "Use Design…").emit_clicked();
    wait_until(|| review_window().is_some(), "fresh Kitty review");
    let review = review_window().unwrap();
    button(&review, "Try in Kitty").emit_clicked();
    this.typed.identity.set(this.typed.identity.get() + 1);
    wait_until(
        || contains_label(&review, "No trial was launched"),
        "stale preparation rejection",
    );
    assert!(!visual_check(&review).is_sensitive());
    assert!(!button(&review, "Create / Update Independent Entry").is_sensitive());
    assert_no_entries();

    review.set_title(Some("TermiMochi point-to-edit test"));
    gtk::prelude::WidgetExt::display(&review).flush();
    let screenshot = glib::user_cache_dir().join("typed-kitty-review-stale-guard.png");
    assert!(
        std::process::Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/preview-pointer-driver.py"
            ))
            .args(["capture", "0", "0"])
            .env("TERMIMOCHI_INSPECT_SCREENSHOT", &screenshot)
            .status()
            .unwrap()
            .success()
    );
    assert!(std::fs::metadata(&screenshot).unwrap().len() > 1024);
    println!(
        "Native GUI gate screenshot (not terminal visual verification): {}",
        screenshot.display()
    );
    review.destroy();
    main.destroy();
    settle();
}
