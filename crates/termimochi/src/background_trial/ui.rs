use super::*;
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

struct State {
    selected: RefCell<Option<PathBuf>>,
    busy: Rc<Cell<bool>>,
    closed: Cell<bool>,
    running: RefCell<Option<(gio::Subprocess, Prepared)>>,
}

pub(crate) fn present(
    parent: Option<&adw::ApplicationWindow>,
    app: &adw::Application,
    appearance: Appearance,
) {
    present_inner(parent, app, appearance, None);
}

pub(crate) fn present_theme(
    parent: &adw::ApplicationWindow,
    app: &adw::Application,
    appearance: Appearance,
    publication: super::session_ui::Publication,
) {
    present_inner(Some(parent), app, appearance, Some(publication));
}

fn present_inner(
    parent: Option<&adw::ApplicationWindow>,
    app: &adw::Application,
    appearance: Appearance,
    publication: Option<super::session_ui::Publication>,
) {
    let from_theme = appearance.background.is_some();
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(if publication.is_some() {
            "Enhanced Ptyxis · Background"
        } else {
            "Ptyxis Background Trial · Experimental"
        })
        .default_width(540)
        .default_height(if from_theme { 320 } else { 440 })
        .build();
    if let Some(parent) = parent {
        window.set_transient_for(Some(parent));
    }
    window.set_modal(publication.is_some());
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.set_margin_top(16);
    content.set_margin_bottom(24);
    let description = gtk::Label::builder().wrap(true).xalign(0.0)
        .label(if from_theme {
            "Background, colors, font and grid from this theme.\nPrompt/Greeting and other layout settings are not included."
        } else { "Try a static image in enhanced Ptyxis.\nPrompt/Greeting are not included; this test is not saved as a theme." }).build();
    let choose = gtk::Button::with_label("Choose Background Image…");
    let file = gtk::Label::builder()
        .label("PNG, JPG or WebP · up to 16 MiB")
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .build();
    choose.set_visible(!from_theme);
    file.set_visible(!from_theme);
    let layout =
        gtk::DropDown::from_strings(&["Cover · fill the window", "Contain · show the whole image"]);
    layout.update_property(&[gtk::accessible::Property::Label("Background layout")]);
    layout.set_visible(!from_theme);
    let tint = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.05);
    tint.set_value(0.35);
    tint.set_draw_value(true);
    tint.set_digits(2);
    tint.update_property(&[gtk::accessible::Property::Label("Background color overlay")]);
    tint.set_visible(!from_theme);
    let warning = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .label("Opens a real shell with your user permissions. Daily settings stay unchanged.")
        .build();
    let details = gtk::Expander::builder()
        .label("Details · temporary session")
        .child(&gtk::Label::builder().wrap(true).xalign(0.0).label(
            "Uses private temporary settings; personal shell startup files are not loaded. This is not a security sandbox.\nTheme edits do not update an already open terminal. The prepared background scales with the window; changing its aspect ratio may stretch it.\nNo background is enabled in system Ptyxis. Opening this window does not record a compatibility pass.")
            .build())
        .build();
    let status = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .selectable(true)
        .build();
    if let Err(error) = runtime() {
        status.set_text(&error);
    }
    // The explicitly labeled click is the execution authorization. Opening
    // this review, preparing data, Save and merely selecting an image are not.
    let start = gtk::Button::with_label("Open Real Terminal");
    start.set_widget_name("background-open-terminal");
    start.add_css_class("suggested-action");
    let stop = gtk::Button::with_label("Stop Trial…");
    stop.set_visible(false);
    stop.set_sensitive(false);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.append(&start);
    buttons.append(&stop);
    let overlay_label = gtk::Label::new(Some("Background color overlay"));
    overlay_label.set_visible(!from_theme);
    for widget in [
        description.upcast::<gtk::Widget>(),
        choose.clone().upcast(),
        file.clone().upcast(),
        layout.clone().upcast(),
        overlay_label.upcast(),
        tint.clone().upcast(),
        warning.upcast(),
        details.upcast(),
        buttons.upcast(),
        status.clone().upcast(),
    ] {
        content.append(&widget);
    }
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&content));
    window.set_content(Some(&toolbar));
    let state = Rc::new(State {
        selected: RefCell::new(None),
        busy: Rc::new(Cell::new(false)),
        closed: Cell::new(false),
        running: RefCell::new(None),
    });
    if let Some(publication) = publication {
        super::session_ui::attach(
            &window,
            &content,
            &start,
            appearance.clone(),
            publication,
            state.busy.clone(),
        );
    }
    let weak_window = window.downgrade();
    let selected = state.clone();
    choose.connect_clicked(move |_| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        let picker = gtk::FileDialog::builder()
            .title("Choose a static background")
            .build();
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Static images"));
        for mime in ["image/png", "image/jpeg", "image/webp"] {
            filter.add_mime_type(mime);
        }
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        picker.set_filters(Some(&filters));
        let selected = selected.clone();
        let file = file.clone();
        picker.open(Some(&window), gio::Cancellable::NONE, move |result| {
            if selected.closed.get() {
                return;
            }
            if let Ok(chosen) = result
                && let Some(path) = chosen.path()
            {
                file.set_text(&path.to_string_lossy());
                *selected.selected.borrow_mut() = Some(path);
            }
        });
    });
    let s = state.clone();
    let weak_window = window.downgrade();
    let stop_button = stop.clone();
    let start_button = start.clone();
    start.connect_clicked(move |_| {
        if s.closed.get() || s.busy.get() || s.running.borrow().is_some() {
            return;
        }
        let path = s.selected.borrow().clone();
        if !from_theme && path.is_none() {
            status.set_text("Choose a background image first.");
            return;
        }
        s.busy.set(true);
        start_button.set_sensitive(false);
        choose.set_sensitive(false);
        layout.set_sensitive(false);
        tint.set_sensitive(false);
        status.set_text("Preparing a bounded static image…");
        let (send, receive) = mpsc::channel();
        let appearance = appearance.clone();
        let contain = layout.selected() == 1;
        let amount = tint.value();
        std::thread::spawn(move || {
            let _ = send.send(if from_theme {
                prepare_current(appearance)
            } else {
                prepare(
                    path.as_deref().expect("selected image"),
                    contain,
                    amount,
                    appearance,
                )
            });
        });
        let s = s.clone();
        let status = status.clone();
        let start = start_button.clone();
        let stop = stop_button.clone();
        let choose = choose.clone();
        let layout = layout.clone();
        let tint = tint.clone();
        let weak_window = weak_window.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            let result = match receive.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("Background preparation stopped unexpectedly.".into()),
            };
            s.busy.set(false);
            if s.closed.get() {
                return glib::ControlFlow::Break;
            }
            let started =
                result.and_then(|prepared| prepared.start().map(|child| (child, prepared)));
            match started {
                Ok((child, prepared)) => {
                    status.set_text("Terminal opened. Close it normally when finished.");
                    stop.set_visible(true);
                    stop.set_sensitive(true);
                    *s.running.borrow_mut() = Some((child.clone(), prepared));
                    let s = s.clone();
                    let status = status.clone();
                    let start = start.clone();
                    let stop = stop.clone();
                    let choose = choose.clone();
                    let layout = layout.clone();
                    let tint = tint.clone();
                    let weak_window = weak_window.clone();
                    child.wait_async(gio::Cancellable::NONE, move |result| {
                        let succeeded = s
                            .running
                            .borrow()
                            .as_ref()
                            .is_some_and(|(p, _)| p.is_successful());
                        s.running.borrow_mut().take(); // supervisor reaped descendants; release temp assets
                        stop.set_sensitive(false);
                        stop.set_visible(false);
                        start.set_sensitive(true);
                        choose.set_sensitive(true);
                        layout.set_sensitive(true);
                        tint.set_sensitive(true);
                        status.set_text(if result.is_ok() && succeeded {
                            "Terminal closed. Temporary session cleaned up."
                        } else {
                            "Terminal exited with an error. Daily settings are unchanged."
                        });
                        if s.closed.get()
                            && let Some(window) = weak_window.upgrade()
                        {
                            window.close();
                        }
                    });
                }
                Err(error) => {
                    status.set_text(&error);
                    start.set_sensitive(true);
                    choose.set_sensitive(true);
                    layout.set_sensitive(true);
                    tint.set_sensitive(true);
                }
            }
            glib::ControlFlow::Break
        });
    });
    let weak_window = window.downgrade();
    let s = state.clone();
    stop.connect_clicked(move |_| {
        if let Some(window) = weak_window.upgrade() {
            confirm_stop(&window, &s, false);
        }
    });
    let s = state.clone();
    window.connect_close_request(move |window| {
        if s.running.borrow().is_some() {
            confirm_stop(window, &s, true);
            glib::Propagation::Stop
        } else {
            s.closed.set(true);
            glib::Propagation::Proceed
        }
    });
    window.present();
}

fn confirm_stop(window: &adw::ApplicationWindow, state: &Rc<State>, close: bool) {
    let dialog = adw::MessageDialog::new(
        Some(window),
        Some("Stop the trial terminal?"),
        Some("This ends the trial shell and its programs. Your normal terminal is unaffected."),
    );
    dialog.add_responses(&[("cancel", "Keep Running"), ("stop", "Stop Trial")]);
    dialog.set_response_appearance("stop", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let state = state.clone();
    dialog.connect_response(None, move |_, response| {
        if response == "stop" {
            if close {
                state.closed.set(true);
            }
            if let Some((child, _)) = state.running.borrow().as_ref() {
                child.send_signal(15);
            }
        }
    });
    dialog.present();
}
