use super::*;
use crate::window::greeting::tests::{controller, settle};

fn ready(this: &Workbench) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    for _ in 0..4 {
        settle();
    }
    while this.preview_loading.get()
        || this.copy_loading.get()
        || this.greeting_official_loading.get()
    {
        assert!(std::time::Instant::now() < deadline);
        settle();
    }
    for _ in 0..4 {
        settle();
    }
}

fn bounds(widget: &impl IsA<gtk::Widget>, window: &gtk::Window) -> [f32; 4] {
    let r = widget.compute_bounds(window).unwrap();
    [r.x(), r.y(), r.width(), r.height()]
}

fn capture(this: &Workbench, name: &str) {
    let window = this.window();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let node = loop {
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(&window)).snapshot(
            &snapshot,
            window.width() as f64,
            window.height() as f64,
        );
        if let Some(node) = snapshot.to_node() {
            break node;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "GTK must produce an allocated frame"
        );
        settle();
    };
    let texture = window.renderer().unwrap().render_texture(node, None);
    texture
        .save_to_png(glib::user_cache_dir().join(format!("{name}.png")))
        .unwrap();
    let t = &this.preview_terminal;
    let shadow = this.full_session.frame.shadow_bounds();
    let data = serde_json::json!({
        "app": [window.width(), window.height()], "device_scale": window.scale_factor(),
        "canvas": bounds(&this.full_session.frame,window.upcast_ref()),
        "shadow_in_canvas": [shadow.x(),shadow.y(),shadow.width(),shadow.height()],
        "window_surface": bounds(&this.preview_terminal_shell,window.upcast_ref()),
        "canvas_overflow": format!("{:?}",this.full_session.frame.overflow()),
        "surface_clip": "rounded 10 logical px, scaled once; shadow outside clip",
        "viewport": bounds(&this.preview_terminal_viewport, window.upcast_ref()),
        "terminal": bounds(t, window.upcast_ref()),
        "input": bounds(&this.full_session.samples.entry, window.upcast_ref()),
        "title": bounds(&this.top_preview.title, window.upcast_ref()),
        "tabs": bounds(&this.top_preview.tabs, window.upcast_ref()),
        "toolbar": bounds(&this.full_session.toolbar, window.upcast_ref()),
        "cell": [t.char_width(), t.char_height()], "grid": [t.column_count(), t.row_count()],
        "cursor": [t.cursor_position().0, t.cursor_position().1],
        "observation_scale": this.full_session.scale.get(),
        "font_size": this.typography_settings().size,
        "scroll": [this.preview_scroll.adjustment.value(), this.preview_scroll.adjustment.upper(), this.preview_scroll.adjustment.page_size()],
        "theme_columns": this.layout_settings().columns,
        "greeting_columns": this.greeting.settings().presentation.columns,
    });
    std::fs::write(
        glib::user_cache_dir().join(format!("{name}.json")),
        serde_json::to_vec_pretty(&data).unwrap(),
    )
    .unwrap();
    println!("GEOMETRY {name}: {data}");
}

