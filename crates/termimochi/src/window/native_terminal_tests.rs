use super::*;
use crate::{
    design_document::{DesignDocument, TargetHint},
    window::greeting::tests::{controller, settle},
};
use glib::translate::ToGlibPtr;
use std::{fs, process::Command, time::Instant};

unsafe extern "C" {
    fn gdk_x11_surface_get_xid(surface: *mut gtk::gdk::ffi::GdkSurface) -> libc::c_ulong;
}
fn until(this: &Rc<Workbench>, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(45);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "Native wait expired: {}",
            this.native_terminal.status.text()
        );
        settle();
        std::thread::sleep(Duration::from_millis(10));
    }
    settle();
}
fn native_capture(this: &Workbench, name: &str, expected: Option<Rgb>) {
    let widget = this
        .native_terminal
        .running
        .borrow()
        .as_ref()
        .unwrap()
        .host
        .clone();
    let deadline = Instant::now() + Duration::from_secs(5);
    let node = loop {
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(&widget)).snapshot(
            &snapshot,
            widget.width() as f64,
            widget.height() as f64,
        );
        if let Some(node) = snapshot.to_node() {
            break node;
        }
        assert!(
            Instant::now() < deadline,
            "actual native surface did not present a frame"
        );
        settle();
        std::thread::sleep(Duration::from_millis(10));
    };
    let texture = widget.native().unwrap().renderer().unwrap().render_texture(
        node,
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
    if let Some(expected) = expected {
        let mut pixels = vec![0; texture.width() as usize * texture.height() as usize * 4];
        texture.download(&mut pixels, texture.width() as usize * 4);
        let count = pixels
            .chunks_exact(4)
            .filter(|p| Rgb::new(p[2], p[1], p[0]) == expected)
            .count();
        assert!(
            count > 1000,
            "native pixels did not contain expected {expected}: {count}; see {name}.png"
        );
    }
}
fn exercise(target: TargetHint) {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    assert!(!matches!(
        std::env::var("DISPLAY").as_deref(),
        Ok(":0" | ":1") | Err(_)
    ));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.NativeInteractiveTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let temp = tempfile::tempdir().unwrap();
    present_with_preset(&app, None, temp.path().join("unused.json"));
    let this = controller(&app.active_window().unwrap());
    until(&this, || {
        !this.preview_loading.get() && !this.copy_loading.get()
    });
    let mut design = DesignDocument::new_theme(target, "Native isolation", true);
    let theme = design.theme.as_mut().unwrap();
    theme.colors.insert("Background".into(), "#E8E5DE".into());
    theme.colors.insert("Foreground".into(), "#31353C".into());
    theme
        .typography
        .insert("size".into(), serde_json::json!(13.0));
    theme.prompt_enabled = true;
    design.components.prompt = Some(crate::design_document::PromptComponent {
        designer: Default::default(),
        starship: Some(
            "format='NATIVE_PROMPT $character'\n[character]\nsuccess_symbol='[>](green)'\n".into(),
        ),
        use_designer: false,
    });
    let mut greeting = crate::greeting::GreetingSettings {
        enabled: true,
        ..Default::default()
    };
    if target == TargetHint::Kitty {
        let source = crate::greeting_image::animation::tests::chunked_fixture();
        greeting
            .import_artwork(
                source
                    .image
                    .decode()
                    .unwrap()
                    .convert(source.options().unwrap())
                    .unwrap()
                    .artwork,
            )
            .unwrap();
        greeting.editable_artwork = Some(source);
        greeting.presentation.visual = crate::greeting_output::Visual::Animation;
        greeting.presentation.columns = 16;
    } else {
        greeting.presentation.visual = crate::greeting_output::Visual::Character;
    }
    greeting.imported_source =
        Some("{\"modules\":[{\"type\":\"os\",\"key\":\"NATIVE_GREETING\"}]}".into());
    design.components.greeting = Some(greeting);
    this.load_design(design);
    this.native_terminal.mode.set_active(true);
    this.native_terminal.accepted.set(true);
    this.start_native_terminal();
    until(&this, || this.native_terminal.running.borrow().is_some());
    let session = this
        .native_terminal
        .running
        .borrow()
        .as_ref()
        .unwrap()
        .session
        .clone();
    let pid = this.native_terminal.running.borrow().as_ref().unwrap().pid;
    until(&this, || !shells(pid.0).is_empty());
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        settle();
    }
    native_capture(&this, "native-greeting-prompt", None);
    if target == TargetHint::Kitty {
        let mut positions = Vec::new();
        for frame in 0..12 {
            let name = format!("native-gif-{frame:02}");
            native_capture(&this, &name, None);
            let picture = image::open(glib::user_cache_dir().join(format!("{name}.png")))
                .unwrap()
                .to_rgba8();
            let red: Vec<_> = picture
                .enumerate_pixels()
                .filter(|(_, _, p)| {
                    p[0].abs_diff(200) < 12 && p[1].abs_diff(30) < 12 && p[2].abs_diff(70) < 12
                })
                .map(|(x, _, _)| x)
                .collect();
            assert!(
                red.len() > 100,
                "actual Kitty GIF must contain the fixture's moving red block"
            );
            positions.push(red.iter().map(|x| *x as u64).sum::<u64>() / red.len() as u64);
            let deadline = Instant::now() + Duration::from_millis(80);
            while Instant::now() < deadline {
                settle();
            }
        }
        assert!(
            positions.iter().max().unwrap() - positions.iter().min().unwrap() > 10,
            "Kitty GIF did not animate: {positions:?}"
        );
        fs::write(
            glib::user_cache_dir().join("native-gif-motion.json"),
            serde_json::to_vec(&positions).unwrap(),
        )
        .unwrap();
    }
    let before = this.committed_design().unwrap();
    for module in [
        &this.palette_module_button,
        &this.typography_module_button,
        &this.layout_module_button,
        &this.prompt_module_button,
        &this.greeting_module_button,
    ] {
        module.set_active(true);
        settle();
        assert_eq!(
            this.native_terminal.stack.visible_child_name().as_deref(),
            Some("native")
        );
        assert_eq!(
            this.native_terminal.running.borrow().as_ref().unwrap().pid,
            pid
        );
    }
    assert_eq!(this.committed_design().unwrap(), before);
    assert!(!this.inspect_button.is_sensitive());
    this.window().clipboard().set_text("NATIVE_CLIPBOARD_你好");
    until(&this, || {
        this.native_terminal
            .running
            .borrow()
            .as_ref()
            .unwrap()
            .host
            .width()
            > 100
    });
    let host = this
        .native_terminal
        .running
        .borrow()
        .as_ref()
        .unwrap()
        .host
        .clone();
    let bounds = host.compute_bounds(&this.window()).unwrap();
    let scale = this.window().scale_factor() as f32;
    let xid = unsafe { gdk_x11_surface_get_xid(this.window().surface().unwrap().to_glib_none().0) };
    fs::write(temp.path().join("input.json"),serde_json::to_vec(&serde_json::json!({"xid":xid,"x":((bounds.x()+bounds.width()/2.0)*scale) as i32,"y":((bounds.y()+bounds.height()/2.0)*scale) as i32,"evidence_dir":glib::user_cache_dir()})).unwrap()).unwrap();
    let mut driver = Command::new("python3")
        .arg(
            std::env::var_os("TERMIMOCHI_NATIVE_INPUT_DRIVER")
                .unwrap_or_else(|| "scripts/native-preview-input.py".into()),
        )
        .arg(temp.path())
        .spawn()
        .unwrap();
    until(&this, || temp.path().join("preedit-ready").exists());
    crate::window::typed_tests::capture(this.window().upcast_ref(), "native-app-ime-preedit");
    native_capture(
        &this,
        "native-client-preedit",
        Some(Rgb::new(232, 229, 222)),
    );
    fs::write(temp.path().join("preedit-captured"), "").unwrap();
    until(&this, || temp.path().join("hot-ready").exists());
    assert_eq!(
        fs::read_to_string(temp.path().join("ime-commit")).unwrap(),
        "你好\n"
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("ime-cancel")).unwrap(),
        "OK\n"
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("clipboard")).unwrap(),
        "NATIVE_CLIPBOARD_你好"
    );
    let shell1 = fs::read_to_string(temp.path().join("shell-before")).unwrap();
    let shell2 = fs::read_to_string(temp.path().join("shell-tab-two")).unwrap();
    let foreground = fs::read_to_string(format!("/proc/{shell2}/task/{shell2}/children")).unwrap();
    let foreground: Vec<_> = foreground
        .split_whitespace()
        .filter(|pid| {
            fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|name| name.trim() == "top")
        })
        .map(str::to_owned)
        .collect();
    assert_eq!(
        foreground.len(),
        1,
        "actual native top TUI is running before appearance sync"
    );
    assert_ne!(shell1.split(':').next().unwrap(), shell2);
    this.palette_module_button.set_active(true);
    this.apply_color("Background", Rgb::new(221, 238, 232));
    this.font_size_input.set_value(17.0);
    until(&this, || {
        !this.native_terminal.busy.get()
            && this.native_terminal.last.borrow().as_ref() == this.native_snapshot().as_ref().ok()
    });
    // File readback is not enough: inspect the actual client texture.
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        settle();
    }
    native_capture(&this, "native-client-hot", Some(Rgb::new(221, 238, 232)));
    assert_eq!(
        this.native_terminal.running.borrow().as_ref().unwrap().pid,
        pid
    );
    println!("Hot sync status: {}", this.native_terminal.status.text());
    for pid in foreground {
        assert!(
            PathBuf::from(format!("/proc/{pid}")).exists(),
            "appearance sync killed the TUI"
        );
    }
    fs::write(temp.path().join("hot-done"), "").unwrap();
    until(&this, || temp.path().join("copy-ready").exists());
    native_capture(&this, "native-copy-selection-source", None);
    let picture = image::open(glib::user_cache_dir().join("native-copy-selection-source.png"))
        .unwrap()
        .to_rgba8();
    let blue: Vec<_> = picture
        .enumerate_pixels()
        .filter(|(_, _, p)| p[0] == 20 && p[1] == 90 && p[2] == 180)
        .map(|(x, y, _)| (x, y))
        .collect();
    assert!(
        blue.len() > 100,
        "actual terminal rendered the selectable clipboard fixture"
    );
    let minx = blue.iter().map(|p| p.0).min().unwrap();
    let maxx = blue.iter().map(|p| p.0).max().unwrap();
    let miny = blue.iter().map(|p| p.1).min().unwrap();
    let maxy = blue.iter().map(|p| p.1).max().unwrap();
    this.window()
        .clipboard()
        .set_text("HOST_SENTINEL_AFTER_PASTE");
    let bounds = host.compute_bounds(&this.window()).unwrap();
    let (dx, dy) = this.window().surface_transform();
    println!(
        "COPY_GEOMETRY bounds={bounds:?} surface_transform={dx},{dy} scale={scale} blue={minx},{miny}..{maxx},{maxy}"
    );
    fs::write(temp.path().join("copy-coordinates.json"),serde_json::to_vec(&serde_json::json!({"x1":((bounds.x()-dx as f32+minx as f32+1.0)*scale) as i32,"x2":((bounds.x()-dx as f32+maxx as f32+2.0)*scale) as i32,"y":((bounds.y()-dy as f32+(miny+maxy) as f32/2.0)*scale) as i32})).unwrap()).unwrap();
    until(&this, || temp.path().join("copy-done").exists());
    native_capture(&this, "native-copy-selection-result", None);
    let copied = Rc::new(RefCell::new(None));
    let output = copied.clone();
    this.window()
        .clipboard()
        .read_text_async(gio::Cancellable::NONE, move |r| {
            *output.borrow_mut() = Some(r.unwrap().unwrap().to_string())
        });
    until(&this, || copied.borrow().is_some());
    assert!(
        copied
            .borrow()
            .as_ref()
            .unwrap()
            .contains("NATIVE_CLIPBOARD_你好"),
        "native-to-host selection copy: {:?}",
        copied.borrow()
    );
    fs::write(temp.path().join("copy-checked"), "").unwrap();
    until(&this, || temp.path().join("input-done").exists());
    assert!(driver.wait().unwrap().success());
    assert!(
        !this.native_terminal.mode.is_active(),
        "reserved Ctrl+Alt+Shift+F12 returns to design without killing the shell"
    );
    assert_eq!(
        this.native_snapshot().unwrap().workspace.typography.size,
        17.0,
        "native Ctrl+Z must not undo the theme"
    );
    this.native_terminal.mode.set_active(true);
    let width = host.width();
    this.native_terminal.expand.set_active(true);
    until(&this, || host.width() > width);
    native_capture(&this, "native-expanded", None);
    this.native_terminal.expand.set_active(false);
    assert!(
        !fs::read_to_string(temp.path().join("stopped-job"))
            .unwrap()
            .trim()
            .is_empty(),
        "Ctrl+Z reaches the native foreground job"
    );
    let prompt_path = fs::read_to_string(temp.path().join("prompt-config")).unwrap();
    assert!(
        fs::read_to_string(prompt_path)
            .unwrap()
            .contains("NATIVE_PROMPT"),
        "the second tab must use the current controlled Prompt"
    );
    assert_ne!(
        fs::read(temp.path().join("grid-before")).unwrap(),
        fs::read(temp.path().join("grid-after")).unwrap(),
        "native font size must change actual PTY grid"
    );
    assert_eq!(
        fs::read(temp.path().join("live-before")).unwrap(),
        fs::read(temp.path().join("live-after")).unwrap(),
        "hot update must preserve shell PID and cwd"
    );
    this.native_terminal.mode.set_active(false);
    settle();
    assert!(
        this.native_is_running(),
        "design mode does not kill native sessions"
    );
    this.native_terminal.mode.set_active(true);
    settle();
    crate::window::typed_tests::capture(this.window().upcast_ref(), "native-app-after-hot");
    // Stop owned supervisor only; its exit means descendants were reaped.
    unsafe {
        libc::kill(pid.0, libc::SIGTERM);
    }
    until(&this, || !this.native_is_running());
    assert!(!PathBuf::from(format!("/proc/{shell2}")).exists());
    assert_eq!(
        fs::read_to_string(session.root.path().join("config/glib-2.0/settings/keyfile")).is_ok(),
        target == TargetHint::Ptyxis
    );
    println!(
        "NATIVE_ENABLED target={target:?} shells={shell1},{shell2} IME=commit+cancel clipboard=host-to-native hot=preserved lifetime=reaped root={}",
        session.root.path().display()
    );
    this.window().destroy();
    settle();
}
#[test]
#[ignore = "enabled feature: actual embedded Ptyxis, IME, tabs, clipboard, hot updates, lifetime"]
fn native_interactive_ptyxis_enabled() {
    exercise(TargetHint::Ptyxis);
}
#[test]
#[ignore = "enabled feature: actual embedded Kitty, IME, tabs, clipboard, hot updates, lifetime"]
fn native_interactive_kitty_enabled() {
    exercise(TargetHint::Kitty);
}

