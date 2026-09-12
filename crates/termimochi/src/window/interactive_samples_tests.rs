use super::*;
use crate::window::greeting::tests::{controller, feed};

fn settle() {
    let until = std::time::Instant::now() + Duration::from_millis(300);
    while std::time::Instant::now() < until {
        glib::MainContext::default().iteration(false);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn ready(this: &Workbench) {
    let until = std::time::Instant::now() + Duration::from_secs(30);
    while this.preview_loading.get()
        || this.copy_loading.get()
        || this.greeting_official_loading.get()
    {
        assert!(std::time::Instant::now() < until);
        settle();
    }
    settle();
}
fn command(this: &Workbench, input: &str) {
    let entry = &this.full_session.samples.entry;
    entry.set_text(input);
    entry.emit_activate();
    settle();
}
fn capture(window: &gtk::Window, name: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let node = loop {
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(window)).snapshot(
            &snapshot,
            window.width() as f64,
            window.height() as f64,
        );
        if let Some(node) = snapshot.to_node() {
            break node;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "window did not produce a frame after resize"
        );
        settle();
    };
    let texture = window.renderer().unwrap().render_texture(node, None);
    let path = glib::user_cache_dir().join(format!("{name}.png"));
    texture.save_to_png(&path).unwrap();
    println!("Screenshot: {}", path.display());
}
#[test]
#[ignore = "isolated GTK: default Full independent tabs, input, palette, font, save and finite-action safety"]
fn interactive_samples_vertical_workbench() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.SampleTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("sample.conf");
    std::fs::write(&path, "background #152536\nforeground #eeeeee\n").unwrap();
    present_with_preset(&app, Some(path), root.path().join("unused-font.json"));
    let window = app.active_window().unwrap();
    let this = controller(&window);
    println!("samples: window constructed");
    ready(&this);
    assert!(this.full_session.active.get());
    let s = &this.full_session.samples;
    assert!(s.entry.is_mapped());
    assert!(this.preview_terminal.pty().is_none());
    let design = this.design_snapshot().unwrap();
    command(&this, "help");
    assert!(feed(&this).contains("no local commands"));
    command(&this, "git diff");
    assert!(feed(&this).contains("diff --git"));
    command(&this, "cd src");
    s.entry.set_text("未提交 👩‍💻");
    let first = s.state.borrow().current().clone();
    s.new_tab.emit_clicked();
    command(&this, "cd /");
    command(&this, "clear");
    assert!(s.state.borrow().current().blocks.is_empty());
    assert_eq!(s.state.borrow().tabs[0].draft, first.draft);
    assert_eq!(s.state.borrow().tabs[0].blocks, first.blocks);
    assert_eq!(s.state.borrow().tabs[0].directory, first.directory);
    assert_eq!(s.state.borrow().tabs[0].history, first.history);
    s.selector.set_selected(0);
    settle();
    assert_eq!(s.entry.text(), "未提交 👩‍💻");
    let before = s.state.borrow().current().clone();
    let transcript = this.full_session.rendered.borrow().clone();
    this.apply_color("Background", Rgb::new(35, 45, 55));
    settle();
    assert_eq!(*this.full_session.rendered.borrow(), transcript);
    this.font_size_input.set_value(15.0);
    settle();
    assert_eq!(s.state.borrow().current().draft, before.draft);
    assert_eq!(s.state.borrow().current().history, before.history);
    assert_eq!(s.state.borrow().current().blocks, before.blocks);
    assert_eq!(s.state.borrow().current().directory, Directory::Src);
    for module in [
        &this.palette_module_button,
        &this.typography_module_button,
        &this.layout_module_button,
        &this.prompt_module_button,
        &this.greeting_module_button,
    ] {
        module.set_active(true);
        settle();
        assert!(this.full_session.active.get());
    }
    for scene in [1, 2, 3, 0] {
        this.preview_scene_selector.set_selected(scene);
        settle();
    }
    assert_eq!(s.entry.text(), before.draft);
    let edited = this.design_snapshot().unwrap();
    assert_eq!(edited.id, design.id);
    for input in [
        "touch /tmp/termimochi-must-not-exist",
        "ls && pwd",
        "$(id)",
        "ls > x",
    ] {
        command(&this, input);
        assert_eq!(s.state.borrow().current().blocks, before.blocks);
    }
    s.entry.set_text("");
    let mut pos = 0;
    s.entry.insert_text("help\ngit diff", &mut pos);
    assert!(s.entry.text().is_empty());
    assert_eq!(
        this.design_snapshot().unwrap(),
        edited,
        "samples must not dirty or own theme values"
    );
    assert!(this.preview_terminal.pty().is_none());
    for (width, height) in [(1024, 700), (1280, 900)] {
        window.set_default_size(width, height);
        settle();
        assert!(s.entry.is_mapped());
        let bounds = s.entry.compute_bounds(&window).unwrap();
        assert!(bounds.y() + bounds.height() <= window.height() as f32);
        capture(&window, &format!("samples-kitty-{width}"));
    }
    let fds = || std::fs::read_dir("/proc/self/fd").unwrap().count();
    let rss = || {
        std::fs::read_to_string("/proc/self/status")
            .unwrap()
            .lines()
            .find(|line| line.starts_with("VmRSS:"))
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    let before_fd = fds();
    let before_rss = rss();
    for _ in 0..50 {
        s.new_tab.emit_clicked();
        s.close_tab.emit_clicked();
    }
    settle();
    assert_eq!(s.state.borrow().tabs.len(), 2);
    let after_fd = fds();
    let after_rss = rss();
    println!(
        "50 sample-tab cycles: FD {before_fd} -> {after_fd}; RSS KiB {before_rss} -> {after_rss}"
    );
    assert!(after_fd <= before_fd + 2);
    assert!(
        after_rss <= before_rss + 32 * 1024,
        "sample tabs must not accumulate unbounded rendering state"
    );
    window.destroy();
}

#[test]
#[ignore = "isolated GTK: current GIF projection anchors follow fastfetch/clear/tab/font without recompilation"]
fn interactive_samples_gif_anchors() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.SampleGifTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    present_with_preset(&app, None, root.path().join("unused.json"));
    let window = app.active_window().unwrap();
    let this = controller(&window);
    this.load_design(crate::design_document::DesignDocument::new_theme(
        crate::design_document::TargetHint::Kitty,
        "GIF sample",
        true,
    ));
    let (mut settings, _) = crate::pixel_trial::tests::fixture(true);
    settings.presentation.visual = crate::greeting_output::Visual::Animation;
    settings.presentation.columns = 16;
    this.greeting.replace(settings, true);
    println!("samples: GIF assigned");
    let until = std::time::Instant::now() + Duration::from_secs(30);
    while this.greeting.presentation.pixel_dimensions().is_none() {
        assert!(std::time::Instant::now() < until);
        settle();
    }
    ready(&this);
    assert!(this.full_session.image.get().is_some());
    let generation = this.greeting.presentation.pixel_generation();
    command(&this, "fastfetch");
    assert_eq!(this.full_session.extra_images.borrow().len(), 1);
    let first = this.full_session.image.get().unwrap();
    let second = this.full_session.extra_images.borrow()[0].1;
    assert!(second.row > first.row + first.rows as usize);
    this.full_session.play.set_active(true);
    let old = this.full_session.picture.paintable();
    let until = std::time::Instant::now() + Duration::from_secs(3);
    while this.full_session.picture.paintable() == old {
        assert!(std::time::Instant::now() < until);
        settle();
    }
    this.font_size_input.set_value(16.0);
    settle();
    assert_eq!(
        this.full_session.picture.width_request(),
        16 * this.preview_terminal.char_width() as i32
    );
    command(&this, "clear");
    assert!(!this.full_session.picture.is_visible());
    assert!(this.full_session.extra_images.borrow().is_empty());
    this.full_session.samples.new_tab.emit_clicked();
    settle();
    assert!(this.full_session.picture.is_visible());
    this.full_session.samples.selector.set_selected(0);
    settle();
    assert!(!this.full_session.picture.is_visible());
    assert_eq!(this.greeting.presentation.pixel_generation(), generation);
    command(&this, "fastfetch");
    capture(&window, "samples-gif-anchored");
    let greeting = this.greeting.settings();
    this.load_design(crate::design_document::DesignDocument::new_theme(
        crate::design_document::TargetHint::Ptyxis,
        "Ptyxis samples",
        true,
    ));
    this.greeting.replace(greeting, true);
    ready(&this);
    assert!(this.full_session.active.get());
    assert!(!this.full_session.picture.is_visible());
    assert!(this.full_session.image.get().is_none());
    assert!(feed(&this).contains("OS:"));
    capture(&window, "samples-ptyxis-characters");
    window.destroy();
}

