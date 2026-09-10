//! Static pixel export is explicit and separate from ANSI preview/application.
use super::*;
use crate::greeting_image::pixel_export::{self, PixelImage, Protocol};
use std::{sync::Arc, thread};

struct PixelExport {
    window: glib::WeakRef<gtk::Window>,
    workbench: std::rc::Weak<Workbench>,
    before: GreetingSettings,
    picture: gtk::Picture,
    protocol: gtk::DropDown,
    columns: gtk::SpinButton,
    playback: gtk::Box,
    play: gtk::ToggleButton,
    frame: gtk::Scale,
    frame_label: gtk::Label,
    frame_index: Cell<usize>,
    frame_started: Cell<std::time::Instant>,
    seeking: Cell<bool>,
    requirements: gtk::Label,
    status: gtk::Label,
    export: gtk::Button,
    image: RefCell<Option<Arc<PixelImage>>>,
    writing: Cell<bool>,
    closed: Cell<bool>,
    ratio: f64,
    cell_size: [u32; 2],
}

impl Workbench {
    pub(in crate::window) fn show_pixel_export(self: &Rc<Self>) {
        if let Some(window) = self.greeting.pixel_export_window.upgrade() {
            window.present();
            return;
        }
        self.greeting.finish();
        let before = self.greeting.settings();
        if !self.art_draft_matches(&before) {
            return;
        }
        if !before.enabled {
            self.toast("Enable Greeting before exporting an image greeting.");
            return;
        }
        let Some(source) = before.editable_artwork.clone() else {
            self.toast("Import a PNG, JPG, WebP, SVG or GIF first. Text and ANSI logos have no original pixels to export.");
            return;
        };
        let is_gif = source.image.is_gif();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        for set in [
            gtk::Widget::set_margin_top,
            gtk::Widget::set_margin_bottom,
            gtk::Widget::set_margin_start,
            gtk::Widget::set_margin_end,
        ] {
            set(content.upcast_ref(), 20);
        }
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.append(
            &gtk::Label::builder()
                .label("Image Greeting")
                .xalign(0.0)
                .hexpand(true)
                .css_classes(["heading"])
                .build(),
        );
        let cancel = gtk::Button::with_label("Cancel");
        let export = gtk::Button::with_label("Export Folder…");
        export.add_css_class("suggested-action");
        export.set_sensitive(false);
        header.append(&cancel);
        header.append(&export);
        content.append(&header);
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Contain)
            .can_shrink(true)
            .height_request(240)
            .vexpand(true)
            .alternative_text("Processed source image; this is not a terminal protocol preview")
            .build();
        let backdrop = gtk::DrawingArea::builder().vexpand(true).build();
        backdrop.set_draw_func(|_, cr, width, height| {
            cr.set_source_rgb(0.97, 0.97, 0.97);
            let _ = cr.paint();
            cr.set_source_rgb(0.90, 0.90, 0.90);
            for y in (0..height).step_by(16) {
                for x in (0..width).step_by(16) {
                    if (x / 16 + y / 16) % 2 == 0 {
                        cr.rectangle(f64::from(x), f64::from(y), 16.0, 16.0);
                    }
                }
            }
            let _ = cr.fill();
        });
        let canvas = gtk::Overlay::new();
        canvas.set_child(Some(&backdrop));
        canvas.add_overlay(&picture);
        canvas.set_measure_overlay(&picture, true);
        canvas.set_vexpand(true);
        content.append(&canvas);
        let options = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let protocol = gtk::DropDown::from_strings(if is_gif {
            &[
                "Kitty · Still PNG",
                "Sixel · Transparent still",
                "Kitty · Animated GIF",
            ]
        } else {
            &["Kitty · Direct PNG", "Sixel"]
        });
        if is_gif {
            protocol.set_selected(2);
        }
        protocol.set_hexpand(true);
        protocol.update_property(&[gtk::accessible::Property::Label("Image protocol")]);
        let columns = gtk::SpinButton::with_range(8.0, 120.0, 1.0);
        columns.set_value(32.0);
        columns.update_property(&[gtk::accessible::Property::Label("Maximum logo columns")]);
        options.append(&protocol);
        options.append(&gtk::Label::new(Some("Max columns")));
        options.append(&columns);
        // Wheel motion must not silently change export parameters.
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
        wheel.connect_scroll(|_, _, _| glib::Propagation::Stop);
        options.add_controller(wheel);
        content.append(&options);
        let playback = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let play = gtk::ToggleButton::with_label("Play");
        play.set_tooltip_text(Some("Play or pause a looping preview of the exported GIF. Playback starts only when requested."));
        let frame = gtk::Scale::with_range(gtk::Orientation::Horizontal, 1.0, 2.0, 1.0);
        frame.set_draw_value(false);
        frame.set_hexpand(true);
        frame.update_property(&[gtk::accessible::Property::Label("Animation frame")]);
        let frame_label = gtk::Label::new(None);
        playback.append(&play);
        playback.append(&frame);
        playback.append(&frame_label);
        playback.set_visible(is_gif);
        playback.set_sensitive(false);
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
        wheel.connect_scroll(|_, _, _| glib::Propagation::Stop);
        playback.add_controller(wheel);
        content.append(&playback);
        let requirements = gtk::Label::builder().wrap(true).xalign(0.0).build();
        content.append(&requirements);
        let note = gtk::Label::builder().wrap(true).xalign(0.0)
            .label("Pixel preview only. Crop, background removal and color edits are retained; character-art effects are not. Live Preview and Apply continue to use ANSI.")
            .css_classes(["dim-label"]).build();
        content.append(&note);
        let status = gtk::Label::builder()
            .wrap(true)
            .selectable(true)
            .xalign(0.0)
            .label("Preparing image…")
            .build();
        content.append(&status);
        let window = gtk::Window::builder()
            .title("Export Image Greeting")
            .transient_for(&self.window())
            .modal(true)
            .default_width(620)
            .default_height(600)
            .child(&content)
            .build();
        let this = Rc::new(PixelExport {
            window: window.downgrade(),
            workbench: Rc::downgrade(self),
            before,
            picture,
            protocol,
            columns,
            playback,
            play,
            frame,
            frame_label,
            frame_index: Cell::new(0),
            frame_started: Cell::new(std::time::Instant::now()),
            seeking: Cell::new(false),
            requirements,
            status,
            export,
            image: RefCell::new(None),
            writing: Cell::new(false),
            closed: Cell::new(false),
            ratio: (self.preview_terminal.char_width().max(1) as f64
                / self.preview_terminal.char_height().max(1) as f64)
                .clamp(0.1, 2.0),
            cell_size: [
                (self.preview_terminal.char_width().max(1) as u32)
                    .saturating_mul(self.preview_terminal.scale_factor().max(1) as u32),
                (self.preview_terminal.char_height().max(1) as u32)
                    .saturating_mul(self.preview_terminal.scale_factor().max(1) as u32),
            ],
        });
        unsafe {
            window.set_data("termimochi-pixel-export", this.clone());
        }
        self.greeting.pixel_export_window.set(Some(&window));
        let weak = Rc::downgrade(&this);
        window.connect_close_request(move |_| {
            if let Some(this) = weak.upgrade() {
                if this.writing.get() {
                    return glib::Propagation::Stop;
                }
                this.closed.set(true);
                if let Some(workbench) = this.workbench.upgrade() {
                    workbench
                        .greeting
                        .pixel_export_window
                        .set(None::<&gtk::Window>);
                }
            }
            glib::Propagation::Proceed
        });
        let weak = window.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.close();
            }
        });
        let weak = Rc::downgrade(&this);
        this.protocol.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                this.play.set_active(false);
                this.show_frame(this.frame_index.get());
                this.refresh();
            }
        });
        let weak = Rc::downgrade(&this);
        this.columns.connect_value_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.refresh();
            }
        });
        let weak = Rc::downgrade(&this);
        this.export.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.choose_folder();
            }
        });
        let weak = Rc::downgrade(&this);
        this.play.connect_toggled(move |button| {
            button.set_label(if button.is_active() { "Pause" } else { "Play" });
            if let Some(this) = weak.upgrade() {
                this.frame_started.set(std::time::Instant::now());
            }
        });
        let weak = Rc::downgrade(&this);
        this.frame.connect_value_changed(move |frame| {
            if let Some(this) = weak.upgrade().filter(|t| !t.seeking.get()) {
                this.play.set_active(false);
                this.show_frame(frame.value().round().max(1.0) as usize - 1);
            }
        });
        this.refresh();
        let (send, receive) = mpsc::channel();
        thread::spawn(move || {
            let _ = send.send(pixel_export::prepare(&source));
        });
        let weak = Rc::downgrade(&this);
        glib::timeout_add_local(Duration::from_millis(40), move || {
            let Some(this) = weak.upgrade().filter(|t| !t.closed.get()) else {
                return glib::ControlFlow::Break;
            };
            match receive.try_recv() {
                Ok(Ok(image)) => {
                    let count = image.animation.as_ref().map_or(0, |a| a.frames.len());
                    this.seeking.set(true);
                    this.frame.set_range(1.0, count.max(2) as f64);
                    this.frame.set_sensitive(count > 1);
                    this.play.set_sensitive(count > 1);
                    this.playback.set_sensitive(count > 0);
                    this.seeking.set(false);
                    *this.image.borrow_mut() = Some(Arc::new(image));
                    this.show_frame(0);
                    this.export.set_sensitive(true);
                    this.refresh();
                }
                Ok(Err(error)) => this.status.set_label(&error),
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => this
                    .status
                    .set_label("Image preparation stopped. Close and try again."),
            }
            glib::ControlFlow::Break
        });
        let weak = Rc::downgrade(&this);
        glib::timeout_add_local(Duration::from_millis(20), move || {
            if !is_gif {
                return glib::ControlFlow::Break;
            }
            let Some(this) = weak.upgrade().filter(|t| !t.closed.get()) else {
                return glib::ControlFlow::Break;
            };
            if this.play.is_active() && !this.writing.get() {
                let image = this.image.borrow();
                if let Some(animation) = image.as_ref().and_then(|i| i.animation.as_ref()) {
                    let index = this.frame_index.get();
                    let elapsed = this.frame_started.get().elapsed();
                    let due = elapsed.as_millis() >= u128::from(animation.frames[index].delay_ms);
                    let (next, remainder) = animation.frame_after(index, elapsed);
                    drop(image);
                    if due {
                        this.show_frame(next);
                        let now = std::time::Instant::now();
                        this.frame_started
                            .set(now.checked_sub(remainder).unwrap_or(now));
                    }
                }
            }
            glib::ControlFlow::Continue
        });
        window.present();
    }
}

