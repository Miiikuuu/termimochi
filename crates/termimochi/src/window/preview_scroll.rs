//! One local scroll range for the fixed terminal canvas and VTE history.
//! The header, diagnostics and editor never participate in this range.
use super::*;

pub(super) struct PreviewScroll {
    pub adjustment: gtk::Adjustment,
    terminal: glib::WeakRef<vte::Terminal>,
    viewport: glib::WeakRef<gtk::ScrolledWindow>,
    updating: Cell<bool>,
    generation: Cell<u64>,
    scale: Cell<f64>,
}

fn pixel_delta(delta: f64, cell: f64, unit: gdk::ScrollUnit) -> f64 {
    if !delta.is_finite() || !cell.is_finite() {
        return 0.0;
    }
    let pixels = if unit == gdk::ScrollUnit::Surface {
        delta
    } else {
        delta * cell.max(1.0) * 3.0
    };
    if pixels.is_finite() { pixels } else { 0.0 }
}

impl PreviewScroll {
    pub fn new(terminal: &vte::Terminal, viewport: &gtk::ScrolledWindow) -> Rc<Self> {
        let this = Rc::new(Self {
            adjustment: gtk::Adjustment::default(),
            terminal: terminal.downgrade(),
            viewport: viewport.downgrade(),
            updating: Cell::new(false),
            generation: Cell::new(0),
            scale: Cell::new(1.0),
        });
        for source in [terminal.vadjustment().unwrap(), viewport.vadjustment()] {
            let weak = Rc::downgrade(&this);
            source.connect_changed(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.sync();
                }
            });
            let weak = Rc::downgrade(&this);
            source.connect_value_changed(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.sync();
                }
            });
        }
        let weak = Rc::downgrade(&this);
        this.adjustment.connect_value_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.apply();
            }
        });
        this.sync();
        this
    }
    pub fn set_scale(&self, scale: f64) {
        self.scale.set(scale);
        self.sync();
    }
    fn row_pixels(&self, terminal: &vte::Terminal) -> f64 {
        if terminal.is_scroll_unit_is_pixels() {
            self.scale.get()
        } else {
            terminal.char_height().max(1) as f64 * self.scale.get()
        }
    }
    fn sync(&self) {
        if self.updating.replace(true) {
            return;
        }
        if let (Some(terminal), Some(viewport)) = (self.terminal.upgrade(), self.viewport.upgrade())
        {
            let history = terminal.vadjustment().unwrap();
            let canvas = viewport.vadjustment();
            let factor = self.row_pixels(&terminal);
            let history_pixels =
                (history.upper() - history.page_size() - history.lower()).max(0.0) * factor;
            let canvas_pixels = (canvas.upper() - canvas.lower()).max(canvas.page_size());
            self.adjustment.configure(
                (history.value() - history.lower()) * factor + canvas.value() - canvas.lower(),
                0.0,
                history_pixels + canvas_pixels,
                terminal.char_height().max(1) as f64 * self.scale.get() * 3.0,
                canvas.page_size() * 0.9,
                canvas.page_size(),
            );
        }
        self.updating.set(false);
    }
    fn apply(&self) {
        if self.updating.replace(true) {
            return;
        }
        self.generation.set(self.generation.get().wrapping_add(1));
        if let (Some(terminal), Some(viewport)) = (self.terminal.upgrade(), self.viewport.upgrade())
        {
            let history = terminal.vadjustment().unwrap();
            let canvas = viewport.vadjustment();
            let factor = self.row_pixels(&terminal);
            let history_pixels =
                (history.upper() - history.page_size() - history.lower()).max(0.0) * factor;
            let value = self.adjustment.value();
            history.set_value(history.lower() + value.min(history_pixels) / factor);
            canvas.set_value(canvas.lower() + (value - history_pixels).max(0.0));
        }
        self.updating.set(false);
    }
    pub fn attach(self: &Rc<Self>, stage: &gtk::Box) {
        let controller =
            gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        controller.connect_scroll(move |event, dx, dy| {
            if let Some(this) = weak.upgrade() {
                this.generation.set(this.generation.get().wrapping_add(1));
                let shift = event
                    .current_event_state()
                    .contains(gdk::ModifierType::SHIFT_MASK);
                if shift || dx.abs() > dy.abs() {
                    this.pan(if shift && dx == 0.0 { dy } else { dx }, event.unit());
                } else {
                    this.scroll(dy, event.unit());
                }
            }
            // Contain even empty/edge gestures; never chain into another pane.
            glib::Propagation::Stop
        });
        stage.add_controller(controller);
    }
    fn scroll(&self, delta: f64, unit: gdk::ScrollUnit) {
        if let Some(terminal) = self.terminal.upgrade() {
            self.adjustment.set_value(
                self.adjustment.value() + pixel_delta(delta, terminal.char_height() as f64, unit),
            );
        }
    }
    fn pan(&self, delta: f64, unit: gdk::ScrollUnit) {
        if let (Some(terminal), Some(viewport)) = (self.terminal.upgrade(), self.viewport.upgrade())
        {
            let adjustment = viewport.hadjustment();
            adjustment.set_value(
                adjustment.value() + pixel_delta(delta, terminal.char_width() as f64, unit),
            );
        }
    }
    /// VTE consumes feeds asynchronously. Reveal the input line after a redraw,
    /// unless the user deliberately scrolls before the queued reveal runs.
    pub fn follow_input(self: &Rc<Self>) {
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(60), move || {
            if let Some(this) = weak.upgrade()
                && this.generation.get() == generation
            {
                this.sync();
                if let (Some(terminal), Some(viewport)) =
                    (this.terminal.upgrade(), this.viewport.upgrade())
                {
                    let history = terminal.vadjustment().unwrap();
                    history.set_value(history.upper() - history.page_size());
                    let canvas = viewport.vadjustment();
                    let cursor_row = preview_visible_origin(&terminal)
                        .map(|(origin, _)| terminal.cursor_position().1 - origin);
                    let y = cursor_row.map(|row| {
                        (f64::from(terminal.margin_top())
                            + row as f64 * terminal.char_height() as f64)
                            * this.scale.get()
                    });
                    if let Some(y) = y {
                        canvas.clamp_page(y, y + terminal.char_height() as f64 * this.scale.get());
                    } else {
                        canvas.set_value(canvas.upper() - canvas.page_size());
                    }
                    let x = (f64::from(terminal.margin_start())
                        + terminal.cursor_position().0 as f64 * terminal.char_width() as f64)
                        * this.scale.get();
                    viewport
                        .hadjustment()
                        .clamp_page(x, x + terminal.char_width() as f64 * this.scale.get());
                    this.sync();
                }
            }
        });
    }
    pub fn start(&self) {
        self.restore(0.0);
    }
    pub fn restore(&self, position: f64) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.sync();
        self.adjustment.set_value(position);
    }
    pub fn restore_after_redraw(self: &Rc<Self>, position: f64) {
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(60), move || {
            if let Some(this) = weak.upgrade()
                && this.generation.get() == generation
            {
                this.restore(position);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settle() {
        let deadline = std::time::Instant::now() + Duration::from_millis(160);
        let context = glib::MainContext::default();
        while std::time::Instant::now() < deadline {
            while context.pending() {
                context.iteration(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn pointer(window: &gtk::Window, widget: &gtk::Widget, mode: &str) {
        window.set_title(Some("TermiMochi point-to-edit test"));
        gtk::prelude::WidgetExt::display(window).flush();
        let rect = widget.compute_bounds(window).unwrap();
        let scale = window.scale_factor() as f32;
        let x = (rect.x() + rect.width() * 0.5) * scale;
        let y = (rect.y() + rect.height() * 0.5) * scale;
        let mut child = std::process::Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/preview-pointer-driver.py"
            ))
            .args([mode, &(x as i32).to_string(), &(y as i32).to_string(), "8"])
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("pointer timeout");
            }
            settle();
        }
        assert!(child.wait().unwrap().success());
        settle();
    }
    #[test]
    fn wheel_and_fractional_touchpad_scroll_use_pixels_without_acceleration() {
        assert_eq!(pixel_delta(-1.0, 20.0, gdk::ScrollUnit::Wheel), -60.0);
        assert_eq!(pixel_delta(0.25, 20.0, gdk::ScrollUnit::Surface), 0.25);
        assert_eq!(pixel_delta(f64::NAN, 20.0, gdk::ScrollUnit::Wheel), 0.0);
        assert_eq!(
            pixel_delta(f64::INFINITY, 20.0, gdk::ScrollUnit::Surface),
            0.0
        );
    }

    #[test]
    #[ignore = "requires X11 GTK/VTE; run separately at 1x and 2x"]
    fn terminal_scroll_is_independent_with_long_art_history_and_fixed_chrome() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.ScrollTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let temp = tempfile::tempdir().unwrap();
        present_advanced_with_preset(&app, None, temp.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        window.set_title(Some("TermiMochi point-to-edit test"));
        window.set_default_size(1120, 700);
        let this = crate::window::greeting::tests::project_controller(&window);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while this.preview_loading.get() || this.copy_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        this.greeting_module_button.set_active(true);
        this.preview_scene_selector.set_selected(3);
        this.greeting.replace(
            crate::greeting::GreetingSettings {
                enabled: true,
                logo: crate::greeting::Logo::Custom,
                custom_logo: (0..64)
                    .map(|n| format!("{n:02} | full-size terminal artwork"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                preview_columns: 120,
                ..Default::default()
            },
            false,
        );
        settle();
        settle();
        settle();
        while this.copy_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        for _ in 0..8 {
            settle();
        }
        let viewport = &this.preview_terminal_viewport;
        let canvas = viewport.vadjustment();
        let range = &this.preview_scroll.adjustment;
        assert!(canvas.upper() > canvas.page_size() + 500.0, "{:?}", canvas);
        assert!(
            viewport.height() < window.height() - 100,
            "terminal must fit below fixed header and above diagnostics"
        );
        assert!(
            this.preview_content
                .ancestor(gtk::ScrolledWindow::static_type())
                .is_none()
        );
        let header = this.preview_content.first_child().unwrap();
        let log = this.preview_content.last_child().unwrap();
        let header_bounds = header.compute_bounds(&window).unwrap();
        let log_bounds = log.compute_bounds(&window).unwrap();
        let left = this.greeting.root.vadjustment();
        let left_value = left.value();
        let layout = this.layout_settings();
        range.set_value(0.0);
        pointer(&window, viewport.upcast_ref(), "scroll_down");
        assert!(
            canvas.value() > 0.0,
            "real wheel must scroll the terminal canvas"
        );
        assert_eq!(header.compute_bounds(&window).unwrap(), header_bounds);
        assert_eq!(log.compute_bounds(&window).unwrap(), log_bounds);
        assert_eq!(left.value(), left_value);
        range.set_value(range.upper() - range.page_size());
        let bottom = range.value();
        pointer(&window, viewport.upcast_ref(), "scroll_down");
        assert_eq!(range.value(), bottom);
        range.set_value(0.0);
        pointer(&window, viewport.upcast_ref(), "scroll_up");
        assert_eq!(range.value(), 0.0);
        pointer(&window, viewport.upcast_ref(), "shift_scroll_down");
        assert!(viewport.hadjustment().value() > 0.0);
        assert_eq!(range.value(), 0.0);
        viewport.hadjustment().set_value(0.0);
        pointer(&window, viewport.upcast_ref(), "scroll_right");
        assert!(viewport.hadjustment().value() > 0.0);
        // The same hidden/visible rail owns both pixel canvas and VTE history.
        this.scrollbar_switch.set_active(true);
        settle();
        this.preview_terminal.reset(true, true);
        let lines = (0..240)
            .map(|n| format!("scrollback row {n:03}\r\n"))
            .collect::<String>();
        this.preview_terminal.feed(lines.as_bytes());
        settle();
        settle();
        let history = this.preview_terminal.vadjustment().unwrap();
        assert!(history.upper() > history.page_size() + 100.0);
        range.set_value(0.0);
        settle();
        assert_eq!(history.value(), history.lower());
        assert_eq!(canvas.value(), canvas.lower());
        pointer(&window, viewport.upcast_ref(), "scroll_down");
        assert!(history.value() > history.lower());
        range.set_value(range.upper() - range.page_size());
        settle();
        assert_eq!(history.value(), history.upper() - history.page_size());
        assert_eq!(canvas.value(), canvas.upper() - canvas.page_size());
        // Async compare restores survive native adjustment updates, but never
        // override a deliberate wheel/rail movement made in the meantime.
        this.preview_scroll.restore_after_redraw(80.0);
        history.set_value(history.lower());
        settle();
        assert!((range.value() - 80.0).abs() < 1.0);
        this.preview_scroll.restore_after_redraw(0.0);
        range.set_value(150.0);
        settle();
        assert!((range.value() - 150.0).abs() < 1.0);
        this.scrollbar_switch.set_active(layout.scrollbar);
        settle();
        assert_eq!(this.layout_settings(), layout);
        // Returning to an input line reveals it inside the local viewport.
        range.set_value(0.0);
        this.commit_preview_input("x");
        settle();
        settle();
        let origin = preview_visible_origin(&this.preview_terminal).unwrap().0;
        let y = (this.preview_terminal.cursor_position().1 - origin) as f64
            * this.preview_terminal.char_height() as f64
            + f64::from(this.preview_terminal.margin_top());
        assert!(
            y >= canvas.value()
                && y + this.preview_terminal.char_height() as f64
                    <= canvas.value() + canvas.page_size() + 1.0,
            "input cursor must remain inside the terminal viewport"
        );
        assert_eq!(header.compute_bounds(&window).unwrap(), header_bounds);
        assert_eq!(log.compute_bounds(&window).unwrap(), log_bounds);
        // Scrolling the editor or its diagnostic log stays local too.
        let terminal_value = range.value();
        pointer(&window, this.greeting.root.upcast_ref(), "scroll_down");
        assert!(left.value() > left_value);
        assert_eq!(range.value(), terminal_value);
        for index in 0..12 {
            this.diagnostics
                .append(&gtk::Label::new(Some(&format!("Local diagnostic {index}"))));
        }
        settle();
        let diagnostics = this
            .diagnostics
            .ancestor(gtk::ScrolledWindow::static_type())
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap();
        pointer(&window, diagnostics.upcast_ref(), "scroll_down");
        assert!(diagnostics.vadjustment().value() > 0.0);
        assert_eq!(range.value(), terminal_value);
        if std::env::var_os("TERMIMOCHI_INSPECT_SCREENSHOT").is_some() {
            pointer(&window, viewport.upcast_ref(), "capture");
        }
        window.close();
        settle();
    }
}
