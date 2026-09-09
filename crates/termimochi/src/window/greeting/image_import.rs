//! A cancelable image-to-ANSI draft. One coalescing worker owns the decoded image.
use super::*;
use crate::greeting_art::Artwork;
use crate::greeting_image::source::{EditableArtwork, SourceImage};
use crate::greeting_image::{
    self, Adjustments, ConvertedImage, Ink, Options, Removal, Structure, Style,
};

type Request = (u64, Options);

struct ImageImport {
    window: glib::WeakRef<gtk::Window>,
    workbench: std::rc::Weak<Workbench>,
    before: GreetingSettings,
    columns: gtk::SpinButton,
    style: gtk::DropDown,
    invert: gtk::CheckButton,
    preset: gtk::DropDown,
    ink: gtk::DropDown,
    structure: gtk::DropDown,
    ascii_controls: gtk::Expander,
    exposure: gtk::Scale,
    contrast: gtk::Scale,
    saturation: gtk::Scale,
    smoothing: gtk::Scale,
    edges: gtk::Scale,
    trim: gtk::CheckButton,
    crop: [gtk::SpinButton; 4],
    remove_background: gtk::CheckButton,
    removal_controls: gtk::Box,
    background_mode: gtk::DropDown,
    background_color: gtk::Entry,
    pick_color: gtk::ToggleButton,
    tolerance: gtk::Scale,
    softness: gtk::Scale,
    connected: gtk::CheckButton,
    reference: RefCell<Option<image::RgbaImage>>,
    result_status: RefCell<String>,
    status: gtk::Label,
    apply: gtk::Button,
    terminal: vte::Terminal,
    canvas: gtk::Stack,
    source: gtk::Picture,
    source_frame: gtk::Fixed,
    viewport: gtk::ScrolledWindow,
    fit: gtk::ToggleButton,
    font: gtk::pango::FontDescription,
    display_grid: Cell<(u32, u32)>,
    viewport_size: Cell<(i32, i32)>,
    last_style: Cell<Style>,
    ascii_columns: Cell<f64>,
    symbol_columns: Cell<f64>,
    updating_controls: Cell<bool>,
    options: Options,
    generation: Cell<u64>,
    artwork: RefCell<Option<Artwork>>,
    editable: RefCell<Option<EditableArtwork>>,
    sender: RefCell<Option<mpsc::Sender<Request>>>,
}

fn image_slider(label: &str, low: f64, high: f64, step: f64, value: f64) -> (gtk::Box, gtk::Scale) {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, low, high, step);
    scale.set_value(value);
    scale.set_draw_value(true);
    scale.set_value_pos(gtk::PositionType::Right);
    scale.set_digits(2);
    scale.update_property(&[gtk::accessible::Property::Label(label)]);
    let field = gtk::Box::new(gtk::Orientation::Vertical, 2);
    field.append(&gtk::Label::builder().label(label).xalign(0.0).build());
    field.append(&scale);
    (field, scale)
}

// Picture coordinates are logical pixels, including at fractional/HiDPI scales.
// ContentFit::Contain centers the image; clicks in the letterbox are not colors.
fn reference_pixel(image: &image::RgbaImage, size: (i32, i32), x: f64, y: f64) -> Option<[u8; 3]> {
    if size.0 <= 0
        || size.1 <= 0
        || image.width() == 0
        || image.height() == 0
        || !x.is_finite()
        || !y.is_finite()
    {
        return None;
    }
    let scale = (f64::from(size.0) / f64::from(image.width()))
        .min(f64::from(size.1) / f64::from(image.height()));
    let px = (x - (f64::from(size.0) - f64::from(image.width()) * scale) / 2.0) / scale;
    let py = (y - (f64::from(size.1) - f64::from(image.height()) * scale) / 2.0) / scale;
    if px < 0.0 || py < 0.0 || px >= f64::from(image.width()) || py >= f64::from(image.height()) {
        return None;
    }
    greeting_image::straight_color(image.get_pixel(px as u32, py as u32))
}

impl Workbench {
    pub(super) fn open_image_artwork(self: &Rc<Self>, before: GreetingSettings, path: PathBuf) {
        self.open_image_source(before, Some(path), None);
    }

    pub(in crate::window) fn edit_image_artwork(self: &Rc<Self>) {
        let before = self.greeting.settings();
        if let Some(source) = before.editable_artwork.clone() {
            self.open_image_source(before, None, Some(source));
        } else {
            self.toast("This artwork has no editable source. Reimport its original PNG, JPG or WebP; previous conversion settings cannot be recovered.");
            self.choose_original_image();
        }
    }

