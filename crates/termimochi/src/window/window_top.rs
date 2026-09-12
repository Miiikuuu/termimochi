use super::*;
use crate::layout::window_top::{self as model, TabEdge, TabStyle, TitlebarColor};
#[cfg(test)]
#[path = "window_top_tests.rs"]
mod tests;

struct ColorControl {
    button: gtk::MenuButton,
    picker: ColorPicker,
    value: Rc<Cell<[u8; 3]>>,
}
impl ColorControl {
    fn new(label: &str, value: [u8; 3]) -> Self {
        let picker = ColorPicker::new(Rgb::new(value[0], value[1], value[2]));
        let popover = gtk::Popover::builder().child(picker.widget()).build();
        let button = gtk::MenuButton::builder()
            .label(model::hex(value))
            .popover(&popover)
            .build();
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        let value = Rc::new(Cell::new(value));
        let v = value.clone();
        let b = button.downgrade();
        picker.connect_changed(move |c| {
            v.set([c.red(), c.green(), c.blue()]);
            if let Some(b) = b.upgrade() {
                b.set_label(&c.to_hex());
            }
        });
        Self {
            button,
            picker,
            value,
        }
    }
    fn set(&self, value: [u8; 3]) {
        self.value.set(value);
        self.button.set_label(&model::hex(value));
        self.picker
            .set_color(Rgb::new(value[0], value[1], value[2]));
    }
}

