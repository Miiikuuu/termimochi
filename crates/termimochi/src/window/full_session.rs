//! One VTE transcript plus a cell-positioned GTK image. Only transient view state
//! lives here; Workspace, GreetingSettings and Presentation remain the owners.
use super::*;
use crate::greeting::{GreetingPart, Position};
use crate::greeting_output::Visual;
use unicode_width::UnicodeWidthStr;

/// Our transcript contains sanitized text, SGR, erase-line, home/clear and
/// cursor visibility CSI only (never arbitrary terminal programs). Measure the
/// complete feed, including Unicode wrapping, not the source artwork's pixels.
fn transcript_rows(text: &str, columns: usize) -> usize {
    let mut plain = String::with_capacity(text.len());
    let mut escape = false;
    let mut csi = false;
    for ch in text.chars() {
        if escape {
            escape = false;
            csi = ch == '[';
        } else if csi {
            if ('@'..='~').contains(&ch) {
                csi = false;
            }
        } else if ch == '\x1b' {
            escape = true;
        } else if ch != '\r' {
            plain.push(ch);
        }
    }
    plain
        .split('\n')
        .map(|line| 1 + line.width().saturating_sub(1) / columns.max(1))
        .sum::<usize>()
        .clamp(1, 1024)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ImageCells {
    pub column: usize,
    pub row: usize,
    pub columns: u32,
    pub rows: u32,
}

pub(super) struct FullSession {
    pub active: Cell<bool>,
    pub toolbar: gtk::Box,
    pub zoom: gtk::DropDown,
    pub play: gtk::ToggleButton,
    pub picture: gtk::Picture,
    pub overlay: gtk::Overlay,
    pub scaled: gtk::Fixed,
    pub scale: Cell<f64>,
    pub image: Cell<Option<ImageCells>>,
    pub rows: Cell<usize>,
    pub collecting: Cell<bool>,
    notice: gtk::Label,
}

impl FullSession {
    pub fn new() -> Self {
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toolbar.set_visible(false);
        let notice = gtk::Label::builder()
            .label("Design simulation · not terminal verification")
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        notice.set_tooltip_text(Some("Greeting, system information, prompt and simulated commands share this design canvas. GTK images do not prove Kitty/Sixel compatibility. Use Try Greeting for a real greeting trial."));
        let zoom = gtk::DropDown::from_strings(&["Fit", "100%", "75%", "50%"]);
        zoom.add_css_class("preview-scenario");
        zoom.update_property(&[gtk::accessible::Property::Label("Full Session zoom")]);
        zoom.set_tooltip_text(Some("Scale the entire scene for observation. Never changes Columns, font settings or saved state."));
        let play = gtk::ToggleButton::with_label("Play GIF");
        play.add_css_class("flat");
        play.set_tooltip_text(Some(
            "Shared Greeting playback · paused when system animations are disabled",
        ));
        toolbar.append(&notice);
        toolbar.append(&zoom);
        toolbar.append(&play);
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Fill)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Start)
            .can_shrink(true)
            .alternative_text("Greeting artwork in the complete design simulation")
            .build();
        picture.set_can_target(false);
        picture.set_visible(false);
        let overlay = gtk::Overlay::new();
        overlay.add_overlay(&picture);
        overlay.set_measure_overlay(&picture, false);
        overlay.set_clip_overlay(&picture, true);
        let scaled = gtk::Fixed::new();
        scaled.set_halign(gtk::Align::Start);
        scaled.set_valign(gtk::Align::Start);
        scaled.put(&overlay, 0.0, 0.0);
        Self {
            active: Cell::new(false),
            toolbar,
            zoom,
            play,
            picture,
            overlay,
            scaled,
            scale: Cell::new(1.0),
            image: Cell::new(None),
            rows: Cell::new(24),
            collecting: Cell::new(false),
            notice,
        }
    }
}

impl Workbench {
    pub(super) fn full_pixel_design(&self) -> bool {
        let settings = self.greeting.settings();
        self.full_session.active.get()
            && settings.enabled
            && settings.position != Position::Card
            && settings.editable_artwork.is_some()
            && settings.presentation.visual != Visual::Character
    }

