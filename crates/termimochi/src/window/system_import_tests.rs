use super::*;
use crate::window::greeting::tests::{controller, descendants};
fn settle() {
    let end = std::time::Instant::now() + Duration::from_millis(150);
    while std::time::Instant::now() < end {
        glib::MainContext::default().iteration(false);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn until(predicate: impl Fn() -> bool) {
    let end = std::time::Instant::now() + Duration::from_secs(30);
    while !predicate() {
        assert!(std::time::Instant::now() < end);
        settle();
    }
    settle();
}
#[test]
#[ignore = "isolated GTK: read-only Ptyxis profiles and GUI detached Kitty import, source reports and save"]
fn system_import_profiles_and_detached_gui() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let global = crate::ptyxis::find_settings("org.gnome.Ptyxis", None).expect("test schema");
    global
        .set_strv("profile-uuids", ["qa-import-a", "qa-import-b"])
        .unwrap();
    global
        .set_string("default-profile-uuid", "qa-import-a")
        .unwrap();
    global.set_boolean("use-system-font", false).unwrap();
    global.set_string("font-name", "Monospace 15").unwrap();
    global.set_uint("default-columns", 97).unwrap();
    global.set_boolean("restore-window-size", false).unwrap();
    let dir = glib::user_data_dir().join("org.gnome.Ptyxis/palettes");
    std::fs::create_dir_all(&dir).unwrap();
    let palette = include_str!("../../resources/themes/fog-paper.palette");
    std::fs::write(dir.join("qa.palette"), palette).unwrap();
    let mut profiles = vec![];
    for (id, width) in [("qa-import-a", 1.1), ("qa-import-b", 1.2)] {
        let p = crate::ptyxis::find_settings(
            "org.gnome.Ptyxis.Profile",
            Some(&format!("/org/gnome/Ptyxis/Profiles/{id}/")),
        )
        .unwrap();
        p.set_string("palette", "qa").unwrap();
        p.set_double("cell-width-scale", width).unwrap();
        profiles.push(p);
    }
    let values = |s: &gio::Settings| {
        let mut keys = s.settings_schema().unwrap().list_keys();
        keys.sort();
        keys.iter()
            .map(|k| (k.to_string(), s.value(k).to_string()))
            .collect::<Vec<_>>()
    };
    let before = values(&global);
    let before_profiles = profiles.iter().map(values).collect::<Vec<_>>();
    let a = crate::system_import::ptyxis("qa-import-a").unwrap();
    let b = crate::system_import::ptyxis("qa-import-b").unwrap();
    assert_eq!(a.theme.as_ref().unwrap().typography["size"], 15.0);
    assert_eq!(a.theme.as_ref().unwrap().typography["cell_width"], 1.1);
    assert_eq!(b.theme.as_ref().unwrap().typography["cell_width"], 1.2);
    assert_eq!(b.theme.as_ref().unwrap().layout["columns"], 97);
    assert!(
        !b.theme
            .as_ref()
            .unwrap()
            .layout
            .contains_key("window_spacing")
    );
    assert!(
        !b.theme
            .as_ref()
            .unwrap()
            .layout
            .contains_key("content_padding")
    );
    assert!(!b.scope().prompt && !b.scope().greeting);
    assert!(b.components.palette.is_some());
    assert_eq!(values(&global), before);
    assert_eq!(
        profiles.iter().map(values).collect::<Vec<_>>(),
        before_profiles
    );
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("kitty.conf");
    let source = "background #234567\nfont_size 16\ninclude colors.conf\ngeninclude do-not-run\n";
    std::fs::write(&path, source).unwrap();
    std::fs::write(root.path().join("colors.conf"), "foreground #eeddcc\n").unwrap();
    // Optional candidates exist, but neither is evidence of target association.
    let config = glib::user_config_dir();
    std::fs::create_dir_all(config.join("fastfetch")).unwrap();
    let prompt_path = config.join("starship.toml");
    let greeting_path = config.join("fastfetch/config.jsonc");
    let prompt_source = "format = '$directory$character'\n";
    let greeting_source =
        r#"{"logo":{"type":"file","source":"logo.txt"},"modules":["os","shell"]}"#;
    std::fs::write(&prompt_path, prompt_source).unwrap();
    std::fs::write(&greeting_path, greeting_source).unwrap();
    std::fs::write(config.join("fastfetch/logo.txt"), "  /\\_/\\\n ( o.o )\n").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.ImportSnapshotTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    present_with_preset(&app, None, root.path().join("font.json"));
    let main = app.active_window().unwrap();
    let old = controller(&main);
    old.apply_color("Background", Rgb::new(52, 62, 72));
    old.full_session.samples.entry.set_text("未提交");
    let design = old.committed_design().unwrap();
    let state = old.full_session.samples.state.borrow().current().clone();
    old.review_system_source(Source::Kitty(path.clone()));
    let review = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.title().as_deref() == Some("From My Terminal"))
        .unwrap();
    let create = descendants(review.upcast_ref())
        .into_iter()
        .find(|w| w.widget_name() == "system-import-create")
        .unwrap()
        .downcast::<gtk::Button>()
        .unwrap();
    until(|| create.is_sensitive());
    let choices: Vec<_> = descendants(review.upcast_ref())
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::CheckButton>().ok())
        .collect();
    assert_eq!(choices.len(), 2);
    assert!(
        choices.iter().all(|c| !c.is_active()),
        "candidates must be opt-in"
    );
    crate::window::typed_tests::capture(&review, "system-import-review");
    create.emit_clicked();
    until(|| app.windows().len() == 2);
    let second = app.windows().into_iter().find(|w| w != &main).unwrap();
    let new = controller(&second);
    assert_eq!(old.committed_design().unwrap(), design);
    assert_eq!(*old.full_session.samples.state.borrow().current(), state);
    let imported = new.committed_design().unwrap();
    assert_ne!(imported.id, design.id);
    assert_eq!(
        imported.target_hint,
        Some(crate::design_document::TargetHint::Kitty)
    );
    assert!(!imported.scope().prompt && !imported.scope().greeting);
    assert!(new.full_session.active.get());
    assert_eq!(new.typography_settings().size, 16.0);
    let saved = root.path().join("snapshot.termimochi-design.json");
    *new.typed.store.borrow_mut() = Some(DocumentStore::open(saved.clone()).unwrap());
    new.save_design(false);
    assert_eq!(
        crate::document_store::decode::<DesignDocument>(&std::fs::read(&saved).unwrap()).unwrap(),
        imported
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(values(&global), before);
    assert_eq!(
        profiles.iter().map(values).collect::<Vec<_>>(),
        before_profiles
    );
    std::fs::write(
        glib::user_cache_dir().join("system-import-report.json"),
        serde_json::to_vec_pretty(&imported.theme.as_ref().unwrap().import_report).unwrap(),
    )
    .unwrap();
    std::fs::write(glib::user_cache_dir().join("no-write.json"),serde_json::to_vec_pretty(&serde_json::json!({"global_unchanged":values(&global)==before,"profiles_unchanged":profiles.iter().map(values).collect::<Vec<_>>()==before_profiles,"kitty_bytes_unchanged":std::fs::read_to_string(&path).unwrap()==source,"original_theme_unchanged":old.committed_design().unwrap()==design})).unwrap()).unwrap();
    crate::window::typed_tests::capture(&second, "system-import-new-theme");
    second.destroy();
    old.review_system_source(Source::Kitty(path.clone()));
    let review = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.title().as_deref() == Some("From My Terminal"))
        .unwrap();
    let create = descendants(review.upcast_ref())
        .into_iter()
        .find(|w| w.widget_name() == "system-import-create")
        .unwrap()
        .downcast::<gtk::Button>()
        .unwrap();
    until(|| create.is_sensitive());
    for check in descendants(review.upcast_ref())
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::CheckButton>().ok())
    {
        check.set_active(true);
    }
    crate::window::typed_tests::capture(&review, "system-import-opt-in");
    create.emit_clicked();
    until(|| app.windows().len() == 2);
    let second = app.windows().into_iter().find(|w| w != &main).unwrap();
    let combined = controller(&second).committed_design().unwrap();
    assert_eq!(combined.target_hint, imported.target_hint);
    assert_ne!(combined.id, imported.id);
    assert!(combined.theme.as_ref().unwrap().prompt_enabled);
    assert!(combined.scope().prompt && combined.scope().greeting);
    assert!(
        combined
            .components
            .greeting
            .as_ref()
            .unwrap()
            .source_logo
            .is_some()
    );
    assert_eq!(old.committed_design().unwrap(), design);
    assert_eq!(*old.full_session.samples.state.borrow().current(), state);
    assert_eq!(
        std::fs::read_to_string(&prompt_path).unwrap(),
        prompt_source
    );
    assert_eq!(
        std::fs::read_to_string(&greeting_path).unwrap(),
        greeting_source
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(values(&global), before);
    assert_eq!(
        profiles.iter().map(values).collect::<Vec<_>>(),
        before_profiles
    );
    std::fs::write(
        glib::user_cache_dir().join("optional-no-write.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"candidates_default_unchecked":true,
            "explicit_selection_attached_prompt_greeting":true,"source_asset_snapshotted":true,
            "source_files_unchanged":true,"original_theme_and_input_unchanged":true,
            "gsettings_unchanged":true}))
        .unwrap(),
    )
    .unwrap();
    second.destroy();
    main.destroy();
}
