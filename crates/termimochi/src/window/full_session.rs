//! One VTE transcript plus a cell-positioned GTK image. Only transient view state
//! lives here; Workspace, GreetingSettings and Presentation remain the owners.
use super::*;
use crate::greeting::{GreetingPart, Position};
use crate::greeting_output::Visual;
use unicode_width::UnicodeWidthStr;

/// Our transcript contains sanitized text, SGR, erase-line, home/clear and
/// cursor visibility CSI only (never arbitrary terminal programs). Measure the
/// complete feed, including Unicode wrapping, not the source artwork's pixels.
pub(super) fn transcript_rows(text: &str, columns: usize) -> usize {
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

pub(super) struct GreetingProjection {
    pub parts: Vec<(String, GreetingPart)>,
    pub image: Option<ImageCells>,
    pub character_fallback: bool,
}

pub(super) struct FullSession {
    pub active: Cell<bool>,
    pub toolbar: gtk::Box,
    pub zoom: gtk::DropDown,
    pub play: gtk::ToggleButton,
    pub picture: gtk::Picture,
    pub overlay: gtk::Overlay,
    pub frame: preview_frame::PreviewFrame,
    pub scale: Cell<f64>,
    pub image: Cell<Option<ImageCells>>,
    pub rows: Cell<usize>,
    pub collecting: Cell<bool>,
    pub samples: interactive_samples::Samples,
    pub rendered: RefCell<String>,
    pub rendered_geometry: Cell<(i64, i64, i64)>,
    pub extra_images: RefCell<Vec<(gtk::Picture, ImageCells)>>,
    pub input_pending: Cell<bool>,
    pub character_fallback: Cell<bool>,
    pub(super) notice: gtk::Label,
}

impl FullSession {
    pub fn new() -> Self {
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toolbar.set_visible(false);
        let notice = gtk::Label::builder()
            .label("Interactive samples · no local commands")
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        notice.set_tooltip_text(Some("Greeting, system information, prompt and simulated commands share this design canvas. GTK images do not prove Kitty/Sixel compatibility. Use Try Greeting for a real greeting trial."));
        let zoom = gtk::DropDown::from_strings(&[
            "Fit window",
            "100% · fixed grid",
            "75%",
            "50%",
            "Actual size",
        ]);
        zoom.add_css_class("preview-scenario");
        zoom.set_selected(4);
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
        let samples = interactive_samples::Samples::new();
        overlay.add_overlay(&samples.input_row);
        overlay.set_measure_overlay(&samples.input_row, false);
        overlay.set_clip_overlay(&samples.input_row, true);
        Self {
            active: Cell::new(false),
            toolbar,
            zoom,
            play,
            picture,
            overlay,
            frame: preview_frame::PreviewFrame::new(),
            scale: Cell::new(1.0),
            image: Cell::new(None),
            rows: Cell::new(24),
            collecting: Cell::new(false),
            samples,
            rendered: RefCell::new(String::new()),
            rendered_geometry: Cell::new((0, 0, 0)),
            extra_images: RefCell::new(Vec::new()),
            input_pending: Cell::new(false),
            character_fallback: Cell::new(false),
            notice,
        }
    }
}

impl Workbench {
    pub(super) fn full_pixel_design(&self) -> bool {
        let settings = self.greeting.settings();
        self.full_session.active.get()
            && self.typed.target.get() != Some(crate::design_document::TargetHint::Ptyxis)
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
        this.full_session.frame.connect_geometry_changed(move || {
            if let Some(this) = weak.upgrade() {
                this.schedule_preview_reflow();
            }
        });
        let weak = Rc::downgrade(this);
        this.full_session.zoom.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                this.refresh_full_transform();
                this.schedule_preview_reflow();
            }
        });
        let weak = Rc::downgrade(this);
        this.preview_terminal_viewport
            .vadjustment()
            .connect_page_size_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    // Adjustment notifications run inside GTK allocation.
                    // Defer size requests, or GTK may drop their resize and
                    // leave a smaller scroll range than the scaled transcript.
                    this.schedule_preview_reflow();
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
        self.preview_terminal_shell
            .set_spacing(if active { 0 } else { 8 });
        if active {
            self.preview_terminal_shell.add_css_class("sample-window");
        } else {
            self.preview_terminal_shell
                .remove_css_class("sample-window");
        }
        #[cfg(feature = "native-preview")]
        let show_toolbar = active && !self.native_terminal.mode.is_active();
        #[cfg(not(feature = "native-preview"))]
        let show_toolbar = active;
        full.toolbar.set_visible(show_toolbar);
        let header = self
            .terminal_title
            .parent()
            .and_then(|p| p.parent())
            .and_downcast::<gtk::Box>()
            .unwrap();
        if active && self.preview_selector.parent().as_ref() != Some(full.toolbar.upcast_ref()) {
            header.remove(&self.preview_selector);
            full.toolbar.append(&self.preview_selector);
        } else if !active
            && self.preview_selector.parent().as_ref() == Some(full.toolbar.upcast_ref())
        {
            full.toolbar.remove(&self.preview_selector);
            header.append(&self.preview_selector);
        }
        header.set_visible(!active);
        full.samples.tabs.set_visible(active);
        full.samples.input_row.set_visible(active);
        if active && full.overlay.child().is_none() {
            let navigating = self.navigating_preview.replace(true);
            if self.preview_selector.selected() == 8 {
                self.preview_selector.set_selected(0);
            }
            self.navigating_preview.set(navigating);
            self.preview_terminal_viewport
                .set_child(None::<&gtk::Widget>);
            full.overlay.set_child(Some(&self.preview_terminal_canvas));
            self.preview_terminal_viewport
                .set_child(Some(&full.overlay));
        } else if !active && full.overlay.child().is_some() {
            self.preview_terminal_viewport
                .set_child(None::<&gtk::Widget>);
            full.overlay.set_child(None::<&gtk::Widget>);
            self.preview_terminal_viewport
                .set_child(Some(&self.preview_terminal_canvas));
            self.preview_scroll.set_scale(1.0);
        }
        if !active {
            full.frame.configure(false, 4, 1, 1);
        }
        if !active {
            full.picture.set_visible(false);
            for (picture, _) in full.extra_images.borrow().iter() {
                picture.set_visible(false);
            }
            full.rendered.borrow_mut().clear();
        }
    }

    pub(super) fn full_greeting_parts(&self, columns: usize) -> GreetingProjection {
        let settings = self.greeting.settings();
        let character_projection = || {
            let parts = self.greeting_parts_for_width(columns);
            let character_fallback = settings.editable_artwork.is_some()
                && settings.presentation.visual != Visual::Character
                && parts
                    .iter()
                    .any(|(text, part)| *part == GreetingPart::Artwork && !text.trim().is_empty());
            GreetingProjection {
                parts,
                image: None,
                character_fallback,
            }
        };
        if !self.full_pixel_design() {
            return character_projection();
        }
        let fallback = |message: &str| {
            let mut projection = character_projection();
            let detail = if projection.character_fallback {
                "showing character fallback"
            } else {
                "no artwork rendered"
            };
            projection.parts.push((
                format!("{message} · {detail}\r\n\r\n"),
                GreetingPart::Artwork,
            ));
            projection
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
        GreetingProjection {
            parts,
            image,
            character_fallback: false,
        }
    }

    pub(super) fn redraw_full_session(&self) {
        self.redraw_interactive_samples();
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
        for (picture, image) in full.extra_images.borrow().iter() {
            picture.set_margin_start(padding + image.column as i32 * cw);
            picture.set_margin_top(padding + image.row as i32 * ch);
            picture.set_size_request(image.columns as i32 * cw, image.rows as i32 * ch);
            picture.set_visible(true);
        }
        let layout = self.layout_settings();
        // GTK 4.14's public API exposes computed CSS padding through this
        // accessor; read it instead of duplicating stylesheet pixel constants.
        #[allow(deprecated)]
        let chrome = self.preview_terminal_shell.style_context().padding();
        let rail = self
            .preview_terminal_scrollbar_revealer
            .measure(gtk::Orientation::Horizontal, -1)
            .1;
        let target_width = layout.columns as i32 * cw
            + 2 * padding
            + i32::from(chrome.left() + chrome.right())
            + rail;
        // Target height is finite window occupancy, never transcript/history.
        let title_height = self
            .top_preview
            .title
            .parent()
            .map_or(0, |p| p.measure(gtk::Orientation::Vertical, target_width).1);
        let tabs_height = if self.top_preview.tabs.is_visible() {
            self.top_preview
                .tabs
                .measure(gtk::Orientation::Vertical, target_width)
                .1
        } else {
            0
        };
        let observation_rows = if full.zoom.selected() == 4 {
            layout.rows.clamp(24, 40)
        } else {
            layout.rows
        };
        let target_height = observation_rows as i32 * ch
            + 2 * padding
            + title_height
            + tabs_height
            + i32::from(chrome.top() + chrome.bottom())
            + self.preview_terminal_shell.spacing() * 2
            + 4;
        full.frame
            .configure(true, full.zoom.selected(), target_width, target_height);
        let scale = full.frame.scale();
        full.scale.set(scale);
        // The scroll adjustment stays in untransformed terminal coordinates;
        // GTK transforms the entire window, including this scroller and IME.
        self.preview_scroll.set_scale(1.0);
        self.position_sample_input();
        full.play.set_visible(
            self.full_pixel_design()
                && full.image.get().is_some()
                && self.greeting.settings().presentation.visual == Visual::Animation,
        );
        let scope = if full.image.get().is_some_and(|image| {
            image.column + image.columns as usize > self.preview_terminal.column_count() as usize
        }) {
            "Interactive samples · artwork exceeds grid"
        } else if full.character_fallback.get() {
            "Interactive samples · character reference; design unchanged"
        } else {
            "Interactive samples · no local commands"
        };
        full.notice
            .set_text(&format!("{scope} · {:.0}%", scale * 100.0));
        full.notice.set_tooltip_text(Some(if full.character_fallback.get() {
            "The visible artwork uses a character fallback, not native image/GIF output. The saved design is unchanged. Artwork view remains available."
        } else { "Interactive samples, not native verification. Actual size follows the viewport at the theme font size. Fixed grid uses theme initial columns; Fit scales the complete finite window. Try Greeting remains a real temporary trial." }));
    }

    pub(super) fn schedule_sample_input(this: &Rc<Self>) {
        if this.full_session.input_pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(this);
        glib::idle_add_local_once(move || {
            if let Some(this) = weak.upgrade() {
                this.full_session.input_pending.set(false);
                this.position_sample_input();
            }
        });
    }

    pub(super) fn position_sample_input(&self) {
        if !self.full_session.active.get() || self.full_session.collecting.get() {
            return;
        }
        let terminal = &self.preview_terminal;
        let (column, row) = terminal.cursor_position();
        let origin = preview_visible_origin(terminal).map_or(0, |(origin, _)| origin);
        let cw = terminal.char_width().max(1) as i32;
        let ch = terminal.char_height().max(1) as i32;
        let input = &self.full_session.samples.input_row;
        input.set_margin_start(terminal.margin_start() + column as i32 * cw);
        input.set_margin_top(terminal.margin_top() + (row - origin).max(0) as i32 * ch);
        input.set_size_request(
            ((terminal.column_count() - column).max(1) as i32 * cw).max(cw),
            ch,
        );
        self.full_session.samples.entry.set_height_request(ch);
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
        // Let the existing 220 ms coalescer start before asserting no worker
        // is active; otherwise a cached old prompt can masquerade as settled.
        settle();
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
        this.preview_scroll.start();
        settle();
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
        // Pixel composition is a Kitty design approximation. Ptyxis now has
        // an explicit character fallback, tested separately.
        this.typed
            .target
            .set(Some(crate::design_document::TargetHint::Kitty));
        let (mut settings, _) = crate::pixel_trial::tests::fixture(true);
        settings.presentation.visual = Visual::Animation;
        settings.presentation.columns = 32;
        settings.message = "Full Session · 你好 🌸".into();
        settings.preview_columns = 100;
        this.greeting.replace(settings.clone(), true);
        this.prompt_source_selector.set_selected(1);
        this.preview_scene_selector.set_selected(0);
        // Fixed artwork occupancy; the geometry test covers adaptive default.
        this.column_count_input.set_value(100.0);
        this.full_session.zoom.set_selected(1);
        ready(&this);
        let full = &this.full_session;
        this.sample_action(crate::interactive_samples::Action::Help);
        settle();
        let generation = this.greeting.presentation.pixel_generation();
        assert!(full.active.get());
        assert!(full.picture.is_mapped());
        assert!(feed(&this).contains("Full Session"));
        assert!(feed(&this).contains("OS:"));
        assert!(feed(&this).contains("Interactive samples"));
        assert!(feed(&this).contains("no local commands are executed"));
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
        super::preview_geometry_tests::aligned(&this);
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
            // Interactive Fit preserves width, not poster height. The bounded
            // transcript and every image share this single vertical viewport.
            assert!(
                this.preview_terminal_viewport.vadjustment().upper()
                    >= bounds.height() as f64 - 2.0,
                "bounds={bounds:?} upper={} scale={} canvas={} overlay={} scaled={} requested={}",
                this.preview_terminal_viewport.vadjustment().upper(),
                full.scale.get(),
                this.preview_terminal_canvas.height_request(),
                full.overlay.height(),
                full.frame.height(),
                full.frame.height_request()
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
        assert!(feed(&this).contains("Interactive samples"));
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
        this.preview_scroll.start();
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
        assert!(feed(&this).contains("OS:") && feed(&this).contains("Interactive samples"));
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
        assert!(feed(&this).contains("OS:") && feed(&this).contains("Interactive samples"));
        let mut disabled = this.greeting.settings();
        disabled.enabled = false;
        this.greeting.replace(disabled, true);
        settle();
        assert!(full.active.get() && full.image.get().is_none());
        assert!(feed(&this).contains("Interactive samples"));
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
