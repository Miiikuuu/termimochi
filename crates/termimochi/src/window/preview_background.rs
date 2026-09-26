//! Background editor, projected from the current theme like font and layout.
use super::*;
use crate::background::{BackgroundSettings, Placement, axis_range, bounds};
use gtk::subclass::prelude::*;

const MIN_ZOOM: f64 = 0.1;
const MAX_ZOOM: f64 = 3.0;

fn axis_position(view: f64, image: f64, origin: f64) -> f64 {
    let (start, span) = axis_range(view, image);
    ((origin - start) / span).clamp(0.0, 1.0)
}
fn zoom_at(size: [f64; 4], p: Placement, zoom: f64, point: [f64; 2]) -> Placement {
    let [w, h, iw, ih] = size;
    let (x, y, old_w, old_h) = bounds(w, h, iw, ih, p);
    let mut next = Placement {
        zoom: zoom.clamp(MIN_ZOOM, MAX_ZOOM),
        ..p
    };
    let (_, _, new_w, new_h) = bounds(w, h, iw, ih, next);
    next.x = axis_position(w, new_w, point[0] - (point[0] - x) * new_w / old_w);
    next.y = axis_position(h, new_h, point[1] - (point[1] - y) * new_h / old_h);
    next
}

#[derive(Clone, Copy)]
struct DragStart {
    placement: Placement,
    size: [f64; 4],
    generation: u64,
    frame_scale: f64,
}

