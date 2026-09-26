//! Compact explicit publication in the existing background review. Worker
//! results belong to one frozen theme; no compatibility approval is fabricated.
use super::{
    Appearance,
    session::{self, Entry, Plan},
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

pub(crate) struct Publication {
    pub id: String,
    pub name: String,
    pub still_current: Rc<dyn Fn() -> bool>,
}

fn worker<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
    done: impl FnOnce(Result<T, String>) + 'static,
) {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(task());
    });
    let mut done = Some(done);
    glib::timeout_add_local(Duration::from_millis(50), move || {
        let result = match receive.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => Err("Background task stopped unexpectedly. Reopen this entry.".into()),
        };
        done.take().unwrap()(result);
        glib::ControlFlow::Break
    });
}
fn note(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .selectable(true)
        .build()
}

pub(super) fn attach(
    window: &adw::ApplicationWindow,
    content: &gtk::Box,
    trial: &gtk::Button,
    appearance: Appearance,
    publication: Publication,
    busy: Rc<Cell<bool>>,
) {
    window.set_default_height(480);
    let title = note("Keep this appearance for daily use");
    title.add_css_class("heading");
    content.append(&title);
    content.append(&note("Apply saves this appearance and adds an independent app-menu shortcut. Clean Bash opens in your home folder; personal startup/Conda are not loaded."));
    let detail = note("Preparing the exact destination and snapshot…");
    content.append(
        &gtk::Expander::builder()
            .label("Application details")
            .child(&detail)
            .build(),
    );
    let status = note("Preparing application…");
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let apply = gtk::Button::with_label("Apply & Open");
    apply.set_widget_name("background-apply-open");
    apply.add_css_class("suggested-action");
    apply.set_sensitive(false);
    let library = gtk::Button::with_label("Saved Entries…");
    buttons.append(&apply);
    buttons.append(&library);
    content.append(&buttons);
    content.append(&status);
    let plan: Rc<RefCell<Option<Plan>>> = Rc::new(RefCell::new(None));
    let applied: Rc<RefCell<Option<Entry>>> = Rc::new(RefCell::new(None));
    let data = glib::user_data_dir();
    let id = publication.id;
    let name = publication.name;
    let current = publication.still_current;
    let weak = window.downgrade();
    let p = plan.clone();
    let valid = current.clone();
    let b = apply.clone();
    let s = status.clone();
    busy.set(true);
    trial.set_sensitive(false);
    let trial = trial.clone();
    let waiting = busy.clone();
    worker(
        move || Plan::prepare(&data, &id, &name, appearance),
        move |result| {
            waiting.set(false);
            trial.set_sensitive(true);
            if !weak.upgrade().is_some_and(|w| w.is_visible()) {
                return;
            }
            if !valid() {
                s.set_text("Theme changed. Close and reopen this review; nothing was applied.");
                return;
            }
            match result {
                Ok(prepared) => {
                    detail.set_text(&prepared.details());
                    *p.borrow_mut() = Some(prepared);
                    b.set_sensitive(true);
                    s.set_text("Ready · no files changed yet.");
                }
                Err(e) => s.set_text(&e),
            }
        },
    );
    let weak = window.downgrade();
    let open = apply.clone();
    let entries = applied.clone();
    let s = status.clone();
    apply.connect_clicked(move |_| {
        if busy.get() { return; }
        if entries.borrow().is_none() && !current() { s.set_text("Theme changed. Close and reopen this review; nothing was applied."); return; }
        busy.set(true); open.set_sensitive(false);
        s.set_text("Opening saved entry…");
        let existing = entries.borrow().clone();
        let pending = if existing.is_none() { plan.borrow_mut().take() } else { None };
        let weak = weak.clone(); let busy = busy.clone(); let open = open.clone(); let s = s.clone(); let entries = entries.clone();
        worker(move || {
            let entry = if let Some(entry) = existing { entry } else { pending.ok_or("Reopen the review before applying.")?.publish()? };
            let opening = entry.launch();
            Ok((entry, opening))
        }, move |result| {
            busy.set(false);
            if !weak.upgrade().is_some_and(|w| w.is_visible()) { return; }
            match result {
                Ok((entry, opening)) => {
                    *entries.borrow_mut() = Some(entry);
                    open.set_label("Open Applied Theme"); open.set_sensitive(true);
                    s.set_text(&match opening { Ok(()) => "Applied · independent shortcut created. Opening the terminal; runtime errors appear separately. It stays open when you close the editor. Reopen from Saved Entries or your app menu.".into(), Err(e) => format!("Applied, but opening failed: {e}\nYour saved entry is retained. Retry Open Applied Theme.") });
                }
                Err(e) => s.set_text(&format!("Not applied: {e}\nClose and reopen the review to retry.")),
            }
        });
    });
    let weak = window.downgrade();
    library.connect_clicked(move |_| {
        if let Some(window) = weak.upgrade()
            && let Some(app) = window.application().and_downcast::<adw::Application>()
        {
            // Chain the library to the actual modal parent, not its sibling.
            present_library(&window, &app);
        }
    });
}

