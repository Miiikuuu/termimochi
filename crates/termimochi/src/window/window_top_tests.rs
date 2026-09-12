use super::*;
use crate::window::greeting::tests::{controller, settle};

fn ready(this: &Workbench) {
    let end = std::time::Instant::now() + Duration::from_secs(30);
    while this.preview_loading.get() || this.copy_loading.get() {
        assert!(std::time::Instant::now() < end);
        settle();
    }
    settle();
}

#[test]
#[ignore = "isolated GTK: Window Top editing, sparse save/reopen, Undo, capabilities, independent Full scene and window colors"]
fn window_top_native_edit_save_reopen_and_scope() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    assert!(!matches!(
        std::env::var("DISPLAY").as_deref(),
        Ok(":0" | ":1") | Err(_)
    ));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.WindowTopTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("Colors.conf");
    std::fs::write(&path, "background #e8e5de\nforeground #31353c\n").unwrap();
    present_with_preset(&app, Some(path), tmp.path().join("unused.json"));
    let main = app.active_window().unwrap();
    let this = controller(&main);
    ready(&this);
    let original = this.committed_design().unwrap();
    assert!(original.theme.as_ref().unwrap().layout.is_empty());
    let scene = this.preview_scene_selector.selected();
    this.layout_module_button.set_active(true);
    settle();
    assert_eq!(this.preview_scene_selector.selected(), scene);
    assert!(
        !this.window_top.title_row.is_sensitive(),
        "X11 may not advertise native titlebar recoloring"
    );
    this.window_top.edge.set_selected(0);
    settle();
    let changed = this.committed_design().unwrap();
    assert_eq!(changed.theme.as_ref().unwrap().layout.len(), 1);
    this.theme_undo(false);
    ready(&this);
    assert_eq!(this.layout_settings().tab_edge, TabEdge::Bottom);
    this.theme_undo(true);
    ready(&this);
    assert_eq!(this.layout_settings().tab_edge, TabEdge::Top);
    this.window_top.style.set_selected(3);
    this.window_top.minimum.set_value(1.0);
    this.window_top.colors[0]
        .picker
        .inspection_field()
        .set_text("#FAFAFA");
    this.window_top.colors[1]
        .picker
        .inspection_field()
        .set_text("#224466");
    this.window_top.colors[2]
        .picker
        .inspection_field()
        .set_text("#DDDDDD");
    this.window_top.colors[3]
        .picker
        .inspection_field()
        .set_text("#735421");
    settle();
    let saved = this.committed_design().unwrap();
    let native = saved
        .theme_kitty_configuration(&this.workspace_snapshot())
        .unwrap();
    assert!(
        native.contains("tab_bar_style powerline")
            && native.contains("active_tab_background #224466"),
        "{native}\nLayout: {:?}\nTheme: {:?}",
        this.layout_settings(),
        saved.theme
    );
    assert!(!native.contains("wayland_titlebar_color") && !native.contains("font_size"));
    let save = tmp.path().join("Saved.termimochi-design.json");
    DocumentStore::open(save.clone())
        .unwrap()
        .save(&saved)
        .unwrap();
    this.window_top.colors[0]
        .picker
        .inspection_field()
        .set_text("not-a-color");
    assert!(this.committed_layout().is_err());
    this.window_top.colors[0].picker.discard_draft();
    assert_eq!(this.preview_scene_selector.selected(), scene);
    let prev = this.top_preview.tabs.prev_sibling();
    for _ in 0..15 {
        this.refresh_window_top();
    }
    assert_eq!(
        this.top_preview.tabs.prev_sibling(),
        prev,
        "redraw must not reorder the preview repeatedly"
    );
    this.window_top.root.grab_focus();
    this.inspect_preview_target(PreviewTarget::TabBar);
    settle();
    crate::window::typed_tests::capture(&main, "window-top-editor");
    this.window_top.colors[3].button.grab_focus();
    settle();
    crate::window::typed_tests::capture(&main, "window-top-tab-colors");
    present_with_preset(&app, Some(save), tmp.path().join("unused-2.json"));
    let other = app.active_window().unwrap();
    let second = controller(&other);
    ready(&second);
    assert_eq!(second.committed_design().unwrap().id, saved.id);
    assert_eq!(second.layout_settings(), this.layout_settings());
    second.layout_module_button.set_active(true);
    second.window_top.edge.set_selected(1);
    settle();
    assert_eq!(this.layout_settings().tab_edge, TabEdge::Top);
    assert_eq!(
        second.top_preview.tabs.clone().upcast::<gtk::Widget>(),
        second.preview_terminal_shell.last_child().unwrap()
    );
    crate::window::typed_tests::capture(&other, "window-top-reopened-bottom");
    let ptyxis = crate::design_document::DesignDocument::new_theme(
        crate::design_document::TargetHint::Ptyxis,
        "Ptyxis",
        true,
    );
    second.load_design(ptyxis);
    ready(&second);
    assert!(!second.window_top.edge.is_sensitive());
    assert!(!second.window_top.title_row.is_sensitive());
    assert!(second.window_top.ptyxis_row.is_sensitive());
    assert!(second.top_preview.title.is_visible());
    assert!(!second.top_preview.tabs.is_visible());
    other.close();
    main.close();
    settle();
}

