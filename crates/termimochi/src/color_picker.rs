use std::{
    cell::{Cell, RefCell},
    f64::consts::TAU,
    rc::Rc,
    str::FromStr,
};

use gtk::{cairo, gdk, prelude::*};
use termimochi_core::Rgb;

type ChangeHandler = Box<dyn Fn(Rgb)>;
type EditHandler = Box<dyn Fn()>;
type DraftHandler = Box<dyn Fn(bool)>;

#[derive(Clone)]
pub(crate) struct ColorPicker {
    root: gtk::Box,
    shared: Rc<PickerShared>,
}

struct PickerShared {
    state: RefCell<PickerState>,
    updating: Cell<bool>,
    edit_active: Cell<bool>,
    invalid_draft: Cell<bool>,
    square: gtk::DrawingArea,
    hue: gtk::Scale,
    preview: gtk::DrawingArea,
    hex: gtk::Entry,
    red: gtk::Entry,
    green: gtk::Entry,
    blue: gtk::Entry,
    handlers: RefCell<Vec<ChangeHandler>>,
    edit_begin_handlers: RefCell<Vec<EditHandler>>,
    edit_end_handlers: RefCell<Vec<EditHandler>>,
    draft_handlers: RefCell<Vec<DraftHandler>>,
}

#[derive(Clone, Copy, Debug)]
struct PickerState {
    hsv: Hsv,
    rgb: Rgb,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Hsv {
    hue: f64,
    saturation: f64,
    value: f64,
}

impl ColorPicker {
    pub(crate) fn new(initial: Rgb) -> Self {
        let square = gtk::DrawingArea::builder()
            .content_width(280)
            .content_height(190)
            .hexpand(true)
            .focusable(true)
            .accessible_role(gtk::AccessibleRole::Group)
            .tooltip_text("Drag to set saturation and brightness")
            .css_classes(["picker-square"])
            .build();
        square.set_cursor_from_name(Some("crosshair"));
        square.update_property(&[
            gtk::accessible::Property::Label("Saturation and Brightness"),
            gtk::accessible::Property::Description("Drag, or use the arrow keys to fine-tune"),
        ]);
        let hue_adjustment = gtk::Adjustment::new(0.0, 0.0, 359.999_999, 1.0, 10.0, 0.0);
        let hue = gtk::Scale::builder()
            .orientation(gtk::Orientation::Vertical)
            .adjustment(&hue_adjustment)
            .draw_value(false)
            .has_origin(false)
            .inverted(true)
            .width_request(26)
            .height_request(190)
            .focusable(true)
            .tooltip_text("Drag to set hue")
            .css_classes(["picker-hue-scale"])
            .build();
        hue.update_property(&[
            gtk::accessible::Property::Label("Hue"),
            gtk::accessible::Property::Description(
                "Drag vertically, or use the arrow keys to fine-tune",
            ),
            gtk::accessible::Property::Orientation(gtk::Orientation::Vertical),
            gtk::accessible::Property::ValueMin(0.0),
            gtk::accessible::Property::ValueMax(360.0),
            gtk::accessible::Property::ValueNow(0.0),
        ]);
        let preview = gtk::DrawingArea::builder()
            .content_width(40)
            .content_height(36)
            .accessible_role(gtk::AccessibleRole::Img)
            .tooltip_text("Current Color")
            .css_classes(["picker-current"])
            .build();
        preview.update_property(&[gtk::accessible::Property::Label("Current Color")]);
        let hex = gtk::Entry::builder()
            .width_chars(8)
            // Keep one character beyond the valid form so a common paste or
            // typing error stays visible and invalid instead of becoming a
            // silently-truncated valid color. The bound also prevents an
            // arbitrarily large paste from stalling the UI.
            .max_length(8)
            .enable_undo(false)
            .placeholder_text("#RRGGBB")
            .css_classes(["hex-entry"])
            .build();
        let red = channel_entry();
        let green = channel_entry();
        let blue = channel_entry();
        hex.update_property(&[gtk::accessible::Property::Label("HEX Color Value")]);
        red.update_property(&[gtk::accessible::Property::Label("Red Channel")]);
        green.update_property(&[gtk::accessible::Property::Label("Green Channel")]);
        blue.update_property(&[gtk::accessible::Property::Label("Blue Channel")]);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.add_css_class("professional-picker");
        let canvas_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        canvas_row.append(&square);
        canvas_row.append(&hue);
        root.append(&canvas_row);

        let exact = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        preview.set_valign(gtk::Align::End);
        exact.append(&preview);
        let hex_field = labeled_field("HEX", &hex);
        hex_field.set_hexpand(true);
        exact.append(&hex_field);
        for field in [
            numeric_field("R", &red),
            numeric_field("G", &green),
            numeric_field("B", &blue),
        ] {
            field.set_hexpand(true);
            exact.append(&field);
        }
        root.append(&exact);

        let shared = Rc::new(PickerShared {
            state: RefCell::new(PickerState {
                hsv: rgb_to_hsv(initial),
                rgb: initial,
            }),
            updating: Cell::new(false),
            edit_active: Cell::new(false),
            invalid_draft: Cell::new(false),
            square,
            hue,
            preview,
            hex,
            red,
            green,
            blue,
            handlers: RefCell::new(Vec::new()),
            edit_begin_handlers: RefCell::new(Vec::new()),
            edit_end_handlers: RefCell::new(Vec::new()),
            draft_handlers: RefCell::new(Vec::new()),
        });
        install_drawing(&shared);
        install_interaction(&shared);
        apply_rgb(&shared, initial, false);

        Self { root, shared }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub(crate) fn set_color(&self, color: Rgb) {
        apply_rgb(&self.shared, color, false);
    }

    pub(crate) fn connect_changed<F>(&self, handler: F)
    where
        F: Fn(Rgb) + 'static,
    {
        self.shared.handlers.borrow_mut().push(Box::new(handler));
    }

    pub(crate) fn connect_edit_begin<F>(&self, handler: F)
    where
        F: Fn() + 'static,
    {
        self.shared
            .edit_begin_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub(crate) fn connect_edit_end<F>(&self, handler: F)
    where
        F: Fn() + 'static,
    {
        self.shared
            .edit_end_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub(crate) fn connect_draft_changed<F>(&self, handler: F)
    where
        F: Fn(bool) + 'static,
    {
        self.shared
            .draft_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub(crate) fn finish_edit(&self) {
        end_edit(&self.shared);
    }

    pub(crate) fn has_invalid_draft(&self) -> bool {
        self.shared.invalid_draft.get()
    }

    pub(crate) fn discard_draft(&self) {
        end_edit(&self.shared);
        sync_controls(&self.shared);
    }
}

#[derive(Clone)]
pub(crate) struct ColorSwatch {
    button: gtk::Button,
    area: gtk::DrawingArea,
    color: Rc<Cell<Rgb>>,
}

impl ColorSwatch {
    pub(crate) fn new(compact: bool, tooltip: &str) -> Self {
        let color = Rc::new(Cell::new(Rgb::new(0, 0, 0)));
        let area = gtk::DrawingArea::builder()
            .content_width(if compact { 24 } else { 48 })
            .content_height(if compact { 24 } else { 36 })
            .hexpand(!compact)
            .build();
        let drawing_color = Rc::clone(&color);
        area.set_draw_func(move |_, context, width, height| {
            let color = drawing_color.get();
            rounded_rectangle(
                context,
                0.0,
                0.0,
                f64::from(width),
                f64::from(height),
                if compact { 2.5 } else { 3.5 },
            );
            set_rgb_source(context, color);
            let _ = context.fill();
        });
        let button = gtk::Button::builder()
            .child(&area)
            .tooltip_text(tooltip)
            .css_classes(if compact {
                ["palette-swatch", "ansi-swatch-button"]
            } else {
                ["palette-swatch", "basic-swatch-button"]
            })
            .build();
        button.update_property(&[gtk::accessible::Property::Label(tooltip)]);
        if !compact {
            button.set_hexpand(true);
        }
        Self {
            button,
            area,
            color,
        }
    }

    pub(crate) fn button(&self) -> &gtk::Button {
        &self.button
    }

    pub(crate) fn set_color(&self, color: Rgb) {
        self.color.set(color);
        self.area.queue_draw();
    }

    pub(crate) fn set_selected(&self, selected: bool) {
        if selected {
            self.button.add_css_class("selected-swatch");
        } else {
            self.button.remove_css_class("selected-swatch");
        }
    }
}

fn channel_entry() -> gtk::Entry {
    gtk::Entry::builder()
        .width_chars(3)
        .max_width_chars(3)
        // Decimal RGB is at most three digits. Retaining a fourth digit makes
        // overflow visible to validation without allowing unbounded input.
        .max_length(4)
        .enable_undo(false)
        .xalign(1.0)
        .input_purpose(gtk::InputPurpose::Digits)
        .css_classes(["rgb-entry"])
        .build()
}

fn numeric_field(label: &str, entry: &gtk::Entry) -> gtk::Box {
    labeled_field(label, entry)
}

fn labeled_field(label: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let field = gtk::Box::new(gtk::Orientation::Vertical, 3);
    let label = gtk::Label::new(Some(label));
    label.set_xalign(0.0);
    label.add_css_class("picker-field-label");
    field.append(&label);
    field.append(widget);
    field
}

fn install_drawing(shared: &Rc<PickerShared>) {
    let weak = Rc::downgrade(shared);
    shared
        .square
        .set_draw_func(move |_, context, width, height| {
            let Some(shared) = weak.upgrade() else {
                return;
            };
            draw_square(&shared, context, width, height);
        });

    let weak = Rc::downgrade(shared);
    shared
        .preview
        .set_draw_func(move |_, context, width, height| {
            let Some(shared) = weak.upgrade() else {
                return;
            };
            let color = shared.state.borrow().rgb;
            rounded_rectangle(context, 0.0, 0.0, f64::from(width), f64::from(height), 3.5);
            set_rgb_source(context, color);
            let _ = context.fill_preserve();
            if color.relative_luminance() > 0.45 {
                context.set_source_rgba(0.0, 0.0, 0.0, 0.24);
            } else {
                context.set_source_rgba(1.0, 1.0, 1.0, 0.35);
            }
            context.set_line_width(1.0);
            let _ = context.stroke();
        });
}

fn begin_edit(shared: &PickerShared) {
    if shared.updating.get() || shared.edit_active.replace(true) {
        return;
    }
    for handler in shared.edit_begin_handlers.borrow().iter() {
        handler();
    }
}

fn end_edit(shared: &PickerShared) {
    if !shared.edit_active.replace(false) {
        return;
    }
    for handler in shared.edit_end_handlers.borrow().iter() {
        handler();
    }
}

fn begin_implicit_edit(shared: &PickerShared, focused: bool) -> bool {
    let started = !shared.edit_active.get();
    if started {
        begin_edit(shared);
    }
    // If an application command finished the previous transaction while this
    // Entry kept focus (for example Ctrl+Z or Save), the next keystroke must
    // start a new focused transaction and remain open until focus leaves.
    started && !focused
}

fn end_implicit_edit(shared: &PickerShared, implicit: bool) {
    if implicit {
        end_edit(shared);
    }
}

fn install_entry_focus(shared: &Rc<PickerShared>, entry: &gtk::Entry) -> gtk::EventControllerFocus {
    let focus = gtk::EventControllerFocus::new();
    let weak = Rc::downgrade(shared);
    focus.connect_enter(move |_| {
        if let Some(shared) = weak.upgrade() {
            begin_edit(&shared);
        }
    });
    let weak = Rc::downgrade(shared);
    focus.connect_leave(move |_| {
        if let Some(shared) = weak.upgrade() {
            end_edit(&shared);
        }
    });
    entry.add_controller(focus.clone());
    focus
}

fn is_arrow_key(key: gdk::Key) -> bool {
    matches!(
        key,
        gdk::Key::Left | gdk::Key::Right | gdk::Key::Up | gdk::Key::Down
    )
}

fn install_interaction(shared: &Rc<PickerShared>) {
    let square_drag = gtk::GestureDrag::new();
    let weak = Rc::downgrade(shared);
    square_drag.connect_drag_begin(move |_, x, y| {
        if let Some(shared) = weak.upgrade() {
            shared.square.grab_focus();
            begin_edit(&shared);
            update_square_from_point(&shared, x, y);
        }
    });
    let weak = Rc::downgrade(shared);
    square_drag.connect_drag_update(move |gesture, offset_x, offset_y| {
        let Some(shared) = weak.upgrade() else {
            return;
        };
        if let Some((start_x, start_y)) = gesture.start_point() {
            update_square_from_point(&shared, start_x + offset_x, start_y + offset_y);
        }
    });
    let weak = Rc::downgrade(shared);
    square_drag.connect_drag_end(move |_, _, _| {
        if let Some(shared) = weak.upgrade() {
            end_edit(&shared);
        }
    });
    let weak = Rc::downgrade(shared);
    square_drag.connect_cancel(move |_, _| {
        if let Some(shared) = weak.upgrade() {
            end_edit(&shared);
        }
    });
    shared.square.add_controller(square_drag);

    let square_keys = gtk::EventControllerKey::new();
    let square_key_edit = Rc::new(Cell::new(false));
    let weak = Rc::downgrade(shared);
    let key_edit = Rc::clone(&square_key_edit);
    square_keys.connect_key_pressed(move |_, key, _, modifiers| {
        let Some(shared) = weak.upgrade() else {
            return gtk::glib::Propagation::Proceed;
        };
        if !is_arrow_key(key) {
            return gtk::glib::Propagation::Proceed;
        }
        if !key_edit.replace(true) {
            begin_edit(&shared);
        }
        let step = if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
            0.1
        } else {
            0.01
        };
        let mut hsv = shared.state.borrow().hsv;
        match key {
            gdk::Key::Left => hsv.saturation -= step,
            gdk::Key::Right => hsv.saturation += step,
            gdk::Key::Up => hsv.value += step,
            gdk::Key::Down => hsv.value -= step,
            _ => return gtk::glib::Propagation::Proceed,
        }
        apply_hsv(&shared, hsv, true);
        gtk::glib::Propagation::Stop
    });
    let weak = Rc::downgrade(shared);
    let key_edit = Rc::clone(&square_key_edit);
    square_keys.connect_key_released(move |_, key, _, _| {
        if is_arrow_key(key)
            && key_edit.replace(false)
            && let Some(shared) = weak.upgrade()
        {
            end_edit(&shared);
        }
    });
    shared.square.add_controller(square_keys);

    let square_focus = gtk::EventControllerFocus::new();
    let weak = Rc::downgrade(shared);
    let key_edit = Rc::clone(&square_key_edit);
    square_focus.connect_leave(move |_| {
        key_edit.set(false);
        if let Some(shared) = weak.upgrade() {
            end_edit(&shared);
        }
    });
    shared.square.add_controller(square_focus);

    let weak = Rc::downgrade(shared);
    shared.hue.connect_value_changed(move |scale| {
        let Some(shared) = weak.upgrade() else {
            return;
        };
        if shared.updating.get() {
            return;
        }
        let implicit = begin_implicit_edit(&shared, false);
        let mut hsv = shared.state.borrow().hsv;
        hsv.hue = scale.value();
        apply_hsv(&shared, hsv, true);
        end_implicit_edit(&shared, implicit);
    });

    let hue_click = gtk::GestureClick::builder()
        .button(gdk::BUTTON_PRIMARY)
        .propagation_phase(gtk::PropagationPhase::Capture)
        .build();
    let hue_pointer_edit = Rc::new(Cell::new(false));
    let weak = Rc::downgrade(shared);
    let pointer_edit = Rc::clone(&hue_pointer_edit);
    hue_click.connect_pressed(move |_, _, _, _| {
        if let Some(shared) = weak.upgrade() {
            shared.hue.grab_focus();
            if !pointer_edit.replace(true) {
                begin_edit(&shared);
            }
        }
    });
    let weak = Rc::downgrade(shared);
    let pointer_edit = Rc::clone(&hue_pointer_edit);
    hue_click.connect_released(move |_, _, _, _| {
        if pointer_edit.replace(false)
            && let Some(shared) = weak.upgrade()
        {
            end_edit(&shared);
        }
    });
    let weak = Rc::downgrade(shared);
    let pointer_edit = Rc::clone(&hue_pointer_edit);
    hue_click.connect_cancel(move |_, _| {
        if pointer_edit.replace(false)
            && let Some(shared) = weak.upgrade()
        {
            end_edit(&shared);
        }
    });
    shared.hue.add_controller(hue_click);

    // GtkScale changes its value for every wheel/touchpad event by default.
    // That is surprising inside the editor's vertical ScrolledWindow: merely
    // trying to reach the fields below the picker would silently change hue.
    // Intercept vertical scrolling before GtkScale and apply the same delta to
    // the nearest parent ScrolledWindow instead.
    let hue_scroll = gtk::EventControllerScroll::new(
        gtk::EventControllerScrollFlags::VERTICAL | gtk::EventControllerScrollFlags::HORIZONTAL,
    );
    hue_scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(shared);
    hue_scroll.connect_scroll(move |controller, _, delta_y| {
        if let Some(shared) = weak.upgrade()
            && delta_y != 0.0
        {
            scroll_parent_vertically(&shared.hue, delta_y, controller.unit());
        }
        // Stop GtkScale for every axis, including a purely horizontal
        // touchpad gesture, so no scroll path can mutate hue. Vertical input
        // has already been forwarded above.
        gtk::glib::Propagation::Stop
    });
    shared.hue.add_controller(hue_scroll);

    let hue_keys = gtk::EventControllerKey::new();
    // Run before GtkScale's built-in arrow handling so its value-changed
    // signal remains inside the same press/release edit transaction.
    hue_keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let hue_key_edit = Rc::new(Cell::new(false));
    let weak = Rc::downgrade(shared);
    let key_edit = Rc::clone(&hue_key_edit);
    hue_keys.connect_key_pressed(move |_, key, _, modifiers| {
        let Some(shared) = weak.upgrade() else {
            return gtk::glib::Propagation::Proceed;
        };
        if !is_arrow_key(key) {
            return gtk::glib::Propagation::Proceed;
        }
        if !key_edit.replace(true) {
            begin_edit(&shared);
        }
        if !modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
            return gtk::glib::Propagation::Proceed;
        }
        let mut hsv = shared.state.borrow().hsv;
        match key {
            gdk::Key::Up | gdk::Key::Left => hsv.hue -= 10.0,
            gdk::Key::Down | gdk::Key::Right => hsv.hue += 10.0,
            _ => return gtk::glib::Propagation::Proceed,
        }
        apply_hsv(&shared, hsv, true);
        gtk::glib::Propagation::Stop
    });
    let weak = Rc::downgrade(shared);
    let key_edit = Rc::clone(&hue_key_edit);
    hue_keys.connect_key_released(move |_, key, _, _| {
        if is_arrow_key(key)
            && key_edit.replace(false)
            && let Some(shared) = weak.upgrade()
        {
            end_edit(&shared);
        }
    });
    shared.hue.add_controller(hue_keys);

    let hue_focus = gtk::EventControllerFocus::new();
    let weak = Rc::downgrade(shared);
    let key_edit = Rc::clone(&hue_key_edit);
    let pointer_edit = Rc::clone(&hue_pointer_edit);
    hue_focus.connect_leave(move |_| {
        key_edit.set(false);
        pointer_edit.set(false);
        if let Some(shared) = weak.upgrade() {
            end_edit(&shared);
        }
    });
    shared.hue.add_controller(hue_focus);

    let hex_focus = install_entry_focus(shared, &shared.hex);
    let weak = Rc::downgrade(shared);
    let focus = hex_focus.clone();
    shared.hex.connect_changed(move |entry| {
        let Some(shared) = weak.upgrade() else {
            return;
        };
        if shared.updating.get() {
            return;
        }
        let implicit = begin_implicit_edit(&shared, focus.contains_focus());
        match Rgb::from_str(entry.text().as_str()) {
            Ok(color) => {
                set_entry_error(entry, None);
                apply_rgb(&shared, color, true);
            }
            Err(_) => set_entry_error(
                entry,
                Some("Enter a 6-digit HEX color, for example #3A7BD5"),
            ),
        }
        update_draft_state(&shared);
        end_implicit_edit(&shared, implicit);
    });

    for entry in [&shared.red, &shared.green, &shared.blue] {
        let entry_focus = install_entry_focus(shared, entry);
        let weak = Rc::downgrade(shared);
        let focus = entry_focus.clone();
        entry.connect_changed(move |_| {
            let Some(shared) = weak.upgrade() else {
                return;
            };
            if shared.updating.get() {
                return;
            }
            let implicit = begin_implicit_edit(&shared, focus.contains_focus());
            apply_channel_entries(&shared);
            update_draft_state(&shared);
            end_implicit_edit(&shared, implicit);
        });
    }
}

const SURFACE_SCROLL_FACTOR: f64 = 2.5;

fn scroll_parent_vertically(widget: &impl IsA<gtk::Widget>, delta: f64, unit: gdk::ScrollUnit) {
    let Some(parent) = widget
        .ancestor(gtk::ScrolledWindow::static_type())
        .and_then(|widget| widget.downcast::<gtk::ScrolledWindow>().ok())
    else {
        return;
    };
    let adjustment = parent.vadjustment();
    let destination = scroll_destination(
        adjustment.value(),
        adjustment.lower(),
        adjustment.upper(),
        adjustment.page_size(),
        delta,
        unit,
    );
    adjustment.set_value(destination);
}

fn scroll_destination(
    value: f64,
    lower: f64,
    upper: f64,
    page_size: f64,
    delta: f64,
    unit: gdk::ScrollUnit,
) -> f64 {
    if !delta.is_finite() {
        return value;
    }
    // Match GtkScrolledWindow's native scaling: a wheel detent is a
    // page-relative step, while touchpad/surface deltas remain continuous.
    let factor = match unit {
        gdk::ScrollUnit::Wheel => page_size.max(0.0).powf(2.0 / 3.0),
        gdk::ScrollUnit::Surface => SURFACE_SCROLL_FACTOR,
        _ => SURFACE_SCROLL_FACTOR,
    };
    let maximum = (upper - page_size).max(lower);
    (value + delta * factor).clamp(lower, maximum)
}

fn apply_channel_entries(shared: &Rc<PickerShared>) {
    let red = validate_channel(&shared.red);
    let green = validate_channel(&shared.green);
    let blue = validate_channel(&shared.blue);
    if let (Some(red), Some(green), Some(blue)) = (red, green, blue) {
        apply_rgb(shared, Rgb::new(red, green, blue), true);
    }
}

fn validate_channel(entry: &gtk::Entry) -> Option<u8> {
    let value = parse_channel_text(entry.text().as_str());
    if value.is_some() {
        set_entry_error(entry, None);
    } else {
        set_entry_error(entry, Some("Enter a whole number from 0 to 255"));
    }
    value
}

fn set_entry_error(entry: &gtk::Entry, message: Option<&str>) {
    if let Some(message) = message {
        entry.add_css_class("error");
        entry.set_tooltip_text(Some(message));
        entry.update_property(&[gtk::accessible::Property::Description(message)]);
        entry.update_state(&[gtk::accessible::State::Invalid(
            gtk::AccessibleInvalidState::True,
        )]);
    } else {
        entry.remove_css_class("error");
        entry.set_tooltip_text(None);
        entry.update_property(&[gtk::accessible::Property::Description("")]);
        entry.update_state(&[gtk::accessible::State::Invalid(
            gtk::AccessibleInvalidState::False,
        )]);
    }
}

fn parse_channel_text(text: &str) -> Option<u8> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn draft_is_invalid(shared: &PickerShared) -> bool {
    Rgb::from_str(shared.hex.text().as_str()).is_err()
        || [&shared.red, &shared.green, &shared.blue]
            .into_iter()
            .any(|entry| parse_channel_text(entry.text().as_str()).is_none())
}

fn update_draft_state(shared: &PickerShared) {
    let invalid = draft_is_invalid(shared);
    if shared.invalid_draft.replace(invalid) == invalid {
        return;
    }
    for handler in shared.draft_handlers.borrow().iter() {
        handler(invalid);
    }
}

fn update_square_from_point(shared: &Rc<PickerShared>, x: f64, y: f64) {
    let width = f64::from(shared.square.width().max(1));
    let height = f64::from(shared.square.height().max(1));
    let mut hsv = shared.state.borrow().hsv;
    hsv.saturation = (x / width).clamp(0.0, 1.0);
    hsv.value = (1.0 - y / height).clamp(0.0, 1.0);
    apply_hsv(shared, hsv, true);
}

fn apply_rgb(shared: &Rc<PickerShared>, color: Rgb, notify: bool) {
    let previous_hue = shared.state.borrow().hsv.hue;
    let mut hsv = rgb_to_hsv(color);
    // Hue is undefined for greys. Keeping the last meaningful hue makes it
    // possible to add saturation again without the picker jumping to red.
    if hsv.saturation <= f64::EPSILON {
        hsv.hue = previous_hue;
    }
    *shared.state.borrow_mut() = PickerState { hsv, rgb: color };
    sync_controls(shared);
    if notify {
        for handler in shared.handlers.borrow().iter() {
            handler(color);
        }
    }
}

fn apply_hsv(shared: &Rc<PickerShared>, hsv: Hsv, notify: bool) {
    let hsv = Hsv {
        hue: hsv.hue.rem_euclid(360.0),
        saturation: hsv.saturation.clamp(0.0, 1.0),
        value: hsv.value.clamp(0.0, 1.0),
    };
    let color = hsv_to_rgb(hsv);
    *shared.state.borrow_mut() = PickerState { hsv, rgb: color };
    sync_controls(shared);
    if notify {
        for handler in shared.handlers.borrow().iter() {
            handler(color);
        }
    }
}

fn sync_controls(shared: &PickerShared) {
    let state = *shared.state.borrow();
    let color = state.rgb;
    let was_updating = shared.updating.replace(true);
    shared.hue.set_value(state.hsv.hue);
    shared.hex.set_text(&color.to_hex());
    set_entry_error(&shared.hex, None);
    sync_channel(&shared.red, color.red());
    sync_channel(&shared.green, color.green());
    sync_channel(&shared.blue, color.blue());
    shared.updating.set(was_updating);
    update_draft_state(shared);
    let square_description = format!(
        "Saturation {:.0}%, brightness {:.0}%; drag or use the arrow keys to fine-tune",
        state.hsv.saturation * 100.0,
        state.hsv.value * 100.0
    );
    let hue_value = format!("{:.0}°", state.hsv.hue);
    let preview_description = color.to_hex();
    shared
        .square
        .update_property(&[gtk::accessible::Property::Description(&square_description)]);
    shared.hue.update_property(&[
        gtk::accessible::Property::ValueNow(state.hsv.hue),
        gtk::accessible::Property::ValueText(&hue_value),
    ]);
    shared
        .preview
        .update_property(&[gtk::accessible::Property::Description(&preview_description)]);
    shared.square.queue_draw();
    shared.hue.queue_draw();
    shared.preview.queue_draw();
}

fn sync_channel(entry: &gtk::Entry, value: u8) {
    if parse_channel_text(entry.text().as_str()) != Some(value) {
        entry.set_text(&value.to_string());
    }
    set_entry_error(entry, None);
}

fn draw_square(shared: &PickerShared, context: &cairo::Context, width: i32, height: i32) {
    let width = f64::from(width);
    let height = f64::from(height);
    let hsv = shared.state.borrow().hsv;
    let hue_color = hsv_to_rgb(Hsv {
        hue: hsv.hue,
        saturation: 1.0,
        value: 1.0,
    });

    rounded_rectangle(context, 0.0, 0.0, width, height, 4.0);
    let _ = context.save();
    context.clip();
    set_rgb_source(context, hue_color);
    let _ = context.paint();

    let white = cairo::LinearGradient::new(0.0, 0.0, width, 0.0);
    white.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 1.0);
    white.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
    let _ = context.set_source(&white);
    let _ = context.paint();

    let black = cairo::LinearGradient::new(0.0, 0.0, 0.0, height);
    black.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
    black.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 1.0);
    let _ = context.set_source(&black);
    let _ = context.paint();
    let _ = context.restore();

