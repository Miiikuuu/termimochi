use super::*;
use crate::{
    greeting_output::{self, Presentation, TargetBinding, VerificationKey, Visual},
    pixel_trial::{Terminal, Trial},
};

type PreparedAction = (
    String,
    Option<(String, String)>,
    crate::scheme_apply::Action,
);

pub(in crate::window) struct PresentationEditor {
    pub(super) edits: RefCell<super::presentation_work::EditTask>,
    pub(super) checks: RefCell<super::presentation_work::OutputChecks>,
    pub(super) check_completed: RefCell<Option<Box<dyn Fn()>>>,
    pub root: gtk::Box,
    pub visual: gtk::DropDown,
    pub width: gtk::SpinButton,
    style: gtk::DropDown,
    protocol: gtk::DropDown,
    fallback: gtk::DropDown,
    pub(super) note: gtk::Label,
    pub target_bar: gtk::Box,
    pub target: gtk::DropDown,
    pub shared: gtk::CheckButton,
    pub state: gtk::Label,
    pub binding: RefCell<TargetBinding>,
    binding_path: PathBuf,
    pub verified: RefCell<Option<(VerificationKey, Rc<Trial>)>>,
    pub deployed: RefCell<
        Option<(
            GreetingSettings,
            TargetBinding,
            crate::fastfetch_apply::Target,
            bool,
        )>,
    >,
    pub canvas: gtk::Box,
    picture: gtk::Picture,
    composition: gtk::Box,
    pub fields: gtk::Label,
    caption: gtk::Label,
    zoom: gtk::DropDown,
    play: gtk::ToggleButton,
    prepared: RefCell<Option<std::sync::Arc<crate::greeting_image::pixel_export::PixelImage>>>,
    source: RefCell<Option<crate::greeting_image::source::EditableArtwork>>,
    generation: Cell<u64>,
    frame: Cell<usize>,
    started: Cell<std::time::Instant>,
    allocation: Cell<(i32, i32)>,
    pub cells: Cell<[u32; 2]>,
}
impl PresentationEditor {
    pub(super) fn new(binding_path: PathBuf) -> Self {
        let mut binding = TargetBinding::default();
        if let Ok(Some(bytes)) = typography_preset::read_private_with_limit(&binding_path, 1024)
            && let Ok(terminal) = serde_json::from_slice::<Terminal>(&bytes)
        {
            binding.terminal = terminal;
        }
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let visual = layout_drop_down(
            &["Recommended", "Image", "Character"],
            2,
            "Display effect",
            "Choose the design, not a file format. Pixel support still needs a real terminal trial.",
        );
        let width = typography_spin_button(
            32.0,
            8.0,
            160.0,
            1.0,
            0,
            "Artwork columns",
            "Terminal occupancy, not preview zoom",
        );
        let style = layout_drop_down(
            &["Detail ░▒▓", "ASCII .:+#", "Half blocks ▀▄"],
            0,
            "Character style",
            "More columns add detail AND occupy more terminal width.",
        );
        root.append(&layout_row("Display", &visual));
        let sizes = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        for (label, value) in [("Compact", 24.0), ("Standard", 32.0), ("Wide", 48.0)] {
            let button = gtk::Button::with_label(label);
            button.add_css_class("flat");
            let input = width.clone();
            button.connect_clicked(move |_| {
                if input.is_sensitive() {
                    input.set_value(value);
                }
            });
            sizes.append(&button);
        }
        root.append(&sizes);
        root.append(&layout_row("Columns", &width));
        root.append(&layout_row("Character style", &style));
        let note = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build();
        root.append(&note);
        let protocol = layout_drop_down(
            &["Automatic", "Kitty PNG", "Kitty Animation", "Sixel"],
            0,
            "Output compatibility",
            "Automatic chooses a protocol from the effect and target, not from assumed support.",
        );
        let fallback = layout_drop_down(
            &["Ask before changing effect", "Offer portable characters"],
            0,
            "Fallback preference",
            "A fallback is always an explicit design edit; no silent image replacement.",
        );
        let advanced = layout_group(
            "Output compatibility",
            [
                layout_row("Protocol", &protocol),
                layout_row("Fallback", &fallback),
            ],
        );
        root.append(
            &gtk::Expander::builder()
                .label("Advanced · output compatibility")
                .child(&advanced)
                .build(),
        );
        let target_bar = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let target = layout_drop_down(
            &["Ptyxis", "Kitty", "Xterm"],
            0,
            "Greeting target",
            "Local target only; never included in a shared scheme.",
        );
        target.set_selected(match binding.terminal {
            Terminal::Ptyxis => 0,
            Terminal::Kitty => 1,
            Terminal::Xterm => 2,
        });
        target_bar.append(&layout_row("Greeting target", &target));
        let shared = gtk::CheckButton::with_label("Replace shared greeting with pixels");
        shared.set_tooltip_text(Some("Explicit opt-in: every terminal using the shared Fastfetch file is affected. Unsupported terminals may show a blank image."));
        target_bar.append(&shared);
        let state = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .max_width_chars(44)
            .css_classes(["dim-label"])
            .build();
        target_bar.append(&state);
        let canvas = gtk::Box::new(gtk::Orientation::Vertical, 8);
        canvas.set_widget_name("termimochi-terminal");
        canvas.set_vexpand(true);
        let caption = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .label("Pixel design preview · not terminal verification")
            .build();
        let zoom = layout_drop_down(
            &["Fit preview", "100% occupancy"],
            0,
            "Design preview zoom",
            "Only changes viewing scale; never artwork columns or save state.",
        );
        let play = gtk::ToggleButton::with_label("Play animation");
        let tools = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        tools.append(&zoom);
        tools.append(&play);
        canvas.append(&caption);
        canvas.append(&tools);
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Contain)
            .can_shrink(true)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Start)
            .alternative_text("Editable greeting artwork · click to edit")
            .build();
        picture.set_cursor_from_name(Some("pointer"));
        picture.set_tooltip_text(Some("Artwork · click to edit the retained original"));
        let fields = gtk::Label::builder()
            .xalign(0.0)
            .yalign(0.0)
            .selectable(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .max_width_chars(28)
            .css_classes(["monospace"])
            .build();
        let composition = gtk::Box::new(gtk::Orientation::Horizontal, 24);
        composition.set_margin_top(12);
        composition.set_margin_start(12);
        composition.append(&picture);
        composition.append(&fields);
        canvas.append(
            &gtk::ScrolledWindow::builder()
                .child(&composition)
                .vexpand(true)
                .hexpand(true)
                .build(),
        );
        Self {
            edits: RefCell::new(super::presentation_work::edit_task()),
            checks: RefCell::new(super::presentation_work::OutputChecks::new()),
            check_completed: RefCell::new(None),
            root,
            visual,
            width,
            style,
            protocol,
            fallback,
            note,
            target_bar,
            target,
            shared,
            state,
            binding: RefCell::new(binding),
            binding_path,
            verified: RefCell::new(None),
            deployed: RefCell::new(None),
            canvas,
            picture,
            composition,
            fields,
            caption,
            zoom,
            play,
            prepared: RefCell::new(None),
            source: RefCell::new(None),
            generation: Cell::new(0),
            frame: Cell::new(0),
            started: Cell::new(std::time::Instant::now()),
            allocation: Cell::new((0, 0)),
            cells: Cell::new([10, 20]),
        }
    }
    pub fn refresh(&self, settings: &GreetingSettings) {
        let edit = self.edits.borrow();
        let intent = edit
            .input()
            .map_or(&settings.presentation, |request| &request.intent);
        let gif = settings
            .editable_artwork
            .as_ref()
            .is_some_and(|s| s.image.is_gif());
        let labels: &[&str] = if gif {
            &["Recommended", "Image", "Character", "Animation"]
        } else {
            &["Recommended", "Image", "Character"]
        };
        if self
            .visual
            .model()
            .is_none_or(|m| m.n_items() != labels.len() as u32)
        {
            self.visual.set_model(Some(&gtk::StringList::new(labels)));
        }
        self.visual.set_selected(match intent.visual {
            Visual::Auto => 0,
            Visual::Image => 1,
            Visual::Character => 2,
            Visual::Animation => {
                if gif {
                    3
                } else {
                    gtk::INVALID_LIST_POSITION
                }
            }
        });
        self.width.set_value(intent.columns.into());
        self.width
            .set_sensitive(settings.editable_artwork.is_some());
        self.style.set_sensitive(
            intent.visual == Visual::Character && settings.editable_artwork.is_some(),
        );
        self.style.set_selected(match intent.character_style {
            crate::greeting_image::Style::Detail => 0,
            crate::greeting_image::Style::Ascii => 1,
            crate::greeting_image::Style::HalfBlocks => 2,
        });
        self.protocol.set_selected(match intent.protocol {
            None => 0,
            Some(crate::greeting_image::pixel_export::Protocol::Kitty) => 1,
            Some(crate::greeting_image::pixel_export::Protocol::KittyAnimation) => 2,
            Some(crate::greeting_image::pixel_export::Protocol::Sixel) => 3,
        });
        self.fallback.set_selected(u32::from(
            intent.fallback == greeting_output::Fallback::PortableCharacter,
        ));
        self.note.set_label(if edit.input().is_some() { "Updating artwork… Undo cancels the pending edit." } else if intent.visual == Visual::Character { "More columns = more detail and more terminal space. Preview zoom does not change the design." } else { "Columns set terminal occupancy, not source resolution. Image / animation support remains unverified until tried." });
        self.shared.set_visible(intent.visual != Visual::Character);
    }
    fn intent(&self) -> Presentation {
        use crate::greeting_image::{Style, pixel_export::Protocol};
        Presentation {
            visual: match self.visual.selected() {
                0 => Visual::Auto,
                1 => Visual::Image,
                3 => Visual::Animation,
                _ => Visual::Character,
            },
            columns: self.width.value_as_int() as u32,
            character_style: match self.style.selected() {
                1 => Style::Ascii,
                2 => Style::HalfBlocks,
                _ => Style::Detail,
            },
            protocol: match self.protocol.selected() {
                1 => Some(Protocol::Kitty),
                2 => Some(Protocol::KittyAnimation),
                3 => Some(Protocol::Sixel),
                _ => None,
            },
            fallback: if self.fallback.selected() == 1 {
                greeting_output::Fallback::PortableCharacter
            } else {
                greeting_output::Fallback::Ask
            },
        }
    }
}
impl GreetingEditor {
    pub(super) fn connect_presentation(this: &Rc<Self>) {
        let change = |this: &Rc<Self>| {
            if this.updating.get() {
                return;
            }
            this.queue_presentation_edit(this.presentation.intent());
        };
        for control in [
            &this.presentation.visual,
            &this.presentation.style,
            &this.presentation.protocol,
            &this.presentation.fallback,
        ] {
            let weak = Rc::downgrade(this);
            control.connect_selected_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    change(&this);
                }
            });
        }
        let weak = Rc::downgrade(this);
        this.presentation.width.connect_value_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                change(&this);
            }
        });
        let weak = Rc::downgrade(this);
        this.presentation
            .target
            .connect_selected_notify(move |input| {
                if let Some(this) = weak.upgrade() {
                    this.presentation.binding.borrow_mut().terminal = match input.selected() {
                        1 => Terminal::Kitty,
                        2 => Terminal::Xterm,
                        _ => Terminal::Ptyxis,
                    };
                    let terminal = this.presentation.binding.borrow().terminal;
                    if let Err(error) = typography_preset::write_private(
                        &this.presentation.binding_path,
                        &serde_json::to_vec(&terminal).unwrap(),
                    ) {
                        this.presentation
                            .state
                            .set_label(&format!("Could not remember local target: {error}"));
                    }
                    this.presentation.verified.borrow_mut().take();
                    this.notify();
                }
            });
        let weak = Rc::downgrade(this);
        this.presentation.shared.connect_toggled(move |input| {
            if let Some(this) = weak.upgrade() {
                this.presentation.binding.borrow_mut().shared_pixels = input.is_active();
                this.notify();
            }
        });
        let weak = Rc::downgrade(this);
        this.presentation.zoom.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                this.refresh_pixel_design();
            }
        });
        let weak = Rc::downgrade(this);
        this.presentation.play.connect_toggled(move |_| {
            if let Some(this) = weak.upgrade() {
                this.presentation.started.set(std::time::Instant::now());
            }
        });
        let click = gtk::GestureClick::new();
        let weak = Rc::downgrade(this);
        click.connect_released(move |_, _, _, _| {
            if let Some(this) = weak.upgrade() {
                let _ = this.root.activate_action("win.edit-image-artwork", None);
            }
        });
        this.presentation.picture.add_controller(click);
        let weak = Rc::downgrade(this);
        glib::timeout_add_local(Duration::from_millis(30), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            this.poll_presentation_work();
            let p = &this.presentation;
            let allocation = (p.canvas.width(), p.canvas.height());
            if p.canvas.is_mapped() && p.allocation.replace(allocation) != allocation {
                this.refresh_pixel_design();
            }
            if p.play.is_active()
                && p.canvas.is_mapped()
                && let Some(image) = p.prepared.borrow().as_ref()
                && let Some(animation) = image.animation.as_ref()
            {
                let index = p.frame.get().min(animation.frames.len() - 1);
                if p.started.get().elapsed()
                    >= Duration::from_millis(animation.frames[index].delay_ms.into())
                {
                    let next = (index + 1) % animation.frames.len();
                    p.frame.set(next);
                    p.started.set(std::time::Instant::now());
                    p.show_pixels(&animation.frames[next].pixels);
                }
            }
            glib::ControlFlow::Continue
        });
    }
    pub(super) fn refresh_pixel_design(&self) {
        let settings = self.settings();
        let p = &self.presentation;
        p.composition.set_orientation(
            if matches!(settings.position, Position::Top | Position::Card) {
                gtk::Orientation::Vertical
            } else {
                gtk::Orientation::Horizontal
            },
        );
        p.composition.reorder_child_after(
            &p.picture,
            if settings.position == Position::Right {
                Some(&p.fields)
            } else {
                None::<&gtk::Label>
            },
        );
        p.composition.set_spacing(i32::from(settings.gap) * 10);
        if let Some(image) = p.prepared.borrow().as_ref() {
            let cells = p.cells.get();
            let (columns, rows) = crate::greeting_image::pixel_export::area(
                image.pixels.dimensions(),
                settings.presentation.columns,
                cells[0] as f64 / cells[1] as f64,
            )
            .unwrap_or((1, 1));
            let mut width = columns as i32 * cells[0] as i32;
            if p.zoom.selected() == 0 {
                let beside = !matches!(settings.position, Position::Top | Position::Card);
                let fields_width = p.fields.measure(gtk::Orientation::Horizontal, -1).1;
                let fields_height = p
                    .fields
                    .measure(gtk::Orientation::Vertical, fields_width.max(1))
                    .1;
                let available = (p.canvas.width()
                    - if beside {
                        fields_width + i32::from(settings.gap) * 10 + 32
                    } else {
                        32
                    })
                .max(80);
                let height_limit = (p.canvas.height()
                    - 110
                    - if beside {
                        0
                    } else {
                        fields_height + i32::from(settings.gap) * 10
                    })
                .max(80) as f64
                    * image.pixels.width() as f64
                    / image.pixels.height() as f64;
                width = width.min(available).min(height_limit as i32).max(1);
            }
            let height = (width as f64 * image.pixels.height() as f64 / image.pixels.width() as f64)
                .round() as i32;
            p.picture.set_size_request(width, height);
            p.caption.set_label(&format!("{} × {} processed px · {columns} columns × {rows} rows\nGTK design composition · verify actual placement in Try Greeting",image.pixels.width(),image.pixels.height()));
        }
        let animated = settings
            .presentation
            .resolve(&settings, p.binding.borrow().terminal)
            .is_ok_and(|s| {
                s.protocol == Some(crate::greeting_image::pixel_export::Protocol::KittyAnimation)
            });
        p.play.set_visible(animated);
        if !animated {
            p.play.set_active(false);
            if let Some(image) = p.prepared.borrow().as_ref() {
                p.show_pixels(&image.pixels);
            }
        }
        if *p.source.borrow() != settings.editable_artwork {
            *p.source.borrow_mut() = settings.editable_artwork.clone();
            p.prepared.borrow_mut().take();
            p.picture.set_paintable(None::<&gdk::Paintable>);
            p.generation.set(p.generation.get().wrapping_add(1));
            if let Some(source) = settings.editable_artwork {
                let generation = p.generation.get();
                let (send, receive) = mpsc::channel();
                std::thread::spawn(move || {
                    let _ = send.send(crate::greeting_image::pixel_export::prepare(&source));
                });
                let weak = self.weak.clone();
                glib::timeout_add_local(Duration::from_millis(50), move || {
                    let Some(this) = weak
                        .upgrade()
                        .filter(|t| t.presentation.generation.get() == generation)
                    else {
                        return glib::ControlFlow::Break;
                    };
                    match receive.try_recv() {
                        Ok(Ok(image)) => {
                            this.presentation.show_pixels(&image.pixels);
                            *this.presentation.prepared.borrow_mut() =
                                Some(std::sync::Arc::new(image));
                            this.presentation.frame.set(0);
                            this.refresh_pixel_design();
                        }
                        Ok(Err(error)) => this.presentation.caption.set_label(&error),
                        Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                        Err(_) => this
                            .presentation
                            .caption
                            .set_label("Image preparation failed; retry the edit."),
                    }
                    glib::ControlFlow::Break
                });
            }
        }
    }
}
impl PresentationEditor {
    fn show_pixels(&self, pixels: &image::RgbaImage) {
        let texture = gdk::MemoryTexture::new(
            pixels.width() as i32,
            pixels.height() as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(pixels.as_raw().clone()),
            pixels.width() as usize * 4,
        );
        self.picture.set_paintable(Some(&texture));
    }
}
impl Workbench {
    pub(in crate::window) fn greeting_verification_key(&self) -> Result<VerificationKey, String> {
        self.greeting.require_presentation_ready()?;
        // Safety boundary: explicit trials/review never authorize from the UI cache.
        self.greeting_check_input().verification_key()
    }
    pub(in crate::window) fn prepare_greeting_action(&self) -> Result<PreparedAction, String> {
        self.greeting.require_presentation_ready()?;
        use crate::scheme_apply::Action;
        let settings = self.greeting.settings();
        let binding = self.greeting.presentation.binding.borrow().clone();
        let spec = settings.presentation.resolve(&settings, binding.terminal)?;
        let destination = spec.destination(&binding);
        let (shared, source) = self.prepare_scheme_fastfetch()?;
        let before = |t: &crate::fastfetch_apply::Target| {
            t.expected
                .as_ref()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_else(|| "No file".into())
        };
        if destination == greeting_output::Destination::SharedCharacter {
            return Ok((
                format!(
                    "Character greeting · shared configuration: {}\nEvery terminal reading this file is affected. Imported commands/network modules are retained, NOT executed by Apply. Shell startup is unchanged.",
                    shared.path.display()
                ),
                Some((before(&shared), source.clone())),
                Action::Fastfetch {
                    target: shared,
                    source,
                },
            ));
        }
        let key = self.greeting_verification_key()?;
        let verified = self.greeting.presentation.verified.borrow();
        let trial=verified.as_ref().filter(|(k,_)|*k==key).map(|(_,t)|t).ok_or("Image / animation has not been verified for this design and target. Use Try Greeting; the intended effect will NOT be replaced with characters.")?;
        if trial.protocol != spec.protocol.unwrap() || trial.ansi {
            return Err("Trial output differs from the current design. Try again.".into());
        }
        let independent = destination == greeting_output::Destination::IndependentImage;
        let target = if independent {
            crate::fastfetch_apply::Target::open(glib::user_data_dir().join(format!(
                "termimochi/targets/{}/config.jsonc",
                binding.terminal.label().to_ascii_lowercase()
            )))?
        } else {
            shared
        };
        let plan = trial
            .install_plan(
                &glib::user_data_dir().join("termimochi/image-greetings"),
                target,
            )?
            .with_current_fields(&source)?;
        let detail = format!(
            "{:?} · tested in {}\n{}\nDestination: {}\nImported commands/network modules and preRun are retained in the full diff; Apply does not execute them. No shell startup hook is added.",
            settings.presentation.visual,
            binding.terminal.label(),
            if independent {
                "Independent image configuration; shared Fastfetch is untouched. Installed but NOT automatically enabled. Use the explicit --config path in the tested terminal."
            } else {
                "WARNING: replaces the SHARED Fastfetch configuration. ALL terminals reading this file are affected; unsupported terminals can display a blank image."
            },
            plan.target.path.display()
        );
        Ok((
            detail,
            Some((before(&plan.target), plan.config.clone())),
            Action::ImageGreeting { plan, independent },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::greeting::tests::{controller, settle};
    #[test]
    #[ignore = "isolated GTK: every main Save, GIF reopen, undo, session zoom, minimum and common windows"]
    fn scheme_presentation_save_reopen_zoom_and_main_actions() {
        assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.SchemeFlowTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        let scheme = root.path().join("animated.termimochi.json");
        let active = glib::user_config_dir().join("fastfetch/config.jsonc");
        std::fs::create_dir_all(active.parent().unwrap()).unwrap();
        std::fs::write(&active, "// sentinel\n{}").unwrap();
        let shell = glib::home_dir().join(".bashrc");
        std::fs::write(&shell, "# untouched\n").unwrap();
        present_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let this = controller(&window);
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        let (mut settings, _) = crate::pixel_trial::tests::fixture(true);
        settings.presentation.visual = Visual::Animation;
        settings.presentation.columns = 48;
        this.greeting.replace(settings.clone(), true);
        this.greeting.presentation.target.set_selected(1);
        this.save_workspace_path(scheme.clone(), this.committed_workspace().unwrap())
            .unwrap();
        for (index, module) in [
            EditorModule::Palette,
            EditorModule::Typography,
            EditorModule::Layout,
            EditorModule::Prompt,
            EditorModule::Greeting,
        ]
        .into_iter()
        .enumerate()
        {
            gio::prelude::ActionGroupExt::activate_action(
                &this.window(),
                module.action_name().trim_start_matches("win."),
                None,
            );
            let mut design = this.greeting.settings();
            design.message = format!("Save from module {index}");
            this.greeting.replace(design, true);
            settle();
            assert!(this.save_action.is_enabled());
            this.save_action.activate(None);
            let reopened = DocumentStore::<Workspace>::open(scheme.clone())
                .unwrap()
                .document()
                .unwrap()
                .unwrap();
            assert_eq!(
                reopened.greeting.message,
                format!("Save from module {index}")
            );
            assert_eq!(reopened.greeting.presentation.visual, Visual::Animation);
            assert_eq!(reopened.greeting.presentation.columns, 48);
        }
        this.greeting_module_button.set_active(true);
        this.preview_scene_selector.set_selected(2);
        settle();
        let saved = this.workspace_snapshot();
        assert!(this.workspace_is_clean());
        this.greeting.presentation.zoom.set_selected(0);
        this.greeting.root.vadjustment().set_value(0.0);
        this.greeting.presentation.zoom.set_selected(1);
        this.greeting.presentation.play.set_active(true);
        settle();
        let prepared = this
            .greeting
            .presentation
            .prepared
            .borrow()
            .clone()
            .unwrap();
        for module in [
            &this.palette_module_button,
            &this.typography_module_button,
            &this.layout_module_button,
            &this.prompt_module_button,
        ] {
            let started = this.greeting.presentation.started.get();
            module.set_active(true);
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while this.greeting.presentation.started.get() == started {
                assert!(
                    std::time::Instant::now() < deadline,
                    "GIF stopped after editor navigation"
                );
                settle();
            }
            assert!(this.greeting.presentation.canvas.is_mapped());
            assert!(this.greeting.presentation.play.is_active());
            assert!(std::sync::Arc::ptr_eq(
                &prepared,
                this.greeting
                    .presentation
                    .prepared
                    .borrow()
                    .as_ref()
                    .unwrap()
            ));
        }
        this.palette_module_button.set_active(true);
        for rgb in [Rgb::new(21, 39, 57), Rgb::new(230, 240, 250)] {
            this.apply_color("Background", rgb);
            settle();
            let canvas = &this.greeting.presentation.canvas;
            let snapshot = gtk::Snapshot::new();
            gtk::WidgetPaintable::new(Some(canvas)).snapshot(
                &snapshot,
                f64::from(canvas.width()),
                f64::from(canvas.height()),
            );
            let texture = canvas
                .native()
                .unwrap()
                .renderer()
                .unwrap()
                .render_texture(snapshot.to_node().unwrap(), None);
            let width = texture.width() as usize;
            let mut bytes = vec![0; width * texture.height() as usize * 4];
            texture.download(&mut bytes, width * 4);
            // Sample the empty lower margin, not the caption's antialiased text.
            let offset = ((texture.height() as usize - 5) * width + 5) * 4;
            let pixel = u32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap());
            assert_eq!(
                pixel & 0x00ff_ffff,
                u32::from(rgb.red()) << 16 | u32::from(rgb.green()) << 8 | u32::from(rgb.blue())
            );
            assert!(canvas.is_mapped());
            this.undo_action.activate(None);
        }
        this.greeting_module_button.set_active(true);
        this.greeting.root.vadjustment().set_value(80.0);
        assert_eq!(this.workspace_snapshot(), saved);
        assert!(this.workspace_is_clean());
        let intent = this.greeting.settings().presentation;
        this.greeting.presentation.width.set_value(40.0);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while this.greeting.presentation_pending() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        assert_eq!(this.greeting.settings().presentation.columns, 40);
        this.greeting.undo();
        assert_eq!(this.greeting.settings().presentation, intent);
        this.greeting.presentation.zoom.set_selected(0);
        this.greeting.root.vadjustment().set_value(0.0);
        for (width, height) in [(1024, 700), (1280, 900)] {
            window.set_default_size(width, height);
            settle();
            settle();
            this.refresh_output_bar();
            this.greeting.refresh_pixel_design();
            settle();
            assert!(
                this.greeting.presentation.composition.width()
                    <= this.greeting.presentation.canvas.width(),
                "Fit must include the information column and margins"
            );
            assert!(this.output_bar.root.is_mapped());
            let bounds = this.output_bar.root.compute_bounds(&window).unwrap();
            assert!(bounds.y() + bounds.height() <= window.height() as f32 + 1.0);
            let path = glib::user_cache_dir().join(format!("scheme-{width}x{height}.png"));
            window.set_title(Some("TermiMochi point-to-edit test"));
            settle();
            gtk::prelude::WidgetExt::display(&window).flush();
            let result = std::process::Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../scripts/preview-pointer-driver.py"
                ))
                .args(["capture", "0", "0"])
                .env("TERMIMOCHI_INSPECT_SCREENSHOT", &path)
                .status()
                .unwrap();
            assert!(result.success());
            println!("Screenshot: {}", path.display());
        }
        window.close();
        settle();
        present_with_preset(
            &app,
            Some(scheme.clone()),
            root.path().join("reopened-font.json"),
        );
        let reopened_window = app.active_window().unwrap();
        let reopened = controller(&reopened_window);
        settle();
        assert_eq!(
            reopened.greeting.settings().presentation.visual,
            Visual::Animation
        );
        assert_eq!(reopened.greeting.settings().presentation.columns, 48);
        assert!(reopened.greeting.presentation.verified.borrow().is_none());
        assert_eq!(
            reopened.greeting.presentation.binding.borrow().terminal,
            Terminal::Kitty
        );
        assert!(
            !reopened
                .greeting
                .presentation
                .binding
                .borrow()
                .shared_pixels
        );
        assert_eq!(std::fs::read_to_string(active).unwrap(), "// sentinel\n{}");
        assert_eq!(std::fs::read_to_string(shell).unwrap(), "# untouched\n");
        reopened_window.close();
        settle();
    }
}