pub(super) struct WindowTop {
    pub root: gtk::Box,
    title_mode: gtk::DropDown,
    title_color: ColorControl,
    title_row: gtk::Box,
    title_note: gtk::Label,
    ptyxis_row: gtk::Box,
    ptyxis_colors: [ColorControl; 2],
    tabs: gtk::Box,
    edge: gtk::DropDown,
    style: gtk::DropDown,
    minimum: gtk::SpinButton,
    colors: [ColorControl; 4],
    syncing: Cell<bool>,
}
impl WindowTop {
    pub fn title_focus(&self) -> gtk::Widget {
        if self.ptyxis_row.is_visible() {
            self.ptyxis_colors[0].button.clone().upcast()
        } else {
            self.title_mode.clone().upcast()
        }
    }
    pub fn new(l: &LayoutSettings, visible: &gtk::Switch) -> Self {
        let title_mode =
            gtk::DropDown::from_strings(&["Follow System", "Theme Background", "Custom Color"]);
        let title_color = ColorControl::new("Title Bar Background", [240, 240, 240]);
        let title_row = layout_group(
            "Title Bar",
            [
                layout_row("Background", &title_mode),
                layout_row("Custom Color", &title_color.button),
            ],
        );
        let title_note = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build();
        let ptyxis_colors = [
            ColorControl::new("Ptyxis Title Bar Background", [240, 240, 240]),
            ColorControl::new("Ptyxis Title Bar Text", [35, 35, 35]),
        ];
        let ptyxis_row = layout_group(
            "Title Bar",
            [
                layout_row("Background", &ptyxis_colors[0].button),
                layout_row("Text", &ptyxis_colors[1].button),
            ],
        );
        ptyxis_row.set_tooltip_text(Some("Shared Palette colors: TitlebarBackground and TitlebarForeground. Changes apply through Use Theme, not Save. Ptyxis defaults are derived from its palette, not Kitty's system/background modes."));
        let edge = gtk::DropDown::from_strings(&["Top", "Bottom"]);
        let style = gtk::DropDown::from_strings(&["Fade", "Slant", "Separator", "Powerline"]);
        let minimum = layout_spin_button(
            l.tab_min_tabs.into(),
            1,
            16,
            "Minimum Tabs",
            "1: always show; 2: show when two or more tabs exist",
        );
        let colors = [
            ColorControl::new("Active Tab Text", l.tab_active_fg),
            ColorControl::new("Active Tab Background", l.tab_active_bg),
            ColorControl::new("Inactive Tab Text", l.tab_inactive_fg),
            ColorControl::new("Inactive Tab Background", l.tab_inactive_bg),
        ];
        let tabs = layout_group(
            "Tab Bar",
            [
                layout_row("Show Tab Bar", visible),
                layout_row("Minimum Tabs", &minimum),
                layout_row("Position", &edge),
                layout_row("Style", &style),
                layout_row("Active Text", &colors[0].button),
                layout_row("Active Background", &colors[1].button),
                layout_row("Inactive Text", &colors[2].button),
                layout_row("Inactive Background", &colors[3].button),
            ],
        );
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let label = gtk::Label::builder()
            .label("Window Top")
            .xalign(0.0)
            .css_classes(["heading"])
            .build();
        root.append(&label);
        root.append(&title_row);
        root.append(&ptyxis_row);
        root.append(&title_note);
        root.append(&tabs);
        let this = Self {
            root,
            title_mode,
            title_color,
            title_row,
            title_note,
            ptyxis_row,
            ptyxis_colors,
            tabs,
            edge,
            style,
            minimum,
            colors,
            syncing: Cell::new(false),
        };
        this.set(l);
        this
    }
    pub fn set(&self, l: &LayoutSettings) {
        self.syncing.set(true);
        for c in &self.ptyxis_colors {
            c.picker.discard_draft();
        }
        self.title_mode.set_selected(match l.titlebar_color {
            TitlebarColor::System => 0,
            TitlebarColor::Background => 1,
            TitlebarColor::Custom(_) => 2,
        });
        self.title_color.set(match l.titlebar_color {
            TitlebarColor::Custom(c) => c,
            _ => [240, 240, 240],
        });
        self.edge
            .set_selected(if l.tab_edge == TabEdge::Top { 0 } else { 1 });
        self.style.set_selected(
            TabStyle::ALL
                .iter()
                .position(|s| *s == l.tab_style)
                .unwrap_or(0) as u32,
        );
        self.minimum.set_value(l.tab_min_tabs.into());
        for (control, value) in self.colors.iter().zip([
            l.tab_active_fg,
            l.tab_active_bg,
            l.tab_inactive_fg,
            l.tab_inactive_bg,
        ]) {
            control.set(value);
        }
        self.syncing.set(false);
    }
    pub fn read(&self, l: &mut LayoutSettings) {
        l.titlebar_color = match self.title_mode.selected() {
            1 => TitlebarColor::Background,
            2 => TitlebarColor::Custom(self.title_color.value.get()),
            _ => TitlebarColor::System,
        };
        l.tab_edge = if self.edge.selected() == 0 {
            TabEdge::Top
        } else {
            TabEdge::Bottom
        };
        l.tab_style = TabStyle::ALL
            .get(self.style.selected() as usize)
            .copied()
            .unwrap_or_default();
        l.tab_min_tabs = self.minimum.value_as_int().clamp(1, 16) as u8;
        [
            l.tab_active_fg,
            l.tab_active_bg,
            l.tab_inactive_fg,
            l.tab_inactive_bg,
        ] = self.colors.each_ref().map(|c| c.value.get());
    }
    pub fn commit(&self) -> Result<(), String> {
        self.minimum.update();
        for c in std::iter::once(&self.title_color).chain(&self.colors) {
            c.picker.finish_edit();
            if c.picker.has_invalid_draft() {
                return Err("Finish or correct the Window Top color before saving.".into());
            }
        }
        Ok(())
    }
    pub fn capabilities(&self, kitty: bool) {
        let supported = kitty && model::titlebar_supported();
        self.title_row.set_sensitive(supported);
        self.title_color
            .button
            .set_sensitive(self.title_mode.selected() == 2);
        // Preserve the existing preview-only switch for Ptyxis, but do not
        // suggest that its advanced Kitty controls can be applied there.
        for child in [
            self.edge.clone().upcast::<gtk::Widget>(),
            self.style.clone().upcast(),
            self.minimum.clone().upcast(),
        ] {
            child.set_sensitive(kitty);
        }
        for c in &self.colors {
            c.button.set_sensitive(kitty);
        }
        self.tabs.set_tooltip_text(Some(if kitty { "Kitty native tabs. The design sample shows two tabs; actual visibility follows Minimum Tabs." } else { "Ptyxis: visibility is preview-only. Native tab position, style and colors are not supported by this adapter." }));
        self.title_note.set_text(if supported { "Kitty · GNOME Wayland title bar. Verify with Try in Kitty." } else if kitty { "Title bar unavailable: requires Kitty on GNOME Wayland with client-side decorations. X11 and other decoration environments are not verified." } else { "Ptyxis · shared Palette header colors. Tab shape and selected-tab shading follow Adwaita; visibility below is preview-only." });
    }
    pub fn palette_draft(&self) -> bool {
        self.ptyxis_colors
            .iter()
            .any(|c| c.picker.has_invalid_draft())
    }
    pub fn finish_palette(&self) {
        for c in &self.ptyxis_colors {
            c.picker.finish_edit();
        }
    }
    pub fn discard_palette(&self) {
        for c in &self.ptyxis_colors {
            c.picker.discard_draft();
        }
    }
    fn sync_palette(&self, colors: [Rgb; 2]) {
        self.syncing.set(true);
        for (c, rgb) in self.ptyxis_colors.iter().zip(colors) {
            let value = [rgb.red(), rgb.green(), rgb.blue()];
            // Do not erase a partial HEX draft on an unrelated preview refresh.
            if c.value.get() != value {
                c.set(value);
            }
        }
        self.syncing.set(false);
    }
}