mod imp {
    use super::*;
    pub struct Surface {
        pub child: RefCell<Option<gtk::Widget>>,
        pub texture: RefCell<Option<gtk::gdk::Texture>>,
        pub placement: Cell<Placement>,
        pub color: Cell<gtk::gdk::RGBA>,
    }
    impl Default for Surface {
        fn default() -> Self {
            Self {
                child: Default::default(),
                texture: Default::default(),
                placement: Default::default(),
                color: Cell::new(gtk::gdk::RGBA::BLACK),
            }
        }
    }
    #[glib::object_subclass]
    impl ObjectSubclass for Surface {
        const NAME: &'static str = "TermiMochiBackgroundSurface";
        type Type = super::Surface;
        type ParentType = gtk::Widget;
        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }
    impl ObjectImpl for Surface {
        fn dispose(&self) {
            if let Some(child) = self.child.borrow_mut().take() {
                child.unparent();
            }
        }
    }
    impl WidgetImpl for Surface {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let rect = gtk::graphene::Rect::new(0.0, 0.0, obj.width() as f32, obj.height() as f32);
            snapshot.push_clip(&rect);
            if let Some(texture) = self.texture.borrow().as_ref() {
                let color = self.color.get();
                snapshot.append_color(&color, &rect);
                let p = self.placement.get();
                let (x, y, w, h) = bounds(
                    obj.width() as f64,
                    obj.height() as f64,
                    texture.width() as f64,
                    texture.height() as f64,
                    p,
                );
                snapshot.append_texture(
                    texture,
                    &gtk::graphene::Rect::new(x as f32, y as f32, w as f32, h as f32),
                );
                let mut tint = color;
                tint.set_alpha(p.tint as f32);
                snapshot.append_color(&tint, &rect);
            }
            if let Some(child) = self.child.borrow().as_ref() {
                obj.snapshot_child(child, snapshot);
            }
            snapshot.pop();
        }
    }
}
glib::wrapper! {
    pub struct Surface(ObjectSubclass<imp::Surface>)
        @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
impl Surface {
    fn new() -> Self {
        glib::Object::new()
    }
    pub fn set_child(&self, child: &impl IsA<gtk::Widget>) {
        child.set_parent(self);
        *self.imp().child.borrow_mut() = Some(child.clone().upcast());
    }
    fn texture(&self, texture: Option<gtk::gdk::Texture>) {
        *self.imp().texture.borrow_mut() = texture;
        self.queue_draw();
    }
    pub fn has_image(&self) -> bool {
        self.imp().texture.borrow().is_some()
    }
    fn image_size(&self) -> Option<[f64; 4]> {
        let texture = self.imp().texture.borrow();
        let texture = texture.as_ref()?;
        (self.width() > 0 && self.height() > 0).then_some([
            self.width() as f64,
            self.height() as f64,
            texture.width() as f64,
            texture.height() as f64,
        ])
    }
}

pub(super) struct Background {
    image: RefCell<Option<crate::greeting_image::source::SourceImage>>,
    pub root: gtk::Box,
    pub surface: Surface,
    choose: gtk::Button,
    clear: gtk::Button,
    center: gtk::Button,
    reset: gtk::Button,
    move_image: gtk::ToggleButton,
    move_hint: gtk::Label,
    mode: gtk::DropDown,
    zoom: gtk::Scale,
    x: gtk::Scale,
    y: gtk::Scale,
    tint: gtk::Scale,
    status: gtk::Label,
    generation: Cell<u64>,
    busy: Cell<bool>,
    drag: Cell<Option<DragStart>>,
    pointer: Cell<Option<[f64; 2]>>,
    positioning: Cell<bool>,
    interaction_active: Cell<bool>,
}
impl Background {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let choose = gtk::Button::with_label("Choose Background Image…");
        let clear = gtk::Button::with_label("Remove");
        let center = gtk::Button::with_label("Center");
        let reset = gtk::Button::with_label("Reset");
        reset.set_tooltip_text(Some("Center the image and restore 100% image zoom"));
        root.append(&choose);
        let mode = gtk::DropDown::from_strings(&["Cover", "Contain"]);
        mode.update_property(&[gtk::accessible::Property::Label("Preview background fit")]);
        root.append(&mode);
        let move_image = gtk::ToggleButton::with_label("Move Image");
        move_image.set_tooltip_text(Some("Drag the background in the right preview; Ctrl+wheel zooms at the pointer. Turn off to select terminal text."));
        move_image.set_sensitive(false);
        root.append(&move_image);
        let move_hint = gtk::Label::builder()
            .label("Drag to move · Ctrl+wheel to zoom")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .visible(false)
            .build();
        root.append(&move_hint);
        fn scale(
            root: &gtk::Box,
            title: &str,
            min: f64,
            max: f64,
            step: f64,
            value: f64,
        ) -> gtk::Scale {
            let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, min, max, step);
            scale.set_value(value);
            scale.set_draw_value(true);
            scale.set_digits(0);
            scale.set_hexpand(true);
            scale.set_width_request(150);
            scale.update_property(&[gtk::accessible::Property::Label(title)]);
            root.append(&layout_row(title, &scale));
            scale
        }
        let zoom = scale(
            &root,
            "Image zoom %",
            MIN_ZOOM * 100.0,
            MAX_ZOOM * 100.0,
            5.0,
            100.0,
        );
        let x = scale(&root, "Horizontal position", 0.0, 100.0, 1.0, 50.0);
        let y = scale(&root, "Vertical position", 0.0, 100.0, 1.0, 50.0);
        let tint = scale(&root, "Overlay %", 0.0, 100.0, 5.0, 35.0);
        tint.set_tooltip_text(Some(
            "Mix the image with the current theme background color",
        ));
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.append(&center);
        buttons.append(&reset);
        buttons.append(&clear);
        root.append(&buttons);
        let status = gtk::Label::builder()
            .label("PNG/JPG/WebP · backgrounds are saved with themes, not component files.")
            .wrap(true)
            .xalign(0.0)
            .build();
        root.append(&status);
        let surface = Surface::new();
        surface.set_hexpand(true);
        surface.set_vexpand(true);
        Self {
            image: RefCell::new(None),
            root,
            surface,
            choose,
            clear,
            center,
            reset,
            move_image,
            move_hint,
            mode,
            zoom,
            x,
            y,
            tint,
            status,
            generation: Cell::new(0),
            busy: Cell::new(false),
            drag: Cell::new(None),
            pointer: Cell::new(None),
            positioning: Cell::new(false),
            interaction_active: Cell::new(false),
        }
    }
    pub fn clear(&self) {
        self.image.borrow_mut().take();
        self.generation.set(self.generation.get().wrapping_add(1));
        self.drag.set(None);
        self.pointer.set(None);
        self.surface.texture(None);
        self.move_image.set_active(false);
        self.move_image.set_sensitive(false);
        self.status
            .set_text("No background image. Theme Save records removal; component exports exclude backgrounds.");
    }
    pub fn settings(&self) -> Option<BackgroundSettings> {
        self.image.borrow().clone().map(|image| BackgroundSettings {
            image,
            placement: self.placement(),
        })
    }
    pub fn pending(&self) -> bool {
        self.busy.get()
    }
    pub fn restore(&self, settings: Option<&BackgroundSettings>) {
        self.clear();
        let p = settings.map_or_else(Placement::default, |s| s.placement);
        self.positioning.set(true);
        self.mode.set_selected(u32::from(p.contain));
        self.tint.set_value(p.tint * 100.0);
        self.set_placement(p);
        if let Some(settings) = settings {
            // Data was validated before load; keep the owned image even if the
            // local renderer fails. Never silently turn a render error into removal.
            *self.image.borrow_mut() = Some(settings.image.clone());
            match gtk::gdk::Texture::from_bytes(&glib::Bytes::from(settings.image.bytes())) {
                Ok(texture) => {
                    self.surface.texture(Some(texture));
                    self.status.set_text(
                        "Background restored from theme. Contrast checks ignore the image.",
                    );
                }
                Err(error) => self
                    .status
                    .set_text(&format!("Background retained, preview failed: {error}")),
            }
        }
    }
    fn placement(&self) -> Placement {
        Placement {
            contain: self.mode.selected() == 1,
            zoom: self.zoom.value() / 100.0,
            x: self.x.value() / 100.0,
            y: self.y.value() / 100.0,
            tint: self.tint.value() / 100.0,
        }
    }
    fn set_placement(&self, p: Placement) {
        self.positioning.set(true);
        self.zoom.set_value(p.zoom * 100.0);
        self.x.set_value(p.x * 100.0);
        self.y.set_value(p.y * 100.0);
        self.positioning.set(false);
    }
}