fn test_app() -> adw::Application {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    assert!(!matches!(
        std::env::var("DISPLAY").as_deref(),
        Ok(":0" | ":1") | Err(_)
    ));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.NativeLifecycleTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    app
}
fn open_native(app: &adw::Application, target: TargetHint, color: &str) -> Rc<Workbench> {
    present_with_preset(app, None, glib::user_cache_dir().join("unused-font.json"));
    let this = controller(&app.active_window().unwrap());
    until(&this, || {
        !this.preview_loading.get() && !this.copy_loading.get()
    });
    let mut design = DesignDocument::new_theme(target, "Private native test", true);
    design
        .theme
        .as_mut()
        .unwrap()
        .colors
        .insert("Background".into(), color.into());
    this.load_design(design);
    this.native_terminal.mode.set_active(true);
    this.native_terminal.accepted.set(true);
    this.start_native_terminal();
    until(&this, || this.native_is_running());
    until(&this, || {
        !shells(
            this.native_terminal
                .running
                .borrow()
                .as_ref()
                .unwrap()
                .pid
                .0,
        )
        .is_empty()
    });
    this
}
fn shells(pid: i32) -> Vec<i32> {
    let mut todo = vec![pid];
    let mut shells = Vec::new();
    while let Some(pid) = todo.pop() {
        if let Ok(children) = fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")) {
            for child in children
                .split_whitespace()
                .filter_map(|s| s.parse::<i32>().ok())
            {
                todo.push(child);
                if fs::read(format!("/proc/{child}/cmdline"))
                    .ok()
                    .is_some_and(|b| b.split(|c| *c == 0).next() == Some(b"/usr/bin/bash"))
                {
                    shells.push(child);
                }
            }
        }
    }
    shells
}
fn stop_now(this: &Rc<Workbench>) {
    let pid = this.native_terminal.running.borrow().as_ref().unwrap().pid;
    unsafe {
        libc::kill(pid.0, libc::SIGTERM);
    }
    until(this, || !this.native_is_running());
}
fn cycles(target: TargetHint) {
    let app = test_app();
    let this = open_native(&app, target, "#E8E5DE");
    stop_now(&this);
    let mut counts = Vec::new();
    let mut seconds = Vec::new();
    let mut rss_kib = Vec::new();
    for cycle in 0..30 {
        let began = Instant::now();
        this.start_native_terminal();
        until(&this, || this.native_is_running());
        until(&this, || {
            !shells(
                this.native_terminal
                    .running
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .pid
                    .0,
            )
            .is_empty()
        });
        let pids = shells(
            this.native_terminal
                .running
                .borrow()
                .as_ref()
                .unwrap()
                .pid
                .0,
        );
        let weak_host = this
            .native_terminal
            .running
            .borrow()
            .as_ref()
            .unwrap()
            .host
            .downgrade();
        stop_now(&this);
        assert!(
            weak_host.upgrade().is_none(),
            "native host retained after stop"
        );
        for pid in pids {
            assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
        }
        counts.push(fs::read_dir("/proc/self/fd").unwrap().count());
        seconds.push(began.elapsed().as_secs_f64());
        rss_kib.push(
            fs::read_to_string("/proc/self/status")
                .unwrap()
                .lines()
                .find_map(|l| {
                    l.strip_prefix("VmRSS:")
                        .and_then(|v| v.split_whitespace().next())
                        .and_then(|v| v.parse::<u64>().ok())
                })
                .unwrap(),
        );
        println!(
            "NATIVE_CYCLE target={target:?} cycle={cycle} fds={} seconds={}",
            counts.last().unwrap(),
            seconds.last().unwrap()
        );
    }
    fs::write(glib::user_cache_dir().join("native-cycles.json"),serde_json::to_vec_pretty(&serde_json::json!({"target":format!("{target:?}"),"cycles":30,"fds":counts,"seconds":seconds,"rss_kib":rss_kib})).unwrap()).unwrap();
    assert!(
        counts[29] <= counts[3] + 4,
        "unbounded FD growth: {counts:?}"
    );
    this.window().destroy();
    settle();
}
#[test]
#[ignore = "enabled feature: 30 actual Ptyxis shell start/stop cycles and FD/descendant checks"]
fn native_lifecycle_ptyxis_30_cycles() {
    cycles(TargetHint::Ptyxis);
}
#[test]
#[ignore = "enabled feature: 30 actual Kitty shell start/stop cycles and FD/descendant checks"]
fn native_lifecycle_kitty_30_cycles() {
    cycles(TargetHint::Kitty);
}

