//! Opt-in reproduction using a private copy of the reported complex theme.
//! No user document/configuration is written; inherited screenshot values live
//! only in the isolated reference Workspace, never in the design's sparse fields.
use super::*;
use crate::window::greeting::tests::controller;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn settle() {
    let end = std::time::Instant::now() + Duration::from_millis(300);
    while std::time::Instant::now() < end {
        glib::MainContext::default().iteration(false);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn ready(this: &Workbench) {
    let end = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        settle();
        if !this.geometry_pending.get()
            && !this.full_session.input_pending.get()
            && !this.preview_loading.get()
            && !this.copy_loading.get()
            && !this.greeting_official_loading.get()
            && this.greeting.presentation.pixel_dimensions().is_some()
        {
            break;
        }
        assert!(
            std::time::Instant::now() < end,
            "complex preview did not settle"
        );
    }
}
fn rect(w: &impl IsA<gtk::Widget>, window: &impl IsA<gtk::Widget>) -> [f32; 4] {
    let b = w.compute_bounds(window).unwrap();
    [b.x(), b.y(), b.width(), b.height()]
}
fn adjustment(a: &gtk::Adjustment) -> [f64; 4] {
    [a.lower(), a.value(), a.upper(), a.page_size()]
}
fn screen_lines(this: &Workbench) -> Vec<String> {
    let terminal = &this.preview_terminal;
    let origin = preview_visible_origin(terminal)
        .expect("unique physical VTE origin")
        .0;
    (0..terminal.row_count())
        .map(|row| {
            terminal
                .text_range_format(vte::Format::Text, origin + row, 0, origin + row + 1, 0)
                .0
                .unwrap()
                .trim_end_matches(['\r', '\n'])
                .to_string()
        })
        .collect()
}
fn record(this: &Workbench, name: &str) {
    ready(this);
    super::preview_geometry_tests::capture(this, name);
    let t = &this.preview_terminal;
    let window = this.window();
    let image = this.full_session.image.get();
    let lines: Vec<_> = t.text_format(vte::Format::Text).unwrap().lines()
        .enumerate().filter(|(_,s)| !s.trim().is_empty())
        .map(|(row,s)| serde_json::json!({"row":row,"text":s,"first_column":s.chars().take_while(|c|*c==' ').count()})).collect();
    let data = serde_json::json!({"name":name,"font":this.typography_settings(),
        "font_scale":t.font_scale(),"cell":[t.char_width(),t.char_height()],
        "theme_layout":this.layout_settings(),"scale":this.full_session.scale.get(),
        "app":[window.width(),window.height()],"canvas":rect(&this.full_session.frame,&window),
        "surface":rect(&this.preview_terminal_shell,&window),
        "viewport":rect(&this.preview_terminal_viewport,&window),
        "terminal":rect(t,&window),"input":rect(&this.full_session.samples.entry,&window),
        "image":rect(&this.full_session.picture,&window),
        "image_cells":image.map(|i|[i.column,i.row,i.columns as usize,i.rows as usize]),
        "image_dimensions":this.greeting.presentation.pixel_dimensions(),
        "observation_columns":t.column_count(),"transcript_rows":this.full_session.rows.get(),
        "h":adjustment(&this.preview_terminal_viewport.hadjustment()),
        "v":adjustment(&this.preview_terminal_viewport.vadjustment()),
        "combined_v":adjustment(&this.preview_scroll.adjustment),
        "history_v":adjustment(&t.vadjustment().unwrap()),
        "cursor":t.cursor_position(),"follow":this.full_session.samples.state.borrow().current().follow,
        "lines":lines,"screen_lines":screen_lines(this)});
    std::fs::write(
        glib::user_cache_dir().join(format!("{name}-reach.json")),
        serde_json::to_vec_pretty(&data).unwrap(),
    )
    .unwrap();
    println!(
        "REACH {name}: h={:?} v={:?} grid={} rows={} image={image:?}",
        adjustment(&this.preview_terminal_viewport.hadjustment()),
        adjustment(&this.preview_scroll.adjustment),
        t.column_count(),
        this.full_session.rows.get()
    );
}
fn pointer(this: &Workbench, mode: &str, count: u32) {
    let b = this
        .preview_terminal_viewport
        .compute_bounds(&this.window())
        .unwrap();
    pointer_at(
        this,
        mode,
        [b.x() + b.width() / 2.0, b.y() + b.height() / 2.0],
        count,
    );
}
fn pointer_at(this: &Workbench, mode: &str, point: [f32; 2], count: u32) {
    let window = this.window();
    window.set_title(Some("TermiMochi point-to-edit test"));
    let (dx, dy) = window.surface_transform();
    let s = window.scale_factor() as f64;
    let mut child = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/preview-pointer-driver.py"
        ))
        .args([
            mode,
            &((point[0] as f64 + dx) * s).round().to_string(),
            &((point[1] as f64 + dy) * s).round().to_string(),
            &count.to_string(),
        ])
        .spawn()
        .unwrap();
    let end = std::time::Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        assert!(std::time::Instant::now() < end);
        settle();
    }
    assert!(child.wait().unwrap().success());
    ready(this);
}

