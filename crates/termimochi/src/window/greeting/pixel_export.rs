//! Static pixel export is explicit and separate from ANSI preview/application.
use super::*;
use crate::greeting_image::pixel_export::{self, PixelImage, Protocol};
use std::{sync::Arc, thread};
mod trial;

struct PixelExport {
    trial_mode: bool,
    key: Option<crate::greeting_output::VerificationKey>,
    feedback: Vec<gtk::Button>,
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
    target: gtk::DropDown,
    test: gtk::Button,
    ansi_test: gtk::Button,
    install: gtk::Button,
    capabilities: gtk::Label,
    trial: RefCell<Option<Rc<crate::pixel_trial::Trial>>>,
    trial_generation: Cell<u64>,
    trial_running: Cell<bool>,
    image: RefCell<Option<Arc<PixelImage>>>,
    writing: Cell<bool>,
    closed: Cell<bool>,
    ratio: f64,
    cell_size: [u32; 2],
}

impl Workbench {
    pub(in crate::window) fn show_pixel_export(self: &Rc<Self>) {
        self.show_pixel_output(false);
    }
    pub(in crate::window) fn show_greeting_trial(self: &Rc<Self>) {
        self.show_pixel_output(true);
    }
    fn show_pixel_output(self: &Rc<Self>, trial_mode: bool) {
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
        let source = before.editable_artwork.clone();
        if source.is_none() && !trial_mode {
            self.toast("Import a PNG, JPG, WebP, SVG or GIF first. Text and ANSI logos have no original pixels to export.");
            return;
        };
        let is_gif = source.as_ref().is_some_and(|s| s.image.is_gif());
        let spec = match before.presentation.resolve(
            &before,
            self.greeting.presentation.binding.borrow().terminal,
        ) {
            Ok(spec) => spec,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
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
                .label(if trial_mode {
                    "Try in Terminal"
                } else {
                    "Export a Copy"
                })
                .xalign(0.0)
                .hexpand(true)
                .css_classes(["heading"])
                .build(),
        );
        let cancel = gtk::Button::with_label("Cancel");
        let export = gtk::Button::with_label("Export Folder…");
        export.set_visible(!trial_mode);
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
        protocol.set_selected(match spec.protocol {
            Some(Protocol::KittyAnimation) => 2,
            Some(Protocol::Sixel) => 1,
            _ => 0,
        });
        protocol.set_hexpand(true);
        protocol.update_property(&[gtk::accessible::Property::Label("Image protocol")]);
        let columns = gtk::SpinButton::with_range(8.0, 120.0, 1.0);
        columns.set_value(before.presentation.columns.into());
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
        let target_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        target_row.append(&gtk::Label::new(Some("Test in")));
        let target = gtk::DropDown::from_strings(&["Kitty", "Ptyxis", "Xterm"]);
        target.set_hexpand(true);
        target.update_property(&[gtk::accessible::Property::Label(
            "Target terminal for image trial",
        )]);
        target_row.append(&target);
        let test = gtk::Button::with_label("Test in Terminal");
        let ansi_test = gtk::Button::with_label("Use Character & Try");
        ansi_test.set_tooltip_text(Some(
            "Explicitly changes the scheme to Character output; Undo restores the image intent.",
        ));
        target_row.append(&test);
        target_row.append(&ansi_test);
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
        wheel.connect_scroll(|_, _, _| glib::Propagation::Stop);
        target_row.add_controller(wheel);
        content.append(&target_row);
        target_row.set_visible(trial_mode);
        target.set_selected(
            crate::pixel_trial::Terminal::ALL
                .iter()
                .position(|t| *t == self.greeting.presentation.binding.borrow().terminal)
                .unwrap() as u32,
        );
        options.set_visible(!trial_mode);
        let feedback_row = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .max_children_per_line(2)
            .build();
        let feedback: Vec<_> = [
            "Looks correct",
            "Nothing displayed",
            "Animation broken",
            "Try unverified output",
        ]
        .into_iter()
        .map(|label| {
            let button = gtk::Button::with_label(label);
            button.set_sensitive(false);
            feedback_row.insert(&button, -1);
            button
        })
        .collect();
        feedback_row.set_visible(trial_mode);
        content.append(&feedback_row);
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
            .label(if trial_mode {"The terminal uses safe sample information, NOT your imported commands. Look at that terminal, then give feedback here. This GTK image is only a design reference. Shared settings and shell startup are untouched."} else {"Creates a portable copy only. Use the main workbench's Try and Apply actions for your terminal; this GTK canvas is not protocol verification."})
            .css_classes(["dim-label"]).build();
        content.append(&note);
        let capabilities = gtk::Label::builder().wrap(true).xalign(0.0).build();
        content.append(&capabilities);
        capabilities.set_visible(trial_mode);
        let status = gtk::Label::builder()
            .wrap(true)
            .selectable(true)
            .xalign(0.0)
            .label("Preparing image…")
            .build();
        content.append(&status);
        let install = gtk::Button::with_label("Review Scheme & Apply…");
        install.set_visible(trial_mode);
        install.set_halign(gtk::Align::End);
        install.set_sensitive(false);
        install.set_tooltip_text(Some("Available after visual confirmation in the target terminal. Review managed asset paths and Fastfetch changes before applying."));
        content.append(&install);
        let window = gtk::Window::builder()
            .title(if trial_mode {
                "Try in Terminal"
            } else {
                "Export Image Greeting"
            })
            .transient_for(&self.window())
            .modal(true)
            .default_width(740)
            .default_height(800)
            .child(
                &gtk::ScrolledWindow::builder()
                    .child(&content)
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .build(),
            )
            .build();
        let this = Rc::new(PixelExport {
            trial_mode,
            key: self.greeting_verification_key().ok(),
            feedback,
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
            target,
            test,
            ansi_test,
            install,
            capabilities,
            trial: RefCell::new(None),
            trial_generation: Cell::new(0),
            trial_running: Cell::new(false),
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
                this.trial.borrow_mut().take();
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
                this.invalidate_trial();
                this.show_frame(this.frame_index.get());
                this.refresh();
            }
        });
        let weak = Rc::downgrade(&this);
        this.columns.connect_value_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.invalidate_trial();
                this.refresh();
            }
        });
        this.connect_trials();
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
            let _ = send.send(if let Some(source) = source {
                pixel_export::prepare(&source)
            } else {
                pixel_export::placeholder()
            });
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
                    if this.trial_mode {
                        let ansi = this
                            .before
                            .presentation
                            .resolve(&this.before, this.terminal())
                            .is_ok_and(|s| s.protocol.is_none());
                        this.start_trial(ansi);
                    }
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
        self.refresh_trial_controls();
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
        self.refresh_trial_controls();
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
            this.refresh_trial_controls();
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
    #[ignore = "requires isolated GTK/VTE; trial install gate, invalidation, review cancellation and restore"]
    fn pixel_trial_review_gate_cancel_apply_and_restore() {
        use crate::{
            fastfetch_apply, pixel_trial,
            scheme_apply::{Action, Item, Plan, Status},
        };
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.PixelTrialTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let workbench = controller(&window);
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while workbench.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        let (mut settings, image) = pixel_trial::tests::fixture(false);
        settings.presentation.visual = crate::greeting_output::Visual::Image;
        workbench.greeting.replace(settings.clone(), false);
        workbench.greeting.presentation.target.set_selected(1);
        let daily = root.path().join("config.jsonc");
        let original = "// user's exact content\n{\"modules\":[\"os\"]}";
        std::fs::write(&daily, original).unwrap();
        workbench.accept_scheme_fastfetch(fastfetch_apply::Target::open(daily.clone()).unwrap());
        assert!(
            workbench.prepare_greeting_action().is_err(),
            "unverified image must not become ANSI"
        );
        let trial = Rc::new(
            pixel_trial::Trial::prepare(
                root.path(),
                &settings,
                &image,
                pixel_trial::tests::options(Protocol::Kitty, false),
            )
            .unwrap(),
        );
        pixel_trial::tests::confirm(&trial); // UI/transaction fixture only; native motion is tested separately.
        let key = workbench.greeting_verification_key().unwrap();
        *workbench.greeting.presentation.verified.borrow_mut() = Some((key.clone(), trial.clone()));
        let (detail, _, action) = workbench.prepare_greeting_action().unwrap();
        workbench.refresh_output_bar();
        assert!(
            workbench
                .greeting
                .presentation
                .state
                .text()
                .contains("Independent image · shared greeting unchanged")
        );
        assert!(detail.contains("shared Fastfetch is untouched"));
        assert!(matches!(
            action,
            Action::ImageGreeting {
                independent: true,
                ..
            }
        ));
        drop(action);
        workbench.request_fastfetch_apply();
        settle();
        super::super::tests::respond("Cancel");
        assert_eq!(std::fs::read_to_string(&daily).unwrap(), original);
        // Size edits expire this evidence; undo restores the exact design identity.
        let mut edited = settings.clone();
        edited.presentation.columns = 48;
        workbench.greeting.replace(edited, true);
        assert!(workbench.prepare_greeting_action().is_err());
        workbench.greeting.undo();
        assert!(workbench.prepare_greeting_action().is_ok());
        let (detail, versions, action) = workbench.prepare_greeting_action().unwrap();
        let mut plan = Plan::new(root.path(), "isolated Kitty".into(), None);
        plan.items.push(Item {
            id: "fastfetch",
            title: "Greeting".into(),
            detail,
            versions,
            action: Some(action),
        });
        let (directory, mut report) = plan.apply(&[true]).unwrap();
        assert_eq!(report.items[0].status, Status::NotEnabled);
        assert_eq!(std::fs::read_to_string(&daily).unwrap(), original);
        let installed = report.items[0].path.as_ref().unwrap().clone();
        let source = std::fs::read_to_string(&installed).unwrap();
        assert_eq!(
            crate::fastfetch_document::value(&source).unwrap()["logo"]["type"],
            "kitty-direct"
        );
        // Reapplying through any entry still produces an image plan, including field-only edits.
        let mut fields = settings.clone();
        fields.items[0].enabled = !fields.items[0].enabled;
        workbench.greeting.replace(fields, true);
        let (_, _, action) = workbench.prepare_greeting_action().unwrap();
        assert!(matches!(action, Action::ImageGreeting { .. }));
        drop(action);
        report.restore(&directory).unwrap();
        assert_eq!(report.items[0].status, Status::Restored);
        assert!(!installed.exists());
        // Explicit character mode is undoable and is the ONLY route back to ANSI.
        let mut character = workbench.greeting.settings();
        character.presentation.visual = crate::greeting_output::Visual::Character;
        workbench.greeting.replace(character, true);
        assert!(matches!(
            workbench.prepare_greeting_action().unwrap().2,
            Action::Fastfetch { .. }
        ));
        workbench.greeting.undo();
        assert!(matches!(
            workbench.prepare_greeting_action().unwrap().2,
            Action::ImageGreeting { .. }
        ));
        // Use the actual common review callback: refresh its shared snapshot so
        // repeat image application and an explicit character switch do not
        // mistake this application's own write for an external conflict.
        workbench.greeting.presentation.shared.set_active(true);
        for character in [false, false, true] {
            if character {
                let mut next = workbench.greeting.settings();
                crate::greeting_output::select_character(&mut next);
                workbench.greeting.replace(next, true);
            }
            workbench.refresh_output_bar();
            assert!(
                workbench
                    .greeting
                    .presentation
                    .state
                    .text()
                    .contains("Shared greeting · affects every reader")
            );
            workbench.request_scheme_apply();
            settle();
            let review = gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|w| w.downcast::<gtk::Window>().ok())
                .find(|w| w.title().as_deref() == Some("Apply This Scheme"))
                .unwrap();
            let check = super::super::tests::descendants(review.upcast_ref())
                .into_iter()
                .find(|w| w.widget_name() == "scheme-fastfetch")
                .unwrap()
                .downcast::<gtk::CheckButton>()
                .unwrap();
            assert!(check.is_sensitive());
            check.set_active(true);
            super::super::tests::respond("Back Up & Apply Selected");
            settle();
            let (_, applied) =
                crate::scheme_apply::Report::latest(&typography_preset::state_directory()).unwrap();
            let row = applied.items.iter().find(|r| r.id == "fastfetch").unwrap();
            assert!(
                matches!(row.status, Status::Applied | Status::Unchanged),
                "{}",
                row.detail
            );
            super::super::tests::respond("Close");
            assert!(workbench.prepare_greeting_action().is_ok());
            let value = crate::fastfetch_document::value(&std::fs::read_to_string(&daily).unwrap())
                .unwrap();
            assert_eq!(value["logo"]["type"] == "kitty-direct", !character);
        }
        window.close();
        settle();
    }

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
        settings.presentation.visual = crate::greeting_output::Visual::Animation;
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

    #[test]
    #[ignore = "isolated GTK + real Kitty: main trial, moving pixels, GUI feedback and shared-scope review"]
    fn native_gui_animation_feedback_uses_the_frozen_scheme() {
        use crate::greeting_output::Visual;
        assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.NativeFeedbackTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let workbench = controller(&window);
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while workbench.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        let (mut settings, _) = crate::pixel_trial::tests::fixture(true);
        settings.presentation.visual = Visual::Animation;
        settings.presentation.columns = 24;
        workbench.greeting.replace(settings, false);
        workbench.greeting.presentation.target.set_selected(1);
        workbench.show_greeting_trial();
        settle();
        let dialog = workbench.greeting.pixel_export_window.upgrade().unwrap();
        let editor = unsafe {
            dialog
                .data::<Rc<PixelExport>>("termimochi-pixel-export")
                .unwrap()
                .as_ref()
                .clone()
        };
        assert!(editor.trial_mode && !editor.export.is_visible());
        let deadline = std::time::Instant::now() + Duration::from_secs(35);
        while !editor.feedback[0].is_sensitive() {
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                editor.status.text()
            );
            settle();
        }
        assert!(!editor.install.is_sensitive());
        let result = std::process::Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/test-pixel-trial.py"
            ))
            .arg("--observe-existing")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        println!("{}", String::from_utf8_lossy(&result.stdout));
        editor.feedback[0].emit_clicked();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !editor.install.is_sensitive() {
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                editor.status.text()
            );
            settle();
        }
        assert!(workbench.greeting.presentation.verified.borrow().is_some());
        let kitty_config = glib::user_config_dir().join("kitty");
        std::fs::create_dir_all(&kitty_config).unwrap();
        std::fs::write(kitty_config.join("kitty.conf"), "font_size 17\n").unwrap();
        assert!(
            workbench.prepare_greeting_action().is_err(),
            "target font changes expire approval"
        );
        // Rebind this test's confirmed artifact only for the remaining scope UI
        // assertions; the next real trial below must clear it and report failure.
        *workbench.greeting.presentation.verified.borrow_mut() = Some((
            workbench.greeting_verification_key().unwrap(),
            editor.trial.borrow().as_ref().unwrap().clone(),
        ));
        assert!(matches!(
            workbench.prepare_greeting_action().unwrap().2,
            crate::scheme_apply::Action::ImageGreeting {
                independent: true,
                ..
            }
        ));
        // Shared replacement requires a separate, explicit local choice.
        workbench.greeting.presentation.shared.set_active(true);
        let (detail, _, action) = workbench.prepare_greeting_action().unwrap();
        assert!(detail.contains("ALL terminals") && detail.contains("SHARED"));
        assert!(matches!(
            action,
            crate::scheme_apply::Action::ImageGreeting {
                independent: false,
                ..
            }
        ));
        drop(action);
        // A new failed trial must supersede this successful confirmation.
        editor.start_trial(false);
        assert!(workbench.greeting.presentation.verified.borrow().is_none());
        assert!(workbench.prepare_greeting_action().is_err());
        let deadline = std::time::Instant::now() + Duration::from_secs(35);
        while !editor.feedback[2].is_sensitive() {
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                editor.status.text()
            );
            settle();
        }
        editor.feedback[2].emit_clicked();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while editor.trial_running.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        assert!(!editor.install.is_sensitive());
        assert!(workbench.greeting.presentation.verified.borrow().is_none());
        assert!(workbench.prepare_greeting_action().is_err());
        dialog.close();
        settle();
        assert!(
            !glib::user_config_dir()
                .join("fastfetch/config.jsonc")
                .exists()
        );
        assert!(!glib::home_dir().join(".bashrc").exists());
        window.close();
        settle();
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