    let marker_x = hsv.saturation * width;
    let marker_y = (1.0 - hsv.value) * height;
    context.arc(marker_x, marker_y, 7.0, 0.0, TAU);
    context.set_source_rgba(0.0, 0.0, 0.0, 0.72);
    context.set_line_width(3.5);
    let _ = context.stroke();
    context.arc(marker_x, marker_y, 6.5, 0.0, TAU);
    context.set_source_rgb(1.0, 1.0, 1.0);
    context.set_line_width(2.0);
    let _ = context.stroke();
}

fn rounded_rectangle(
    context: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) {
    let radius = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    context.new_sub_path();
    context.arc(x + width - radius, y + radius, radius, -TAU / 4.0, 0.0);
    context.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        TAU / 4.0,
    );
    context.arc(
        x + radius,
        y + height - radius,
        radius,
        TAU / 4.0,
        TAU / 2.0,
    );
    context.arc(x + radius, y + radius, radius, TAU / 2.0, 3.0 * TAU / 4.0);
    context.close_path();
}

fn set_rgb_source(context: &cairo::Context, color: Rgb) {
    context.set_source_rgb(
        f64::from(color.red()) / 255.0,
        f64::from(color.green()) / 255.0,
        f64::from(color.blue()) / 255.0,
    );
}

