//! Read-only, fitted ANSI artwork. Text editing stays in its separate buffer.
use super::*;
use crate::greeting_art::Artwork;
use unicode_width::UnicodeWidthStr;

#[derive(Clone)]
struct Theme {
    font: gtk::pango::FontDescription,
    foreground: gdk::RGBA,
    background: gdk::RGBA,
    palette: Vec<gdk::RGBA>,
    line_height: f64,
    cell_width: f64,
}

pub(in crate::window) struct ArtPreview {
    pub viewport: gtk::ScrolledWindow,
    pub terminal: vte::Terminal,
    frame: gtk::Fixed,
    art: RefCell<Option<Artwork>>,
    theme: RefCell<Theme>,
    size: Cell<(i32, i32)>,
    fit: Cell<bool>,
    large: bool,
    expanded: glib::WeakRef<gtk::Window>,
}

impl ArtPreview {
    pub fn new(large: bool) -> Rc<Self> {
        let terminal = vte::Terminal::builder()
            .input_enabled(false)
            .audible_bell(false)
            .bold_is_bright(false)
            .scrollback_lines(0)
            .can_focus(false)
            .can_target(false)
            .build();
        terminal.update_property(&[gtk::accessible::Property::Label("ANSI Artwork Thumbnail")]);
        let frame = gtk::Fixed::new();
        frame.set_widget_name("termimochi-terminal");
        frame.put(&terminal, 0.0, 0.0);
        let viewport = gtk::ScrolledWindow::builder()
            .child(&frame)
            .min_content_height(if large { 200 } else { 180 })
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .hexpand(true)
            .vexpand(large)
            .css_classes(["artwork-thumbnail"])
            .build();
        // With scrollbars set to Never, GTK may ignore min_content_height
        // inside another scroller. Keep the thumbnail from collapsing to a strip.
        viewport.set_size_request(-1, if large { 200 } else { 180 });
        let this = Rc::new(Self {
            viewport,
            terminal,
            frame,
            art: RefCell::new(None),
            theme: RefCell::new(Theme {
                font: gtk::pango::FontDescription::from_string("Monospace 11"),
                foreground: rgba_from_rgb(Rgb::new(35, 38, 42)),
                background: rgba_from_rgb(Rgb::new(255, 255, 255)),
                palette: Vec::new(),
                line_height: 1.0,
                cell_width: 1.0,
            }),
            size: Cell::new((0, 0)),
            fit: Cell::new(true),
            large,
            expanded: glib::WeakRef::new(),
        });
        let weak = Rc::downgrade(&this);
        this.viewport.add_tick_callback(move |viewport, _| {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if this.size.replace((viewport.width(), viewport.height()))
                != (viewport.width(), viewport.height())
                && this.fit.get()
            {
                this.render();
            }
            glib::ControlFlow::Continue
        });
        if !large {
            this.viewport.set_cursor_from_name(Some("zoom-in"));
            this.viewport.set_tooltip_text(Some(
                "Fitted color preview · click to enlarge. Exported artwork is unchanged.",
            ));
            let click = gtk::GestureClick::new();
            click.set_button(1);
            let weak = Rc::downgrade(&this);
            click.connect_released(move |_, _, _, _| {
                if let Some(this) = weak.upgrade() {
                    this.expand();
                }
            });
            this.viewport.add_controller(click);
        }
        this
    }

    pub fn set_art(&self, art: Option<Artwork>) {
        if *self.art.borrow() == art {
            return;
        }
        self.art.replace(art);
        self.render();
    }

    pub fn set_theme(
        &self,
        typography: &TypographySettings,
        foreground: gdk::RGBA,
        background: gdk::RGBA,
        palette: Vec<gdk::RGBA>,
    ) {
        self.theme.replace(Theme {
            font: typography.font_description(),
            foreground,
            background,
            palette,
            line_height: typography.line_height,
            cell_width: typography.cell_width,
        });
        self.render();
    }

