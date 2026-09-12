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

fn check(this: &Workbench, name: &str, expected: Rgb) {
    settle();
    let widget = &this.preview_terminal_shell;
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(widget)).snapshot(
        &snapshot,
        f64::from(widget.width()),
        f64::from(widget.height()),
    );
    let texture = widget.native().unwrap().renderer().unwrap().render_texture(
        snapshot.to_node().unwrap(),
        Some(&gtk::graphene::Rect::new(
            0.0,
            0.0,
            widget.width() as f32,
            widget.height() as f32,
        )),
    );
    texture
        .save_to_png(glib::user_cache_dir().join(format!("{name}.png")))
        .unwrap();
    let width = texture.width() as usize;
    let mut pixels = vec![0; width * texture.height() as usize * 4];
    texture.download(&mut pixels, width * 4);
    // Top padding belongs to the CSS shell, not VTE's independently set
    // background. Model-only and VTE-only checks miss cross-window CSS leaks.
    let offset = (3 * width + width / 2) * 4;
    let actual = Rgb::new(pixels[offset + 2], pixels[offset + 1], pixels[offset]);
    assert_eq!(
        actual, expected,
        "{name}: another window changed the rendered shell"
    );
    let model = this.model.borrow();
    let foreground = model
        .palette
        .variant(model.active_variant)
        .unwrap()
        .get("Foreground")
        .unwrap();
    let rendered = widget.color();
    assert_eq!(
        Rgb::new(
            (rendered.red() * 255.0).round() as u8,
            (rendered.green() * 255.0).round() as u8,
            (rendered.blue() * 255.0).round() as u8,
        ),
        foreground,
        "{name}: another window changed the inherited text color"
    );
}

#[test]
#[ignore = "isolated GTK: rendered preview colors stay local across file windows, edits and reopen"]
fn theme_windows_keep_rendered_preview_colors_local() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.WindowColorTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    let a_path = root.path().join("First.palette");
    std::fs::write(&a_path, SAMPLE_PALETTE).unwrap();
    present_with_preset(
        &app,
        Some(a_path.clone()),
        root.path().join("unused-font.json"),
    );
    let a = controller(&app.active_window().unwrap());
    ready(&a);
    a.apply_color("Background", Rgb::new(251, 238, 223));
    a.apply_color("Foreground", Rgb::new(37, 43, 51));
    settle();
    let original = a.committed_design().unwrap();
    check(&a, "window-colors-a-before", Rgb::new(251, 238, 223));

    let b_path = root.path().join("Second.conf");
    std::fs::write(&b_path, "background #ede8ef\nforeground #373a43\n").unwrap();
    present_with_preset(
        &app,
        Some(b_path.clone()),
        root.path().join("unused-font.json"),
    );
    let b = controller(&app.active_window().unwrap());
    ready(&b);
    assert_ne!(a.window(), b.window());
    assert_eq!(a.committed_design().unwrap(), original);
    check(&a, "window-colors-a-after-open", Rgb::new(251, 238, 223));
    check(&b, "window-colors-b-open", Rgb::new(237, 232, 239));
    b.apply_color("Background", Rgb::new(21, 32, 43));
    b.apply_color("Foreground", Rgb::new(230, 220, 210));
    for scene in [0, 1, 2, 3] {
        a.preview_scene_selector.set_selected(scene);
        b.preview_scene_selector.set_selected(scene);
        check(
            &a,
            &format!("window-colors-a-scene-{scene}"),
            Rgb::new(251, 238, 223),
        );
        check(
            &b,
            &format!("window-colors-b-scene-{scene}"),
            Rgb::new(21, 32, 43),
        );
    }
    a.refresh_preview();
    check(&b, "window-colors-b-after-a-refresh", Rgb::new(21, 32, 43));
    let b_provider = b.terminal_css_provider.downgrade();
    b.window().destroy();
    drop(b);
    settle();
    check(&a, "window-colors-a-after-close", Rgb::new(251, 238, 223));
    assert!(
        b_provider.upgrade().is_none(),
        "Closed window's provider must leave the display"
    );
    present_with_preset(&app, Some(b_path), root.path().join("unused-font.json"));
    let reopened = controller(&app.active_window().unwrap());
    ready(&reopened);
    check(&a, "window-colors-a-after-reopen", Rgb::new(251, 238, 223));
    check(
        &reopened,
        "window-colors-b-reopened",
        Rgb::new(237, 232, 239),
    );
    assert_eq!(a.committed_design().unwrap(), original);
    reopened.window().destroy();
    a.window().destroy();
}

