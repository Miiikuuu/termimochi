use super::*;
use crate::window::greeting::tests::{controller, feed, respond, settle, wait_official};
use std::os::unix::fs::PermissionsExt;

fn wait_sync(this: &Workbench) {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while this.greeting.sync.pending.get() {
        assert!(std::time::Instant::now() < deadline, "sync did not settle");
        settle();
    }
}

fn saved(path: &Path) -> GreetingSettings {
    DocumentStore::<GreetingPreset>::open(path.to_owned())
        .unwrap()
        .document()
        .unwrap()
        .unwrap()
        .greeting
}

#[test]
#[ignore = "requires GTK/VTE and Fastfetch; isolated temporary configs, no real terminal launch"]
fn greeting_sync_load_apply_restart_conflicts_and_draft_protection() {
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
        .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.GreetingSyncTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    let preset = root.path().join(typography_preset::PRESET_NAME);
    let greeting_path = root.path().join(PRESET_NAME);
    let state = root.path().join("fastfetch-state");
    let external = root.path().join("external/config.jsonc");
    let mut old = GreetingSettings::starter();
    old.import_artwork(
        crate::greeting_art::Artwork::parse("\x1b[38;2;255;255;255m████\x1b[0m\n████").unwrap(),
    )
    .unwrap();
    old.official_preset = None;
    old.official_items.clear();
    DocumentStore::<GreetingPreset>::open(greeting_path.clone())
        .unwrap()
        .save(&GreetingPreset::new(old.clone()))
        .unwrap();
    let mut transparent = old.clone();
    transparent
        .import_artwork(
            crate::greeting_art::Artwork::parse("    \n\x1b[38;2;12;34;56m ▀▀ \x1b[0m").unwrap(),
        )
        .unwrap();
    let source = transparent.fastfetch_config().unwrap();
    let mut target = fastfetch_apply::Target::open(external.clone()).unwrap();
    fastfetch_apply::apply(&mut target, &source, &state).unwrap();
    let before_local = std::fs::read(&greeting_path).unwrap();
    present_with_preset(&app, None, preset.clone());
    let window = app.active_window().unwrap();
    let this = controller(&window);
    gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-greeting", None);
    this.preview_scene_selector.set_selected(3);
    wait_sync(&this);
    assert!(
        this.greeting.sync.difference.is_visible(),
        "message={} tooltip={:?}, observed={:?}, root={}, row-property={}",
        this.greeting.sync.message.text(),
        this.greeting.sync.difference.tooltip_text(),
        this.greeting.sync.observed.borrow(),
        this.greeting.sync.root.is_visible(),
        this.greeting.sync.difference.property::<bool>("visible")
    );
    assert!(this.greeting.sync.load.is_visible());
    assert_eq!(
        *this.greeting.sync.observed.borrow(),
        Some(external.clone())
    );
    assert!(this.greeting.fastfetch_target.borrow().is_none());
    assert_eq!(
        this.greeting.settings(),
        old,
        "startup must not replace a draft"
    );
    assert_eq!(std::fs::read(&greeting_path).unwrap(), before_local);

    this.greeting.sync.load.emit_clicked();
    wait_official(&this);
    wait_sync(&this);
    let loaded = this.greeting.settings();
    assert!(matches(&loaded, &source).unwrap());
    assert_eq!(saved(&greeting_path), loaded);
    assert!(!this.greeting.dirty());
    assert!(!this.greeting.sync.root.is_visible());
    assert!(!feed(&this).contains("38;2;255;255;255"));
    assert!(feed(&this).contains("38;2;12;34;56"));
    assert_eq!(std::fs::read_to_string(&external).unwrap(), source);
    this.greeting.undo();
    assert_eq!(this.greeting.settings(), old);
    this.load_applied_fastfetch_path(external.clone());
    respond("Cancel");
    assert_eq!(this.greeting.settings(), old);
    assert_eq!(saved(&greeting_path), loaded);
    this.load_applied_fastfetch_path(external.clone());
    std::fs::write(&external, "// changed during confirmation\n{}").unwrap();
    respond("Replace Changes");
    assert_eq!(this.greeting.settings(), old);
    assert_eq!(saved(&greeting_path), loaded);
    std::fs::write(&external, &source).unwrap();
    this.load_applied_fastfetch_path(external.clone());
    let mut edited = old.clone();
    edited.message = "Changed during confirmation".into();
    this.greeting.replace(edited.clone(), true);
    respond("Replace Changes");
    assert_eq!(this.greeting.settings(), edited);
    assert_eq!(saved(&greeting_path), loaded);

    this.load_applied_fastfetch_path(external.clone());
    respond("Replace Changes");
    let loaded = this.greeting.settings();
    assert_eq!(saved(&greeting_path), loaded);
    window.destroy();
    drop(this);
    settle();
    present_with_preset(&app, None, preset.clone());
    let window = app.active_window().unwrap();
    let this = controller(&window);
    gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-greeting", None);
    this.preview_scene_selector.set_selected(3);
    wait_sync(&this);
    assert_eq!(this.greeting.settings(), loaded);
    assert!(!this.greeting.sync.root.is_visible());
    assert!(this.greeting.fastfetch_target.borrow().is_none());

    // Apply without Save Preset; cancellation and destination conflicts must
    // not persist the draft or request a terminal launch.
    transparent.message = "Auto-saved after apply".into();
    this.greeting.replace(transparent.clone(), true);
    crate::fastfetch_run::LAUNCHES.with(|runs| runs.borrow_mut().clear());
    this.request_fastfetch_apply();
    respond("Cancel");
    assert_eq!(saved(&greeting_path), loaded);
    assert!(crate::fastfetch_run::LAUNCHES.with(|runs| runs.borrow().is_empty()));
    this.request_fastfetch_apply();
    std::fs::write(&external, "// external conflict\n{}").unwrap();
    super::super::tests::select_scheme_greeting();
    respond("Back Up & Apply Selected");
    respond("Close");
    assert_eq!(saved(&greeting_path), loaded);
    assert_eq!(
        std::fs::read_to_string(&external).unwrap(),
        "// external conflict\n{}"
    );
    assert!(crate::fastfetch_run::LAUNCHES.with(|runs| runs.borrow().is_empty()));
    std::fs::write(&external, &source).unwrap();
    this.request_fastfetch_apply();
    super::super::tests::select_scheme_greeting();
    respond("Back Up & Apply Selected");
    respond("Close");
    wait_sync(&this);
    assert_eq!(saved(&greeting_path), transparent);
    assert!(!this.greeting.dirty());
    assert!(!this.greeting.sync.root.is_visible());
    assert_eq!(
        crate::fastfetch_run::LAUNCHES.with(|runs| runs.borrow().clone()),
        Vec::<PathBuf>::new() // Running the complete config now needs its own result-page confirmation.
    );

    // External success + local read-only failure is visible, keeps the draft
    // and does not advance the baseline. Retry still uses checked persistence.
    let baseline = std::fs::read(&greeting_path).unwrap();
    let mut next = transparent.clone();
    next.message = "External success, local failure".into();
    this.greeting.replace(next.clone(), true);
    std::fs::set_permissions(&greeting_path, std::fs::Permissions::from_mode(0o400)).unwrap();
    this.request_fastfetch_apply();
    super::super::tests::select_scheme_greeting();
    respond("Back Up & Apply Selected");
    respond("Close");
    wait_sync(&this);
    assert!(this.greeting.sync.save_failure.is_visible());
    assert!(this.greeting.sync.root.is_visible());
    assert!(this.greeting.dirty());
    assert_eq!(this.greeting.settings(), next);
    assert_eq!(std::fs::read(&greeting_path).unwrap(), baseline);
    assert!(matches(&next, &std::fs::read_to_string(&external).unwrap()).unwrap());
    this.save_greeting_preset();
    assert!(this.greeting.sync.save_failure.is_visible());
    std::fs::set_permissions(&greeting_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    this.save_greeting_preset();
    assert_eq!(saved(&greeting_path), next);
    assert!(!this.greeting.sync.save_failure.is_visible());

    let baseline = std::fs::read(&greeting_path).unwrap();
    let mut last = next.clone();
    last.message = "Do not overwrite an externally edited preset".into();
    this.greeting.replace(last.clone(), true);
    let other = crate::document_store::encode(&GreetingPreset::new(old)).unwrap();
    std::fs::write(&greeting_path, &other).unwrap();
    this.request_fastfetch_apply();
    super::super::tests::select_scheme_greeting();
    respond("Back Up & Apply Selected");
    respond("Close");
    assert!(this.greeting.sync.save_failure.is_visible());
    assert_eq!(std::fs::read(&greeting_path).unwrap(), other);
    this.save_greeting_preset();
    assert_eq!(std::fs::read(&greeting_path).unwrap(), other);
    // Test-only resolution: restore precisely the bytes our store expects.
    std::fs::write(&greeting_path, &baseline).unwrap();
    this.save_greeting_preset();
    assert_eq!(saved(&greeting_path), last);

    // An unchanged Fastfetch file still commits preview-only preset changes.
    let before = std::fs::read(&external).unwrap();
    last.opening = Opening::ALL
        .into_iter()
        .find(|v| *v != last.opening)
        .unwrap();
    this.greeting.replace(last.clone(), true);
    this.request_fastfetch_apply();
    super::super::tests::select_scheme_greeting();
    respond("Back Up & Apply Selected");
    respond("Close");
    assert_eq!(std::fs::read(&external).unwrap(), before);
    assert_eq!(saved(&greeting_path), last);
    window.destroy();
    drop(this);
    settle();
    present_with_preset(&app, None, preset);
    let window = app.active_window().unwrap();
    let this = controller(&window);
    gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-greeting", None);
    this.preview_scene_selector.set_selected(3);
    wait_sync(&this);
    assert_eq!(this.greeting.settings(), last);
    assert!(!this.greeting.sync.root.is_visible());
    for index in 0..12 {
        let mut draft = last.clone();
        draft.message = format!("Coalesced draft {index}");
        this.greeting.replace(draft, true);
    }
    this.greeting.replace(last.clone(), true);
    wait_sync(&this);
    assert!(
        !this.greeting.sync.root.is_visible(),
        "stale checks must not re-show a mismatch"
    );
    std::fs::write(&external, "{broken").unwrap();
    this.schedule_fastfetch_sync();
    wait_sync(&this);
    assert!(this.greeting.sync.root.is_visible());
    assert!(!this.greeting.sync.load.get_visible());
    assert_eq!(this.greeting.settings(), last);
    assert_eq!(saved(&greeting_path), last);
    window.destroy();
}

#[test]
#[ignore = "requires GTK/VTE and Fastfetch; temporary fixtures or optional read-only reference paths"]
fn real_applied_artwork_replaces_stale_preview() {
    let references = tempfile::tempdir().unwrap();
    let (preset_source, config_source) = match (
        std::env::var_os("TERMIMOCHI_SYNC_TEST_PRESET"),
        std::env::var_os("TERMIMOCHI_SYNC_TEST_CONFIG"),
    ) {
        (Some(preset), Some(config)) => (PathBuf::from(preset), PathBuf::from(config)),
        (None, None) => {
            let preset = references.path().join(PRESET_NAME);
            let config = references.path().join("config.jsonc");
            let mut settings = GreetingSettings {
                enabled: true,
                ..Default::default()
            };
            settings
                .import_artwork(
                    crate::greeting_art::Artwork::parse("\x1b[47m    \n ██ \x1b[0m").unwrap(),
                )
                .unwrap();
            DocumentStore::<GreetingPreset>::open(preset.clone())
                .unwrap()
                .save(&GreetingPreset::new(settings.clone()))
                .unwrap();
            settings
                .import_artwork(
                    crate::greeting_art::Artwork::parse("    \n\x1b[38;2;12;34;56m ▀▀ \x1b[0m")
                        .unwrap(),
                )
                .unwrap();
            std::fs::write(&config, settings.fastfetch_config().unwrap()).unwrap();
            (preset, config)
        }
        _ => panic!("Provide both reference paths, or neither for isolated generated fixtures"),
    };
    let reference_preset = std::fs::read(&preset_source).unwrap();
    let reference_config = std::fs::read(&config_source).unwrap();
    let old = saved(&preset_source);
    let source = std::str::from_utf8(&reference_config).unwrap();
    assert!(!matches(&old, source).unwrap());
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
        .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.AppliedArtworkTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    let preset = root.path().join(typography_preset::PRESET_NAME);
    let greeting_path = root.path().join(PRESET_NAME);
    DocumentStore::<GreetingPreset>::open(greeting_path.clone())
        .unwrap()
        .save(&GreetingPreset::new(old.clone()))
        .unwrap();
    let target = root.path().join("config.jsonc");
    fastfetch_apply::apply(
        &mut fastfetch_apply::Target::open(target.clone()).unwrap(),
        source,
        &root.path().join("fastfetch-state"),
    )
    .unwrap();
    present_with_preset(&app, None, preset.clone());
    let window = app.active_window().unwrap();
    window.set_default_size(1320, 850);
    let this = controller(&window);
    gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-greeting", None);
    this.preview_scene_selector.set_selected(3);
    wait_sync(&this);
    assert!(this.greeting.sync.difference.is_visible());
    if let Some(path) = std::env::var_os("TERMIMOCHI_SYNC_NOTICE_SCREENSHOT") {
        capture(&window, path);
    }
    this.greeting.sync.load.emit_clicked();
    wait_official(&this);
    wait_sync(&this);
    assert!(!this.greeting.sync.root.is_visible());
    let value = fastfetch_document::value(source).unwrap();
    let artwork =
        crate::greeting_art::Artwork::parse(value["logo"]["source"].as_str().unwrap()).unwrap();
    assert!(artwork.plain.lines().next().unwrap().trim().is_empty());
    let normalized = |text: &str| {
        text.lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end()
            .to_owned()
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        let text = this
            .greeting
            .artwork_preview
            .terminal
            .text_format(vte::Format::Text)
            .unwrap_or_default();
        if normalized(&text) == normalized(&artwork.plain) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "fitted thumbnail differs from applied ANSI"
        );
        settle();
    }
    if let Some(path) = std::env::var_os("TERMIMOCHI_SYNC_LOADED_SCREENSHOT") {
        capture(&window, path);
    }
    // The character's face may legitimately be white. Check the removed
    // background cells, not the absence of white anywhere in the artwork.
    let visible = this
        .preview_terminal
        .text_format(vte::Format::Text)
        .unwrap();
    let width = artwork.plain.lines().next().unwrap().chars().count();
    assert!(
        visible
            .lines()
            .next()
            .unwrap()
            .chars()
            .take(width)
            .all(char::is_whitespace),
        "removed top background must remain blank in Live Preview"
    );
    let loaded = this.greeting.settings();
    assert_eq!(saved(&greeting_path), loaded);
    window.destroy();
    drop(this);
    settle();
    present_with_preset(&app, None, preset);
    let window = app.active_window().unwrap();
    let this = controller(&window);
    wait_sync(&this);
    assert_eq!(this.greeting.settings(), loaded);
    assert!(!this.greeting.sync.difference.get_visible());
    assert_eq!(std::fs::read(&preset_source).unwrap(), reference_preset);
    assert_eq!(std::fs::read(&config_source).unwrap(), reference_config);
    window.destroy();
}

fn capture(window: &gtk::Window, path: std::ffi::OsString) {
    window.set_title(Some("TermiMochi point-to-edit test"));
    settle();
    let mut child = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/preview-pointer-driver.py"
        ))
        .args(["capture", "0", "0"])
        .env("TERMIMOCHI_INSPECT_SCREENSHOT", path)
        .spawn()
        .unwrap();
    while child.try_wait().unwrap().is_none() {
        settle();
    }
    assert!(child.wait().unwrap().success());
}