pub(crate) fn present_library(parent: &adw::ApplicationWindow, app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .transient_for(parent)
        .modal(true)
        .title("Enhanced Ptyxis · Saved Entries")
        .default_width(580)
        .default_height(460)
        .build();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
    body.set_margin_top(20);
    body.set_margin_bottom(20);
    body.set_margin_start(20);
    body.set_margin_end(20);
    body.append(&note("Independent background appearances · clean Bash. Opening does not change system Ptyxis or the current editor theme."));
    let status = note("Reading saved entries…");
    body.append(&status);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 16);
    body.append(&list);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(
        &gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&body)
            .build(),
    ));
    window.set_content(Some(&toolbar));
    let data = glib::user_data_dir();
    let weak = window.downgrade();
    worker(
        move || session::list(&data),
        move |result| {
            let Some(window) = weak.upgrade().filter(|w| w.is_visible()) else {
                return;
            };
            match result {
                Err(e) => status.set_text(&e),
                Ok((entries, issues)) => {
                    status.set_text(&if entries.is_empty() && issues.is_empty() {
                        "No applied enhanced-Ptyxis appearances yet.".into()
                    } else {
                        issues.join("\n")
                    });
                    let busy = Rc::new(Cell::new(false));
                    for entry in entries {
                        entry_row(&window, &list, entry, busy.clone());
                    }
                }
            }
        },
    );
    window.present();
}

fn entry_row(window: &adw::ApplicationWindow, list: &gtk::Box, entry: Entry, busy: Rc<Cell<bool>>) {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let title = note(entry.name());
    title.add_css_class("heading");
    body.append(&title);
    let detail = note(&entry.details());
    body.append(
        &gtk::Expander::builder()
            .label("Entry details")
            .child(&detail)
            .build(),
    );
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let open = gtk::Button::with_label("Open");
    let restore = gtk::Button::with_label(if entry.has_previous() {
        "Restore Previous…"
    } else {
        "Remove Entry…"
    });
    buttons.append(&open);
    buttons.append(&restore);
    body.append(&buttons);
    let status = note("");
    body.append(&status);
    list.append(&body);
    let entry = Rc::new(RefCell::new(entry));
    let active = entry.clone();
    let lock = busy.clone();
    let s = status.clone();
    let weak = window.downgrade();
    open.connect_clicked(move |_| {
        if lock.replace(true) {
            return;
        }
        let active = active.borrow().clone();
        let lock = lock.clone();
        let s = s.clone();
        let weak = weak.clone();
        s.set_text("Opening…");
        worker(
            move || active.launch(),
            move |r| {
                lock.set(false);
                if weak.upgrade().is_some_and(|w| w.is_visible()) {
                    s.set_text(&r.map_or_else(
                        |e| e,
                        |()| {
                            "Opening saved theme. Existing terminal windows are unaffected.".into()
                        },
                    ));
                }
            },
        );
    });
    let weak = window.downgrade();
    restore.connect_clicked(move |button| {
        if busy.get() { return; }
        let Some(window) = weak.upgrade() else { return; };
        let has_previous = entry.borrow().has_previous();
        let prompt = gtk::AlertDialog::builder().message(if has_previous { "Restore the previous appearance?" } else { "Remove this independent entry?" })
            .detail("Only this app-menu/library entry changes. Existing terminal windows keep running. Version files are retained; daily settings stay unchanged.")
            .buttons(["Cancel", if has_previous { "Restore Previous" } else { "Remove Entry" }]).cancel_button(0).default_button(0).modal(true).build();
        let current = entry.borrow().clone(); let target = entry.clone(); let busy = busy.clone(); let status = status.clone();
        let body = body.clone(); let detail = detail.clone(); let title = title.clone(); let button = button.clone();
        prompt.choose(Some(&window), gio::Cancellable::NONE, move |answer| {
            if answer != Ok(1) || busy.replace(true) { return; }
            worker(move || current.restore(), move |r| {
                busy.set(false);
                match r {
                    Ok(Some(old)) => {
                        title.set_text(old.name()); detail.set_text(&old.details());
                        button.set_label(if old.has_previous() { "Restore Previous…" } else { "Remove Entry…" });
                        *target.borrow_mut() = old; status.set_text("Previous version restored. Open to check it.");
                    }
                    Ok(None) => { body.set_sensitive(false); status.set_text("Entry removed. Version assets are retained; no terminal was stopped."); }
                    Err(e) => status.set_text(&e),
                }
            });
        });
    });
}