    fn open_image_source(
        self: &Rc<Self>,
        before: GreetingSettings,
        path: Option<PathBuf>,
        saved: Option<EditableArtwork>,
    ) {
        if let Some(dialog) = self.greeting.image_import_window.upgrade() {
            dialog.present();
            return;
        }
        if !self.art_draft_matches(&before) {
            return;
        }
        let model = self.model.borrow();
        let variant = model.palette.variant(model.active_variant).unwrap();
        let background = variant.get("Background").unwrap_or(Rgb::new(0, 0, 0));
        let foreground = variant.get("Foreground").unwrap_or(Rgb::new(255, 255, 255));
        drop(model);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        content.set_margin_start(18);
        content.set_margin_end(18);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = gtk::Label::builder()
            .label("Image to Text")
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["heading"])
            .build();
        let cancel = gtk::Button::with_label("Cancel");
        let apply = gtk::Button::with_label("Use Artwork");
        apply.add_css_class("suggested-action");
        apply.set_sensitive(false);
        header.append(&title);
        header.append(&cancel);
        header.append(&apply);
        content.append(&header);
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 20);
        body.set_vexpand(true);
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 16);
        controls.add_css_class("editor-workspace");
        controls.set_margin_end(8);
        let sidebar = gtk::ScrolledWindow::builder()
            .child(&controls)
            .hexpand(false)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_width(260)
            .max_content_width(260)
            .build();
        sidebar.set_size_request(260, -1);
        body.append(&sidebar);
        let viewing = gtk::Box::new(gtk::Orientation::Vertical, 14);
        viewing.set_hexpand(true);
        body.append(&viewing);
        content.append(&body);
        let columns = gtk::SpinButton::with_range(8.0, 120.0, 1.0);
        columns.set_value(64.0);
        columns.set_numeric(true);
        columns.set_tooltip_text(Some(
            "Maximum width. Aspect ratio is preserved within 64 rows and 3072 cells.",
        ));
        columns.update_property(&[gtk::accessible::Property::Label("Image Maximum Columns")]);
        let style = layout_drop_down(
            &["ANSI Detail", "ASCII", "Half blocks"],
            0,
            "Image Character Style",
            "ANSI Detail matches fine block and diagonal shapes with two colors per cell. ASCII combines tone with connected contours. Half blocks uses two samples per cell.",
        );
        let invert = gtk::CheckButton::with_label("Invert density");
        invert.set_tooltip_text(Some(
            "Reverse ASCII shading density. Contours-only output is unaffected.",
        ));
        controls.append(&typography_field("Characters", &style));
        controls.append(&typography_field("Max columns", &columns));
        let preset = layout_drop_down(
            &["Balanced", "Illustration", "Line Art", "Photo", "Custom"],
            0,
            "Image Recipe",
            "Starting points for adjustments. Changes stay in this import draft until Use Artwork.",
        );
        controls.append(&typography_field("Recipe", &preset));
        let ink = layout_drop_down(
            &["Source colors", "Grayscale", "Theme ink"],
            0,
            "Image Color Mode",
            "Theme ink uses your terminal foreground and background. Source colors retain image colors before adjustments.",
        );
        controls.append(&typography_field("Color", &ink));
        let removal = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let remove_background = gtk::CheckButton::with_label("Remove background");
        removal.append(&remove_background);
        let removal_controls = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let background_mode = layout_drop_down(
            &["Automatic", "Custom color"],
            0,
            "Background Color Source",
            "Automatic requires a near-uniform opaque border. Use Custom color for manual selection. This is solid-color removal, not AI subject detection.",
        );
        background_mode.set_halign(gtk::Align::Fill);
        removal_controls.append(&background_mode);
        let color_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let background_color = gtk::Entry::builder()
            .text("#FFFFFF")
            .width_chars(8)
            .max_width_chars(8)
            .hexpand(true)
            .build();
        background_color
            .update_property(&[gtk::accessible::Property::Label("Background HEX Color")]);
        background_color.set_tooltip_text(Some("Background color in #RRGGBB format."));
        let pick_color = gtk::ToggleButton::with_label("Pick");
        pick_color.set_tooltip_text(Some(
            "Click a background pixel in Original. Press Esc or Pick again to cancel.",
        ));
        color_row.append(&background_color);
        color_row.append(&pick_color);
        removal_controls.append(&color_row);
        let (field, tolerance) = image_slider("Tolerance %", 0.0, 50.0, 1.0, 8.0);
        tolerance.set_digits(0);
        tolerance.set_tooltip_text(Some(
            "Higher values remove a wider range of similar colors; start low to protect details.",
        ));
        removal_controls.append(&field);
        let (field, softness) = image_slider("Edge softness %", 0.0, 25.0, 1.0, 4.0);
        softness.set_digits(0);
        softness.set_tooltip_text(Some("Fade near-matching colors to transparency. This is a color transition range, not a blur radius."));
        removal_controls.append(&field);
        let connected = gtk::CheckButton::with_label("Connected to edges only");
        connected.set_active(true);
        connected.set_tooltip_text(Some("Protect enclosed same-color details. Gaps in an outline can still let removal reach the inside. Turn off to remove matching colors everywhere."));
        removal_controls.append(&connected);
        removal.append(&removal_controls);
        controls.append(
            &gtk::Expander::builder()
                .label("Background")
                .expanded(true)
                .child(&removal)
                .build(),
        );
        let tone = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let (field, exposure) = image_slider("Exposure (EV)", -2.0, 2.0, 0.05, 0.0);
        tone.append(&field);
        let (field, contrast) = image_slider("Contrast", 0.5, 2.0, 0.05, 1.0);
        tone.append(&field);
        let (field, saturation) = image_slider("Saturation", 0.0, 2.0, 0.05, 1.0);
        tone.append(&field);
        let (field, smoothing) = image_slider("Smooth noise", 0.0, 1.0, 0.05, 0.0);
        tone.append(&field);
        controls.append(
            &gtk::Expander::builder()
                .label("Tone & detail")
                .expanded(true)
                .child(&tone)
                .build(),
        );
        let contours = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let structure = layout_drop_down(
            &["Balanced", "Contours only", "Tone only"],
            0,
            "ASCII Structure",
            "Connected edges preserve silhouettes. Contours only removes tonal fill; Tone only disables edge emphasis.",
        );
        contours.append(&structure);
        let (field, edges) = image_slider("Edge sensitivity", 0.0, 2.0, 0.05, 0.7);
        contours.append(&field);
        contours.append(&invert);
        let ascii_controls = gtk::Expander::builder()
            .label("ASCII structure")
            .expanded(true)
            .child(&contours)
            .build();
        controls.append(&ascii_controls);
        let framing = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let trim = gtk::CheckButton::with_label("Trim empty margins");
        trim.set_tooltip_text(Some("Trim transparent or near-uniform outer margins. This does not remove a complex background. Manual crop is applied first."));
        framing.append(&trim);
        let crop: [gtk::SpinButton; 4] = std::array::from_fn(|_| {
            let spin = gtk::SpinButton::with_range(0.0, 45.0, 1.0);
            spin.set_numeric(true);
            spin
        });
        let crop_grid = gtk::Grid::builder()
            .column_spacing(10)
            .row_spacing(8)
            .build();
        for (i, label) in ["Left %", "Top %", "Right %", "Bottom %"]
            .into_iter()
            .enumerate()
        {
            crop[i].update_property(&[gtk::accessible::Property::Label(label)]);
            crop_grid.attach(
                &typography_field(label, &crop[i]),
                (i % 2) as i32,
                (i / 2) as i32,
                1,
                1,
            );
        }
        framing.append(&crop_grid);
        controls.append(
            &gtk::Expander::builder()
                .label("Crop & margins")
                .child(&framing)
                .build(),
        );
        let reset = gtk::Button::with_label("Reset adjustments");
        reset.add_css_class("flat");
        reset.set_tooltip_text(Some(
            "Reset background removal, tone, structure, colors and crop. Keep the selected character mode and width.",
        ));
        controls.append(&reset);
        let terminal = vte::Terminal::builder()
            .input_enabled(false)
            .audible_bell(false)
            .bold_is_bright(false)
            .scrollback_lines(0)
            .build();
        let typography = self.typography_settings();
        let mut font = typography.font_description();
        font.set_size(11 * gtk::pango::SCALE);
        terminal.set_font(Some(&font));
        terminal.set_cell_height_scale(typography.line_height);
        terminal.set_cell_width_scale(typography.cell_width);
        terminal.set_colors(
            Some(&rgba_from_rgb(foreground)),
            Some(&rgba_from_rgb(background)),
            &[],
        );
        terminal.update_property(&[gtk::accessible::Property::Label(
            "Converted ANSI Artwork Preview",
        )]);
        let canvas = gtk::Stack::builder()
            .hhomogeneous(true)
            .vhomogeneous(true)
            .transition_type(gtk::StackTransitionType::None)
            .build();
        canvas.set_valign(gtk::Align::Start);
        canvas.set_halign(gtk::Align::Start);
        let source = gtk::Picture::builder()
            .can_shrink(true)
            .content_fit(gtk::ContentFit::Contain)
            .build();
        source.update_property(&[gtk::accessible::Property::Label("Original Image Reference")]);
        let source_frame = gtk::Fixed::new();
        source_frame.put(&source, 0.0, 0.0);
        canvas.add_titled(&source_frame, Some("source"), "Original");
        canvas.add_titled(&terminal, Some("converted"), "Converted");
        canvas.set_visible_child_name("converted");
        let compare = gtk::StackSwitcher::builder()
            .stack(&canvas)
            .halign(gtk::Align::Start)
            .css_classes(["variant-switch"])
            .build();
        compare.set_tooltip_text(Some("Compare the same crop at the same display size. Original retains source colors; only Converted receives tone and structure edits. Source images are never saved in presets."));
        let comparison = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        comparison.append(&compare);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        comparison.append(&spacer);
        let fit = gtk::ToggleButton::with_label("Fit preview");
        fit.set_active(true);
        fit.add_css_class("flat");
        fit.set_tooltip_text(Some("Scale the display to fit. Turn off for full-size characters and scrolling. Exported columns, rows and text never change."));
        comparison.append(&fit);
        viewing.append(&comparison);
        let viewport = gtk::ScrolledWindow::builder()
            .child(&canvas)
            .hexpand(true)
            .vexpand(true)
            .min_content_height(200)
            .build();
        viewport.add_css_class("terminal-viewport");
        viewport.set_widget_name("termimochi-terminal");
        viewing.append(&viewport);
        let status = gtk::Label::builder()
            .label("Reading image…")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        status.set_tooltip_text(Some("The original image copy and all conversion settings are kept in your TermiMochi preset/workspace for Edit Artwork. Exports to Fastfetch contain only final ANSI. Original image files are never modified. Sharing a preset also shares its embedded source image."));
        viewing.append(&status);
        let window = gtk::Window::builder()
            .title("Image to Text")
            .transient_for(&self.window())
            .modal(true)
            .destroy_with_parent(true)
            .default_width(1040)
            .default_height(780)
            .child(&content)
            .build();
        window.add_css_class("termimochi-window");
        let (sender, receiver) = mpsc::channel::<Request>();
        let (finished, results) = mpsc::sync_channel(1);
        let restored = match saved.as_ref().map(EditableArtwork::options).transpose() {
            Ok(options) => options,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        std::thread::spawn(move || {
            let image = saved
                .map(|saved| Ok(saved.image))
                .unwrap_or_else(|| {
                    path.as_deref()
                        .ok_or("Missing original image.".to_owned())
                        .and_then(SourceImage::read)
                })
                .and_then(|source| source.decode().map(|image| (source, image)));
            while let Ok(mut request) = receiver.recv() {
                // Rapid spin-button changes never create more decode workers.
                while let Ok(newest) = receiver.try_recv() {
                    request = newest;
                }
                let result = image
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|(source, image)| {
                        let editable = EditableArtwork::new(source.clone(), request.1)?;
                        image
                            .convert(request.1)
                            .map(|converted| (converted, editable))
                    });
                if finished.send((request.0, result)).is_err() {
                    break;
                }
            }
        });
        let options = restored.unwrap_or(Options {
            columns: 64,
            style: Style::Detail,
            cell_ratio: (self.preview_terminal.char_width().max(1) as f64
                / self.preview_terminal.char_height().max(1) as f64)
                .clamp(0.1, 2.0),
            background: [background.red(), background.green(), background.blue()],
            foreground: [foreground.red(), foreground.green(), foreground.blue()],
            invert: false,
            edits: Adjustments::default(),
        });
        let this = Rc::new(ImageImport {
            window: window.downgrade(),
            workbench: Rc::downgrade(self),
            before,
            columns,
            style,
            invert,
            preset,
            ink,
            structure,
            ascii_controls,
            exposure,
            contrast,
            saturation,
            smoothing,
            edges,
            trim,
            crop,
            remove_background,
            removal_controls,
            background_mode,
            background_color,
            pick_color,
            tolerance,
            softness,
            connected,
            reference: RefCell::new(None),
            result_status: RefCell::new(String::new()),
            status,
            apply,
            terminal,
            canvas,
            source,
            source_frame,
            viewport,
            fit,
            font,
            display_grid: Cell::new((0, 0)),
            viewport_size: Cell::new((0, 0)),
            last_style: Cell::new(Style::Detail),
            ascii_columns: Cell::new(64.0),
            symbol_columns: Cell::new(64.0),
            updating_controls: Cell::new(false),
            options,
            generation: Cell::new(0),
            artwork: RefCell::new(None),
            editable: RefCell::new(None),
            sender: RefCell::new(Some(sender)),
        });
        // The controller holds only a weak window, so closing the modal drops
        // its channel and decoded pixels rather than retaining a hidden draft.
        unsafe {
            window.set_data("termimochi-image-import", this.clone());
        }
        self.greeting.image_import_window.set(Some(&window));
        let weak = Rc::downgrade(&this);
        this.fit.connect_toggled(move |_| {
            if let Some(this) = weak.upgrade() {
                this.render_preview();
            }
        });
        let weak = Rc::downgrade(&this);
        this.viewport.add_tick_callback(move |viewport, _| {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if this.sender.borrow().is_none() {
                return glib::ControlFlow::Break;
            }
            let size = (viewport.width(), viewport.height());
            if this.viewport_size.replace(size) != size && this.fit.is_active() {
                this.render_preview();
            }
            glib::ControlFlow::Continue
        });
        let weak = Rc::downgrade(&this);
        this.columns.connect_value_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.queue();
            }
        });
        let weak = Rc::downgrade(&this);
        this.style.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                this.queue();
            }
        });
        let weak = Rc::downgrade(&this);
        this.invert.connect_toggled(move |_| {
            if let Some(this) = weak.upgrade() {
                this.custom_recipe();
                this.queue();
            }
        });
        for slider in [
            &this.exposure,
            &this.contrast,
            &this.saturation,
            &this.smoothing,
            &this.edges,
            &this.tolerance,
            &this.softness,
        ] {
            let weak = Rc::downgrade(&this);
            slider.connect_value_changed(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.custom_recipe();
                    this.queue();
                }
            });
        }
        for selector in [&this.ink, &this.structure, &this.background_mode] {
            let weak = Rc::downgrade(&this);
            selector.connect_selected_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.custom_recipe();
                    this.queue();
                }
            });
        }
        for checkbox in [&this.remove_background, &this.connected] {
            let weak = Rc::downgrade(&this);
            checkbox.connect_toggled(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.custom_recipe();
                    this.queue();
                }
            });
        }
        let weak = Rc::downgrade(&this);
        this.background_color.connect_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.custom_recipe();
                this.queue();
            }
        });
        let weak = Rc::downgrade(&this);
        this.pick_color.connect_toggled(move |pick| {
            if let Some(this) = weak.upgrade() {
                if pick.is_active() {
                    this.canvas.set_visible_child_name("source");
                    this.source.set_cursor_from_name(Some("crosshair"));
                    this.status
                        .set_text("Click the background in Original. Esc cancels.");
                } else {
                    this.source.set_cursor_from_name(None);
                    this.status.set_text(&this.result_status.borrow());
                }
            }
        });
        let gesture = gtk::GestureClick::new();
        gesture.set_button(1);
        let weak = Rc::downgrade(&this);
        gesture.connect_released(move |_, _, x, y| {
            if let Some(this) = weak.upgrade() {
                this.pick_at(x, y);
            }
        });
        this.source.add_controller(gesture);
        for crop in &this.crop {
            let weak = Rc::downgrade(&this);
            crop.connect_value_changed(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.custom_recipe();
                    this.queue();
                }
            });
        }
        let weak = Rc::downgrade(&this);
        this.trim.connect_toggled(move |_| {
            if let Some(this) = weak.upgrade() {
                this.custom_recipe();
                this.queue();
            }
        });
        let weak = Rc::downgrade(&this);
        this.preset.connect_selected_notify(move |preset| {
            if let Some(this) = weak.upgrade()
                && !this.updating_controls.get()
                && preset.selected() < 4
            {
                this.recipe(preset.selected());
            }
        });
        let weak = Rc::downgrade(&this);
        reset.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.updating_controls.set(true);
                this.remove_background.set_active(false);
                this.background_mode.set_selected(0);
                this.background_color.set_text("#FFFFFF");
                this.tolerance.set_value(8.0);
                this.softness.set_value(4.0);
                this.connected.set_active(true);
                this.updating_controls.set(false);
                this.recipe(0);
            }
        });
        let weak = Rc::downgrade(&this);
        this.apply.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.apply();
            }
        });
        let weak = window.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.close();
            }
        });
        let weak = Rc::downgrade(&this);
        window.connect_close_request(move |_| {
            if let Some(this) = weak.upgrade() {
                this.stop();
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(&this);
        window.connect_destroy(move |_| {
            if let Some(this) = weak.upgrade() {
                this.stop();
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&this);
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                if let Some(this) = weak.upgrade() {
                    if this.pick_color.is_active() {
                        this.pick_color.set_active(false);
                    } else if let Some(window) = this.window.upgrade() {
                        window.close();
                    }
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(keys);
        let weak = Rc::downgrade(&this);
        glib::timeout_add_local(Duration::from_millis(40), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if this.sender.borrow().is_none() {
                return glib::ControlFlow::Break;
            }
            loop {
                match results.try_recv() {
                    Ok((generation, result)) => {
                        if generation == this.generation.get() {
                            this.finish(result.map(|(image, source)| {
                                *this.editable.borrow_mut() = Some(source);
                                image
                            }));
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                    Err(_) => {
                        this.finish(Err(
                            "Image converter stopped. Close this dialog and try again.".into(),
                        ));
                        return glib::ControlFlow::Break;
                    }
                }
            }
        });
        if let Some(options) = restored {
            this.restore_options(options);
        }
        window.present();
        this.queue();
    }
}

impl ImageImport {
    fn restore_options(&self, options: Options) {
        self.updating_controls.set(true);
        self.preset.set_selected(4);
        self.style.set_selected(match options.style {
            Style::Detail => 0,
            Style::Ascii => 1,
            Style::HalfBlocks => 2,
        });
        self.last_style.set(options.style);
        self.columns
            .set_range(8.0, f64::from(greeting_image::grid_limits(options.style).0));
        self.columns.set_value(f64::from(options.columns));
        self.ascii_columns.set(f64::from(options.columns));
        self.symbol_columns.set(f64::from(options.columns));
        self.invert.set_active(options.invert);
        let edits = options.edits;
        self.exposure.set_value(edits.exposure);
        self.contrast.set_value(edits.contrast);
        self.saturation.set_value(edits.saturation);
        self.smoothing.set_value(edits.smoothing);
        self.edges.set_value(edits.edges);
        self.trim.set_active(edits.trim);
        for (control, value) in self.crop.iter().zip(edits.crop) {
            control.set_value(f64::from(value));
        }
        self.ink.set_selected(match edits.ink {
            Ink::Color => 0,
            Ink::Gray => 1,
            Ink::Theme => 2,
        });
        self.structure.set_selected(match edits.structure {
            Structure::Balanced => 0,
            Structure::Outline => 1,
            Structure::Tone => 2,
        });
        self.remove_background.set_active(edits.removal.enabled);
        self.background_mode
            .set_selected(u32::from(edits.removal.color.is_some()));
        if let Some([r, g, b]) = edits.removal.color {
            self.background_color
                .set_text(&format!("#{r:02X}{g:02X}{b:02X}"));
        }
        self.tolerance.set_value(edits.removal.tolerance);
        self.softness.set_value(edits.removal.softness);
        self.connected.set_active(edits.removal.connected);
        self.updating_controls.set(false);
    }
    fn pick_at(&self, x: f64, y: f64) {
        if !self.pick_color.is_active() || !self.pick_color.is_sensitive() {
            return;
        }
        let color = self.reference.borrow().as_ref().and_then(|image| {
            reference_pixel(image, (self.source.width(), self.source.height()), x, y)
        });
        let Some([r, g, b]) = color else {
            self.status
                .set_text("Pick an opaque pixel inside the image, not the empty margin.");
            return;
        };
        self.updating_controls.set(true);
        self.background_mode.set_selected(1);
        self.background_color
            .set_text(&format!("#{r:02X}{g:02X}{b:02X}"));
        self.updating_controls.set(false);
        self.pick_color.set_active(false);
        self.canvas.set_visible_child_name("converted");
        self.custom_recipe();
        self.queue();
    }
    fn custom_recipe(&self) {
        if !self.updating_controls.get() {
            self.preset.set_selected(4);
        }
    }
    fn recipe(&self, index: u32) {
        let edits = Adjustments::preset(index);
        self.updating_controls.set(true);
        self.preset.set_selected(index);
        self.exposure.set_value(edits.exposure);
        self.contrast.set_value(edits.contrast);
        self.saturation.set_value(edits.saturation);
        self.smoothing.set_value(edits.smoothing);
        self.edges.set_value(edits.edges);
        self.trim.set_active(edits.trim);
        for crop in &self.crop {
            crop.set_value(0.0);
        }
        self.ink.set_selected(match edits.ink {
            Ink::Color => 0,
            Ink::Gray => 1,
            Ink::Theme => 2,
        });
        self.structure.set_selected(match edits.structure {
            Structure::Balanced => 0,
            Structure::Outline => 1,
            Structure::Tone => 2,
        });
        self.invert.set_active(false);
        if index == 2 {
            self.style.set_selected(1);
        }
        self.updating_controls.set(false);
        self.queue();
    }
    fn adjustments(&self) -> Adjustments {
        Adjustments {
            exposure: self.exposure.value(),
            contrast: self.contrast.value(),
            saturation: self.saturation.value(),
            smoothing: self.smoothing.value(),
            edges: self.edges.value(),
            trim: self.trim.is_active(),
            crop: std::array::from_fn(|i| self.crop[i].value_as_int() as u8),
            removal: Removal {
                enabled: self.remove_background.is_active(),
                color: (self.background_mode.selected() == 1).then(|| {
                    let c = self
                        .background_color
                        .text()
                        .parse::<Rgb>()
                        .unwrap_or(Rgb::new(255, 255, 255));
                    [c.red(), c.green(), c.blue()]
                }),
                tolerance: self.tolerance.value(),
                softness: self.softness.value(),
                connected: self.connected.is_active(),
            },
            ink: match self.ink.selected() {
                1 => Ink::Gray,
                2 => Ink::Theme,
                _ => Ink::Color,
            },
            structure: match self.structure.selected() {
                1 => Structure::Outline,
                2 => Structure::Tone,
                _ => Structure::Balanced,
            },
        }
    }
    fn stop(&self) {
        self.sender.borrow_mut().take();
        if let Some(workbench) = self.workbench.upgrade()
            && workbench.greeting.image_import_window.upgrade() == self.window.upgrade()
        {
            workbench
                .greeting
                .image_import_window
                .set(None::<&gtk::Window>);
        }
    }
    fn queue(&self) {
        if self.updating_controls.get() {
            return;
        }
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        self.artwork.borrow_mut().take();
        self.editable.borrow_mut().take();
        self.apply.set_sensitive(false);
        self.pick_color.set_active(false);
        self.pick_color.set_sensitive(false);
        self.status.set_text("Converting…");
        self.result_status.replace("Converting…".into());
        let removing = self.remove_background.is_active();
        self.removal_controls.set_sensitive(removing);
        self.background_color
            .set_sensitive(self.background_mode.selected() == 1);
        self.background_color.remove_css_class("error");
        if removing
            && self.background_mode.selected() == 1
            && self.background_color.text().parse::<Rgb>().is_err()
        {
            self.background_color.add_css_class("error");
            let message = "Enter a valid background color, such as #FFFFFF, or use Pick.";
            self.status.set_text(message);
            self.result_status.replace(message.into());
            self.pick_color
                .set_sensitive(self.reference.borrow().is_some());
            return;
        }
        let style = match self.style.selected() {
            1 => Style::Ascii,
            2 => Style::HalfBlocks,
            _ => Style::Detail,
        };
        if style != self.last_style.get() {
            self.updating_controls.set(true);
            if self.last_style.get() == Style::Ascii {
                self.ascii_columns.set(self.columns.value());
            } else {
                self.symbol_columns.set(self.columns.value());
            }
            let width = if style == Style::Ascii {
                self.ascii_columns.get()
            } else {
                self.symbol_columns.get()
            };
            self.columns
                .set_range(8.0, f64::from(greeting_image::grid_limits(style).0));
            self.columns.set_value(width);
            self.last_style.set(style);
            self.updating_controls.set(false);
        }
        self.columns.set_tooltip_text(Some(if style == Style::Ascii {
            "High-resolution ASCII: up to 160 columns × 96 rows. Fit preview changes display size, not text resolution. Wide exports need a wide terminal."
        } else {
            "Maximum width. Aspect ratio is preserved within 64 rows and 3072 cells. The actual fitted dimensions are shown below."
        }));
        let ascii = style == Style::Ascii;
        self.ascii_controls.set_visible(ascii);
        self.structure.set_sensitive(ascii);
        self.edges
            .set_sensitive(ascii && self.structure.selected() != 2);
        self.invert
            .set_sensitive(ascii && self.structure.selected() != 1);
        self.saturation.set_sensitive(self.ink.selected() == 0);
        let options = Options {
            columns: self.columns.value_as_int() as u32,
            style,
            invert: self.invert.is_active(),
            edits: self.adjustments(),
            ..self.options
        };
        if let Some(sender) = self.sender.borrow().as_ref() {
            let _ = sender.send((generation, options));
        }
    }
    fn finish(&self, result: Result<ConvertedImage, String>) {
        match result {
            Ok(image) => {
                let (width, height) = image.reference.dimensions();
                let texture = gdk::MemoryTexture::new(
                    width as i32,
                    height as i32,
                    gdk::MemoryFormat::R8g8b8a8Premultiplied,
                    &glib::Bytes::from_owned(image.reference.as_raw().clone()),
                    width as usize * 4,
                );
                self.source.set_paintable(Some(&texture));
                self.reference.replace(Some(image.reference));
                self.pick_color.set_sensitive(true);
                if self.background_mode.selected() == 0
                    && let Some([r, g, b]) = image.removal.color
                {
                    self.updating_controls.set(true);
                    self.background_color
                        .set_text(&format!("#{r:02X}{g:02X}{b:02X}"));
                    self.updating_controls.set(false);
                }
                let removal_note = if let Some(issue) = image.removal.issue {
                    format!(" · {issue}")
                } else if let Some([r, g, b]) = image.removal.color {
                    format!(
                        " · Removed #{r:02X}{g:02X}{b:02X} ({:.0}%)",
                        image.removal.changed_pixels as f64 * 100.0 / f64::from(width * height)
                    )
                } else {
                    String::new()
                };
                self.status.set_text(&format!(
                    "{} × {} cells{} · {} {} × {} px · {:.1} KiB ANSI{}",
                    image.columns,
                    image.rows,
                    if image.columns < self.columns.value_as_int() as u32 {
                        format!(" (fit from {} columns)", self.columns.value_as_int())
                    } else {
                        String::new()
                    },
                    image.format,
                    image.source_dimensions.0,
                    image.source_dimensions.1,
                    image.artwork.ansi.len() as f64 / 1024.0,
                    removal_note
                ));
                self.result_status.replace(self.status.text().to_string());
                self.display_grid.set((image.columns, image.rows));
                self.artwork.replace(Some(image.artwork));
                self.render_preview();
                self.apply.set_sensitive(image.removal.issue.is_none());
            }
            Err(error) => {
                self.artwork.borrow_mut().take();
                self.apply.set_sensitive(false);
                self.terminal.reset(true, true);
                self.source.set_paintable(None::<&gdk::Paintable>);
                self.reference.borrow_mut().take();
                self.pick_color.set_sensitive(false);
                self.status.set_text(&error);
                self.result_status.replace(error);
            }
        }
    }
    fn render_preview(&self) {
        let artwork = self.artwork.borrow();
        let Some(artwork) = artwork.as_ref() else {
            return;
        };
        let (columns, rows) = self.display_grid.get();
        if columns == 0 || rows == 0 {
            return;
        }
        let (available_width, available_height) = (
            self.viewport.width().max(100) - 8,
            self.viewport.height().max(100) - 8,
        );
        let mut size = 11.0;
        for _ in 0..5 {
            let mut font = self.font.clone();
            font.set_size((size * f64::from(gtk::pango::SCALE)) as i32);
            self.terminal.set_font(Some(&font));
            if !self.fit.is_active() {
                break;
            }
            let width = self.terminal.char_width().max(1) as f64 * f64::from(columns);
            let height = self.terminal.char_height().max(1) as f64 * f64::from(rows);
            let scale =
                (f64::from(available_width) / width).min(f64::from(available_height) / height);
            if scale >= 1.0 || size <= 2.0 {
                break;
            }
            size = (size * scale * 0.97).max(2.0);
        }
        let width = self.terminal.char_width().max(1) as i32 * columns as i32;
        let height = self.terminal.char_height().max(1) as i32 * rows as i32;
        self.terminal.reset(true, true);
        self.terminal.set_size(i64::from(columns), i64::from(rows));
        self.terminal.set_size_request(width, height);
        self.canvas.set_size_request(width, height);
        self.source_frame.set_size_request(width, height);
        self.source.set_size_request(width, height);
        // VTE consumes feed asynchronously: clear/home must be in the same
        // stream as this frame, after any earlier generation still queued.
        self.terminal.feed(
            format!(
                "\x1b[0m\x1b[2J\x1b[H\x1b[?25l{}",
                artwork.ansi.replace('\n', "\r\n")
            )
            .as_bytes(),
        );
        self.viewport.vadjustment().set_value(0.0);
        self.viewport.hadjustment().set_value(0.0);
    }
    fn apply(&self) {
        if !self.apply.is_sensitive() {
            return;
        }
        let Some(workbench) = self.workbench.upgrade() else {
            return;
        };
        let Some(artwork) = self.artwork.borrow().clone() else {
            return;
        };
        if !workbench.art_draft_matches(&self.before) {
            self.status.set_text("Greeting changed while this dialog was open. Close and import again to keep your latest edits.");
            self.apply.set_sensitive(false);
            return;
        }
        let mut next = self.before.clone();
        let prepared = next
            .import_artwork(artwork)
            .and_then(|()| {
                next.editable_artwork = Some(
                    self.editable
                        .borrow()
                        .clone()
                        .ok_or("Editable source is not ready.")?,
                );
                Ok(())
            })
            .and_then(|()| crate::fastfetch_document::value(&next.fastfetch_config()?).map(|_| ()))
            .and_then(|()| {
                crate::document_store::encode(&GreetingPreset::new(next.clone())).map(|_| ())
            });
        if let Err(error) = prepared {
            self.status.set_text(&error);
            return;
        }
        workbench.greeting.replace(next, true);
        workbench.greeting_module_button.set_active(true);
        workbench.toast("Image imported as ANSI artwork. Save Preset to keep it; Undo restores the previous logo.");
        if let Some(window) = self.window.upgrade() {
            window.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{controller, descendants, feed, respond, settle, wait_official};
    use super::*;

    fn active_art(settings: &GreetingSettings) -> Option<Artwork> {
        if settings.imported_source.is_some() {
            settings
                .source_logo
                .as_ref()
                .map(|snapshot| snapshot.artwork.clone())
        } else {
            settings.custom_art.clone()
        }
    }

    fn run_after_control() -> gtk::CheckButton {
        gtk::Window::list_toplevels()
            .into_iter()
            .flat_map(|w| descendants(&w))
            .find_map(|w| {
                w.downcast::<gtk::CheckButton>().ok().filter(|w| {
                    w.label().as_deref() == Some("Run Fastfetch in a new terminal after applying")
                })
            })
            .unwrap()
    }

    #[test]
    #[ignore = "requires GTK/VTE; embedded source, local temporary presets only"]
    fn editable_artwork_restores_all_controls_after_restart_and_source_deletion() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.EditableArtworkTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        let preset = root.path().join(typography_preset::PRESET_NAME);
        let source = root.path().join("original.png");
        image::RgbaImage::from_fn(160, 120, |x, y| {
            if !(30..130).contains(&x) || !(20..100).contains(&y) {
                image::Rgba([255; 4])
            } else {
                image::Rgba([(x * 2) as u8, (y * 2) as u8, 80, 255])
            }
        })
        .save(&source)
        .unwrap();
        present_with_preset(&app, None, preset.clone());
        let window = app.active_window().unwrap();
        let this = controller(&window);
        let (_, preview) = draft(&this, &source);
        ready(&preview);
        let options = Options {
            columns: 88,
            style: Style::Ascii,
            cell_ratio: 0.5,
            background: [240; 3],
            foreground: [25; 3],
            invert: true,
            edits: Adjustments {
                exposure: 0.25,
                contrast: 1.2,
                saturation: 0.7,
                smoothing: 0.2,
                edges: 1.3,
                trim: true,
                crop: [5, 5, 5, 5],
                ink: Ink::Gray,
                structure: Structure::Balanced,
                removal: Removal {
                    enabled: true,
                    color: Some([255; 3]),
                    tolerance: 10.0,
                    softness: 6.0,
                    connected: true,
                },
            },
        };
        // Environment-dependent cell proportions/colors remain the captured
        // ones; all user-facing controls use the non-default recipe below.
        let options = Options {
            cell_ratio: preview.options.cell_ratio,
            background: preview.options.background,
            foreground: preview.options.foreground,
            ..options
        };
        preview.restore_options(options);
        preview.queue();
        let artwork = ready(&preview);
        preview.apply.emit_clicked();
        let saved = this.greeting.settings();
        assert_eq!(
            saved.editable_artwork.as_ref().unwrap().options().unwrap(),
            options
        );
        this.save_greeting_preset();
        let workspace_path = root.path().join("editable.termimochi.json");
        this.save_workspace_path(workspace_path.clone(), this.workspace_snapshot())
            .unwrap();
        std::fs::remove_file(&source).unwrap();
        window.destroy();
        drop(this);
        settle();
        present_with_preset(&app, None, preset);
        let window = app.active_window().unwrap();
        let this = controller(&window);
        assert_eq!(this.greeting.settings(), saved);
        let reopen = || {
            this.edit_image_artwork();
            let dialog = this.greeting.image_import_window.upgrade().unwrap();
            let preview = unsafe {
                dialog
                    .data::<Rc<ImageImport>>("termimochi-image-import")
                    .unwrap()
                    .as_ref()
                    .clone()
            };
            (dialog, preview)
        };
        let (dialog, preview) = reopen();
        assert_eq!(ready(&preview), artwork);
        assert_eq!(preview.adjustments(), options.edits);
        assert_eq!(preview.columns.value_as_int(), 88);
        assert!(preview.invert.is_active());
        preview.contrast.set_value(1.6);
        ready(&preview);
        dialog.close();
        settle();
        assert_eq!(
            this.greeting.settings(),
            saved,
            "Cancel must keep the saved source recipe"
        );
        let (_, preview) = reopen();
        assert_eq!(ready(&preview), artwork);
        preview.remove_background.set_active(false);
        assert_ne!(ready(&preview), artwork);
        preview.apply.emit_clicked();
        assert_ne!(this.greeting.settings(), saved);
        this.greeting.undo();
        assert_eq!(this.greeting.settings(), saved);
        this.greeting.redo();
        assert!(
            !this
                .greeting
                .settings()
                .editable_artwork
                .unwrap()
                .options()
                .unwrap()
                .edits
                .removal
                .enabled
        );
        this.greeting.undo();
        this.open_workspace_path(&workspace_path);
        assert_eq!(this.greeting.settings(), saved);
        let (dialog, preview) = reopen();
        assert_eq!(ready(&preview), artwork);
        dialog.close();
        settle();
        window.destroy();
    }

    fn draft(workbench: &Rc<Workbench>, path: &Path) -> (gtk::Window, Rc<ImageImport>) {
        workbench.open_image_artwork(workbench.greeting.settings(), path.into());
        let window = workbench.greeting.image_import_window.upgrade().unwrap();
        let draft = unsafe {
            window
                .data::<Rc<ImageImport>>("termimochi-image-import")
                .unwrap()
                .as_ref()
                .clone()
        };
        (window, draft)
    }

    fn ready(draft: &ImageImport) -> Artwork {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !draft.apply.is_sensitive() {
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                draft.status.text()
            );
            settle();
        }
        settle();
        let artwork = draft.artwork.borrow().clone().unwrap();
        let normalize = |text: &str| {
            text.lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
                .trim_end()
                .to_owned()
        };
        let expected = normalize(&artwork.plain);
        loop {
            let actual = normalize(
                draft
                    .terminal
                    .text_format(vte::Format::Text)
                    .as_deref()
                    .unwrap_or(""),
            );
            if actual == expected {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Visible VTE frame differs from the draft at {} × {}: actual {actual:?}; expected {expected:?}",
                draft.terminal.column_count(),
                draft.terminal.row_count()
            );
            settle();
        }
        artwork
    }

    fn capture(window: &gtk::Window, key: &str) {
        if let Some(file) = std::env::var_os(key) {
            let previous = window.title();
            window.set_title(Some("TermiMochi point-to-edit test"));
            let mut child = std::process::Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../scripts/preview-pointer-driver.py"
                ))
                .args(["capture", "0", "0"])
                .env("TERMIMOCHI_INSPECT_SCREENSHOT", file)
                .spawn()
                .unwrap();
            while child.try_wait().unwrap().is_none() {
                settle();
            }
            assert!(child.wait().unwrap().success());
            window.set_title(previous.as_deref());
        }
    }

    #[test]
    fn color_picker_maps_contained_images_and_rejects_empty_space() {
        let pixels =
            image::RgbaImage::from_raw(2, 1, vec![255, 255, 255, 255, 64, 32, 16, 128]).unwrap();
        for multiplier in [1, 2] {
            let size = (200 * multiplier, 200 * multiplier);
            let m = f64::from(multiplier);
            assert_eq!(
                reference_pixel(&pixels, size, 50.0 * m, 100.0 * m),
                Some([255; 3])
            );
            assert_eq!(
                reference_pixel(&pixels, size, 150.0 * m, 100.0 * m),
                Some([128, 64, 32])
            );
            for (x, y) in [
                (50.0, 49.0),
                (50.0, 150.0),
                (-1.0, 100.0),
                (200.0, 100.0),
                (f64::NAN, 0.0),
            ] {
                assert_eq!(reference_pixel(&pixels, size, x * m, y * m), None);
            }
        }
        assert_eq!(reference_pixel(&pixels, (0, 0), 0.0, 0.0), None);
        assert_eq!(
            reference_pixel(&image::RgbaImage::new(2, 1), (200, 100), 50.0, 50.0),
            None
        );
    }

    fn background_controls(this: &Rc<Workbench>, root: &Path) {
        let before = this.greeting.settings();
        let file = root.join("background-fixture.png");
        image::RgbaImage::from_fn(96, 96, |x, y| {
            if (24..72).contains(&x)
                && (24..72).contains(&y)
                && !((36..60).contains(&x) && (36..60).contains(&y))
            {
                image::Rgba([30, 55, 90, 255])
            } else {
                image::Rgba([255; 4])
            }
        })
        .save(&file)
        .unwrap();
        let bytes = std::fs::read(&file).unwrap();
        let (dialog, preview) = draft(this, &file);
        let untouched = ready(&preview);
        assert!(!preview.remove_background.is_active());
        preview.remove_background.set_active(true);
        let connected = ready(&preview);
        assert_ne!(connected, untouched);
        assert!(preview.status.text().contains("Removed #FFFFFF"));
        assert_eq!(
            preview.reference.borrow().as_ref().unwrap().get_pixel(0, 0),
            &image::Rgba([255; 4])
        );
        preview.connected.set_active(false);
        assert_ne!(ready(&preview), connected);
        preview.connected.set_active(true);
        assert_eq!(ready(&preview), connected);
        // Preset changes preserve the user's background key; Reset clears it.
        preview.preset.set_selected(3);
        ready(&preview);
        assert!(preview.remove_background.is_active());
        preview.recipe(0);
        assert_eq!(ready(&preview), connected);
        preview.background_mode.set_selected(1);
        ready(&preview);
        preview.background_color.set_text("#not-a-color");
        settle();
        assert!(!preview.apply.is_sensitive());
        assert!(preview.background_color.has_css_class("error"));
        preview.apply();
        assert_eq!(this.greeting.settings(), before);
        preview.pick_color.set_active(true);
        settle();
        let keys = dialog.observe_controllers();
        let keys = (0..keys.n_items())
            .find_map(|i| keys.item(i)?.downcast::<gtk::EventControllerKey>().ok())
            .unwrap();
        let stopped = keys.emit_by_name::<bool>(
            "key-pressed",
            &[&gdk::Key::Escape, &0u32, &gdk::ModifierType::empty()],
        );
        assert!(stopped);
        assert!(!preview.pick_color.is_active());
        assert!(this.greeting.image_import_window.upgrade().is_some());
        preview.pick_color.set_active(true);
        settle();
        preview.pick_at(-1.0, -1.0);
        assert!(preview.pick_color.is_active());
        // Emit the actual Picture gesture, using the same logical mapping at 1x/2x.
        let side = preview.source.width().min(preview.source.height()) as f64;
        let x = (preview.source.width() as f64 - side) / 2.0 + side * 0.1;
        let y = (preview.source.height() as f64 - side) / 2.0 + side * 0.1;
        let controllers = preview.source.observe_controllers();
        let gesture = (0..controllers.n_items())
            .find_map(|i| controllers.item(i)?.downcast::<gtk::GestureClick>().ok())
            .unwrap();
        gesture.emit_by_name::<()>("released", &[&1i32, &x, &y]);
        assert!(!preview.pick_color.is_active());
        assert_eq!(preview.background_color.text(), "#FFFFFF");
        assert_eq!(ready(&preview), connected);
        preview.background_color.set_text("#00FF00");
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !preview
            .status
            .text()
            .contains("No background pixels matched")
        {
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                preview.status.text()
            );
            settle();
        }
        assert!(!preview.apply.is_sensitive());
        preview.apply();
        assert_eq!(this.greeting.settings(), before);
        preview.remove_background.set_active(false);
        assert_eq!(ready(&preview), untouched);
        preview.background_mode.set_selected(0);
        preview.remove_background.set_active(true);
        assert_eq!(ready(&preview), connected);
        for (index, style) in [
            (1, Style::Ascii),
            (2, Style::HalfBlocks),
            (0, Style::Detail),
        ] {
            preview.style.set_selected(index);
            let art = ready(&preview);
            assert!(
                art.plain.lines().next().unwrap().trim().is_empty(),
                "{style:?}"
            );
        }
        let removed = ready(&preview);
        preview.apply.emit_clicked();
        wait_official(this);
        assert_eq!(active_art(&this.greeting.settings()), Some(removed.clone()));
        let exported = root.join("removed.ans");
        crate::greeting_art::export_file(&exported, &removed.ansi, true, &None).unwrap();
        assert_eq!(crate::greeting_art::file_art(&exported).unwrap(), removed);
        super::super::export_fastfetch(
            &root.join("removed.fastfetch.jsonc"),
            &this.greeting.settings(),
            &None,
        )
        .unwrap();
        this.greeting.undo();
        settle();
        assert_eq!(this.greeting.settings(), before);
        this.greeting.redo();
        wait_official(this);
        assert_eq!(active_art(&this.greeting.settings()), Some(removed));
        this.greeting.undo();
        settle();
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
        drop(dialog);
        let (dialog, preview) = draft(this, &file);
        ready(&preview);
        preview.remove_background.set_active(true);
        ready(&preview);
        let reset = descendants(dialog.upcast_ref())
            .into_iter()
            .find_map(|w| {
                w.downcast::<gtk::Button>()
                    .ok()
                    .filter(|b| b.label().as_deref() == Some("Reset adjustments"))
            })
            .unwrap();
        reset.emit_clicked();
        assert_eq!(ready(&preview), untouched);
        assert_eq!(preview.adjustments(), Adjustments::default());
        dialog.close();
        settle();
        assert_eq!(this.greeting.settings(), before);

        // Ambiguous edges must stay available for picking, but not acceptance.
        let complex = root.join("complex-background.png");
        image::RgbaImage::from_fn(32, 32, |x, y| {
            if (x + y) % 2 == 0 {
                image::Rgba([255; 4])
            } else {
                image::Rgba([0, 0, 0, 255])
            }
        })
        .save(&complex)
        .unwrap();
        let (dialog, preview) = draft(this, &complex);
        ready(&preview);
        preview.remove_background.set_active(true);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !preview
            .status
            .text()
            .contains("No uniform background detected")
        {
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                preview.status.text()
            );
            settle();
        }
        assert!(preview.source.paintable().is_some());
        assert!(preview.pick_color.is_sensitive());
        assert!(!preview.apply.is_sensitive());
        preview.apply();
        assert_eq!(this.greeting.settings(), before);
        dialog.close();
        settle();
        let blank = root.join("blank-background.png");
        image::RgbaImage::from_pixel(16, 16, image::Rgba([255; 4]))
            .save(&blank)
            .unwrap();
        let (dialog, preview) = draft(this, &blank);
        let opaque = ready(&preview);
        preview.remove_background.set_active(true);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !preview.status.text().contains("No visible artwork remains") {
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                preview.status.text()
            );
            settle();
        }
        assert!(!preview.apply.is_sensitive());
        preview.remove_background.set_active(false);
        assert_eq!(ready(&preview), opaque);
        dialog.close();
        settle();
    }

    #[test]
    fn large_image_fastfetch_file_export_retains_backups_and_detects_conflicts() {
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("source.png");
        greeting_image::tests::fixture().save(&image).unwrap();
        let artwork = greeting_image::load(&image)
            .unwrap()
            .convert(Options {
                columns: 120,
                style: Style::HalfBlocks,
                cell_ratio: 0.5,
                background: [255; 3],
                foreground: [30; 3],
                invert: false,
                edits: Adjustments::default(),
            })
            .unwrap()
            .artwork;
        let mut settings = GreetingSettings::starter();
        settings.import_artwork(artwork.clone()).unwrap();
        let path = root.path().join("config.jsonc");
        super::super::export_fastfetch(&path, &settings, &None).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() > 65536);
        assert_eq!(
            crate::fastfetch_document::value(std::str::from_utf8(&bytes).unwrap()).unwrap()["logo"]
                ["source"],
            artwork.ansi
        );
        super::super::export_fastfetch(&path, &settings, &Some(bytes.clone())).unwrap();
        assert!(
            std::fs::read_dir(root.path().join("termimochi-backups"))
                .unwrap()
                .any(|e| std::fs::read(e.unwrap().path()).unwrap() == bytes)
        );
        std::fs::write(&path, "// external edit\n{}").unwrap();
        assert!(super::super::export_fastfetch(&path, &settings, &Some(bytes)).is_err());
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "// external edit\n{}"
        );
    }

    #[test]
    #[ignore = "requires GTK/VTE, Fastfetch and Bubblewrap; run separately at 1x and 2x"]
    fn image_import_real_vte_cancel_coalescing_undo_exports_and_restart() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.ImageImportTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        let preset = root.path().join(typography_preset::PRESET_NAME);
        let existing = std::env::var_os("TERMIMOCHI_GREETING_TEST_PRESET").map(|source| {
            let bytes = std::fs::read(source).unwrap();
            let document = crate::document_store::decode::<GreetingPreset>(&bytes).unwrap();
            std::fs::write(root.path().join(crate::greeting::PRESET_NAME), bytes).unwrap();
            document.greeting
        });
        let source = root.path().join("private-source.png");
        if let Some(reference) = std::env::var_os("TERMIMOCHI_IMAGE_TEST_SOURCE") {
            std::fs::copy(reference, &source).unwrap();
        } else {
            std::fs::write(
                &source,
                include_bytes!(
                    "../../../resources/icons/256x256/apps/io.github.miiikuuu.termimochi.png"
                ),
            )
            .unwrap();
        }
        present_with_preset(&app, None, preset.clone());
        let window = app.active_window().unwrap();
        window.set_default_size(1320, 850);
        let this = controller(&window);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        this.greeting_module_button.set_active(true);
        settle();
        let import = descendants(window.upcast_ref())
            .into_iter()
            .find_map(|w| {
                w.downcast::<gtk::Button>()
                    .ok()
                    .filter(|b| b.has_css_class("artwork-import"))
            })
            .unwrap();
        assert!(import.is_visible() && import.is_sensitive());
        assert!(!import.has_css_class("flat"));
        assert_eq!(
            import.action_name().as_deref(),
            Some("win.import-greeting-art")
        );
        assert!(import.width() > 100 && import.height() >= 32);
        let import_label = descendants(import.upcast_ref())
            .into_iter()
            .find_map(|w| {
                w.downcast::<gtk::Label>()
                    .ok()
                    .filter(|label| label.text() == "Import Artwork…")
            })
            .unwrap();
        let (label_width, _) = import_label.layout().pixel_size();
        assert!(
            import_label.width() >= label_width,
            "Import label is clipped"
        );
        let before = this.greeting.settings();
        if let Some(existing) = existing {
            assert_eq!(before, existing);
        }
        let layout = this.layout_settings();
        let typography = this.typography_settings();

        background_controls(&this, root.path());

        // Closing during decode cannot apply a late worker result or block reopen.
        let (dialog, pending) = draft(&this, &source);
        assert!(!pending.apply.is_sensitive());
        dialog.close();
        settle();
        assert!(pending.sender.borrow().is_none());
        assert_eq!(this.greeting.settings(), before);
        let (dialog, preview) = draft(&this, &source);
        let detailed = ready(&preview);
        assert_eq!(preview.style.selected(), 0);
        assert_eq!(preview.columns.value_as_int(), 64);
        assert!(!detailed.plain.is_ascii());
        assert!(preview.source.paintable().is_some());
        let converted_size = (preview.canvas.width(), preview.canvas.height());
        capture(&dialog, "TERMIMOCHI_IMAGE_DETAIL_SCREENSHOT");
        preview.canvas.set_visible_child_name("source");
        settle();
        assert_eq!(
            (preview.canvas.width(), preview.canvas.height()),
            converted_size
        );
        assert_eq!(*preview.artwork.borrow(), Some(detailed));
        capture(&dialog, "TERMIMOCHI_IMAGE_ORIGINAL_SCREENSHOT");
        preview.canvas.set_visible_child_name("converted");

        // Every recipe and editor control belongs only to this draft. The
        // reference keeps original colors but follows the selected crop.
        preview.columns.set_value(48.0);
        preview.preset.set_selected(1);
        let illustration = ready(&preview);
        assert_eq!(preview.adjustments(), Adjustments::preset(1));
        assert!(!preview.structure.is_sensitive());
        assert!(!preview.edges.is_sensitive());
        capture(&dialog, "TERMIMOCHI_IMAGE_EDIT_DETAIL_SCREENSHOT");
        if std::env::var_os("TERMIMOCHI_IMAGE_TEST_SOURCE").is_some() {
            preview.remove_background.set_active(true);
            let removed = ready(&preview);
            assert_ne!(removed, illustration);
            capture(&dialog, "TERMIMOCHI_IMAGE_REMOVED_SCREENSHOT");
            preview.remove_background.set_active(false);
            assert_eq!(ready(&preview), illustration);
        }
        let reference_size = preview.source.paintable().unwrap().intrinsic_width();
        preview.trim.set_active(false);
        ready(&preview);
        let full_width = preview.source.paintable().unwrap().intrinsic_width();
        assert!(full_width >= reference_size);
        preview.crop[0].set_value(15.0);
        ready(&preview);
        assert!(preview.source.paintable().unwrap().intrinsic_width() < full_width);
        assert_eq!(preview.preset.selected(), 4);
        preview.recipe(0);
        let neutral = ready(&preview);
        assert_eq!(preview.columns.value_as_int(), 48);
        assert_eq!(preview.adjustments(), Adjustments::default());
        assert_ne!(neutral, illustration);
        for slider in [
            &preview.exposure,
            &preview.contrast,
            &preview.saturation,
            &preview.smoothing,
        ] {
            let before = ready(&preview);
            slider.set_value(slider.value() + 0.5);
            assert!(!preview.apply.is_sensitive());
            assert_ne!(ready(&preview), before);
        }
        preview.preset.set_selected(2);
        let outline = ready(&preview);
        assert_eq!(preview.style.selected(), 1);
        assert!(outline.plain.chars().all(|c| " |/-\\+\n".contains(c)));
        assert!(!preview.invert.is_sensitive());
        assert!(!preview.saturation.is_sensitive());
        preview.columns.set_value(48.0);
        ready(&preview);
        capture(&dialog, "TERMIMOCHI_IMAGE_EDIT_OUTLINE_SCREENSHOT");
        preview.preset.set_selected(1);
        ready(&preview);
        capture(&dialog, "TERMIMOCHI_IMAGE_EDIT_ASCII_SCREENSHOT");
        preview.structure.set_selected(2);
        ready(&preview);
        assert!(!preview.edges.is_sensitive());
        preview.ink.set_selected(1);
        ready(&preview);
        preview.recipe(0);
        ready(&preview);
        assert_eq!(this.greeting.settings(), before);
        preview.columns.set_value(64.0);
        preview.style.set_selected(1);
        let ascii = ready(&preview);
        assert_eq!(preview.columns.value_as_int(), 64);
        assert_eq!(ascii.plain.lines().next().unwrap().chars().count(), 64);
        let generation = preview.generation.get();
        let fitted_font = preview.terminal.font().unwrap().size();
        preview.fit.set_active(false);
        settle();
        assert!(preview.terminal.font().unwrap().size() >= fitted_font);
        assert_eq!(*preview.artwork.borrow(), Some(ascii.clone()));
        preview.fit.set_active(true);
        settle();
        assert_eq!(preview.generation.get(), generation);
        assert_eq!(*preview.artwork.borrow(), Some(ascii.clone()));
        assert!(ascii.ansi.contains("38;2;"));
        assert!(!ascii.plain.contains('▀'));
        assert_eq!(this.greeting.settings(), before);
        capture(&dialog, "TERMIMOCHI_IMAGE_ASCII_SCREENSHOT");
        preview.invert.set_active(true);
        assert!(!preview.apply.is_sensitive());
        assert_ne!(ready(&preview).plain, ascii.plain);
        preview.columns.set_value(160.0);
        let high = ready(&preview);
        assert_eq!(high.plain.lines().next().unwrap().chars().count(), 160);
        capture(&dialog, "TERMIMOCHI_IMAGE_ASCII_HIGH_SCREENSHOT");
        preview.style.set_selected(2);
        for width in [120.0, 8.0, 96.0, 32.0, 64.0] {
            preview.columns.set_value(width);
        }
        let art = ready(&preview);
        assert!(art.plain.contains('▀'));
        assert!(!preview.invert.is_sensitive());
        assert_eq!(art.plain.lines().next().unwrap().chars().count(), 64);
        assert!(preview.terminal.width() <= preview.canvas.width());
        capture(&dialog, "TERMIMOCHI_IMAGE_BLOCKS_SCREENSHOT");

        // Conversion retains decoded pixels, never rereads or persists the path.
        std::fs::remove_file(&source).unwrap();
        preview.style.set_selected(1);
        assert_eq!(preview.columns.value_as_int(), 160);
        preview.invert.set_active(false);
        let art = ready(&preview);
        preview.apply.emit_clicked();
        wait_official(&this);
        let saved = this.greeting.settings();
        assert_eq!(active_art(&saved), Some(art.clone()));
        assert_eq!(saved.official_items, before.official_items);
        assert_eq!(this.layout_settings(), layout);
        assert_eq!(this.typography_settings(), typography);
        assert!(!this.greeting.artwork.is_editable());
        assert_eq!(
            this.greeting.artwork_view.visible_child_name().as_deref(),
            Some("preview")
        );
        let thumbnail = &this.greeting.artwork_preview;
        assert!(
            thumbnail.viewport.height() >= 180,
            "Thumbnail collapsed inside the editor scroller"
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let normalized = |s: &str| {
            s.lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
                .trim_end()
                .to_owned()
        };
        while normalized(
            thumbnail
                .terminal
                .text_format(vte::Format::Text)
                .as_deref()
                .unwrap_or(""),
        ) != normalized(&art.plain)
        {
            assert!(
                std::time::Instant::now() < deadline,
                "Thumbnail differs from saved ANSI"
            );
            settle();
        }
        assert!(thumbnail.terminal.width() <= thumbnail.viewport.width());
        assert!(thumbnail.terminal.height() <= thumbnail.viewport.height());
        thumbnail.expand();
        settle();
        let enlarged = gtk::Window::list_toplevels()
            .into_iter()
            .find_map(|w| {
                w.downcast::<gtk::Window>()
                    .ok()
                    .filter(|w| w.title().as_deref() == Some("Artwork Preview"))
            })
            .unwrap();
        capture(&enlarged, "TERMIMOCHI_ARTWORK_EXPANDED_SCREENSHOT");
        let fit = descendants(enlarged.upcast_ref())
            .into_iter()
            .find_map(|w| w.downcast::<gtk::ToggleButton>().ok())
            .unwrap();
        fit.set_active(false);
        settle();
        fit.set_active(true);
        settle();
        assert_eq!(this.greeting.settings(), saved);
        enlarged.close();
        settle();
        assert!(feed(&this).contains("38;2;"));
        assert!(art.plain.is_ascii());
        assert!(feed(&this).len() > art.plain.len());
        assert!(this.greeting.image_import_window.upgrade().is_none());
        this.greeting.undo();
        settle();
        assert_eq!(this.greeting.settings(), before);
        this.greeting.redo();
        wait_official(&this);
        assert_eq!(this.greeting.settings(), saved);

        let ansi = root.path().join("image.ans");
        crate::greeting_art::export_file(&ansi, &art.ansi, true, &None).unwrap();
        assert_eq!(crate::greeting_art::file_art(&ansi).unwrap(), art);
        super::super::export_fastfetch(&root.path().join("config.jsonc"), &saved, &None).unwrap();
        let external = root.path().join("external/config.jsonc");
        *this.greeting.fastfetch_target.borrow_mut() =
            Some(crate::fastfetch_apply::Target::open(external.clone()).unwrap());
        this.save_greeting_preset();
        assert!(
            !external.exists(),
            "Save Preset must not apply external configuration"
        );
        settle();
        crate::fastfetch_run::LAUNCHES.with(|runs| runs.borrow_mut().clear());
        respond("Review & Apply…");
        assert!(!external.exists(), "Opening review must not apply");
        respond("Cancel");
        assert!(crate::fastfetch_run::LAUNCHES.with(|runs| runs.borrow().is_empty()));
        assert!(!external.exists(), "Cancel must not apply");
        this.save_greeting_preset();
        respond("Review & Apply…");
        assert_eq!(
            run_after_control().is_active(),
            saved.imported_source.is_none()
        );
        run_after_control().set_active(true);
        respond("Back Up & Apply");
        assert_eq!(
            crate::fastfetch_run::LAUNCHES.with(|runs| runs.borrow().clone()),
            vec![external.clone()]
        );
        this.request_fastfetch_apply();
        let run = run_after_control();
        assert_eq!(run.is_active(), saved.imported_source.is_none());
        run.set_active(false);
        respond("Back Up & Apply");
        assert_eq!(
            crate::fastfetch_run::LAUNCHES.with(|runs| runs.borrow().len()),
            1
        );
        this.request_fastfetch_apply();
        run_after_control().set_active(true);
        respond("Back Up & Apply");
        assert_eq!(
            crate::fastfetch_run::LAUNCHES.with(|runs| runs.borrow().len()),
            2
        );
        let applied =
            crate::fastfetch_document::value(&std::fs::read_to_string(&external).unwrap()).unwrap();
        assert_eq!(applied["logo"]["source"], art.ansi);
        capture(&window, "TERMIMOCHI_IMAGE_PREVIEW_SCREENSHOT");
        let mut closeup = saved.clone();
        closeup.import_artwork(illustration).unwrap();
        this.greeting.replace(closeup, true);
        wait_official(&this);
        capture(&window, "TERMIMOCHI_ARTWORK_CARD_DETAIL_SCREENSHOT");
        this.greeting.undo();
        settle();
        assert_eq!(this.greeting.settings(), saved);
        window.close();
        drop(this);
        settle();
        present_with_preset(&app, None, preset);
        let window = app.active_window().unwrap();
        let this = controller(&window);
        settle();
        assert_eq!(this.greeting.settings(), saved);
        assert!(
            !crate::document_store::encode(&GreetingPreset::new(saved.clone()))
                .unwrap()
                .windows(b"private-source.png".len())
                .any(|w| w == b"private-source.png")
        );

        // Corrupt and empty images leave Apply disabled and the current logo intact.
        for bytes in [b"fake png".to_vec(), {
            let mut data = std::io::Cursor::new(Vec::new());
            image::RgbaImage::new(8, 8)
                .write_to(&mut data, image::ImageFormat::Png)
                .unwrap();
            data.into_inner()
        }] {
            std::fs::write(&source, bytes).unwrap();
            let (dialog, bad) = draft(&this, &source);
            let deadline = std::time::Instant::now() + Duration::from_secs(15);
            while bad.status.text() == "Converting…" {
                assert!(std::time::Instant::now() < deadline);
                settle();
            }
            assert!(!bad.apply.is_sensitive());
            assert!(bad.artwork.borrow().is_none());
            bad.apply();
            assert_eq!(this.greeting.settings(), saved);
            dialog.close();
            settle();
        }
        greeting_image::tests::fixture().save(&source).unwrap();
        let (dialog, stale) = draft(&this, &source);
        ready(&stale);
        let mut newer = saved.clone();
        newer.message = "Keep my latest edit".into();
        this.greeting.replace(newer.clone(), true);
        stale.apply.emit_clicked();
        settle();
        assert_eq!(this.greeting.settings(), newer);
        dialog.close();
        window.close();
        settle();
    }
}
