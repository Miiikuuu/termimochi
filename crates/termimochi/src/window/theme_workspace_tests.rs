//! Normal theme workflow; the old typed regressions remain advanced-mode tests.
use super::*;
use crate::window::greeting::tests::{controller, settle};

fn ready(this: &Workbench) {
    let until = std::time::Instant::now() + Duration::from_secs(30);
    while this.preview_loading.get() || this.copy_loading.get() {
        assert!(std::time::Instant::now() < until);
        settle();
    }
    settle();
}

#[test]
#[ignore = "isolated GTK: Ptyxis theme scope, sparse edits, target isolation and unsupported pixel guidance"]
fn theme_workspace_ptyxis_and_two_windows_keep_scope_and_target() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.ThemePtyxisTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Theme.palette");
    std::fs::write(&path, SAMPLE_PALETTE).unwrap();
    present_with_preset(
        &app,
        Some(path.clone()),
        root.path().join(typography_preset::PRESET_NAME),
    );
    let a = controller(&app.active_window().unwrap());
    ready(&a);
    assert!(a.is_theme());
    assert_eq!(a.typed.target.get(), Some(TargetHint::Ptyxis));
    assert_eq!(a.preview_scene_selector.selected(), 0);
    let original = a.committed_design().unwrap();
    for b in [
        &a.typography_module_button,
        &a.layout_module_button,
        &a.prompt_module_button,
        &a.greeting_module_button,
    ] {
        b.set_active(true);
        settle();
        assert_eq!(a.committed_design().unwrap(), original);
    }
    let plan = a.scheme_plan(&a.workspace_snapshot());
    assert!(
        plan.items
            .iter()
            .all(|i| matches!(i.id, "palette" | "activate"))
    );
    a.font_size_input.set_value(17.0);
    settle();
    let edited = a.committed_design().unwrap();
    assert_eq!(edited.theme.as_ref().unwrap().typography.len(), 1);
    assert!(
        a.scheme_plan(&a.workspace_snapshot())
            .items
            .iter()
            .any(|i| i.id == "typography")
    );
    a.name_entry.set_text("My Ptyxis Theme");
    settle();
    assert_eq!(
        a.committed_design().unwrap().theme.as_ref().unwrap().name,
        "My Ptyxis Theme"
    );
    a.open_design_window(DesignDocument::new_theme(
        TargetHint::Kitty,
        "Other Kitty",
        true,
    ));
    let b = controller(&app.active_window().unwrap());
    ready(&b);
    assert_ne!(a.typed.identity.get(), b.typed.identity.get());
    b.font_size_input.set_value(22.0);
    settle();
    assert_eq!(a.font_size_input.value(), 17.0);
    assert_eq!(a.typed.target.get(), Some(TargetHint::Ptyxis));
    assert_eq!(
        b.greeting.presentation.binding.borrow().terminal,
        crate::pixel_trial::Terminal::Kitty
    );
    assert_eq!(
        a.greeting.presentation.binding.borrow().terminal,
        crate::pixel_trial::Terminal::Ptyxis
    );
    assert!(a.merge_theme_input(b.committed_design().unwrap()).is_err());
    let mut art = crate::pixel_trial::tests::fixture(true).0;
    art.presentation.visual = crate::greeting_output::Visual::Animation;
    a.greeting.replace(art, true);
    ready(&a);
    a.request_scheme_apply();
    settle();
    assert!(
        gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .any(
                |w| crate::window::greeting::tests::descendants(w.upcast_ref())
                    .into_iter()
                    .filter_map(|w| w.downcast::<gtk::Label>().ok())
                    .any(|l| l.text().contains("not supported by this target"))
            )
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), SAMPLE_PALETTE);
    assert_eq!(a.typed.target.get(), Some(TargetHint::Ptyxis));
    for w in app.windows() {
        w.destroy();
    }
}