    fn render(&self) {
        let art = self.art.borrow();
        let Some(art) = art.as_ref() else {
            self.terminal.reset(true, true);
            return;
        };
        let columns = art.plain.lines().map(str::width).max().unwrap_or(1).max(1) as i64;
        let rows = art.plain.lines().count().max(1) as i64;
        let theme = self.theme.borrow();
        self.terminal.set_colors(
            Some(&theme.foreground),
            Some(&theme.background),
            &theme.palette.iter().collect::<Vec<_>>(),
        );
        self.terminal.set_cell_height_scale(theme.line_height);
        self.terminal.set_cell_width_scale(theme.cell_width);
        // GTK's requested allocation includes CSS padding/borders; VTE's
        // column count uses only the content box. At tiny fonts, two omitted
        // padding pixels would lose two columns and rewrap every row.
        #[allow(deprecated)]
        let (padding, border) = (
            self.terminal.style_context().padding(),
            self.terminal.style_context().border(),
        );
        let inset_x = i32::from(padding.left() + padding.right() + border.left() + border.right());
        let inset_y = i32::from(padding.top() + padding.bottom() + border.top() + border.bottom());
        let available = (
            self.viewport.width().max(32) - 16 - inset_x,
            self.viewport.height().max(32) - 16 - inset_y,
        );
        let mut font = theme.font.clone();
        let mut points: f64 = if self.large { 18.0 } else { 10.0 };
        for _ in 0..12 {
            if self.fit.get() {
                font.set_size((points * f64::from(gtk::pango::SCALE)) as i32);
            }
            self.terminal.set_font(Some(&font));
            let width = self.terminal.char_width().max(1) * columns;
            let height = self.terminal.char_height().max(1) * rows;
            let scale =
                (f64::from(available.0) / width as f64).min(f64::from(available.1) / height as f64);
            if !self.fit.get() || scale >= 1.0 || points <= 0.5 {
                break;
            }
            points = (points * scale * 0.92).max(0.5);
        }
        let width = (self.terminal.char_width().max(1) * columns) as i32 + inset_x;
        let height = (self.terminal.char_height().max(1) * rows) as i32 + inset_y;
        self.terminal.reset(true, true);
        self.terminal.set_size(columns, rows);
        self.terminal.set_size_request(width, height);
        self.frame.set_size_request(
            if self.fit.get() { 1 } else { width + 16 },
            if self.fit.get() { 1 } else { height + 16 },
        );
        // VTE rounds tiny fonts up to whole pixels. Scale only the final view
        // if that minimum still exceeds the available space (e.g. 160×96).
        let scale = if self.fit.get() {
            ((self.viewport.width().max(32) - 16) as f32 / width.max(1) as f32)
                .min((self.viewport.height().max(32) - 16) as f32 / height.max(1) as f32)
                .min(1.0)
        } else {
            1.0
        };
        let x = ((self.viewport.width() as f32 - width as f32 * scale) / 2.0).max(8.0);
        let y = ((self.viewport.height() as f32 - height as f32 * scale) / 2.0).max(8.0);
        self.frame.set_child_transform(
            &self.terminal,
            Some(
                &gtk::gsk::Transform::new()
                    .translate(&gtk::graphene::Point::new(x, y))
                    .scale(scale, scale),
            ),
        );
        self.terminal.feed(
            format!(
                "\x1b[0m\x1b[2J\x1b[H\x1b[?25l{}",
                art.ansi.replace('\n', "\r\n")
            )
            .as_bytes(),
        );
    }

