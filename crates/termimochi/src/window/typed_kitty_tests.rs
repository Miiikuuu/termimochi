//! Separate stale/unverified gates and a private native apply/open lifecycle.
//! Automated UI confirmation is not a human appearance/GIF approval receipt.
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
        .find(|check| check.widget_name() == "kitty-trial-confirmed")
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
    let publish = button(&review, "Apply & Open in Kitty");
    assert_eq!(
        descendants(review.upcast_ref())
            .into_iter()
            .filter(|w| w.is::<gtk::CheckButton>())
            .count(),
        1
    );
    assert!(
        descendants(review.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Expander>().ok())
            .any(|e| e.label().is_some_and(|l| l.starts_with("Advanced")) && !e.is_expanded())
    );
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
    assert!(!button(&review, "Apply & Open in Kitty").is_sensitive());
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

#[test]
#[ignore = "isolated GTK + real Kitty: compact main flow publishes/opens, retries failed opening without republishing"]
fn typed_kitty_compact_apply_opens_real_target_and_retries() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    assert!(!matches!(
        std::env::var("DISPLAY").unwrap().split('.').next(),
        Some(":0" | ":1")
    ));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.CompactKittyTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let temp = tempfile::tempdir().unwrap();
    crate::window::present_advanced_with_preset(
        &app,
        None,
        temp.path().join(typography_preset::PRESET_NAME),
    );
    let main = app.active_window().unwrap();
    let this = controller(&main);
    let mut design = DesignDocument::from_kitty_source(
        "background #dbeaf0\nforeground #202020\nfont_family Liberation Mono\nfont_size 15\n"
            .into(),
    )
    .unwrap();
    design = design
        .into_theme(TargetHint::Kitty, "Compact Kitty Apply")
        .unwrap();
    this.load_design(design);
    settle();
    let original = this.committed_design().unwrap();
    let rc = glib::home_dir().join(".bashrc");
    std::fs::write(&rc, "# private untouched sentinel\n").unwrap();
    let native_windows = || {
        let out = std::process::Command::new("xwininfo")
            .args(["-root", "-tree"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|line| {
                line.contains("Compact Kitty Apply") && line.contains("(\"kitty\" \"kitty\")")
            })
            .count()
    };
    let pointer = |mode: &str, name: &str| {
        let result = std::process::Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/preview-pointer-driver.py"
            ))
            .args([mode, "0", "0"])
            .env(
                "TERMIMOCHI_TEST_WINDOW_TITLE",
                "TermiMochi · Compact Kitty Apply",
            )
            .env(
                "TERMIMOCHI_INSPECT_SCREENSHOT",
                glib::user_cache_dir().join(name),
            )
            .status()
            .unwrap();
        assert!(result.success());
    };
    assert_eq!(native_windows(), 0);
    button(&main, "Use Theme…").emit_clicked();
    wait_until(|| review_window().is_some(), "theme review");
    let review = review_window().unwrap();
    settle();
    crate::window::typed_tests::capture(&review, "compact-kitty-review");
    button(&review, "Try in Kitty").emit_clicked();
    wait_until(
        || visual_check(&review).is_sensitive() && native_windows() == 1,
        "actual Kitty trial window",
    );
    pointer("capture", "compact-kitty-trial.png");
    pointer("close_test_shell", "unused.png");
    wait_until(
        || native_windows() == 0,
        "close the inspected trial before publication",
    );
    visual_check(&review).set_active(true);
    button(&review, "Apply & Open in Kitty").emit_clicked();
    wait_until(
        || native_windows() == 1,
        "publication automatically opens the applied Kitty window",
    );
    let result = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.is_visible() && w.title().as_deref() == Some("Independent Kitty Entry"))
        .unwrap();
    wait_until(
        || button(&result, "Open in Kitty").is_sensitive(),
        "open result callback",
    );
    pointer("capture", "compact-kitty-applied.png");
    let pixels = image::open(glib::user_cache_dir().join("compact-kitty-applied.png"))
        .unwrap()
        .to_rgb8();
    assert!(
        pixels.pixels().filter(|p| p.0 == [219, 234, 240]).count()
            > (pixels.width() * pixels.height()) as usize / 3,
        "the actually opened window must use the reviewed palette, not Kitty's black defaults"
    );
    assert_eq!(this.committed_design().unwrap(), original);
    assert_eq!(
        std::fs::read_to_string(&rc).unwrap(),
        "# private untouched sentinel\n"
    );
    assert!(
        !glib::user_data_dir().join("applications").exists(),
        "apply must not install an app-menu launcher"
    );
    let entries = kitty_session::list_with_issues(&root()).unwrap().0;
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    let config = root()
        .join(&entry.id)
        .join("versions")
        .join(&entry.version)
        .join("kitty.conf");
    let bytes = std::fs::read(&config).unwrap();
    std::fs::write(&config, "# external edit\n").unwrap();
    button(&result, "Open in Kitty").emit_clicked();
    wait_until(
        || contains_label(&result, "opening failed"),
        "tamper blocked and retry available",
    );
    assert_eq!(native_windows(), 1);
    assert_eq!(
        std::fs::read_to_string(&config).unwrap(),
        "# external edit\n"
    );
    std::fs::write(&config, bytes).unwrap(); // restore only this test's deliberate damage
    button(&result, "Open in Kitty").emit_clicked();
    wait_until(
        || native_windows() == 2,
        "retry opens without applying again",
    );
    assert_eq!(
        kitty_session::list_with_issues(&root()).unwrap().0[0].version,
        entry.version
    );
    for remaining in [1, 0] {
        pointer("close_test_shell", "unused.png");
        wait_until(|| native_windows() == remaining, "private shell exit");
    }
    result.destroy();
    main.destroy();
    settle();
}