#[test]
#[ignore = "isolated GTK: theme target, sparse edit/undo, Full, in-place import and unified Save/reopen"]
fn theme_workspace_kitty_vertical_edit_save_reopen() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    assert!(!matches!(
        std::env::var("DISPLAY").unwrap().as_str(),
        ":0" | ":1"
    ));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.ThemeWorkspaceTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    present_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
    let window = app.active_window().unwrap();
    let this = controller(&window);
    ready(&this);
    assert!(this.is_theme());
    assert_eq!(this.preview_scene_selector.selected(), 0);
    let path = root.path().join("kitty.conf");
    let source = "background #fafafa\nforeground #242424\n# native retained\n";
    std::fs::write(&path, source).unwrap();
    this.open_design_path(&path);
    ready(&this);
    let before = this.committed_design().unwrap();
    assert_eq!(before.target_hint, Some(TargetHint::Kitty));
    assert!(!before.scope().typography);
    assert!(before.components.prompt.is_none());
    for button in [
        &this.typography_module_button,
        &this.layout_module_button,
        &this.prompt_module_button,
        &this.greeting_module_button,
        &this.palette_module_button,
    ] {
        assert!(button.is_visible() && button.is_sensitive());
        button.set_active(true);
        settle();
        assert_eq!(this.preview_scene_selector.selected(), 0);
        assert_eq!(
            this.committed_design().unwrap(),
            before,
            "navigation must not author settings"
        );
    }
    this.typography_module_button.set_active(true);
    this.font_size_input.set_value(18.0);
    settle();
    let edited = this.committed_design().unwrap();
    assert_eq!(edited.id, before.id);
    assert_eq!(
        edited
            .theme
            .as_ref()
            .unwrap()
            .typography
            .keys()
            .collect::<Vec<_>>(),
        vec!["size"]
    );
    this.undo_edit();
    ready(&this);
    assert_eq!(
        this.committed_design().unwrap(),
        before,
        "Undo restores inherited font"
    );
    this.redo_edit();
    ready(&this);
    assert_eq!(this.committed_design().unwrap(), edited);
    this.merge_theme_input(
        DesignDocument::from_kitty_source("foreground #202020\n# second source\n".into()).unwrap(),
    )
    .unwrap();
    ready(&this);
    let merged = this.committed_design().unwrap();
    assert_eq!(merged.id, before.id);
    assert_eq!(this.font_size_input.value(), 18.0);
    assert!(
        merged
            .native
            .as_ref()
            .unwrap()
            .text
            .contains("background #fafafa")
    );
    assert!(
        merged
            .theme
            .as_ref()
            .unwrap()
            .sources
            .iter()
            .any(|s| s.text.contains("second source"))
    );
    let prompt = crate::design_document::import(
        b"format = 'THEME_PROMPT $directory$character'\n[character]\nsuccess_symbol = '[>](green)'\n",
        &this.workspace_snapshot(),
    )
    .unwrap();
    this.merge_theme_input(prompt).unwrap();
    ready(&this);
    this.toggle_theme_prompt();
    ready(&this);
    assert!(this.theme_prompt_disabled());
    assert!(this.committed_design().unwrap().components.prompt.is_some());
    assert!(
        !crate::window::greeting::tests::feed(&this).contains("THEME_PROMPT"),
        "disabled Prompt must not masquerade as active in Full"
    );
    this.toggle_theme_prompt();
    ready(&this);
    assert!(
        crate::window::greeting::tests::feed(&this)
            .matches("THEME_PROMPT")
            .count()
            >= 2,
        "Full must render the imported theme Prompt on both sides of the command"
    );
    let mut greeting = crate::pixel_trial::tests::fixture(true).0;
    greeting.presentation.visual = crate::greeting_output::Visual::Animation;
    this.greeting.replace(greeting, true);
    ready(&this);
    let complete = this.committed_design().unwrap();
    assert_eq!(complete.id, before.id);
    assert_eq!(complete.target_hint, before.target_hint);
    assert!(complete.components.prompt.is_some());
    assert!(complete.components.greeting.is_some());
    let saved = root.path().join("theme.termimochi-design.json");
    *this.typed.store.borrow_mut() =
        Some(DocumentStore::<DesignDocument>::open(saved.clone()).unwrap());
    gio::prelude::ActionGroupExt::activate_action(&this.window(), "save", None);
    settle();
    assert!(saved.is_file());
    assert!(this.design_clean());
    this.open_design_path(&saved);
    ready(&this);
    assert!(
        crate::window::greeting::tests::feed(&this)
            .matches("THEME_PROMPT")
            .count()
            >= 2,
        "reopened Full must show the saved Prompt, not a reference"
    );
    assert_eq!(this.committed_design().unwrap(), complete);
    assert_eq!(this.preview_scene_selector.selected(), 0);
    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
    assert!(!this.allows_document_action("apply-typography"));
    assert!(!this.allows_document_action("install-ptyxis"));
    assert_eq!(
        this.greeting.presentation.binding.borrow().terminal,
        crate::pixel_trial::Terminal::Kitty
    );
    for (width, height, name) in [
        (1024, 700, "theme-workspace-1024"),
        (1280, 900, "theme-workspace-1280"),
    ] {
        window.set_default_size(width, height);
        settle();
        crate::window::typed_tests::capture(&window, name);
    }
    // End-to-end GUI routing with automated confirmation clicks in private
    // XDG only. These clicks are NOT a human visual/usability approval; the
    // separate controlled_kitty_real_gif_prompt_and_reopen test checks pixels.
    let find_window = |title: &str| {
        gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.is_visible() && w.title().as_deref() == Some(title))
    };
    let button = |w: &gtk::Window, label: &str| {
        crate::window::greeting::tests::descendants(w.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Button>().ok())
            .find(|b| b.label().as_deref() == Some(label))
            .unwrap()
    };
    this.use_kitty_design();
    let until = std::time::Instant::now() + Duration::from_secs(30);
    while find_window("Use Design in Kitty").is_none() {
        assert!(
            std::time::Instant::now() < until,
            "Kitty review unavailable"
        );
        settle();
    }
    let review = find_window("Use Design in Kitty").unwrap();
    assert!(!button(&review, "Create / Update Independent Entry").is_sensitive());
    button(&review, "Try in Kitty").emit_clicked();
    let checks: Vec<_> = crate::window::greeting::tests::descendants(review.upcast_ref())
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::CheckButton>().ok())
        .collect();
    let until = std::time::Instant::now() + Duration::from_secs(30);
    while !checks.iter().all(|c| c.is_sensitive()) {
        assert!(
            std::time::Instant::now() < until,
            "real Kitty launch did not complete"
        );
        settle();
    }
    assert_eq!(checks.len(), 2);
    for check in checks {
        check.set_active(true);
    }
    button(&review, "Create / Update Independent Entry").emit_clicked();
    let until = std::time::Instant::now() + Duration::from_secs(30);
    while find_window("Independent Kitty Entry").is_none() {
        assert!(std::time::Instant::now() < until);
        settle();
    }
    let result = find_window("Independent Kitty Entry").unwrap();
    let sessions = glib::user_data_dir().join("termimochi/kitty-sessions");
    let (entries, issues) = crate::kitty_session::list_with_issues(&sessions).unwrap();
    assert!(issues.is_empty());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, complete.id);
    button(&result, "Open in Kitty").emit_clicked();
    settle();
    crate::window::typed_tests::capture(&result, "theme-workspace-kitty-result");
    result.destroy();
    window.destroy();
    present_with_preset(
        &app,
        Some(saved),
        root.path().join(typography_preset::PRESET_NAME),
    );
    let reopened = controller(&app.active_window().unwrap());
    ready(&reopened);
    assert_eq!(reopened.committed_design().unwrap(), complete);
    reopened.show_kitty_library();
    settle();
    assert!(find_window("Independent Kitty Schemes").is_some());
    crate::kitty_session::restore(&sessions, &entries[0].id, &entries[0].version).unwrap();
    assert!(
        crate::kitty_session::list_with_issues(&sessions)
            .unwrap()
            .0
            .is_empty()
    );
    // Only terminal children launched by this isolated test process.
    let _ = std::process::Command::new("pkill")
        .args([
            "-TERM",
            "-P",
            &std::process::id().to_string(),
            "-x",
            "kitty",
        ])
        .status();
    for w in app.windows() {
        w.destroy();
    }
    window.destroy();
}