#[test]
#[ignore = "isolated XTest GTK input: keys, completion/history, Unicode clipboard and optional real Fcitx preedit"]
fn interactive_samples_keyboard() {
    use glib::translate::ToGlibPtr;
    unsafe extern "C" {
        fn gdk_x11_surface_get_xid(
            surface: *mut gtk::gdk::ffi::GdkSurface,
        ) -> std::os::raw::c_ulong;
    }
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.SampleKeysTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    present_with_preset(&app, None, root.path().join("unused.json"));
    let window = app.active_window().unwrap();
    let this = controller(&window);
    ready(&this);
    let s = &this.full_session.samples;
    s.entry.grab_focus();
    settle();
    app.set_accels_for_action("win.undo", &["<Control>z"]);
    app.set_accels_for_action("win.redo", &["<Control><Shift>z"]);
    app.set_accels_for_action("win.save", &["<Control>s"]);
    let saved_path = root.path().join("keys.termimochi-design.json");
    *this.typed.store.borrow_mut() =
        Some(crate::document_store::DocumentStore::open(saved_path.clone()).unwrap());
    this.apply_color("Background", Rgb::new(31, 42, 53));
    let before = this.design_snapshot().unwrap();
    let ime = std::env::var("GTK_IM_MODULE").as_deref() == Ok("fcitx");
    let xid = unsafe { gdk_x11_surface_get_xid(window.surface().unwrap().to_glib_none().0) };
    std::fs::write(
        root.path().join("input.json"),
        serde_json::to_vec(&serde_json::json!({"xid":xid,"ime":ime})).unwrap(),
    )
    .unwrap();
    let mut child = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/sample-preview-input.py"
        ))
        .arg(root.path())
        .spawn()
        .unwrap();
    let check = |name: &str, assertion: &dyn Fn()| {
        let until = std::time::Instant::now() + Duration::from_secs(30);
        while !root.path().join(name).exists() {
            assert!(
                std::time::Instant::now() < until,
                "driver checkpoint {name}"
            );
            settle();
        }
        assertion();
        std::fs::write(root.path().join(format!("{name}-ok")), "").unwrap();
    };
    check("completed", &|| assert_eq!(s.entry.text(), "git diff"));
    check("history", &|| assert_eq!(s.entry.text(), "git diff"));
    if ime {
        check("preedit", &|| {
            assert!(s.preedit.get());
            assert_eq!(s.state.borrow().current().history.len(), 2);
            capture(&window, "sample-ime-preedit");
        });
        check("preedit-enter", &|| {
            assert!(!s.entry.text().is_empty());
            assert_eq!(
                s.state.borrow().current().history.len(),
                2,
                "IME Enter must not submit a sample"
            );
        });
        check("committed", &|| {
            assert_eq!(s.entry.text(), "你好");
            assert_eq!(s.state.borrow().current().history.len(), 2);
        });
        check("cancelled", &|| {
            assert!(!s.preedit.get());
            assert!(s.entry.text().is_empty());
        });
        println!("Real Fcitx Pinyin preedit / commit / cancel verified");
    } else {
        println!("Chinese IME NOT VERIFIED in the simple-input-method run");
    }
    window.clipboard().set_text("COPY_你好 👩‍💻 e\u{301}");
    check("pasted", &|| {
        assert_eq!(s.entry.text(), "COPY_你好 👩‍💻 e\u{301}")
    });
    check("copied", &|| {
        let text = glib::MainContext::default()
            .block_on(window.clipboard().read_text_future())
            .unwrap()
            .unwrap();
        assert_eq!(text, "COPY_你好 👩‍💻 e\u{301}");
        window.clipboard().set_text("help\ngit diff");
    });
    check("multiline", &|| {
        assert!(
            s.entry.text().is_empty(),
            "multiline paste inserted {:?}",
            s.entry.text()
        );
        assert_eq!(s.state.borrow().current().history.len(), 2);
    });
    check("undo-input", &|| {
        assert!(s.entry.text().len() < 5);
        assert_eq!(this.design_snapshot().unwrap(), before);
    });
    check("redo-input", &|| assert_eq!(s.entry.text(), "draft"));
    check("cancel-input", &|| assert!(s.entry.text().is_empty()));
    check("new-tab", &|| {
        assert_eq!(s.state.borrow().tabs.len(), 2);
        assert!(feed(&this).contains("/demo"));
    });
    check("save", &|| {
        let saved: crate::design_document::DesignDocument =
            serde_json::from_str(&std::fs::read_to_string(&saved_path).unwrap()).unwrap();
        assert_eq!(saved, before);
        assert_eq!(s.state.borrow().tabs.len(), 2);
    });
    assert!(child.wait().unwrap().success());
    assert!(this.preview_terminal.pty().is_none());
    assert_eq!(this.design_snapshot().unwrap(), before);
    capture(&window, "sample-keys-finished");
    window.destroy();
}