impl Workbench {
    pub(super) fn sync_background_preview(&self) {
        let b = &self.preview_background;
        let model = self.model.borrow();
        let color = model
            .palette
            .variant(model.active_variant)
            .and_then(|v| v.get("Background"))
            .unwrap_or(Rgb::new(0, 0, 0));
        let mut color = rgba_from_rgb(color);
        b.surface.imp().color.set(color);
        b.surface.imp().placement.set(b.placement());
        b.surface.queue_draw();
        if b.surface.has_image() {
            color.set_alpha(0.0);
            self.preview_terminal
                .add_css_class("with-preview-background");
        } else {
            self.preview_terminal
                .remove_css_class("with-preview-background");
        }
        self.preview_terminal
            .set_clear_background(!b.surface.has_image());
        self.preview_terminal.set_color_background(&color);
        drop(model);
        if !self.updating.get() {
            self.refresh_history_actions();
        }
    }
    fn background_moving(&self) -> bool {
        self.preview_background.move_image.is_active()
            && self.background_module_button.is_active()
            && self.preview_background.surface.has_image()
            && !self.inspect_button.is_active()
    }
    fn background_content_point(&self, x: f64, y: f64) -> bool {
        let viewport = &self.preview_terminal_viewport;
        self.preview_background
            .surface
            .compute_point(viewport, &gtk::graphene::Point::new(x as f32, y as f32))
            .is_some_and(|p| {
                p.x() >= 0.0
                    && p.y() >= 0.0
                    && p.x() < viewport.width() as f32
                    && p.y() < viewport.height() as f32
            })
    }
    fn sync_background_interaction(&self) {
        let b = &self.preview_background;
        let active = self.background_moving();
        if !active {
            b.drag.set(None);
        }
        b.move_image.set_sensitive(b.surface.has_image());
        b.move_hint.set_visible(active);
        let was_active = b.interaction_active.replace(active);
        if !active && !was_active {
            // A denied gesture belongs to VTE/Inspect. Do not overwrite its
            // cursor just because this background controller observed it.
            return;
        }
        let cursor = active.then_some(if b.drag.get().is_some() {
            "grabbing"
        } else {
            "grab"
        });
        b.surface.set_cursor_from_name(cursor);
        self.preview_terminal.set_cursor_from_name(cursor);
        self.full_session.samples.entry.set_cursor_from_name(cursor);
    }
    pub(super) fn connect_background_preview(this: &Rc<Self>) {
        let b = &this.preview_background;
        for scale in [&b.x, &b.y, &b.tint] {
            let weak = Rc::downgrade(this);
            scale.connect_value_changed(move |_| {
                if let Some(this) = weak.upgrade()
                    && !this.preview_background.positioning.get()
                {
                    this.sync_background_preview();
                }
            });
        }
        let weak = Rc::downgrade(this);
        b.zoom.connect_value_changed(move |zoom| {
            let Some(this) = weak.upgrade() else { return };
            let b = &this.preview_background;
            if b.positioning.get() {
                return;
            }
            b.drag.set(None);
            if let Some(size) = b.surface.image_size() {
                let old = b.surface.imp().placement.get();
                let (x, y, w, h) = bounds(size[0], size[1], size[2], size[3], old);
                b.set_placement(zoom_at(
                    size,
                    old,
                    zoom.value() / 100.0,
                    [x + w / 2.0, y + h / 2.0],
                ));
            }
            this.sync_background_preview();
            this.sync_background_interaction();
        });
        let weak = Rc::downgrade(this);
        b.mode.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade()
                && !this.preview_background.positioning.get()
            {
                this.preview_background.drag.set(None);
                this.sync_background_preview();
                this.sync_background_interaction();
            }
        });
        let weak = Rc::downgrade(this);
        b.clear.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.preview_background.clear();
                this.sync_background_preview();
                this.sync_background_interaction();
            }
        });
        let weak = Rc::downgrade(this);
        b.center.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                let b = &this.preview_background;
                b.set_placement(Placement {
                    x: 0.5,
                    y: 0.5,
                    ..b.placement()
                });
                this.sync_background_preview();
            }
        });
        let weak = Rc::downgrade(this);
        b.reset.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                let b = &this.preview_background;
                b.set_placement(Placement {
                    zoom: 1.0,
                    x: 0.5,
                    y: 0.5,
                    ..b.placement()
                });
                this.sync_background_preview();
            }
        });
        Self::connect_background_gestures(this);
        let weak = Rc::downgrade(this);
        b.choose.connect_clicked(move |_| {
            let Some(this) = weak.upgrade() else {
                return;
            };
            let b = &this.preview_background;
            if b.busy.replace(true) {
                return;
            }
            b.choose.set_sensitive(false);
            let generation = b.generation.get();
            let identity = this.typed.identity.get();
            let dialog = gtk::FileDialog::builder()
                .title("Choose background image")
                .build();
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Static images"));
            for mime in ["image/png", "image/jpeg", "image/webp"] {
                filter.add_mime_type(mime);
            }
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            dialog.set_filters(Some(&filters));
            let weak = Rc::downgrade(&this);
            dialog.open(
                Some(&this.window()),
                gio::Cancellable::NONE,
                move |result| {
                    let Some(this) = weak.upgrade() else {
                        return;
                    };
                    let b = &this.preview_background;
                    b.busy.set(false);
                    b.choose.set_sensitive(true);
                    this.refresh_history_actions();
                    if identity != this.typed.identity.get() || generation != b.generation.get() {
                        return;
                    }
                    if let Ok(file) = result
                        && let Some(path) = file.path()
                    {
                        this.load_preview_background(path);
                    }
                },
            );
        });
    }
    fn connect_background_gestures(this: &Rc<Self>) {
        let b = &this.preview_background;
        let weak = Rc::downgrade(this);
        b.move_image.connect_toggled(move |button| {
            if let Some(this) = weak.upgrade() {
                if button.is_active() && this.background_module_button.is_active() {
                    this.inspect_button.set_active(false);
                }
                this.sync_background_interaction();
            }
        });
        let weak = Rc::downgrade(this);
        this.background_module_button
            .connect_toggled(move |button| {
                if let Some(this) = weak.upgrade() {
                    if button.is_active() && this.preview_background.move_image.is_active() {
                        this.inspect_button.set_active(false);
                    }
                    this.sync_background_interaction();
                }
            });
        let weak = Rc::downgrade(this);
        this.inspect_button.connect_toggled(move |button| {
            if let Some(this) = weak.upgrade()
                && button.is_active()
            {
                this.preview_background.move_image.set_active(false);
            }
        });
        let weak = Rc::downgrade(this);
        b.surface.connect_unmap(move |_| {
            if let Some(this) = weak.upgrade() {
                this.preview_background.drag.set(None);
                this.preview_background.pointer.set(None);
                this.sync_background_interaction();
            }
        });
        let motion = gtk::EventControllerMotion::new();
        let weak = Rc::downgrade(this);
        motion.connect_motion(move |_, x, y| {
            if let Some(this) = weak.upgrade() {
                this.preview_background.pointer.set(Some([x, y]));
            }
        });
        let weak = Rc::downgrade(this);
        motion.connect_leave(move |_| {
            if let Some(this) = weak.upgrade() {
                this.preview_background.pointer.set(None);
            }
        });
        b.surface.add_controller(motion);

        // GTK maps events through the Full frame's allocation transform, so
        // these deltas are already local image coordinates at Fit and 100%.
        let drag = gtk::GestureDrag::new();
        drag.set_button(1);
        drag.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(this);
        drag.connect_drag_begin(move |gesture, x, y| {
            let Some(this) = weak.upgrade() else { return };
            let b = &this.preview_background;
            if this.background_moving()
                && this.background_content_point(x, y)
                && let Some(size) = b.surface.image_size()
            {
                b.drag.set(Some(DragStart {
                    placement: b.placement(),
                    size,
                    generation: b.generation.get(),
                    frame_scale: this.full_session.frame.scale(),
                }));
                gesture.set_state(gtk::EventSequenceState::Claimed);
                this.sync_background_interaction();
            } else {
                gesture.set_state(gtk::EventSequenceState::Denied);
            }
        });
        let weak = Rc::downgrade(this);
        drag.connect_drag_update(move |_, dx, dy| {
            let Some(this) = weak.upgrade() else { return };
            let b = &this.preview_background;
            let Some(start) = b.drag.get() else { return };
            if !this.background_moving()
                || b.generation.get() != start.generation
                || b.surface.image_size() != Some(start.size)
                || this.full_session.frame.scale() != start.frame_scale
            {
                b.drag.set(None);
                this.sync_background_interaction();
                return;
            }
            let [w, h, iw, ih] = start.size;
            let (x, y, width, height) = bounds(w, h, iw, ih, start.placement);
            b.set_placement(Placement {
                x: axis_position(w, width, x + dx),
                y: axis_position(h, height, y + dy),
                ..start.placement
            });
            this.sync_background_preview();
        });
        let weak = Rc::downgrade(this);
        drag.connect_drag_end(move |_, _, _| {
            if let Some(this) = weak.upgrade() {
                this.preview_background.drag.set(None);
                this.sync_background_interaction();
            }
        });
        let weak = Rc::downgrade(this);
        drag.connect_cancel(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.preview_background.drag.set(None);
                this.sync_background_interaction();
            }
        });
        b.surface.add_controller(drag);

        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(this);
        wheel.connect_scroll(move |event, _, dy| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !this.background_moving()
                || !event
                    .current_event_state()
                    .contains(gdk::ModifierType::CONTROL_MASK)
            {
                return glib::Propagation::Proceed;
            }
            let b = &this.preview_background;
            if b.pointer
                .get()
                .is_some_and(|p| !this.background_content_point(p[0], p[1]))
            {
                return glib::Propagation::Proceed;
            }
            if let Some(size) = b.surface.image_size()
                && dy.is_finite()
            {
                let steps = if event.unit() == gdk::ScrollUnit::Surface {
                    dy / 100.0
                } else {
                    dy
                };
                let p = b.placement();
                let anchor = b.pointer.get().unwrap_or([size[0] / 2.0, size[1] / 2.0]);
                b.drag.set(None);
                b.set_placement(zoom_at(
                    size,
                    p,
                    p.zoom * 1.1_f64.powf(-steps.clamp(-8.0, 8.0)),
                    anchor,
                ));
                this.sync_background_preview();
                this.sync_background_interaction();
            }
            glib::Propagation::Stop
        });
        b.surface.add_controller(wheel);
    }

    fn load_preview_background(self: &Rc<Self>, path: PathBuf) {
        let b = &self.preview_background;
        if b.busy.replace(true) {
            return;
        }
        b.choose.set_sensitive(false);
        b.status.set_text("Preparing preview background…");
        let generation = b.generation.get();
        let identity = self.typed.identity.get();
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = crate::typography_preset::read_private_with_limit(
                &path,
                crate::greeting_image::FILE_LIMIT,
            )
            .and_then(|bytes| bytes.ok_or_else(|| "Image no longer exists".to_string()))
            .and_then(|bytes| crate::greeting_image::background_png(&bytes))
            .and_then(|png| BackgroundSettings::from_png(png, Placement::default()));
            let _ = send.send(result);
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(40), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let result = match receive.try_recv() {
                Ok(value) => value,
                Err(std::sync::mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("Background preparation stopped".into()),
            };
            let b = &this.preview_background;
            b.busy.set(false);
            b.choose.set_sensitive(true);
            if generation == b.generation.get() && identity == this.typed.identity.get() {
                match result.and_then(|settings| {
                    gtk::gdk::Texture::from_bytes(&glib::Bytes::from(settings.image.bytes()))
                        .map(|texture| (texture, settings.image))
                        .map_err(|e| e.to_string())
                }) {
                    Ok((texture, image)) => {
                        *b.image.borrow_mut() = Some(image);
                        b.drag.set(None);
                        b.surface.texture(Some(texture));
                        b.move_image.set_active(true);
                        b.status.set_text(if this.is_theme() {
                            "Included in theme Save. Use Theme reviews target support. Contrast checks ignore the image."
                        } else { "Preview reference only in this component document. Create/open a theme to save backgrounds. Contrast checks ignore the image." });
                        this.sync_background_preview();
                        this.sync_background_interaction();
                    }
                    Err(error) => b.status.set_text(&error),
                }
            } else if b.status.text() == "Preparing preview background…" {
                b.status
                    .set_text("Preview background cancelled after document change.");
            }
            this.refresh_history_actions();
            glib::ControlFlow::Break
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::greeting::tests::{controller, settle};
    use super::*;
    use crate::background::axis_origin;

    fn without_background(
        mut design: crate::design_document::DesignDocument,
    ) -> crate::design_document::DesignDocument {
        design.theme.as_mut().unwrap().background = None;
        design
    }

    fn pointer(
        this: &Workbench,
        widget: &impl IsA<gtk::Widget>,
        mode: &str,
        from: [f64; 2],
        to: Option<[f64; 2]>,
    ) {
        let window = this.window();
        window.set_title(Some("TermiMochi point-to-edit test"));
        let (dx, dy) = window.surface_transform();
        let scale = window.scale_factor() as f64;
        let point = |p: [f64; 2]| {
            let p = widget
                .compute_point(
                    &window,
                    &gtk::graphene::Point::new(p[0] as f32, p[1] as f32),
                )
                .unwrap();
            [
                ((p.x() as f64 + dx) * scale).round().to_string(),
                ((p.y() as f64 + dy) * scale).round().to_string(),
            ]
        };
        let mut command = std::process::Command::new("python3");
        command.arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/preview-pointer-driver.py"
        ));
        command.arg(mode).args(point(from));
        if let Some(to) = to {
            command.args(point(to));
        } else {
            command.arg("3");
        }
        let mut child = command.spawn().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while child.try_wait().unwrap().is_none() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        assert!(child.wait().unwrap().success());
        settle();
    }

    fn image_rect(b: &Background) -> (f64, f64, f64, f64) {
        let [w, h, iw, ih] = b.surface.image_size().unwrap();
        bounds(w, h, iw, ih, b.placement())
    }
    #[test]
    fn cover_contain_position_and_zoom_use_viewport_coordinates() {
        let p = Placement::default();
        assert_eq!(
            bounds(100.0, 100.0, 200.0, 100.0, p),
            (-50.0, 0.0, 200.0, 100.0)
        );
        assert_eq!(
            bounds(100.0, 100.0, 200.0, 100.0, Placement { contain: true, ..p }),
            (0.0, 25.0, 100.0, 50.0)
        );
        assert_eq!(
            bounds(
                100.0,
                100.0,
                200.0,
                100.0,
                Placement {
                    zoom: 2.0,
                    x: 1.0,
                    y: 0.0,
                    ..p
                }
            ),
            (75.0, -175.0, 400.0, 200.0)
        );
    }

    #[test]
    fn small_and_exact_fit_images_can_move_and_remain_reachable() {
        let p = Placement::default();
        assert_eq!(
            bounds(100.0, 100.0, 100.0, 100.0, Placement { zoom: 0.1, ..p }),
            (45.0, 45.0, 10.0, 10.0)
        );
        for image in [10.0, 100.0, 300.0, 10000.0] {
            let origin = axis_origin(100.0, image, 0.5);
            let moved = axis_position(100.0, image, origin + 10.0);
            assert!((axis_origin(100.0, image, moved) - origin - 10.0).abs() < 1e-8);
            assert!(axis_origin(100.0, image, 0.0) + image > 0.0);
            assert!(axis_origin(100.0, image, 1.0) < 100.0);
            assert_eq!(axis_position(100.0, image, -100000.0), 0.0);
            assert_eq!(axis_position(100.0, image, 100000.0), 1.0);
        }
    }

    #[test]
    fn zoom_keeps_the_point_under_the_pointer_and_obeys_limits() {
        let size = [800.0, 600.0, 400.0, 300.0];
        let p = Placement::default();
        let point = [300.0, 200.0];
        let next = zoom_at(size, p, 0.5, point);
        let (x, y, w, h) = bounds(size[0], size[1], size[2], size[3], next);
        assert_eq!((x, y, w, h), (150.0, 100.0, 400.0, 300.0));
        assert_eq!((point[0] - x) / w, point[0] / size[0]);
        assert_eq!((point[1] - y) / h, point[1] / size[1]);
        assert_eq!(zoom_at(size, p, 0.0, point).zoom, MIN_ZOOM);
        assert_eq!(zoom_at(size, p, 999.0, point).zoom, MAX_ZOOM);
    }

    #[test]
    #[ignore = "isolated GTK: background composition, input/history, Fit and document isolation"]
    fn background_preview_native_composition_and_session_isolation() {
        assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
        assert!(!matches!(
            std::env::var("DISPLAY").unwrap().as_str(),
            ":0" | ":1"
        ));
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.BackgroundPreviewQA")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_with_preset(&app, None, root.path().join("font.json"));
        let window = app.active_window().unwrap();
        let this = controller(&window);
        this.load_design(crate::design_document::DesignDocument::new_theme(
            crate::design_document::TargetHint::Ptyxis,
            "Background preview QA",
            false,
        ));
        let ready = || {
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            loop {
                settle();
                if !this.preview_background.busy.get()
                    && !this.preview_loading.get()
                    && !this.copy_loading.get()
                    && !this.geometry_pending.get()
                {
                    break;
                }
                assert!(std::time::Instant::now() < deadline);
            }
        };
        ready();
        assert!(this.full_session.active.get());
        this.background_module_button.set_active(true);
        assert_eq!(this.output_module(), EditorModule::Background);
        assert_eq!(this.preview_scene_selector.selected(), 0);
        let baseline = this.design_snapshot().unwrap();
        for _ in 0..4 {
            this.full_session.samples.entry.set_text("help");
            this.full_session.samples.entry.emit_activate();
        }
        this.full_session.samples.entry.set_text("git diff");
        ready();
        this.preview_scroll.adjustment.set_value(0.0);
        settle();
        let feed = this.full_session.rendered.borrow().clone();
        let path = root.path().join("background.png");
        let mut pixels = image::RgbaImage::from_pixel(400, 300, image::Rgba([180, 30, 80, 255]));
        for (x, _, pixel) in pixels.enumerate_pixels_mut() {
            if x >= 200 {
                *pixel = image::Rgba([25, 90, 170, 255]);
            }
        }
        pixels.save(&path).unwrap();
        super::super::preview_geometry_tests::capture(&this, "background-preview-before");
        this.load_preview_background(path.clone());
        assert!(
            this.committed_design().is_err(),
            "Save/use must wait for image preparation"
        );
        ready();
        assert!(this.preview_background.surface.has_image());
        let plan = this.scheme_plan(&this.workspace_snapshot());
        assert!(
            plan.items
                .iter()
                .any(|item| item.id == "background" && item.action.is_none())
        );
        // The real GUI action consumes the current image, without asking the
        // user to pick it again. Opening the review does not launch a shell.
        gio::prelude::ActionGroupExt::activate_action(&this.window(), "background-trial", None);
        settle();
        let review = app
            .windows()
            .into_iter()
            .find(|w| w.title().as_deref() == Some("Enhanced Ptyxis · Background"))
            .unwrap();
        let widgets = super::super::greeting::tests::descendants(review.upcast_ref());
        assert!(
            widgets
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Button>())
                .any(|b| b.label().as_deref() == Some("Open Real Terminal") && b.is_visible())
        );
        assert!(
            !widgets.iter().any(|w| w.is::<gtk::CheckButton>()),
            "Opening a real background session should not require duplicate consent checkboxes"
        );
        assert!(
            widgets
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Expander>())
                .any(|e| !e.is_expanded())
        );
        assert!(
            !widgets
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Button>())
                .any(|b| b.label().as_deref() == Some("Stop Trial…") && b.is_visible())
        );
        assert!(!widgets.iter().filter_map(|w| w.downcast_ref::<gtk::Button>()).any(|b| b.label().as_deref() == Some("Choose Background Image…") && b.is_visible()));
        review.close();
        ready();
        // The Apply review is modal. Navigating to the background trial must
        // release its input grab, not leave an unclickable sibling window.
        this.request_scheme_apply();
        settle();
        let apply = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.title().as_deref() == Some("Apply This Scheme"))
            .unwrap();
        assert!(apply.is_modal() && apply.is_visible());
        let open = super::super::greeting::tests::descendants(apply.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Button>().ok())
            .find(|b| b.label().as_deref() == Some("Open in Enhanced Ptyxis…"))
            .unwrap();
        open.emit_clicked();
        settle();
        assert!(
            !apply.is_visible(),
            "Apply review must release modal input before the trial opens"
        );
        let review = app
            .windows()
            .into_iter()
            .find(|w| w.title().as_deref() == Some("Enhanced Ptyxis · Background"))
            .unwrap();
        assert_eq!(
            review.transient_for().as_ref(),
            Some(this.window().upcast_ref())
        );
        review.close();
        ready();
        this.preview_background.tint.set_value(0.0);
        let editor_scroll = this
            .preview_background
            .root
            .ancestor(gtk::ScrolledWindow::static_type())
            .and_downcast::<gtk::ScrolledWindow>()
            .unwrap();
        let adjustment = editor_scroll.vadjustment();
        adjustment.set_value(adjustment.upper() - adjustment.page_size());
        ready();
        assert_eq!(this.full_session.samples.entry.text(), "git diff");
        assert_eq!(*this.full_session.rendered.borrow(), feed);
        assert_eq!(this.preview_scroll.adjustment.value(), 0.0);
        assert_eq!(
            without_background(this.design_snapshot().unwrap()),
            baseline
        );
        assert!(!this.design_clean());
        assert!(
            this.design_snapshot()
                .unwrap()
                .theme
                .unwrap()
                .background
                .is_some()
        );
        // Render the real content surface with VTE on top, not just the image
        // widget. Sample a background-only bottom row to catch opaque VTE.
        let surface = &this.preview_background.surface;
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(surface)).snapshot(
            &snapshot,
            surface.width() as f64,
            surface.height() as f64,
        );
        let texture = window
            .renderer()
            .unwrap()
            .render_texture(snapshot.to_node().unwrap(), None);
        let mut bytes = vec![0; texture.width() as usize * texture.height() as usize * 4];
        texture.download(&mut bytes, texture.width() as usize * 4);
        texture
            .save_to_png(glib::user_cache_dir().join("background-surface.png"))
            .unwrap();
        super::super::preview_geometry_tests::capture(&this, "background-preview-actual");
        let mut visible_image_pixels = 0;
        for pixel in bytes.chunks_exact(4) {
            let rgb = if cfg!(target_endian = "little") {
                [pixel[2], pixel[1], pixel[0]]
            } else {
                [pixel[1], pixel[2], pixel[3]]
            };
            if rgb == [180, 30, 80] || rgb == [25, 90, 170] {
                visible_image_pixels += 1;
            }
        }
        assert!(
            visible_image_pixels > (surface.width() * surface.height()) as usize / 4,
            "Background must actually be visible through VTE"
        );
        super::super::preview_geometry_tests::capture(&this, "background-preview-actual");
        // Real XTest drags through GTK's transformed surface. The same local
        // motion must move the background equally at Actual, Fit and 100%.
        let b = &this.preview_background;
        assert!(this.background_moving());
        let mut records = Vec::new();
        for mode in [4, 0, 1] {
            this.full_session.zoom.set_selected(mode);
            b.zoom.set_value(50.0);
            b.center.emit_clicked();
            ready();
            this.preview_scroll.adjustment.set_value(0.0);
            ready();
            let before = image_rect(b);
            let size = b.surface.image_size().unwrap();
            let from = [size[0] / 2.0, size[1] / 2.0];
            let to = [from[0] + 70.0, from[1] + 40.0];
            let scroll_before = this.preview_scroll.adjustment.value();
            let text_before = this.full_session.rendered.borrow().clone();
            pointer(&this, surface, "drag_to", from, Some(to));
            ready();
            let after = image_rect(b);
            let scale = this.full_session.frame.scale();
            let tolerance = 3.0 / scale;
            assert!(
                (after.0 - before.0 - 70.0).abs() < tolerance,
                "mode {mode}: {before:?} -> {after:?}, scale {scale}"
            );
            assert!((after.1 - before.1 - 40.0).abs() < tolerance);
            assert_eq!(after.2, before.2);
            assert_eq!(after.3, before.3);
            assert_eq!(this.preview_scroll.adjustment.value(), scroll_before);
            assert_eq!(*this.full_session.rendered.borrow(), text_before);
            assert_eq!(this.full_session.samples.entry.text(), "git diff");
            assert_eq!(
                without_background(this.design_snapshot().unwrap()),
                baseline
            );
            assert!(
                !this.preview_terminal.has_selection(),
                "Moving an image must not select VTE text"
            );
            records.push(serde_json::json!({"mode":mode,"frame_scale":scale,"surface":size,"before":before,"after":after,"local_drag":[70,40],"scroll":scroll_before}));
            super::super::preview_geometry_tests::capture(
                &this,
                &format!("background-moved-{mode}"),
            );
            pointer(&this, surface, "ctrl_scroll_down", from, None);
            assert!(b.zoom.value() < 50.0);
            assert_eq!(this.preview_scroll.adjustment.value(), scroll_before);
            assert_eq!(*this.full_session.rendered.borrow(), text_before);
            assert_eq!(this.full_session.samples.entry.text(), "git diff");
        }
        std::fs::write(
            glib::user_cache_dir().join("background-drag-geometry.json"),
            serde_json::to_vec_pretty(&records).unwrap(),
        )
        .unwrap();
        this.full_session.zoom.set_selected(4);
        b.zoom.set_value(10.0);
        b.center.emit_clicked();
        ready();
        super::super::preview_geometry_tests::capture(&this, "background-shrunk-10");
        assert_eq!(b.zoom.value(), 10.0);
        b.reset.emit_clicked();
        assert_eq!(b.zoom.value(), 100.0);
        assert_eq!((b.x.value(), b.y.value()), (50.0, 50.0));
        // Wheel without Control still scrolls the sample. Turning Move off
        // returns pointer input to VTE without changing the image placement.
        ready();
        let from = [surface.width() as f64 / 2.0, surface.height() as f64 / 2.0];
        this.preview_scroll.adjustment.set_value(0.0);
        pointer(&this, surface, "scroll_down", from, None);
        assert!(this.preview_scroll.adjustment.value() > 0.0);
        assert_eq!(b.zoom.value(), 100.0);
        b.move_image.set_active(false);
        let before = image_rect(b);
        pointer(&this, surface, "drag", from, None);
        assert_eq!(image_rect(b), before);
        this.preview_terminal.unselect_all();
        this.full_session.samples.entry.set_text("");
        let blocks_before = this
            .full_session
            .samples
            .state
            .borrow()
            .current()
            .blocks
            .len();
        this.preview_scroll.follow_input();
        ready();
        let entry = &this.full_session.samples.entry;
        pointer(
            &this,
            entry,
            "sample_help",
            [20.0, entry.height() as f64 / 2.0],
            None,
        );
        ready();
        assert!(this.full_session.samples.entry.text().is_empty());
        assert_eq!(
            this.full_session
                .samples
                .state
                .borrow()
                .current()
                .blocks
                .len(),
            blocks_before + 1
        );
        assert!(
            this.full_session
                .rendered
                .borrow()
                .contains("Directories: /, /demo, /demo/src")
        );
        this.full_session.samples.entry.set_text("git diff");
        b.move_image.set_active(true);
        this.inspect_button.set_active(true);
        assert!(!b.move_image.is_active());
        b.move_image.set_active(true);
        assert!(!this.inspect_button.is_active());
        this.palette_module_button.set_active(true);
        assert!(!this.background_moving());
        this.background_module_button.set_active(true);
        assert!(this.background_moving());
        this.preview_background.zoom.set_value(160.0);
        this.preview_background.x.set_value(80.0);
        this.preview_background.y.set_value(20.0);
        this.preview_background.tint.set_value(45.0);
        this.full_session.zoom.set_selected(0);
        this.palette_module_button.set_active(true);
        ready();
        assert!(this.full_session.active.get());
        assert!(surface.has_image());
        assert_eq!(this.full_session.samples.entry.text(), "git diff");
        assert_eq!(
            without_background(this.design_snapshot().unwrap()),
            baseline
        );
        super::super::preview_geometry_tests::capture(&this, "background-preview-fit");
        this.apply_color("Background", Rgb::new(22, 28, 36));
        this.font_size_input.set_value(14.0);
        ready();
        assert!(surface.has_image());
        assert_eq!(
            surface.imp().color.get(),
            rgba_from_rgb(Rgb::new(22, 28, 36))
        );
        assert_eq!(this.full_session.samples.entry.text(), "git diff");
        super::super::preview_geometry_tests::capture(&this, "background-preview-theme-edit");
        // A separate window owns neither the image nor its settings.
        present_with_preset(&app, None, root.path().join("second.json"));
        let other_window = app.active_window().unwrap();
        let other = controller(&other_window);
        assert!(!other.preview_background.surface.has_image());
        other_window.destroy();
        // Production Save action embeds bytes and framing; reopen without the
        // original source, then verify removal/undo restores the image.
        let saved = root.path().join("background.termimochi-design.json");
        *this.typed.store.borrow_mut() = Some(DocumentStore::open(saved.clone()).unwrap());
        this.save_design(false);
        assert!(this.design_clean());
        let committed = this.committed_design().unwrap();
        let saved_bg = committed
            .theme
            .as_ref()
            .unwrap()
            .background
            .clone()
            .unwrap();
        std::fs::remove_file(&path).unwrap();
        let reopened = DocumentStore::<crate::design_document::DesignDocument>::open(saved)
            .unwrap()
            .document()
            .unwrap()
            .unwrap();
        assert_eq!(reopened, committed);
        this.load_design(reopened);
        ready();
        assert_eq!(this.preview_background.settings().as_ref(), Some(&saved_bg));
        assert!(surface.has_image());
        this.preview_background.clear.emit_clicked();
        assert!(this.preview_background.settings().is_none());
        this.theme_undo(false);
        ready();
        assert_eq!(this.preview_background.settings().as_ref(), Some(&saved_bg));
        assert!(surface.has_image());
        super::super::preview_geometry_tests::capture(&this, "background-saved-reopened");
        // A source path may disappear after Save without losing the design.
        pixels.save(&path).unwrap();
        // Changing documents before the worker is delivered invalidates it.
        this.load_preview_background(path);
        this.load_design(crate::design_document::DesignDocument::new_theme(
            crate::design_document::TargetHint::Ptyxis,
            "Other theme",
            false,
        ));
        ready();
        assert!(!surface.has_image());
        assert!(!this.preview_background.busy.get());
        super::super::preview_geometry_tests::capture(&this, "background-preview-cleared");
        window.destroy();
    }
}
