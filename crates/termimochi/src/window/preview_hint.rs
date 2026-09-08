//! One quiet, non-interactive hint when a terminal grid needs more room.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use gtk::{glib, prelude::*};

const SETTLE_DELAY: Duration = Duration::from_millis(500);
const HINT_DURATION: Duration = Duration::from_secs(7);
const HINT_TEXT: &str = "Drag the divider or widen the window for more room";

struct FitHint {
    viewport: glib::WeakRef<gtk::ScrolledWindow>,
    revealer: glib::WeakRef<gtk::Revealer>,
    shown: Cell<bool>,
    pending: RefCell<Option<glib::SourceId>>,
    expiry: RefCell<Option<glib::SourceId>>,
}

pub(super) fn attach(
    divider: &gtk::Paned,
    viewport: &gtk::ScrolledWindow,
    content: &impl IsA<gtk::Widget>,
) -> gtk::Overlay {
    build(divider, viewport, content).0
}

fn build(
    divider: &gtk::Paned,
    viewport: &gtk::ScrolledWindow,
    content: &impl IsA<gtk::Widget>,
) -> (gtk::Overlay, Rc<FitHint>) {
    let label = gtk::Label::builder()
        .label(HINT_TEXT)
        .wrap(true)
        .max_width_chars(36)
        .xalign(0.0)
        .css_classes(["preview-fit-hint"])
        .build();
    let revealer = gtk::Revealer::builder()
        .child(&label)
        .transition_type(gtk::RevealerTransitionType::Crossfade)
        .transition_duration(180)
        .reveal_child(false)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .margin_start(18)
        .margin_end(18)
        .margin_top(54)
        // Never steal terminal input, selection, hover or divider gestures.
        .can_target(false)
        .focusable(false)
        .build();
    let root = gtk::Overlay::new();
    root.set_child(Some(content));
    root.add_overlay(&revealer);
    root.set_measure_overlay(&revealer, false);
    root.set_clip_overlay(&revealer, true);

    let hint = Rc::new(FitHint {
        viewport: viewport.downgrade(),
        revealer: revealer.downgrade(),
        shown: Cell::new(false),
        pending: RefCell::new(None),
        expiry: RefCell::new(None),
    });
    // This is the only owning callback; everything pointing back into the
    // widget tree is weak, including both timers.
    let owner = hint.clone();
    root.connect_map(move |_| owner.schedule());
    let weak = Rc::downgrade(&hint);
    root.connect_unmap(move |_| {
        if let Some(hint) = weak.upgrade() {
            hint.dismiss();
        }
    });
    let weak = Rc::downgrade(&hint);
    viewport.hadjustment().connect_changed(move |_| {
        if let Some(hint) = weak.upgrade() {
            hint.schedule();
        }
    });
    let weak = Rc::downgrade(&hint);
    viewport.hadjustment().connect_value_changed(move |_| {
        if let Some(hint) = weak.upgrade() {
            hint.dismiss();
        }
    });
    let weak = Rc::downgrade(&hint);
    divider.connect_position_notify(move |_| {
        if let Some(hint) = weak.upgrade() {
            hint.dismiss();
        }
    });
    (root, hint)
}

impl FitHint {
    fn needs_room(&self) -> bool {
        self.viewport.upgrade().is_some_and(|viewport| {
            let adjustment = viewport.hadjustment();
            viewport.is_mapped()
                && adjustment.page_size() > 0.0
                && adjustment.upper() - adjustment.lower() > adjustment.page_size() + 1.0
        })
    }

    fn schedule(self: &Rc<Self>) {
        if !self.needs_room() {
            self.dismiss();
            return;
        }
        if self.shown.get() || self.pending.borrow().is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        *self.pending.borrow_mut() = Some(glib::timeout_add_local_once(SETTLE_DELAY, move || {
            if let Some(hint) = weak.upgrade() {
                hint.pending.borrow_mut().take();
                hint.reveal();
            }
        }));
    }

    fn reveal(self: &Rc<Self>) {
        if !self.needs_room() || self.shown.get() {
            return;
        }
        let Some(revealer) = self.revealer.upgrade() else {
            return;
        };
        self.shown.set(true);
        // Crossfade changes opacity only, never the preview's allocation.
        // Explicitly respect reduced motion, including changes after launch.
        revealer.set_transition_duration(if revealer.settings().is_gtk_enable_animations() {
            180
        } else {
            0
        });
        revealer.set_reveal_child(true);
        let weak = Rc::downgrade(self);
        *self.expiry.borrow_mut() = Some(glib::timeout_add_local_once(HINT_DURATION, move || {
            if let Some(hint) = weak.upgrade() {
                hint.expiry.borrow_mut().take();
                hint.dismiss();
            }
        }));
    }

    fn dismiss(&self) {
        if let Some(timer) = self.pending.borrow_mut().take() {
            timer.remove();
        }
        if let Some(timer) = self.expiry.borrow_mut().take() {
            timer.remove();
        }
        if let Some(revealer) = self.revealer.upgrade() {
            revealer.set_reveal_child(false);
        }
    }
}