fn covered_cell(this: &Workbench, row: usize, col: usize) -> bool {
    let window = this.window();
    let view = rect(&this.preview_terminal_viewport, &window);
    let term = rect(&this.preview_terminal, &window);
    let scale = this.full_session.scale.get() as f32;
    let cw = this.preview_terminal.char_width() as f32 * scale;
    let ch = this.preview_terminal.char_height() as f32 * scale;
    let x = term[0] + col as f32 * cw;
    let y = term[1] + row as f32 * ch;
    x >= view[0] - 0.5
        && x + cw <= view[0] + view[2] + 0.5
        && y >= view[1] - 0.5
        && y + ch <= view[1] + view[3] + 0.5
}
fn scan_visible_content(this: &Workbench, name: &str) {
    // Coverage is of allocated on-screen cells across real wheel gestures,
    // not merely the presence of strings in the transcript cache.
    let mut cells = Vec::new();
    // text_format joins soft-wrapped lines. Read each physical row separately
    // so a wide Prompt is not mistaken for unreachable cells beyond the grid.
    for (row, line) in screen_lines(this).iter().enumerate() {
        let mut col = 0;
        for glyph in line.graphemes(true) {
            let width = glyph.width();
            if !glyph.chars().all(char::is_whitespace) {
                for x in col..col + width {
                    cells.push((row, x, false));
                }
            }
            col += width;
        }
    }
    let image = this.full_session.image.get().unwrap();
    for y in image.row..image.row + image.rows as usize {
        for x in image.column..image.column + image.columns as usize {
            cells.push((y, x, false));
        }
    }
    let h = &this.preview_terminal_viewport.hadjustment();
    let v = &this.preview_scroll.adjustment;
    let hstep = (h.page_size() / this.preview_terminal.char_width() as f64 / 3.0 * 0.75)
        .floor()
        .max(1.0) as u32;
    let vstep = (v.page_size() / this.preview_terminal.char_height() as f64 / 3.0 * 0.75)
        .floor()
        .max(1.0) as u32;
    let mut frames = 0;
    for y in 0..16 {
        for x in 0..16 {
            record(this, &format!("{name}-scan-{y}-{x}"));
            frames += 1;
            for (row, col, seen) in &mut cells {
                *seen |= covered_cell(this, *row, *col);
            }
            let old = h.value();
            pointer(this, "scroll_right", hstep);
            if (h.value() - old).abs() < 0.5 {
                break;
            }
        }
        pointer(this, "scroll_left", 100);
        let old = v.value();
        pointer(this, "scroll_down", vstep);
        if (v.value() - old).abs() < 0.5 {
            break;
        }
    }
    let missing: Vec<_> = cells.iter().filter(|(_, _, seen)| !seen).collect();
    std::fs::write(glib::user_cache_dir().join(format!("{name}-coverage.json")),
        serde_json::to_vec_pretty(&serde_json::json!({"frames":frames,"text_and_image_cells":cells.len(),"missing":missing})).unwrap()).unwrap();
    assert!(
        missing.is_empty(),
        "Allocated content never visible after pointer traversal: {:?}",
        &missing[..missing.len().min(12)]
    );
}

#[test]
#[ignore = "isolated GTK + private complex theme/font: Fit proportions and pointer visibility"]
fn preview_complex_greeting_fit() {
    complex_greeting(0, "fit");
}
#[test]
#[ignore = "isolated GTK + private complex theme/font: fixed-grid overflow and input"]
fn preview_complex_greeting_fixed() {
    complex_greeting(1, "fixed");
}