#[test]
#[ignore = "isolated GTK: same-file windows, variant/font/layout/Prompt edits, undo and Save conflict"]
fn theme_same_file_windows_keep_edits_history_and_save_local() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.SameFileTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    present_with_preset(&app, None, root.path().join("font.json"));
    let a = controller(&app.active_window().unwrap());
    ready(&a);
    let path = root.path().join("shared.termimochi-design.json");
    *a.typed.store.borrow_mut() = Some(DocumentStore::open(path.clone()).unwrap());
    a.save_design(false);
    assert!(a.design_clean());
    present_with_preset(&app, Some(path.clone()), root.path().join("font.json"));
    let b = controller(&app.active_window().unwrap());
    ready(&b);
    let original = b.committed_design().unwrap();
    assert_eq!(a.committed_design().unwrap().id, original.id);
    assert_ne!(a.typed.identity.get(), b.typed.identity.get());
    assert_ne!(a.terminal_css_scope, b.terminal_css_scope);
    let b_font = b.preview_terminal.font_desc();
    let b_layout = b.layout_settings();
    let b_scene = b.preview_scene_selector.selected();
    let b_background = b
        .model
        .borrow()
        .palette
        .variant(Variant::Light)
        .unwrap()
        .get("Background")
        .unwrap();
    a.change_variant(Variant::Dark);
    a.apply_color("Background", Rgb::new(19, 29, 39));
    a.font_size_input.set_value(23.0);
    a.content_padding_input.set_value(27.0);
    a.cursor_shape_selector.set_selected(1);
    a.column_count_input.set_value(100.0);
    let prompt = crate::design_document::import(
        b"format = 'ONLY_WINDOW_A $directory$character'\n",
        &a.workspace_snapshot(),
    )
    .unwrap();
    a.merge_theme_input(prompt).unwrap();
    ready(&a);
    assert_eq!(b.committed_design().unwrap(), original);
    assert_eq!(b.model.borrow().active_variant, Variant::Light);
    assert_eq!(b.preview_terminal.font_desc(), b_font);
    assert_eq!(b.layout_settings(), b_layout);
    assert_eq!(b.preview_scene_selector.selected(), b_scene);
    assert!(crate::window::greeting::tests::feed(&a).contains("ONLY_WINDOW_A"));
    assert!(!crate::window::greeting::tests::feed(&b).contains("ONLY_WINDOW_A"));
    check(&a, "same-file-a-dark", Rgb::new(19, 29, 39));
    check(&b, "same-file-b-light", b_background);
    let a_before_rename = a.committed_design().unwrap();
    a.name_entry.set_text("Window A renamed");
    settle();
    let renamed = a.committed_design().unwrap();
    assert_ne!(renamed, a_before_rename);
    a.undo_edit();
    assert_eq!(a.committed_design().unwrap(), a_before_rename);
    a.redo_edit();
    assert_eq!(a.committed_design().unwrap(), renamed);
    assert_eq!(b.committed_design().unwrap(), original);

    a.save_design(false);
    assert!(a.design_clean());
    let a_saved = std::fs::read(&path).unwrap();
    b.name_entry.set_text("Window B conflicting edit");
    settle();
    let b_edited = b.committed_design().unwrap();
    b.save_design(false); // Real GUI action path must refuse the stale source.
    assert_eq!(std::fs::read(&path).unwrap(), a_saved);
    assert!(!b.design_clean());
    assert_eq!(b.committed_design().unwrap(), b_edited);
    assert_eq!(a.committed_design().unwrap(), renamed);
    b.window().destroy();
    let a_after_close = a.committed_design().unwrap();
    a.open_design_path(&path);
    ready(&a);
    assert_eq!(a.committed_design().unwrap(), a_after_close);
    check(&a, "same-file-a-saved-reopened", Rgb::new(19, 29, 39));
    a.window().destroy();
}