#[test]
#[ignore = "isolated GTK: finite centered canvas, stable history, target tops, long labels and rounded clipping"]
fn preview_presentation_ptyxis_light() {
    presentation_canvas(crate::design_document::TargetHint::Ptyxis, false);
}
#[test]
#[ignore = "isolated GTK: Ptyxis dark canvas and target top"]
fn preview_presentation_ptyxis_dark() {
    presentation_canvas(crate::design_document::TargetHint::Ptyxis, true);
}
#[test]
#[ignore = "isolated GTK: Kitty light canvas and target top"]
fn preview_presentation_kitty_light() {
    presentation_canvas(crate::design_document::TargetHint::Kitty, false);
}
#[test]
#[ignore = "isolated GTK: Kitty dark canvas and target top"]
fn preview_presentation_kitty_dark() {
    presentation_canvas(crate::design_document::TargetHint::Kitty, true);
}
fn presentation_canvas(target: crate::design_document::TargetHint, dark: bool) {
    // Drain allocations and the actual bounded jobs instead of imposing the
    // legacy test's eight fixed waits on every matrix cell. Capture still has
    // its own frame deadline; every geometry assertion remains in place.
    let ready = |this: &Workbench| {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            settle();
            if !this.geometry_pending.get()
                && !this.full_session.input_pending.get()
                && !this.preview_loading.get()
                && !this.copy_loading.get()
                && !this.greeting_official_loading.get()
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "preview did not settle"
            );
        }
    };
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.CanvasTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    present_with_preset(&app, None, root.path().join("font.json"));
    let window = app.active_window().unwrap();
    let this = controller(&window);
    {
        this.load_design(crate::design_document::DesignDocument::new_theme(
            target,
            "Canvas fixture",
            true,
        ));
        this.tab_bar_switch.set_active(true);
        this.font_size_input.set_value(13.0);
        ready(&this);
        {
            this.apply_color(
                "Background",
                if dark {
                    Rgb::new(27, 31, 38)
                } else {
                    Rgb::new(241, 238, 231)
                },
            );
            this.apply_color(
                "Foreground",
                if dark {
                    Rgb::new(235, 238, 242)
                } else {
                    Rgb::new(35, 40, 45)
                },
            );
            for (width, height) in [(1130, 830), (2048, 1126)] {
                window.set_default_size(width, height);
                this.full_session.zoom.set_selected(4);
                ready(&this);
                let saved = this.committed_design().unwrap();
                assert!(
                    !crate::window::greeting::tests::descendants(
                        this.preview_terminal_shell.upcast_ref()
                    )
                    .iter()
                    .any(|w| w.is::<gtk::WindowControls>() || w.is::<gtk::WindowHandle>()),
                    "sample top must not manipulate the host window"
                );
                let frame = bounds(&this.full_session.frame, &window);
                let shell = bounds(&this.preview_terminal_shell, &window);
                assert!(
                    shell[0] >= frame[0] + 20.0 && shell[1] >= frame[1] + 20.0,
                    "canvas needs shadow gutter"
                );
                assert!(shell[0] + shell[2] <= frame[0] + frame[2] - 20.0);
                assert!(shell[1] + shell[3] <= frame[1] + frame[3] - 20.0);
                assert!(
                    (shell[0] - frame[0] - (frame[2] - shell[2]) / 2.0).abs() < 2.0,
                    "centered surface"
                );
                assert_eq!(this.full_session.scale.get(), 1.0);
                let shadow = this.full_session.frame.shadow_bounds();
                assert!(
                    shadow.x() >= 0.0
                        && shadow.y() >= 0.0
                        && shadow.x() + shadow.width() <= frame[2]
                        && shadow.y() + shadow.height() <= frame[3],
                    "GSK shadow bounds must fit within outer clip: {shadow:?}"
                );
                assert!(
                    this.preview_terminal.column_count() <= this.layout_settings().columns as i64
                );
                this.full_session
                    .samples
                    .entry
                    .set_text("尚未提交的中文与长输入 — draft draft draft");
                for _ in 0..55 {
                    this.sample_action(crate::interactive_samples::Action::Help);
                }
                ready(&this);
                assert_eq!(
                    bounds(&this.preview_terminal_shell, &window),
                    shell,
                    "history changes only internal scroll"
                );
                assert!(
                    this.full_session
                        .samples
                        .entry
                        .text()
                        .starts_with("尚未提交")
                );
                this.sample_action(crate::interactive_samples::Action::Clear);
                ready(&this);
                assert!(!crate::window::greeting::tests::feed(&this).contains("TermiMochi sample"));
                for tabs in [1, 2, 6] {
                    *this.full_session.samples.state.borrow_mut() = Default::default();
                    for _ in 1..tabs {
                        assert!(this.full_session.samples.state.borrow_mut().add());
                    }
                    this.sync_sample_controls();
                    this.redraw_preview_contents();
                    ready(&this);
                    if target == crate::design_document::TargetHint::Ptyxis {
                        assert_eq!(this.top_preview.tabs.is_visible(), tabs > 1);
                    }
                    if tabs == 6 {
                        let mut child = this.top_preview.buttons.first_child();
                        while let Some(widget) = child {
                            widget
                                .clone()
                                .downcast::<gtk::Button>()
                                .unwrap()
                                .set_label("very-long-build-title · 中文工作目录 · session");
                            child = widget.next_sibling();
                        }
                        this.refresh_window_top();
                        ready(&this);
                    }
                    for mode in [4, 0, 1] {
                        this.full_session.zoom.set_selected(mode);
                        ready(&this);
                        aligned(&this);
                        assert_eq!(
                            this.committed_design().unwrap(),
                            saved,
                            "view changes never author theme fields"
                        );
                        capture(
                            &this,
                            &format!(
                                "canvas-{target:?}-{width}-{}-{tabs}tabs-mode{mode}",
                                if dark { "dark" } else { "light" }
                            ),
                        );
                    }
                }
            }
        }
    }
    window.destroy();
}