    pub(super) fn connect_full_session(this: &Rc<Self>) {
        this.greeting
            .presentation
            .observe_pixels(&this.full_session.picture, &this.full_session.play);
        let weak = Rc::downgrade(this);
        this.full_session.zoom.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                this.refresh_full_transform();
            }
        });
        let weak = Rc::downgrade(this);
        this.preview_terminal_viewport
            .vadjustment()
            .connect_page_size_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.refresh_full_transform();
                }
            });
        // Preparation and playback reuse the existing worker and textures.
        // Only preparation changes geometry; individual frames never refeed VTE.
        let weak = Rc::downgrade(this);
        this.greeting.connect_pixels_prepared(move || {
            if let Some(this) = weak.upgrade()
                && this.full_session.active.get()
            {
                this.redraw_preview_contents();
            }
        });
    }

    pub(super) fn sync_full_session(&self) {
        let full = &self.full_session;
        let active = full.active.get();
        full.toolbar.set_visible(active);
        if active && full.overlay.child().is_none() {
            self.preview_terminal_viewport
                .set_child(None::<&gtk::Widget>);
            full.overlay.set_child(Some(&self.preview_terminal_canvas));
            self.preview_terminal_viewport.set_child(Some(&full.scaled));
        } else if !active && full.overlay.child().is_some() {
            self.preview_terminal_viewport
                .set_child(None::<&gtk::Widget>);
            full.overlay.set_child(None::<&gtk::Widget>);
            self.preview_terminal_viewport
                .set_child(Some(&self.preview_terminal_canvas));
            self.preview_scroll.set_scale(1.0);
        }
        if !active {
            full.picture.set_visible(false);
        }
    }

    pub(super) fn full_greeting_parts(
        &self,
        columns: usize,
    ) -> (Vec<(String, GreetingPart)>, Option<ImageCells>) {
        if !self.full_pixel_design() {
            return (self.greeting_parts_for_width(columns), None);
        }
        let settings = self.greeting.settings();
        let fallback = |message: &str| {
            let mut parts = self.greeting_parts_for_width(columns);
            parts.push((
                format!("{message} · showing character fallback\r\n\r\n"),
                GreetingPart::Artwork,
            ));
            (parts, None)
        };
        let Some(dimensions) = self.greeting.presentation.pixel_dimensions() else {
            let message = self
                .greeting
                .presentation
                .pixel_error()
                .map(|error| {
                    format!(
                        "Artwork unavailable: {}",
                        crate::preview_context::display_text(&error, 160)
                    )
                })
                .unwrap_or_else(|| "Preparing artwork…".into());
            return fallback(&message);
        };
        let cells = [
            self.preview_terminal.char_width().max(1),
            self.preview_terminal.char_height().max(1),
        ];
        let area = crate::greeting_image::pixel_export::area(
            dimensions,
            settings.presentation.columns,
            cells[0] as f64 / cells[1] as f64,
        );
        let Ok((art_columns, art_rows)) = area else {
            return fallback("Artwork occupancy unavailable; check Columns");
        };
        let blank = vec![" ".repeat(art_columns as usize); art_rows as usize].join("\n");
        let context = self.current_preview_context.borrow();
        let fallback = crate::greeting::GreetingContext::default();
        let facts = context.as_ref().map(|c| &c.greeting).unwrap_or(&fallback);
        let result = self.greeting_official_result.borrow();
        let native = match result.as_ref() {
            Some(Ok(output)) => Some(output.render(columns.clamp(12, 240) - 1)),
            Some(Err(_)) => {
                Some("Native information unavailable · see Greeting compatibility".into())
            }
            None => None,
        };
        let parts =
            settings.render_parts_with_artwork(facts, columns, native.as_deref(), Some(&blank));
        let mut image = None;
        let (mut row, mut column) = (0, 0);
        for (text, part) in &parts {
            let plain = crate::greeting_art::Artwork::parse(text)
                .map(|a| a.plain)
                .unwrap_or_else(|_| text.clone());
            if *part == GreetingPart::Artwork && image.is_none() {
                let width = plain.lines().next().unwrap_or("").width();
                image = Some(ImageCells {
                    column: column + width.saturating_sub(art_columns as usize),
                    row,
                    columns: art_columns,
                    rows: art_rows,
                });
            }
            for ch in plain.chars() {
                match ch {
                    '\n' => {
                        row += 1;
                        column = 0;
                    }
                    '\r' => column = 0,
                    ch => column += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0),
                }
            }
        }
        (parts, image)
    }

    pub(super) fn redraw_full_session(&self) {
        let columns = self.preview_terminal.column_count().max(12) as usize;
        let (parts, image) = self.full_greeting_parts(columns);
        self.full_session.image.set(image);
        for (text, part) in parts {
            let target = match part {
                GreetingPart::Artwork => PreviewTarget::GreetingArtwork,
                GreetingPart::Message => PreviewTarget::GreetingMessage,
                GreetingPart::Fields => PreviewTarget::GreetingFields,
                GreetingPart::Field(kind) => PreviewTarget::GreetingField(kind),
            };
            self.feed_scoped_preview(&text, Some(target));
        }
        self.feed_greeting_prompt();
        self.feed_preview("printf 'Ready · 你好 · 🌸\\n'\r\n".as_bytes());
        self.feed_preview("\x1b[32mReady\x1b[0m · 你好 · 🌸\r\n\r\n".as_bytes());
        self.redraw_prompt_preview();
        self.terminal_title
            .set_text(&format!("full session · {columns} cols"));
        self.preview_selector.set_visible(false);
        self.prompt_preview_selector
            .set_visible(self.preview_prompt_source.get() == 1);
        self.prompt_compare_selector.set_visible(false);
        self.feed_preview(PREVIEW_SHOW_CURSOR);
        // Feeds are queued by VTE. Size the same canvas before GTK processes
        // them, so a tall composition does not move its logo into scrollback.
        let text: String = self
            .preview_feed
            .borrow()
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect();
        self.full_session.rows.set(transcript_rows(&text, columns));
        self.refresh_terminal_geometry(&self.layout_settings());
        self.full_session.collecting.set(false);
        self.preview_terminal.reset(true, true);
        self.preview_terminal.feed(text.as_bytes());
    }

    pub(super) fn refresh_full_transform(&self) {
        let full = &self.full_session;
        if !full.active.get() {
            return;
        }
        let padding = self.layout_settings().content_padding;
        let cw = self.preview_terminal.char_width().max(1) as i32;
        let ch = self.preview_terminal.char_height().max(1) as i32;
        if let Some(image) = full.image.get() {
            full.picture
                .set_margin_start(padding + image.column as i32 * cw);
            full.picture.set_margin_top(padding + image.row as i32 * ch);
            full.picture
                .set_size_request(image.columns as i32 * cw, image.rows as i32 * ch);
            full.picture.set_visible(true);
        } else {
            full.picture.set_visible(false);
        }
        let logical_width = self.preview_terminal_canvas.width_request().max(1);
        let logical_height = self.preview_terminal_canvas.height_request().max(1);
        let available = self
            .preview_terminal_viewport
            .hadjustment()
            .page_size()
            .max(1.0);
        let scale = match full.zoom.selected() {
            1 => 1.0,
            2 => 0.75,
            3 => 0.5,
            _ => (available / f64::from(logical_width))
                .min(
                    self.preview_terminal_viewport
                        .vadjustment()
                        .page_size()
                        .max(1.0)
                        / f64::from(logical_height),
                )
                .clamp(0.001, 1.0),
        };
        if full.scale.replace(scale) != scale {
            full.scaled.set_child_transform(
                &full.overlay,
                Some(&gtk::gsk::Transform::new().scale(scale as f32, scale as f32)),
            );
            self.preview_scroll.set_scale(scale);
        }
        full.notice.set_text(
            if full.image.get().is_some_and(|image| {
                image.column + image.columns as usize
                    > self.preview_terminal.column_count() as usize
            }) {
                "Design simulation · artwork exceeds grid; reduce Columns or widen the grid"
            } else {
                "Design simulation · not terminal verification"
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::greeting::tests::{feed, project_controller as controller, settle};

    #[test]
    fn transcript_measurement_includes_unicode_wraps_but_not_csi() {
        assert_eq!(
            transcript_rows("\x1b[H\x1b[2J\x1b[32mhello\x1b[0m\r\n❯ ", 10),
            2
        );
        assert_eq!(transcript_rows("你好🌸abc\r\nnext", 4), 4);
        assert_eq!(transcript_rows("1234\r\n", 4), 2);
        assert_eq!(transcript_rows(&"x\n".repeat(3000), 1), 1024);
    }

    fn ready(this: &Workbench) {
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while this.preview_loading.get()
            || this.greeting.presentation.pixel_dimensions().is_none()
            || this.greeting_official_loading.get()
            || this.copy_loading.get()
        {
            assert!(
                std::time::Instant::now() < deadline,
                "preview failed to settle"
            );
            settle();
        }
        settle();
    }

    fn capture(window: &gtk::Window, name: &str) {
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
        println!("Full Session screenshot: {}", path.display());
    }

    fn transparent_artwork_background(this: &Workbench) -> [u8; 3] {
        let widget = &this.preview_terminal_shell;
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(widget)).snapshot(
            &snapshot,
            widget.width() as f64,
            widget.height() as f64,
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
        let mut data = vec![0; texture.width() as usize * texture.height() as usize * 4];
        texture.download(&mut data, texture.width() as usize * 4);
        let point = this
            .full_session
            .picture
            .compute_point(widget, &gtk::graphene::Point::new(4.0, 4.0))
            .unwrap();
        let offset = (point.y() as usize * texture.width() as usize + point.x() as usize) * 4;
        if cfg!(target_endian = "little") {
            [data[offset + 2], data[offset + 1], data[offset]]
        } else {
            [data[offset + 1], data[offset + 2], data[offset + 3]]
        }
    }

    #[test]
    #[ignore = "isolated GTK: Full Session GIF/text composition, all modules, zoom, save and captures"]
    fn full_session_live_composition_and_independent_navigation() {
        assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.FullSessionTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        let active = glib::user_config_dir().join("fastfetch/config.jsonc");
        std::fs::create_dir_all(active.parent().unwrap()).unwrap();
        std::fs::write(&active, "// untouched\n{}").unwrap();
        let shell = glib::home_dir().join(".bashrc");
        std::fs::write(&shell, "# untouched\n").unwrap();
        present_advanced_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        window.set_default_size(1280, 900);
        let this = controller(&window);
        let (mut settings, _) = crate::pixel_trial::tests::fixture(true);
        settings.presentation.visual = Visual::Animation;
        settings.presentation.columns = 32;
        settings.message = "Full Session · 你好 🌸".into();
        settings.preview_columns = 100;
        this.greeting.replace(settings.clone(), true);
        this.prompt_source_selector.set_selected(1);
        this.preview_scene_selector.set_selected(0);
        ready(&this);
        let full = &this.full_session;
        let generation = this.greeting.presentation.pixel_generation();
        assert!(full.active.get());
        assert!(full.picture.is_mapped());
        assert!(feed(&this).contains("Full Session"));
        assert!(feed(&this).contains("OS:"));
        assert!(feed(&this).contains("printf 'Ready"));
        assert!(feed(&this).contains("\x1b[32mReady"));
        assert_eq!(this.preview_terminal.column_count(), 100);
        let image = full.image.get().unwrap();
        assert_eq!(image.columns, 32);
        assert_eq!(
            full.picture.width_request(),
            32 * this.preview_terminal.char_width() as i32
        );
        let scheme = root.path().join("full.termimochi.json");
        this.save_workspace_path(scheme.clone(), this.workspace_snapshot())
            .unwrap();
        let saved = this.workspace_snapshot();
        capture(&window, "full-gif-before");
        full.play.set_active(true);
        for module in [
            &this.palette_module_button,
            &this.typography_module_button,
            &this.layout_module_button,
            &this.prompt_module_button,
            &this.greeting_module_button,
        ] {
            let old_texture = full.picture.paintable();
            let text = feed(&this);
            module.set_active(true);
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while full.picture.paintable() == old_texture {
                assert!(
                    std::time::Instant::now() < deadline,
                    "GIF must keep playing in Full"
                );
                settle();
            }
            assert_eq!(this.preview_scene_selector.selected(), 0);
            assert_eq!(
                feed(&this),
                text,
                "animation/navigation must not refeed the terminal"
            );
            assert_eq!(this.workspace_snapshot(), saved);
        }
        capture(&window, "full-gif-playing");
        assert_eq!(this.greeting.presentation.pixel_generation(), generation);
        for zoom in [1, 2, 3, 0] {
            full.zoom.set_selected(zoom);
            settle();
            assert_eq!(this.preview_terminal.column_count(), 100);
            assert_eq!(full.image.get(), Some(image));
            assert_eq!(this.workspace_snapshot(), saved);
            assert!(this.workspace_is_clean());
        }
        this.palette_module_button.set_active(true);
        this.apply_color("Background", Rgb::new(22, 28, 36));
        this.apply_color("Foreground", Rgb::new(235, 241, 245));
        settle();
        assert!(full.active.get() && full.picture.is_mapped());
        assert!(feed(&this).contains("Full Session"));
        this.typography_module_button.set_active(true);
        let previous_metrics = (
            this.preview_terminal.char_width(),
            this.preview_terminal.char_height(),
        );
        this.font_size_input.set_value(16.0);
        this.line_height_input.set_value(1.3);
        this.cell_width_input.set_value(1.2);
        settle();
        settle();
        assert_eq!(
            full.picture.width_request(),
            32 * this.preview_terminal.char_width() as i32
        );
        assert_eq!(this.preview_terminal.column_count(), 100);
        assert_ne!(
            (
                this.preview_terminal.char_width(),
                this.preview_terminal.char_height()
            ),
            previous_metrics
        );
        assert_eq!(
            this.greeting.presentation.pixel_generation(),
            generation,
            "palette, zoom and typography must not decode the image again"
        );
        this.layout_module_button.set_active(true);
        this.content_padding_input.set_value(24.0);
        settle();
        assert_eq!(
            full.picture.margin_start(),
            24 + full.image.get().unwrap().column as i32
                * this.preview_terminal.char_width() as i32
        );
        for scene in [1, 2, 3, 0, 3, 1, 0] {
            let before = this.workspace_snapshot();
            this.preview_scene_selector.set_selected(scene);
            settle();
            assert_eq!(full.active.get(), scene == 0);
            assert_eq!(this.workspace_snapshot(), before);
        }
        full.play.set_active(false);
        let gtk_settings = gtk::Settings::default().unwrap();
        let animations = gtk_settings.is_gtk_enable_animations();
        full.play.set_active(true);
        gtk_settings.set_gtk_enable_animations(false);
        settle();
        assert!(!full.play.is_active(), "respect reduced motion");
        let paused = full.picture.paintable();
        settle();
        assert_eq!(full.picture.paintable(), paused);
        gtk_settings.set_gtk_enable_animations(animations);
        this.prompt_module_button.set_active(true);
        let previous_text = feed(&this);
        this.prompt_layout_selector.set_selected(1);
        settle();
        assert_ne!(feed(&this), previous_text);
        assert!(full.active.get());
        this.prompt_compare_selector.set_selected(1);
        assert_eq!(
            this.preview_prompt_settings(),
            *this.prompt_settings.borrow(),
            "Full must not inherit the dedicated Prompt scene's Original comparison"
        );
        for (width, height) in [(1024, 700), (1280, 900)] {
            window.set_default_size(width, height);
            this.preview_scroll.start();
            settle();
            settle();
            let bounds = full
                .overlay
                .compute_bounds(&this.preview_terminal_viewport)
                .unwrap();
            assert!(
                bounds.width() <= this.preview_terminal_viewport.width() as f32 + 2.0,
                "Fit scales the complete scene: {bounds:?}"
            );
            assert!(
                bounds.height() <= this.preview_terminal_viewport.height() as f32 + 2.0,
                "Fit includes the entire transcript: {bounds:?}"
            );
            let history = this.preview_terminal.vadjustment().unwrap();
            assert!(
                history.upper() - history.page_size() - history.lower() < 1.0,
                "image/text scrollback: upper={} page={} lower={} VTE={} rows={} cursor={:?} feed={:?}",
                history.upper(),
                history.page_size(),
                history.lower(),
                this.preview_terminal.row_count(),
                full.rows.get(),
                this.preview_terminal.cursor_position(),
                feed(&this)
            );
            window.set_title(Some("TermiMochi point-to-edit test"));
            let path = glib::user_cache_dir().join(format!("full-{width}x{height}.png"));
            gtk::prelude::WidgetExt::display(&window).flush();
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
            println!("Full Session screenshot: {}", path.display());
        }
        this.save_workspace_path(scheme.clone(), this.workspace_snapshot())
            .unwrap();
        let before = this.workspace_snapshot();
        this.open_workspace_path(&scheme);
        assert_eq!(
            this.preview_scene_selector.selected(),
            3,
            "open selects enabled Greeting, not the previously observed Full"
        );
        this.preview_scene_selector.set_selected(0);
        ready(&this);
        assert_eq!(this.workspace_snapshot(), before);
        assert!(this.greeting.presentation.verified.borrow().is_none());
        // Character and static pixel compositions use the same saved settings
        // and text/inspection paths as GIF; no source is required for character.
        let mut character = crate::greeting::GreetingSettings {
            enabled: true,
            ..Default::default()
        };
        character.message = "Character greeting".into();
        this.greeting.replace(character, true);
        settle();
        assert!(full.active.get() && !full.picture.is_visible());
        assert!(feed(&this).contains("Character greeting") && feed(&this).contains("OS:"));
        assert!(feed(&this).contains("printf 'Ready"));
        capture(&window, "full-character");
        let (mut still, _) = crate::pixel_trial::tests::fixture(false);
        still.presentation.visual = Visual::Image;
        still.presentation.columns = 24;
        this.greeting.replace(still.clone(), true);
        ready(&this);
        assert!(full.picture.is_mapped());
        assert!(!full.play.is_visible());
        assert_eq!(transparent_artwork_background(&this), [22, 28, 36]);
        let prepared_generation = this.greeting.presentation.pixel_generation();
        this.apply_color("Background", Rgb::new(248, 249, 250));
        settle();
        assert_eq!(transparent_artwork_background(&this), [248, 249, 250]);
        assert_eq!(
            this.greeting.presentation.pixel_generation(),
            prepared_generation
        );
        this.apply_color("Background", Rgb::new(22, 28, 36));
        settle();
        let native = full.picture.native().unwrap();
        let (dx, dy) = native.surface_transform();
        let native_widget = native.dynamic_cast::<gtk::Widget>().unwrap();
        let point = full
            .picture
            .compute_point(
                &native_widget,
                &gtk::graphene::Point::new(
                    full.picture.width() as f32 / 2.0,
                    full.picture.height() as f32 / 2.0,
                ),
            )
            .unwrap();
        let hit = this
            .preview_hit_at(f64::from(point.x()) + dx, f64::from(point.y()) + dy)
            .unwrap();
        assert_eq!(
            hit.target,
            PreviewTarget::GreetingArtwork,
            "Inspect uses the transformed picture, not background"
        );
        capture(&window, "full-static");
        assert_eq!(full.image.get().unwrap().columns, 24);
        assert!(feed(&this).contains("OS:") && feed(&this).contains("printf 'Ready"));
        still.presentation.columns = 40;
        this.greeting.replace(still, true);
        ready(&this);
        assert_eq!(
            full.image.get().unwrap().columns,
            40,
            "Columns changes occupancy"
        );
        let before_inspect = this.workspace_snapshot();
        this.inspect_preview_target(PreviewTarget::Typography);
        assert!(full.active.get());
        assert_eq!(this.workspace_snapshot(), before_inspect);
        for position in [Position::Top, Position::Right, Position::Left] {
            let mut settings = this.greeting.settings();
            settings.position = position;
            this.greeting.replace(settings, true);
            ready(&this);
            let placement = full.image.get().unwrap();
            assert_eq!(placement.column > 0, position == Position::Right);
            assert_eq!(placement.row, 0);
        }
        // At 100% both image and transcript move together in the existing local
        // scroll canvas; the editor and preview header stay outside that range.
        full.zoom.set_selected(1);
        settle();
        let separation = || {
            let image = full.picture.compute_bounds(&this.inspect_layer).unwrap();
            let text = this
                .preview_terminal
                .compute_bounds(&this.inspect_layer)
                .unwrap();
            image.y() - text.y()
        };
        let before_scroll = separation();
        this.preview_scroll.adjustment.set_value(80.0);
        settle();
        assert!((separation() - before_scroll).abs() < 1.0);
        let mut invalid_pixels = this.greeting.settings();
        invalid_pixels.presentation.columns = 144; // valid character size, not pixel size
        this.greeting.replace(invalid_pixels, true);
        ready(&this);
        assert!(full.image.get().is_none());
        assert!(feed(&this).contains("occupancy unavailable"));
        assert!(feed(&this).contains("OS:") && feed(&this).contains("printf 'Ready"));
        let mut disabled = this.greeting.settings();
        disabled.enabled = false;
        this.greeting.replace(disabled, true);
        settle();
        assert!(full.active.get() && full.image.get().is_none());
        assert!(feed(&this).contains("printf 'Ready"));
        assert!(!feed(&this).contains("OS:"));
        this.starship_editor
            .begin_detached(Some("format = '[CURRENT](bold green) '\n".into()))
            .unwrap();
        this.activate_prompt_preview(0);
        this.schedule_copy_preview();
        ready(&this);
        *this.prompt_initial_ansi.borrow_mut() = "OUTDATED-FROZEN-PROMPT ".into();
        this.copy_initial_sample.borrow_mut().take();
        this.copy_scene_history.borrow_mut().clear();
        this.redraw_preview_contents();
        settle();
        assert!(!feed(&this).contains("OUTDATED-FROZEN-PROMPT"));
        assert!(
            feed(&this).matches("CURRENT").count() >= 2,
            "Full uses current detached Starship before and after the simulated command"
        );
        assert_eq!(std::fs::read_to_string(active).unwrap(), "// untouched\n{}");
        assert_eq!(std::fs::read_to_string(shell).unwrap(), "# untouched\n");
        window.destroy();
    }
}
