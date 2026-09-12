//! Native observation is independent of both editor modules and design scenes.
use super::*;
use crate::native_preview::{self, Session, Snapshot};
use std::sync::Arc;

#[cfg(test)]
#[path = "native_terminal_tests.rs"]
mod tests;

pub(super) struct NativePane {
    pub stack: gtk::Stack,
    pub mode: gtk::ToggleButton,
    title: gtk::Label,
    root: gtk::Box,
    start: gtk::Button,
    stop: gtk::Button,
    resync: gtk::Button,
    expand: gtk::ToggleButton,
    status: gtk::Label,
    running: RefCell<Option<Running>>,
    busy: Cell<bool>,
    accepted: Cell<bool>,
    epoch: Cell<u64>,
    revision: Cell<u64>,
    last: RefCell<Option<Snapshot>>,
    pub dirty: Cell<bool>,
    checked_at: Cell<std::time::Instant>,
}
struct Running {
    pid: glib::Pid,
    session: Arc<Session>,
    host: gtk::Widget,
}
impl Drop for Running {
    fn drop(&mut self) {
        // This is our unreaped direct child; its PID cannot yet be reused.
        if self.pid.0 > 0 {
            unsafe {
                libc::kill(self.pid.0, libc::SIGTERM);
            }
        }
    }
}
impl NativePane {
    pub fn new(design: &gtk::Overlay, title: &gtk::Label) -> Self {
        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.add_named(design, Some("design"));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let start = gtk::Button::with_label("Start native terminal");
        let stop = gtk::Button::with_label("Stop…");
        let resync = gtk::Button::with_label("Resync theme");
        let expand = gtk::ToggleButton::with_label("Expand");
        let weak_stack = stack.downgrade();
        expand.connect_toggled(move |button| {
            if let Some(stack) = weak_stack.upgrade()
                && let Some(paned) = stack
                    .ancestor(gtk::Paned::static_type())
                    .and_downcast::<gtk::Paned>()
                && let Some(editor) = paned.start_child()
            {
                editor.set_visible(!button.is_active());
                button.set_label(if button.is_active() {
                    "Show editor"
                } else {
                    "Expand"
                });
            }
        });
        for button in [&start, &stop, &resync] {
            row.append(button);
        }
        root.append(&row);
        row.append(&expand);
        let status = gtk::Label::builder().label("Experimental · actual commands execute · private configuration, not a filesystem sandbox.").wrap(true).xalign(0.0).build();
        root.append(&status);
        stack.add_named(&root, Some("native"));
        stack.set_visible_child_name("design");
        let mode = gtk::ToggleButton::with_label("Real terminal");
        mode.set_tooltip_text(Some("Observe the current theme in its actual native target. Design scenes remain available."));
        Self {
            stack,
            mode,
            title: title.clone(),
            root,
            start,
            stop,
            resync,
            expand,
            status,
            running: RefCell::new(None),
            busy: Cell::new(false),
            accepted: Cell::new(false),
            epoch: Cell::new(0),
            revision: Cell::new(0),
            last: RefCell::new(None),
            dirty: Cell::new(true),
            checked_at: Cell::new(std::time::Instant::now()),
        }
    }
}