fn rgb_to_hsv(color: Rgb) -> Hsv {
    let red = f64::from(color.red()) / 255.0;
    let green = f64::from(color.green()) / 255.0;
    let blue = f64::from(color.blue()) / 255.0;
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let delta = maximum - minimum;
    let hue = if delta <= f64::EPSILON {
        0.0
    } else if (maximum - red).abs() <= f64::EPSILON {
        60.0 * ((green - blue) / delta).rem_euclid(6.0)
    } else if (maximum - green).abs() <= f64::EPSILON {
        60.0 * ((blue - red) / delta + 2.0)
    } else {
        60.0 * ((red - green) / delta + 4.0)
    };
    Hsv {
        hue,
        saturation: if maximum <= f64::EPSILON {
            0.0
        } else {
            delta / maximum
        },
        value: maximum,
    }
}

fn hsv_to_rgb(hsv: Hsv) -> Rgb {
    let hue = hsv.hue.rem_euclid(360.0);
    let saturation = hsv.saturation.clamp(0.0, 1.0);
    let value = hsv.value.clamp(0.0, 1.0);
    let chroma = value * saturation;
    let sector = hue / 60.0;
    let x = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let (red, green, blue) = match sector.floor() as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let minimum = value - chroma;
    let channel = |channel: f64| ((channel + minimum) * 255.0).round() as u8;
    Rgb::new(channel(red), channel(green), channel(blue))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_hsv_round_trips_exactly_for_representative_colors() {
        for color in [
            Rgb::new(0, 0, 0),
            Rgb::new(255, 255, 255),
            Rgb::new(255, 0, 0),
            Rgb::new(0, 255, 0),
            Rgb::new(0, 0, 255),
            Rgb::new(121, 191, 232),
            Rgb::new(231, 232, 229),
        ] {
            assert_eq!(hsv_to_rgb(rgb_to_hsv(color)), color);
        }
    }

    #[test]
    fn hue_sectors_produce_primary_and_secondary_colors() {
        let at = |hue| {
            hsv_to_rgb(Hsv {
                hue,
                saturation: 1.0,
                value: 1.0,
            })
        };
        assert_eq!(at(0.0), Rgb::new(255, 0, 0));
        assert_eq!(at(60.0), Rgb::new(255, 255, 0));
        assert_eq!(at(120.0), Rgb::new(0, 255, 0));
        assert_eq!(at(180.0), Rgb::new(0, 255, 255));
        assert_eq!(at(240.0), Rgb::new(0, 0, 255));
        assert_eq!(at(300.0), Rgb::new(255, 0, 255));
    }

    #[test]
    fn hue_wraps_at_full_circle() {
        let color = |hue| {
            hsv_to_rgb(Hsv {
                hue,
                saturation: 0.75,
                value: 0.8,
            })
        };
        assert_eq!(color(0.0), color(360.0));
        assert_eq!(color(30.0), color(390.0));
    }

    #[test]
    fn rgb_channel_text_is_decimal_and_bounded() {
        assert_eq!(parse_channel_text("0"), Some(0));
        assert_eq!(parse_channel_text("042"), Some(42));
        assert_eq!(parse_channel_text("255"), Some(255));
        for invalid in ["", "-1", "+1", "1.0", "256", "2559", "red", "１２"] {
            assert_eq!(parse_channel_text(invalid), None);
        }
    }

    #[test]
    fn wheel_scroll_uses_the_parent_viewport_size() {
        let destination =
            scroll_destination(100.0, 0.0, 1_000.0, 216.0, 1.0, gdk::ScrollUnit::Wheel);
        // GtkScrolledWindow uses page_size^(2/3): 216^(2/3) = 36.
        assert!((destination - 136.0).abs() < 1e-9);
    }

    #[test]
    fn surface_scroll_keeps_fractional_touchpad_deltas() {
        let destination =
            scroll_destination(100.0, 0.0, 1_000.0, 200.0, 1.25, gdk::ScrollUnit::Surface);
        assert!((destination - 103.125).abs() < 1e-9);
    }

    #[test]
    fn forwarded_scroll_is_clamped_to_the_adjustment_range() {
        assert_eq!(
            scroll_destination(10.0, 10.0, 510.0, 100.0, -20.0, gdk::ScrollUnit::Surface,),
            10.0
        );
        assert_eq!(
            scroll_destination(390.0, 10.0, 510.0, 100.0, 20.0, gdk::ScrollUnit::Surface,),
            410.0
        );
    }
}