#[test]
#[ignore = "isolated GTK: two GIF previews, coalesced edits, replacement and close discard stale work"]
fn theme_windows_keep_gif_and_pending_work_local() {
    fn finish(this: &Workbench) {
        let until = std::time::Instant::now() + Duration::from_secs(20);
        while this.greeting.presentation_pending() {
            assert!(std::time::Instant::now() < until);
            settle();
        }
        ready(this);
    }
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.TwoGifTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    present_with_preset(&app, None, root.path().join("font.json"));
    let a = controller(&app.active_window().unwrap());
    a.load_design(DesignDocument::new_theme(TargetHint::Kitty, "A", true));
    let mut greeting = crate::pixel_trial::tests::fixture(true).0;
    greeting.presentation.visual = crate::greeting_output::Visual::Animation;
    a.greeting.replace(greeting.clone(), true);
    ready(&a);
    a.open_design_window(DesignDocument::new_theme(TargetHint::Kitty, "B", true));
    let b = controller(&app.active_window().unwrap());
    b.greeting.replace(greeting, true);
    ready(&b);
    let until = std::time::Instant::now() + Duration::from_secs(20);
    while a.full_session.picture.paintable().is_none()
        || b.full_session.picture.paintable().is_none()
    {
        assert!(std::time::Instant::now() < until);
        settle();
    }
    a.full_session.play.set_active(false);
    b.full_session.play.set_active(true);
    settle();
    let paused = a.full_session.picture.paintable();
    let playing = b.full_session.picture.paintable();
    let until = std::time::Instant::now() + Duration::from_secs(3);
    while b.full_session.picture.paintable() == playing {
        assert!(
            std::time::Instant::now() < until,
            "B's GIF must animate independently"
        );
        settle();
    }
    assert_eq!(a.full_session.picture.paintable(), paused);
    let b_original = b.committed_design().unwrap();
    for columns in 25..41 {
        a.greeting.presentation.width.set_value(columns.into());
    }
    assert!(a.greeting.presentation_pending());
    finish(&a);
    assert_eq!(a.greeting.settings().presentation.columns, 40);
    assert_eq!(b.committed_design().unwrap(), b_original);
    let a_before_b_edit = a.committed_design().unwrap();
    b.greeting.presentation.width.set_value(53.0);
    finish(&b);
    assert_eq!(b.greeting.settings().presentation.columns, 53);
    assert_eq!(a.committed_design().unwrap(), a_before_b_edit);
    let b_after_edit = b.committed_design().unwrap();
    a.greeting.presentation.width.set_value(61.0);
    assert!(a.greeting.presentation_pending());
    a.load_design(DesignDocument::new_theme(
        TargetHint::Ptyxis,
        "Replacement",
        true,
    ));
    finish(&a);
    assert_eq!(a.typed.target.get(), Some(TargetHint::Ptyxis));
    assert!(a.greeting.settings().editable_artwork.is_none());
    assert!(a.full_session.picture.paintable().is_none());
    assert_eq!(b.committed_design().unwrap(), b_after_edit);
    b.greeting.presentation.width.set_value(65.0);
    assert!(b.greeting.presentation_pending());
    let unchanged = a.committed_design().unwrap();
    b.window().destroy();
    drop(b);
    for _ in 0..4 {
        settle();
    }
    assert_eq!(a.committed_design().unwrap(), unchanged);
    assert!(a.full_session.picture.paintable().is_none());
    a.window().destroy();
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