pub(super) struct TopPreview {
    pub title: gtk::Overlay,
    title_canvas: gtk::DrawingArea,
    title_label: gtk::Label,
    header_css: gtk::CssProvider,
    pub tabs: gtk::Overlay,
    canvas: gtk::DrawingArea,
    pub buttons: gtk::Box,
}
impl TopPreview {
    #[allow(deprecated)]
    fn header_colors(&self, background: Rgb, foreground: Rgb) {
        self.header_css.load_from_data(&format!(
            "box {{ background-color: {background}; color: {foreground}; }}"
        ));
        if let Some(parent) = self.title.parent() {
            parent
                .style_context()
                .add_provider(&self.header_css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    }
    fn render_ptyxis(&self, background: Rgb, foreground: Rgb, family: String, size: f64) {
        self.header_colors(background, foreground);
        self.title_canvas.set_height_request(46);
        self.title_label.set_text("Terminal");
        self.title_label
            .set_attributes(Some(&title_attributes(foreground)));
        self.title_canvas.set_draw_func(move |_, cr, _, _| {
            rgb(cr, background);
            let _ = cr.paint();
        });
        self.canvas.set_height_request(38);
        self.canvas.set_draw_func(move |_, cr, w, h| {
            rgb(cr, background);
            let _ = cr.paint();
            // Adwaita-like sample, not four independently configurable colors.
            cr.set_source_rgba(
                f64::from(foreground.red()) / 255.0,
                f64::from(foreground.green()) / 255.0,
                f64::from(foreground.blue()) / 255.0,
                0.08,
            );
            cr.rectangle(4.0, 2.0, f64::from(w) / 2.0 - 6.0, f64::from(h) - 4.0);
            let _ = cr.fill();
            rgb(cr, foreground);
            cr.select_font_face(
                &family,
                gtk::cairo::FontSlant::Normal,
                gtk::cairo::FontWeight::Normal,
            );
            cr.set_font_size(size.clamp(8.0, 18.0));
            for (x, text) in [(18.0, "bash"), (f64::from(w) / 2.0 + 14.0, "build")] {
                cr.move_to(x, f64::from(h) / 2.0 + 5.0);
                let _ = cr.show_text(text);
            }
        });
    }
    pub fn new() -> Self {
        let title_canvas = gtk::DrawingArea::builder()
            .height_request(28)
            .tooltip_text(
                "Window title bar · design approximation, not system decoration verification",
            )
            .build();
        let title = gtk::Overlay::new();
        title.set_child(Some(&title_canvas));
        let title_label = gtk::Label::builder()
            .label("Terminal")
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        title_label.add_css_class("heading");
        title_label.set_can_target(false);
        title.add_overlay(&title_label);
        let canvas = gtk::DrawingArea::builder()
            .height_request(32)
            .tooltip_text(
                "Two sample tabs · inspect to edit Window Top; verify exact rendering in Kitty",
            )
            .build();
        let tabs = gtk::Overlay::new();
        tabs.set_child(Some(&canvas));
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        buttons.set_homogeneous(true);
        tabs.add_overlay(&buttons);
        tabs.set_clip_overlay(&buttons, true);
        Self {
            title,
            title_canvas,
            title_label,
            header_css: gtk::CssProvider::new(),
            tabs,
            canvas,
            buttons,
        }
    }
    pub fn render(
        &self,
        l: LayoutSettings,
        background: Rgb,
        foreground: Rgb,
        family: String,
        size: f64,
        supported: bool,
    ) {
        let title_bg = match l.titlebar_color {
            TitlebarColor::Custom(c) if supported => Rgb::new(c[0], c[1], c[2]),
            TitlebarColor::Background if supported => background,
            _ => Rgb::new(241, 241, 241),
        };
        self.title_canvas.set_height_request(28);
        self.header_colors(
            title_bg,
            if title_bg.relative_luminance() > 0.4 {
                Rgb::new(35, 35, 35)
            } else {
                Rgb::new(240, 240, 240)
            },
        );
        self.title_label.set_text("bash — Kitty");
        self.title_label.set_attributes(Some(&title_attributes(
            if title_bg.relative_luminance() > 0.4 {
                Rgb::new(35, 35, 35)
            } else {
                Rgb::new(240, 240, 240)
            },
        )));
        self.canvas
            .set_height_request((size * 4.0 / 3.0).ceil() as i32 + 6);
        self.title_canvas.set_draw_func(move |_, cr, _, _| {
            rgb(cr, title_bg);
            let _ = cr.paint();
        });
        self.canvas.set_draw_func(move |_, cr, _, h| {
            rgb(cr, background);
            let _ = cr.paint();
            for (index, (fg, bg, label)) in [
                (l.tab_active_fg, l.tab_active_bg, "1  bash"),
                (l.tab_inactive_fg, l.tab_inactive_bg, "2  build"),
            ]
            .into_iter()
            .enumerate()
            {
                let x = 10.0 + index as f64 * 146.0;
                let height = f64::from(h);
                rgb(cr, Rgb::new(bg[0], bg[1], bg[2]));
                cr.move_to(x, 0.0);
                match l.tab_style {
                    TabStyle::Slant => {
                        cr.line_to(x + 126.0, 0.0);
                        cr.line_to(x + 144.0, height);
                        cr.line_to(x - 8.0, height);
                    }
                    TabStyle::Powerline => {
                        cr.line_to(x + 130.0, 0.0);
                        cr.line_to(x + 144.0, height / 2.0);
                        cr.line_to(x + 130.0, height);
                        cr.line_to(x, height);
                        cr.line_to(x + 12.0, height / 2.0);
                    }
                    _ => {
                        cr.line_to(x + 142.0, 0.0);
                        cr.line_to(x + 142.0, height);
                        cr.line_to(x, height);
                    }
                }
                cr.close_path();
                let _ = cr.fill();
                if l.tab_style == TabStyle::Fade {
                    let gradient = gtk::cairo::LinearGradient::new(x, 0.0, x + 142.0, 0.0);
                    for (offset, alpha) in [(0.0, 1.0), (0.16, 0.0), (0.84, 0.0), (1.0, 1.0)] {
                        gradient.add_color_stop_rgba(
                            offset,
                            f64::from(background.red()) / 255.0,
                            f64::from(background.green()) / 255.0,
                            f64::from(background.blue()) / 255.0,
                            alpha,
                        );
                    }
                    let _ = cr.set_source(&gradient);
                    cr.rectangle(x, 0.0, 142.0, height);
                    let _ = cr.fill();
                }
                rgb(cr, Rgb::new(fg[0], fg[1], fg[2]));
                cr.select_font_face(
                    &family,
                    gtk::cairo::FontSlant::Normal,
                    if index == 0 {
                        gtk::cairo::FontWeight::Bold
                    } else {
                        gtk::cairo::FontWeight::Normal
                    },
                );
                cr.set_font_size(size.clamp(8.0, 18.0));
                cr.move_to(x + 20.0, height / 2.0 + 5.0);
                let _ = cr.show_text(label);
                if l.tab_style == TabStyle::Separator {
                    rgb(cr, foreground);
                    cr.rectangle(x + 142.0, 4.0, 1.0, height - 8.0);
                    let _ = cr.fill();
                }
            }
        });
    }
}
fn title_attributes(foreground: Rgb) -> gtk::pango::AttrList {
    let attrs = gtk::pango::AttrList::new();
    attrs.insert(gtk::pango::AttrColor::new_foreground(
        u16::from(foreground.red()) * 257,
        u16::from(foreground.green()) * 257,
        u16::from(foreground.blue()) * 257,
    ));
    attrs
}
fn rgb(cr: &gtk::cairo::Context, c: Rgb) {
    cr.set_source_rgb(
        f64::from(c.red()) / 255.0,
        f64::from(c.green()) / 255.0,
        f64::from(c.blue()) / 255.0,
    );
}

impl Workbench {
    pub(super) fn connect_window_top(this: &Rc<Self>) {
        for (c, key) in this
            .window_top
            .ptyxis_colors
            .iter()
            .zip(["TitlebarBackground", "TitlebarForeground"])
        {
            let weak = Rc::downgrade(this);
            c.picker.connect_changed(move |color| {
                if let Some(this) = weak.upgrade()
                    && !this.window_top.syncing.get()
                    && this.typed.target.get() == Some(crate::design_document::TargetHint::Ptyxis)
                    && this.owns_module(EditorModule::Palette)
                {
                    this.apply_color(key, color);
                    if *this.selected_color_key.borrow() == key {
                        this.color_picker.set_color(color);
                    }
                }
            });
            let weak = Rc::downgrade(this);
            c.picker.connect_edit_begin(move || {
                if let Some(this) = weak.upgrade()
                    && !this.window_top.syncing.get()
                    && this.typed.target.get() == Some(crate::design_document::TargetHint::Ptyxis)
                    && this.owns_module(EditorModule::Palette)
                {
                    this.begin_history_edit();
                }
            });
            let weak = Rc::downgrade(this);
            c.picker.connect_edit_end(move || {
                if let Some(this) = weak.upgrade()
                    && !this.window_top.syncing.get()
                {
                    this.end_history_edit();
                }
            });
            let weak = Rc::downgrade(this);
            c.picker.connect_draft_changed(move |_| {
                if let Some(this) = weak.upgrade()
                    && !this.window_top.syncing.get()
                {
                    this.refresh_history_actions();
                }
            });
        }
        for selector in [
            &this.window_top.title_mode,
            &this.window_top.edge,
            &this.window_top.style,
        ] {
            let weak = Rc::downgrade(this);
            selector.connect_selected_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.window_top_changed();
                }
            });
        }
        let weak = Rc::downgrade(this);
        this.window_top.minimum.connect_value_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.window_top_changed();
            }
        });
        for c in std::iter::once(&this.window_top.title_color).chain(&this.window_top.colors) {
            let weak = Rc::downgrade(this);
            c.picker.connect_changed(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.window_top_changed();
                }
            });
        }
    }
    fn window_top_changed(&self) {
        if self.updating.get() || self.window_top.syncing.get() {
            return;
        }
        self.refresh_layout();
    }
    pub(super) fn refresh_window_top(&self) {
        let kitty = self.typed.target.get() == Some(crate::design_document::TargetHint::Kitty);
        let ptyxis = self.typed.target.get() == Some(crate::design_document::TargetHint::Ptyxis);
        self.top_preview.tabs.set_tooltip_text(Some(if ptyxis {
            "Ptyxis two-tab design approximation. Adwaita controls native shape and selection shading; Show Tab Bar is preview-only."
        } else { "Two sample Kitty tabs · inspect to edit Window Top; verify exact rendering in Kitty" }));
        self.top_preview.title.set_tooltip_text(Some(if ptyxis {
            "Ptyxis header · shared Palette colors. Missing optional colors use a design fallback; native default shading may differ."
        } else { "Kitty title bar · design approximation, not system decoration verification" }));
        self.window_top.capabilities(kitty);
        self.window_top.title_row.set_visible(!ptyxis);
        self.window_top.ptyxis_row.set_visible(ptyxis);
        self.window_top
            .ptyxis_row
            .set_sensitive(ptyxis && self.owns_module(EditorModule::Palette));
        let l = self.layout_settings();
        self.top_preview.title.set_visible(kitty || ptyxis);
        self.top_preview.tabs.set_visible(
            l.tab_bar
                && (ptyxis
                    && (!self.full_session.active.get()
                        || self.full_session.samples.state.borrow().tabs.len() > 1)
                    || kitty
                        && usize::from(l.tab_min_tabs)
                            <= if self.full_session.active.get() {
                                self.full_session.samples.state.borrow().tabs.len()
                            } else {
                                2
                            }),
        );
        self.top_preview
            .buttons
            .set_visible(self.full_session.active.get());
        let shell = &self.preview_terminal_shell;
        let tab_widget = self.top_preview.tabs.clone().upcast::<gtk::Widget>();
        let sibling = if kitty && l.tab_edge == TabEdge::Bottom {
            if shell.last_child().as_ref() == Some(&tab_widget) {
                self.top_preview.tabs.prev_sibling()
            } else {
                shell.last_child()
            }
        } else {
            self.top_preview.title.parent()
        };
        // A no-op reorder must remain a no-op; avoid disturbing active GIF/VTE.
        if self.top_preview.tabs.prev_sibling() != sibling {
            shell.reorder_child_after(&self.top_preview.tabs, sibling.as_ref());
        }
        let model = self.model.borrow();
        if let Some(p) = model.palette.variant(model.active_variant) {
            let font = self.typography_settings();
            if ptyxis {
                let bg = p
                    .get("TitlebarBackground")
                    .or_else(|| p.get("Background"))
                    .unwrap_or(Rgb::new(240, 240, 240));
                let fg = p
                    .get("TitlebarForeground")
                    .or_else(|| p.get("Foreground"))
                    .unwrap_or(Rgb::new(35, 35, 35));
                self.window_top.sync_palette([bg, fg]);
                self.top_preview
                    .render_ptyxis(bg, fg, font.family, font.size);
                if self.full_session.active.get() {
                    self.render_sample_top(l, bg, fg, true);
                }
                return;
            }
            self.top_preview.render(
                l,
                p.get("Background").unwrap_or(Rgb::new(0, 0, 0)),
                p.get("Foreground").unwrap_or(Rgb::new(255, 255, 255)),
                font.family,
                font.size,
                model::titlebar_supported(),
            );
            if self.full_session.active.get() {
                self.render_sample_top(
                    l,
                    p.get("Background").unwrap_or(Rgb::new(0, 0, 0)),
                    p.get("Foreground").unwrap_or(Rgb::new(255, 255, 255)),
                    false,
                );
            }
        }
    }

    fn render_sample_top(&self, l: LayoutSettings, background: Rgb, foreground: Rgb, ptyxis: bool) {
        // Transparent, accessible GTK buttons use their allocated Pango text
        // widths. Paint only the target's supported tab styling underneath.
        let state = self.full_session.samples.state.borrow();
        let active = state.active;
        let buttons = self.top_preview.buttons.downgrade();
        self.top_preview.canvas.set_draw_func(move |_, cr, _, h| {
            rgb(cr, background);
            let _ = cr.paint();
            let Some(buttons) = buttons.upgrade() else {
                return;
            };
            let mut child = buttons.first_child();
            let mut index = 0;
            while let Some(button) = child {
                let Some(bounds) = button.compute_bounds(&buttons) else {
                    break;
                };
                let x = f64::from(bounds.x());
                let width = f64::from(bounds.width());
                let height = f64::from(h);
                if ptyxis {
                    cr.set_source_rgba(
                        f64::from(foreground.red()) / 255.0,
                        f64::from(foreground.green()) / 255.0,
                        f64::from(foreground.blue()) / 255.0,
                        0.08,
                    );
                    if index == active {
                        cr.rectangle(x + 2.0, 2.0, width - 4.0, height - 4.0);
                        let _ = cr.fill();
                    }
                } else {
                    let bg = if index == active {
                        l.tab_active_bg
                    } else {
                        l.tab_inactive_bg
                    };
                    rgb(cr, Rgb::new(bg[0], bg[1], bg[2]));
                    cr.move_to(x, 0.0);
                    match l.tab_style {
                        TabStyle::Slant => {
                            cr.line_to(x + width - 10.0, 0.0);
                            cr.line_to(x + width, height);
                            cr.line_to(x, height);
                        }
                        TabStyle::Powerline => {
                            cr.line_to(x + width - 8.0, 0.0);
                            cr.line_to(x + width, height / 2.0);
                            cr.line_to(x + width - 8.0, height);
                            cr.line_to(x, height);
                        }
                        _ => {
                            cr.line_to(x + width, 0.0);
                            cr.line_to(x + width, height);
                            cr.line_to(x, height);
                        }
                    }
                    cr.close_path();
                    let _ = cr.fill();
                    if l.tab_style == TabStyle::Separator {
                        rgb(cr, foreground);
                        cr.rectangle(x + width - 1.0, 3.0, 1.0, height - 6.0);
                        let _ = cr.fill();
                    }
                    if l.tab_style == TabStyle::Fade {
                        let gradient = gtk::cairo::LinearGradient::new(x, 0.0, x + width, 0.0);
                        for (offset, alpha) in [(0.0, 1.0), (0.15, 0.0), (0.85, 0.0), (1.0, 1.0)] {
                            gradient.add_color_stop_rgba(
                                offset,
                                f64::from(background.red()) / 255.0,
                                f64::from(background.green()) / 255.0,
                                f64::from(background.blue()) / 255.0,
                                alpha,
                            );
                        }
                        let _ = cr.set_source(&gradient);
                        cr.rectangle(x, 0.0, width, height);
                        let _ = cr.fill();
                    }
                }
                child = button.next_sibling();
                index += 1;
            }
        });
        let font = self.typography_settings();
        let mut child = self.top_preview.buttons.first_child();
        let mut index = 0;
        while let Some(button) = child {
            if let Some(label) = button.first_child().and_downcast::<gtk::Label>() {
                let fg = if ptyxis {
                    [foreground.red(), foreground.green(), foreground.blue()]
                } else if index == active {
                    l.tab_active_fg
                } else {
                    l.tab_inactive_fg
                };
                let attrs = gtk::pango::AttrList::new();
                attrs.insert(gtk::pango::AttrColor::new_foreground(
                    u16::from(fg[0]) * 257,
                    u16::from(fg[1]) * 257,
                    u16::from(fg[2]) * 257,
                ));
                if !ptyxis {
                    attrs.insert(gtk::pango::AttrFontDesc::new(&font.font_description()));
                    attrs.insert(gtk::pango::AttrInt::new_weight(if index == active {
                        gtk::pango::Weight::Bold
                    } else {
                        gtk::pango::Weight::Normal
                    }));
                }
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.set_width_chars(1);
                label.set_attributes(Some(&attrs));
            }
            child = button.next_sibling();
            index += 1;
        }
    }
}