#[test]
#[ignore = "isolated GTK + TERMIMOCHI_REACHABILITY_THEME: complex user Greeting, real pointer navigation"]
fn preview_complex_greeting_actual() {
    complex_greeting(4, "actual");
}
fn complex_greeting(mode: u32, name: &str) {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    let file = PathBuf::from(
        std::env::var_os("TERMIMOCHI_REACHABILITY_THEME").expect("private theme copy required"),
    );
    let bytes = std::fs::read(&file).unwrap();
    let design =
        crate::document_store::decode::<crate::design_document::DesignDocument>(&bytes).unwrap();
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.Reachability")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    present_with_preset(&app, None, root.path().join("font.json"));
    let window = app.active_window().unwrap();
    let this = controller(&window);
    let mut reference = this.workspace_snapshot();
    reference.typography =
        TypographySettings::from_font_name("CaskaydiaCove Nerd Font 11", 1.0, 1.0);
    reference.layout.columns = 120;
    reference.layout.rows = 36;
    reference.layout.content_padding = 6;
    reference.layout.scrollbar = false;
    reference.layout.window_spacing = 18;
    *this.typed.theme_reference.borrow_mut() = Some(reference);
    this.load_design(design);
    this.layout_module_button.set_active(true);
    window.set_default_size(1090, 790);
    ready(&this);
    this.diagnostic_surface
        .first_child()
        .and_downcast::<gtk::Expander>()
        .unwrap()
        .set_expanded(false);
    ready(&this);
    assert!(
        this.greeting_official_result
            .borrow()
            .as_ref()
            .is_some_and(|r| r.is_ok()),
        "native information actually rendered"
    );
    let baseline = this.committed_design().unwrap();
    {
        this.full_session.zoom.set_selected(mode);
        ready(&this);
        this.preview_scroll.start();
        this.preview_terminal_viewport.hadjustment().set_value(0.0);
        record(&this, &format!("{name}-initial"));
        assert_eq!(this.typography_settings().size, 16.0);
        assert_eq!(this.preview_terminal.font_scale(), 1.0);
        assert_eq!(
            (
                this.preview_terminal.char_width(),
                this.preview_terminal.char_height()
            ),
            (13, 25),
            "Use the copied screenshot font, not a fallback"
        );
        assert_eq!(
            (this.layout_settings().columns, this.layout_settings().rows),
            (120, 36)
        );
        assert_eq!(this.full_session.image.get().unwrap().columns, 32);
        if mode == 0 {
            assert!((this.full_session.scale.get() - 0.3327067669).abs() < 0.001);
            assert_eq!(this.preview_terminal.column_count(), 120);
        } else {
            assert_eq!(this.full_session.scale.get(), 1.0);
            assert!(this.full_session.navigation_v.is_visible());
            assert!(this.full_session.input_jump.is_visible());
        }
        if mode == 1 {
            assert!(this.full_session.navigation_h.is_visible());
        }
        if mode == 4 {
            assert_eq!(this.preview_terminal.column_count(), 38);
            let actual = this
                .preview_terminal
                .text_format(vte::Format::Text)
                .unwrap();
            let first_info = actual.lines().position(|l| !l.trim().is_empty()).unwrap();
            let image = this.full_session.image.get().unwrap();
            assert!(
                first_info >= image.row + image.rows as usize,
                "narrow view places information below the image"
            );
        }
        let native = this
            .greeting_official_result
            .borrow()
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .render(240);
        let clean = |s: &str| {
            crate::greeting_art::Artwork::parse(s)
                .unwrap()
                .plain
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        };
        let actual = this
            .preview_terminal
            .text_format(vte::Format::Text)
            .unwrap();
        assert!(
            clean(&actual).contains(&clean(&native)),
            "no native information tails dropped by observation wrapping"
        );
        scan_visible_content(&this, name);
        pointer(&this, "scroll_right", 100);
        record(&this, &format!("{name}-right"));
        pointer(&this, "scroll_down", 100);
        record(&this, &format!("{name}-bottom-right"));
        pointer(&this, "scroll_left", 100);
        record(&this, &format!("{name}-bottom-left"));
        this.full_session.samples.entry.set_text("help");
        ready(&this);
        record(&this, &format!("{name}-typing"));
        this.full_session.samples.entry.set_text("");
        // Return from history to the real GTK input using an actual pointer,
        // then type a finite sample command through XTest, not emit_activate.
        if this.full_session.input_jump.is_visible() {
            pointer(&this, "scroll_up", 100);
            let r = rect(&this.full_session.input_jump, &window);
            pointer_at(&this, "click", [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0], 1);
        }
        let entry = rect(&this.full_session.samples.entry, &window);
        let view = rect(&this.preview_terminal_viewport, &window);
        assert!(
            entry[0] >= view[0] - 0.5
                && entry[0] + 10.0 <= view[0] + view[2]
                && entry[1] >= view[1] - 0.5
                && entry[1] + entry[3] <= view[1] + view[3] + 0.5,
            "real input must be reachable: {entry:?} {view:?}"
        );
        pointer_at(
            &this,
            "sample_help",
            [entry[0] + 3.0, entry[1] + entry[3] / 2.0],
            1,
        );
        assert_eq!(
            this.full_session
                .samples
                .state
                .borrow()
                .current()
                .history
                .back()
                .map(String::as_str),
            Some("help")
        );
        record(&this, &format!("{name}-help-input"));
        // GTK's own text scrolling and the outer viewport must agree for a
        // draft wider than both. These are character indices, including CJK.
        let input = &this.full_session.samples.entry;
        input.set_text(&"中文 draft ".repeat(24));
        let text = input.delegate().and_downcast::<gtk::Text>().unwrap();
        for (position, label) in [(-1, "end"), (0, "start")] {
            input.set_position(position);
            ready(&this);
            let (caret, _) = text.compute_cursor_extents(input.position().max(0) as usize);
            let point = text
                .compute_point(&window, &gtk::graphene::Point::new(caret.x(), caret.y()))
                .unwrap();
            let view = rect(&this.preview_terminal_viewport, &window);
            assert!(
                point.x() >= view[0] - 1.0
                    && point.x() + 1.0 <= view[0] + view[2]
                    && point.y() >= view[1] - 1.0
                    && point.y() + caret.height() * this.full_session.scale.get() as f32
                        <= view[1] + view[3] + 1.0,
                "GTK caret at {label} must be visible: {point:?}, viewport={view:?}"
            );
            record(&this, &format!("{name}-long-draft-{label}"));
        }
        input.set_text("");
        ready(&this);
        if this.full_session.navigation_v.is_visible() {
            let bar = rect(&this.full_session.navigation_v, &window);
            pointer_at(
                &this,
                "click",
                [bar[0] + bar[2] / 2.0, bar[1] + bar[3] * 0.2],
                1,
            );
            assert!(
                !this.full_session.samples.state.borrow().current().follow,
                "scrollbar navigation must suspend input following"
            );
        } else {
            pointer(&this, "scroll_up", 100);
        }
        pointer(&this, "scroll_right", 5);
        let before_v = this.preview_scroll.adjustment.value();
        let before_h = this.preview_terminal_viewport.hadjustment().value();
        let background = this.model.borrow().active_variant;
        let old_color = this
            .model
            .borrow()
            .palette
            .variant(background)
            .unwrap()
            .get("Background")
            .unwrap();
        this.apply_color("Background", Rgb::new(225, 230, 240));
        ready(&this);
        assert!(
            (this.preview_scroll.adjustment.value() - before_v).abs() < 1.0,
            "color edit jumped vertical history"
        );
        assert!(
            (this.preview_terminal_viewport.hadjustment().value() - before_h).abs() < 1.0,
            "color edit jumped horizontal history"
        );
        this.apply_color("Background", old_color);
        ready(&this);
        record(&this, &format!("{name}-history-color-preserved"));
        assert_eq!(
            this.committed_design().unwrap(),
            baseline,
            "observation must not author the theme"
        );
        if mode == 4 {
            // Additional viewport-only stress: the fixed image occupancy is
            // wider than Actual's observation grid. No font/art/theme edits.
            let paned = crate::window::greeting::tests::descendants(window.upcast_ref())
                .into_iter()
                .find_map(|w| w.downcast::<gtk::Paned>().ok())
                .unwrap();
            paned.set_position(650);
            *this.full_session.samples.state.borrow_mut() = Default::default();
            this.sync_sample_controls();
            this.redraw_preview_contents();
            ready(&this);
            this.preview_scroll.start();
            record(&this, "actual-extra-narrow");
            assert!(this.preview_terminal.column_count() < 32);
            assert!(
                this.full_session.navigation_h.is_visible(),
                "fixed artwork occupancy must extend the scrollable canvas"
            );
            scan_visible_content(&this, "actual-extra-narrow");
            assert_eq!(this.committed_design().unwrap(), baseline);
        }
    }
    assert_eq!(std::fs::read(file).unwrap(), bytes);
    window.destroy();
}