fn header_pixel(this: &Workbench, expected: Rgb) {
    settle();
    let widget = &this.top_preview.title;
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(widget)).snapshot(
        &snapshot,
        f64::from(widget.width()),
        f64::from(widget.height()),
    );
    let texture = widget
        .native()
        .unwrap()
        .renderer()
        .unwrap()
        .render_texture(snapshot.to_node().unwrap(), None);
    let width = texture.width() as usize;
    let mut bytes = vec![0; width * texture.height() as usize * 4];
    texture.download(&mut bytes, width * 4);
    let pos = (3 * width + width / 2) * 4;
    assert_eq!(
        Rgb::new(bytes[pos + 2], bytes[pos + 1], bytes[pos]),
        expected
    );
}

#[test]
#[ignore = "isolated GTK: Ptyxis header shared palette edits, pixels, variants, undo, save, scope and window isolation"]
fn window_top_ptyxis_shared_palette_preview_and_save() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.PtyxisTopTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    present_with_preset(&app, None, tmp.path().join("unused.json"));
    let main = app.active_window().unwrap();
    let this = controller(&main);
    this.load_design(crate::design_document::DesignDocument::new_theme(
        crate::design_document::TargetHint::Ptyxis,
        "Ptyxis Header",
        true,
    ));
    ready(&this);
    let untouched = this.committed_design().unwrap();
    assert!(untouched.theme.as_ref().unwrap().colors.is_empty());
    this.layout_module_button.set_active(true);
    let scene = this.preview_scene_selector.selected();
    assert!(this.window_top.ptyxis_row.is_sensitive());
    assert!(!this.window_top.title_row.is_visible());
    assert!(!this.window_top.style.is_sensitive());
    this.window_top.ptyxis_colors[0]
        .picker
        .inspection_field()
        .set_text("#224466");
    this.window_top.ptyxis_colors[1]
        .picker
        .inspection_field()
        .set_text("#FAEEDD");
    this.settle_active_edit();
    header_pixel(&this, Rgb::new(34, 68, 102));
    let saved = this.committed_design().unwrap();
    let theme = saved.theme.as_ref().unwrap();
    assert_eq!(theme.colors.len(), 2);
    assert_eq!(theme.colors["TitlebarBackground"], "#224466");
    assert!(
        theme.layout.is_empty(),
        "header colors must not become a parallel Layout write set"
    );
    this.theme_undo(false);
    ready(&this);
    assert_ne!(this.committed_design().unwrap(), saved);
    this.theme_undo(true);
    ready(&this);
    assert_eq!(this.committed_design().unwrap(), saved);
    this.palette_module_button.set_active(true);
    let scope = crate::window::greeting::tests::descendants(this.color_targets.root.upcast_ref())
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::DropDown>().ok())
        .find(|d| {
            d.model()
                .and_then(|m| m.downcast::<gtk::StringList>().ok())
                .is_some_and(|m| m.string(4).as_deref() == Some("Ptyxis Window Top"))
        })
        .unwrap();
    scope.set_selected(4);
    assert_eq!(
        this.selected_color_key.borrow().as_str(),
        "TitlebarBackground"
    );
    this.color_picker.inspection_field().set_text("#735421");
    this.settle_active_edit();
    assert_eq!(this.window_top.ptyxis_colors[0].value.get(), [115, 84, 33]);
    header_pixel(&this, Rgb::new(115, 84, 33));
    this.change_variant(Variant::Dark);
    this.window_top.ptyxis_colors[0]
        .picker
        .inspection_field()
        .set_text("#182838");
    this.settle_active_edit();
    header_pixel(&this, Rgb::new(24, 40, 56));
    this.change_variant(Variant::Light);
    header_pixel(&this, Rgb::new(115, 84, 33));
    this.window_top.ptyxis_colors[0]
        .picker
        .inspection_field()
        .set_text("invalid");
    this.refresh_preview();
    assert!(this.committed_design().is_err());
    this.window_top.discard_palette();
    assert_eq!(this.preview_scene_selector.selected(), scene);
    this.tab_bar_switch.set_active(true);
    // Full no longer paints two decorative tabs for a one-tab session.
    assert!(this.full_session.samples.state.borrow_mut().add());
    this.sync_sample_controls();
    for scene in 0..4 {
        this.preview_scene_selector.set_selected(scene);
        ready(&this);
        assert!(this.top_preview.tabs.is_visible());
        header_pixel(&this, Rgb::new(115, 84, 33));
    }
    this.preview_scene_selector.set_selected(0);
    this.inspect_preview_target(PreviewTarget::TitleBar);
    this.window_top.ptyxis_colors[1].button.grab_focus();
    settle();
    crate::window::typed_tests::capture(&main, "window-top-ptyxis-editor");
    let save = tmp.path().join("Ptyxis.termimochi-design.json");
    let document = this.committed_design().unwrap();
    let partial_plan = this.scheme_plan(&this.workspace_snapshot());
    assert!(
        partial_plan
            .items
            .iter()
            .find(|i| i.id == "palette")
            .unwrap()
            .action
            .is_none(),
        "two header overrides cannot silently install reference ANSI colors"
    );
    // Same explicit base inclusion offered by Use Theme's review dialog.
    let mut included = document;
    included.components.palette = Some(crate::design_document::PaletteComponent {
        source: this.workspace_snapshot().palette,
        light: true,
    });
    this.replace_theme(included, true);
    ready(&this);
    let document = this.committed_design().unwrap();
    // The same Use Theme plan installs both variants through the existing
    // private installer and restoration machinery, not a Layout side channel.
    let plan = this.scheme_plan(&this.workspace_snapshot());
    let selected: Vec<_> = plan.items.iter().map(|item| item.id == "palette").collect();
    assert!(selected.iter().any(|v| *v));
    assert!(
        glib::user_data_dir()
            .to_string_lossy()
            .contains("termimochi-regression-")
    );
    let (directory, mut report) = plan.apply(&selected).unwrap();
    let result = report
        .items
        .iter()
        .find(|item| item.id == "palette")
        .unwrap();
    assert_eq!(result.status, crate::scheme_apply::Status::NotEnabled);
    let installed = termimochi_core::PtyxisPalette::from_text(
        &std::fs::read_to_string(result.path.as_ref().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        installed
            .variant(Variant::Light)
            .unwrap()
            .get("TitlebarBackground"),
        Some(Rgb::new(115, 84, 33))
    );
    assert_eq!(
        installed
            .variant(Variant::Dark)
            .unwrap()
            .get("TitlebarBackground"),
        Some(Rgb::new(24, 40, 56))
    );
    report.restore(&directory).unwrap();
    assert_eq!(
        report
            .items
            .iter()
            .find(|item| item.id == "palette")
            .unwrap()
            .status,
        crate::scheme_apply::Status::Restored
    );
    DocumentStore::open(save.clone())
        .unwrap()
        .save(&document)
        .unwrap();
    present_with_preset(&app, Some(save), tmp.path().join("unused2.json"));
    let other = app.active_window().unwrap();
    let second = controller(&other);
    ready(&second);
    assert_eq!(second.committed_design().unwrap().id, document.id);
    header_pixel(&second, Rgb::new(115, 84, 33));
    second.window_top.ptyxis_colors[0]
        .picker
        .inspection_field()
        .set_text("#667788");
    header_pixel(&this, Rgb::new(115, 84, 33));
    second.load_design(crate::design_document::DesignDocument::new_theme(
        crate::design_document::TargetHint::Kitty,
        "Kitty",
        true,
    ));
    ready(&second);
    let before = second.committed_design().unwrap();
    second.window_top.ptyxis_colors[0]
        .picker
        .inspection_field()
        .set_text("#ABCDEF");
    assert_eq!(
        second.committed_design().unwrap(),
        before,
        "hidden Ptyxis controls cannot mutate Kitty"
    );
    header_pixel(&this, Rgb::new(115, 84, 33));
    other.close();
    main.close();
    settle();
}
