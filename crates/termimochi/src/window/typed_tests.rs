//! Native current-document scope checks. Opt in only with private HOME/XDG,
//! memory GSettings, a private D-Bus and a dedicated Xvfb display.
use super::greeting::tests::{controller, descendants, feed, settle};
use super::*;
use crate::design_document::{DesignDocument, Kind, Scope, TargetHint};
use std::collections::BTreeMap;

fn application(suffix: &str) -> adw::Application {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    assert!(std::env::var_os("XDG_CONFIG_HOME").is_some());
    assert!(std::env::var_os("XDG_DATA_HOME").is_some());
    let display = std::env::var("DISPLAY").unwrap();
    assert!(!matches!(display.as_str(), ":0" | ":1" | ":0.0" | ":1.0"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id(format!("io.github.miiikuuu.termimochi.Typed{suffix}"))
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    app
}

fn open_window(app: &adw::Application, root: &Path) -> (gtk::Window, Rc<Workbench>) {
    present_advanced_with_preset(app, None, root.join(typography_preset::PRESET_NAME));
    let window = app.active_window().unwrap();
    let this = controller(&window);
    ready(&this);
    // These historical tests exercise the retained advanced single-component
    // contract; normal-theme behavior has separate regression cases below.
    this.typed.advanced.set(true);
    this.load_design(
        DesignDocument::from_workspace(
            Kind::Palette,
            Scope::for_kind(Kind::Palette),
            &this.workspace_snapshot(),
            None,
        )
        .unwrap(),
    );
    (window, this)
}

fn ready(this: &Workbench) {
    let deadline = std::time::Instant::now() + Duration::from_secs(25);
    while this.preview_loading.get() || this.copy_loading.get() {
        assert!(
            std::time::Instant::now() < deadline,
            "typed preview did not settle"
        );
        settle();
    }
    settle();
}

pub(super) fn capture(window: &gtk::Window, name: &str) {
    window.set_title(Some("TermiMochi point-to-edit test"));
    let path = glib::user_cache_dir().join(format!("{name}.png"));
    gtk::prelude::WidgetExt::display(window).flush();
    assert!(
        std::process::Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/preview-pointer-driver.py"
            ))
            .args(["capture", "0", "0"])
            .env("TERMIMOCHI_INSPECT_SCREENSHOT", &path)
            .status()
            .unwrap()
            .success()
    );
    assert!(std::fs::metadata(&path).unwrap().len() > 1024);
    println!("Native typed document screenshot: {}", path.display());
}

fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        if !path.exists() {
            return;
        }
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, result);
            } else if path.is_file() {
                result.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

fn save_without_chooser(this: &Rc<Workbench>, path: &Path) {
    *this.typed.store.borrow_mut() =
        Some(DocumentStore::<DesignDocument>::open(path.to_owned()).unwrap());
    // Same registered GAction as Ctrl+S; no native-source write permission.
    gio::prelude::ActionGroupExt::activate_action(&this.window(), "save", None);
    settle();
    assert!(
        path.is_file(),
        "Save must produce the current design document"
    );
    assert!(
        this.design_clean(),
        "successful Save must establish a stable content baseline"
    );
}

#[test]
#[ignore = "isolated GTK: Palette action boundaries, reference Inspect and design-only Save"]
fn typed_palette_actions_and_save_exclude_reference_write_sets() {
    let app = application("PaletteScopeTest");
    let root = tempfile::tempdir().unwrap();
    let (window, this) = open_window(&app, root.path());
    assert_eq!(this.typed.kind.get(), Kind::Palette);
    assert_eq!(this.typed.scope.get(), Scope::for_kind(Kind::Palette));
    assert!(this.palette_module_button.is_visible());
    assert!(!this.typography_module_button.is_visible());
    assert!(!this.layout_module_button.is_visible());
    assert!(!this.prompt_module_button.is_visible());
    assert!(!this.greeting_module_button.is_visible());
    let config_before = tree(&glib::user_config_dir());
    let data_before = tree(&glib::user_data_dir());
    let before = this.design_snapshot().unwrap();
    for action in [
        "apply-fastfetch",
        "apply-typography",
        "apply-layout",
        "export-starship",
        "show-greeting",
        "show-prompt",
    ] {
        assert!(
            !this.allows_document_action(action),
            "business guard: {action}"
        );
        assert!(
            !this.require_document_action(action),
            "direct boundary: {action}"
        );
        let registered = this.window().lookup_action(action).unwrap();
        assert!(
            !registered.is_enabled(),
            "registered action must reject {action}"
        );
        gio::prelude::ActionGroupExt::activate_action(&this.window(), action, None);
    }
    for target in [
        PreviewTarget::Typography,
        PreviewTarget::PromptCopy,
        PreviewTarget::GreetingFields,
        PreviewTarget::Padding,
    ] {
        this.inspect_preview_target(target);
        assert_eq!(this.output_module(), EditorModule::Palette);
    }
    assert_eq!(this.design_snapshot().unwrap(), before);
    // A preview bridge can contain rich reference content without acquiring it.
    let mut reference_greeting = this.greeting.settings();
    reference_greeting.enabled = true;
    reference_greeting.logo = crate::greeting::Logo::Custom;
    reference_greeting.custom_logo = "REFERENCE ONLY".into();
    reference_greeting.message = "REFERENCE MUST STAY UNSAVED".into();
    this.greeting.replace(reference_greeting, false);
    this.select_preview_scene(preview_scene::PreviewScene::Full);
    ready(&this);
    assert!(
        feed(&this).contains("REFERENCE ONLY"),
        "Full must retain reference rendering"
    );
    assert_eq!(this.design_snapshot().unwrap(), before);
    let plan = this.scheme_plan(&this.workspace_snapshot());
    assert!(!plan.items.is_empty());
    assert!(
        plan.items
            .iter()
            .all(|item| matches!(item.id, "palette" | "activate")),
        "Palette plan leaked reference components"
    );
    let path = root.path().join("only-colors.termimochi-design.json");
    save_without_chooser(&this, &path);
    let bytes = std::fs::read(&path).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let components = saved["components"].as_object().unwrap();
    assert_eq!(components.len(), 1);
    assert!(components.contains_key("palette"));
    assert!(!String::from_utf8_lossy(&bytes).contains("REFERENCE MUST STAY UNSAVED"));
    assert_eq!(config_before, tree(&glib::user_config_dir()));
    assert_eq!(data_before, tree(&glib::user_data_dir()));
    for (width, height) in [(1280, 900), (1024, 700)] {
        window.set_default_size(width, height);
        ready(&this);
        assert!(this.preview_terminal.is_mapped());
        assert!(this.preview_terminal.column_count() >= 12);
        let bounds = this.palette_module_button.compute_bounds(&window).unwrap();
        assert!(bounds.x() >= 0.0 && bounds.y() >= 0.0);
        let use_design = descendants(this.output_bar.root.upcast_ref())
            .into_iter()
            .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
            .find(|button| button.action_name().as_deref() == Some("win.apply-scheme"))
            .expect("Palette must retain its main Use Design entry");
        for widget in [
            use_design.upcast_ref::<gtk::Widget>(),
            this.output_bar.more_button().upcast_ref::<gtk::Widget>(),
        ] {
            assert!(widget.is_mapped() && widget.is_visible());
            let bounds = widget.compute_bounds(&window).unwrap();
            assert!(bounds.width() > 10.0 && bounds.height() > 10.0);
            assert!(bounds.x() >= 0.0 && bounds.y() >= 0.0);
            assert!(bounds.x() + bounds.width() <= window.width() as f32);
            assert!(bounds.y() + bounds.height() <= window.height() as f32);
        }
        capture(&window, &format!("typed-palette-{width}x{height}"));
    }
    window.destroy();
}

#[test]
#[ignore = "isolated GTK: native Starship/Fastfetch source retention, scoped Save and legacy migration"]
fn typed_native_documents_save_reopen_without_cross_document_writes() {
    let app = application("NativeRoundTripTest");
    let root = tempfile::tempdir().unwrap();
    let prompt = "# untouched native comment\nformat = '$directory$character'\n[character]\nsuccess_symbol = '[OK>](green)'\n";
    let greeting = "{\n // untouched native comment\n \"logo\": {\"type\": \"none\"},\n \"modules\": [\"title\", \"separator\", \"os\", \"os\"]\n}\n";
    let prompt_path = root.path().join("starship.toml");
    let greeting_path = root.path().join("fastfetch.jsonc");
    std::fs::write(&prompt_path, prompt).unwrap();
    std::fs::write(&greeting_path, greeting).unwrap();
    let (window, this) = open_window(&app, root.path());
    let config_before = tree(&glib::user_config_dir());
    let data_before = tree(&glib::user_data_dir());
    let initial_identity = this.typed.identity.get();
    this.open_design_path(&prompt_path);
    ready(&this);
    assert_eq!(this.typed.kind.get(), Kind::Prompt);
    assert_ne!(this.typed.identity.get(), initial_identity);
    assert_eq!(this.typed.scope.get(), Scope::for_kind(Kind::Prompt));
    assert_eq!(this.typed.native.borrow().as_ref().unwrap().text, prompt);
    assert_eq!(
        this.design_snapshot()
            .unwrap()
            .components
            .prompt
            .unwrap()
            .starship
            .as_deref(),
        Some(prompt)
    );
    assert!(!this.allows_document_action("apply-fastfetch"));
    assert!(!this.allows_document_action("install-ptyxis"));
    let saved_prompt = root.path().join("prompt.termimochi-design.json");
    save_without_chooser(&this, &saved_prompt);
    let saved_prompt_bytes = std::fs::read(&saved_prompt).unwrap();
    let prompt_identity = this.typed.identity.get();
    this.open_design_path(&greeting_path);
    ready(&this);
    assert_eq!(this.typed.kind.get(), Kind::Greeting);
    assert_ne!(this.typed.identity.get(), prompt_identity);
    assert_eq!(this.typed.scope.get(), Scope::for_kind(Kind::Greeting));
    assert_eq!(this.typed.native.borrow().as_ref().unwrap().text, greeting);
    assert_eq!(
        this.greeting.settings().imported_source.as_deref(),
        Some(greeting)
    );
    assert!(!this.allows_document_action("save-starship"));
    assert!(!this.allows_document_action("apply-typography"));
    assert!(this.greeting.presentation.verified.borrow().is_none());
    let saved_greeting = root.path().join("greeting.termimochi-design.json");
    save_without_chooser(&this, &saved_greeting);
    let saved_greeting_bytes = std::fs::read(&saved_greeting).unwrap();
    assert_eq!(std::fs::read(&saved_prompt).unwrap(), saved_prompt_bytes);
    this.choose_document_use_target();
    settle();
    let use_widgets: Vec<_> = gtk::Window::list_toplevels()
        .into_iter()
        .flat_map(|window| descendants(&window))
        .collect();
    let terminal = use_widgets
        .iter()
        .find(|widget| widget.widget_name() == "document-use-terminal")
        .unwrap()
        .clone()
        .downcast::<gtk::DropDown>()
        .unwrap();
    let trial = use_widgets
        .iter()
        .find(|widget| widget.widget_name() == "document-use-trial")
        .unwrap()
        .clone()
        .downcast::<gtk::Button>()
        .unwrap();
    for (index, expected) in [
        (1, crate::pixel_trial::Terminal::Kitty),
        (0, crate::pixel_trial::Terminal::Ptyxis),
        (2, crate::pixel_trial::Terminal::Xterm),
    ] {
        terminal.set_selected(index);
        assert_eq!(
            terminal
                .selected_item()
                .unwrap()
                .downcast::<gtk::StringObject>()
                .unwrap()
                .string()
                .as_str(),
            expected.label()
        );
        trial.emit_clicked();
        settle();
        assert_eq!(
            this.greeting.presentation.binding.borrow().terminal,
            expected,
            "Use flow must launch the terminal actually selected by name"
        );
        if let Some(trial_window) = this.greeting.pixel_export_window.upgrade() {
            trial_window.destroy();
        }
        settle();
    }
    for dialog in gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Window>().ok())
        .filter(|dialog| dialog != &window)
    {
        dialog.destroy();
    }
    this.open_design_path(&saved_prompt);
    ready(&this);
    assert_eq!(this.typed.kind.get(), Kind::Prompt);
    assert!(this.design_clean());
    assert_eq!(
        this.design_snapshot()
            .unwrap()
            .components
            .prompt
            .unwrap()
            .starship
            .as_deref(),
        Some(prompt)
    );
    assert_eq!(
        std::fs::read(&saved_greeting).unwrap(),
        saved_greeting_bytes
    );
    assert_eq!(std::fs::read_to_string(&prompt_path).unwrap(), prompt);
    assert_eq!(std::fs::read_to_string(&greeting_path).unwrap(), greeting);
    assert_eq!(config_before, tree(&glib::user_config_dir()));
    assert_eq!(data_before, tree(&glib::user_data_dir()));
    // Legacy opening is a data migration, not a grant to target Kitty or write.
    let legacy = this.workspace_snapshot();
    let legacy_bytes = document_store::encode(&legacy).unwrap();
    let legacy_path = root.path().join("legacy.termimochi.json");
    std::fs::write(&legacy_path, &legacy_bytes).unwrap();
    this.open_design_path(&legacy_path);
    ready(&this);
    assert_eq!(this.typed.kind.get(), Kind::Legacy);
    for action in [
        "install-ptyxis",
        "apply-typography",
        "apply-layout",
        "save-starship",
        "apply-fastfetch",
        "greeting-startup",
    ] {
        assert!(
            !this.allows_document_action(action),
            "Legacy {action} must require an explicit converted document"
        );
        assert!(!this.window().lookup_action(action).unwrap().is_enabled());
    }
    assert_eq!(this.typed.scope.get(), Scope::for_kind(Kind::Legacy));
    assert_ne!(this.typed.target.get(), Some(TargetHint::Kitty));
    assert_eq!(this.workspace_snapshot(), legacy);
    assert_eq!(std::fs::read(&legacy_path).unwrap(), legacy_bytes);
    assert!(this.greeting.presentation.verified.borrow().is_none());
    assert_eq!(config_before, tree(&glib::user_config_dir()));
    assert_eq!(data_before, tree(&glib::user_data_dir()));
    // Independent presets must reach a target-specific review through the GUI,
    // even when the isolated machine cannot connect to a Ptyxis profile bus.
    for kind in [Kind::Typography, Kind::Layout] {
        let design = DesignDocument::from_workspace(
            kind,
            Scope::for_kind(kind),
            &this.workspace_snapshot(),
            None,
        )
        .unwrap();
        this.load_design(design);
        assert!(this.typed.target.get().is_none());
        this.choose_document_use_target();
        settle();
        let ptyxis = gtk::Window::list_toplevels()
            .into_iter()
            .flat_map(|window| descendants(&window))
            .find(|widget| widget.widget_name() == "document-use-ptyxis")
            .unwrap()
            .downcast::<gtk::Button>()
            .unwrap();
        ptyxis.emit_clicked();
        settle();
        assert_eq!(this.typed.target.get(), Some(TargetHint::Ptyxis));
        let plan = this.scheme_plan(&this.workspace_snapshot());
        assert!(!plan.items.is_empty());
        assert!(plan.items.iter().all(|item| match kind {
            Kind::Typography => item.id == "typography",
            Kind::Layout => matches!(item.id, "layout" | "preview-only"),
            _ => false,
        }));
        assert_eq!(config_before, tree(&glib::user_config_dir()));
        assert_eq!(data_before, tree(&glib::user_data_dir()));
        for dialog in gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|widget| widget.downcast::<gtk::Window>().ok())
            .filter(|dialog| dialog != &window)
        {
            dialog.destroy();
        }
    }
    window.destroy();
}

