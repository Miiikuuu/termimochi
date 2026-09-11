//! Deterministic cross-module abuse cases, not just one-control happy paths.
use super::greeting::tests::{project_controller as controller, settle};
use super::*;

#[test]
#[ignore = "requires isolated GTK/VTE; extreme legal controls and rapid module switching at 1x/2x"]
fn extreme_settings_and_rapid_navigation_keep_preview_bounded_and_recoverable() {
    adw::init().unwrap();
    gio::resources_register_include!("termimochi.gresource").unwrap();
    gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
        .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
    let app = adw::Application::builder()
        .application_id("io.github.miiikuuu.termimochi.ExtremeSettingsTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let root = tempfile::tempdir().unwrap();
    present_advanced_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
    let window = app.active_window().unwrap();
    let this = controller(&window);
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while this.preview_loading.get() {
        assert!(std::time::Instant::now() < deadline);
        settle();
    }
    let original_palette = this.model.borrow().palette.clone();
    let mut art = crate::greeting::GreetingSettings {
        enabled: true,
        logo: crate::greeting::Logo::Custom,
        custom_logo: (0..96)
            .map(|row| format!("{row:02}{}", "x".repeat(158)))
            .collect::<Vec<_>>()
            .join("\n"),
        message: "中🙂".repeat(80),
        ..Default::default()
    };
    art.validate().unwrap();
    for step in 0..48 {
        // Deliberately batch notifications before GTK allocates or a worker
        // responds, as when dragging several sliders or switching pages fast.
        this.font_size_input
            .set_value(if step & 1 == 0 { 8.0 } else { 24.0 });
        this.line_height_input
            .set_value(if step & 2 == 0 { 1.0 } else { 2.0 });
        this.cell_width_input
            .set_value(if step & 4 == 0 { 1.0 } else { 2.0 });
        this.content_padding_input
            .set_value(if step & 8 == 0 { 0.0 } else { 24.0 });
        this.window_spacing_input
            .set_value(if step & 16 == 0 { 0.0 } else { 32.0 });
        this.column_count_input
            .set_value(if step & 1 == 0 { 40.0 } else { 120.0 });
        this.row_count_input
            .set_value(if step & 2 == 0 { 8.0 } else { 36.0 });
        this.fit_preview_switch.set_active(step % 2 == 0);
        this.scrollbar_switch.set_active(step % 3 == 0);
        this.tab_bar_switch.set_active(step % 4 == 0);
        this.cursor_shape_selector.set_selected(step % 3);
        this.cursor_blink_selector.set_selected((step + 1) % 3);
        art.preview_columns = [0, 80, 100, 120][step as usize % 4];
        art.position = crate::greeting::Position::ALL[step as usize % 4];
        art.gap = if step % 2 == 0 { 0 } else { 8 };
        this.greeting.replace(art.clone(), true);
        for action in [
            "show-layout",
            "show-typography",
            "show-prompt",
            "show-greeting",
        ] {
            gio::prelude::ActionGroupExt::activate_action(&this.window(), action, None);
        }
        window.set_default_size(
            if step % 2 == 0 { 1024 } else { 1440 },
            if step % 3 == 0 { 700 } else { 950 },
        );
        this.show_greeting_preview();
        settle();
        assert!(!this.updating_geometry.get());
        assert!((12..=240).contains(&this.preview_terminal.column_count()));
        assert!((8..=96).contains(&this.preview_terminal.row_count()));
        assert!(this.preview_terminal.width_request() > 0);
        assert!(this.preview_terminal.height_request() > 0);
        this.typography_settings().validate().unwrap();
        this.layout_settings().validate().unwrap();
        this.greeting.settings().validate().unwrap();
        let scroll = &this.preview_scroll.adjustment;
        assert!(scroll.upper().is_finite() && scroll.value().is_finite());
        assert!(scroll.value() >= scroll.lower());
        scroll.set_value((scroll.upper() - scroll.page_size()).max(0.0));
        scroll.set_value(0.0);
        assert_eq!(this.model.borrow().palette, original_palette);
    }
    // Restore a compact, unmistakable scene: no residual large-grid artwork,
    // dropped row, stuck update guard or unresponsive controls may survive.
    this.font_size_input.set_value(11.0);
    this.line_height_input.set_value(1.0);
    this.cell_width_input.set_value(1.0);
    art.custom_logo = "RECOVERED\nSECOND ROW".into();
    art.message.clear();
    art.position = crate::greeting::Position::Top;
    art.preview_columns = 80;
    this.greeting.replace(art, true);
    this.show_greeting_preview();
    settle();
    this.preview_scroll.adjustment.set_value(0.0);
    settle();
    let text = this
        .preview_terminal
        .text_format(vte::Format::Text)
        .unwrap();
    assert!(text.contains("RECOVERED\nSECOND ROW"), "{text:?}");
    assert!(!text.contains("xxxxxxxx"));
    assert_eq!(this.preview_terminal.column_count(), 80);
    window.destroy();
}