fn isolated_windows(target: TargetHint) {
    let app = test_app();
    let a = open_native(&app, target, "#E8E5DE");
    let b = open_native(&app, target, "#DDEEE8");
    let daily = open_native(&app, target, "#EEDDDD");
    let roots = [&a, &b, &daily].map(|w| {
        w.native_terminal
            .running
            .borrow()
            .as_ref()
            .unwrap()
            .session
            .root
            .path()
            .to_owned()
    });
    assert!(roots[0] != roots[1] && roots[1] != roots[2]);
    let file = if target == TargetHint::Ptyxis {
        "config/glib-2.0/settings/keyfile"
    } else {
        "kitty.conf"
    };
    let b_before = fs::read(roots[1].join(file)).unwrap();
    let daily_before = fs::read(roots[2].join(file)).unwrap();
    a.apply_color("Background", Rgb::new(204, 221, 238));
    a.font_size_input.set_value(19.0);
    if target == TargetHint::Ptyxis {
        a.apply_color("TitlebarBackground", Rgb::new(95, 125, 155));
    }
    until(&a, || {
        !a.native_terminal.busy.get()
            && a.native_terminal.last.borrow().as_ref() == a.native_snapshot().as_ref().ok()
    });
    let end = Instant::now() + Duration::from_secs(1);
    while Instant::now() < end {
        settle();
    }
    native_capture(&a, "native-isolation-a", Some(Rgb::new(204, 221, 238)));
    if target == TargetHint::Ptyxis {
        native_capture(&a, "native-titlebar-color", Some(Rgb::new(95, 125, 155)));
    }
    native_capture(&b, "native-isolation-b", Some(Rgb::new(221, 238, 232)));
    native_capture(
        &daily,
        "native-isolation-mock-daily",
        Some(Rgb::new(238, 221, 221)),
    );
    assert_eq!(fs::read(roots[1].join(file)).unwrap(), b_before);
    assert_eq!(fs::read(roots[2].join(file)).unwrap(), daily_before);
    let pid = a.native_terminal.running.borrow().as_ref().unwrap().pid;
    let mut inherited = a.committed_design().unwrap();
    let theme = inherited.theme.as_mut().unwrap();
    theme.colors.clear();
    theme.typography.clear();
    a.load_design(inherited);
    until(&a, || {
        !a.native_terminal.busy.get()
            && a.native_terminal.last.borrow().as_ref() == a.native_snapshot().as_ref().ok()
    });
    let end = Instant::now() + Duration::from_secs(1);
    while Instant::now() < end {
        settle();
    }
    native_capture(
        &a,
        "native-inherited-baseline",
        Some(if target == TargetHint::Ptyxis {
            Rgb::new(255, 255, 255)
        } else {
            Rgb::new(0, 0, 0)
        }),
    );
    assert_eq!(
        a.native_terminal.running.borrow().as_ref().unwrap().pid,
        pid
    );
    let alien = b.native_snapshot().unwrap();
    assert!(
        a.native_terminal
            .running
            .borrow()
            .as_ref()
            .unwrap()
            .session
            .apply(&alien, true)
            .is_err()
    );
    if target == TargetHint::Ptyxis {
        let before = a.committed_design().unwrap();
        let session = a
            .native_terminal
            .running
            .borrow()
            .as_ref()
            .unwrap()
            .session
            .clone();
        let helper = PathBuf::from(std::env::var_os("TERMIMOCHI_NATIVE_PREFIX").unwrap())
            .join("bin/termimochi-settings-helper");
        assert!(
            Command::new(helper)
                .env_clear()
                .envs(&session.env)
                .arg(roots[0].join(file))
                .args(["org.gnome.Ptyxis", "-", "font-name", "'Monospace 21'"])
                .status()
                .unwrap()
                .success()
        );
        until(&a, || {
            a.native_terminal
                .status
                .text()
                .contains("temporary native settings")
        });
        assert_eq!(a.committed_design().unwrap(), before);
        assert!(session.temporary_difference().unwrap());
        a.native_terminal.resync.emit_clicked();
        until(&a, || {
            !a.native_terminal.busy.get()
                && !a.native_terminal.dirty.get()
                && !session.temporary_difference().unwrap()
        });
    }
    stop_now(&a);
    assert_eq!(fs::read(roots[1].join(file)).unwrap(), b_before);
    assert_eq!(fs::read(roots[2].join(file)).unwrap(), daily_before);
    stop_now(&b);
    stop_now(&daily);
    for w in [&a, &b, &daily] {
        w.window().destroy();
    }
    settle();
    println!(
        "NATIVE_ISOLATION {target:?}: A changed; B and mock daily pixels + settings unchanged, including after A stopped"
    );
}
#[test]
#[ignore = "enabled feature: Ptyxis A/B/mock daily global settings and native pixel isolation"]
fn native_isolation_ptyxis_three_instances() {
    isolated_windows(TargetHint::Ptyxis);
}
#[test]
#[ignore = "enabled feature: Kitty A/B/mock daily endpoint and native pixel isolation"]
fn native_isolation_kitty_three_instances() {
    isolated_windows(TargetHint::Kitty);
}