impl Workbench {
    fn native_snapshot(&self) -> Result<Snapshot, String> {
        if self.has_draft()
            || self.starship_editor.invalid()
            || self.greeting.invalid.get()
            || self.greeting.presentation_pending()
        {
            return Err("An edit is incomplete; keeping the last valid native appearance.".into());
        }
        let design = self.design_snapshot()?;
        let mut workspace = self.workspace_snapshot();
        if let Some(greeting) = design.greeting_output() {
            workspace.greeting = greeting;
        }
        Ok(Snapshot { design, workspace })
    }
    pub(super) fn connect_native_terminal(this: &Rc<Self>) {
        let weak = Rc::downgrade(this);
        this.native_terminal.mode.connect_toggled(move |button| {
            let Some(this) = weak.upgrade() else {
                return;
            };
            let native = button.is_active();
            if !native {
                this.native_terminal.expand.set_active(false);
            }
            this.native_terminal.title.set_text(if native {
                "Native Preview · Experimental"
            } else if this.native_is_running() {
                "Design Preview · native session running"
            } else {
                "Design Preview"
            });
            this.native_terminal
                .stack
                .set_visible_child_name(if native { "native" } else { "design" });
            if native {
                this.inspect_button.set_active(false);
            }
            this.inspect_button.set_sensitive(!native);
            this.preview_scene_selector.set_visible(!native);
            this.full_session
                .toolbar
                .set_visible(!native && this.full_session.active.get());
            button.set_label(if native {
                "Return to design"
            } else {
                "Real terminal"
            });
        });
        let weak = Rc::downgrade(this);
        this.native_terminal.start.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.request_native_start();
            }
        });
        let weak = Rc::downgrade(this);
        this.native_terminal.stop.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.request_native_stop(false);
            }
        });
        let weak = Rc::downgrade(this);
        this.native_terminal.resync.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                *this.native_terminal.last.borrow_mut() = None;
                this.native_terminal.dirty.set(true);
            }
        });
        // Run before application accelerators, forwarding the original event,
        // including IM filtering, not translating it to terminal text.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(this);
        keys.connect_key_pressed(move |controller, key, _, modifiers| {
            if let Some(this) = weak.upgrade()
                && let Some(running) = this.native_terminal.running.borrow().as_ref()
                && running.host.has_focus()
            {
                if key==gdk::Key::F12 && modifiers.contains(gdk::ModifierType::CONTROL_MASK|gdk::ModifierType::ALT_MASK|gdk::ModifierType::SHIFT_MASK) {
                    this.native_terminal.mode.set_active(false);
                    this.palette_module_button.grab_focus();
                    return glib::Propagation::Stop;
                }
                if modifiers.contains(gdk::ModifierType::CONTROL_MASK|gdk::ModifierType::SHIFT_MASK)
                    && matches!(key,gdk::Key::plus|gdk::Key::equal|gdk::Key::minus|gdk::Key::BackSpace) {
                    this.native_terminal.status.set_text("Native font shortcut: preview may have a temporary difference. Resync theme restores its appearance; the saved theme is unchanged.");
                }
                controller.forward(&running.host);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(this);
        keys.connect_key_released(move |controller, _, _, _| {
            if let Some(this) = weak.upgrade()
                && let Some(running) = this.native_terminal.running.borrow().as_ref()
                && running.host.has_focus()
            {
                controller.forward(&running.host);
            }
        });
        this.window().add_controller(keys);
        let weak = Rc::downgrade(this);
        glib::timeout_add_local(Duration::from_millis(150), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            this.native_tick();
            glib::ControlFlow::Continue
        });
    }
    fn request_native_start(self: &Rc<Self>) {
        if self.native_terminal.busy.get() || self.native_terminal.running.borrow().is_some() {
            return;
        }
        if self.native_terminal.accepted.get() {
            self.start_native_terminal();
            return;
        }
        let dialog = gtk::AlertDialog::builder().message("Start an actual terminal?")
            .detail("Commands you type will execute. This terminal uses private configuration and controlled Bash without personal startup files, but is NOT a filesystem sandbox. Native preferences are temporary and do not edit your theme. GPU/IME behavior outside the tested environment remains experimental.")
            .buttons(["Cancel", "Start native terminal"]).cancel_button(0).default_button(0).build();
        let weak = Rc::downgrade(self);
        dialog.choose(Some(&self.window()), gio::Cancellable::NONE, move |r| {
            if r == Ok(1)
                && let Some(this) = weak.upgrade()
            {
                this.native_terminal.accepted.set(true);
                this.start_native_terminal();
            }
        });
    }
    fn start_native_terminal(self: &Rc<Self>) {
        let snapshot = match self.native_snapshot() {
            Ok(s) => s,
            Err(e) => {
                self.native_terminal.status.set_text(&e);
                return;
            }
        };
        let pane = &self.native_terminal;
        if pane.busy.replace(true) {
            return;
        }
        let epoch = pane.epoch.get().wrapping_add(1);
        pane.epoch.set(epoch);
        pane.status
            .set_text("Preparing private terminal configuration…");
        let (send, receive) = mpsc::sync_channel(1);
        let expected = snapshot.clone();
        std::thread::spawn(move || {
            let _ = send.send(Session::prepare(snapshot));
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(30), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let result = match receive.try_recv() {
                Ok(r) => r,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("Native preparation stopped.".into()),
            };
            let pane = &this.native_terminal;
            pane.busy.set(false);
            if pane.epoch.get() != epoch || this.native_snapshot().as_ref().ok() != Some(&expected)
            {
                pane.status.set_text(
                    "Theme changed while preparing. Start again; no stale terminal was opened.",
                );
                return glib::ControlFlow::Break;
            }
            match result {
                Ok(session) => {
                    let host = native_preview::host();
                    host.set_hexpand(true);
                    host.set_vexpand(true);
                    host.set_size_request(200, 240);
                    pane.root.append(&host);
                    match native_preview::spawn(&host, &session) {
                        Ok(pid) => {
                            pane.status.set_text("Actual native shell · private settings · Prompt/Greeting changes require Stop / Start.");
                            *pane.last.borrow_mut() = Some(expected.clone());
                            *pane.running.borrow_mut() = Some(Running {
                                pid,
                                session: session.clone(),
                                host: host.clone(),
                            });
                            // Keep assets until the supervisor reaps all descendants.
                            let weak = Rc::downgrade(&this);
                            glib::child_watch_add_local(pid, move |_, status| {
                                let _keep = &session;
                                if let Some(this) = weak.upgrade() {
                                    let mut running = this.native_terminal.running.borrow_mut();
                                    if running.as_ref().is_some_and(|r| r.pid == pid) {
                                        // Child already reaped; do not signal a recycled PID.
                                        let mut done = running.take().unwrap();
                                        done.pid = glib::Pid(0);
                                        this.native_terminal.root.remove(&done.host);
                                        this.native_terminal.status.set_text(if status==0 {
                                            "Native session ended. Start opens a new controlled shell."
                                        } else {
                                            "Native terminal exited unexpectedly. Owned processes were stopped; the theme is retained. Start opens a new controlled shell."
                                        });
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            pane.root.remove(&host);
                            pane.status.set_text(&e);
                        }
                    }
                }
                Err(e) => pane.status.set_text(&e),
            }
            glib::ControlFlow::Break
        });
    }
    fn native_tick(self: &Rc<Self>) {
        let pane = &self.native_terminal;
        let running = pane.running.borrow();
        if !pane.mode.is_active() {
            pane.title.set_text(if running.is_some() {
                "Design Preview · native session running"
            } else {
                "Design Preview"
            });
        }
        pane.start
            .set_sensitive(!pane.busy.get() && running.is_none());
        pane.stop.set_sensitive(running.is_some());
        pane.resync
            .set_sensitive(running.is_some() && !pane.busy.get());
        let Some(running) = running.as_ref() else {
            return;
        };
        if pane.mode.is_active() {
            self.full_session.toolbar.set_visible(false);
        }
        if pane.busy.get() {
            return;
        }
        if *self.typed.document_id.borrow() != running.session.start.design.id
            || self.typed.target.get() != running.session.start.design.target_hint
        {
            pane.status.set_text("This running terminal belongs to the previous theme. Stop it before starting this theme; no settings cross between themes.");
            return;
        }
        if !pane.dirty.replace(false) {
            if pane.checked_at.get().elapsed() >= Duration::from_secs(2)
                && running.session.start.target().ok()
                    == Some(crate::design_document::TargetHint::Ptyxis)
            {
                pane.checked_at.set(std::time::Instant::now());
                pane.busy.set(true);
                let session = running.session.clone();
                let epoch = pane.epoch.get();
                let identity = self.typed.identity.get();
                let (send, receive) = mpsc::sync_channel(1);
                std::thread::spawn(move || {
                    let _ = send.send(session.temporary_difference());
                });
                let weak = Rc::downgrade(self);
                glib::timeout_add_local(Duration::from_millis(30), move || {
                    let Some(this) = weak.upgrade() else {
                        return glib::ControlFlow::Break;
                    };
                    let result = match receive.try_recv() {
                        Ok(r) => r,
                        Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                        Err(_) => Err("Private settings observation stopped.".into()),
                    };
                    let pane = &this.native_terminal;
                    pane.busy.set(false);
                    if pane.epoch.get() == epoch
                        && !pane.dirty.get()
                        && this.typed.identity.get() == identity
                    {
                        match result {
                            Ok(true)=>pane.status.set_text("Preview has temporary native settings. Resync theme restores the managed appearance; your theme and other terminals are unchanged."),
                            Err(e)=>pane.status.set_text(&e),
                            Ok(false)=>(),
                        }
                    }
                    glib::ControlFlow::Break
                });
            }
            return;
        }
        let snapshot = match self.native_snapshot() {
            Ok(s) => s,
            Err(e) => {
                pane.status.set_text(&e);
                return;
            }
        };
        if snapshot.design.id != running.session.start.design.id
            || snapshot.design.target_hint != running.session.start.design.target_hint
        {
            pane.status.set_text("This running terminal belongs to the previous theme. Stop it before starting this theme; no settings cross between themes.");
            return;
        }
        if pane.last.borrow().as_ref() == Some(&snapshot) {
            return;
        }
        *pane.last.borrow_mut() = Some(snapshot.clone());
        pane.busy.set(true);
        let epoch = pane.epoch.get();
        let revision = pane.revision.get().wrapping_add(1);
        pane.revision.set(revision);
        let session = running.session.clone();
        let expected = snapshot.clone();
        let (send, receive) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = send.send(session.apply(&snapshot, true));
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(30), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let result = match receive.try_recv() {
                Ok(r) => r,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("Native sync worker stopped.".into()),
            };
            let pane = &this.native_terminal;
            pane.busy.set(false);
            if pane.epoch.get() == epoch
                && pane.revision.get() == revision
                && this.native_snapshot().as_ref().ok() == Some(&expected)
            {
                pane.status.set_text(&result.unwrap_or_else(|e| e));
            }
            glib::ControlFlow::Break
        });
    }
    pub(super) fn request_native_stop(self: &Rc<Self>, close_after: bool) {
        if self.native_terminal.running.borrow().is_none() {
            return;
        }
        let dialog = gtk::AlertDialog::builder().message("Stop the running native terminal?")
            .detail("All tabs and programs in this preview will be terminated. Other terminal windows are not affected.")
            .buttons(["Keep running", "Stop preview"]).cancel_button(0).default_button(0).build();
        let weak = Rc::downgrade(self);
        dialog.choose(Some(&self.window()), gio::Cancellable::NONE, move |r| {
            if r != Ok(1) {
                return;
            }
            let Some(this) = weak.upgrade() else {
                return;
            };
            let pane = &this.native_terminal;
            pane.epoch.set(pane.epoch.get().wrapping_add(1));
            if let Some(running) = pane.running.borrow().as_ref() {
                unsafe {
                    libc::kill(running.pid.0, libc::SIGTERM);
                }
            }
            pane.status.set_text("Stopping owned processes…");
            if close_after {
                let weak = Rc::downgrade(&this);
                glib::timeout_add_local(Duration::from_millis(50), move || {
                    let Some(this) = weak.upgrade() else {
                        return glib::ControlFlow::Break;
                    };
                    if this.native_terminal.running.borrow().is_some() {
                        return glib::ControlFlow::Continue;
                    }
                    this.window().close();
                    glib::ControlFlow::Break
                });
            }
        });
    }
    pub(super) fn native_is_running(&self) -> bool {
        self.native_terminal.running.borrow().is_some()
    }
}