impl Drop for FitHint {
    fn drop(&mut self) {
        for timer in [self.pending.get_mut().take(), self.expiry.get_mut().take()]
            .into_iter()
            .flatten()
        {
            timer.remove();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{greeting::GreetingSettings, window::Workbench};
    use gtk::{gdk, gio};
    use vte::prelude::*;

    fn settle(duration: Duration) {
        let end = std::time::Instant::now() + duration;
        let context = glib::MainContext::default();
        while std::time::Instant::now() < end {
            while context.pending() {
                context.iteration(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn open(
        app: &adw::Application,
        path: &std::path::Path,
    ) -> (gtk::Window, Rc<Workbench>, gtk::Paned, gtk::Revealer) {
        crate::window::present_with_preset(app, None, path.to_owned());
        let window = app.active_window().unwrap();
        window.set_default_size(1120, 800);
        let this = unsafe {
            window
                .data::<Rc<Workbench>>("termimochi-workbench")
                .unwrap()
                .as_ref()
                .clone()
        };
        let paned = this
            .toast_overlay
            .child()
            .unwrap()
            .downcast::<gtk::Paned>()
            .unwrap();
        let revealer = paned
            .end_child()
            .unwrap()
            .last_child()
            .unwrap()
            .downcast::<gtk::Revealer>()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle(Duration::from_millis(50));
        }
        settle(Duration::from_millis(800));
        assert!(!revealer.reveals_child(), "fitted grids need no hint");
        (window, this, paned, revealer)
    }

    fn widen(this: &Workbench) {
        this.greeting_module_button.set_active(true);
        this.greeting.replace(
            GreetingSettings {
                enabled: true,
                preview_columns: 120,
                logo: crate::greeting::Logo::Ubuntu,
                ..Default::default()
            },
            false,
        );
    }

    fn wait_visible(revealer: &gtk::Revealer) {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !revealer.reveals_child() {
            assert!(
                std::time::Instant::now() < deadline,
                "overflow hint never appeared"
            );
            settle(Duration::from_millis(25));
        }
        settle(Duration::from_millis(220));
    }

    fn pointer(window: &gtk::Window, args: &[String], screenshot: Option<&std::ffi::OsStr>) {
        window.set_title(Some("TermiMochi point-to-edit test"));
        let mut command = std::process::Command::new("python3");
        command
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/preview-pointer-driver.py"
            ))
            .args(args);
        if let Some(path) = screenshot {
            command.env("TERMIMOCHI_INSPECT_SCREENSHOT", path);
        }
        let mut child = command.spawn().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("pointer driver timed out");
            }
            settle(Duration::from_millis(10));
        }
        assert!(child.wait().unwrap().success());
    }

    #[test]
    #[ignore = "requires GTK/VTE display; run separately with --ignored --test-threads=1"]
    fn preview_fit_hint_is_once_nonblocking_and_dismisses_on_resize_pan_or_timeout() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.PreviewHintTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(crate::typography_preset::PRESET_NAME);
        let (window, this, paned, revealer) = open(&app, &path);
        // Make room for a real leftward drag; the default editor can already
        // be at its minimum width (hence the hint's window-resize alternative).
        paned.set_position(paned.min_position() + 100);
        settle(Duration::from_millis(250));
        let layout = this.layout_settings();
        widen(&this);
        settle(Duration::from_millis(250));
        let bounds = this
            .preview_terminal_viewport
            .compute_bounds(&window)
            .unwrap();
        let focus = gtk::prelude::GtkWindowExt::focus(&window);
        wait_visible(&revealer);
        assert!(!revealer.can_target());
        assert!(!revealer.is_focusable());
        assert_eq!(
            this.preview_terminal_viewport
                .compute_bounds(&window)
                .unwrap(),
            bounds,
            "a hint must not relayout the terminal"
        );
        assert_eq!(gtk::prelude::GtkWindowExt::focus(&window), focus);
        assert_eq!(this.preview_terminal.column_count(), 120);
        assert_eq!(this.layout_settings(), layout);
        if let Some(path) = std::env::var_os("TERMIMOCHI_FIT_HINT_SCREENSHOT") {
            pointer(
                &window,
                &["capture".into(), "0".into(), "0".into()],
                Some(&path),
            );
        }
        let position = paned.position();
        if std::env::var_os("TERMIMOCHI_POINTER_TEST").is_some() {
            let start = paned
                .start_child()
                .unwrap()
                .compute_bounds(&window)
                .unwrap();
            let scale = window.scale_factor() as f32;
            let x = start.x() + start.width() + 2.0;
            let y = start.y() + start.height() / 2.0;
            let mut args = vec!["drag_to".to_owned()];
            args.extend([x, y, x - 80.0, y].map(|v| ((v * scale).round() as i32).to_string()));
            pointer(&window, &args, None);
            assert!(
                paned.position() < position,
                "real divider drag must resize the pane"
            );
        } else {
            paned.set_position(position - 80);
        }
        assert!(!revealer.reveals_child());
        assert_eq!(this.layout_settings(), layout);
        // Reflow and further edits never replay a dismissed hint in this window.
        paned.set_position(position);
        widen(&this);
        settle(Duration::from_millis(850));
        assert!(!revealer.reveals_child());
        window.destroy();

        let (window, this, _, revealer) = open(&app, &path);
        let settings = gtk::Settings::default().unwrap();
        let animations = settings.is_gtk_enable_animations();
        settings.set_gtk_enable_animations(false);
        widen(&this);
        wait_visible(&revealer);
        assert_eq!(revealer.transition_duration(), 0);
        settle(HINT_DURATION);
        assert!(
            !revealer.reveals_child(),
            "hint expires without user action"
        );
        window.destroy();
        settings.set_gtk_enable_animations(animations);

        let (window, this, _, revealer) = open(&app, &path);
        widen(&this);
        wait_visible(&revealer);
        this.preview_terminal_viewport.hadjustment().set_value(30.0);
        assert!(
            !revealer.reveals_child(),
            "panning also acknowledges the hint"
        );
        window.destroy();

        // Closing during the settle delay or while visible leaves no callbacks
        // owning the controller or updating a destroyed widget.
        let (window, this, _, _) = open(&app, &path);
        widen(&this);
        settle(Duration::from_millis(100));
        window.destroy();
        settle(Duration::from_millis(850));
        assert!(
            !temp
                .path()
                .join("greeting.termimochi-greeting.json")
                .exists()
        );
    }
}