#[test]
#[ignore = "isolated GTK: native Kitty appearance never acquires Prompt/Greeting or Ptyxis actions"]
fn typed_plain_kitty_is_not_an_implicit_project() {
    let app = application("KittyAppearanceTest");
    let root = tempfile::tempdir().unwrap();
    let source =
        "# independent native appearance\nbackground #18232a\nforeground #dce7ee\nfont_size 13.0\n";
    let path = root.path().join("kitty.conf");
    std::fs::write(&path, source).unwrap();
    let (window, this) = open_window(&app, root.path());
    this.open_design_path(&path);
    ready(&this);
    assert_eq!(this.typed.kind.get(), Kind::KittyAppearance);
    assert_eq!(this.typed.target.get(), Some(TargetHint::Kitty));
    assert!(!this.owns_module(EditorModule::Prompt));
    assert!(!this.owns_module(EditorModule::Greeting));
    assert!(!this.prompt_module_button.is_visible());
    assert!(!this.greeting_module_button.is_visible());
    for action in [
        "install-ptyxis",
        "apply-typography",
        "apply-layout",
        "apply-fastfetch",
        "save-starship",
    ] {
        assert!(
            !this.allows_document_action(action),
            "Kitty must not route {action} to Ptyxis/shared config"
        );
    }
    let snapshot = this.design_snapshot().unwrap();
    assert!(snapshot.components.prompt.is_none());
    assert!(snapshot.components.greeting.is_none());
    assert_eq!(snapshot.native.as_ref().unwrap().text, source);
    let saved = root.path().join("kitty-appearance.termimochi-design.json");
    save_without_chooser(&this, &saved);
    this.open_design_path(&saved);
    ready(&this);
    assert_eq!(this.design_snapshot().unwrap(), snapshot);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    for (width, height) in [(1280, 900), (1024, 700)] {
        window.set_default_size(width, height);
        ready(&this);
        assert!(this.preview_terminal.is_mapped());
        capture(&window, &format!("typed-kitty-appearance-{width}x{height}"));
    }
    window.destroy();
}