impl PixelExport {
    fn protocol(&self) -> Protocol {
        match self.protocol.selected() {
            1 => Protocol::Sixel,
            2 => Protocol::KittyAnimation,
            _ => Protocol::Kitty,
        }
    }
    fn show_frame(&self, index: usize) {
        let image = self.image.borrow();
        let Some(image) = image.as_ref() else {
            return;
        };
        let animated = self.protocol() == Protocol::KittyAnimation;
        self.playback
            .set_visible(animated && image.animation.is_some());
        let pixels = if animated && let Some(animation) = image.animation.as_ref() {
            let index = index.min(animation.frames.len() - 1);
            self.frame_index.set(index);
            self.frame_started.set(std::time::Instant::now());
            self.seeking.set(true);
            self.frame.set_value((index + 1) as f64);
            self.seeking.set(false);
            let frame = &animation.frames[index];
            self.frame_label.set_label(&format!(
                "{} / {} · {} ms",
                index + 1,
                animation.frames.len(),
                frame.delay_ms
            ));
            &frame.pixels
        } else {
            &image.pixels
        };
        let bytes = glib::Bytes::from_owned(pixels.as_raw().clone());
        let texture = gdk::MemoryTexture::new(
            pixels.width() as i32,
            pixels.height() as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            pixels.width() as usize * 4,
        );
        self.picture.set_paintable(Some(&texture));
    }
    fn refresh(&self) {
        self.requirements.set_label(self.protocol().requirements());
        if !self.writing.get()
            && let Some(image) = self.image.borrow().as_ref()
        {
            match pixel_export::area(image.pixels.dimensions(), self.columns.value_as_int() as u32, self.ratio) {
                Ok((w, h)) => self.status.set_label(&format!("{} × {} px · {w} columns × {h} rows\nExports a new folder; your active Fastfetch configuration is unchanged.", image.pixels.width(), image.pixels.height())),
                Err(error) => self.status.set_label(&error),
            }
        }
    }
    fn choose_folder(self: &Rc<Self>) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        self.export.set_sensitive(false);
        let dialog = gtk::FileDialog::builder()
            .title("Choose Parent Folder")
            .accept_label("Export Here")
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.select_folder(Some(&window), gio::Cancellable::NONE, move |result| {
            let Some(this) = weak.upgrade().filter(|t| !t.closed.get()) else {
                return;
            };
            this.export.set_sensitive(true);
            match result {
                Ok(file) => match file.path() {
                    Some(path) => this.export_to(path),
                    None => this.status.set_label("Choose a local folder."),
                },
                Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                Err(error) => this
                    .status
                    .set_label(&format!("Could not choose folder: {error}")),
            }
        });
    }
    fn export_to(self: &Rc<Self>, parent: PathBuf) {
        if self.closed.get() || self.writing.get() {
            return;
        }
        let Some(workbench) = self.workbench.upgrade() else {
            return;
        };
        if !workbench.art_draft_matches(&self.before) {
            return;
        }
        let Some(image) = self.image.borrow().clone() else {
            return;
        };
        self.writing.set(true);
        self.export.set_sensitive(false);
        self.protocol.set_sensitive(false);
        self.columns.set_sensitive(false);
        self.status.set_label("Exporting image greeting…");
        let settings = self.before.clone();
        let protocol = self.protocol();
        let columns = self.columns.value_as_int() as u32;
        let cell_size = self.cell_size;
        let (send, receive) = mpsc::channel();
        thread::spawn(move || {
            let _ = send.send(pixel_export::export_bundle(
                &parent, &settings, &image, protocol, columns, cell_size,
            ));
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(40), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let result = match receive.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => {
                    Err("Export worker stopped. Inspect the chosen folder before retrying.".into())
                }
            };
            this.writing.set(false);
            this.export.set_sensitive(true);
            this.protocol.set_sensitive(true);
            this.columns.set_sensitive(true);
            match result {
                Ok(path) => this.status.set_label(&format!("Exported to {}\nOpen a terminal in that folder and run: fastfetch --config config.jsonc", path.display())),
                Err(error) => this.status.set_label(&format!("Export failed: {error}")),
            }
            glib::ControlFlow::Break
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{controller, settle};
    use super::*;
    use crate::greeting_image::{
        Adjustments, Options, Style,
        source::{EditableArtwork, SourceImage},
    };

    #[test]
    #[ignore = "requires isolated GTK/VTE display; animation playback, seeking and static fallback"]
    fn gif_pixel_dialog_play_pause_seek_and_static_fallback() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.GifPixelTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let workbench = controller(&window);
        let source = crate::greeting_image::animation::tests::fixture();
        let mut settings = GreetingSettings::starter();
        settings
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
        settings.enabled = true;
        settings.editable_artwork = Some(source);
        workbench.greeting.replace(settings.clone(), false);
        let (dialog, editor) = draft(&workbench);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !editor.export.is_sensitive() {
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                editor.status.text()
            );
            settle();
        }
        assert_eq!(editor.protocol(), Protocol::KittyAnimation);
        assert!(editor.playback.is_visible());
        assert!(!editor.play.is_active(), "Do not auto-play motion");
        assert!(editor.picture.height() >= 240);
        editor.frame.set_value(3.0);
        assert_eq!(editor.frame_index.get(), 2);
        assert!(editor.frame_label.text().starts_with("3 / 3"));
        if std::env::var_os("TERMIMOCHI_POINTER_TEST").is_some() {
            dialog.set_title(Some("TermiMochi point-to-edit test"));
            super::super::tests::pointer(&dialog, editor.frame.upcast_ref(), "scroll_up");
            assert_eq!(editor.frame_index.get(), 2);
            if std::env::var_os("TERMIMOCHI_INSPECT_SCREENSHOT").is_some() {
                super::super::tests::pointer(&dialog, editor.picture.upcast_ref(), "capture");
            }
        }
        editor.play.set_active(true);
        let mut seen = std::collections::HashSet::new();
        let deadline = std::time::Instant::now() + Duration::from_millis(600);
        while std::time::Instant::now() < deadline {
            while glib::MainContext::default().pending() {
                glib::MainContext::default().iteration(false);
            }
            seen.insert(editor.frame_index.get());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(seen.len(), 3);
        editor.play.set_active(false);
        let paused = editor.frame_index.get();
        settle();
        assert_eq!(editor.frame_index.get(), paused);
        editor.protocol.set_selected(0);
        assert!(!editor.playback.is_visible());
        assert!(!editor.play.is_active());
        editor.protocol.set_selected(2);
        assert!(editor.playback.is_visible());
        editor.export_to(root.path().into());
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while editor.writing.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        assert!(
            editor.status.text().starts_with("Exported to"),
            "{}",
            editor.status.text()
        );
        assert_eq!(workbench.greeting.settings(), settings);
        dialog.close();
        settle();
        assert!(editor.closed.get());
        window.destroy();
    }

    fn draft(workbench: &Rc<Workbench>) -> (gtk::Window, Rc<PixelExport>) {
        workbench
            .window()
            .lookup_action("export-pixel-greeting")
            .unwrap()
            .activate(None);
        let window = workbench.greeting.pixel_export_window.upgrade().unwrap();
        let draft = unsafe {
            window
                .data::<Rc<PixelExport>>("termimochi-pixel-export")
                .unwrap()
                .as_ref()
                .clone()
        };
        (window, draft)
    }

    #[test]
    #[ignore = "requires isolated GTK/VTE display; tests pixel dialog lifecycle and explicit bundle export"]
    fn pixel_export_dialog_cancel_guard_geometry_and_both_protocols() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.PixelExportTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let workbench = controller(&window);
        workbench.show_pixel_export();
        assert!(workbench.greeting.pixel_export_window.upgrade().is_none());
        let source = SourceImage::new(
            include_bytes!(
                "../../../resources/icons/256x256/apps/io.github.miiikuuu.termimochi.png"
            )
            .to_vec(),
        )
        .unwrap();
        let options = Options {
            columns: 32,
            style: Style::Detail,
            cell_ratio: 0.5,
            foreground: [0; 3],
            background: [255; 3],
            invert: false,
            edits: Adjustments::default(),
        };
        let art = source.decode().unwrap().convert(options).unwrap().artwork;
        let settings = GreetingSettings {
            enabled: true,
            logo: Logo::Custom,
            custom_logo: art.plain.clone(),
            custom_art: Some(art),
            editable_artwork: Some(EditableArtwork::new(source, options).unwrap()),
            ..Default::default()
        };
        workbench.greeting.replace(settings.clone(), false);
        let (dialog, canceled) = draft(&workbench);
        dialog.close();
        settle();
        canceled.export_to(root.path().into());
        assert!(canceled.closed.get());
        assert_eq!(workbench.greeting.settings(), settings);
        let (dialog, editor) = draft(&workbench);
        workbench.show_pixel_export();
        assert_eq!(
            workbench.greeting.pixel_export_window.upgrade().unwrap(),
            dialog
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !editor.export.is_sensitive() {
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                editor.status.text()
            );
            settle();
        }
        settle();
        assert!(editor.picture.paintable().is_some());
        assert!(editor.picture.width() > 200 && editor.picture.height() >= 240);
        if std::env::var_os("TERMIMOCHI_POINTER_TEST").is_some() {
            dialog.set_title(Some("TermiMochi point-to-edit test"));
            settle();
            editor.columns.grab_focus();
            super::super::tests::pointer(&dialog, editor.columns.upcast_ref(), "scroll_up");
            assert_eq!(editor.columns.value_as_int(), 32);
            super::super::tests::pointer(&dialog, editor.protocol.upcast_ref(), "scroll_down");
            assert_eq!(editor.protocol.selected(), 0);
            if std::env::var_os("TERMIMOCHI_INSPECT_SCREENSHOT").is_some() {
                super::super::tests::pointer(&dialog, editor.picture.upcast_ref(), "capture");
            }
        }
        for (index, protocol) in [Protocol::Kitty, Protocol::Sixel].into_iter().enumerate() {
            editor.protocol.set_selected(index as u32);
            editor.columns.set_value(40.0);
            assert_eq!(editor.requirements.text(), protocol.requirements());
            let parent = root.path().join(format!("export-{index}"));
            std::fs::create_dir(&parent).unwrap();
            editor.export_to(parent.clone());
            assert!(editor.writing.get() && !editor.export.is_sensitive());
            dialog.close(); // An in-flight write finishes before closing is allowed.
            assert!(!editor.closed.get());
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while editor.writing.get() {
                assert!(std::time::Instant::now() < deadline);
                settle();
            }
            assert!(
                editor.status.text().starts_with("Exported to"),
                "{}",
                editor.status.text()
            );
            let folders: Vec<_> = std::fs::read_dir(&parent)
                .unwrap()
                .map(|p| p.unwrap().path())
                .collect();
            assert_eq!(folders.len(), 1);
            let config = std::fs::read_to_string(folders[0].join("config.jsonc")).unwrap();
            let config = crate::fastfetch_document::value(&config).unwrap();
            assert_eq!(config["logo"]["type"], protocol.logo_type());
            assert_eq!(config["logo"]["width"], 40);
            assert!(folders[0].join("logo.png").is_file());
            assert_eq!(workbench.greeting.settings(), settings);
        }
        let mut changed = settings.clone();
        changed.gap += 1;
        workbench.greeting.replace(changed, false);
        let refused = root.path().join("stale-export");
        std::fs::create_dir(&refused).unwrap();
        editor.export_to(refused.clone());
        assert!(!editor.writing.get());
        assert_eq!(std::fs::read_dir(refused).unwrap().count(), 0);
        dialog.close();
        settle();
        assert!(workbench.greeting.pixel_export_window.upgrade().is_none());
        window.destroy();
    }
}