pub(super) fn aligned(this: &Workbench) {
    let terminal = &this.preview_terminal;
    let origin = preview_visible_origin(terminal)
        .expect("visible prompt origin")
        .0;
    let (col, row) = terminal.cursor_position();
    let window = this.window();
    let t = bounds(terminal, window.upcast_ref());
    let e = bounds(&this.full_session.samples.entry, window.upcast_ref());
    let s = this.full_session.scale.get() as f32;
    let x = t[0] + col as f32 * terminal.char_width() as f32 * s;
    let y = t[1] + (row - origin) as f32 * terminal.char_height() as f32 * s;
    assert!(
        (e[0] - x).abs() <= 1.5 && (e[1] - y).abs() <= 1.5,
        "input {e:?}, expected origin {x},{y}, scale {s}"
    );
    for widget in [
        this.top_preview.title.upcast_ref::<gtk::Widget>(),
        this.full_session.samples.entry.upcast_ref(),
        terminal.upcast_ref(),
    ] {
        let b = bounds(widget, window.upcast_ref());
        assert!(
            (b[2] - widget.width() as f32 * s).abs() < 2.0,
            "one transform for chrome/body/input"
        );
    }
    assert!(
        crate::window::greeting::tests::feed(this).ends_with("\x1b[?25l"),
        "VTE cursor must stay hidden; GTK owns the input cursor"
    );
}

fn pointer(this: &Workbench, widget: &impl IsA<gtk::Widget>, mode: &str, x: f32, y: f32) {
    let window = this.window();
    window.set_title(Some("TermiMochi point-to-edit test"));
    let point = widget
        .compute_point(&window, &gtk::graphene::Point::new(x, y))
        .unwrap();
    let (dx, dy) = window.surface_transform();
    let s = window.scale_factor() as f64;
    let mut child = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/preview-pointer-driver.py"
        ))
        .args([
            mode,
            &(((point.x() as f64 + dx) * s) as i32).to_string(),
            &(((point.y() as f64 + dy) * s) as i32).to_string(),
            "3",
        ])
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        assert!(std::time::Instant::now() < deadline);
        settle();
    }
    assert!(child.wait().unwrap().success());
    ready(this);
}