#[test]
#[ignore = "enabled feature: two Ptyxis and one Kitty live at the same time"]
fn native_mixed_targets_three_workspaces() {
    let app = test_app();
    let a = open_native(&app, TargetHint::Ptyxis, "#E8E5DE");
    let b = open_native(&app, TargetHint::Ptyxis, "#DDEEE8");
    let c = open_native(&app, TargetHint::Kitty, "#EEDDDD");
    for (w, name, color) in [
        (&a, "native-mixed-ptyxis-a", Rgb::new(232, 229, 222)),
        (&b, "native-mixed-ptyxis-b", Rgb::new(221, 238, 232)),
        (&c, "native-mixed-kitty", Rgb::new(238, 221, 221)),
    ] {
        native_capture(w, name, Some(color));
    }
    stop_now(&a);
    assert!(b.native_is_running() && c.native_is_running());
    stop_now(&b);
    stop_now(&c);
    for w in [&a, &b, &c] {
        w.window().destroy();
    }
    settle();
}

fn client_crash(target: TargetHint) {
    let app = test_app();
    let this = open_native(&app, target, "#E8E5DE");
    let (pid, executable, root) = {
        let running = this.native_terminal.running.borrow();
        let r = running.as_ref().unwrap();
        (
            r.pid.0,
            PathBuf::from(&r.session.argv[3]).canonicalize().unwrap(),
            r.session.root.path().to_owned(),
        )
    };
    let shells = shells(pid);
    let children = fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")).unwrap();
    let client = children
        .split_whitespace()
        .find(|p| fs::read_link(format!("/proc/{p}/exe")).is_ok_and(|path| path == executable))
        .unwrap()
        .parse::<i32>()
        .unwrap();
    unsafe {
        libc::kill(client, libc::SIGKILL);
    }
    until(&this, || !this.native_is_running());
    assert!(this.native_terminal.status.text().contains("unexpectedly"));
    for pid in shells {
        assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
    }
    assert!(
        !root.exists(),
        "private assets are released after owned children exit"
    );
    this.start_native_terminal();
    until(&this, || this.native_is_running());
    stop_now(&this);
    this.window().destroy();
    settle();
}
#[test]
#[ignore = "enabled feature: crashed Kitty client is reaped and restart remains usable"]
fn native_crash_kitty_recovery() {
    client_crash(TargetHint::Kitty);
}
#[test]
#[ignore = "enabled feature: crashed Ptyxis client is reaped and restart remains usable"]
fn native_crash_ptyxis_recovery() {
    client_crash(TargetHint::Ptyxis);
}
