//! Local target selection for single-component documents. A picked native file
//! is a conflict-checked local binding, never portable overwrite permission.
use super::*;
use crate::design_document::{Kind, TargetHint};

pub(super) struct DocumentUseBinding {
    pub shared_review: Cell<bool>,
    pub starship: RefCell<Option<crate::starship_file::FileSnapshot>>,
}

impl DocumentUseBinding {
    pub fn new() -> Self {
        Self {
            shared_review: Cell::new(false),
            starship: RefCell::new(None),
        }
    }

    pub fn reset(&self) {
        self.shared_review.set(false);
        self.starship.borrow_mut().take();
    }
}

impl Workbench {
    pub(super) fn choose_document_use_target(self: &Rc<Self>) {
        let kind = self.typed.kind.get();
        if !matches!(
            kind,
            Kind::Prompt | Kind::Greeting | Kind::Artwork | Kind::Typography | Kind::Layout
        ) {
            self.toast("This document needs its own supported target adapter. Convert an explicit project copy to choose a project target.");
            return;
        }
        let identity = self.typed.identity.get();
        let (window, body, buttons) = super::scheme::dialog(
            &self.window(),
            "Use This Document",
            "Choose where this document will be used. Preview reference colors, fonts and other components are not part of the write set.",
        );
        let note = gtk::Label::builder().wrap(true).xalign(0.0).label(
            "An independent Kitty entry leaves daily terminal and shell files untouched. Its controlled Bash does not load personal aliases or startup integrations. You can reopen a published entry from TermiMochi.").build();
        body.append(&note);
        let kitty = gtk::Button::with_label("Create Independent Kitty Entry…");
        kitty.set_widget_name("document-use-kitty");
        body.append(&kitty);
        let weak = Rc::downgrade(self);
        let parent = window.downgrade();
        kitty.connect_clicked(move |_| {
            let Some(this) = weak
                .upgrade()
                .filter(|this| this.typed.identity.get() == identity)
            else {
                return;
            };
            if let Some(parent) = parent.upgrade() {
                parent.destroy();
            }
            this.document_use.reset();
            this.typed.target.set(Some(TargetHint::Kitty));
            this.greeting.presentation.verified.borrow_mut().take();
            this.refresh_document_scope();
            this.use_kitty_design();
        });

        if matches!(kind, Kind::Typography | Kind::Layout) {
            let note = gtk::Label::builder().wrap(true).xalign(0.0).label(if kind == Kind::Typography {
                "Ptyxis font family, size and weight affect all Ptyxis windows. Supported spacing applies to the reviewed profile. The next page discloses the exact scope; Palette, Prompt and Greeting are not included."
            } else {
                "Use the supported Layout settings with Ptyxis. Global and profile-specific effects are disclosed in the next review. Exact padding, tab bar and window spacing may be preview-only; other document components are not included."
            }).build();
            body.append(&note);
            let ptyxis = gtk::Button::with_label("Use with Ptyxis…");
            ptyxis.set_widget_name("document-use-ptyxis");
            body.append(&ptyxis);
            let weak = Rc::downgrade(self);
            let parent = window.downgrade();
            ptyxis.connect_clicked(move |_| {
                let Some(this) = weak
                    .upgrade()
                    .filter(|this| this.typed.identity.get() == identity)
                else {
                    return;
                };
                if let Some(parent) = parent.upgrade() {
                    parent.destroy();
                }
                this.document_use.reset();
                this.typed.target.set(Some(TargetHint::Ptyxis));
                this.refresh_document_scope();
                this.request_scheme_apply();
            });
        }

        if matches!(kind, Kind::Prompt | Kind::Greeting) {
            let description = gtk::Label::builder().wrap(true).xalign(0.0).label(
                "Or update a specific existing native configuration. This can affect every shell already using that file; it does not install a startup hook or execute imported commands. The exact file and full changes are reviewed before writing.").build();
            body.append(&description);
            let shared = gtk::Button::with_label(if kind == Kind::Prompt {
                "Choose Starship Configuration…"
            } else {
                "Choose Fastfetch Configuration…"
            });
            shared.set_widget_name("document-use-native-file");
            body.append(&shared);
            let selected = gtk::Label::builder()
                .wrap(true)
                .xalign(0.0)
                .selectable(true)
                .label("No native file selected. Choosing a file does not modify it.")
                .build();
            body.append(&selected);
            // Same explicit order as PresentationEditor.target; never use
            // Terminal::ALL (whose first two entries have the opposite order).
            let terminal = gtk::DropDown::from_strings(&["Ptyxis", "Kitty", "Xterm"]);
            terminal.set_widget_name("document-use-terminal");
            terminal.set_selected(match self.greeting.presentation.binding.borrow().terminal {
                crate::pixel_trial::Terminal::Ptyxis => 0,
                crate::pixel_trial::Terminal::Kitty => 1,
                crate::pixel_trial::Terminal::Xterm => 2,
            });
            terminal.set_visible(kind == Kind::Greeting);
            terminal.update_property(&[gtk::accessible::Property::Label(
                "Native Greeting test terminal",
            )]);
            body.append(&terminal);
            let pixels = gtk::CheckButton::with_label(
                "Allow images in this selected shared configuration (other terminals may show blank output)",
            );
            pixels.set_visible(kind == Kind::Greeting);
            body.append(&pixels);
            let trial = gtk::Button::with_label("Try Greeting Now…");
            trial.set_widget_name("document-use-trial");
            trial.set_visible(kind == Kind::Greeting);
            body.append(&trial);
            let review = gtk::Button::with_label("Review Native Update…");
            review.set_widget_name("document-use-review");
            buttons.append(&review);

            let weak = Rc::downgrade(self);
            let parent = window.downgrade();
            let message = selected.clone();
            shared.connect_clicked(move |_| {
                let Some(this) = weak.upgrade().filter(|this| this.typed.identity.get() == identity) else { return; };
                let Some(parent) = parent.upgrade() else { return; };
                let chooser = gtk::FileDialog::builder().title(if kind == Kind::Prompt { "Choose Existing Starship .toml" } else { "Choose Existing Fastfetch config.jsonc / config.json" }).modal(true).build();
                let weak = Rc::downgrade(&this);
                let message = message.clone();
                chooser.open(Some(&parent), gio::Cancellable::NONE, move |result| {
                    let Some(this) = weak.upgrade().filter(|this| this.typed.identity.get() == identity) else { return; };
                    let Ok(file) = result else { return; };
                    let result = file.path().ok_or("Choose a local native configuration.".to_owned()).and_then(|path| {
                        if kind == Kind::Prompt {
                            let snapshot = crate::starship_file::FileSnapshot::read(&path)?;
                            crate::starship_draft::StarshipDraft::new(path.clone(), snapshot.contents.clone())?;
                            if path.extension().is_none_or(|e| !e.eq_ignore_ascii_case("toml")) { return Err("Choose a Starship .toml configuration, not a shell startup file.".into()); }
                            *this.document_use.starship.borrow_mut() = Some(snapshot);
                        } else {
                            let target = crate::fastfetch_apply::Target::open(path.clone())?;
                            crate::fastfetch_document::parse(&target.source()?)?;
                            *this.greeting.fastfetch_target.borrow_mut() = Some(target);
                            this.greeting.invalidate_output_checks();
                            this.greeting.presentation.verified.borrow_mut().take();
                        }
                        this.typed.target.set(None);
                        this.document_use.shared_review.set(true);
                        Ok(path)
                    });
                    match result {
                        Ok(path) => message.set_text(&format!("Selected native target: {}\nOnly this document's owned content will enter the next review. Shared scope; existing shells may need a new prompt or explicit Fastfetch run.", path.display())),
                        Err(error) => { this.document_use.shared_review.set(false); message.set_text(&format!("Cannot use this file: {error}\nChoose another file or create an independent Kitty entry.")); }
                    }
                });
            });

            let weak = Rc::downgrade(self);
            let parent = window.downgrade();
            let terminal_for_trial = terminal.clone();
            let message = selected.clone();
            trial.connect_clicked(move |_| {
                let Some(this) = weak.upgrade().filter(|this| this.typed.identity.get() == identity) else { return; };
                this.greeting.presentation.target.set_selected(terminal_for_trial.selected());
                this.show_greeting_trial();
                if let Some(trial) = this.greeting.pixel_export_window.upgrade() {
                    if let Some(parent) = parent.upgrade() { trial.set_transient_for(Some(&parent)); }
                    let message = message.clone();
                    trial.connect_hide(move |_| message.set_text("Trial closed. Use Review Native Update to recheck this design, selected file and current visual result. A design preview or failed trial does not authorize image output."));
                }
            });

            let weak = Rc::downgrade(self);
            let parent = window.downgrade();
            review.connect_clicked(move |_| {
                let Some(this) = weak.upgrade().filter(|this| this.typed.identity.get() == identity) else { return; };
                if !this.document_use.shared_review.get() {
                    selected.set_text("Choose the native configuration above first. Nothing has been changed.");
                    return;
                }
                let result = this.committed_design().and_then(|_| {
                    if kind == Kind::Prompt {
                        this.document_use.starship.borrow().as_ref().ok_or("Choose the Starship file again.".to_owned())?.verify()
                    } else {
                        this.greeting.presentation.target.set_selected(terminal.selected());
                        let settings = this.greeting.settings();
                        let spec = settings.presentation.resolve(&settings, this.greeting.presentation.binding.borrow().terminal)?;
                        if spec.protocol.is_some() && !pixels.is_active() {
                            return Err("This design uses pixels. Choose the independent Kitty entry above, or explicitly allow shared images before proceeding. There is no silent conversion to characters.".into());
                        }
                        this.greeting.presentation.shared.set_active(pixels.is_active());
                        this.prepare_greeting_action().map(|_| ())
                    }
                });
                match result {
                    Ok(()) => { if let Some(parent) = parent.upgrade() { parent.destroy(); } this.request_scheme_apply(); }
                    Err(error) => selected.set_text(&format!("Not ready: {error}\nUse Try Greeting Now for visual validation, or choose the native file again after an external change. Your design and target remain selected.")),
                }
            });
        }
        let cancel = gtk::Button::with_label("Cancel");
        buttons.append(&cancel);
        let parent = window.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(parent) = parent.upgrade() {
                parent.destroy();
            }
        });
        window.present();
    }
}