#[test]
#[ignore = "isolated GTK: same-theme geometry at screenshot dimensions, actual-size and whole-window Fit"]
fn preview_geometry_same_theme() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.GeometryTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    present_with_preset(&app, None, root.path().join("unused-font.json"));
    let window = app.active_window().unwrap();
    let this = controller(&window);
    this.load_design(crate::design_document::DesignDocument::new_theme(
        crate::design_document::TargetHint::Ptyxis,
        "Geometry fixture",
        true,
    ));
    this.apply_color("Background", Rgb::new(250, 237, 221));
    this.apply_color("Foreground", Rgb::new(37, 43, 51));
    this.font_size_input.set_value(13.0);
    ready(&this);
    for action in [
        crate::interactive_samples::Action::Clear,
        crate::interactive_samples::Action::Help,
        crate::interactive_samples::Action::Sample(PreviewScenario::GitDiff),
    ] {
        this.sample_action(action);
        ready(&this);
    }
    this.full_session.samples.entry.set_text("git status");
    this.full_session.samples.entry.grab_focus();
    let design = this.design_snapshot().unwrap();
    let phase = if std::env::var_os("TERMIMOCHI_GEOMETRY_BEFORE").is_some() {
        "before"
    } else {
        "after"
    };
    for (width, height, mode, name) in [
        (2048, 1126, 0, "large-fit"),
        (1130, 830, 1, "small-100"),
        (1130, 830, 0, "small-fit"),
    ] {
        window.set_default_size(width, height);
        this.full_session.zoom.set_selected(mode);
        ready(&this);
        this.preview_scroll.follow_input();
        ready(&this);
        capture(&this, &format!("{phase}-{name}"));
        assert_eq!(this.design_snapshot().unwrap(), design);
        if phase == "after" {
            aligned(&this);
        }
    }
    if phase == "after" {
        this.full_session.zoom.set_selected(4);
        let mut columns = Vec::new();
        let cell = (
            this.preview_terminal.char_width(),
            this.preview_terminal.char_height(),
        );
        for (width, height) in [(2048, 1126), (1130, 830)] {
            window.set_default_size(width, height);
            ready(&this);
            this.preview_scroll.follow_input();
            ready(&this);
            aligned(&this);
            assert_eq!(this.full_session.scale.get(), 1.0);
            assert_eq!(
                (
                    this.preview_terminal.char_width(),
                    this.preview_terminal.char_height()
                ),
                cell
            );
            columns.push(this.preview_terminal.column_count());
            assert_eq!(this.design_snapshot().unwrap(), design);
            capture(&this, &format!("after-actual-{width}"));
        }
        assert!(
            columns[0] > columns[1],
            "adaptive grid at unchanged actual font size"
        );
        this.full_session.zoom.set_selected(0);
        ready(&this);
        let scale = this.full_session.scale.get();
        let viewport = bounds(&this.preview_terminal_viewport, &window);
        for _ in 0..20 {
            this.sample_action(crate::interactive_samples::Action::Help);
        }
        ready(&this);
        assert!(
            (this.full_session.scale.get() - scale).abs() < 0.001,
            "Fit must not shrink with history"
        );
        assert_eq!(bounds(&this.preview_terminal_viewport, &window), viewport);
        aligned(&this);
        pointer(
            &this,
            &this.preview_terminal_viewport,
            "scroll_up",
            30.0,
            30.0,
        );
        ready(&this);
        let scroll = this.preview_scroll.adjustment.value();
        this.apply_color("Background", Rgb::new(250, 237, 220));
        ready(&this);
        assert!(
            (this.preview_scroll.adjustment.value() - scroll).abs() < 1.0,
            "palette must not jump to input"
        );
        this.sample_action(crate::interactive_samples::Action::Clear);
        ready(&this);
        aligned(&this);
        capture(&this, "after-clear");
        assert!(
            !this.full_session.play.is_visible(),
            "Ptyxis has no pixel playback action"
        );
        this.tab_bar_switch.set_active(true);
        this.full_session.samples.entry.set_text("第一标签");
        this.full_session
            .samples
            .tabs
            .first_child()
            .unwrap()
            .downcast::<gtk::Button>()
            .unwrap()
            .emit_clicked();
        this.full_session.samples.entry.set_text("second");
        ready(&this);
        let button = this.top_preview.buttons.first_child().unwrap();
        pointer(
            &this,
            &button,
            "click",
            button.width() as f32 / 2.0,
            button.height() as f32 / 2.0,
        );
        assert_eq!(
            this.full_session.samples.entry.text(),
            "第一标签",
            "transformed tab hit restores its own draft"
        );
        this.sample_action(crate::interactive_samples::Action::Help);
        ready(&this);
        this.preview_scroll.start();
        ready(&this);
        pointer(&this, &this.preview_terminal, "drag", 10.0, 10.0);
        assert!(
            this.preview_terminal.has_selection(),
            "transformed VTE selection, not input focus theft"
        );
        assert!(this.focused_sample_text().is_none());
        capture(&this, "after-fit-selection-tabs");
    }
    window.destroy();
}