    pub fn expand(&self) {
        if let Some(window) = self.expanded.upgrade() {
            window.present();
            return;
        }
        let Some(art) = self.art.borrow().clone() else {
            return;
        };
        let Some(parent) = self.viewport.root().and_downcast::<gtk::Window>() else {
            return;
        };
        let preview = Self::new(true);
        preview.theme.replace(self.theme.borrow().clone());
        preview.set_art(Some(art));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(16);
        content.set_margin_end(16);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.append(
            &gtk::Label::builder()
                .label("Artwork Preview")
                .hexpand(true)
                .xalign(0.0)
                .css_classes(["heading"])
                .build(),
        );
        let fit = gtk::ToggleButton::with_label("Fit");
        fit.set_active(true);
        header.append(&fit);
        let close = gtk::Button::with_label("Close");
        header.append(&close);
        content.append(&header);
        content.append(&preview.viewport);
        let window = gtk::Window::builder()
            .title("Artwork Preview")
            .transient_for(&parent)
            .modal(true)
            .destroy_with_parent(true)
            .default_width(840)
            .default_height(600)
            .child(&content)
            .build();
        window.add_css_class("termimochi-window");
        self.expanded.set(Some(&window));
        let weak = Rc::downgrade(&preview);
        fit.connect_toggled(move |fit| {
            if let Some(preview) = weak.upgrade() {
                preview.fit.set(fit.is_active());
                let policy = if fit.is_active() {
                    gtk::PolicyType::Never
                } else {
                    gtk::PolicyType::Automatic
                };
                preview.viewport.set_policy(policy, policy);
                preview.render();
            }
        });
        let weak = window.downgrade();
        close.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.close();
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = window.downgrade();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                if let Some(window) = weak.upgrade() {
                    window.close();
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(keys);
        unsafe {
            window.set_data("termimochi-artwork-preview", preview);
        }
        window.present();
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::settle;
    use super::*;
    #[test]
    #[ignore = "requires an isolated GTK/VTE display; checks full grid after fitting"]
    fn thumbnail_and_expansion_preserve_dense_text() {
        adw::init().unwrap();
        let preview = ArtPreview::new(false);
        let source = (0..71)
            .map(|n| format!("{n:03}{}", "x".repeat(157)))
            .collect::<Vec<_>>()
            .join("\n");
        let art = Artwork::parse(&source).unwrap();
        preview.set_art(Some(art.clone()));
        let window = gtk::Window::builder()
            .default_width(330)
            .default_height(180)
            .child(&preview.viewport)
            .build();
        window.present();
        settle();
        let text = preview
            .terminal
            .text_format(vte::Format::Text)
            .unwrap()
            .to_string();
        assert_eq!(
            text.trim_end(),
            art.plain.trim_end(),
            "grid {}x{}, widget {}x{}, font {:?}",
            preview.terminal.column_count(),
            preview.terminal.row_count(),
            preview.terminal.width(),
            preview.terminal.height(),
            preview.terminal.font()
        );
        assert!(preview.terminal.width() <= preview.viewport.width());
        assert!(preview.terminal.height() <= preview.viewport.height());
        preview.expand();
        settle();
        let expanded = preview.expanded.upgrade().unwrap();
        let full = unsafe {
            expanded
                .data::<Rc<ArtPreview>>("termimochi-artwork-preview")
                .unwrap()
                .as_ref()
                .clone()
        };
        let text = full
            .terminal
            .text_format(vte::Format::Text)
            .unwrap()
            .to_string();
        assert_eq!(text.trim_end(), art.plain.trim_end());
        expanded.close();
        preview.theme.borrow_mut().line_height = 2.0;
        preview.theme.borrow_mut().cell_width = 2.0;
        let art = Artwork::parse(
            &(0..96)
                .map(|n| format!("{n:03}{}", "x".repeat(157)))
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        preview.set_art(Some(art.clone()));
        settle();
        assert_eq!(
            preview
                .terminal
                .text_format(vte::Format::Text)
                .unwrap()
                .trim_end(),
            art.plain.trim_end()
        );
        let bounds = preview.terminal.compute_bounds(&preview.viewport).unwrap();
        assert!(bounds.x() >= 0.0 && bounds.y() >= 0.0);
        assert!(bounds.x() + bounds.width() <= preview.viewport.width() as f32 + 1.0);
        assert!(bounds.y() + bounds.height() <= preview.viewport.height() as f32 + 1.0);
        window.close();
        settle();
    }
}
