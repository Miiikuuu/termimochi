//! GTK adapter for the pure sample reducer. No process-launching path here.
use super::*;
use crate::interactive_samples::{Action, COMMANDS, Directory, Output, Session};
#[cfg(test)]
#[path = "interactive_samples_tests.rs"]
mod tests;

pub(super) struct Samples {
    anchors: RefCell<Vec<(u64, usize)>>,
    pub prompt_projection: RefCell<Option<PromptProjection>>,
    pub state: RefCell<Session>,
    pub tabs: gtk::Box,
    pub input_row: gtk::Box,
    pub entry: gtk::Entry,
    selector: gtk::DropDown,
    new_tab: gtk::Button,
    close_tab: gtk::Button,
    reset: gtk::Button,
    pub notice: gtk::Label,
    syncing: Cell<bool>,
    preedit: Cell<bool>,
}
pub(super) type PromptProjection = (String, usize, [Result<String, String>; 3]);
impl Samples {
    pub fn new() -> Self {
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let selector = gtk::DropDown::from_strings(&["1 · /demo"]);
        selector.set_hexpand(true);
        selector.update_property(&[gtk::accessible::Property::Label("Sample tabs")]);
        let new_tab = gtk::Button::from_icon_name("list-add-symbolic");
        new_tab.set_tooltip_text(Some("New sample tab · Ctrl+Shift+T · at most 8"));
        let close_tab = gtk::Button::from_icon_name("window-close-symbolic");
        close_tab.set_tooltip_text(Some("Close this sample tab"));
        let reset = gtk::Button::with_label("Reset samples");
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .build();
        more.set_tooltip_text(Some("Sample session controls"));
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 6);
        menu.append(&selector);
        menu.append(&close_tab);
        menu.append(&reset);
        let popover = gtk::Popover::builder().child(&menu).build();
        more.set_popover(Some(&popover));
        tabs.append(&new_tab);
        tabs.append(&more);
        let input_row = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let entry = gtk::Entry::builder()
            .max_length(256)
            .width_chars(1)
            .enable_undo(true)
            .build();
        entry.update_property(&[gtk::accessible::Property::Label("Sample command input")]);
        entry.add_css_class("sample-command");
        input_row.set_halign(gtk::Align::Start);
        input_row.set_valign(gtk::Align::Start);
        let notice = gtk::Label::builder().xalign(0.0).wrap(true).build();
        notice.set_visible(false);
        input_row.append(&entry);
        Self {
            anchors: RefCell::new(Vec::new()),
            prompt_projection: RefCell::new(None),
            state: RefCell::new(Session::default()),
            tabs,
            input_row,
            entry,
            selector,
            new_tab,
            close_tab,
            reset,
            notice,
            syncing: Cell::new(false),
            preedit: Cell::new(false),
        }
    }
}
impl Workbench {
    pub(super) fn connect_interactive_samples(this: &Rc<Self>) {
        let s = &this.full_session.samples;
        let weak = Rc::downgrade(this);
        this.preview_terminal.connect_cursor_moved(move |_| {
            if let Some(this) = weak.upgrade() {
                Self::schedule_sample_input(&this);
            }
        });
        let weak = Rc::downgrade(this);
        this.preview_terminal.connect_contents_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                Self::schedule_sample_input(&this);
            }
        });
        let weak = Rc::downgrade(this);
        this.preview_terminal
            .vadjustment()
            .unwrap()
            .connect_changed(move |_| {
                if let Some(this) = weak.upgrade() {
                    Self::schedule_sample_input(&this);
                }
            });
        let weak = Rc::downgrade(this);
        *this.preview_scroll.user_scrolled.borrow_mut() = Some(Box::new(move || {
            if let Some(this) = weak.upgrade() {
                this.save_sample_scroll();
            }
        }));
        if let Some(text) = s.entry.delegate().and_downcast::<gtk::Text>() {
            text.set_truncate_multiline(false);
            // Clipboard insertion is performed on GtkText, not its GtkEntry
            // delegate. Guard both signal sources, including native paste.
            let weak = Rc::downgrade(this);
            text.connect_insert_text(move |text, inserted, _| {
                if inserted.chars().any(char::is_control) {
                    text.stop_signal_emission_by_name("insert-text");
                    if let Some(this) = weak.upgrade() {
                        this.sample_notice("Paste rejected: use a single line without control characters. Nothing was run.");
                    }
                }
            });
            let weak = Rc::downgrade(this);
            text.connect_preedit_changed(move |_, preedit| {
                if let Some(this) = weak.upgrade() {
                    this.full_session.samples.preedit.set(!preedit.is_empty());
                }
            });
        }
        for index in 0..crate::interactive_samples::MAX_TABS {
            let button = gtk::Button::builder()
                .label("")
                .accessible_role(gtk::AccessibleRole::Tab)
                .build();
            button.add_css_class("flat");
            let css = gtk::CssProvider::new();
            css.load_from_data("button { background: transparent; border: none; box-shadow: none; border-radius: 0; padding: 4px 12px; min-height: 24px; } button:hover { outline: 1px solid alpha(currentColor,0.3); }");
            #[allow(deprecated)]
            button
                .style_context()
                .add_provider(&css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
            let weak = Rc::downgrade(this);
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    if this.inspect_button.is_active() {
                        this.inspect_preview_target(PreviewTarget::TabBar);
                    } else {
                        this.full_session
                            .samples
                            .selector
                            .set_selected(index as u32);
                    }
                }
            });
            this.top_preview.buttons.append(&button);
        }
        let weak = Rc::downgrade(this);
        s.entry.connect_changed(move |entry| {
            if let Some(this) = weak.upgrade()
                && !this.full_session.samples.syncing.get()
            {
                this.full_session
                    .samples
                    .state
                    .borrow_mut()
                    .current_mut()
                    .draft = entry.text().into();
                this.preview_scroll.follow_input();
            }
        });
        let weak = Rc::downgrade(this);
        s.entry.connect_insert_text(move |entry, text, _| {
            if text.chars().any(char::is_control) {
                entry.stop_signal_emission_by_name("insert-text");
                if let Some(this) = weak.upgrade() {
                    this.sample_notice("Paste rejected: use a single line without control characters. Nothing was run.");
                }
            }
        });
        let weak = Rc::downgrade(this);
        s.entry.connect_activate(move |entry| {
            let Some(this) = weak.upgrade() else {
                return;
            };
            let result = this
                .full_session
                .samples
                .state
                .borrow_mut()
                .current_mut()
                .submit(&entry.text());
            match result {
                Ok(()) => {
                    this.sample_notice("");
                    this.sync_sample_controls();
                    this.redraw_preview_contents();
                }
                Err(message) => this.sample_notice(message),
            }
        });
        let weak = Rc::downgrade(this);
        s.selector.connect_selected_notify(move |selector| {
            let Some(this) = weak.upgrade() else {
                return;
            };
            let s = &this.full_session.samples;
            if s.syncing.get() {
                return;
            }
            let index = selector.selected() as usize;
            if index == s.state.borrow().active {
                return;
            }
            this.save_sample_scroll();
            if index < s.state.borrow().tabs.len() {
                s.state.borrow_mut().active = index;
                this.sync_sample_controls();
                this.redraw_preview_contents();
            }
        });
        let weak = Rc::downgrade(this);
        s.new_tab.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.new_sample_tab();
            }
        });
        let weak = Rc::downgrade(this);
        s.close_tab.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.full_session.samples.state.borrow_mut().close();
                this.sync_sample_controls();
                this.redraw_preview_contents();
            }
        });
        let weak = Rc::downgrade(this);
        s.reset.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                *this.full_session.samples.state.borrow_mut() = Session::default();
                this.sync_sample_controls();
                this.redraw_preview_contents();
            }
        });
        // Leave every preedit event to GtkText. Outside composition only our
        // finite history/completion and terminal-style shortcuts are captured.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(this);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let s = &this.full_session.samples;
            if s.preedit.get() {
                return glib::Propagation::Proceed;
            }
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
            if ctrl && shift && matches!(key, gdk::Key::V | gdk::Key::v) {
                if let Some(text) = s.entry.delegate().and_downcast::<gtk::Text>() {
                    text.emit_paste_clipboard();
                }
            } else if ctrl && shift && matches!(key, gdk::Key::C | gdk::Key::c) {
                if this.preview_terminal.has_selection() {
                    this.preview_terminal
                        .copy_clipboard_format(vte::Format::Text);
                } else if let Some(text) = s.entry.delegate().and_downcast::<gtk::Text>() {
                    text.emit_copy_clipboard();
                }
            } else if ctrl && shift && matches!(key, gdk::Key::T | gdk::Key::t) {
                this.new_sample_tab();
            } else if ctrl && matches!(key, gdk::Key::l | gdk::Key::L) {
                this.sample_action(Action::Clear);
            } else if ctrl && !shift && matches!(key, gdk::Key::c | gdk::Key::C) {
                if s.entry.selection_bounds().is_some() {
                    return glib::Propagation::Proceed;
                }
                s.entry.set_text("");
                this.sample_notice("Input cancelled · no process was running.");
            } else if !ctrl && matches!(key, gdk::Key::Up | gdk::Key::Down) {
                let text = s
                    .state
                    .borrow_mut()
                    .current_mut()
                    .recall(key == gdk::Key::Up);
                s.entry.set_text(&text);
                s.entry.set_position(-1);
            } else if key == gdk::Key::Tab && !ctrl && !shift {
                let text = s.entry.text();
                let candidates: Vec<_> = COMMANDS
                    .iter()
                    .filter(|s| s.starts_with(text.as_str()))
                    .collect();
                if candidates.len() == 1 {
                    s.entry.set_text(candidates[0]);
                    s.entry.set_position(-1);
                } else {
                    this.sample_notice(
                        &candidates
                            .into_iter()
                            .copied()
                            .collect::<Vec<_>>()
                            .join(" · "),
                    );
                }
            } else {
                return glib::Propagation::Proceed;
            }
            glib::Propagation::Stop
        });
        s.entry.add_controller(keys);
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(this);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !this.full_session.active.get()
                || !this.preview_terminal.has_focus()
                || !modifiers.contains(gdk::ModifierType::CONTROL_MASK)
            {
                return glib::Propagation::Proceed;
            }
            let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
            if shift && matches!(key, gdk::Key::T | gdk::Key::t) {
                this.new_sample_tab();
            } else if matches!(key, gdk::Key::L | gdk::Key::l) {
                this.sample_action(Action::Clear);
            } else if matches!(key, gdk::Key::C | gdk::Key::c) {
                if this.preview_terminal.has_selection() {
                    this.preview_terminal
                        .copy_clipboard_format(vte::Format::Text);
                } else {
                    this.full_session.samples.entry.set_text("");
                }
            } else if shift && matches!(key, gdk::Key::V | gdk::Key::v) {
                this.full_session.samples.entry.grab_focus();
                if let Some(text) = this.focused_sample_text() {
                    text.emit_paste_clipboard();
                }
            } else {
                return glib::Propagation::Proceed;
            }
            glib::Propagation::Stop
        });
        this.preview_terminal_shell.add_controller(keys);
        this.sync_sample_controls();
    }
    pub(super) fn focused_sample_text(&self) -> Option<gtk::Text> {
        let entry = &self.full_session.samples.entry;
        gtk::prelude::GtkWindowExt::focus(&self.window())
            .filter(|focus| focus == entry.upcast_ref::<gtk::Widget>() || focus.is_ancestor(entry))
            .and_then(|_| entry.delegate().and_downcast::<gtk::Text>())
    }
    fn sample_notice(&self, message: &str) {
        self.full_session.samples.notice.set_text(message);
        self.full_session
            .samples
            .notice
            .set_visible(!message.is_empty());
    }
    pub(super) fn save_sample_scroll(&self) {
        if !self.full_session.active.get() {
            return;
        }
        let adjustment = &self.preview_scroll.adjustment;
        let mut state = self.full_session.samples.state.borrow_mut();
        let tab = state.current_mut();
        tab.scroll_row = adjustment.value() / self.preview_terminal.char_height().max(1) as f64;
        tab.follow = adjustment.value() + adjustment.page_size() >= adjustment.upper() - 2.0;
        tab.scroll_anchor = self
            .full_session
            .samples
            .anchors
            .borrow()
            .iter()
            .rev()
            .find(|(_, row)| *row <= tab.scroll_row as usize)
            .map(|(id, row)| (*id, (tab.scroll_row as usize).saturating_sub(*row)));
    }
    fn new_sample_tab(&self) {
        self.save_sample_scroll();
        if !self.full_session.samples.state.borrow_mut().add() {
            self.sample_notice("At most 8 sample tabs. Close a tab to create another.");
            return;
        }
        self.sync_sample_controls();
        self.redraw_preview_contents();
        self.full_session.samples.entry.grab_focus();
    }
    pub(super) fn sync_sample_controls(&self) {
        let s = &self.full_session.samples;
        s.syncing.set(true);
        let state = s.state.borrow();
        let labels: Vec<_> = state
            .tabs
            .iter()
            .map(|t| format!("{} · {}", t.id, t.directory.path()))
            .collect();
        let refs: Vec<_> = labels.iter().map(String::as_str).collect();
        let unchanged = s
            .selector
            .model()
            .and_downcast::<gtk::StringList>()
            .is_some_and(|list| {
                list.n_items() as usize == refs.len()
                    && refs
                        .iter()
                        .enumerate()
                        .all(|(i, value)| list.string(i as u32).as_deref() == Some(*value))
            });
        // Never replace GtkSingleSelection's model from its selected signal.
        // Besides recursive notifications, that can invalidate GTK's emitter.
        if !unchanged {
            s.selector.set_model(Some(&gtk::StringList::new(&refs)));
        }
        s.selector.set_selected(state.active as u32);
        // GtkText's undo history belongs to the currently mounted input, not
        // the preceding tab or theme. Command history remains per-tab.
        s.entry.set_enable_undo(false);
        s.entry.set_text(&state.current().draft);
        s.entry.set_enable_undo(true);
        s.entry.set_position(-1);
        let mut child = self.top_preview.buttons.first_child();
        let mut index = 0;
        while let Some(widget) = child {
            let button = widget.clone().downcast::<gtk::Button>().unwrap();
            button.set_visible(index < state.tabs.len());
            if let Some(tab) = state.tabs.get(index) {
                button.set_label(&format!("{}  {}", tab.id, tab.directory.path()));
                button.update_state(&[gtk::accessible::State::Selected(Some(
                    index == state.active,
                ))]);
            }
            child = widget.next_sibling();
            index += 1;
        }
        s.syncing.set(false);
        drop(state);
        self.refresh_window_top();
    }
    pub(super) fn sample_action(&self, action: Action) {
        self.full_session
            .samples
            .state
            .borrow_mut()
            .current_mut()
            .act(action, &action.label());
        self.redraw_preview_contents();
    }
    fn feed_sample_prompt(&self, directory: Directory) {
        if self.theme_prompt_disabled() {
            self.feed_scoped_preview(
                &format!("{} $ ", directory.path()),
                Some(PreviewTarget::Prompt),
            );
        } else if self.preview_prompt_source.get() == 1 {
            let mut context = prompt_preview_contexts()[0];
            context.path = directory.path();
            if directory == Directory::Root {
                context.git_branch = None;
                context.git_status = None;
            }
            self.feed_designed_prompt(&self.prompt_settings.borrow(), &context);
        } else {
            // Reuse the existing sandboxed/cached Starship projection, never
            // invoke a renderer from a typed command or interpolate input.
            let source = self
                .starship_editor
                .draft
                .borrow()
                .as_ref()
                .map(|d| d.contents().to_owned());
            let projection = self.full_session.samples.prompt_projection.borrow();
            let index = match directory {
                Directory::Root => 0,
                Directory::Demo => 1,
                Directory::Src => 2,
            };
            if let Some((_, _, prompts)) = projection.as_ref().filter(|(text, columns, _)| {
                Some(text) == source.as_ref()
                    && *columns == self.preview_terminal.column_count().max(12) as usize
            }) {
                match &prompts[index] {
                    Ok(text) => self.feed_scoped_preview(text, Some(PreviewTarget::PromptCopy)),
                    Err(error) => self.feed_scoped_preview(
                        &format!(
                            "Prompt preview unavailable: {}\r\n{} $ ",
                            crate::preview_context::display_text(error, 120),
                            directory.path()
                        ),
                        Some(PreviewTarget::PromptCopy),
                    ),
                }
            } else {
                self.feed_greeting_prompt();
            }
        }
    }
    pub(super) fn redraw_interactive_samples(&self) {
        let full = &self.full_session;
        let columns = self.preview_terminal.column_count().max(12) as usize;
        let state = full.samples.state.borrow();
        let tab = state.current();
        let mut images = Vec::new();
        let mut anchors = Vec::new();
        // Resolve this immutable Greeting projection once per render, not once
        // per historical fastfetch. Bound the visible canvas as well as records.
        let greeting = self.full_greeting_parts(columns);
        let greeting_text: String = greeting.0.iter().map(|(text, _)| text.as_str()).collect();
        let prompt_rows =
            super::full_session::transcript_rows(&self.greeting_prompt_ansi(), columns).max(8);
        let mut budget = 900usize.saturating_sub(prompt_rows);
        let mut visible = Vec::new();
        for block in tab.blocks.iter().rev() {
            let text = match &block.output {
                Output::Text(text) => text.as_str(),
                Output::Sample(s) => s.transcript(),
                Output::Greeting => &greeting_text,
            };
            let rows = super::full_session::transcript_rows(text, columns) + prompt_rows + 8;
            if rows > budget {
                break;
            }
            budget -= rows;
            visible.push(block);
        }
        if visible.len() < tab.blocks.len() {
            self.feed_preview(
                b"Older/oversized sample output omitted from the bounded viewport.\r\n",
            );
        }
        for block in visible.into_iter().rev() {
            let preceding: String = self
                .preview_feed
                .borrow()
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect();
            anchors.push((
                block.id,
                super::full_session::transcript_rows(&preceding, columns).saturating_sub(1),
            ));
            if !block.command.is_empty() {
                self.feed_sample_prompt(block.directory);
                self.feed_preview(block.command.as_bytes());
                self.feed_preview(b"\r\n");
            }
            match &block.output {
                Output::Text(text) => self.feed_preview(text.as_bytes()),
                Output::Sample(scenario) => {
                    self.feed_preview(b"\x1b[0mSample output only - no project tests or commands were executed.\r\n");
                    let transcript = scenario.transcript();
                    // Strip the fixture's old fixed shell heading: this tab's
                    // real virtual directory and current theme prompt lead it.
                    let transcript = if matches!(
                        scenario,
                        PreviewScenario::GitStatus | PreviewScenario::GitDiff
                    ) {
                        transcript
                            .split_once("\r\n")
                            .map_or(transcript, |(_, body)| body)
                    } else {
                        transcript
                    };
                    self.feed_preview(transcript.as_bytes());
                    self.feed_preview(b"\x1b[0m\r\n");
                }
                Output::Greeting => {
                    let preceding: String = self
                        .preview_feed
                        .borrow()
                        .iter()
                        .map(|c| c.text.as_str())
                        .collect();
                    let row =
                        super::full_session::transcript_rows(&preceding, columns).saturating_sub(1);
                    let (parts, image) = greeting.clone();
                    if let Some(mut image) = image {
                        image.row += row;
                        images.push(image);
                    }
                    for (text, part) in parts {
                        use crate::greeting::GreetingPart;
                        let scope = match part {
                            GreetingPart::Artwork => PreviewTarget::GreetingArtwork,
                            GreetingPart::Message => PreviewTarget::GreetingMessage,
                            GreetingPart::Fields => PreviewTarget::GreetingFields,
                            GreetingPart::Field(kind) => PreviewTarget::GreetingField(kind),
                        };
                        self.feed_scoped_preview(&text, Some(scope));
                    }
                }
            }
        }
        self.feed_sample_prompt(tab.directory);
        self.feed_preview(b"\x1b[?25l");
        let text: String = self
            .preview_feed
            .borrow()
            .iter()
            .map(|c| c.text.as_str())
            .collect();
        full.rows
            .set(super::full_session::transcript_rows(&text, columns));
        full.image.set(images.first().copied());
        let mut extras = full.extra_images.borrow_mut();
        while extras.len() > images.len().saturating_sub(1) {
            let (picture, _) = extras.pop().unwrap();
            full.overlay.remove_overlay(&picture);
        }
        for (index, image) in images.into_iter().skip(1).enumerate() {
            if let Some((_, old)) = extras.get_mut(index) {
                *old = image;
            } else {
                let picture = gtk::Picture::builder()
                    .content_fit(gtk::ContentFit::Fill)
                    .can_shrink(true)
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Start)
                    .build();
                picture.set_can_target(false);
                full.picture
                    .bind_property("paintable", &picture, "paintable")
                    .sync_create()
                    .build();
                full.overlay.add_overlay(&picture);
                full.overlay.set_measure_overlay(&picture, false);
                full.overlay.set_clip_overlay(&picture, true);
                extras.push((picture, image));
            }
        }
        drop(extras);
        self.refresh_terminal_geometry(&self.layout_settings());
        full.collecting.set(false);
        // Pure palette changes update VTE's color table above, not its contents.
        // Projection changes re-render records; they never re-run sample actions.
        let text_changed = *full.rendered.borrow() != text;
        let metrics = (
            self.preview_terminal.column_count(),
            self.preview_terminal.char_width(),
            self.preview_terminal.char_height(),
        );
        let geometry_changed = full.rendered_geometry.replace(metrics) != metrics;
        if text_changed {
            self.preview_terminal.reset(true, true);
            self.preview_terminal.feed(text.as_bytes());
            *full.rendered.borrow_mut() = text;
        }
        self.preview_selector.set_visible(true);
        self.prompt_preview_selector.set_visible(false);
        self.prompt_compare_selector.set_visible(false);
        self.terminal_title.set_text(&format!(
            "sample · {} · {columns} cols",
            tab.directory.path()
        ));
        if tab.follow {
            self.preview_scroll.follow_input();
        } else if text_changed || geometry_changed {
            let row = tab
                .scroll_anchor
                .and_then(|(id, offset)| {
                    anchors
                        .iter()
                        .find(|(block, _)| *block == id)
                        .map(|(_, row)| (row + offset) as f64)
                })
                .unwrap_or(tab.scroll_row);
            self.preview_scroll
                .restore_after_redraw(row * self.preview_terminal.char_height().max(1) as f64);
        }
        *full.samples.anchors.borrow_mut() = anchors;
    }
}
