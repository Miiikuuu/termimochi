use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use termimochi_core::{
    ExportFormat, Issue, PtyxisPalette, Rgb, Severity, Target, Variant, contrast_ratio,
    export_palette, lint_palette, paths_refer_to_same_file, write_atomically,
};
use vte::prelude::*;

mod color_targets;
mod daily_launcher;
mod document_use;
mod documents;
mod full_session;
mod greeting;
mod output_bar;
mod preview_hint;
mod preview_scene;
mod preview_scroll;
mod scheme;
#[cfg(test)]
mod stress_tests;
mod theme_workspace;
mod typed_documents;
mod typed_kitty;
#[cfg(test)]
mod typed_tests;

use crate::{
    color_picker::{ColorPicker, ColorSwatch, scroll_parent_vertically},
    document_store::{self, Document, DocumentStore},
    layout::{
        DEFAULT_CONTENT_PADDING, LayoutPreset, LayoutSettings, MAX_COLUMNS, MAX_CONTENT_PADDING,
        MAX_ROWS, MAX_WINDOW_SPACING, MIN_COLUMNS, MIN_CONTENT_PADDING, MIN_ROWS,
        MIN_WINDOW_SPACING, PreviewCursorBlink, PreviewCursorShape,
    },
    layout_apply,
    preview::{
        PREVIEW_COLUMNS, PREVIEW_HOME_AND_CLEAR, PREVIEW_INPUT_PROMPT, PREVIEW_ROWS,
        PREVIEW_SHOW_CURSOR, PreviewInput, PreviewInputEvent, PreviewScenario,
    },
    preview_context::CurrentPreviewContext,
    preview_inspect::{ANSI_NAMES, PreviewClick, PreviewMap, PreviewTarget},
    prompt::{
        PreviewTone, PromptCharacter, PromptHostnameMode, PromptLayout, PromptPreset,
        PromptPreviewContext, PromptSegmentKind, PromptSettings, STARSHIP_FILE_NAME,
    },
    prompt_diagnostics::{PromptDiagnostics, is_font_issue, is_prompt_character, issue_module},
    ptyxis::{
        CurrentTerminalAppearance, InstallOutcome, PtyxisInstaller, RollbackOutcome,
        import_current_appearance,
    },
    starship_editor::StarshipEditor,
    style::chrome_css,
    typography::{
        DEFAULT_FONT_FAMILY, MAX_CELL_SCALE, MAX_FONT_SIZE, MIN_CELL_SCALE, MIN_FONT_SIZE,
        PreviewFontWeight, TypographySettings, detect_nerd_font_support, is_usable_terminal_family,
    },
    typography_apply::{RestoreRequest, TypographyTarget},
    typography_preset::{self, PresetStore},
    workspace::Workspace,
};

const SAMPLE_PALETTE: &str = include_str!("../resources/themes/fog-paper.palette");
const EDIT_HISTORY_LIMIT: usize = 64;
const PREVIEW_MIN_HEIGHT: i32 = 128;

const BASIC_COLORS: [(&str, &str, &str); 4] = [
    ("Foreground", "Text", "Default terminal text"),
    ("Background", "Background", "Terminal canvas"),
    ("Cursor", "Cursor", "Input position"),
    ("CursorForeground", "Cursor Text", "Text beneath the cursor"),
];

#[derive(Clone)]
struct ColorControl {
    swatch: ColorSwatch,
}

struct Model {
    palette: PtyxisPalette,
    active_variant: Variant,
    current_path: Option<PathBuf>,
    source_label: String,
    saved_contents: String,
    dirty: bool,
}

impl Model {
    fn new(
        palette: PtyxisPalette,
        active_variant: Variant,
        current_path: Option<PathBuf>,
        source_label: String,
    ) -> Self {
        let saved_contents = palette.to_palette_string();
        Self {
            palette,
            active_variant,
            current_path,
            source_label,
            saved_contents,
            dirty: false,
        }
    }

    fn refresh_dirty(&mut self) {
        self.dirty = self.palette.to_palette_string() != self.saved_contents;
    }

    fn mark_saved(&mut self) {
        self.saved_contents = self.palette.to_palette_string();
        self.dirty = false;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EditorSnapshot {
    palette: PtyxisPalette,
    active_variant: Variant,
    selected_color_key: String,
}

#[derive(Clone)]
struct EditorContext {
    active_variant: Variant,
    selected_color_key: String,
}

trait HistorySnapshot: Clone + PartialEq {
    type Context: Clone;

    fn context(&self) -> Self::Context;
    fn set_context(&mut self, context: Self::Context);
}

#[cfg(test)]
impl HistorySnapshot for i32 {
    type Context = ();

    fn context(&self) -> Self::Context {}

    fn set_context(&mut self, (): Self::Context) {}
}

impl HistorySnapshot for EditorSnapshot {
    type Context = EditorContext;

    fn context(&self) -> Self::Context {
        EditorContext {
            active_variant: self.active_variant,
            selected_color_key: self.selected_color_key.clone(),
        }
    }

    fn set_context(&mut self, context: Self::Context) {
        self.active_variant = context.active_variant;
        self.selected_color_key = context.selected_color_key;
    }
}

impl HistorySnapshot for PromptSettings {
    type Context = ();

    fn context(&self) -> Self::Context {}

    fn set_context(&mut self, (): Self::Context) {}
}

impl HistorySnapshot for TypographySettings {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn set_context(&mut self, (): Self::Context) {}
}

impl HistorySnapshot for LayoutSettings {
    type Context = ();
    fn context(&self) -> Self::Context {}
    fn set_context(&mut self, (): Self::Context) {}
}

struct UndoPoint<T: HistorySnapshot> {
    before: T,
    after_context: T::Context,
}

struct EditHistory<T: HistorySnapshot> {
    undo: VecDeque<UndoPoint<T>>,
    redo: VecDeque<T>,
    pending: Option<T>,
    pending_changed: bool,
    limit: usize,
}

impl<T: HistorySnapshot> EditHistory<T> {
    fn new(limit: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            pending: None,
            pending_changed: false,
            limit,
        }
    }

    fn is_editing(&self) -> bool {
        self.pending.is_some()
    }

    fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.pending = None;
        self.pending_changed = false;
    }

    fn mark_changed(&mut self) {
        if self.pending.is_some() {
            self.pending_changed = true;
        }
    }

    fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.pending_changed
    }

    fn can_redo(&self) -> bool {
        !self.redo.is_empty() && !self.pending_changed
    }

    fn abandon_unchanged(&mut self) -> bool {
        if self.pending.is_none() || self.pending_changed {
            return false;
        }
        self.pending = None;
        true
    }
}

impl<T: HistorySnapshot> EditHistory<T> {
    fn begin(&mut self, current: T) {
        if self.pending.is_none() {
            self.pending = Some(current);
            self.pending_changed = false;
        }
    }

    fn commit(&mut self, current: T) -> bool {
        let Some(before) = self.pending.take() else {
            return false;
        };
        let changed = self.pending_changed;
        self.pending_changed = false;
        if !changed || before == current {
            return false;
        }
        let after_context = current.context();
        self.push_undo(UndoPoint {
            before,
            after_context,
        });
        self.redo.clear();
        true
    }

    fn undo(&mut self, current: T) -> Option<T> {
        self.commit(current.clone());
        let point = self.undo.pop_back()?;
        let mut redo_target = current;
        redo_target.set_context(point.after_context);
        self.redo.push_back(redo_target);
        Some(point.before)
    }

    fn undo_pending(&mut self, current: T) -> Option<T> {
        let before = self.pending.take()?;
        let changed = std::mem::take(&mut self.pending_changed);
        if !changed || before == current {
            return None;
        }

        // This is equivalent to committing the in-progress edit and undoing it
        // immediately, but avoids exposing a valid intermediate value as an
        // extra Undo step while an Entry still contains an invalid draft.
        self.redo.clear();
        self.redo.push_back(current);
        Some(before)
    }

    fn redo(&mut self, current: T) -> Option<T> {
        self.commit(current.clone());
        let target = self.redo.pop_back()?;
        self.push_undo(UndoPoint {
            before: current,
            after_context: target.context(),
        });
        Some(target)
    }

    fn push_undo(&mut self, entry: UndoPoint<T>) {
        if self.limit == 0 {
            return;
        }
        if self.undo.len() == self.limit {
            self.undo.pop_front();
        }
        self.undo.push_back(entry);
    }
}

#[derive(Clone)]
struct PreviewChunk {
    text: String,
    scope: Option<PreviewTarget>,
}

type InitialPromptKey = (u64, PathBuf, usize, u32);

struct Workbench {
    typed: typed_documents::TypedSession,
    document_use: document_use::DocumentUseBinding,
    window: glib::WeakRef<adw::ApplicationWindow>,
    toast_overlay: adw::ToastOverlay,
    brand_title: gtk::Label,
    save_button: gtk::MenuButton,
    output_bar: output_bar::OutputBar,
    color_targets: color_targets::ColorTargets,
    open_button: gtk::Button,
    greeting: Rc<greeting::GreetingEditor>,
    greeting_module_button: gtk::ToggleButton,
    greeting_preview: Cell<bool>,
    greeting_redraw_pending: Cell<bool>,
    greeting_motion: greeting::GreetingMotion,
    greeting_official_key: RefCell<Option<greeting::OfficialKey>>,
    greeting_official_result:
        RefCell<Option<Result<crate::greeting_official::NativeOutput, String>>>,
    greeting_official_loading: Cell<bool>,
    save_action: gio::SimpleAction,
    undo_action: gio::SimpleAction,
    redo_action: gio::SimpleAction,
    palette_module_button: gtk::ToggleButton,
    typography_module_button: gtk::ToggleButton,
    layout_module_button: gtk::ToggleButton,
    prompt_module_button: gtk::ToggleButton,
    install_action: gio::SimpleAction,
    rollback_action: gio::SimpleAction,
    name_entry: gtk::Entry,
    light_button: gtk::ToggleButton,
    dark_button: gtk::ToggleButton,
    controls: BTreeMap<String, ColorControl>,
    selected_color_key: RefCell<String>,
    selected_color_title: gtk::Label,
    color_picker: ColorPicker,
    terminal_css_provider: gtk::CssProvider,
    terminal_css_scope: String,
    preview_content: gtk::Box,
    terminal_title: gtk::Label,
    preview_terminal_shell: gtk::Box,
    inspect_button: gtk::ToggleButton,
    inspect_layer: gtk::Overlay,
    inspect_label: gtk::Label,
    inspect_highlight: gtk::DrawingArea,
    preview_terminal_tab: gtk::Box,
    preview_terminal: vte::Terminal,
    preview_terminal_canvas: gtk::Box,
    preview_terminal_viewport: gtk::ScrolledWindow,
    preview_terminal_scrollbar_revealer: gtk::Revealer,
    preview_scroll: Rc<preview_scroll::PreviewScroll>,
    preview_selector: gtk::DropDown,
    preview_scene_selector: gtk::DropDown,
    full_session: full_session::FullSession,
    prompt_preview_selector: gtk::DropDown,
    prompt_compare_selector: gtk::DropDown,
    appearance_source: gtk::Label,
    appearance_details: gtk::Label,
    preview_context_source: gtk::Label,
    fit_preview_switch: gtk::Switch,
    font_family_selector: gtk::DropDown,
    font_size_input: gtk::SpinButton,
    font_weight_selector: gtk::DropDown,
    line_height_input: gtk::SpinButton,
    cell_width_input: gtk::SpinButton,
    nerd_status_label: gtk::Label,
    typography_status: gtk::Label,
    typography_store: RefCell<Option<PresetStore>>,
    typography_baseline: RefCell<TypographySettings>,
    last_typography: RefCell<TypographySettings>,
    typography_history: RefCell<EditHistory<TypographySettings>>,
    typography_has_saved: Cell<bool>,
    layout_status: gtk::Label,
    layout_store: RefCell<Option<DocumentStore<LayoutPreset>>>,
    layout_baseline: Cell<LayoutSettings>,
    last_layout: Cell<LayoutSettings>,
    layout_history: RefCell<EditHistory<LayoutSettings>>,
    layout_has_saved: Cell<bool>,
    workspace_store: RefCell<Option<DocumentStore<Workspace>>>,
    workspace_baseline: RefCell<Option<Workspace>>,
    workspace_prompt_loaded: Cell<bool>,
    content_padding_input: gtk::SpinButton,
    column_count_input: gtk::SpinButton,
    row_count_input: gtk::SpinButton,
    cursor_shape_selector: gtk::DropDown,
    cursor_blink_selector: gtk::DropDown,
    tab_bar_switch: gtk::Switch,
    scrollbar_switch: gtk::Switch,
    window_spacing_input: gtk::SpinButton,
    prompt_preset_label: gtk::Label,
    prompt_preset_popover: gtk::Popover,
    prompt_preset_buttons: Vec<gtk::Button>,
    prompt_module_list: gtk::Box,
    prompt_module_count: gtk::Label,
    prompt_empty_state: gtk::Box,
    prompt_module_rows: Vec<gtk::Box>,
    prompt_module_select_buttons: Vec<gtk::ToggleButton>,
    prompt_module_samples: Vec<gtk::Label>,
    prompt_module_move_up_buttons: Vec<gtk::Button>,
    prompt_module_move_down_buttons: Vec<gtk::Button>,
    prompt_module_remove_buttons: Vec<gtk::Button>,
    prompt_add_button: gtk::MenuButton,
    prompt_add_popover: gtk::Popover,
    prompt_add_module_buttons: Vec<gtk::Button>,
    prompt_inspector: gtk::Box,
    prompt_inspector_title: gtk::Label,
    prompt_tone_selector: gtk::DropDown,
    prompt_character_selector: gtk::DropDown,
    prompt_layout_selector: gtk::DropDown,
    prompt_hostname_selector: gtk::DropDown,
    prompt_spacing_switch: gtk::Switch,
    prompt_source_selector: gtk::DropDown,
    prompt_import_panel: gtk::Box,
    prompt_design_panel: gtk::Box,
    prompt_import_status: gtk::Label,
    starship_editor: Rc<StarshipEditor>,
    copy_generation: Cell<u64>,
    copy_loading: Cell<bool>,
    copy_preview: RefCell<Option<Result<String, String>>>,
    copy_rendered_source: RefCell<Option<String>>,
    copy_prompt_key: RefCell<Option<(String, PathBuf, usize, u32)>>,
    copy_notices: RefCell<Vec<String>>,
    copy_initial_sample: RefCell<Option<(InitialPromptKey, Result<String, String>)>>,
    copy_scene: RefCell<Option<crate::starship_scene::RenderedScene>>,
    copy_scene_history: RefCell<VecDeque<crate::starship_scene::RenderedScene>>,
    scene_diagnostics: RefCell<PromptDiagnostics>,
    used_prompt_characters: RefCell<BTreeSet<char>>,
    prompt_diagnostics: RefCell<PromptDiagnostics>,
    reported_issues: RefCell<Option<Vec<Issue>>>,
    diagnostics_pending: Cell<bool>,
    body_ratio: gtk::Label,
    composer_ratio: gtk::Label,
    summary_icon: gtk::Box,
    summary_title: gtk::Label,
    diagnostic_header: gtk::Box,
    diagnostic_surface: gtk::Box,
    diagnostics: gtk::ListBox,
    ptyxis_installer: Option<PtyxisInstaller>,
    model: RefCell<Model>,
    preview_input: RefCell<PreviewInput>,
    current_preview_context: RefCell<Option<CurrentPreviewContext>>,
    preview_directory: RefCell<PathBuf>,
    preview_generation: Cell<u64>,
    preview_loading: Cell<bool>,
    updating_geometry: Cell<bool>,
    geometry_pending: Cell<bool>,
    preview_map: RefCell<PreviewMap>,
    preview_uses_prompt: Cell<bool>,
    preview_prompt_source: Cell<u32>,
    preview_feed: RefCell<Vec<PreviewChunk>>,
    prompt_preview_base: RefCell<Option<Vec<PreviewChunk>>>,
    prompt_initial_ansi: RefCell<String>,
    designer_original: RefCell<Option<PromptSettings>>,
    navigating_preview: Cell<bool>,
    inspect_generation: Cell<u64>,
    inspect_hit: RefCell<Option<PreviewHit>>,
    inspect_pointer: Cell<Option<(f64, f64)>>,
    inspect_hover_pending: Cell<bool>,
    inspect_origin: Cell<Option<Option<(i64, f64)>>>,
    prompt_settings: RefCell<PromptSettings>,
    last_exported_prompt: RefCell<PromptSettings>,
    selected_prompt_kind: Cell<Option<PromptSegmentKind>>,
    prompt_history: RefCell<EditHistory<PromptSettings>>,
    history: RefCell<EditHistory<EditorSnapshot>>,
    name_valid: Cell<bool>,
    updating: Cell<bool>,
    updating_prompt: Cell<bool>,
}

fn fallback_document() -> (PtyxisPalette, Option<PathBuf>, String, Option<Variant>) {
    (
        PtyxisPalette::from_text(SAMPLE_PALETTE).expect("bundled palette is valid"),
        None,
        "Default Template".to_owned(),
        None,
    )
}

fn preferred_ui_variant() -> Variant {
    if adw::StyleManager::default().is_dark() {
        Variant::Dark
    } else {
        Variant::Light
    }
}

fn set_menu_verb_icon(item: &gio::MenuItem, icon_name: &str) {
    let icon = gio::ThemedIcon::new(icon_name);
    if let Some(serialized) = icon.serialize() {
        item.set_attribute_value("verb-icon", Some(&serialized));
    }
}

pub fn present(application: &adw::Application, initial_path: Option<PathBuf>) {
    present_with_preset(
        application,
        initial_path,
        typography_preset::state_directory().join(typography_preset::PRESET_NAME),
    );
}

fn present_with_preset(
    application: &adw::Application,
    initial_path: Option<PathBuf>,
    preset_path: PathBuf,
) {
    present_with_mode(application, initial_path, preset_path, false);
}

#[cfg(test)]
fn present_advanced_with_preset(
    application: &adw::Application,
    initial_path: Option<PathBuf>,
    preset_path: PathBuf,
) {
    present_with_mode(application, initial_path, preset_path, true);
}

fn present_with_mode(
    application: &adw::Application,
    initial_path: Option<PathBuf>,
    preset_path: PathBuf,
    advanced: bool,
) {
    if let Some(display) = gdk::Display::default() {
        gtk::IconTheme::for_display(&display)
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
    }
    let initial_document = initial_path;
    let initial_path: Option<PathBuf> = None;
    let layout_path = preset_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("layout.termimochi-layout.json");
    let greeting_path = preset_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(crate::greeting::PRESET_NAME);
    let mut appearance = import_current_appearance();
    let (palette, current_path, source_label, preferred_variant, startup_notice) =
        if let Some(path) = initial_path.as_deref() {
            match PtyxisPalette::from_file(path) {
                Ok(palette) => (
                    palette,
                    Some(path.to_owned()),
                    "Local File".to_owned(),
                    None,
                    None,
                ),
                Err(error) => {
                    eprintln!("TermiMochi: {error}");
                    let (palette, path, source, variant) = fallback_document();
                    (
                        palette,
                        path,
                        source,
                        variant,
                        Some(format!(
                            "Could not open the theme; using the default template: {error}"
                        )),
                    )
                }
            }
        } else {
            match appearance.palette.take() {
                Some(palette) => (
                    palette,
                    None,
                    appearance.source_label.clone(),
                    appearance.preferred_variant,
                    None,
                ),
                None => {
                    let (palette, path, source, variant) = fallback_document();
                    (
                        palette,
                        path,
                        source,
                        variant,
                        Some(
                            "Using preview defaults. Open Preview Source for import details."
                                .to_owned(),
                        ),
                    )
                }
            }
        };
    let preferred_variant = preferred_variant.unwrap_or_else(preferred_ui_variant);
    let active_variant = if palette.variant(preferred_variant).is_some() {
        preferred_variant
    } else if palette.variant(Variant::Light).is_some() {
        Variant::Light
    } else {
        Variant::Dark
    };
    let mut initial_typography = appearance.typography.clone();
    let mut layout_notice = None;
    let layout_store = match DocumentStore::<LayoutPreset>::open(layout_path) {
        Ok(store) => Some(store),
        Err(error) => {
            layout_notice = Some(format!("Saved layout unavailable: {error}"));
            None
        }
    };
    let saved_layout = layout_store
        .as_ref()
        .and_then(|store| store.document().ok().flatten());
    let initial_layout = saved_layout
        .as_ref()
        .map(|preset| preset.layout)
        .unwrap_or(appearance.layout);
    let mut typography_notice = None;
    let typography_store = match PresetStore::open(preset_path) {
        Ok(store) => Some(store),
        Err(error) => {
            typography_notice = Some(format!("Saved typography unavailable: {error}"));
            None
        }
    };
    let mut typography_has_saved = false;
    if let Some(store) = &typography_store {
        match store.settings() {
            Ok(Some(settings)) => {
                initial_typography = settings;
                typography_has_saved = true;
            }
            Ok(None) => {}
            Err(error) => {
                typography_notice = Some(format!("Saved typography could not be loaded: {error}"))
            }
        }
    }

    // White workspace with a softly separated navigation rail. Palette variants
    // continue to control VTE only; the desktop accent is not modified.
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);

    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("TermiMochi")
        .icon_name(crate::APPLICATION_ID)
        .default_width(1080)
        .default_height(780)
        // Palette's fixed ANSI grid and the Activity Rail need about 438px;
        // 800px preserves the right preview's 360px minimum beside them.
        .width_request(800)
        .height_request(560)
        .build();
    window.add_css_class("termimochi-window");

    let toast_overlay = adw::ToastOverlay::new();
    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_css_class("workbench-toolbar");
    let header_bar = adw::HeaderBar::new();
    header_bar.add_css_class("workbench-header");
    header_bar.set_show_title(false);
    let brand_name = gtk::Label::builder()
        .label("TermiMochi")
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(32)
        .css_classes(["brand-title-name"])
        .build();
    let app_logo = gtk::Image::builder()
        .icon_name("termimochi-header-logo")
        .pixel_size(44)
        .css_classes(["brand-mark"])
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    let brand_lockup = gtk::Box::new(gtk::Orientation::Horizontal, 7);
    brand_lockup.set_margin_start(2);
    brand_lockup.set_margin_end(10);
    brand_lockup.append(&app_logo);
    brand_lockup.append(&brand_name);
    header_bar.pack_start(&brand_lockup);

    let open_button = gtk::Button::builder()
        .icon_name("termimochi-open-symbolic")
        .tooltip_text("Open a Ptyxis .palette file  Ctrl+O")
        .action_name("win.open")
        .css_classes(["tool-button", "open-button"])
        .build();
    open_button.update_property(&[
        gtk::accessible::Property::Label("Open Theme"),
        gtk::accessible::Property::Description("Choose a Ptyxis .palette file"),
        gtk::accessible::Property::KeyShortcuts("Control+O"),
    ]);
    let undo_action = gio::SimpleAction::new("undo", None);
    undo_action.set_enabled(false);
    let redo_action = gio::SimpleAction::new("redo", None);
    redo_action.set_enabled(false);
    let save_action = gio::SimpleAction::new("save", None);
    save_action.set_enabled(false);
    let install_action = gio::SimpleAction::new("install-ptyxis", None);
    install_action.set_enabled(false);
    let rollback_action = gio::SimpleAction::new("rollback-ptyxis", None);

    let undo_button = gtk::Button::builder()
        .icon_name("edit-undo-symbolic")
        .tooltip_text("Undo  Ctrl+Z")
        .action_name("win.undo")
        .css_classes(["tool-button"])
        .build();
    undo_button.update_property(&[gtk::accessible::Property::Label("Undo")]);
    let redo_button = gtk::Button::builder()
        .icon_name("edit-redo-symbolic")
        .tooltip_text("Redo  Ctrl+Shift+Z / Ctrl+Y")
        .action_name("win.redo")
        .css_classes(["tool-button"])
        .build();
    redo_button.update_property(&[gtk::accessible::Property::Label("Redo")]);
    let history_controls = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    history_controls.add_css_class("history-controls");
    history_controls.set_spacing(2);
    history_controls.append(&undo_button);
    history_controls.append(&redo_button);

    let light_button = gtk::ToggleButton::with_label("Light");
    light_button.set_tooltip_text(Some("Edit and preview the light palette variant"));
    light_button.update_property(&[gtk::accessible::Property::Description(
        "Edit and preview the light palette variant",
    )]);
    let dark_button = gtk::ToggleButton::with_label("Dark");
    dark_button.set_tooltip_text(Some("Edit and preview the dark palette variant"));
    dark_button.update_property(&[gtk::accessible::Property::Description(
        "Edit and preview the dark palette variant",
    )]);
    dark_button.set_group(Some(&light_button));
    let variant_switch = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    variant_switch.add_css_class("variant-switch");
    variant_switch.set_tooltip_text(Some("Palette variant"));
    variant_switch.set_spacing(2);
    variant_switch.append(&light_button);
    variant_switch.append(&dark_button);

    let save_button = gtk::MenuButton::builder()
        .label("Save")
        .tooltip_text("Save a preset or workspace")
        .always_show_arrow(false)
        .css_classes(["tool-menu", "save-menu"])
        .build();
    if let Some(popover) = save_button.popover() {
        popover.add_css_class("save-popover");
    }
    save_button.update_property(&[
        gtk::accessible::Property::Label("Save"),
        gtk::accessible::Property::Description("Save the current preset or a complete workspace"),
    ]);

    // Brand and history stay separate from contextual output actions.
    let header_actions = gtk::Box::new(gtk::Orientation::Horizontal, 3);
    header_actions.set_valign(gtk::Align::Center);
    header_actions.add_css_class("header-actions");
    header_actions.append(&open_button);
    header_actions.append(&history_controls);
    // Output actions live in the fixed editor bar, not the global header.
    header_bar.pack_end(&header_actions);

    toolbar_view.add_top_bar(&header_bar);
    window.set_content(Some(&toolbar_view));

    let main_paned = gtk::Paned::builder()
        .orientation(gtk::Orientation::Horizontal)
        .position(430)
        .wide_handle(false)
        .build();
    main_paned.add_css_class("workbench-split");
    toolbar_view.set_content(Some(&main_paned));

    let preview = build_preview(&variant_switch, &initial_typography, &main_paned);
    let editor = build_editor();
    let typography = build_typography_editor(&preview.terminal, &initial_typography);
    let layout = build_layout_editor(&initial_layout);
    let initial_prompt = PromptSettings::default();
    let prompt = build_prompt_editor(&initial_prompt);
    let greeting = greeting::GreetingEditor::new(greeting_path);
    let editor_workspace = build_editor_workspace(
        &editor.root,
        &typography.root,
        &layout.root,
        &prompt.root,
        &greeting.root,
    );
    let output_bar = output_bar::OutputBar::new(&save_button);
    let editor_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    editor_column.append(&editor_workspace.root);
    editor_column.append(&output_bar.root);
    main_paned.set_start_child(Some(&editor_column));
    toast_overlay.set_child(Some(&preview.root));
    main_paned.set_end_child(Some(&toast_overlay));
    main_paned.set_resize_start_child(false);
    main_paned.set_shrink_start_child(false);
    main_paned.set_shrink_end_child(false);

    let base_css_provider = gtk::CssProvider::new();
    base_css_provider.load_from_data(&chrome_css(gtk::check_version(4, 16, 0).is_none()));
    let terminal_css_provider = gtk::CssProvider::new();
    // Display providers match every window. Scope mutable preview colors to
    // this shell, independently of document identity (including Save As/copies).
    let terminal_css_scope = format!("terminal-instance-{}", glib::uuid_string_random());
    preview.terminal_shell.add_css_class(&terminal_css_scope);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &base_css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        gtk::style_context_add_provider_for_display(
            &display,
            &terminal_css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
        let colors = terminal_css_provider.clone();
        window.connect_destroy(move |_| {
            gtk::style_context_remove_provider_for_display(&display, &colors);
            gtk::style_context_remove_provider_for_display(&display, &base_css_provider);
        });
    }

    let window_ref = glib::WeakRef::new();
    window_ref.set(Some(&window));
    install_editor_module_actions(&window, &editor_workspace);
    let workbench = Rc::new(Workbench {
        typed: {
            let session = typed_documents::TypedSession::new();
            session.advanced.set(advanced);
            session
        },
        document_use: document_use::DocumentUseBinding::new(),
        window: window_ref,
        toast_overlay,
        brand_title: brand_name,
        save_button,
        output_bar,
        open_button: open_button.clone(),
        greeting_module_button: editor_workspace.greeting_button.clone(),
        greeting_preview: Cell::new(greeting.settings().enabled),
        greeting_redraw_pending: Cell::new(false),
        greeting_motion: greeting::GreetingMotion::new(),
        greeting_official_key: RefCell::new(None),
        greeting_official_result: RefCell::new(None),
        greeting_official_loading: Cell::new(false),
        greeting,
        save_action,
        undo_action,
        redo_action,
        palette_module_button: editor_workspace.palette_button.clone(),
        typography_module_button: editor_workspace.typography_button.clone(),
        layout_module_button: editor_workspace.layout_button.clone(),
        prompt_module_button: editor_workspace.prompt_button.clone(),
        install_action,
        rollback_action,
        name_entry: editor.name_entry,
        light_button,
        dark_button,
        controls: editor.controls,
        selected_color_key: RefCell::new("Foreground".to_owned()),
        selected_color_title: editor.selected_color_title,
        color_picker: editor.color_picker,
        color_targets: editor.color_targets,
        terminal_css_provider,
        terminal_css_scope,
        preview_content: preview.content,
        terminal_title: preview.terminal_title,
        preview_terminal_shell: preview.terminal_shell,
        inspect_button: preview.inspect_button,
        inspect_layer: preview.inspect_layer,
        inspect_label: preview.inspect_label,
        inspect_highlight: preview.inspect_highlight,
        preview_terminal_tab: preview.terminal_tab,
        preview_terminal: preview.terminal,
        preview_terminal_canvas: preview.terminal_canvas,
        preview_terminal_viewport: preview.terminal_viewport,
        preview_terminal_scrollbar_revealer: preview.terminal_scrollbar_revealer,
        preview_scroll: preview.scroll,
        preview_selector: preview.selector,
        preview_scene_selector: preview.scene_selector,
        full_session: preview.full_session,
        prompt_preview_selector: preview.prompt_selector,
        prompt_compare_selector: preview.compare_selector,
        appearance_source: preview.appearance_source,
        appearance_details: preview.appearance_details,
        preview_context_source: preview.context_source,
        fit_preview_switch: preview.fit_switch,
        font_family_selector: typography.font_family_selector,
        font_size_input: typography.font_size_input,
        font_weight_selector: typography.font_weight_selector,
        line_height_input: typography.line_height_input,
        cell_width_input: typography.cell_width_input,
        nerd_status_label: typography.nerd_status_label,
        typography_status: typography.status,
        typography_store: RefCell::new(typography_store),
        typography_baseline: RefCell::new(initial_typography.clone()),
        last_typography: RefCell::new(initial_typography.clone()),
        typography_history: RefCell::new(EditHistory::new(EDIT_HISTORY_LIMIT)),
        typography_has_saved: Cell::new(typography_has_saved),
        layout_status: layout.status,
        layout_store: RefCell::new(layout_store),
        layout_baseline: Cell::new(initial_layout),
        last_layout: Cell::new(initial_layout),
        layout_history: RefCell::new(EditHistory::new(EDIT_HISTORY_LIMIT)),
        layout_has_saved: Cell::new(saved_layout.is_some()),
        workspace_store: RefCell::new(None),
        workspace_baseline: RefCell::new(None),
        workspace_prompt_loaded: Cell::new(false),
        content_padding_input: layout.content_padding_input,
        column_count_input: layout.column_count_input,
        row_count_input: layout.row_count_input,
        cursor_shape_selector: layout.cursor_shape_selector,
        cursor_blink_selector: layout.cursor_blink_selector,
        tab_bar_switch: layout.tab_bar_switch,
        scrollbar_switch: layout.scrollbar_switch,
        window_spacing_input: layout.window_spacing_input,
        prompt_preset_label: prompt.preset_label,
        prompt_preset_popover: prompt.preset_popover,
        prompt_preset_buttons: prompt.preset_buttons,
        prompt_module_list: prompt.module_list,
        prompt_module_count: prompt.module_count,
        prompt_empty_state: prompt.empty_state,
        prompt_module_rows: prompt.module_rows,
        prompt_module_select_buttons: prompt.module_select_buttons,
        prompt_module_samples: prompt.module_samples,
        prompt_module_move_up_buttons: prompt.module_move_up_buttons,
        prompt_module_move_down_buttons: prompt.module_move_down_buttons,
        prompt_module_remove_buttons: prompt.module_remove_buttons,
        prompt_add_button: prompt.add_button,
        prompt_add_popover: prompt.add_popover,
        prompt_add_module_buttons: prompt.add_module_buttons,
        prompt_inspector: prompt.inspector,
        prompt_inspector_title: prompt.inspector_title,
        prompt_tone_selector: prompt.tone_selector,
        prompt_character_selector: prompt.character_selector,
        prompt_layout_selector: prompt.layout_selector,
        prompt_hostname_selector: prompt.hostname_selector,
        prompt_spacing_switch: prompt.spacing_switch,
        prompt_source_selector: prompt.source_selector,
        prompt_import_panel: prompt.import_panel,
        prompt_design_panel: prompt.design_panel,
        prompt_import_status: prompt.import_status,
        starship_editor: prompt.copy_editor,
        copy_generation: Cell::new(0),
        copy_loading: Cell::new(false),
        copy_preview: RefCell::new(None),
        copy_rendered_source: RefCell::new(None),
        copy_prompt_key: RefCell::new(None),
        copy_notices: RefCell::new(Vec::new()),
        copy_initial_sample: RefCell::new(None),
        copy_scene: RefCell::new(None),
        copy_scene_history: RefCell::new(VecDeque::new()),
        scene_diagnostics: RefCell::new(PromptDiagnostics::default()),
        used_prompt_characters: RefCell::new(BTreeSet::new()),
        prompt_diagnostics: RefCell::new(PromptDiagnostics::default()),
        reported_issues: RefCell::new(None),
        diagnostics_pending: Cell::new(false),
        body_ratio: preview.body_ratio,
        composer_ratio: preview.composer_ratio,
        summary_icon: preview.summary_icon,
        summary_title: preview.summary_title,
        diagnostic_header: preview.diagnostic_header,
        diagnostic_surface: preview.diagnostic_surface,
        diagnostics: preview.diagnostics,
        ptyxis_installer: PtyxisInstaller::from_environment().ok(),
        model: RefCell::new(Model::new(
            palette,
            active_variant,
            current_path,
            source_label,
        )),
        preview_input: RefCell::new(PreviewInput::default()),
        current_preview_context: RefCell::new(None),
        preview_directory: RefCell::new(
            std::env::current_dir().unwrap_or_else(|_| glib::home_dir()),
        ),
        preview_generation: Cell::new(0),
        preview_loading: Cell::new(false),
        updating_geometry: Cell::new(false),
        geometry_pending: Cell::new(false),
        preview_map: RefCell::new(PreviewMap::default()),
        preview_uses_prompt: Cell::new(false),
        preview_prompt_source: Cell::new(0),
        preview_feed: RefCell::new(Vec::new()),
        prompt_preview_base: RefCell::new(None),
        prompt_initial_ansi: RefCell::new("$ ".into()),
        designer_original: RefCell::new(None),
        navigating_preview: Cell::new(false),
        inspect_generation: Cell::new(0),
        inspect_hit: RefCell::new(None),
        inspect_pointer: Cell::new(None),
        inspect_hover_pending: Cell::new(false),
        inspect_origin: Cell::new(None),
        last_exported_prompt: RefCell::new(initial_prompt.clone()),
        prompt_settings: RefCell::new(initial_prompt),
        selected_prompt_kind: Cell::new(Some(PromptSegmentKind::Directory)),
        prompt_history: RefCell::new(EditHistory::new(EDIT_HISTORY_LIMIT)),
        history: RefCell::new(EditHistory::new(EDIT_HISTORY_LIMIT)),
        name_valid: Cell::new(true),
        updating: Cell::new(false),
        updating_prompt: Cell::new(false),
    });

    Workbench::install_actions(&workbench);
    Workbench::install_document_actions(&workbench);
    Workbench::connect_signals(&workbench);
    Workbench::connect_greeting_motion(&workbench);
    workbench.refresh_all();
    workbench.show_appearance_source(&appearance);
    workbench
        .preview_terminal
        .set_bold_is_bright(appearance.bold_is_bright);
    workbench
        .preview_terminal
        .set_cjk_ambiguous_width(appearance.cjk_ambiguous_width);
    workbench.refresh_current_context();
    // VTE only animates its cursor while focused. Make the harmless local
    // scratch terminal the initial focus so the preview feels alive on first
    // presentation; interacting with any editor control moves focus normally.
    gtk::prelude::GtkWindowExt::set_focus(&window, Some(&workbench.preview_terminal));
    if let Some(notice) = startup_notice {
        workbench.toast(&notice);
    }
    if let Some(notice) = typography_notice {
        workbench.toast(&notice);
    }
    if let Some(notice) = layout_notice {
        workbench.toast(&notice);
    }
    if let Some(path) = initial_document {
        workbench.open_design_path(&path);
    } else {
        workbench.initialize_design();
    }
    workbench.schedule_fastfetch_sync();
    // The window owns the controller. The controller only keeps a weak window
    // reference, so closing the window releases the complete object graph.
    unsafe {
        window.set_data("termimochi-workbench", workbench);
    }
    window.present();
}

struct PreviewWidgets {
    root: gtk::Overlay,
    content: gtk::Box,
    terminal_title: gtk::Label,
    terminal_shell: gtk::Box,
    inspect_button: gtk::ToggleButton,
    inspect_layer: gtk::Overlay,
    inspect_label: gtk::Label,
    inspect_highlight: gtk::DrawingArea,
    terminal_tab: gtk::Box,
    terminal: vte::Terminal,
    terminal_canvas: gtk::Box,
    terminal_viewport: gtk::ScrolledWindow,
    terminal_scrollbar_revealer: gtk::Revealer,
    scroll: Rc<preview_scroll::PreviewScroll>,
    selector: gtk::DropDown,
    scene_selector: gtk::DropDown,
    full_session: full_session::FullSession,
    prompt_selector: gtk::DropDown,
    compare_selector: gtk::DropDown,
    appearance_source: gtk::Label,
    appearance_details: gtk::Label,
    context_source: gtk::Label,
    fit_switch: gtk::Switch,
    body_ratio: gtk::Label,
    composer_ratio: gtk::Label,
    summary_icon: gtk::Box,
    summary_title: gtk::Label,
    diagnostic_header: gtk::Box,
    diagnostic_surface: gtk::Box,
    diagnostics: gtk::ListBox,
}

#[derive(Clone, Debug, PartialEq)]
struct PreviewHit {
    target: PreviewTarget,
    bounds: gtk::graphene::Rect,
}

struct EditorWidgets {
    root: gtk::ScrolledWindow,
    name_entry: gtk::Entry,
    controls: BTreeMap<String, ColorControl>,
    selected_color_title: gtk::Label,
    color_picker: ColorPicker,
    color_targets: color_targets::ColorTargets,
}

struct TypographyWidgets {
    root: gtk::ScrolledWindow,
    font_family_selector: gtk::DropDown,
    font_size_input: gtk::SpinButton,
    font_weight_selector: gtk::DropDown,
    line_height_input: gtk::SpinButton,
    cell_width_input: gtk::SpinButton,
    nerd_status_label: gtk::Label,
    status: gtk::Label,
}

struct LayoutWidgets {
    root: gtk::ScrolledWindow,
    content_padding_input: gtk::SpinButton,
    column_count_input: gtk::SpinButton,
    row_count_input: gtk::SpinButton,
    cursor_shape_selector: gtk::DropDown,
    cursor_blink_selector: gtk::DropDown,
    tab_bar_switch: gtk::Switch,
    scrollbar_switch: gtk::Switch,
    window_spacing_input: gtk::SpinButton,
    status: gtk::Label,
}

struct PromptWidgets {
    root: gtk::ScrolledWindow,
    source_selector: gtk::DropDown,
    import_panel: gtk::Box,
    design_panel: gtk::Box,
    import_status: gtk::Label,
    copy_editor: Rc<StarshipEditor>,
    preset_label: gtk::Label,
    preset_popover: gtk::Popover,
    preset_buttons: Vec<gtk::Button>,
    module_list: gtk::Box,
    module_count: gtk::Label,
    empty_state: gtk::Box,
    module_rows: Vec<gtk::Box>,
    module_select_buttons: Vec<gtk::ToggleButton>,
    module_samples: Vec<gtk::Label>,
    module_move_up_buttons: Vec<gtk::Button>,
    module_move_down_buttons: Vec<gtk::Button>,
    module_remove_buttons: Vec<gtk::Button>,
    add_button: gtk::MenuButton,
    add_popover: gtk::Popover,
    add_module_buttons: Vec<gtk::Button>,
    inspector: gtk::Box,
    inspector_title: gtk::Label,
    tone_selector: gtk::DropDown,
    character_selector: gtk::DropDown,
    layout_selector: gtk::DropDown,
    hostname_selector: gtk::DropDown,
    spacing_switch: gtk::Switch,
}

struct PromptSegmentRow {
    root: gtk::Box,
    select_button: gtk::ToggleButton,
    sample: gtk::Label,
    move_up_button: gtk::Button,
    move_down_button: gtk::Button,
    remove_button: gtk::Button,
}

struct EditorWorkspaceWidgets {
    root: gtk::Box,
    palette_button: gtk::ToggleButton,
    typography_button: gtk::ToggleButton,
    layout_button: gtk::ToggleButton,
    prompt_button: gtk::ToggleButton,
    greeting_button: gtk::ToggleButton,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditorModule {
    Palette,
    Typography,
    Layout,
    Prompt,
    Greeting,
}

impl EditorModule {
    const ALL: [Self; 5] = [
        Self::Palette,
        Self::Typography,
        Self::Layout,
        Self::Prompt,
        Self::Greeting,
    ];

    const fn stack_name(self) -> &'static str {
        match self {
            Self::Palette => "palette",
            Self::Typography => "typography",
            Self::Layout => "layout",
            Self::Prompt => "prompt",
            Self::Greeting => "greeting",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Palette => "Palette",
            Self::Typography => "Typography",
            Self::Layout => "Layout",
            Self::Prompt => "Prompt",
            Self::Greeting => "Greeting",
        }
    }

    const fn icon_name(self) -> &'static str {
        match self {
            Self::Palette => "preferences-color-symbolic",
            Self::Typography => "font-x-generic-symbolic",
            Self::Layout => "termimochi-layout-symbolic",
            Self::Prompt => "termimochi-prompt-symbolic",
            Self::Greeting => "face-smile-symbolic",
        }
    }

    const fn action_name(self) -> &'static str {
        match self {
            Self::Palette => "show-palette",
            Self::Typography => "show-typography",
            Self::Layout => "show-layout",
            Self::Prompt => "show-prompt",
            Self::Greeting => "show-greeting",
        }
    }

    const fn shortcut(self) -> &'static str {
        match self {
            Self::Palette => "Control+1",
            Self::Typography => "Control+2",
            Self::Layout => "Control+3",
            Self::Prompt => "Control+4",
            Self::Greeting => "Control+5",
        }
    }

    const fn shortcut_hint(self) -> &'static str {
        match self {
            Self::Palette => "Ctrl+1",
            Self::Typography => "Ctrl+2",
            Self::Layout => "Ctrl+3",
            Self::Prompt => "Ctrl+4",
            Self::Greeting => "Ctrl+5",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Palette => "Show the Palette editor",
            Self::Typography => "Show the Typography editor",
            Self::Layout => "Show the Layout editor",
            Self::Prompt => "Show the Prompt editor",
            Self::Greeting => "Show the Greeting editor",
        }
    }
}

fn build_editor() -> EditorWidgets {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.add_css_class("termimochi-editor");
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(15);
    content.set_margin_end(15);

    let heading = gtk::Label::new(Some("Palette"));
    heading.set_xalign(0.0);
    heading.add_css_class("preview-heading");

    let identity = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    identity.add_css_class("palette-identity");
    let identity_label = gtk::Label::new(Some("Theme"));
    identity_label.add_css_class("identity-label");
    let name_entry = gtk::Entry::builder()
        .hexpand(true)
        .valign(gtk::Align::Center)
        .enable_undo(false)
        // The core accepts 128 characters. Preserve one extra character so
        // overlong input is visibly invalid rather than silently accepted
        // after truncation, while keeping pasted input bounded.
        .max_length(129)
        .placeholder_text("Theme name")
        .css_classes(["palette-name-entry"])
        .build();
    name_entry.update_property(&[gtk::accessible::Property::Label("Theme Name")]);
    identity.append(&heading);
    identity.append(&identity_label);
    identity.append(&name_entry);
    content.append(&identity);
    let color_targets = color_targets::ColorTargets::new();
    content.append(&color_targets.root);

    let mut controls = BTreeMap::new();

    let basics_label = gtk::Label::new(Some("Core Colors"));
    basics_label.set_xalign(0.0);
    basics_label.add_css_class("section-heading");
    content.append(&basics_label);
    let basics_grid = gtk::Grid::builder()
        .column_spacing(8)
        .column_homogeneous(true)
        .row_spacing(6)
        .build();
    let mut swatch_group: Option<gtk::ToggleButton> = None;
    for (index, (key, title, detail)) in BASIC_COLORS.into_iter().enumerate() {
        let tile = gtk::Box::new(gtk::Orientation::Vertical, 5);
        let swatch = ColorSwatch::new(false, &format!("{key} · {detail}"));
        if let Some(group) = &swatch_group {
            swatch.button().set_group(Some(group));
        } else {
            swatch_group = Some(swatch.button().clone());
        }
        let label = gtk::Label::new(Some(title));
        label.add_css_class("palette-label");
        tile.append(swatch.button());
        tile.append(&label);
        basics_grid.attach(&tile, index as i32, 0, 1, 1);
        controls.insert(key.to_owned(), ColorControl { swatch });
    }
    content.append(&basics_grid);

    let ansi_grid = gtk::Grid::builder()
        .column_spacing(6)
        .row_spacing(7)
        .build();
    for (row, label_text) in ["Normal", "Bright"].into_iter().enumerate() {
        let label = gtk::Label::new(Some(label_text));
        label.set_xalign(1.0);
        label.add_css_class("palette-row-label");
        ansi_grid.attach(&label, 0, row as i32, 1, 1);
    }
    for (index, name) in ANSI_NAMES.iter().enumerate() {
        let key = format!("Color{index}");
        let swatch = ColorSwatch::new(true, &format!("{key} · {name}"));
        if let Some(group) = &swatch_group {
            swatch.button().set_group(Some(group));
        }
        ansi_grid.attach(
            swatch.button(),
            (index % 8 + 1) as i32,
            (index / 8) as i32,
            1,
            1,
        );
        controls.insert(key, ColorControl { swatch });
    }
    let ansi_section = gtk::Box::new(gtk::Orientation::Vertical, 9);
    ansi_section.add_css_class("ansi-palette");
    let ansi_label = gtk::Label::new(Some("Terminal Colors"));
    ansi_label.set_xalign(0.0);
    ansi_label.add_css_class("section-heading");
    ansi_section.append(&ansi_label);
    ansi_section.append(&ansi_grid);
    content.append(&ansi_section);

    let selected_color_title = gtk::Label::new(Some("Text · Foreground"));
    selected_color_title.set_hexpand(true);
    selected_color_title.set_xalign(0.0);
    selected_color_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    selected_color_title.add_css_class("section-heading");
    content.append(&selected_color_title);
    let color_picker = ColorPicker::new(Rgb::new(0, 0, 0));
    content.append(color_picker.widget());

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        // The Activity Rail is part of the existing 430px editor allocation.
        // Keep the module's own minimum below that shared allocation.
        .min_content_width(330)
        .css_classes(["editor-scroll"])
        .child(&content)
        .build();
    EditorWidgets {
        root: scroll,
        name_entry,
        controls,
        selected_color_title,
        color_picker,
        color_targets,
    }
}

fn build_typography_editor(
    terminal: &vte::Terminal,
    initial_typography: &TypographySettings,
) -> TypographyWidgets {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.add_css_class("termimochi-typography-pane");
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(15);
    content.set_margin_end(15);

    let typography_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    typography_header.add_css_class("typography-header");
    typography_header.set_valign(gtk::Align::Center);
    let typography_heading = gtk::Label::new(Some("Typography"));
    typography_heading.set_xalign(0.0);
    typography_heading.set_hexpand(true);
    typography_heading.set_valign(gtk::Align::Center);
    typography_heading.add_css_class("preview-heading");
    typography_heading.add_css_class("typography-title");
    let nerd_status_label = gtk::Label::new(Some("Checking Nerd Font…"));
    nerd_status_label.set_xalign(1.0);
    nerd_status_label.set_valign(gtk::Align::Center);
    nerd_status_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    nerd_status_label.add_css_class("nerd-status-label");
    typography_header.append(&typography_heading);
    typography_header.append(&nerd_status_label);
    content.append(&typography_header);

    let (font_families, default_font_index) = monospace_font_choices(terminal, initial_typography);
    let family_names: Vec<_> = font_families.iter().map(String::as_str).collect();
    let font_family_selector = gtk::DropDown::from_strings(&family_names);
    font_family_selector.set_selected(default_font_index);
    // Custom font specimens still need a string expression for native search.
    // Merely enabling the search entry leaves the font list unfiltered.
    let font_search = gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<&gtk::Expression>,
        "string",
    );
    font_family_selector.set_expression(Some(&font_search));
    font_family_selector.set_enable_search(true);
    font_family_selector.set_hexpand(true);
    font_family_selector.add_css_class("typography-control");
    font_family_selector.add_css_class("font-family-control");
    let selected_font_factory = font_family_factory("font-family-selection");
    font_family_selector.set_factory(Some(&selected_font_factory));
    let font_list_factory = font_family_factory("font-family-option");
    font_family_selector.set_list_factory(Some(&font_list_factory));
    font_family_selector.set_tooltip_text(Some(
        "Save remembers this font; Apply in the bottom bar reviews terminal changes.",
    ));
    font_family_selector.update_property(&[
        gtk::accessible::Property::Label("Font Family"),
        gtk::accessible::Property::Description(
            "Choose an installed monospace font for the terminal preview",
        ),
    ]);

    let font_size_input = typography_spin_button(
        initial_typography.size,
        MIN_FONT_SIZE,
        MAX_FONT_SIZE,
        0.5,
        1,
        "Font Size",
        "Font size in points",
    );
    font_size_input.add_css_class("font-size-control");

    let weight_labels = PreviewFontWeight::ALL.map(PreviewFontWeight::label);
    let font_weight_selector = gtk::DropDown::from_strings(&weight_labels);
    font_weight_selector.set_selected(initial_typography.weight.index());
    font_weight_selector.set_hexpand(true);
    font_weight_selector.add_css_class("typography-control");
    font_weight_selector.add_css_class("font-weight-control");
    font_weight_selector.set_tooltip_text(Some("Preview font weight"));
    font_weight_selector.update_property(&[gtk::accessible::Property::Label("Font Weight")]);

    let line_height_input = typography_spin_button(
        initial_typography.line_height,
        MIN_CELL_SCALE,
        MAX_CELL_SCALE,
        0.05,
        2,
        "Line Height",
        "Terminal cell height scale from 1.00 to 2.00",
    );
    let cell_width_input = typography_spin_button(
        initial_typography.cell_width,
        MIN_CELL_SCALE,
        MAX_CELL_SCALE,
        0.05,
        2,
        "Cell Width",
        "Terminal cell width scale from 1.00 to 2.00",
    );

    let typography_fields = gtk::Box::new(gtk::Orientation::Vertical, 10);
    typography_fields.add_css_class("typography-fields");
    for field in [
        typography_field("Font Family", &font_family_selector),
        typography_field("Font Size", &font_size_input),
        typography_field("Font Weight", &font_weight_selector),
        typography_field("Line Height", &line_height_input),
        typography_field("Cell Width", &cell_width_input),
    ] {
        typography_fields.append(&field);
    }
    content.append(&typography_fields);

    // Saving and applying remain in the fixed output bar.
    let status = gtk::Label::new(Some("Preview only · not saved"));
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.add_css_class("nerd-status-label");
    content.append(&status);

    // Typography inputs remain keyboard/click editable, while wheel and
    // touchpad gestures move this module instead of changing values.
    let typography_scroll = gtk::EventControllerScroll::new(
        gtk::EventControllerScrollFlags::VERTICAL | gtk::EventControllerScrollFlags::HORIZONTAL,
    );
    typography_scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
    let scroll_source = content.clone();
    typography_scroll.connect_scroll(move |controller, _, delta_y| {
        scroll_parent_vertically(&scroll_source, delta_y, controller.unit());
        glib::Propagation::Stop
    });
    content.add_controller(typography_scroll);

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(330)
        .css_classes(["typography-scroll"])
        .child(&content)
        .build();

    TypographyWidgets {
        root: scroll,
        font_family_selector,
        font_size_input,
        font_weight_selector,
        line_height_input,
        cell_width_input,
        nerd_status_label,
        status,
    }
}

fn build_layout_editor(defaults: &LayoutSettings) -> LayoutWidgets {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 15);
    content.add_css_class("termimochi-layout-pane");
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(15);
    content.set_margin_end(15);

    let heading = gtk::Label::new(Some("Layout"));
    heading.set_xalign(0.0);
    heading.set_valign(gtk::Align::Center);
    heading.add_css_class("preview-heading");
    heading.add_css_class("layout-title");
    content.append(&heading);

    let content_padding_input = layout_spin_button(
        defaults.content_padding,
        MIN_CONTENT_PADDING,
        MAX_CONTENT_PADDING,
        "Content Padding",
        "Space in pixels between terminal text and the terminal edge",
    );
    let column_count_input = layout_spin_button(
        i32::try_from(defaults.columns).expect("default columns fit i32"),
        i32::try_from(MIN_COLUMNS).expect("minimum columns fit i32"),
        i32::try_from(MAX_COLUMNS).expect("maximum columns fit i32"),
        "Columns",
        "Number of columns in the terminal preview",
    );
    let row_count_input = layout_spin_button(
        i32::try_from(defaults.rows).expect("default rows fit i32"),
        i32::try_from(MIN_ROWS).expect("minimum rows fit i32"),
        i32::try_from(MAX_ROWS).expect("maximum rows fit i32"),
        "Rows",
        "Number of rows in the terminal preview",
    );
    content.append(&layout_group(
        "Terminal",
        [
            layout_row("Content Padding", &content_padding_input),
            layout_row("Columns", &column_count_input),
            layout_row("Rows", &row_count_input),
        ],
    ));

    let cursor_shape_labels = PreviewCursorShape::ALL.map(PreviewCursorShape::label);
    let cursor_shape_selector = layout_drop_down(
        &cursor_shape_labels,
        defaults.cursor_shape.index(),
        "Cursor Shape",
        "Shape of the cursor shown in the terminal preview",
    );
    let cursor_blink_labels = PreviewCursorBlink::ALL.map(PreviewCursorBlink::label);
    let cursor_blink_selector = layout_drop_down(
        &cursor_blink_labels,
        defaults.cursor_blink.index(),
        "Cursor Blink",
        "Use the system cursor blink setting, force blinking on, or keep it off",
    );
    content.append(&layout_group(
        "Cursor",
        [
            layout_row("Shape", &cursor_shape_selector),
            layout_row("Blink", &cursor_blink_selector),
        ],
    ));

    let tab_bar_switch = layout_switch(
        defaults.tab_bar,
        "Tab Bar",
        "Show the terminal tab title in the preview",
    );
    let scrollbar_switch = layout_switch(
        defaults.scrollbar,
        "Scrollbar",
        "Show a scrollbar for terminal content and history",
    );
    let window_spacing_input = layout_spin_button(
        defaults.window_spacing,
        MIN_WINDOW_SPACING,
        MAX_WINDOW_SPACING,
        "Window Spacing",
        "Whitespace in pixels around the live preview",
    );
    content.append(&layout_group(
        "Window",
        [
            layout_row("Tab Bar", &tab_bar_switch),
            layout_row("Scrollbar", &scrollbar_switch),
            layout_row("Window Spacing", &window_spacing_input),
        ],
    ));

    // Saving and applying remain in the fixed output bar.
    let status = gtk::Label::new(Some("Preview only · not saved"));
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.add_css_class("nerd-status-label");
    content.append(&status);
    content_padding_input.set_tooltip_text(Some(
        "Exact padding is saved with the preset but is preview-only in Ptyxis",
    ));
    tab_bar_switch.set_tooltip_text(Some(
        "Saved preview setting; Ptyxis manages tab bar visibility itself",
    ));
    window_spacing_input
        .set_tooltip_text(Some("Saved preview setting, not a Ptyxis window setting"));

    // Preserve deliberate numeric values while letting wheel and touchpad
    // gestures move the module, matching the Typography editor behavior.
    let layout_scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    layout_scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
    let scroll_source = content.clone();
    layout_scroll.connect_scroll(move |controller, _, delta_y| {
        scroll_parent_vertically(&scroll_source, delta_y, controller.unit());
        glib::Propagation::Stop
    });
    content.add_controller(layout_scroll);

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(330)
        .css_classes(["layout-scroll"])
        .child(&content)
        .build();

    LayoutWidgets {
        root: scroll,
        content_padding_input,
        column_count_input,
        row_count_input,
        cursor_shape_selector,
        cursor_blink_selector,
        tab_bar_switch,
        scrollbar_switch,
        window_spacing_input,
        status,
    }
}

fn build_prompt_editor(settings: &PromptSettings) -> PromptWidgets {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.add_css_class("termimochi-prompt-pane");
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(15);
    content.set_margin_end(15);

    let heading = gtk::Label::new(Some("Shell Prompt"));
    heading.set_xalign(0.0);
    heading.set_valign(gtk::Align::Center);
    heading.add_css_class("preview-heading");
    heading.add_css_class("prompt-title");
    heading.set_hexpand(true);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    header.set_hexpand(true);
    header.set_valign(gtk::Align::Center);
    header.add_css_class("prompt-header");
    header.append(&heading);
    // Starship output actions use the same fixed bar as other modules.
    content.append(&header);

    let source_selector = gtk::DropDown::from_strings(&["Your Starship", "Designer"]);
    source_selector.add_css_class("prompt-property-control");
    source_selector.update_property(&[gtk::accessible::Property::Label("Prompt Source")]);
    content.append(&layout_row("Prompt Source", &source_selector));
    let import_panel = gtk::Box::new(gtk::Orientation::Vertical, 14);
    import_panel.set_margin_top(12);
    let import_heading = gtk::Label::new(Some("Your Starship"));
    import_heading.set_xalign(0.0);
    import_heading.add_css_class("section-heading");
    import_panel.append(&import_heading);
    let import_status = gtk::Label::new(Some("Reading starship.toml…"));
    import_status.set_xalign(0.0);
    import_status.set_wrap(true);
    import_status.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    import_status.set_max_width_chars(32);
    import_status.add_css_class("preview-source-detail");
    import_status.update_property(&[gtk::accessible::Property::Label("Imported Starship Status")]);
    import_panel.append(&import_status);
    let reload = gtk::Button::builder()
        .label("Reload Starship")
        .action_name("win.reload-starship")
        .css_classes(["prompt-menu-item"])
        .build();
    import_panel.append(&reload);
    let note = gtk::Label::new(Some(
        "Your configuration opens here automatically. Use Designer to create a new prompt.",
    ));
    note.set_xalign(0.0);
    note.set_wrap(true);
    note.set_max_width_chars(32);
    note.add_css_class("preview-source-detail");
    import_panel.append(&note);
    content.append(&import_panel);
    let page = content;
    let copy_editor = StarshipEditor::new();
    page.append(&copy_editor.root);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_visible(false);
    page.append(&content);

    let preset_label = gtk::Label::new(Some(
        settings
            .source_preset()
            .map_or("Custom", PromptPreset::label),
    ));
    preset_label.set_xalign(0.0);
    let preset_chevron = gtk::Image::builder()
        .icon_name("pan-down-symbolic")
        .pixel_size(12)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    let preset_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    preset_content.append(&preset_label);
    preset_content.append(&preset_chevron);
    let preset_popover = gtk::Popover::new();
    preset_popover.add_css_class("prompt-menu-popover");
    let preset_menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    preset_menu.set_margin_top(6);
    preset_menu.set_margin_bottom(6);
    preset_menu.set_margin_start(6);
    preset_menu.set_margin_end(6);
    let preset_buttons = PromptPreset::ALL
        .into_iter()
        .map(|preset| {
            let button = gtk::Button::with_label(preset.label());
            button.set_halign(gtk::Align::Fill);
            button.set_tooltip_text(Some(&format!(
                "Replace the prompt with the {} starting point",
                preset.label()
            )));
            button.add_css_class("prompt-menu-item");
            preset_menu.append(&button);
            button
        })
        .collect::<Vec<_>>();
    preset_popover.set_child(Some(&preset_menu));
    let preset_button = gtk::MenuButton::builder()
        .child(&preset_content)
        .tooltip_text("Choose a starting point")
        .css_classes(["prompt-starter-button"])
        .build();
    preset_button.set_popover(Some(&preset_popover));
    preset_button.update_property(&[
        gtk::accessible::Property::Label("Prompt starting point"),
        gtk::accessible::Property::Description(
            "Choose a starting prompt. Applying it resets structure and appearance; later edits create a custom prompt",
        ),
    ]);

    let starter_title = gtk::Label::new(Some("Starting Point"));
    starter_title.set_xalign(0.0);
    starter_title.set_hexpand(true);
    starter_title.add_css_class("prompt-starter-title");
    let starter_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    starter_row.set_hexpand(true);
    starter_row.set_valign(gtk::Align::Center);
    starter_row.add_css_class("prompt-starter-row");
    starter_row.append(&starter_title);
    starter_row.append(&preset_button);
    content.append(&starter_row);

    let add_icon = gtk::Image::builder()
        .icon_name("list-add-symbolic")
        .pixel_size(13)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    let add_label = gtk::Label::new(Some("Add Module"));
    let add_content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    add_content.append(&add_icon);
    add_content.append(&add_label);
    let add_popover = gtk::Popover::new();
    add_popover.add_css_class("prompt-catalog-popover");
    let catalog = gtk::Box::new(gtk::Orientation::Vertical, 3);
    catalog.set_margin_top(8);
    catalog.set_margin_bottom(8);
    catalog.set_margin_start(8);
    catalog.set_margin_end(8);
    let mut previous_category = "";
    let mut add_module_buttons = Vec::with_capacity(PromptSegmentKind::ALL.len());
    for kind in PromptSegmentKind::ALL {
        if kind.category() != previous_category {
            let category = gtk::Label::new(Some(kind.category()));
            category.set_xalign(0.0);
            category.add_css_class("prompt-catalog-category");
            if !previous_category.is_empty() {
                category.set_margin_top(5);
            }
            catalog.append(&category);
            previous_category = kind.category();
        }
        let label = gtk::Label::new(Some(kind.label()));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        let sample = gtk::Label::new(Some(kind.sample()));
        sample.set_xalign(1.0);
        sample.set_ellipsize(gtk::pango::EllipsizeMode::End);
        sample.add_css_class("prompt-catalog-sample");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.append(&label);
        row.append(&sample);
        let button = gtk::Button::builder()
            .child(&row)
            .hexpand(true)
            .tooltip_text(kind.description())
            .css_classes(["prompt-catalog-item"])
            .build();
        button.update_property(&[
            gtk::accessible::Property::Label(kind.label()),
            gtk::accessible::Property::Description(&format!(
                "{}. Example: {}",
                kind.description(),
                kind.sample()
            )),
        ]);
        catalog.append(&button);
        add_module_buttons.push(button);
    }
    let catalog_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .min_content_width(280)
        .max_content_height(390)
        .propagate_natural_height(true)
        .child(&catalog)
        .build();
    add_popover.set_child(Some(&catalog_scroll));
    let add_button = gtk::MenuButton::builder()
        .child(&add_content)
        .tooltip_text("Add a prompt module")
        .css_classes(["prompt-add-button"])
        .build();
    add_button.set_popover(Some(&add_popover));
    add_button.update_property(&[
        gtk::accessible::Property::Label("Add Prompt Module"),
        gtk::accessible::Property::Description(
            "Open the categorized module library and add a module at the end",
        ),
    ]);

    let module_count = gtk::Label::new(None);
    module_count.add_css_class("prompt-module-count");
    let structure_title = gtk::Label::new(Some("Prompt Structure"));
    structure_title.set_xalign(0.0);
    structure_title.set_hexpand(true);
    structure_title.add_css_class("layout-group-title");
    let structure_header = gtk::Box::new(gtk::Orientation::Horizontal, 7);
    structure_header.set_valign(gtk::Align::Center);
    structure_header.append(&structure_title);
    structure_header.append(&module_count);
    structure_header.append(&add_button);

    let module_list = gtk::Box::new(gtk::Orientation::Vertical, 1);
    module_list.add_css_class("prompt-module-list");
    let segment_rows = PromptSegmentKind::ALL
        .into_iter()
        .map(prompt_segment_row)
        .collect::<Vec<_>>();
    for row in &segment_rows {
        module_list.append(&row.root);
    }
    let module_rows = segment_rows
        .iter()
        .map(|row| row.root.clone())
        .collect::<Vec<_>>();
    let module_select_buttons = segment_rows
        .iter()
        .map(|row| row.select_button.clone())
        .collect::<Vec<_>>();
    if let Some(first) = module_select_buttons.first() {
        for button in module_select_buttons.iter().skip(1) {
            button.set_group(Some(first));
        }
    }
    let module_samples = segment_rows
        .iter()
        .map(|row| row.sample.clone())
        .collect::<Vec<_>>();
    let module_move_up_buttons = segment_rows
        .iter()
        .map(|row| row.move_up_button.clone())
        .collect::<Vec<_>>();
    let module_move_down_buttons = segment_rows
        .iter()
        .map(|row| row.move_down_button.clone())
        .collect::<Vec<_>>();
    let module_remove_buttons = segment_rows
        .iter()
        .map(|row| row.remove_button.clone())
        .collect::<Vec<_>>();

    let empty_mark = gtk::Label::new(Some(">_"));
    empty_mark.add_css_class("prompt-empty-mark");
    let empty_label = gtk::Label::new(Some("No modules yet"));
    empty_label.add_css_class("prompt-empty-label");
    let empty_state = gtk::Box::new(gtk::Orientation::Vertical, 5);
    empty_state.set_valign(gtk::Align::Center);
    empty_state.add_css_class("prompt-empty-state");
    empty_state.append(&empty_mark);
    empty_state.append(&empty_label);

    let structure = gtk::Box::new(gtk::Orientation::Vertical, 5);
    structure.add_css_class("layout-group");
    structure.append(&structure_header);
    structure.append(&module_list);
    structure.append(&empty_state);
    content.append(&structure);

    let tone_labels = PreviewTone::ALL.map(PreviewTone::label);
    let tone_selector = gtk::DropDown::from_strings(&tone_labels);
    tone_selector.set_hexpand(false);
    tone_selector.set_width_request(116);
    tone_selector.add_css_class("prompt-property-control");
    tone_selector.update_property(&[
        gtk::accessible::Property::Label("Module accent"),
        gtk::accessible::Property::Description("Choose the selected module's terminal color"),
    ]);
    let inspector_title = gtk::Label::new(Some("Module Accent"));
    inspector_title.set_xalign(0.0);
    inspector_title.set_hexpand(true);
    inspector_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    inspector_title.add_css_class("layout-row-label");
    let inspector = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    inspector.add_css_class("layout-row");
    inspector.add_css_class("prompt-property-row");
    inspector.append(&inspector_title);
    inspector.append(&tone_selector);

    let character_labels = PromptCharacter::ALL.map(PromptCharacter::label);
    let character_selector = gtk::DropDown::from_strings(&character_labels);
    character_selector.set_hexpand(false);
    character_selector.set_width_request(116);
    character_selector.add_css_class("prompt-property-control");
    character_selector.update_property(&[
        gtk::accessible::Property::Label("Prompt symbol"),
        gtk::accessible::Property::Description("Choose the final input symbol"),
    ]);
    let character_label = gtk::Label::new(Some("Prompt Symbol"));
    character_label.set_xalign(0.0);
    character_label.set_hexpand(true);
    character_label.add_css_class("layout-row-label");
    let character_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    character_row.add_css_class("layout-row");
    character_row.add_css_class("prompt-property-row");
    character_row.append(&character_label);
    character_row.append(&character_selector);

    let appearance_card = gtk::Box::new(gtk::Orientation::Vertical, 1);
    appearance_card.add_css_class("layout-card");
    appearance_card.add_css_class("prompt-properties");
    appearance_card.append(&inspector);
    appearance_card.append(&character_row);
    let layout_labels = PromptLayout::ALL.map(PromptLayout::label);
    let layout_selector = gtk::DropDown::from_strings(&layout_labels);
    layout_selector.add_css_class("prompt-property-control");
    layout_selector.update_property(&[gtk::accessible::Property::Label("Prompt Layout")]);
    appearance_card.append(&layout_row("Prompt Layout", &layout_selector));
    let spacing_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
    spacing_switch.update_property(&[gtk::accessible::Property::Label("Space Between Prompts")]);
    appearance_card.append(&layout_row("Space Between Prompts", &spacing_switch));
    let appearance = gtk::Box::new(gtk::Orientation::Vertical, 5);
    appearance.add_css_class("layout-group");
    let appearance_title = gtk::Label::new(Some("Appearance"));
    appearance_title.set_xalign(0.0);
    appearance_title.add_css_class("layout-group-title");
    appearance.append(&appearance_title);
    appearance.append(&appearance_card);
    content.append(&appearance);

    let hostname_labels = PromptHostnameMode::ALL.map(PromptHostnameMode::label);
    let hostname_selector = gtk::DropDown::from_strings(&hostname_labels);
    hostname_selector.add_css_class("prompt-property-control");
    hostname_selector.update_property(&[gtk::accessible::Property::Label("Host Visibility")]);
    let compatibility = gtk::Box::new(gtk::Orientation::Vertical, 8);
    compatibility.append(&layout_row("Host Visibility", &hostname_selector));
    let compatibility_note = gtk::Label::new(Some(
        "Starship · Bash, Zsh, Fish and more. Choose ASCII > for basic fonts. Designer creates a separate configuration; it does not overwrite your imported prompt.",
    ));
    compatibility_note.set_wrap(true);
    compatibility_note.set_xalign(0.0);
    compatibility_note.set_max_width_chars(36);
    compatibility_note.add_css_class("preview-source-detail");
    compatibility.append(&compatibility_note);
    let compatibility_expander = gtk::Expander::builder()
        .label("Compatibility")
        .child(&compatibility)
        .build();
    content.append(&compatibility_expander);

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(330)
        .css_classes(["prompt-scroll"])
        .child(&page)
        .build();

    PromptWidgets {
        root: scroll,
        source_selector,
        import_panel,
        design_panel: content,
        import_status,
        copy_editor,
        preset_label,
        preset_popover,
        preset_buttons,
        module_list,
        module_count,
        empty_state,
        module_rows,
        module_select_buttons,
        module_samples,
        module_move_up_buttons,
        module_move_down_buttons,
        module_remove_buttons,
        add_button,
        add_popover,
        add_module_buttons,
        inspector,
        inspector_title,
        tone_selector,
        character_selector,
        layout_selector,
        hostname_selector,
        spacing_switch,
    }
}

fn prompt_segment_row(kind: PromptSegmentKind) -> PromptSegmentRow {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    row.set_hexpand(true);
    row.set_valign(gtk::Align::Center);
    row.add_css_class("prompt-module-row");

    let label = gtk::Label::new(Some(kind.label()));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.add_css_class("layout-row-label");

    let sample = gtk::Label::new(Some(kind.sample()));
    sample.set_xalign(1.0);
    sample.set_ellipsize(gtk::pango::EllipsizeMode::End);
    sample.add_css_class("prompt-module-sample");
    let selection_content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    selection_content.append(&label);
    selection_content.append(&sample);
    let select_button = gtk::ToggleButton::builder()
        .child(&selection_content)
        .hexpand(true)
        .tooltip_text(kind.description())
        .css_classes(["prompt-module-select"])
        .build();
    select_button.update_property(&[
        gtk::accessible::Property::Label(kind.label()),
        gtk::accessible::Property::Description(kind.description()),
    ]);

    let move_up_button = prompt_menu_action("Move Up", "go-up-symbolic", false);
    let move_down_button = prompt_menu_action("Move Down", "go-down-symbolic", false);
    let remove_button = prompt_menu_action("Remove", "edit-delete-symbolic", true);
    let actions = gtk::Box::new(gtk::Orientation::Vertical, 2);
    actions.set_margin_top(6);
    actions.set_margin_bottom(6);
    actions.set_margin_start(6);
    actions.set_margin_end(6);
    actions.append(&move_up_button);
    actions.append(&move_down_button);
    actions.append(&remove_button);
    let actions_popover = gtk::Popover::new();
    actions_popover.add_css_class("prompt-menu-popover");
    actions_popover.set_child(Some(&actions));
    let more_button = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(format!("Actions for {}", kind.label()))
        .css_classes(["prompt-module-more"])
        .build();
    more_button.set_popover(Some(&actions_popover));
    more_button.update_property(&[
        gtk::accessible::Property::Label("Module Actions"),
        gtk::accessible::Property::Description(&format!("Move or remove {}", kind.label())),
    ]);

    row.append(&select_button);
    row.append(&more_button);
    PromptSegmentRow {
        root: row,
        select_button,
        sample,
        move_up_button,
        move_down_button,
        remove_button,
    }
}

fn prompt_menu_action(label: &str, icon_name: &str, destructive: bool) -> gtk::Button {
    let icon = gtk::Image::builder()
        .icon_name(icon_name)
        .pixel_size(14)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_hexpand(true);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.append(&icon);
    content.append(&text);
    let button = gtk::Button::builder()
        .child(&content)
        .hexpand(true)
        .css_classes(["prompt-menu-item"])
        .build();
    if destructive {
        button.add_css_class("destructive-action");
    }
    button
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PromptPreviewScenario {
    CurrentFolder,
    Projects,
    Failure,
    Ssh,
    Root,
    Alignment,
}

impl PromptPreviewScenario {
    const ALL: [Self; 6] = [
        Self::CurrentFolder,
        Self::Projects,
        Self::Failure,
        Self::Ssh,
        Self::Root,
        Self::Alignment,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::CurrentFolder => "Current Folder",
            Self::Projects => "Projects",
            Self::Failure => "Failure",
            Self::Ssh => "SSH",
            Self::Root => "Root",
            Self::Alignment => "Alignment",
        }
    }

    fn from_index(index: u32) -> Self {
        Self::ALL
            .get(index as usize)
            .copied()
            .unwrap_or(Self::CurrentFolder)
    }
}

fn prompt_preview_contexts() -> [PromptPreviewContext<'static>; 4] {
    let base = PromptPreviewContext {
        username: "mii",
        hostname: "mochi",
        path: "~/tm",
        git_branch: Some("dev"),
        git_status: Some("!?"),
        rust_version: Some("v1.89"),
        node_version: None,
        python_version: None,
        go_version: None,
        command_duration: Some("8ms"),
        jobs: None,
        time: Some("23:42"),
        exit_status: 0,
        is_root: false,
        is_ssh: false,
    };
    let node = PromptPreviewContext {
        path: "~/web",
        git_branch: None,
        git_status: None,
        rust_version: None,
        node_version: Some("v24"),
        command_duration: Some("1s"),
        ..base
    };
    let go = PromptPreviewContext {
        path: "~/api",
        git_branch: Some("main"),
        git_status: None,
        rust_version: None,
        node_version: None,
        go_version: Some("v1.25"),
        command_duration: Some("24ms"),
        ..base
    };
    let failed = PromptPreviewContext {
        path: "~/py",
        git_branch: Some("fix"),
        git_status: None,
        rust_version: None,
        node_version: None,
        python_version: Some("v3.14"),
        command_duration: None,
        jobs: Some("2"),
        exit_status: 1,
        ..base
    };
    [base, node, go, failed]
}

fn close_containing_popover(button: &gtk::Button) {
    if let Some(widget) = button.ancestor(gtk::Popover::static_type())
        && let Ok(popover) = widget.downcast::<gtk::Popover>()
    {
        popover.popdown();
    }
}

fn layout_spin_button(
    value: i32,
    minimum: i32,
    maximum: i32,
    accessible_label: &str,
    description: &str,
) -> gtk::SpinButton {
    let input = typography_spin_button(
        f64::from(value),
        f64::from(minimum),
        f64::from(maximum),
        1.0,
        0,
        accessible_label,
        description,
    );
    input.set_hexpand(false);
    input.set_width_request(116);
    input.add_css_class("layout-control");
    input
}

fn layout_drop_down(
    labels: &[&str],
    selected: u32,
    accessible_label: &str,
    description: &str,
) -> gtk::DropDown {
    let selector = gtk::DropDown::from_strings(labels);
    selector.set_selected(selected);
    selector.set_width_request(116);
    selector.set_halign(gtk::Align::End);
    selector.add_css_class("typography-control");
    selector.add_css_class("layout-control");
    selector.set_tooltip_text(Some(description));
    selector.update_property(&[
        gtk::accessible::Property::Label(accessible_label),
        gtk::accessible::Property::Description(description),
    ]);
    selector
}

fn layout_switch(active: bool, accessible_label: &str, description: &str) -> gtk::Switch {
    let switch = gtk::Switch::builder()
        .active(active)
        .valign(gtk::Align::Center)
        .halign(gtk::Align::End)
        .build();
    switch.add_css_class("layout-switch");
    switch.set_tooltip_text(Some(description));
    switch.update_property(&[
        gtk::accessible::Property::Label(accessible_label),
        gtk::accessible::Property::Description(description),
    ]);
    switch
}

fn layout_row(title: &str, child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_hexpand(true);
    row.set_valign(gtk::Align::Center);
    row.add_css_class("layout-row");
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.add_css_class("layout-row-label");
    child.set_halign(gtk::Align::End);
    row.append(&label);
    row.append(child);
    row
}

fn layout_group<const N: usize>(title: &str, rows: [gtk::Box; N]) -> gtk::Box {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 5);
    group.add_css_class("layout-group");
    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.add_css_class("layout-group-title");
    group.append(&heading);
    let card = gtk::Box::new(gtk::Orientation::Vertical, 1);
    card.add_css_class("layout-card");
    for row in rows {
        card.append(&row);
    }
    group.append(&card);
    group
}

fn build_activity_item(module: EditorModule) -> (gtk::Overlay, gtk::ToggleButton) {
    let image = gtk::Image::builder()
        .icon_name(module.icon_name())
        .pixel_size(20)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    let button = gtk::ToggleButton::builder()
        .child(&image)
        .width_request(36)
        .height_request(36)
        .halign(gtk::Align::Center)
        .tooltip_text(format!("{}  {}", module.label(), module.shortcut_hint()))
        .css_classes(["activity-button"])
        .build();
    button.update_property(&[
        gtk::accessible::Property::Label(module.label()),
        gtk::accessible::Property::Description(module.description()),
        gtk::accessible::Property::KeyShortcuts(module.shortcut()),
    ]);

    let indicator = gtk::Box::new(gtk::Orientation::Vertical, 0);
    indicator.set_size_request(2, 20);
    indicator.set_halign(gtk::Align::Start);
    indicator.set_valign(gtk::Align::Center);
    indicator.set_can_target(false);
    indicator.set_visible(false);
    indicator.add_css_class("activity-indicator");

    let item = gtk::Overlay::new();
    item.set_width_request(48);
    item.set_child(Some(&button));
    item.add_overlay(&indicator);
    item.add_css_class("activity-item");

    let indicator_ref = indicator.clone();
    button.connect_toggled(move |button| indicator_ref.set_visible(button.is_active()));

    (item, button)
}

fn build_editor_workspace(
    palette: &gtk::ScrolledWindow,
    typography: &gtk::ScrolledWindow,
    layout: &gtk::ScrolledWindow,
    prompt: &gtk::ScrolledWindow,
    greeting: &gtk::ScrolledWindow,
) -> EditorWorkspaceWidgets {
    let stack = gtk::Stack::builder()
        .hexpand(true)
        .vexpand(true)
        .hhomogeneous(true)
        .vhomogeneous(true)
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(120)
        .css_classes(["editor-module-stack"])
        .build();
    stack.add_named(palette, Some(EditorModule::Palette.stack_name()));
    stack.add_named(typography, Some(EditorModule::Typography.stack_name()));
    stack.add_named(layout, Some(EditorModule::Layout.stack_name()));
    stack.add_named(prompt, Some(EditorModule::Prompt.stack_name()));
    stack.add_named(greeting, Some(EditorModule::Greeting.stack_name()));

    let rail = gtk::Box::new(gtk::Orientation::Vertical, 6);
    rail.set_width_request(48);
    rail.set_vexpand(true);
    rail.set_valign(gtk::Align::Fill);
    rail.add_css_class("activity-rail");
    rail.update_property(&[
        gtk::accessible::Property::Label("Editor Modules"),
        gtk::accessible::Property::Description(
            "Switch the editor module without changing the live preview",
        ),
    ]);

    let (palette_item, palette_button) = build_activity_item(EditorModule::Palette);
    let (typography_item, typography_button) = build_activity_item(EditorModule::Typography);
    let (layout_item, layout_button) = build_activity_item(EditorModule::Layout);
    let (prompt_item, prompt_button) = build_activity_item(EditorModule::Prompt);
    let (greeting_item, greeting_button) = build_activity_item(EditorModule::Greeting);
    typography_button.set_group(Some(&palette_button));
    layout_button.set_group(Some(&palette_button));
    prompt_button.set_group(Some(&palette_button));
    greeting_button.set_group(Some(&palette_button));
    rail.append(&palette_item);
    rail.append(&typography_item);
    rail.append(&layout_item);
    rail.append(&prompt_item);
    rail.append(&greeting_item);

    let stack_ref = stack.clone();
    palette_button.connect_toggled(move |button| {
        if button.is_active() {
            stack_ref.set_visible_child_name(EditorModule::Palette.stack_name());
        }
    });
    let stack_ref = stack.clone();
    typography_button.connect_toggled(move |button| {
        if button.is_active() {
            stack_ref.set_visible_child_name(EditorModule::Typography.stack_name());
        }
    });
    let stack_ref = stack.clone();
    layout_button.connect_toggled(move |button| {
        if button.is_active() {
            stack_ref.set_visible_child_name(EditorModule::Layout.stack_name());
        }
    });
    let stack_ref = stack.clone();
    prompt_button.connect_toggled(move |button| {
        if button.is_active() {
            stack_ref.set_visible_child_name(EditorModule::Prompt.stack_name());
        }
    });
    let stack_ref = stack.clone();
    greeting_button.connect_toggled(move |button| {
        if button.is_active() {
            stack_ref.set_visible_child_name(EditorModule::Greeting.stack_name());
        }
    });
    palette_button.set_active(true);
    stack.set_visible_child_name(EditorModule::Palette.stack_name());

    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.set_hexpand(true);
    root.set_vexpand(true);
    root.add_css_class("editor-workspace");
    root.append(&rail);
    root.append(&stack);

    EditorWorkspaceWidgets {
        root,
        palette_button,
        typography_button,
        layout_button,
        prompt_button,
        greeting_button,
    }
}

fn install_editor_module_actions(
    window: &adw::ApplicationWindow,
    workspace: &EditorWorkspaceWidgets,
) {
    for module in EditorModule::ALL {
        let button = match module {
            EditorModule::Palette => workspace.palette_button.clone(),
            EditorModule::Typography => workspace.typography_button.clone(),
            EditorModule::Layout => workspace.layout_button.clone(),
            EditorModule::Prompt => workspace.prompt_button.clone(),
            EditorModule::Greeting => workspace.greeting_button.clone(),
        };
        let action = gio::SimpleAction::new(module.action_name(), None);
        action.connect_activate(move |_, _| {
            if !button.is_sensitive() || !button.is_visible() {
                return;
            }
            if !button.is_active() {
                // Moving focus settles an in-progress field edit before its
                // page is hidden. Repeating the current module shortcut stays
                // a no-op and does not interrupt typing.
                button.grab_focus();
                button.set_active(true);
            }
        });
        window.add_action(&action);
    }
}

fn build_preview(
    variant_switch: &gtk::Box,
    initial_typography: &TypographySettings,
    divider: &gtk::Paned,
) -> PreviewWidgets {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.add_css_class("termimochi-preview-pane");
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(18);
    content.set_margin_end(18);
    content.set_width_request(360);
    content.set_vexpand(true);

    let heading = gtk::Label::new(Some("Design Preview"));
    heading.set_xalign(0.0);
    heading.set_valign(gtk::Align::Center);
    heading.set_hexpand(true);
    heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
    heading.add_css_class("preview-heading");
    let appearance_source = gtk::Label::new(Some("Reading terminal appearance…"));
    appearance_source.set_xalign(0.0);
    appearance_source.add_css_class("layout-group-title");
    let appearance_details = gtk::Label::new(None);
    appearance_details.set_xalign(0.0);
    appearance_details.set_wrap(true);
    appearance_details.set_max_width_chars(38);
    appearance_details.add_css_class("preview-source-detail");
    let context_source = gtk::Label::new(Some("Reading current folder…"));
    context_source.set_xalign(0.0);
    context_source.set_wrap(true);
    context_source.set_max_width_chars(38);
    context_source.add_css_class("preview-source-detail");
    let fit_switch = gtk::Switch::builder()
        .active(true)
        .valign(gtk::Align::Center)
        .build();
    fit_switch.update_property(&[gtk::accessible::Property::Label("Fit Preview Width")]);
    let source_content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    source_content.set_margin_top(14);
    source_content.set_margin_bottom(14);
    source_content.set_margin_start(14);
    source_content.set_margin_end(14);
    source_content.append(&appearance_source);
    source_content.append(&appearance_details);
    source_content.append(
        &gtk::Button::builder()
            .label("Reload Terminal Appearance")
            .action_name("win.reload-terminal")
            .css_classes(["prompt-menu-item"])
            .build(),
    );
    source_content.append(&layout_row("Fit Preview Width", &fit_switch));
    source_content.append(&context_source);
    for (label, action) in [
        ("Choose Preview Folder…", "win.preview-folder"),
        ("Refresh Folder", "win.refresh-preview-folder"),
        ("Reset Preview Session", "win.reset-preview"),
    ] {
        source_content.append(
            &gtk::Button::builder()
                .label(label)
                .action_name(action)
                .css_classes(["prompt-menu-item"])
                .build(),
        );
    }
    let source_popover = gtk::Popover::builder().child(&source_content).build();
    let source_button = gtk::MenuButton::builder()
        .icon_name("preferences-system-symbolic")
        .popover(&source_popover)
        .tooltip_text("Preview source and terminal appearance")
        .css_classes(["preview-source-button"])
        .build();
    source_button.update_property(&[
        gtk::accessible::Property::Label("Preview Source"),
        gtk::accessible::Property::Description(
            "Inspect the terminal profile, reload its appearance, or choose the preview folder",
        ),
    ]);
    variant_switch.set_valign(gtk::Align::Center);
    variant_switch.set_halign(gtk::Align::End);
    variant_switch.set_hexpand(false);
    let preview_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    preview_header.set_hexpand(true);
    preview_header.set_halign(gtk::Align::Fill);
    preview_header.set_valign(gtk::Align::Center);
    preview_header.add_css_class("preview-title-row");
    preview_header.append(&heading);
    let scene_selector = gtk::DropDown::from_strings(&["Full", "Terminal", "Prompt", "Greeting"]);
    // Keep the existing fresh/reset Terminal default. Open chooses Greeting or
    // Prompt from the document's enabled content; scene itself is never saved.
    scene_selector.set_selected(1);
    scene_selector.add_css_class("preview-scenario");
    scene_selector.set_tooltip_text(Some(
        "Preview scene · independent of the editor tabs on the left",
    ));
    scene_selector.update_property(&[gtk::accessible::Property::Label("Preview Scene")]);
    preview_header.append(&scene_selector);
    let inspect_button = gtk::ToggleButton::with_label("Inspect");
    inspect_button.set_valign(gtk::Align::Center);
    inspect_button.add_css_class("preview-inspect-toggle");
    inspect_button.set_tooltip_text(Some(
        "Enable point-to-edit · hover to identify a detail · Esc to exit",
    ));
    inspect_button.update_property(&[
        gtk::accessible::Property::Label("Inspect Preview"),
        gtk::accessible::Property::Description(
            "Off by default. Enable hover feedback and click-to-edit without changing the preview",
        ),
    ]);
    preview_header.append(&inspect_button);
    preview_header.append(&source_button);
    preview_header.append(variant_switch);
    content.append(&preview_header);
    let full_session = full_session::FullSession::new();
    content.append(&full_session.toolbar);

    let terminal = gtk::Box::new(gtk::Orientation::Vertical, 8);
    terminal.set_widget_name("termimochi-terminal");
    terminal.add_css_class("terminal-shell");
    terminal.set_vexpand(true);
    terminal.set_margin_bottom(4);

    let terminal_header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    terminal_header.add_css_class("terminal-header");
    let terminal_title = gtk::Label::new(Some("bash"));
    terminal_title.set_xalign(0.0);
    terminal_title.add_css_class("terminal-chrome");
    let terminal_tab = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    terminal_tab.set_valign(gtk::Align::Center);
    terminal_tab.add_css_class("terminal-tab");
    terminal_tab.append(&terminal_title);
    let terminal_header_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    terminal_header_spacer.set_hexpand(true);
    let scenario_labels = PreviewScenario::ALL.map(PreviewScenario::label);
    let selector = gtk::DropDown::from_strings(&scenario_labels);
    selector.set_selected((PreviewScenario::ALL.len() - 1) as u32);
    selector.set_tooltip_text(Some(
        "Current Folder uses a local snapshot; other entries are test samples",
    ));
    selector.update_property(&[gtk::accessible::Property::Label("Preview Scenario")]);
    selector.add_css_class("preview-scenario");
    let prompt_labels = PromptPreviewScenario::ALL.map(PromptPreviewScenario::label);
    let prompt_selector = gtk::DropDown::from_strings(&prompt_labels);
    prompt_selector.set_visible(false);
    prompt_selector.add_css_class("preview-scenario");
    prompt_selector.update_property(&[gtk::accessible::Property::Label("Prompt Preview Scenario")]);
    prompt_selector.set_tooltip_text(Some(
        "Check the prompt in your current folder or a controlled sample",
    ));
    let compare_selector = gtk::DropDown::from_strings(&["Edited", "Original"]);
    compare_selector.set_visible(false);
    compare_selector.add_css_class("preview-scenario");
    compare_selector.update_property(&[gtk::accessible::Property::Label("Compare Prompt")]);
    compare_selector.set_tooltip_text(Some(
        "Compare edited and original prompts in the same appended sample. Commands, paths and versions stay fixed. Original uses the configuration loaded when editing began.",
    ));
    terminal_header.append(&terminal_tab);
    terminal_header.append(&terminal_header_spacer);
    terminal_header.append(&compare_selector);
    terminal_header.append(&selector);
    terminal_header.append(&prompt_selector);
    terminal.append(&terminal_header);

    let vte_terminal = vte::Terminal::builder()
        .audible_bell(false)
        .bold_is_bright(false)
        .cursor_blink_mode(vte::CursorBlinkMode::System)
        .input_enabled(true)
        .scroll_on_keystroke(true)
        .scroll_on_output(true)
        .scrollback_lines(256)
        .build();
    vte_terminal.set_hexpand(true);
    vte_terminal.set_vexpand(false);
    vte_terminal.set_size(
        PREVIEW_COLUMNS
            .try_into()
            .expect("preview column count fits c_long"),
        PREVIEW_ROWS
            .try_into()
            .expect("preview row count fits c_long"),
    );
    // VTE owns text input and selection. Its local scroll controller works
    // even when the profile hides the terminal's scrollbar.
    vte_terminal.set_can_target(true);
    vte_terminal.set_focusable(true);
    vte_terminal.set_font(Some(&initial_typography.font_description()));
    vte_terminal.set_margin_top(DEFAULT_CONTENT_PADDING);
    vte_terminal.set_margin_bottom(DEFAULT_CONTENT_PADDING);
    vte_terminal.set_margin_start(DEFAULT_CONTENT_PADDING);
    vte_terminal.set_margin_end(DEFAULT_CONTENT_PADDING);
    vte_terminal.add_css_class("vte-preview");
    vte_terminal.set_tooltip_text(Some(
        "Drag to select text · type to try input · enable Inspect to find a detail's settings",
    ));
    vte_terminal.update_property(&[
        gtk::accessible::Property::Label("Interactive Terminal Preview"),
        gtk::accessible::Property::Description(
            "Drag to select and copy. Type locally; commands are never executed. Enable Inspect for hover feedback and point-to-edit. Escape restores the sample when Inspect is off",
        ),
        gtk::accessible::Property::KeyShortcuts("Escape"),
    ]);
    // VTE implements GtkScrollable itself and otherwise follows the viewport
    // width by changing its column count. A plain wrapper gives the horizontal
    // scroller a fixed-size canvas, preserving the selected grid when the
    // window is narrow.
    let terminal_canvas = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    terminal_canvas.set_vexpand(false);
    terminal_canvas.set_valign(gtk::Align::Start);
    terminal_canvas.add_css_class("terminal-canvas");
    terminal_canvas.append(&vte_terminal);
    let terminal_scrollbar =
        gtk::Scrollbar::new(gtk::Orientation::Vertical, None::<&gtk::Adjustment>);
    terminal_scrollbar.set_vexpand(true);
    terminal_scrollbar.set_valign(gtk::Align::Fill);
    terminal_scrollbar.set_tooltip_text(Some("Scroll terminal content and history"));
    terminal_scrollbar.add_css_class("terminal-scrollbar");
    terminal_scrollbar.update_property(&[gtk::accessible::Property::Label("Terminal Scrollbar")]);
    let terminal_scrollbar_revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::None)
        .reveal_child(false)
        .child(&terminal_scrollbar)
        .build();
    terminal_scrollbar_revealer.set_vexpand(true);
    terminal_scrollbar_revealer.set_valign(gtk::Align::Fill);
    let terminal_viewport = gtk::ScrolledWindow::builder()
        // Keep the selected column grid intact at narrow window sizes. The
        // external policy keeps its rail invisible while touchpad and
        // Shift+wheel panning remain available.
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::External)
        .propagate_natural_width(false)
        .propagate_natural_height(false)
        .vexpand(true)
        .height_request(PREVIEW_MIN_HEIGHT)
        .css_classes(["terminal-viewport"])
        .child(&terminal_canvas)
        .build();
    terminal_viewport.set_hexpand(true);
    terminal_viewport.set_tooltip_text(Some(
        "Scroll terminal content independently · Shift+wheel or a sideways touchpad gesture pans wide grids",
    ));
    terminal_viewport.update_property(&[
        gtk::accessible::Property::Label("Terminal Preview"),
        gtk::accessible::Property::Description(
            "Scroll terminal content without moving the editor or diagnostics; pan wide grids horizontally with Shift+wheel",
        ),
    ]);
    let scroll = preview_scroll::PreviewScroll::new(&vte_terminal, &terminal_viewport);
    terminal_scrollbar.set_adjustment(Some(&scroll.adjustment));
    let terminal_stage = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    terminal_stage.set_hexpand(true);
    terminal_stage.set_vexpand(true);
    scroll.attach(&terminal_stage);
    terminal_stage.add_css_class("terminal-stage");
    terminal_stage.append(&terminal_viewport);
    terminal_stage.append(&terminal_scrollbar_revealer);
    terminal.append(&terminal_stage);

    let inspect_layer = gtk::Overlay::new();
    inspect_layer.set_vexpand(true);
    inspect_layer.set_child(Some(&terminal));
    let inspect_highlight = gtk::DrawingArea::new();
    inspect_highlight.set_can_target(false);
    inspect_highlight.set_focusable(false);
    inspect_layer.add_overlay(&inspect_highlight);
    inspect_layer.set_measure_overlay(&inspect_highlight, false);
    let inspect_label = gtk::Label::new(None);
    inspect_label.add_css_class("preview-inspect-hint");
    inspect_label.set_can_target(false);
    inspect_label.set_max_width_chars(36);
    inspect_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    inspect_label.set_single_line_mode(true);
    // Dock feedback instead of chasing the pointer. GTK handles natural text
    // sizing and edge constraints without a GtkFixed measurement/move cycle.
    inspect_label.set_halign(gtk::Align::Start);
    inspect_label.set_valign(gtk::Align::End);
    inspect_label.set_margin_start(10);
    inspect_label.set_margin_end(10);
    inspect_label.set_margin_bottom(10);
    inspect_label.set_visible(false);
    inspect_layer.add_overlay(&inspect_label);
    inspect_layer.set_measure_overlay(&inspect_label, false);
    inspect_layer.set_clip_overlay(&inspect_label, true);
    content.append(&inspect_layer);

    let quality = gtk::Box::new(gtk::Orientation::Vertical, 4);
    quality.add_css_class("quality-section");

    let metrics = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    metrics.add_css_class("quality-row");
    let quality_heading = gtk::Label::new(Some("Preview Checks"));
    quality_heading.set_xalign(0.0);
    quality_heading.set_hexpand(true);
    quality_heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
    quality_heading.add_css_class("quality-heading");
    let (body_metric, body_ratio) = inline_metric("Text");
    let (composer_metric, composer_ratio) = inline_metric("Codex input");
    metrics.append(&quality_heading);
    metrics.append(&body_metric);
    metrics.append(&composer_metric);
    quality.append(&metrics);

    let diagnostic_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    diagnostic_header.add_css_class("diagnostic-header");
    let summary_icon = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    summary_icon.set_size_request(7, 7);
    summary_icon.set_valign(gtk::Align::Center);
    summary_icon.add_css_class("status-dot");
    let summary_title = gtk::Label::new(Some("Checking…"));
    summary_title.set_xalign(0.0);
    summary_title.set_hexpand(true);
    summary_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    summary_title.add_css_class("diagnostic-title");
    diagnostic_header.append(&summary_icon);
    diagnostic_header.append(&summary_title);
    quality.append(&diagnostic_header);

    let diagnostics = gtk::ListBox::new();
    diagnostics.set_selection_mode(gtk::SelectionMode::None);
    diagnostics.add_css_class("diagnostic-list");
    let diagnostic_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        // A late diagnostic must not resize the independent terminal viewport.
        .min_content_height(96)
        .max_content_height(96)
        .propagate_natural_height(true)
        .css_classes(["diagnostic-scroll"])
        .child(&diagnostics)
        .build();
    diagnostic_scroll.update_property(&[gtk::accessible::Property::Label("Diagnostics")]);
    let diagnostic_surface = gtk::Box::new(gtk::Orientation::Vertical, 0);
    diagnostic_surface.add_css_class("diagnostic-log-surface");
    diagnostic_surface.set_overflow(gtk::Overflow::Hidden);
    diagnostic_surface.set_visible(false);
    diagnostic_surface.append(&diagnostic_scroll);
    quality.append(&diagnostic_surface);
    content.append(&quality);

    PreviewWidgets {
        root: preview_hint::attach(divider, &terminal_viewport, &content),
        content,
        terminal_title,
        terminal_shell: terminal,
        inspect_button,
        inspect_layer,
        inspect_label,
        inspect_highlight,
        terminal_tab,
        terminal: vte_terminal,
        terminal_canvas,
        terminal_viewport,
        terminal_scrollbar_revealer,
        scroll,
        selector,
        scene_selector,
        full_session,
        prompt_selector,
        compare_selector,
        appearance_source,
        appearance_details,
        context_source,
        fit_switch,
        body_ratio,
        composer_ratio,
        summary_icon,
        summary_title,
        diagnostic_header,
        diagnostic_surface,
        diagnostics,
    }
}

fn typography_field(title: &str, child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let field = gtk::Box::new(gtk::Orientation::Vertical, 4);
    field.set_hexpand(true);
    field.add_css_class("typography-field");
    child.set_hexpand(true);
    child.set_halign(gtk::Align::Fill);
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.add_css_class("typography-field-label");
    field.append(&label);
    field.append(child);
    field
}

#[allow(clippy::too_many_arguments)]
fn typography_spin_button(
    value: f64,
    minimum: f64,
    maximum: f64,
    step: f64,
    digits: u32,
    accessible_label: &str,
    description: &str,
) -> gtk::SpinButton {
    let adjustment = gtk::Adjustment::new(value, minimum, maximum, step, step * 4.0, 0.0);
    let input = gtk::SpinButton::builder()
        .adjustment(&adjustment)
        .climb_rate(step)
        .digits(digits)
        .hexpand(true)
        .numeric(true)
        .snap_to_ticks(true)
        .width_chars(5)
        .build();
    input.add_css_class("typography-control");
    input.set_tooltip_text(Some(description));
    input.update_property(&[
        gtk::accessible::Property::Label(accessible_label),
        gtk::accessible::Property::Description(description),
    ]);
    input
}

fn monospace_font_choices(
    terminal: &vte::Terminal,
    preferred: &TypographySettings,
) -> (Vec<String>, u32) {
    let context = terminal.pango_context();
    let mut names: Vec<_> = context
        .list_families()
        .into_iter()
        .filter(|family| is_usable_terminal_family(&context, family))
        .map(|family| family.name().to_string())
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    // Preserve a saved family even if it is no longer installed. Pango may
    // preview a fallback, but saving must not silently replace the request.
    if !names
        .iter()
        .any(|name| name.eq_ignore_ascii_case(&preferred.family))
    {
        names.push(preferred.family.clone());
    }

    let resolved_family = context
        .load_font(&preferred.font_description())
        .and_then(|font| font.face())
        .map(|face| face.family().name().to_string());
    let system_family = context
        .load_font(&TypographySettings::default().font_description())
        .and_then(|font| font.face())
        .map(|face| face.family().name().to_string());

    if names.is_empty() {
        return (vec![DEFAULT_FONT_FAMILY.to_owned()], 0);
    }
    let default_index = preferred_font_index(
        &names,
        &preferred.family,
        resolved_family.as_deref(),
        system_family.as_deref(),
    );
    (names, default_index)
}

fn font_family_factory(css_class: &'static str) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, object| {
        let Some(item) = object.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let label = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .single_line_mode(true)
            .max_width_chars(32)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes([css_class])
            .build();
        item.set_child(Some(&label));
    });
    factory.connect_bind(|_, object| {
        let Some(item) = object.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(label) = item.child().and_downcast::<gtk::Label>() else {
            return;
        };
        let Some(font) = item.item().and_downcast::<gtk::StringObject>() else {
            return;
        };
        let family = font.string();
        let mut description = gtk::pango::FontDescription::new();
        description.set_family(&family);
        let attributes = gtk::pango::AttrList::new();
        attributes.insert(gtk::pango::AttrFontDesc::new(&description));
        label.set_text(&family);
        label.set_attributes(Some(&attributes));
        label.set_tooltip_text(Some(&family));
    });
    factory.connect_unbind(|_, object| {
        let Some(label) = object
            .downcast_ref::<gtk::ListItem>()
            .and_then(gtk::ListItem::child)
            .and_downcast::<gtk::Label>()
        else {
            return;
        };
        label.set_text("");
        label.set_attributes(None);
        label.set_tooltip_text(None);
    });
    factory
}

fn preferred_font_index(
    names: &[String],
    requested: &str,
    resolved_requested: Option<&str>,
    resolved_system: Option<&str>,
) -> u32 {
    [Some(requested), resolved_requested, resolved_system]
        .into_iter()
        .flatten()
        .find_map(|candidate| {
            names
                .iter()
                .position(|name| name.eq_ignore_ascii_case(candidate))
        })
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

/// GTK adjustments can be relative to the retained ring while VTE text APIs
/// use absolute rows (notably after reset). Locate the visible range using
/// VTE's own extraction instead of assuming the two coordinate systems match.
fn preview_visible_origin(terminal: &vte::Terminal) -> Option<(i64, f64)> {
    let visible = terminal.text_format(vte::Format::Text)?;
    if visible.trim().is_empty() {
        return None;
    }
    let adjustment = terminal.vadjustment()?;
    let units = if terminal.is_scroll_unit_is_pixels() {
        terminal.char_height() as f64
    } else {
        1.0
    };
    let fraction = (adjustment.value() / units).fract();
    let rows = terminal.row_count() + i64::from(fraction > 0.0);
    let cursor_row = terminal.cursor_position().1;
    let first = (cursor_row - terminal.scrollback_lines() - rows).max(0);
    let mut matched = None;
    for row in first..=cursor_row {
        let text = terminal
            .text_range_format(vte::Format::Text, row, 0, row + rows, 0)
            .0;
        if text.as_ref() == Some(&visible) {
            // Identical repeated screens have no safe unique absolute anchor.
            if matched.is_some() {
                return None;
            }
            matched = Some((row, fraction));
        }
    }
    matched
}

fn fitted_preview_columns(
    configured: usize,
    viewport_width: i64,
    cell_width: i64,
    padding: i32,
    fit: bool,
) -> usize {
    if !fit || viewport_width <= 0 || cell_width <= 0 {
        return configured;
    }
    let available = viewport_width.saturating_sub(i64::from(padding).saturating_mul(2));
    usize::try_from((available / cell_width).max(12))
        .unwrap_or(configured)
        .min(configured)
}

fn terminal_grid_extent(
    cell_pixels: std::os::raw::c_long,
    cell_count: usize,
    padding: i64,
    minimum: i32,
) -> i32 {
    let cell_pixels = i128::from(cell_pixels.max(0));
    let cell_count = i128::try_from(cell_count).unwrap_or(i128::MAX);
    let extent = cell_pixels
        .saturating_mul(cell_count)
        .saturating_add(i128::from(padding.max(0)));
    extent.clamp(i128::from(minimum.max(1)), i128::from(i32::MAX)) as i32
}

fn add_widget_padding(extent: i32, padding_each_side: i32) -> i32 {
    let padding = i64::from(padding_each_side.max(0)).saturating_mul(2);
    i64::from(extent.max(1))
        .saturating_add(padding)
        .clamp(1, i64::from(i32::MAX)) as i32
}

fn inline_metric(title: &str) -> (gtk::Box, gtk::Label) {
    let metric = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    metric.add_css_class("inline-metric");
    let title = gtk::Label::new(Some(title));
    title.set_xalign(0.0);
    title.add_css_class("inline-metric-label");
    let ratio = gtk::Label::new(Some("—"));
    ratio.set_xalign(1.0);
    ratio.add_css_class("metric-value");
    metric.append(&title);
    metric.append(&ratio);
    (metric, ratio)
}

impl Workbench {
    fn window(&self) -> adw::ApplicationWindow {
        self.window
            .upgrade()
            .expect("the workbench only runs while its window exists")
    }

    fn editor_snapshot(&self) -> EditorSnapshot {
        let model = self.model.borrow();
        let mut palette = model.palette.clone();
        // The document path belongs to the current window, not to an edit
        // revision. In particular, Undo after Save As must not revive the old
        // path embedded in an earlier palette clone.
        palette.set_source(None);
        EditorSnapshot {
            palette,
            active_variant: model.active_variant,
            selected_color_key: self.selected_color_key.borrow().clone(),
        }
    }

    fn begin_history_edit(&self) {
        if self.updating.get() || self.history.borrow().is_editing() {
            return;
        }
        let snapshot = self.editor_snapshot();
        self.history.borrow_mut().begin(snapshot);
        self.refresh_history_actions();
    }

    fn end_history_edit(&self) {
        if self.updating.get() {
            return;
        }
        if !self.history.borrow().is_editing() {
            self.refresh_history_actions();
            return;
        }
        if self.history.borrow_mut().abandon_unchanged() {
            self.refresh_history_actions();
            return;
        }
        let snapshot = self.editor_snapshot();
        self.history.borrow_mut().commit(snapshot);
        self.model.borrow_mut().refresh_dirty();
        self.refresh_titles();
        self.refresh_history_actions();
    }

    fn finish_active_edit(&self) {
        self.color_picker.finish_edit();
        // This also closes a theme-name transaction. It is intentionally
        // idempotent because ColorPicker::finish_edit may already have called
        // the edit-end handler above.
        self.end_history_edit();
    }

    fn normalize_name_entry(&self) {
        if !self.name_valid.get() {
            return;
        }
        let name = self.model.borrow().palette.name().to_owned();
        if self.name_entry.text().as_str() == name {
            return;
        }
        self.updating.set(true);
        self.name_entry.set_text(&name);
        self.updating.set(false);
        set_name_entry_error(&self.name_entry, None);
        self.refresh_history_actions();
    }

    fn settle_active_edit(&self) {
        self.finish_active_edit();
        self.normalize_name_entry();
    }

    fn has_draft(&self) -> bool {
        let model = self.model.borrow();
        !self.name_valid.get()
            || self.name_entry.text().as_str() != model.palette.name()
            || self.color_picker.has_invalid_draft()
    }

    fn discard_drafts(&self) {
        let name = self.model.borrow().palette.name().to_owned();
        self.updating.set(true);
        self.name_entry.set_text(&name);
        self.name_valid.set(true);
        set_name_entry_error(&self.name_entry, None);
        self.updating.set(false);
        self.color_picker.discard_draft();
        self.refresh_titles();
        self.refresh_deployment();
        self.refresh_history_actions();
    }

    fn refresh_history_actions(&self) {
        self.refresh_document_scope();
        if self.is_theme() && !self.updating.get() {
            self.record_theme_edit();
        }
        self.refresh_output_bar();
        self.refresh_workspace_title();
        self.greeting.refresh_status();
        if self.is_theme() {
            let history = self.typed.theme_history.borrow();
            self.undo_action
                .set_enabled(!history.undo.is_empty() || !self.document_inputs_valid());
            self.redo_action
                .set_enabled(!history.redo.is_empty() && self.document_inputs_valid());
            return;
        }
        if !self.greeting.invalid.get()
            && !self.greeting.presentation_pending()
            && self
                .workspace_baseline
                .borrow()
                .as_ref()
                .is_some_and(|saved| saved.greeting == self.greeting.settings())
        {
            self.greeting.status.set_text("Saved in workspace");
        }
        if self.greeting_module_button.is_active() {
            let history = self.greeting.history.borrow();
            self.undo_action.set_enabled(
                self.greeting.presentation_pending()
                    || self.greeting.invalid.get()
                    || history.can_undo(),
            );
            self.redo_action.set_enabled(
                !self.greeting.presentation_pending()
                    && !self.greeting.invalid.get()
                    && history.can_redo(),
            );
            return;
        }
        if self.layout_module_button.is_active() {
            let history = self.layout_history.borrow();
            self.undo_action.set_enabled(history.can_undo());
            self.redo_action.set_enabled(history.can_redo());
            return;
        }
        if self.typography_module_button.is_active() {
            let history = self.typography_history.borrow();
            self.undo_action.set_enabled(history.can_undo());
            self.redo_action.set_enabled(history.can_redo());
            return;
        }
        if self.prompt_module_button.is_active() {
            if self.prompt_source_selector.selected() == 0 {
                self.undo_action
                    .set_enabled(self.starship_editor.can_undo());
                self.redo_action
                    .set_enabled(self.starship_editor.can_redo());
                return;
            }
            let history = self.prompt_history.borrow();
            let designer = self.prompt_source_selector.selected() == 1;
            self.undo_action.set_enabled(designer && history.can_undo());
            self.redo_action.set_enabled(designer && history.can_redo());
            return;
        }
        if !self.palette_module_button.is_active() {
            // Avoid mutating a hidden module during activity transitions.
            self.undo_action.set_enabled(false);
            self.redo_action.set_enabled(false);
            return;
        }
        let has_draft = self.has_draft();
        let history = self.history.borrow();
        self.undo_action
            .set_enabled(has_draft || history.can_undo());
        self.redo_action
            .set_enabled(!has_draft && history.can_redo());
    }

    fn undo_edit(self: &Rc<Self>) {
        if self.is_theme() && self.document_inputs_valid() {
            self.theme_undo(false);
            return;
        }
        if self.greeting_module_button.is_active() {
            self.greeting.undo();
            self.refresh_history_actions();
            return;
        }
        if self.layout_module_button.is_active() {
            let target = self
                .layout_history
                .borrow_mut()
                .undo(self.layout_settings());
            if let Some(settings) = target {
                self.set_layout_settings(settings, false);
            }
            self.refresh_history_actions();
            return;
        }
        if self.typography_module_button.is_active() {
            let target = self
                .typography_history
                .borrow_mut()
                .undo(self.typography_settings());
            if let Some(settings) = target {
                self.set_typography_settings(&settings, false);
            }
            self.refresh_history_actions();
            return;
        }
        if self.prompt_module_button.is_active() {
            if self.prompt_source_selector.selected() == 0 {
                self.starship_editor.undo();
                return;
            }
            let current = self.prompt_settings.borrow().clone();
            let target = self.prompt_history.borrow_mut().undo(current);
            if let Some(target) = target {
                self.restore_prompt_settings(target);
            } else {
                self.refresh_history_actions();
            }
            return;
        }
        if self.has_draft() {
            let current = self.editor_snapshot();
            let target = self.history.borrow_mut().undo_pending(current);
            self.discard_drafts();
            if let Some(target) = target {
                self.restore_editor_snapshot(target);
            }
            return;
        }

        self.finish_active_edit();
        let current = self.editor_snapshot();
        let target = self.history.borrow_mut().undo(current);
        if let Some(target) = target {
            self.restore_editor_snapshot(target);
        } else {
            self.refresh_history_actions();
        }
    }

    fn redo_edit(self: &Rc<Self>) {
        if self.is_theme() {
            self.theme_undo(true);
            return;
        }
        if self.greeting_module_button.is_active() {
            self.greeting.redo();
            self.refresh_history_actions();
            return;
        }
        if self.layout_module_button.is_active() {
            let target = self
                .layout_history
                .borrow_mut()
                .redo(self.layout_settings());
            if let Some(settings) = target {
                self.set_layout_settings(settings, false);
            }
            self.refresh_history_actions();
            return;
        }
        if self.typography_module_button.is_active() {
            let target = self
                .typography_history
                .borrow_mut()
                .redo(self.typography_settings());
            if let Some(settings) = target {
                self.set_typography_settings(&settings, false);
            }
            self.refresh_history_actions();
            return;
        }
        if self.prompt_module_button.is_active() {
            if self.prompt_source_selector.selected() == 0 {
                self.starship_editor.redo();
                return;
            }
            let current = self.prompt_settings.borrow().clone();
            let target = self.prompt_history.borrow_mut().redo(current);
            if let Some(target) = target {
                self.restore_prompt_settings(target);
            } else {
                self.refresh_history_actions();
            }
            return;
        }
        self.finish_active_edit();
        if self.has_draft() {
            self.refresh_history_actions();
            return;
        }
        let current = self.editor_snapshot();
        let target = self.history.borrow_mut().redo(current);
        if let Some(target) = target {
            self.restore_editor_snapshot(target);
        } else {
            self.refresh_history_actions();
        }
    }

    fn restore_editor_snapshot(&self, snapshot: EditorSnapshot) {
        let EditorSnapshot {
            mut palette,
            active_variant,
            selected_color_key,
        } = snapshot;
        let current_path = self.model.borrow().current_path.clone();
        palette.set_source(current_path);
        {
            let mut model = self.model.borrow_mut();
            model.palette = palette;
            model.active_variant = active_variant;
            model.refresh_dirty();
        }
        let selected_color_key = if self.controls.contains_key(&selected_color_key) {
            selected_color_key
        } else {
            "Foreground".to_owned()
        };
        *self.selected_color_key.borrow_mut() = selected_color_key;
        self.refresh_all();
    }

    fn restore_prompt_settings(&self, settings: PromptSettings) {
        if self.observes_prompt() {
            self.activate_prompt_preview(1);
        }
        *self.prompt_settings.borrow_mut() = settings;
        self.refresh_prompt_controls();
        self.redraw_preview_contents();
        self.refresh_history_actions();
    }

    fn update_prompt_settings(&self, update: impl FnOnce(&mut PromptSettings)) {
        if self.updating_prompt.get() {
            return;
        }
        let before = self.prompt_settings.borrow().clone();
        let mut after = before.clone();
        update(&mut after);
        if before == after {
            self.refresh_prompt_controls();
            return;
        }

        let mut history = self.prompt_history.borrow_mut();
        history.begin(before);
        history.mark_changed();
        history.commit(after.clone());
        drop(history);
        if self.observes_prompt() {
            self.activate_prompt_preview(1);
        }
        *self.prompt_settings.borrow_mut() = after;
        self.refresh_prompt_controls();
        self.redraw_preview_contents();
        self.refresh_history_actions();
    }

    fn refresh_prompt_controls(&self) {
        let settings = self.prompt_settings.borrow().clone();
        let active_kinds = settings
            .enabled_segments()
            .map(|segment| segment.kind)
            .collect::<Vec<_>>();
        let selected = self
            .selected_prompt_kind
            .get()
            .filter(|kind| settings.is_enabled(*kind))
            .or_else(|| active_kinds.first().copied());
        self.selected_prompt_kind.set(selected);

        self.updating_prompt.set(true);
        self.prompt_preset_label.set_text(
            settings
                .source_preset()
                .map_or("Custom", PromptPreset::label),
        );
        self.prompt_module_count
            .set_text(&settings.enabled_count().to_string());
        self.prompt_module_count.set_tooltip_text(Some(&format!(
            "{} active modules",
            settings.enabled_count()
        )));
        self.prompt_empty_state
            .set_visible(settings.enabled_count() == 0);
        self.prompt_module_list
            .set_visible(settings.enabled_count() != 0);
        self.prompt_add_button
            .set_sensitive(settings.enabled_count() < PromptSegmentKind::ALL.len());
        if settings.enabled_count() == PromptSegmentKind::ALL.len() {
            self.prompt_add_button
                .set_tooltip_text(Some("All modules are already in the prompt"));
        } else {
            self.prompt_add_button
                .set_tooltip_text(Some("Add a prompt module"));
        }

        let mut previous: Option<&gtk::Box> = None;
        for segment in settings.segments().iter().filter(|segment| segment.enabled) {
            let index = usize::try_from(segment.kind.index()).unwrap_or_default();
            let row = &self.prompt_module_rows[index];
            self.prompt_module_list.reorder_child_after(row, previous);
            previous = Some(row);
        }

        for kind in PromptSegmentKind::ALL {
            let index = usize::try_from(kind.index()).unwrap_or_default();
            let enabled = settings.is_enabled(kind);
            let row = &self.prompt_module_rows[index];
            row.set_visible(enabled);
            if selected == Some(kind) {
                row.add_css_class("selected");
            } else {
                row.remove_css_class("selected");
            }
            self.prompt_module_select_buttons[index].set_active(selected == Some(kind));

            let sample = &self.prompt_module_samples[index];
            for tone in PreviewTone::ALL {
                let class = tone.css_class();
                sample.remove_css_class(class);
            }
            sample.set_text(kind.sample());
            sample.add_css_class(settings.tone(kind).css_class());

            if let Some((position, total)) = settings.active_position(kind) {
                self.prompt_module_move_up_buttons[index].set_sensitive(position > 0);
                self.prompt_module_move_down_buttons[index].set_sensitive(position + 1 < total);
                self.prompt_module_select_buttons[index].update_property(&[
                    gtk::accessible::Property::Label(kind.label()),
                    gtk::accessible::Property::Description(&format!(
                        "{}. Position {} of {}",
                        kind.description(),
                        position + 1,
                        total
                    )),
                ]);
            }
            self.prompt_add_module_buttons[index].set_sensitive(!enabled);
            self.prompt_add_module_buttons[index].set_visible(true);
        }

        self.prompt_inspector.set_visible(selected.is_some());
        if let Some(kind) = selected {
            self.prompt_inspector_title
                .set_text(&format!("{} Accent", kind.label()));
            self.prompt_tone_selector
                .set_selected(settings.tone(kind).index());
        }
        self.prompt_character_selector
            .set_selected(settings.character().index());
        self.prompt_layout_selector
            .set_selected(settings.layout().index());
        self.prompt_hostname_selector
            .set_selected(settings.hostname_mode().index());
        self.prompt_spacing_switch
            .set_active(settings.add_newline());
        self.updating_prompt.set(false);
    }

    fn install_actions(this: &Rc<Self>) {
        let window = this.window();

        for (name, review) in [("apply-scheme", true), ("last-scheme-application", false)] {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(this);
            action.connect_activate(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    if review {
                        this.request_scheme_apply();
                    } else {
                        this.show_last_scheme_application();
                    }
                }
            });
            window.add_action(&action);
        }

        let fonts = gio::SimpleAction::new("diagnostic-fonts", None);
        let weak = Rc::downgrade(this);
        fonts.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.inspect_preview_target(PreviewTarget::Typography);
            }
        });
        window.add_action(&fonts);
        let module = gio::SimpleAction::new("diagnostic-module", Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(this);
        module.connect_activate(move |_, parameter| {
            let Some(target) = parameter.and_then(|p| p.str()) else {
                return;
            };
            let (id, message) = target.split_once('\n').unwrap_or((target, ""));
            if crate::starship_modules::module_index(id).is_none() {
                return;
            }
            if let Some(this) = weak.upgrade() {
                if this.preview_uses_prompt.get() && this.preview_prompt_source.get() == 1 {
                    if id == "character" {
                        this.inspect_preview_target(PreviewTarget::PromptCharacter);
                    } else if let Some(kind) = PromptSegmentKind::ALL
                        .into_iter()
                        .find(|kind| kind.starship_module() == id)
                    {
                        this.inspect_preview_target(PreviewTarget::PromptSegment(kind));
                    }
                    return;
                }
                if !this.require_document_action("diagnostic-module") {
                    return;
                }
                this.prompt_module_button.set_active(true);
                this.prompt_source_selector.set_selected(0);
                this.starship_editor.open_finding(id, message);
            }
        });
        window.add_action(&module);

        let weak = Rc::downgrade(this);
        this.undo_action.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.undo_edit();
            }
        });
        window.add_action(&this.undo_action);

        let weak = Rc::downgrade(this);
        this.redo_action.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.redo_edit();
            }
        });
        window.add_action(&this.redo_action);

        let open = gio::SimpleAction::new("open", None);
        let weak = Rc::downgrade(this);
        open.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.choose_open();
            }
        });
        window.add_action(&open);

        let weak = Rc::downgrade(this);
        this.save_action.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.save();
            }
        });
        window.add_action(&this.save_action);

        let save_as = gio::SimpleAction::new("save-as", None);
        let weak = Rc::downgrade(this);
        save_as.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.choose_save_as();
            }
        });
        window.add_action(&save_as);

        for (name, operation) in [
            ("save-layout", 0),
            ("save-theme", 8),
            ("export-theme", 9),
            ("try-greeting", 10),
            ("save-font-preset", 11),
            ("export-layout", 1),
            ("reload-layout", 2),
            ("apply-layout", 3),
            ("restore-layout", 4),
            ("open-workspace", 5),
            ("save-workspace", 6),
            ("save-workspace-as", 7),
        ] {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(this);
            action.connect_activate(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    if !this.require_document_action(name) {
                        return;
                    }
                    match operation {
                        0 => this.save_layout_preset(),
                        1 => this.choose_layout_export(),
                        2 => this.reload_layout_preset(),
                        3 => this.request_layout_apply(),
                        4 => this.request_layout_restore(),
                        5 => this.choose_workspace_open(),
                        6 => this.save_workspace(),
                        8 => this.save_theme(),
                        9 => this.choose_theme_save_as(),
                        10 => this.show_greeting_trial(),
                        11 => this.save_typography_preset(),
                        _ => this.choose_workspace_save_as(),
                    }
                }
            });
            window.add_action(&action);
        }

        for (name, operation) in [
            ("save-greeting", 0),
            ("export-greeting", 1),
            ("reload-greeting", 2),
            ("export-fastfetch", 3),
        ] {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(this);
            action.connect_activate(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    if !this.require_document_action(name) {
                        return;
                    }
                    match operation {
                        0 => this.save_greeting_preset(),
                        1 => this.choose_greeting_export(false),
                        2 => this.reload_greeting_preset(),
                        _ => this.choose_greeting_export(true),
                    }
                }
            });
            window.add_action(&action);
        }

        for (name, operation) in [
            ("load-current-fastfetch", 0),
            ("import-fastfetch", 1),
            ("apply-fastfetch", 2),
            ("restore-fastfetch", 3),
            ("import-greeting-art", 4),
            ("export-greeting-txt", 5),
            ("export-greeting-ans", 6),
            ("edit-greeting-art-text", 7),
            ("edit-image-artwork", 8),
            ("greeting-startup", 9),
            ("export-pixel-greeting", 10),
        ] {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(this);
            action.connect_activate(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    if !this.require_document_action(name) {
                        return;
                    }
                    match operation {
                        0 => this.load_fastfetch_path(crate::fastfetch_apply::default_path()),
                        1 => this.choose_fastfetch_import(),
                        2 => this.request_fastfetch_apply(),
                        3 => this.request_fastfetch_restore(),
                        4 => this.choose_greeting_art_import(),
                        5 => this.choose_greeting_art_export(false),
                        6 => this.choose_greeting_art_export(true),
                        7 => this.edit_greeting_art_text(),
                        8 => this.edit_image_artwork(),
                        9 => this.show_greeting_startup(),
                        _ => this.show_pixel_export(),
                    }
                }
            });
            window.add_action(&action);
        }

        for (name, operation) in [
            ("save-typography", 0),
            ("export-typography", 1),
            ("apply-typography", 2),
            ("restore-typography", 3),
            ("reload-typography", 4),
        ] {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(this);
            action.connect_activate(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    if !this.require_document_action(name) {
                        return;
                    }
                    match operation {
                        0 => this.save_typography_preset(),
                        1 => this.choose_typography_export(),
                        2 => this.request_typography_apply(),
                        3 => this.request_typography_restore(),
                        _ => this.reload_typography_preset(),
                    }
                }
            });
            window.add_action(&action);
        }

        let export_starship = gio::SimpleAction::new("export-starship", None);
        let weak = Rc::downgrade(this);
        export_starship.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.choose_starship_export();
            }
        });
        window.add_action(&export_starship);

        for (name, operation) in [
            ("save-starship", 0),
            ("restore-starship", 1),
            ("reload-starship", 2),
        ] {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(this);
            action.connect_activate(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    if !this.require_document_action(name) {
                        return;
                    }
                    match operation {
                        0 => this.request_starship_save(),
                        1 => this.request_starship_restore(),
                        _ => this.request_starship_reload(),
                    }
                }
            });
            window.add_action(&action);
        }

        let reload_terminal = gio::SimpleAction::new("reload-terminal", None);
        let weak = Rc::downgrade(this);
        reload_terminal.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                if !this.require_document_action("reload-terminal") {
                    return;
                }
                this.confirm_typography_discard(|this| {
                    this.confirm_layout_discard(|this| {
                        this.confirm_discard(|this| this.reload_terminal_appearance())
                    })
                });
            }
        });
        window.add_action(&reload_terminal);
        let preview_folder = gio::SimpleAction::new("preview-folder", None);
        let weak = Rc::downgrade(this);
        preview_folder.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.choose_preview_folder();
            }
        });
        window.add_action(&preview_folder);
        let refresh_folder = gio::SimpleAction::new("refresh-preview-folder", None);
        let weak = Rc::downgrade(this);
        refresh_folder.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.reset_prompt_preview();
                this.refresh_current_context();
            }
        });
        window.add_action(&refresh_folder);
        let reset_preview = gio::SimpleAction::new("reset-preview", None);
        let weak = Rc::downgrade(this);
        reset_preview.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.reset_prompt_preview();
                this.redraw_preview_contents();
            }
        });
        window.add_action(&reset_preview);

        let weak = Rc::downgrade(this);
        this.install_action.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.request_install();
            }
        });
        window.add_action(&this.install_action);

        let weak = Rc::downgrade(this);
        this.rollback_action.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.request_rollback();
            }
        });
        window.add_action(&this.rollback_action);

        for format in ExportFormat::ALL {
            let action = gio::SimpleAction::new(&format!("export-{}", format.as_str()), None);
            let weak = Rc::downgrade(this);
            action.connect_activate(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    this.choose_export(format);
                }
            });
            window.add_action(&action);
        }
    }

    fn connect_signals(this: &Rc<Self>) {
        Self::connect_color_targets(this);
        Self::connect_preview_scene(this);
        Self::connect_full_session(this);
        let weak = Rc::downgrade(this);
        this.window().connect_close_request(move |_| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            this.settle_active_edit();
            if !this.has_unsaved_setup() {
                return glib::Propagation::Proceed;
            }
            this.confirm_close_discard();
            glib::Propagation::Stop
        });

        let weak = Rc::downgrade(this);
        this.palette_module_button.connect_toggled(move |button| {
            if let Some(this) = weak.upgrade() {
                if !button.is_active() {
                    this.settle_active_edit();
                } else {
                    this.refresh_color_targets(false);
                }
                this.refresh_history_actions();
            }
        });

        for button in [
            &this.palette_module_button,
            &this.typography_module_button,
            &this.layout_module_button,
            &this.prompt_module_button,
            &this.greeting_module_button,
        ] {
            let weak = Rc::downgrade(this);
            button.connect_toggled(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.refresh_editor_menus();
                }
            });
        }

        let weak = Rc::downgrade(this);
        this.greeting_module_button.connect_toggled(move |_| {
            if let Some(this) = weak.upgrade() {
                this.greeting.finish();
            }
        });
        let weak = Rc::downgrade(this);
        this.greeting.connect_output_checked(move || {
            if let Some(this) = weak.upgrade() {
                this.refresh_output_bar();
            }
        });
        let weak = Rc::downgrade(this);
        this.greeting.connect_changed(move || {
            let Some(this) = weak.upgrade() else {
                return;
            };
            if this.updating.get() {
                return;
            }
            this.refresh_history_actions();
            this.schedule_fastfetch_sync();
            this.ensure_official_greeting_preview();
            this.schedule_diagnostics();
            if !this.greeting_redraw_pending.replace(true) {
                let weak = Rc::downgrade(&this);
                glib::timeout_add_local_once(Duration::from_millis(80), move || {
                    if let Some(this) = weak.upgrade() {
                        this.greeting_redraw_pending.set(false);
                        if !this.greeting.invalid.get() && this.greeting_preview.get() {
                            this.show_greeting_preview();
                        }
                    }
                });
            }
        });

        let weak = Rc::downgrade(this);
        this.window().connect_is_active_notify(move |window| {
            if window.is_active()
                && let Some(this) = weak.upgrade()
            {
                this.greeting.invalidate_output_checks();
                this.refresh_output_bar();
                this.schedule_fastfetch_sync();
            }
        });
        Workbench::connect_fastfetch_sync(this);

        let weak = Rc::downgrade(this);
        this.prompt_module_button.connect_toggled(move |button| {
            if let Some(this) = weak.upgrade() {
                if button.is_active() {
                    let navigating = this.navigating_preview.replace(true);
                    this.sync_prompt_page();
                    this.navigating_preview.set(navigating);
                }
                this.refresh_history_actions();
            }
        });

        let weak = Rc::downgrade(this);
        this.prompt_source_selector
            .connect_selected_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    if this.updating.get() {
                        return;
                    }
                    this.sync_prompt_page();
                    if !this.navigating_preview.get() {
                        this.preview_prompt_source
                            .set(this.prompt_source_selector.selected());
                    }
                    if !this.navigating_preview.get() && this.observes_prompt() {
                        if this.prompt_source_selector.selected() == 0 {
                            this.activate_prompt_preview(0);
                            this.schedule_copy_preview();
                        } else {
                            this.activate_prompt_preview(1);
                        }
                        this.redraw_preview_contents();
                    }
                    this.refresh_history_actions();
                }
            });

        let weak = Rc::downgrade(this);
        this.starship_editor.connect_changed(move || {
            if let Some(this) = weak.upgrade() {
                if this.updating.get() {
                    return;
                }
                this.refresh_history_actions();
                if !this.navigating_preview.get() && this.observes_prompt() {
                    this.activate_prompt_preview(0);
                    this.schedule_copy_preview();
                }
            }
        });

        for (index, button) in this.prompt_preset_buttons.iter().enumerate() {
            let weak = Rc::downgrade(this);
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    let preset = PromptPreset::from_index(u32::try_from(index).unwrap_or(1));
                    this.selected_prompt_kind.set(None);
                    this.update_prompt_settings(|settings| settings.apply_preset(preset));
                    this.prompt_preset_popover.popdown();
                    if let Some(kind) = this.selected_prompt_kind.get() {
                        let index = usize::try_from(kind.index()).unwrap_or_default();
                        this.prompt_module_select_buttons[index].grab_focus();
                    } else {
                        this.prompt_add_button.grab_focus();
                    }
                }
            });
        }

        for kind in PromptSegmentKind::ALL {
            let index = usize::try_from(kind.index()).unwrap_or_default();

            let weak = Rc::downgrade(this);
            this.prompt_module_select_buttons[index].connect_toggled(move |button| {
                if !button.is_active() {
                    return;
                }
                if let Some(this) = weak.upgrade() {
                    if this.updating_prompt.get() {
                        return;
                    }
                    this.selected_prompt_kind.set(Some(kind));
                    this.refresh_prompt_controls();
                }
            });

            let weak = Rc::downgrade(this);
            this.prompt_add_module_buttons[index].connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.selected_prompt_kind.set(Some(kind));
                    this.update_prompt_settings(|settings| {
                        settings.add_module(kind);
                    });
                    this.prompt_add_popover.popdown();
                    this.prompt_module_select_buttons[index].grab_focus();
                }
            });

            let weak = Rc::downgrade(this);
            this.prompt_module_move_up_buttons[index].connect_clicked(move |button| {
                if let Some(this) = weak.upgrade() {
                    this.selected_prompt_kind.set(Some(kind));
                    this.update_prompt_settings(|settings| {
                        settings.move_module_up(kind);
                    });
                    close_containing_popover(button);
                    this.prompt_module_select_buttons[index].grab_focus();
                }
            });

            let weak = Rc::downgrade(this);
            this.prompt_module_move_down_buttons[index].connect_clicked(move |button| {
                if let Some(this) = weak.upgrade() {
                    this.selected_prompt_kind.set(Some(kind));
                    this.update_prompt_settings(|settings| {
                        settings.move_module_down(kind);
                    });
                    close_containing_popover(button);
                    this.prompt_module_select_buttons[index].grab_focus();
                }
            });

            let weak = Rc::downgrade(this);
            this.prompt_module_remove_buttons[index].connect_clicked(move |button| {
                if let Some(this) = weak.upgrade() {
                    let replacement = if this.selected_prompt_kind.get() == Some(kind) {
                        let settings = this.prompt_settings.borrow();
                        let active = settings
                            .enabled_segments()
                            .map(|segment| segment.kind)
                            .collect::<Vec<_>>();
                        active
                            .iter()
                            .position(|active_kind| *active_kind == kind)
                            .and_then(|position| {
                                active
                                    .get(position + 1)
                                    .or_else(|| position.checked_sub(1).and_then(|p| active.get(p)))
                            })
                            .copied()
                    } else {
                        this.selected_prompt_kind.get()
                    };
                    this.selected_prompt_kind.set(replacement);
                    this.update_prompt_settings(|settings| {
                        settings.remove_module(kind);
                    });
                    close_containing_popover(button);
                    if let Some(replacement) = replacement {
                        let replacement_index =
                            usize::try_from(replacement.index()).unwrap_or_default();
                        this.prompt_module_select_buttons[replacement_index].grab_focus();
                    } else {
                        this.prompt_add_button.grab_focus();
                    }
                }
            });
        }

        let weak = Rc::downgrade(this);
        this.prompt_tone_selector
            .connect_selected_notify(move |selector| {
                if let Some(this) = weak.upgrade() {
                    let Some(kind) = this.selected_prompt_kind.get() else {
                        return;
                    };
                    let tone = PreviewTone::from_index(selector.selected());
                    this.update_prompt_settings(|settings| {
                        settings.set_tone(kind, tone);
                    });
                }
            });

        let weak = Rc::downgrade(this);
        this.prompt_character_selector
            .connect_selected_notify(move |selector| {
                if let Some(this) = weak.upgrade() {
                    let character = PromptCharacter::from_index(selector.selected());
                    this.update_prompt_settings(|settings| {
                        settings.set_character(character);
                    });
                }
            });

        let weak = Rc::downgrade(this);
        this.prompt_layout_selector
            .connect_selected_notify(move |selector| {
                if let Some(this) = weak.upgrade() {
                    this.update_prompt_settings(|settings| {
                        settings.set_layout(PromptLayout::from_index(selector.selected()));
                    });
                }
            });
        let weak = Rc::downgrade(this);
        this.prompt_hostname_selector
            .connect_selected_notify(move |selector| {
                if let Some(this) = weak.upgrade() {
                    this.update_prompt_settings(|settings| {
                        settings
                            .set_hostname_mode(PromptHostnameMode::from_index(selector.selected()));
                    });
                }
            });
        let weak = Rc::downgrade(this);
        this.prompt_spacing_switch
            .connect_active_notify(move |switch| {
                if let Some(this) = weak.upgrade() {
                    this.update_prompt_settings(|settings| {
                        settings.set_add_newline(switch.is_active());
                    });
                }
            });
        let weak = Rc::downgrade(this);
        this.prompt_preview_selector
            .connect_selected_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    if this.navigating_preview.get() {
                        return;
                    }
                    this.activate_prompt_preview(1);
                    this.preview_input.borrow_mut().reset();
                    this.redraw_preview_contents();
                }
            });
        let weak = Rc::downgrade(this);
        this.prompt_compare_selector
            .connect_selected_notify(move |_| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                if this.navigating_preview.get() || !this.preview_uses_prompt.get() {
                    return;
                }
                let position = this.preview_scroll.adjustment.value();
                this.redraw_preview_contents();
                this.preview_scroll.restore_after_redraw(position);
                this.schedule_diagnostics();
            });
        let weak = Rc::downgrade(this);
        this.fit_preview_switch.connect_active_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                this.refresh_terminal_geometry(&this.layout_settings());
                this.redraw_preview_contents();
            }
        });
        let weak = Rc::downgrade(this);
        this.preview_terminal_viewport
            .hadjustment()
            .connect_page_size_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.schedule_preview_reflow();
                }
            });

        let weak = Rc::downgrade(this);
        this.light_button.connect_toggled(move |button| {
            if button.is_active()
                && let Some(this) = weak.upgrade()
            {
                this.change_variant(Variant::Light);
            }
        });

        let weak = Rc::downgrade(this);
        this.dark_button.connect_toggled(move |button| {
            if button.is_active()
                && let Some(this) = weak.upgrade()
            {
                this.change_variant(Variant::Dark);
            }
        });

        let weak = Rc::downgrade(this);
        this.preview_selector.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                if this.navigating_preview.get() {
                    return;
                }
                if !this.greeting_preview.get() {
                    let prompt = this.preview_uses_prompt.get();
                    this.reset_prompt_preview();
                    if prompt {
                        this.activate_prompt_preview(0);
                        this.schedule_copy_preview();
                    }
                }
                this.refresh_preview();
            }
        });

        let terminal_shell = this.preview_terminal_shell.clone();
        this.preview_terminal
            .connect_has_focus_notify(move |terminal| {
                if terminal.has_focus() {
                    terminal_shell.add_css_class("preview-active");
                } else {
                    terminal_shell.remove_css_class("preview-active");
                }
            });

        let weak = Rc::downgrade(this);
        this.preview_terminal.connect_commit(move |_, text, _| {
            if let Some(this) = weak.upgrade() {
                this.commit_preview_input(text);
            }
        });

        Self::connect_preview_inspection(this);

        // Character metrics are authoritative only after VTE is realized.
        // Reapply once here so its fixed preview grid becomes the scrollable
        // child's minimum size instead of being recomputed from the viewport.
        let weak = Rc::downgrade(this);
        this.preview_terminal.connect_realize(move |_| {
            if let Some(this) = weak.upgrade() {
                this.refresh_typography();
            }
        });

        let weak = Rc::downgrade(this);
        this.preview_terminal
            .connect_char_size_changed(move |_, _, _| {
                if let Some(this) = weak.upgrade() {
                    this.schedule_preview_reflow();
                }
            });

        let weak = Rc::downgrade(this);
        this.font_family_selector.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                this.refresh_typography();
            }
        });

        let weak = Rc::downgrade(this);
        this.font_weight_selector.connect_selected_notify(move |_| {
            if let Some(this) = weak.upgrade() {
                this.refresh_typography();
            }
        });

        for input in [
            &this.font_size_input,
            &this.line_height_input,
            &this.cell_width_input,
        ] {
            let weak = Rc::downgrade(this);
            input.connect_value_changed(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.refresh_typography();
                }
            });
        }

        for input in [&this.content_padding_input, &this.window_spacing_input] {
            let weak = Rc::downgrade(this);
            input.connect_value_changed(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.refresh_layout();
                }
            });
        }

        for input in [&this.column_count_input, &this.row_count_input] {
            let weak = Rc::downgrade(this);
            input.connect_value_changed(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.preview_input.borrow_mut().reset();
                    this.refresh_layout();
                    this.refresh_preview();
                }
            });
        }

        for selector in [&this.cursor_shape_selector, &this.cursor_blink_selector] {
            let weak = Rc::downgrade(this);
            selector.connect_selected_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.refresh_layout();
                }
            });
        }

        for switch in [&this.tab_bar_switch, &this.scrollbar_switch] {
            let weak = Rc::downgrade(this);
            switch.connect_active_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.refresh_layout();
                }
            });
        }

        let weak = Rc::downgrade(this);
        this.name_entry.connect_changed(move |entry| {
            let Some(this) = weak.upgrade() else {
                return;
            };
            if this.updating.get() {
                return;
            }
            let implicit_edit = !this.history.borrow().is_editing();
            if implicit_edit {
                this.begin_history_edit();
            }
            let name = entry.text();
            let (result, changed) = {
                let mut model = this.model.borrow_mut();
                let previous = model.palette.name().to_owned();
                let result = model.palette.set_name(name.as_str());
                let changed = result.is_ok() && model.palette.name() != previous;
                if changed {
                    // Exact savepoint comparison is deferred until the end of
                    // the focused edit, keeping continuous typing inexpensive.
                    model.dirty = true;
                }
                (result, changed)
            };
            match result {
                Ok(()) => {
                    this.name_valid.set(true);
                    set_name_entry_error(entry, None);
                }
                Err(error) => {
                    this.name_valid.set(false);
                    set_name_entry_error(entry, Some(&format!("Invalid theme name: {error}")));
                }
            }
            if changed {
                this.history.borrow_mut().mark_changed();
            }
            if implicit_edit && !entry.has_focus() {
                this.end_history_edit();
            }
            this.refresh_titles();
            this.refresh_deployment();
            this.refresh_history_actions();
        });

        let name_focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(this);
        name_focus.connect_enter(move |_| {
            if let Some(this) = weak.upgrade() {
                this.begin_history_edit();
            }
        });
        let weak = Rc::downgrade(this);
        name_focus.connect_leave(move |_| {
            if let Some(this) = weak.upgrade() {
                this.end_history_edit();
                this.normalize_name_entry();
            }
        });
        this.name_entry.add_controller(name_focus);

        for (key, control) in &this.controls {
            let key = key.clone();
            let weak = Rc::downgrade(this);
            control.swatch.button().connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.color_targets.reset_scope();
                    this.select_color(&key);
                }
            });
        }

        let weak = Rc::downgrade(this);
        this.color_picker.connect_changed(move |color| {
            if let Some(this) = weak.upgrade() {
                let key = this.selected_color_key.borrow().clone();
                this.apply_color(&key, color);
            }
        });

        let weak = Rc::downgrade(this);
        this.color_picker.connect_edit_begin(move || {
            if let Some(this) = weak.upgrade() {
                this.begin_history_edit();
            }
        });

        let weak = Rc::downgrade(this);
        this.color_picker.connect_edit_end(move || {
            if let Some(this) = weak.upgrade() {
                this.end_history_edit();
            }
        });

        let weak = Rc::downgrade(this);
        this.color_picker.connect_draft_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.refresh_titles();
                this.refresh_deployment();
                this.refresh_history_actions();
            }
        });
    }

    fn change_variant(&self, variant: Variant) {
        if self.updating.get() || self.model.borrow().active_variant == variant {
            return;
        }
        if self.model.borrow().palette.variant(variant).is_none() {
            return;
        }
        self.settle_active_edit();
        self.model.borrow_mut().active_variant = variant;
        self.refresh_all();
    }

    fn select_color(&self, key: &str) {
        let Some(selected) = self.controls.get(key) else {
            return;
        };
        if *self.selected_color_key.borrow() != key {
            self.settle_active_edit();
        }
        selected.swatch.set_selected(true);
        *self.selected_color_key.borrow_mut() = key.to_owned();

        let color = {
            let model = self.model.borrow();
            model
                .palette
                .variant(model.active_variant)
                .and_then(|variant| variant.get(key))
        };
        self.selected_color_title.set_text(&color_display_name(key));
        self.color_picker
            .set_color(color.unwrap_or_else(|| Rgb::new(0, 0, 0)));
    }

    fn apply_color(&self, key: &str, color: Rgb) {
        if self.updating.get() {
            return;
        }
        let implicit_edit = !self.history.borrow().is_editing();
        if implicit_edit {
            self.begin_history_edit();
        }
        let variant = self.model.borrow().active_variant;
        let changed = {
            let mut model = self.model.borrow_mut();
            let Some(theme) = model.palette.variant_mut(variant) else {
                return;
            };
            if theme.get(key) == Some(color) {
                false
            } else {
                let _ = theme.set(key, color);
                // Avoid serializing a potentially large palette for every
                // pixel of a drag. The exact saved-state comparison happens
                // once when the transaction ends.
                model.dirty = true;
                true
            }
        };

        if let Some(control) = self.controls.get(key) {
            control.swatch.set_color(color);
        }
        if changed {
            self.history.borrow_mut().mark_changed();
            self.refresh_preview();
            self.refresh_titles();
        }
        if implicit_edit {
            self.end_history_edit();
        } else {
            self.refresh_history_actions();
        }
    }

    fn refresh_all(&self) {
        let model = self.model.borrow();
        self.updating.set(true);
        self.name_entry.set_text(model.palette.name());
        self.name_valid.set(true);
        set_name_entry_error(&self.name_entry, None);
        self.light_button
            .set_sensitive(model.palette.variant(Variant::Light).is_some());
        self.dark_button
            .set_sensitive(model.palette.variant(Variant::Dark).is_some());
        match model.active_variant {
            Variant::Light => self.light_button.set_active(true),
            Variant::Dark => self.dark_button.set_active(true),
        }

        if let Some(variant) = model.palette.variant(model.active_variant) {
            for (key, control) in &self.controls {
                if let Some(color) = variant.get(key) {
                    control.swatch.set_color(color);
                } else {
                    control.swatch.set_color(Rgb::new(0, 0, 0));
                }
            }
        }
        self.updating.set(false);
        drop(model);
        let selected_key = self.selected_color_key.borrow().clone();
        self.select_color(&selected_key);
        self.refresh_titles();
        self.refresh_typography();
        self.refresh_layout();
        self.greeting.refresh_status();
        self.refresh_prompt_controls();
        self.refresh_preview();
        self.refresh_color_targets(false);
        self.refresh_deployment();
        self.refresh_history_actions();
    }

    fn refresh_titles(&self) {
        let model = self.model.borrow();
        let dirty = if model.dirty { " • Modified" } else { "" };
        let source = model
            .current_path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| model.source_label.clone());
        self.brand_title.set_tooltip_text(Some(&format!(
            "{source}{dirty} · {}",
            model.active_variant.section_name()
        )));
        self.window().set_title(Some(&format!(
            "{} — TermiMochi{dirty}",
            model.palette.name()
        )));
        let inputs_valid = self.name_valid.get() && !self.color_picker.has_invalid_draft();
        let (can_save, emphasize_save) =
            save_button_state(model.dirty, model.current_path.is_some(), inputs_valid);
        self.save_action.set_enabled(can_save);
        if emphasize_save {
            self.save_button.add_css_class("save-ready");
        } else {
            self.save_button.remove_css_class("save-ready");
        }
        self.refresh_prompt_save_actions();
    }

    // Drive controls from VTE's committed terminal stream rather than a
    // capture-phase key handler. This lets IME preedit and window shortcuts
    // resolve before the local scratch line interprets Enter or Backspace.
    fn commit_preview_input(&self, committed: &str) {
        match PreviewInputEvent::from_commit(committed) {
            PreviewInputEvent::Backspace => {
                if self.preview_input.borrow_mut().backspace() {
                    self.redraw_preview_contents();
                }
            }
            PreviewInputEvent::Submit => {
                self.preview_input.borrow_mut().submit();
                self.redraw_preview_contents();
            }
            PreviewInputEvent::Reset => {
                self.preview_input.borrow_mut().reset();
                self.refresh_preview();
                if self.prompt_module_button.is_active() {
                    self.prompt_module_button.grab_focus();
                } else {
                    self.preview_selector.grab_focus();
                }
            }
            PreviewInputEvent::FocusForward => {
                gtk::prelude::WidgetExt::child_focus(
                    &self.window(),
                    gtk::DirectionType::TabForward,
                );
            }
            PreviewInputEvent::FocusBackward => {
                gtk::prelude::WidgetExt::child_focus(
                    &self.window(),
                    gtk::DirectionType::TabBackward,
                );
            }
            PreviewInputEvent::Text(text) => {
                let changed = self.preview_input.borrow_mut().commit(text);
                if changed {
                    self.redraw_preview_contents();
                }
            }
            PreviewInputEvent::Ignore => {}
        }
    }

    fn activate_prompt_preview(&self, source: u32) {
        if self.prompt_preview_base.borrow().is_none() {
            *self.prompt_preview_base.borrow_mut() = Some(self.preview_feed.borrow().clone());
            *self.prompt_initial_ansi.borrow_mut() = self
                .current_preview_context
                .borrow()
                .as_ref()
                .and_then(|context| context.imported_prompt.ansi.clone())
                .unwrap_or_else(|| "$ ".into());
            // Existing scratch input remains visible in the retained transcript.
            self.preview_input.borrow_mut().reset();
        }
        if source == 1 && self.designer_original.borrow().is_none() {
            *self.designer_original.borrow_mut() = Some(self.prompt_settings.borrow().clone());
        }
        self.preview_prompt_source.set(source);
        self.preview_uses_prompt.set(true);
        let navigating = self.navigating_preview.replace(true);
        self.prompt_compare_selector.set_selected(0);
        self.navigating_preview.set(navigating);
    }

    fn reset_prompt_preview(&self) {
        self.full_session.active.set(false);
        self.greeting_preview.set(false);
        self.greeting_motion.stop();
        self.refresh_terminal_geometry(&self.layout_settings());
        self.preview_uses_prompt.set(false);
        self.copy_generation
            .set(self.copy_generation.get().wrapping_add(1));
        self.prompt_preview_base.borrow_mut().take();
        self.designer_original.borrow_mut().take();
        self.copy_scene_history.borrow_mut().clear();
        self.copy_scene.borrow_mut().take();
        self.copy_initial_sample.borrow_mut().take();
        self.preview_input.borrow_mut().reset();
    }

    fn preview_prompt_settings(&self) -> PromptSettings {
        if self.previewing_original_prompt()
            && let Some(original) = self.designer_original.borrow().as_ref()
        {
            return original.clone();
        }
        self.prompt_settings.borrow().clone()
    }

    fn previewing_original_prompt(&self) -> bool {
        !self.full_session.active.get() && self.prompt_compare_selector.selected() == 1
    }

    fn redraw_preview_contents(&self) {
        self.sync_preview_scene();
        self.sync_full_session();
        self.refresh_output_bar();
        if !self.greeting_preview.get()
            || !self.preview_input.borrow().text().is_empty()
            || !self.preview_input.borrow().submitted().is_empty()
        {
            self.preview_scroll.follow_input();
        }
        self.greeting_motion.stop();
        if self.greeting_preview.get() {
            self.refresh_terminal_geometry(&self.layout_settings());
        }
        self.preview_feed.borrow_mut().clear();
        self.used_prompt_characters.borrow_mut().clear();
        *self.preview_map.borrow_mut() = PreviewMap::default();
        self.invalidate_preview_inspection();
        self.full_session
            .collecting
            .set(self.full_session.active.get());
        if !self.full_session.collecting.get() {
            self.preview_terminal.reset(true, true);
        }
        // Queue a screen clear with the feed, so pending VTE input from the
        // loading state or a previous scene cannot survive an async redraw.
        self.feed_preview(PREVIEW_HOME_AND_CLEAR);
        if self.full_session.active.get() {
            self.redraw_full_session();
            return;
        }
        if self.greeting_preview.get() {
            self.redraw_greeting();
            return;
        }
        self.prompt_compare_selector
            .set_visible(self.preview_uses_prompt.get());
        if self.preview_uses_prompt.get() {
            self.preview_selector
                .set_visible(self.preview_prompt_source.get() == 0);
            self.prompt_preview_selector
                .set_visible(self.preview_prompt_source.get() == 1);
            if let Some(base) = self.prompt_preview_base.borrow().as_ref() {
                for chunk in base {
                    self.feed_historical_preview(&chunk.text, chunk.scope);
                }
                self.feed_preview(b"\x1b[0m\r\n\r\n");
            }
            self.terminal_title.set_text("starship · sample");
            self.redraw_prompt_preview();
            self.feed_preview(PREVIEW_SHOW_CURSOR);
            return;
        }

        self.preview_selector.set_visible(true);
        self.prompt_preview_selector.set_visible(false);
        let scenario = PreviewScenario::from_index(self.preview_selector.selected());
        if scenario == PreviewScenario::CurrentFolder {
            self.redraw_current_folder(false);
            self.feed_preview(PREVIEW_SHOW_CURSOR);
            return;
        }
        self.terminal_title.set_text(scenario.terminal_title());
        for chunk in scenario.refresh_chunks() {
            self.feed_preview(chunk);
        }

        let input = self.preview_input.borrow();
        if input.is_active() {
            for line in input.submitted() {
                self.feed_preview(b"\r\n");
                self.feed_preview(PREVIEW_INPUT_PROMPT.as_bytes());
                self.feed_preview(line.as_bytes());
            }
            self.feed_preview(b"\r\n");
            self.feed_preview(PREVIEW_INPUT_PROMPT.as_bytes());
            self.feed_preview(input.text().as_bytes());
        }
        self.feed_preview(PREVIEW_SHOW_CURSOR);
    }

    fn sync_prompt_page(&self) {
        let designer = self.prompt_source_selector.selected() == 1;
        let navigating = self.navigating_preview.replace(true);
        let loaded = !designer && self.ensure_starship_copy();
        self.navigating_preview.set(navigating);
        self.prompt_design_panel.set_visible(designer);
        self.prompt_import_panel.set_visible(!designer && !loaded);
        self.starship_editor.root.set_visible(loaded);
        self.refresh_prompt_save_actions();
    }

    fn refresh_prompt_save_actions(&self) {
        let designer = self.prompt_source_selector.selected() == 1;
        let loaded = self.starship_editor.draft.borrow().is_some();
        let valid = !self.starship_editor.invalid();
        if let Some(action) = self
            .window()
            .lookup_action("save-starship")
            .and_downcast::<gio::SimpleAction>()
        {
            action.set_enabled(
                self.allows_document_action("save-starship") && (designer || (loaded && valid)),
            );
        }
        if let Some(action) = self
            .window()
            .lookup_action("restore-starship")
            .and_downcast::<gio::SimpleAction>()
        {
            action.set_enabled(
                self.allows_document_action("restore-starship")
                    && !designer
                    && loaded
                    && !self.starship_editor.detached.get(),
            );
        }
        if let Some(action) = self
            .window()
            .lookup_action("reload-starship")
            .and_downcast::<gio::SimpleAction>()
        {
            action.set_enabled(
                self.allows_document_action("reload-starship")
                    && !designer
                    && !self.starship_editor.detached.get(),
            );
        }
        if let Some(action) = self
            .window()
            .lookup_action("export-starship")
            .and_downcast::<gio::SimpleAction>()
        {
            action.set_enabled(
                self.allows_document_action("export-starship") && (designer || (loaded && valid)),
            );
        }
        // Keep the menu available even for invalid fields: Reload and Restore
        // are recovery actions. Individual write actions stay disabled.
        self.save_action.set_enabled(self.document_inputs_valid());
        if self.has_unsaved_setup() {
            self.save_button.add_css_class("save-ready");
        } else {
            self.save_button.remove_css_class("save-ready");
        }
    }

    fn ensure_starship_copy(&self) -> bool {
        if self.starship_editor.draft.borrow().is_some() {
            return true;
        }
        if self.workspace_prompt_loaded.get() {
            return false;
        }
        let imported = self
            .current_preview_context
            .borrow()
            .as_ref()
            .and_then(|context| {
                context
                    .imported_prompt
                    .source
                    .clone()
                    .map(|source| (context.imported_prompt.path.clone(), source))
            });
        let Some((path, source)) = imported else {
            return false;
        };
        match self.starship_editor.begin(path, source) {
            Ok(()) => true,
            Err(error) => {
                self.toast(&error);
                false
            }
        }
    }

    fn schedule_copy_preview(self: &Rc<Self>) {
        if !self.preview_uses_prompt.get() || self.preview_prompt_source.get() != 0 {
            return;
        }
        let generation = self.copy_generation.get().wrapping_add(1);
        self.copy_generation.set(generation);
        if self.starship_editor.invalid() {
            return;
        }
        self.starship_editor.status.set_text("Updating preview…");
        let weak = Rc::downgrade(self);
        // Editing text should not start a process per keystroke. Keep at most
        // one bounded renderer alive and discard results for superseded edits.
        glib::timeout_add_local_once(Duration::from_millis(220), move || {
            if let Some(this) = weak.upgrade()
                && this.copy_generation.get() == generation
            {
                this.start_copy_preview();
            }
        });
    }

    fn start_copy_preview(self: &Rc<Self>) {
        if self.starship_editor.invalid()
            || self.copy_loading.get()
            || !self.preview_uses_prompt.get()
            || self.preview_prompt_source.get() != 0
        {
            return;
        }
        let Some(contents) = self
            .starship_editor
            .draft
            .borrow()
            .as_ref()
            .map(|d| d.contents().to_owned())
        else {
            return;
        };
        let generation = self.copy_generation.get();
        self.copy_loading.set(true);
        let directory = self.preview_directory.borrow().clone();
        let columns = self.preview_terminal.column_count().max(12) as usize;
        let sample = self.starship_editor.scenario.selected();
        let scene = self.starship_editor.scene();
        let original_source = self
            .starship_editor
            .draft
            .borrow()
            .as_ref()
            .map(|draft| draft.original_contents().to_owned())
            .unwrap_or_default();
        let initial_key = (
            self.starship_editor.document(),
            directory.clone(),
            columns,
            sample,
        );
        let cached_initial = self
            .copy_initial_sample
            .borrow()
            .as_ref()
            .filter(|(key, _)| *key == initial_key)
            .map(|(_, ansi)| ansi.clone());
        let key = (contents.clone(), directory.clone(), columns, sample);
        let cached_notices = self.copy_notices.borrow().clone();
        let cached = (self.copy_prompt_key.borrow().as_ref() == Some(&key))
            .then(|| self.copy_preview.borrow().clone())
            .flatten()
            .and_then(Result::ok);
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            // Explicit starting samples remain functional, but the initial
            // prompt is frozen from the original document for both A/B views.
            let initial = (sample != 0).then(|| {
                cached_initial.unwrap_or_else(|| {
                    crate::starship_import::render_copy(
                        &original_source,
                        &directory,
                        columns,
                        sample,
                    )
                    .map(|(ansi, _)| ansi)
                })
            });
            let result = if let Some(ansi) = cached {
                Ok((ansi, cached_notices))
            } else {
                crate::starship_import::render_copy(&contents, &directory, columns, sample)
            };
            let scene = scene.map(|scene| {
                let original = scene.original().build();
                let scene = scene.build();
                debug_assert_eq!(original.command, scene.command);
                debug_assert_eq!(original.output, scene.output);
                let original_ansi =
                    crate::starship_import::render_scene(&original, &directory, columns);
                let ansi = crate::starship_import::render_scene(&scene, &directory, columns);
                crate::starship_scene::RenderedScene {
                    scene,
                    ansi,
                    original_ansi,
                    original_source: original.diagnostic_source,
                }
            });
            let _ = sender.send((contents, result, scene, initial));
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(40), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let (rendered_source, result, scene, initial) = match receiver.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => (
                    String::new(),
                    Err("Prompt preview worker stopped. Edit a field to retry.".into()),
                    None,
                    None,
                ),
            };
            this.copy_loading.set(false);
            if generation != this.copy_generation.get() {
                this.start_copy_preview();
                return glib::ControlFlow::Break;
            }
            let result = result.map(|(ansi, notices)| {
                *this.copy_notices.borrow_mut() = notices.clone();
                let status = if this.starship_editor.detached.get() {
                    "Workspace prompt · Export to write a Starship file"
                } else {
                    "Simulated preview. Apply to review changes."
                };
                this.starship_editor.status.set_text(status);
                let mut tooltip = scene.as_ref().map_or_else(String::new, |frame| {
                    format!("Current transition: {}.", frame.scene.title)
                });
                if !notices.is_empty() {
                    tooltip.push_str(&format!(
                        "\nNot executed in preview: {}. These settings are preserved when saving.",
                        notices.join(", ")
                    ));
                }
                this.starship_editor.status.set_tooltip_text(Some(&tooltip));
                ansi
            });
            if let Err(error) = &result {
                this.starship_editor.status.set_text(error);
            }
            *this.copy_rendered_source.borrow_mut() = result.as_ref().ok().map(|_| rendered_source);
            *this.copy_prompt_key.borrow_mut() = Some(key.clone());
            *this.copy_initial_sample.borrow_mut() =
                initial.map(|ansi| (initial_key.clone(), ansi));
            if let Some(scene) = scene.as_ref() {
                crate::starship_scene::record(
                    &mut this.copy_scene_history.borrow_mut(),
                    scene.clone(),
                );
            }
            *this.copy_scene.borrow_mut() = scene;
            *this.copy_preview.borrow_mut() = Some(result);
            if this.preview_prompt_source.get() == 0 && this.preview_uses_prompt.get() {
                this.redraw_preview_contents();
            }
            glib::ControlFlow::Break
        });
    }

    fn redraw_copy_preview(&self) {
        self.terminal_title.set_text("starship · simulated session");
        let preview = self.copy_preview.borrow();
        if preview.is_none() {
            self.feed_preview(b"Rendering your Starship prompt...\r\n");
            return;
        }
        let initial = self.prompt_initial_ansi.borrow();
        let sample = self.copy_initial_sample.borrow();
        let initial = match sample.as_ref().map(|(_, ansi)| ansi) {
            Some(Ok(ansi)) => ansi.as_str(),
            Some(Err(error)) => {
                self.feed_preview(
                    format!(
                        "Starting sample unavailable: {}\r\n",
                        crate::preview_context::display_text(error, 180)
                    )
                    .as_bytes(),
                );
                initial.as_str()
            }
            None => initial.as_str(),
        };
        // Full observes the current design, not Prompt's frozen A/B starting
        // prompt. Use the same cache/staleness resolution as its greeting line.
        let full_initial = self
            .full_session
            .active
            .get()
            .then(|| self.greeting_prompt_ansi());
        let initial = full_initial.as_deref().unwrap_or(initial);
        self.feed_simulated_prompt(initial);
        let history = self.copy_scene_history.borrow();
        let mut prompt = initial;
        for frame in history.iter() {
            self.feed_preview(frame.scene.command.as_bytes());
            self.feed_preview(b"\r\n");
            if !frame.scene.output.is_empty() {
                self.feed_preview(frame.scene.output.as_bytes());
                self.feed_preview(b"\r\n");
            }
            let ansi = if self.previewing_original_prompt() {
                &frame.original_ansi
            } else {
                &frame.ansi
            };
            match ansi {
                Ok(ansi) => prompt = ansi,
                Err(error) => self.feed_preview(
                    format!(
                        "\x1b[2mPreview unavailable: {}\x1b[0m\r\n",
                        crate::preview_context::display_text(error, 180)
                    )
                    .as_bytes(),
                ),
            }
            self.feed_simulated_prompt(prompt);
        }
        let input = self.preview_input.borrow();
        for line in input.submitted() {
            self.feed_preview(line.as_bytes());
            self.feed_preview(b"\r\n");
            self.feed_simulated_prompt(prompt);
        }
        self.feed_preview(input.text().as_bytes());
    }

    fn feed_simulated_prompt(&self, ansi: &str) {
        self.feed_historical_preview(ansi, Some(PreviewTarget::PromptCopy));
        self.feed_preview(b"\x1b[0m");
    }

    fn feed_historical_preview(&self, text: &str, scope: Option<PreviewTarget>) {
        // History has separate provenance. Only the newest simulated state is
        // diagnosed; obsolete glyphs in earlier command lines are not warnings.
        self.preview_map.borrow_mut().record(
            text,
            scope,
            self.preview_terminal.is_bold_is_bright(),
        );
        self.preview_feed.borrow_mut().push(PreviewChunk {
            text: text.into(),
            scope,
        });
        if !self.full_session.collecting.get() {
            self.preview_terminal.feed(text.as_bytes());
        }
    }

    fn redraw_prompt_preview(&self) {
        if self.full_session.active.get() && self.theme_prompt_disabled() {
            self.feed_scoped_preview("$ ", Some(PreviewTarget::Prompt));
            self.feed_preview(self.preview_input.borrow().text().as_bytes());
            return;
        }
        if self.preview_prompt_source.get() == 0 && self.starship_editor.draft.borrow().is_some() {
            self.redraw_copy_preview();
            return;
        }
        if self.preview_prompt_source.get() == 0 {
            self.redraw_current_folder(false);
            return;
        }
        let scenario = PromptPreviewScenario::from_index(self.prompt_preview_selector.selected());
        if scenario == PromptPreviewScenario::CurrentFolder {
            self.redraw_current_folder(true);
            return;
        }
        let settings = self.preview_prompt_settings();
        let [rust_success, node_success, go_success, failed] = prompt_preview_contexts();

        if scenario != PromptPreviewScenario::Projects {
            let context = match scenario {
                PromptPreviewScenario::Failure => failed,
                PromptPreviewScenario::Ssh => PromptPreviewContext {
                    username: "deploy",
                    hostname: "staging.example.net",
                    path: "/srv/api",
                    rust_version: None,
                    is_ssh: true,
                    ..rust_success
                },
                PromptPreviewScenario::Root => PromptPreviewContext {
                    username: "root",
                    path: "/etc",
                    git_branch: None,
                    git_status: None,
                    rust_version: None,
                    is_root: true,
                    ..rust_success
                },
                PromptPreviewScenario::Alignment => PromptPreviewContext {
                    path: "~/Projects/终端/preview",
                    git_branch: Some("feature/宽度"),
                    rust_version: None,
                    ..rust_success
                },
                _ => rust_success,
            };
            if scenario == PromptPreviewScenario::Alignment {
                self.feed_preview(
                    "English  | 中文对齐 | 日本語  | 한국어\r\nEmoji    | 🌸 ✨ 🦀 | (≧◡≦) | A → B\r\n\r\n".as_bytes(),
                );
            }
            self.feed_designed_prompt(&settings, &context);
            let command = match scenario {
                PromptPreviewScenario::Failure => "false",
                PromptPreviewScenario::Ssh => "hostname",
                PromptPreviewScenario::Root => "whoami",
                _ => "printf 'hello 你好'",
            };
            self.feed_preview(command.as_bytes());
            self.feed_preview(b"\r\n");
            let output = match scenario {
                PromptPreviewScenario::Ssh => "staging.example.net\r\n",
                PromptPreviewScenario::Root => "root\r\n",
                PromptPreviewScenario::Alignment => "hello 你好\r\n",
                _ => "",
            };
            self.feed_preview(output.as_bytes());
            self.feed_prompt_input(&settings, &context);
            return;
        }

        self.feed_designed_prompt(&settings, &rust_success);
        self.feed_preview(b"cargo test\r\n");
        self.feed_preview(b"\x1b[2m94 passed\x1b[0m\r\n");
        self.feed_designed_prompt(&settings, &node_success);
        self.feed_preview(b"npm test\r\n");
        self.feed_preview(b"\x1b[2m18 passed\x1b[0m\r\n");
        self.feed_designed_prompt(&settings, &go_success);
        self.feed_preview(b"false\r\n");

        self.feed_prompt_input(&settings, &failed);
    }

    fn feed_prompt_input(&self, settings: &PromptSettings, context: &PromptPreviewContext<'_>) {
        let input = self.preview_input.borrow();
        for line in input.submitted() {
            self.feed_designed_prompt(settings, context);
            self.feed_preview(line.as_bytes());
            self.feed_preview(b"\r\n");
        }
        self.feed_designed_prompt(settings, context);
        self.feed_preview(input.text().as_bytes());
    }

    fn redraw_current_folder(&self, use_designed_prompt: bool) {
        let snapshot = self.current_preview_context.borrow();
        let Some(snapshot) = snapshot.as_ref() else {
            self.terminal_title.set_text("current folder");
            self.feed_preview(b"Reading current folder...\r\n");
            return;
        };
        self.terminal_title.set_text(&format!(
            "{} · context",
            if use_designed_prompt || snapshot.imported_prompt.ansi.is_some() {
                "starship"
            } else {
                &snapshot.shell
            },
        ));
        let context = snapshot.as_prompt_context();
        let settings = self.preview_prompt_settings();
        let prompt = if use_designed_prompt {
            settings.preview_ansi(&context)
        } else if let Some(ansi) = &snapshot.imported_prompt.ansi {
            ansi.clone()
        } else {
            format!(
                "\x1b[32m{}@{}\x1b[0m:\x1b[34m{}\x1b[0m{} ",
                context.username,
                context.hostname,
                context.path,
                if context.is_root { '#' } else { '$' },
            )
        };
        let feed_prompt = || {
            if use_designed_prompt {
                self.feed_designed_prompt(&settings, &context);
            } else {
                self.feed_scoped_preview(&prompt, Some(PreviewTarget::Prompt));
            }
        };
        feed_prompt();
        self.feed_preview(b"pwd\r\n");
        // Snapshot fields are sanitized at the read boundary before reaching VTE.
        self.feed_preview(snapshot.absolute_path.as_bytes());
        self.feed_preview(b"\r\n");
        feed_prompt();
        if snapshot.git_branch.is_some() {
            self.feed_preview(b"git status --short\r\n");
            for line in &snapshot.git_status_lines {
                self.feed_preview(b"\x1b[33m");
                self.feed_preview(line.as_bytes());
                self.feed_preview(b"\x1b[0m\r\n");
            }
        } else {
            self.feed_preview(b"ls\r\n");
            self.feed_preview(snapshot.entries.join("  ").as_bytes());
            self.feed_preview(b"\r\n");
        }
        let input = self.preview_input.borrow();
        for line in input.submitted() {
            feed_prompt();
            self.feed_preview(line.as_bytes());
            self.feed_preview(b"\r\n");
        }
        feed_prompt();
        self.feed_preview(input.text().as_bytes());
    }

    fn feed_preview(&self, bytes: &[u8]) {
        let text = std::str::from_utf8(bytes).expect("preview feeds are UTF-8");
        self.feed_scoped_preview(text, None);
    }

    fn feed_scoped_preview(&self, text: &str, scope: Option<PreviewTarget>) {
        self.preview_feed.borrow_mut().push(PreviewChunk {
            text: text.into(),
            scope,
        });
        if matches!(
            scope,
            Some(
                PreviewTarget::Prompt
                    | PreviewTarget::PromptCopy
                    | PreviewTarget::PromptSegment(_)
                    | PreviewTarget::PromptCharacter
            )
        ) {
            let mut characters = self.used_prompt_characters.borrow_mut();
            for ch in text.chars().filter(|ch| is_prompt_character(*ch)) {
                if characters.len() < 128 {
                    characters.insert(ch);
                }
            }
        }
        self.preview_map.borrow_mut().record(
            text,
            scope,
            self.preview_terminal.is_bold_is_bright(),
        );
        if !self.full_session.collecting.get() {
            self.preview_terminal.feed(text.as_bytes());
        }
    }

    fn feed_designed_prompt(&self, settings: &PromptSettings, context: &PromptPreviewContext<'_>) {
        for (kind, text) in settings.preview_ansi_runs(context) {
            self.feed_scoped_preview(
                &text,
                Some(kind.map_or(PreviewTarget::PromptCharacter, PreviewTarget::PromptSegment)),
            );
        }
    }

    fn connect_preview_inspection(this: &Rc<Self>) {
        let weak = Rc::downgrade(this);
        this.inspect_button.connect_toggled(move |button| {
            if let Some(this) = weak.upgrade() {
                this.invalidate_preview_inspection();
                this.preview_terminal_viewport.set_has_tooltip(!button.is_active());
                this.preview_terminal.set_tooltip_text(if button.is_active() { None } else {
                    Some("Drag to select text · type to try input · enable Inspect to find a detail's settings")
                });
            }
        });

        let weak = Rc::downgrade(this);
        this.inspect_highlight.set_draw_func(move |_, cr, _, _| {
            let Some(this) = weak.upgrade() else {
                return;
            };
            let hit = this.inspect_hit.borrow();
            let Some(hit) = hit.as_ref() else {
                return;
            };
            let r = &hit.bounds;
            cr.rectangle(
                f64::from(r.x()),
                f64::from(r.y()),
                f64::from(r.width()),
                f64::from(r.height()),
            );
            cr.set_source_rgba(0.25, 0.53, 0.65, 0.14);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.85);
            cr.set_line_width(3.0);
            let _ = cr.stroke_preserve();
            cr.set_source_rgba(0.20, 0.43, 0.54, 0.95);
            cr.set_line_width(1.0);
            let _ = cr.stroke();
        });

        let weak = Rc::downgrade(this);
        this.preview_terminal.connect_contents_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.invalidate_preview_inspection();
                this.schedule_diagnostics();
            }
        });
        for adjustment in [
            this.preview_terminal.vadjustment(),
            Some(this.preview_terminal_viewport.hadjustment()),
            Some(this.preview_terminal_viewport.vadjustment()),
        ]
        .into_iter()
        .flatten()
        {
            let weak = Rc::downgrade(this);
            adjustment.connect_value_changed(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.greeting_motion.stop();
                    this.invalidate_preview_inspection();
                }
            });
        }

        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(this);
        key.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(this) = weak.upgrade()
                && this.inspect_button.is_active()
                && this.preview_terminal.has_focus()
            {
                this.inspect_button.set_active(false);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        // Capture on the ancestor, before VTE's own key controller can turn
        // Escape into a scratch-input reset.
        this.preview_terminal_shell.add_controller(key);

        let controller = gtk::EventControllerLegacy::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let click = RefCell::new(PreviewClick::default());
        let pressed_target = Cell::new(None);
        let pressed_generation = Cell::new(0);
        let weak = Rc::downgrade(this);
        controller.connect_event(move |_, event| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !this.inspect_button.is_active() {
                click.borrow_mut().cancel();
                pressed_target.set(None);
                return glib::Propagation::Proceed;
            }
            let threshold = f64::from(
                this.preview_terminal
                    .settings()
                    .gtk_dnd_drag_threshold()
                    .max(1),
            );
            let Some((x, y)) = event.position() else {
                return glib::Propagation::Proceed;
            };
            let primary = event
                .downcast_ref::<gdk::ButtonEvent>()
                .is_some_and(|event| event.button() == 1);
            match event.event_type() {
                gdk::EventType::ButtonPress => {
                    this.inspect_pointer.set(None);
                    this.inspect_generation
                        .set(this.inspect_generation.get().wrapping_add(1));
                    let modified = event.modifier_state().intersects(
                        gdk::ModifierType::CONTROL_MASK
                            | gdk::ModifierType::ALT_MASK
                            | gdk::ModifierType::SHIFT_MASK
                            | gdk::ModifierType::SUPER_MASK,
                    );
                    if primary && !modified {
                        click.borrow_mut().press(x, y);
                        pressed_generation.set(this.inspect_generation.get());
                        let hit = this.preview_hit_at(x, y);
                        pressed_target.set(hit.as_ref().map(|hit| hit.target));
                        this.show_inspection_feedback(hit);
                    } else {
                        click.borrow_mut().cancel();
                        pressed_target.set(None);
                        this.clear_inspection_feedback();
                    }
                }
                gdk::EventType::EnterNotify | gdk::EventType::MotionNotify => {
                    click.borrow_mut().motion(x, y, threshold);
                    if event.modifier_state().intersects(
                        gdk::ModifierType::BUTTON1_MASK
                            | gdk::ModifierType::BUTTON2_MASK
                            | gdk::ModifierType::BUTTON3_MASK,
                    ) {
                        this.clear_inspection_feedback();
                    } else {
                        this.schedule_inspection_hover(x, y);
                    }
                }
                gdk::EventType::ButtonRelease if primary => {
                    this.inspect_pointer.set(None);
                    if click.borrow_mut().release(x, y, threshold)
                        && pressed_generation.get() == this.inspect_generation.get()
                    {
                        let hit = this.preview_hit_at(x, y);
                        let target = hit.as_ref().map(|hit| hit.target);
                        this.show_inspection_feedback(hit);
                        let pressed = pressed_target.take();
                        if target.is_none() || target != pressed {
                            return glib::Propagation::Proceed;
                        }
                        let generation = this.inspect_generation.get();
                        let delay = this
                            .preview_terminal
                            .settings()
                            .gtk_double_click_time()
                            .clamp(100, 1000) as u64;
                        let weak = Rc::downgrade(&this);
                        glib::timeout_add_local_once(Duration::from_millis(delay), move || {
                            if let Some(this) = weak.upgrade()
                                && generation == this.inspect_generation.get()
                                && this.inspect_button.is_active()
                                && !this.preview_terminal.has_selection()
                                && this.inspect_hit.borrow().as_ref().map(|hit| hit.target)
                                    == target
                                && let Some(target) = target
                            {
                                this.inspect_preview_target(target);
                            }
                        });
                    }
                }
                gdk::EventType::Scroll
                | gdk::EventType::TouchCancel
                | gdk::EventType::LeaveNotify => {
                    click.borrow_mut().cancel();
                    pressed_target.set(None);
                    this.invalidate_preview_inspection();
                }
                _ => {}
            }
            // Observing instead of claiming is essential: VTE still owns
            // selection, word selection, primary paste and its input method.
            glib::Propagation::Proceed
        });
        this.preview_terminal_shell.add_controller(controller);
    }

    fn clear_inspection_feedback(&self) {
        self.inspect_pointer.set(None);
        if self.inspect_hit.borrow_mut().take().is_some() {
            self.inspect_highlight.queue_draw();
            self.preview_terminal.set_cursor_from_name(None);
        }
        self.inspect_label.set_visible(false);
    }

    fn invalidate_preview_inspection(&self) {
        self.inspect_origin.set(None);
        self.inspect_generation
            .set(self.inspect_generation.get().wrapping_add(1));
        self.clear_inspection_feedback();
    }

    fn schedule_inspection_hover(self: &Rc<Self>, x: f64, y: f64) {
        self.inspect_pointer.set(Some((x, y)));
        if self.inspect_hover_pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        // Resolve the latest pointer once per display frame, not on a 35 ms
        // timer that visibly steps behind a smoothly moving mouse.
        self.inspect_layer.add_tick_callback(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.inspect_hover_pending.set(false);
                if this.inspect_button.is_active()
                    && let Some((x, y)) = this.inspect_pointer.take()
                {
                    let hit = this.preview_hit_at(x, y);
                    this.show_inspection_feedback(hit);
                }
            }
            glib::ControlFlow::Break
        });
    }

    fn show_inspection_feedback(&self, hit: Option<PreviewHit>) {
        let previous_target = self.inspect_hit.borrow().as_ref().map(|hit| hit.target);
        let target = hit.as_ref().map(|hit| hit.target);
        if previous_target != target || !self.inspect_label.is_visible() {
            let label = target.map_or_else(|| "No editable detail".into(), PreviewTarget::label);
            self.inspect_label.set_text(&label);
        }
        if previous_target.is_some() != target.is_some() {
            self.preview_terminal
                .set_cursor_from_name(target.map(|_| "crosshair"));
        }
        if *self.inspect_hit.borrow() != hit {
            *self.inspect_hit.borrow_mut() = hit;
            self.inspect_highlight.queue_draw();
        }
        self.inspect_label.set_visible(true);
    }

    #[cfg(test)]
    fn preview_target_at(&self, x: f64, y: f64) -> Option<PreviewTarget> {
        self.preview_hit_at(x, y).map(|hit| hit.target)
    }

    fn preview_hit_at(&self, x: f64, y: f64) -> Option<PreviewHit> {
        let native = self.preview_terminal.native()?;
        let (dx, dy) = native.surface_transform();
        let origin = native.dynamic_cast::<gtk::Widget>().ok()?;
        // Match GTK's event dispatch: remove the native surface translation
        // before converting into child-widget coordinates (CSD shadows count).
        let point = gtk::graphene::Point::new((x - dx) as f32, (y - dy) as f32);
        let within = |widget: &gtk::Widget| {
            let p = origin.compute_point(widget, &point)?;
            (widget.is_visible()
                && p.x() >= 0.0
                && p.y() >= 0.0
                && p.x() < widget.width() as f32
                && p.y() < widget.height() as f32)
                .then_some(p)
        };
        let hit = |target, widget: &gtk::Widget, rect: gtk::graphene::Rect| {
            let p = widget.compute_point(
                &self.inspect_layer,
                &gtk::graphene::Point::new(rect.x(), rect.y()),
            )?;
            let end = widget.compute_point(
                &self.inspect_layer,
                &gtk::graphene::Point::new(rect.x() + rect.width(), rect.y() + rect.height()),
            )?;
            let left = p.x().max(1.0);
            let top = p.y().max(1.0);
            let right = end.x().min(self.inspect_layer.width() as f32 - 1.0);
            let bottom = end.y().min(self.inspect_layer.height() as f32 - 1.0);
            (right > left && bottom > top).then_some(PreviewHit {
                target,
                bounds: gtk::graphene::Rect::new(left, top, right - left, bottom - top),
            })
        };
        if self.full_session.active.get()
            && self.full_session.picture.is_mapped()
            && within(self.preview_terminal_viewport.upcast_ref()).is_some()
            && within(self.full_session.picture.upcast_ref()).is_some()
        {
            let picture = &self.full_session.picture;
            return hit(
                PreviewTarget::GreetingArtwork,
                picture.upcast_ref(),
                gtk::graphene::Rect::new(0.0, 0.0, picture.width() as f32, picture.height() as f32),
            );
        }
        if within(self.preview_terminal_tab.upcast_ref()).is_some() {
            let tab = &self.preview_terminal_tab;
            return hit(
                PreviewTarget::TabBar,
                tab.upcast_ref(),
                gtk::graphene::Rect::new(0.0, 0.0, tab.width() as f32, tab.height() as f32),
            );
        }
        let terminal = &self.preview_terminal;
        within(self.preview_terminal_viewport.upcast_ref())?;
        let Some(p) = within(terminal.upcast_ref()) else {
            let canvas = &self.preview_terminal_canvas;
            let p = within(canvas.upcast_ref())?;
            let r = terminal.compute_bounds(canvas)?;
            let rect = if p.x() < r.x() {
                gtk::graphene::Rect::new(0.0, r.y(), r.x(), r.height())
            } else if p.x() >= r.x() + r.width() {
                gtk::graphene::Rect::new(
                    r.x() + r.width(),
                    r.y(),
                    canvas.width() as f32 - r.x() - r.width(),
                    r.height(),
                )
            } else if p.y() < r.y() {
                gtk::graphene::Rect::new(r.x(), 0.0, r.width(), r.y())
            } else {
                gtk::graphene::Rect::new(
                    r.x(),
                    r.y() + r.height(),
                    r.width(),
                    canvas.height() as f32 - r.y() - r.height(),
                )
            };
            return hit(PreviewTarget::Padding, canvas.upcast_ref(), rect);
        };
        // .vte-preview has zero CSS padding; Layout uses widget margins.
        let px = f64::from(p.x());
        let py = f64::from(p.y());
        let cw = terminal.char_width();
        let ch = terminal.char_height();
        if cw <= 0 || ch <= 0 {
            return None;
        }
        if px < 0.0 || py < 0.0 {
            return None;
        }
        let mut col = (px / cw as f64).floor() as i64;
        let (top, fraction) = self.inspect_origin.get().unwrap_or_else(|| {
            let origin = preview_visible_origin(terminal);
            self.inspect_origin.set(Some(origin));
            origin
        })?;
        let row = top + (py / ch as f64 + fraction).floor() as i64;
        if col >= terminal.column_count() {
            return None;
        }
        if terminal.cursor_position() == (col, row) {
            return hit(
                PreviewTarget::Cursor,
                terminal.upcast_ref(),
                gtk::graphene::Rect::new(
                    (col * cw) as f32,
                    ((row - top) as f64 - fraction) as f32 * ch as f32,
                    cw as f32,
                    ch as f32,
                ),
            );
        }
        let text = |sr, sc, er, ec| {
            terminal
                .text_range_format(vte::Format::Text, sr, sc, er, ec)
                .0
                .unwrap_or_default()
        };
        // VTE ranges are half-open, in terminal columns (not UTF-8 bytes).
        let mut cell = text(row, col, row, col + 1);
        if cell.is_empty() && col > 0 {
            let previous = text(row, col - 1, row, col);
            if previous.chars().next().is_some_and(|ch| {
                glib::Unichar::is_wide(ch)
                    || (terminal.cjk_ambiguous_width() == 2 && glib::Unichar::is_wide_cjk(ch))
            }) {
                col -= 1;
                cell = previous;
            }
        }
        if cell.trim().is_empty() {
            return None;
        }
        let first =
            (terminal.cursor_position().1 - terminal.scrollback_lines() - terminal.row_count())
                .max(0);
        let end = terminal.cursor_position().1.max(row);
        let buffer = text(first, 0, end, terminal.column_count());
        let prefix = text(first, 0, row, col);
        let target = self.preview_map.borrow().resolve(&buffer, &prefix, &cell)?;
        let wide = cell.chars().next().is_some_and(|ch| {
            glib::Unichar::is_wide(ch)
                || (terminal.cjk_ambiguous_width() == 2 && glib::Unichar::is_wide_cjk(ch))
        });
        hit(
            target,
            terminal.upcast_ref(),
            gtk::graphene::Rect::new(
                (col * cw) as f32,
                ((row - top) as f64 - fraction) as f32 * ch as f32,
                (cw * if wide { 2 } else { 1 }) as f32,
                ch as f32,
            ),
        )
    }

    fn inspect_preview_target(&self, target: PreviewTarget) {
        if matches!(
            target,
            PreviewTarget::GreetingMessage
                | PreviewTarget::GreetingFields
                | PreviewTarget::GreetingField(_)
        ) && !self.typed.scope.get().greeting
        {
            self.toast("Preview reference — system fields and welcome text are not owned by this artwork document.");
            return;
        }
        let module = match target {
            PreviewTarget::Ansi(_) => EditorModule::Palette,
            PreviewTarget::GreetingArtwork
            | PreviewTarget::GreetingMessage
            | PreviewTarget::GreetingFields
            | PreviewTarget::GreetingField(_) => EditorModule::Greeting,
            PreviewTarget::Typography => EditorModule::Typography,
            PreviewTarget::Cursor | PreviewTarget::Padding | PreviewTarget::TabBar => {
                EditorModule::Layout
            }
            PreviewTarget::Prompt
            | PreviewTarget::PromptCopy
            | PreviewTarget::PromptSegment(_)
            | PreviewTarget::PromptCharacter => EditorModule::Prompt,
        };
        if !self.owns_module(module) {
            self.toast("Preview reference — this content does not belong to this document.");
            return;
        }
        self.navigating_preview.set(true);
        match target {
            PreviewTarget::Prompt => self.prompt_source_selector.set_selected(0),
            PreviewTarget::PromptCopy => self.prompt_source_selector.set_selected(0),
            PreviewTarget::PromptSegment(_) | PreviewTarget::PromptCharacter => {
                self.prompt_source_selector.set_selected(1)
            }
            _ => {}
        }
        gio::prelude::ActionGroupExt::activate_action(&self.window(), module.action_name(), None);
        self.navigating_preview.set(false);
        let focus: Option<gtk::Widget> = match target {
            PreviewTarget::GreetingArtwork
            | PreviewTarget::GreetingMessage
            | PreviewTarget::GreetingFields
            | PreviewTarget::GreetingField(_) => self.greeting.inspection_control(target),
            PreviewTarget::Ansi(index) => {
                self.color_targets.reset_scope();
                self.select_color(&format!("Color{index}"));
                Some(self.color_picker.inspection_field().upcast())
            }
            PreviewTarget::Typography => Some(self.font_family_selector.clone().upcast()),
            PreviewTarget::Cursor => Some(self.cursor_shape_selector.clone().upcast()),
            PreviewTarget::Padding => Some(self.content_padding_input.clone().upcast()),
            PreviewTarget::TabBar => Some(self.tab_bar_switch.clone().upcast()),
            PreviewTarget::Prompt => Some(self.prompt_source_selector.clone().upcast()),
            PreviewTarget::PromptCopy => Some(self.starship_editor.symbol.clone().upcast()),
            PreviewTarget::PromptCharacter => Some(self.prompt_character_selector.clone().upcast()),
            PreviewTarget::PromptSegment(kind) => {
                self.selected_prompt_kind.set(Some(kind));
                self.refresh_prompt_controls();
                Some(self.prompt_tone_selector.clone().upcast())
            }
        };
        if let Some(widget) = focus {
            widget.add_css_class("preview-inspected");
            // Focus scrolls the editor to the selected field; restore focus to
            // VTE afterwards so point-to-edit doesn't interrupt scratch input.
            widget.grab_focus();
            // A newly switched Stack page is allocated on the next frame.
            // Explicitly reveal the target then: restoring VTE focus below can
            // otherwise cancel GTK's deferred focus-scroll request.
            widget.add_tick_callback(|widget, _| {
                if let Some(scroll) = widget
                    .ancestor(gtk::ScrolledWindow::static_type())
                    .and_then(|w| w.downcast::<gtk::ScrolledWindow>().ok())
                    && let Some(rect) = widget.compute_bounds(&scroll)
                {
                    let adjustment = scroll.vadjustment();
                    let value = adjustment.value();
                    let top = value + f64::from(rect.y()) - 12.0;
                    let bottom = value + f64::from(rect.y() + rect.height()) + 12.0;
                    if top < value {
                        adjustment.set_value(top.max(adjustment.lower()));
                    } else if bottom > value + adjustment.page_size() {
                        adjustment.set_value((bottom - adjustment.page_size()).min(
                            (adjustment.upper() - adjustment.page_size()).max(adjustment.lower()),
                        ));
                    }
                }
                glib::ControlFlow::Break
            });
            glib::timeout_add_local_once(Duration::from_millis(1100), move || {
                widget.remove_css_class("preview-inspected")
            });
        }
        self.preview_terminal.grab_focus();
    }

    fn refresh_preview(&self) {
        let model = self.model.borrow();
        let kind = model.active_variant;
        let Some(variant) = model.palette.variant(kind) else {
            return;
        };
        let foreground = variant.get("Foreground").unwrap_or(Rgb::new(255, 255, 255));
        let background = variant.get("Background").unwrap_or(Rgb::new(0, 0, 0));
        let color0 = variant.get("Color0").unwrap_or(background);

        let css = format!(
            "#termimochi-terminal.{} {{ background-color: {background}; color: {foreground}; }}",
            self.terminal_css_scope
        );
        let terminal_palette: Vec<_> = (0..16)
            .map(|index| rgba_from_rgb(variant.get(&format!("Color{index}")).unwrap_or(background)))
            .collect();
        self.terminal_css_provider.load_from_data(&css);

        let terminal_palette_refs: Vec<_> = terminal_palette.iter().collect();
        let foreground_rgba = rgba_from_rgb(foreground);
        let background_rgba = rgba_from_rgb(background);
        let cursor_rgba = rgba_from_rgb(variant.get("Cursor").unwrap_or(foreground));
        let cursor_foreground_rgba =
            rgba_from_rgb(variant.get("CursorForeground").unwrap_or(background));
        self.preview_terminal.set_colors(
            Some(&foreground_rgba),
            Some(&background_rgba),
            &terminal_palette_refs,
        );
        self.greeting.artwork_preview.set_theme(
            &self.typography_settings(),
            foreground_rgba,
            background_rgba,
            terminal_palette.clone(),
        );
        self.preview_terminal.set_color_cursor(Some(&cursor_rgba));
        self.preview_terminal
            .set_color_cursor_foreground(Some(&cursor_foreground_rgba));
        // Reconstructing the small deterministic transcript also guarantees
        // that deleting text which wrapped across rows leaves no stale cells.
        self.redraw_preview_contents();

        set_metric(
            &self.body_ratio,
            Some(contrast_ratio(foreground, background)),
            4.5,
            "Foreground / Background",
        );
        set_metric(
            &self.composer_ratio,
            Some(contrast_ratio(foreground, color0)),
            4.5,
            "Foreground / Color0",
        );

        let report = lint_palette(&model.palette, Target::Codex);
        let issues: Vec<_> = report.issues_for(kind).cloned().collect();
        drop(model);
        self.refresh_diagnostics(&issues);
    }

    fn typography_settings(&self) -> TypographySettings {
        let family = self
            .font_family_selector
            .selected_item()
            .and_then(|item| item.downcast::<gtk::StringObject>().ok())
            .map(|item| item.string().to_string())
            .unwrap_or_else(|| DEFAULT_FONT_FAMILY.to_owned());
        TypographySettings::new(
            &family,
            self.font_size_input.value(),
            PreviewFontWeight::from_index(self.font_weight_selector.selected()),
            self.line_height_input.value(),
            self.cell_width_input.value(),
        )
    }

    fn typography_dirty(&self) -> bool {
        self.typography_settings() != *self.typography_baseline.borrow()
    }

    fn set_typography_settings(&self, settings: &TypographySettings, record: bool) {
        let was_updating = self.updating.replace(true);
        let (names, index) = monospace_font_choices(&self.preview_terminal, settings);
        let names: Vec<_> = names.iter().map(String::as_str).collect();
        self.font_family_selector
            .set_model(Some(&gtk::StringList::new(&names)));
        self.font_family_selector.set_selected(index);
        self.font_size_input.set_value(settings.size);
        self.font_weight_selector
            .set_selected(settings.weight.index());
        self.line_height_input.set_value(settings.line_height);
        self.cell_width_input.set_value(settings.cell_width);
        if !record {
            *self.last_typography.borrow_mut() = settings.clone();
        }
        self.updating.set(was_updating);
        self.refresh_typography();
    }

    fn committed_typography(&self) -> Result<TypographySettings, String> {
        for input in [
            &self.font_size_input,
            &self.line_height_input,
            &self.cell_width_input,
        ] {
            input.update();
        }
        let settings = self.typography_settings();
        settings.validate()?;
        Ok(settings)
    }

    fn save_typography_preset(&self) {
        if !self.require_document_action("save-typography") {
            return;
        }
        let result = self.committed_typography().and_then(|settings| {
            let mut store = self.typography_store.borrow_mut();
            let store = store
                .as_mut()
                .ok_or("The preset location is unavailable. Export a preset instead.")?;
            store.save(&settings)?;
            *self.typography_baseline.borrow_mut() = settings;
            self.typography_has_saved.set(true);
            Ok(())
        });
        match result {
            Ok(()) => {
                self.refresh_typography();
                self.toast("Typography preset saved for the next launch. Ptyxis was not changed.");
            }
            Err(error) => self.toast(&format!("Could not save typography: {error}")),
        }
    }

    fn reload_typography_preset(self: &Rc<Self>) {
        let path = self
            .typography_store
            .borrow()
            .as_ref()
            .map(|store| store.path.clone())
            .unwrap_or_else(|| {
                typography_preset::state_directory().join(typography_preset::PRESET_NAME)
            });
        let loaded = PresetStore::open(path).and_then(|store| {
            let settings = store
                .settings()?
                .ok_or("No typography preset has been saved yet.")?;
            Ok((store, settings))
        });
        match loaded {
            Ok((store, settings)) => self.confirm_typography_discard(move |this| {
                *this.typography_store.borrow_mut() = Some(store);
                *this.typography_baseline.borrow_mut() = settings.clone();
                this.typography_has_saved.set(true);
                this.set_typography_settings(&settings, true);
                this.toast("Saved typography preset reloaded.");
            }),
            Err(error) => self.toast(&format!("Could not reload typography: {error}")),
        }
    }

    fn typography_file_dialog(title: &str) -> gtk::FileDialog {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("TermiMochi typography preset"));
        filter.add_pattern("*.termimochi-font.json");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        gtk::FileDialog::builder()
            .title(title)
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build()
    }

    fn choose_typography_export(self: &Rc<Self>) {
        if !self.require_document_action("export-typography") {
            return;
        }
        let settings = match self.committed_typography() {
            Ok(settings) => settings,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let dialog = Self::typography_file_dialog("Export Typography Preset");
        dialog.set_initial_name(Some(typography_preset::PRESET_NAME));
        let weak = Rc::downgrade(self);
        dialog.save(Some(&self.window()), gio::Cancellable::NONE, move |result| {
            let Some(this) = weak.upgrade() else { return; };
            match result {
                Ok(file) => {
                    let outcome = (|| {
                        let path = file.path().ok_or("Only local files can be saved.")?;
                        if !path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.ends_with(".termimochi-font.json")) {
                            return Err("Use a filename ending in .termimochi-font.json; other configurations are never overwritten.".into());
                        }
                        let mut store = PresetStore::open(path)?;
                        store.settings()?; // Refuse to replace a foreign or malformed document.
                        store.save(&settings)
                    })();
                    match outcome {
                        Ok(()) => this.toast("Preset exported. Save Preset also remembers it for the next launch."),
                        Err(error) => this.toast(&format!("Could not export preset: {error}")),
                    }
                }
                Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                Err(error) => this.toast(&format!("Could not export preset: {error}")),
            }
        });
    }

    fn confirm_typography_discard(self: &Rc<Self>, action: impl FnOnce(Rc<Self>) + 'static) {
        if !self.typography_dirty() || self.workspace_is_clean() {
            action(self.clone());
            return;
        }
        let dialog = gtk::AlertDialog::builder()
            .message("Replace unsaved typography changes?")
            .detail("Save Preset first if you want to keep these font settings.")
            .buttons(["Cancel", "Replace Changes"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if result == Ok(1)
                    && let Some(this) = weak.upgrade()
                {
                    action(this);
                }
            },
        );
    }

    fn request_typography_apply(self: &Rc<Self>) {
        if !self.require_document_action("apply-typography") {
            return;
        }
        let prepared = self.committed_typography().and_then(|settings| {
            let font = self.preview_terminal.pango_context().load_font(&settings.font_description())
                .ok_or("The selected font is unavailable. Install it or select an installed family.")?;
            if !settings.family.eq_ignore_ascii_case(DEFAULT_FONT_FAMILY)
                && font.face().is_none_or(|face| !face.family().name().eq_ignore_ascii_case(&settings.family)) {
                return Err("The selected font is missing; the preview is using a fallback. Install it before applying.".into());
            }
            TypographyTarget::discover()?.prepare(&settings)
        });
        let request = match prepared {
            Ok(request) => request,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let reviewed_identity = self.typed.identity.get();
        let reviewed_target = self.typed.target.get();
        let reviewed_scope = self.typed.scope.get();
        let reviewed_settings = self.typography_settings();
        let dialog = gtk::AlertDialog::builder()
            .message("Apply typography to Ptyxis?")
            .detail(request.detail())
            .buttons(["Cancel", "Back Up and Apply"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if result != Ok(1) {
                    return;
                }
                let Some(this) = weak.upgrade() else {
                    return;
                };
                if this.typed.identity.get() != reviewed_identity
                    || this.typed.target.get() != reviewed_target
                    || this.typed.scope.get() != reviewed_scope
                    || !this.allows_document_action("apply-typography")
                    || this.committed_typography().ok().as_ref() != Some(&reviewed_settings)
                {
                    this.toast("Document, target or typography changed during review. Review again; nothing was applied.");
                    return;
                }
                match request.apply(&typography_preset::state_directory()) {
                    Ok(Some(backup)) => this.toast(&format!(
                        "Typography applied to Ptyxis. Backup: {}",
                        backup.display()
                    )),
                    Ok(None) => this.toast(
                        "Ptyxis already uses these typography settings. Previous backup kept.",
                    ),
                    Err(error) => this.toast(&format!("Could not apply typography: {error}")),
                }
            },
        );
    }

    fn request_typography_restore(self: &Rc<Self>) {
        if !self.require_document_action("restore-typography") {
            return;
        }
        let request = match RestoreRequest::load(&typography_preset::state_directory()) {
            Ok(request) => request,
            Err(error) => {
                self.toast(&error);
                return;
            }
        };
        let dialog = gtk::AlertDialog::builder()
            .message("Restore previous typography?")
            .detail(request.detail())
            .buttons(["Cancel", "Restore Typography"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(Some(&self.window()), gio::Cancellable::NONE, move |result| {
            if result != Ok(1) { return; }
            let Some(this) = weak.upgrade() else { return; };
            match request.restore() {
                Ok(()) => this.toast("Previous Ptyxis typography restored. Your preview and saved preset are unchanged."),
                Err(error) => this.toast(&format!("Could not restore typography: {error}")),
            }
        });
    }

    fn layout_settings(&self) -> LayoutSettings {
        LayoutSettings::new(
            self.content_padding_input.value_as_int(),
            usize::try_from(self.column_count_input.value_as_int()).unwrap_or(MIN_COLUMNS),
            usize::try_from(self.row_count_input.value_as_int()).unwrap_or(MIN_ROWS),
            PreviewCursorShape::from_index(self.cursor_shape_selector.selected()),
            PreviewCursorBlink::from_index(self.cursor_blink_selector.selected()),
            self.tab_bar_switch.is_active(),
            self.scrollbar_switch.is_active(),
            self.window_spacing_input.value_as_int(),
        )
    }

    fn schedule_preview_reflow(self: &Rc<Self>) {
        if self.geometry_pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(this) = weak.upgrade() else {
                return;
            };
            this.geometry_pending.set(false);
            // Adjustment notifications occur during GTK allocation. Apply
            // the new grid afterwards and replay content at that width;
            // buffered VTE input can otherwise retain the initial wide grid.
            let old_columns = this.preview_terminal.column_count();
            this.refresh_terminal_geometry(&this.layout_settings());
            if this.preview_terminal.column_count() != old_columns
                && this.starship_editor.draft.borrow().is_some()
            {
                this.schedule_copy_preview();
            }
            this.redraw_preview_contents();
        });
    }

    fn refresh_terminal_geometry(&self, layout: &LayoutSettings) {
        if self.updating_geometry.replace(true) {
            return;
        }
        let viewport_width = self.preview_terminal_viewport.hadjustment().page_size() as i64;
        let cell_width = self.preview_terminal.char_width();
        let greeting_width = if self.greeting_preview.get() {
            self.greeting.settings().preview_columns
        } else {
            0
        };
        let columns = if self.full_session.active.get() {
            if greeting_width > 0 {
                usize::from(greeting_width)
            } else {
                layout.columns
            }
        } else if greeting_width > 0 {
            usize::from(greeting_width)
        } else {
            fitted_preview_columns(
                layout.columns,
                viewport_width,
                cell_width,
                layout.content_padding,
                self.fit_preview_switch.is_active(),
            )
        };
        self.preview_terminal_canvas.set_halign(
            if greeting_width > 0 || self.full_session.active.get() {
                gtk::Align::Start
            } else {
                gtk::Align::Fill
            },
        );
        self.preview_terminal
            .set_hexpand(greeting_width == 0 && !self.full_session.active.get());
        self.preview_terminal_viewport.set_hscrollbar_policy(
            if greeting_width > 0 || self.full_session.active.get() {
                gtk::PolicyType::Automatic
            } else {
                gtk::PolicyType::External
            },
        );
        // A complete logo can be much taller than the old six-line sketch.
        // Keep the full grid in the terminal's own viewport; the header/log
        // stay mounted. This never changes the saved Layout document.
        let rows = if self.full_session.active.get() {
            layout.rows.max(self.full_session.rows.get())
        } else if self.greeting_preview.get() {
            layout.rows.max(
                (self.greeting_text_for_width(columns).lines().count()
                    + 5
                    + self.preview_input.borrow().submitted().len() * 3)
                    .min(96),
            )
        } else {
            layout.rows
        };
        self.preview_terminal.set_size(
            columns
                .try_into()
                .expect("normalized terminal columns fit c_long"),
            rows.try_into()
                .expect("normalized terminal rows fit c_long"),
        );
        self.preview_terminal.set_margin_top(layout.content_padding);
        self.preview_terminal
            .set_margin_bottom(layout.content_padding);
        self.preview_terminal
            .set_margin_start(layout.content_padding);
        self.preview_terminal.set_margin_end(layout.content_padding);

        let terminal_width =
            terminal_grid_extent(self.preview_terminal.char_width(), columns, 0, 1);
        let terminal_height = terminal_grid_extent(
            self.preview_terminal.char_height(),
            rows,
            0,
            PREVIEW_MIN_HEIGHT,
        );
        let preview_width = add_widget_padding(terminal_width, layout.content_padding);
        let preview_height = add_widget_padding(terminal_height, layout.content_padding);
        self.preview_terminal.set_width_request(terminal_width);
        self.preview_terminal.set_height_request(terminal_height);
        self.preview_terminal_canvas
            .set_width_request(preview_width);
        self.preview_terminal_canvas
            .set_height_request(preview_height);
        self.updating_geometry.set(false);
        self.refresh_full_transform();
    }

    fn refresh_layout(&self) {
        if self.updating.get() {
            return;
        }
        let layout = self.layout_settings();
        if self.last_layout.get() != layout {
            let mut history = self.layout_history.borrow_mut();
            history.begin(self.last_layout.get());
            history.mark_changed();
            history.commit(layout);
            self.last_layout.set(layout);
        }
        self.preview_terminal
            .set_cursor_shape(layout.cursor_shape.vte_shape());
        self.preview_terminal
            .set_cursor_blink_mode(layout.cursor_blink.vte_mode());
        self.preview_terminal_tab.set_visible(layout.tab_bar);
        self.preview_terminal_scrollbar_revealer
            .set_reveal_child(layout.scrollbar);
        self.preview_content.set_margin_top(layout.window_spacing);
        self.preview_content
            .set_margin_bottom(layout.window_spacing);
        self.preview_content.set_margin_start(layout.window_spacing);
        self.preview_content.set_margin_end(layout.window_spacing);
        self.refresh_terminal_geometry(&layout);
        if self.full_session.active.get() {
            self.redraw_preview_contents();
        }
        self.layout_status.set_text(
            if self
                .workspace_baseline
                .borrow()
                .as_ref()
                .is_some_and(|saved| saved.layout == layout)
            {
                "Saved in workspace"
            } else if self.layout_dirty() {
                "Unsaved layout changes"
            } else if self.layout_has_saved.get() {
                "Preset saved · restored on next launch"
            } else {
                "Preview only · not saved"
            },
        );
        self.refresh_history_actions();
    }

    fn refresh_typography(&self) {
        if self.updating.get() {
            return;
        }
        let settings = self.typography_settings();
        if *self.last_typography.borrow() != settings {
            let before = self.last_typography.borrow().clone();
            let mut history = self.typography_history.borrow_mut();
            history.begin(before);
            history.mark_changed();
            history.commit(settings.clone());
            *self.last_typography.borrow_mut() = settings.clone();
        }
        let description = settings.font_description();
        self.preview_terminal.set_font(Some(&description));
        self.starship_editor.set_preview_font(&description);
        self.preview_terminal
            .set_cell_height_scale(settings.line_height);
        self.preview_terminal
            .set_cell_width_scale(settings.cell_width);
        self.refresh_terminal_geometry(&self.layout_settings());

        let support =
            detect_nerd_font_support(&self.preview_terminal.pango_context(), &description);
        let label = support.label();
        self.nerd_status_label.set_text(&label);
        let detail = format!(
            "{}. Coverage is checked on the selected face without font fallback.",
            support.detail()
        );
        self.nerd_status_label.set_tooltip_text(Some(&detail));
        self.nerd_status_label.update_property(&[
            gtk::accessible::Property::Label(&label),
            gtk::accessible::Property::Description(&detail),
        ]);
        self.refresh_all_diagnostics();
        self.typography_status.set_text(
            if self
                .workspace_baseline
                .borrow()
                .as_ref()
                .is_some_and(|saved| saved.typography == settings)
            {
                "Saved in workspace"
            } else if self.typography_dirty() {
                "Unsaved typography changes"
            } else if self.typography_has_saved.get() {
                "Preset saved · restored on next launch"
            } else {
                "Preview only · not saved"
            },
        );
        self.refresh_history_actions();
    }

    fn schedule_diagnostics(self: &Rc<Self>) {
        if self.diagnostics_pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(80), move || {
            if let Some(this) = weak.upgrade() {
                this.diagnostics_pending.set(false);
                this.refresh_all_diagnostics();
            }
        });
    }

    fn refresh_all_diagnostics(&self) {
        let model = self.model.borrow();
        let issues: Vec<_> = lint_palette(&model.palette, Target::Codex)
            .issues_for(model.active_variant)
            .cloned()
            .collect();
        drop(model);
        self.refresh_diagnostics(&issues);
    }

    fn refresh_diagnostics(&self, issues: &[Issue]) {
        let source = match (
            self.preview_uses_prompt.get(),
            self.preview_prompt_source.get(),
        ) {
            (true, 0) => self.copy_rendered_source.borrow().clone(),
            (true, 1) => Some(self.preview_prompt_settings().to_starship_toml()),
            _ => self
                .current_preview_context
                .borrow()
                .as_ref()
                .and_then(|context| context.imported_prompt.source.clone()),
        };
        let mut issues = issues.to_vec();
        issues.extend(self.greeting_compatibility_issues());
        issues.extend(self.prompt_diagnostics.borrow_mut().check(
            &self.preview_terminal.pango_context(),
            &self.typography_settings().font_description(),
            &self.used_prompt_characters.borrow(),
            source.as_deref(),
        ));
        if self.preview_uses_prompt.get()
            && self.preview_prompt_source.get() == 0
            && let Some(sample) = self.copy_scene.borrow().as_ref()
        {
            let (ansi, source) = if self.previewing_original_prompt() {
                (&sample.original_ansi, &sample.original_source)
            } else {
                (&sample.ansi, &sample.scene.diagnostic_source)
            };
            if let Ok(ansi) = ansi {
                let used = ansi.chars().filter(|ch| is_prompt_character(*ch)).collect();
                for mut issue in self.scene_diagnostics.borrow_mut().check(
                    &self.preview_terminal.pango_context(),
                    &self.typography_settings().font_description(),
                    &used,
                    Some(source),
                ) {
                    if issues.contains(&issue) {
                        continue;
                    }
                    if let Some((title, detail)) = issue.message.split_once('\n') {
                        issue.message = format!("{title} · simulated\n{detail}");
                    }
                    issues.push(issue);
                }
            }
        }
        // Cursor/selection changes can emit VTE contents-changed without any
        // new finding. Keep row widgets and their focus/scroll positions then.
        if self.reported_issues.borrow().as_ref() == Some(&issues) {
            return;
        }
        *self.reported_issues.borrow_mut() = Some(issues.clone());
        while let Some(child) = self.diagnostics.first_child() {
            self.diagnostics.remove(&child);
        }

        let visible_issues: Vec<_> = issues
            .iter()
            .filter(|issue| issue.code != "variant-passed")
            .collect();
        let issue_count = visible_issues.len();

        let errors = visible_issues
            .iter()
            .filter(|issue| issue.severity == Severity::Error)
            .count();
        let warnings = visible_issues
            .iter()
            .filter(|issue| issue.severity == Severity::Warning)
            .count();
        let notes = issue_count - errors - warnings;

        remove_status_classes(&self.summary_icon);
        // Keep the summary row allocated as async checks add/remove notices.
        self.diagnostic_header.set_visible(true);
        self.summary_icon.set_visible(issue_count == 0);
        if errors > 0 {
            self.summary_icon.add_css_class("status-error");
            let errors = if errors == 1 {
                "1 error".to_owned()
            } else {
                format!("{errors} errors")
            };
            if warnings == 0 {
                self.summary_title.set_text(&errors);
            } else {
                let warnings = if warnings == 1 {
                    "1 warning".to_owned()
                } else {
                    format!("{warnings} warnings")
                };
                self.summary_title
                    .set_text(&format!("{errors} · {warnings}"));
            }
        } else if warnings > 0 {
            self.summary_icon.add_css_class("status-warning");
            let warnings = if warnings == 1 {
                "1 warning".to_owned()
            } else {
                format!("{warnings} warnings")
            };
            self.summary_title.set_text(&warnings);
        } else if notes > 0 {
            self.summary_icon.add_css_class("status-note");
            self.summary_title.set_text(&format!(
                "{notes} compatibility {}",
                if notes == 1 { "note" } else { "notes" }
            ));
        } else {
            self.summary_icon.add_css_class("status-good");
            self.summary_title.set_text("All checks passed");
        }

        self.diagnostic_surface.set_visible(issue_count > 0);

        for issue in visible_issues {
            let (icon_name, class, severity_name) = match issue.severity {
                Severity::Error => ("dialog-error-symbolic", "status-error", "Error"),
                Severity::Warning => ("dialog-warning-symbolic", "status-warning", "Warning"),
                Severity::Note => ("dialog-information-symbolic", "status-note", "Note"),
            };
            let icon = gtk::Image::builder()
                .icon_name(icon_name)
                .pixel_size(16)
                .accessible_role(gtk::AccessibleRole::Presentation)
                .build();
            icon.add_css_class(class);
            icon.add_css_class("diagnostic-state");
            let ratio = issue
                .ratio
                .map_or_else(String::new, |ratio| format!(" · {ratio:.2}:1"));
            let full_detail = format!("{}{}", diagnostic_detail(issue), ratio);

            let row_content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row_content.add_css_class("diagnostic-row");
            let title_text = diagnostic_title(issue);
            let title = gtk::Label::new(Some(&title_text));
            title.set_xalign(0.0);
            title.set_hexpand(true);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.add_css_class("diagnostic-row-title");
            row_content.append(&icon);
            row_content.append(&title);
            if is_font_issue(issue) {
                let fonts = gtk::Button::with_label("Fonts");
                fonts.set_action_name(Some("win.diagnostic-fonts"));
                fonts.add_css_class("diagnostic-action");
                fonts.set_tooltip_text(Some("Choose a preview font without changing the prompt"));
                row_content.append(&fonts);
                if let Some(module) = issue_module(issue) {
                    let edit = gtk::Button::with_label("Edit");
                    edit.set_action_name(Some("win.diagnostic-module"));
                    edit.set_action_target_value(Some(
                        &format!("{}\n{}", module.id, issue.message).to_variant(),
                    ));
                    edit.add_css_class("diagnostic-action");
                    edit.set_tooltip_text(Some(&format!(
                        "Edit {}; changes stay in preview until saved",
                        module.label
                    )));
                    row_content.append(&edit);
                }
            }
            if let Some(ratio) = issue.ratio {
                let ratio = gtk::Label::new(Some(&format!("{ratio:.2}:1")));
                ratio.set_xalign(1.0);
                ratio.add_css_class("diagnostic-value");
                ratio.add_css_class(class);
                row_content.append(&ratio);
            }

            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);
            row.set_selectable(false);
            row.set_child(Some(&row_content));
            row.set_tooltip_text(Some(&full_detail));
            let accessible_label = issue.ratio.map_or_else(
                || format!("{severity_name}, {title_text}"),
                |ratio| format!("{severity_name}, {title_text}, {ratio:.2} to 1"),
            );
            row.update_property(&[
                gtk::accessible::Property::Label(&accessible_label),
                gtk::accessible::Property::Description(&full_detail),
            ]);
            self.diagnostics.append(&row);
        }
    }

    fn refresh_deployment(&self) {
        if !self.allows_document_action("install-ptyxis") {
            self.install_action.set_enabled(false);
            self.rollback_action.set_enabled(false);
            return;
        }
        // Rollback remains actionable even without a receipt so the request
        // can explain why there is nothing to restore instead of looking dead.
        self.rollback_action.set_enabled(true);
        if self.ptyxis_installer.is_none() {
            self.install_action.set_enabled(false);
            return;
        }

        if self.name_valid.get() && !self.color_picker.has_invalid_draft() {
            self.install_action.set_enabled(true);
        } else {
            self.install_action.set_enabled(false);
        }
    }

    fn palette_file_name(&self) -> String {
        let model = self.model.borrow();
        model
            .current_path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .filter(|name| {
                Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext == "palette")
            })
            .map(str::to_owned)
            .unwrap_or_else(|| palette_slug(model.palette.name()))
    }

    fn request_install(self: &Rc<Self>) {
        if !self.require_document_action("install-ptyxis") {
            return;
        }
        self.settle_active_edit();
        if !self.require_valid_name() {
            return;
        }
        let Some(installer) = &self.ptyxis_installer else {
            self.toast("Could not locate the Ptyxis theme directory");
            return;
        };
        let file_name = self.palette_file_name();
        let target = installer.palette_dir().join(&file_name);
        let model = self.model.borrow();
        let error_count = lint_palette(&model.palette, Target::Codex)
            .issues
            .iter()
            .filter(|issue| issue.severity == Severity::Error)
            .count();
        drop(model);

        let message = if target.exists() {
            "Replace the existing theme in Ptyxis?"
        } else {
            "Install this theme in Ptyxis?"
        };
        let mut detail = format!(
            "Destination: {}\nA backup and verified rollback record will be created before overwriting.",
            target.display()
        );
        let install_label = if error_count > 0 {
            detail.push_str(&format!(
                "\n\nReadability errors: {error_count}. Installing the theme will not fix them."
            ));
            "Install Anyway"
        } else {
            "Install"
        };

        let dialog = gtk::AlertDialog::builder()
            .message(message)
            .detail(detail)
            .buttons(["Cancel", install_label])
            .cancel_button(0)
            .default_button(i32::from(error_count == 0))
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if result == Ok(1)
                    && let Some(this) = weak.upgrade()
                {
                    this.install_now(&file_name);
                }
            },
        );
    }

    fn install_now(&self, file_name: &str) {
        if !self.require_valid_name() {
            return;
        }
        let Some(installer) = &self.ptyxis_installer else {
            return;
        };
        let contents = self.model.borrow().palette.to_palette_string();
        match installer.install(file_name, contents.as_bytes()) {
            Ok(InstallOutcome::Installed(receipt)) => {
                if receipt.backup().is_some() {
                    self.toast(
                        "Theme installed. The original file was backed up and can be restored.",
                    );
                } else {
                    self.toast("Theme installed in Ptyxis. You can roll it back at any time.");
                }
            }
            Ok(InstallOutcome::Unchanged(_)) => {
                self.toast("Ptyxis already has this exact version; no rollback was created.")
            }
            Err(error) => self.toast(&format!("Installation failed: {error}")),
        }
        self.refresh_deployment();
    }

    fn request_rollback(self: &Rc<Self>) {
        if !self.require_document_action("rollback-ptyxis") {
            return;
        }
        let Some(installer) = &self.ptyxis_installer else {
            self.toast("Could not locate the rollback directory");
            return;
        };
        let receipt = match installer.last_receipt() {
            Ok(Some(receipt)) => receipt,
            Ok(None) => {
                self.toast("No Ptyxis installation is available to roll back");
                return;
            }
            Err(error) => {
                self.toast(&format!("Could not read the rollback record: {error}"));
                return;
            }
        };
        let operation = if receipt.backup().is_some() {
            "Restore the pre-installation backup"
        } else {
            "Remove the theme added by TermiMochi"
        };
        let dialog = gtk::AlertDialog::builder()
            .message("Roll back the last Ptyxis installation?")
            .detail(format!(
                "Destination: {}\nAction: {}\n\nRollback will stop if another application modified the file after installation.",
                receipt.target().display(),
                operation
            ))
            .buttons(["Cancel", "Roll Back"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if result == Ok(1)
                    && let Some(this) = weak.upgrade()
                {
                    this.rollback_now();
                }
            },
        );
    }

    fn rollback_now(&self) {
        let Some(installer) = &self.ptyxis_installer else {
            return;
        };
        match installer.rollback() {
            Ok(RollbackOutcome::Restored { .. }) => self.toast(
                "Restored the pre-installation theme file. Your current edits are unchanged.",
            ),
            Ok(RollbackOutcome::Removed { .. }) => self
                .toast("Removed the newly installed theme file. Your current edits are unchanged."),
            Err(error) => self.toast(&format!("Rollback failed: {error}")),
        }
        self.refresh_deployment();
    }

    fn starship_file_error(&self, detail: &str) {
        gtk::AlertDialog::builder()
            .message("Starship configuration was not changed")
            .detail(detail)
            .buttons(["Close"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build()
            .choose(Some(&self.window()), gio::Cancellable::NONE, |_| {});
    }

    fn request_starship_save(self: &Rc<Self>) {
        if !self.require_document_action("save-starship") {
            return;
        }
        if self.prompt_source_selector.selected() == 1 || self.starship_editor.detached.get() {
            self.choose_starship_export();
            return;
        }
        if self.starship_editor.invalid() {
            self.toast("Fix the highlighted field before saving.");
            return;
        }
        let binding = self.starship_editor.file.borrow().clone();
        let file = match binding.and_then(|file| {
            file.verify()?;
            Ok(file)
        }) {
            Ok(file) => file,
            Err(error) => {
                self.starship_file_error(&error);
                return;
            }
        };
        let Some(contents) = self
            .starship_editor
            .draft
            .borrow()
            .as_ref()
            .map(|draft| draft.contents().to_owned())
        else {
            return;
        };
        let document = self.starship_editor.document();
        if contents == file.contents {
            self.starship_editor.accept_saved(document, file);
            self.toast("No changes to save.");
            return;
        }
        let dialog = gtk::AlertDialog::builder()
            .message("Save changes to your Starship configuration?")
            .detail(format!("{}\n\nA private backup of the current file will be kept beside it before replacement. New prompts in your terminal may use these changes. Shell startup files will not be changed.", file.path.display()))
            .buttons(["Cancel", "Back Up and Save"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |response| {
                if response == Ok(1)
                    && let Some(this) = weak.upgrade()
                {
                    this.commit_starship_save(document, &file, &contents);
                }
            },
        );
    }

    fn commit_starship_save(
        self: &Rc<Self>,
        document: u64,
        file: &crate::starship_file::FileSnapshot,
        contents: &str,
    ) {
        if document != self.starship_editor.document()
            || self.starship_editor.invalid()
            || self
                .starship_editor
                .draft
                .borrow()
                .as_ref()
                .is_none_or(|draft| draft.contents() != contents)
        {
            self.toast("The draft changed while confirming. Review it and save again.");
            return;
        }
        match file.save(contents) {
            Ok(saved) => {
                let backup = saved.backup;
                self.starship_editor.accept_saved(document, saved.snapshot);
                self.toast(&format!("Starship saved. Backup: {}", backup.display()));
                self.refresh_current_context();
            }
            Err(error) => self.starship_file_error(&error),
        }
    }

    fn request_starship_restore(self: &Rc<Self>) {
        if !self.require_document_action("restore-starship") {
            return;
        }
        let binding = self.starship_editor.file.borrow().clone();
        let (file, backup) = match binding.and_then(|file| {
            let backup = file.latest_backup()?;
            Ok((file, backup))
        }) {
            Ok(pair) => pair,
            Err(error) => {
                self.starship_file_error(&error);
                return;
            }
        };
        let document = self.starship_editor.document();
        let dialog = gtk::AlertDialog::builder()
            .message("Restore the previous Starship version?")
            .detail(format!("From: {}\nTo: {}\n\nThis replaces your current configuration and draft. The current file will be backed up first. Editing history stays available through Undo; shell startup files are unchanged.", backup.path.display(), file.path.display()))
            .buttons(["Cancel", "Back Up and Restore"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(Some(&self.window()), gio::Cancellable::NONE, move |response| {
            if response != Ok(1) { return; }
            let Some(this) = weak.upgrade() else { return; };
            if document != this.starship_editor.document() {
                this.toast("The loaded document changed. Review it and restore again.");
                return;
            }
            match backup.verify().and_then(|()| file.save(&backup.contents)) {
                Ok(saved) => {
                    this.starship_editor.accept_restored(document, saved.snapshot);
                    this.toast("Previous Starship version restored. The replaced version was backed up.");
                    this.refresh_current_context();
                }
                Err(error) => this.starship_file_error(&error),
            }
        });
    }

    fn request_starship_reload(self: &Rc<Self>) {
        if !self.require_document_action("reload-starship") {
            return;
        }
        if !self.starship_editor.dirty() {
            self.reload_starship_from_disk();
            return;
        }
        let dialog = gtk::AlertDialog::builder()
            .message("Discard prompt edits and reload from disk?")
            .detail("Unsaved prompt edits will be discarded. The configuration on disk will not be changed.")
            .buttons(["Cancel", "Reload"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |response| {
                if response == Ok(1)
                    && let Some(this) = weak.upgrade()
                {
                    this.reload_starship_from_disk();
                }
            },
        );
    }

    fn reload_starship_from_disk(self: &Rc<Self>) {
        if self.starship_editor.detached.get() {
            self.toast("This prompt is stored in a workspace. Reopen that workspace to reload it.");
            return;
        }
        let path = self
            .starship_editor
            .draft
            .borrow()
            .as_ref()
            .map(|draft| draft.source_path.clone())
            .or_else(|| {
                self.current_preview_context
                    .borrow()
                    .as_ref()
                    .map(|context| context.imported_prompt.path.clone())
            });
        let Some(path) = path else {
            self.refresh_current_context();
            return;
        };
        let result = crate::starship_file::FileSnapshot::read(&path).and_then(|file| {
            // Reload starts a new comparison baseline; preserve the old
            // preview if reading or validation fails.
            crate::starship_draft::StarshipDraft::new(path.clone(), file.contents.clone())?;
            self.reset_prompt_preview();
            self.redraw_preview_contents();
            self.starship_editor.begin(path, file.contents)
        });
        match result {
            Ok(()) => {
                self.sync_prompt_page();
                self.refresh_current_context();
            }
            Err(error) => self.starship_file_error(&error),
        }
    }

    fn choose_starship_export(self: &Rc<Self>) {
        if !self.require_document_action("export-starship") {
            return;
        }
        let copy = self.prompt_source_selector.selected() == 0;
        let document = self.starship_editor.document();
        if copy && self.starship_editor.invalid() {
            self.toast("Fix the highlighted module field before exporting.");
            return;
        }
        let exported_settings = (!copy).then(|| self.prompt_settings.borrow().clone());
        let copy_snapshot = if copy {
            let draft = self.starship_editor.draft.borrow();
            let Some(draft) = draft.as_ref() else {
                return;
            };
            Some((draft.source_path.clone(), draft.contents().to_owned()))
        } else {
            None
        };
        let contents = if let Some((_, contents)) = &copy_snapshot {
            contents.clone()
        } else {
            exported_settings.as_ref().unwrap().to_starship_toml()
        };
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Starship configuration"));
        filter.add_pattern("*.toml");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title("Save Starship As")
            .accept_label("Save As")
            .initial_name(if copy {
                "starship-copy.toml"
            } else {
                STARSHIP_FILE_NAME
            })
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build();
        let weak = Rc::downgrade(self);
        dialog.save(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(file) => {
                        let Some(path) = file.path() else {
                            this.toast("Only local files can be exported");
                            return;
                        };
                        if !is_starship_toml_path(&path) {
                            this.toast(
                                "Choose a .toml file; shell startup files are never overwritten",
                            );
                            return;
                        }
                        if let Some((source, contents)) = &copy_snapshot {
                            let guard = crate::starship_draft::StarshipDraft::new(source.clone(), contents.clone());
                            if let Err(error) = guard.and_then(|draft| draft.validate_destination(&path)) {
                                this.toast(&error); return;
                            }
                        }
                        if let Some(source) = this.model.borrow().current_path.clone() {
                            match paths_refer_to_same_file(&source, &path) {
                                Ok(true) => {
                                    this.toast(
                                        "The Starship export cannot overwrite the theme being edited",
                                    );
                                    return;
                                }
                                Ok(false) => {}
                                Err(error) => {
                                    this.toast(&format!(
                                        "Could not verify the export path: {error}"
                                    ));
                                    return;
                                }
                            }
                        }
                        let result = if path.exists() {
                            crate::starship_file::FileSnapshot::read(&path).and_then(|snapshot| snapshot.save(&contents)).map(|saved| Some(saved.backup))
                        } else if path.is_symlink() {
                            Err("Save As cannot replace a dangling symbolic link.".into())
                        } else {
                            write_atomically(&path, contents.as_bytes()).map(|()| None).map_err(|e| e.to_string())
                        };
                        match result {
                            Ok(backup) => {
                                if let Some(settings) = &exported_settings {
                                    *this.last_exported_prompt.borrow_mut() = settings.clone();
                                } else if document == this.starship_editor.document()
                                    && let Some(draft) = this.starship_editor.draft.borrow_mut().as_mut() {
                                    draft.mark_exported(contents.clone());
                                }
                                this.refresh_history_actions();
                                this.toast(&backup.map_or_else(
                                    || "Saved separately. Your active Starship configuration is unchanged.".into(),
                                    |path| format!("Saved separately. Previous destination backed up at {}", path.display()),
                                ));
                            }
                            Err(error) => this.toast(&format!("Export failed: {error}")),
                        }
                    }
                    Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                    Err(error) => this.toast(&format!("Could not export: {error}")),
                }
            },
        );
    }

    fn choose_export(self: &Rc<Self>, format: ExportFormat) {
        if !self.require_document_action("export-theme") {
            return;
        }
        self.settle_active_edit();
        if !self.require_valid_name() {
            return;
        }
        let exported = {
            let model = self.model.borrow();
            match export_palette(&model.palette, model.active_variant, format) {
                Ok(exported) => exported,
                Err(error) => {
                    self.toast(&format!("Could not export: {error}"));
                    return;
                }
            }
        };

        let filter = gtk::FileFilter::new();
        filter.set_name(Some(&format!("{} Theme", format.display_name())));
        filter.add_pattern(&format!("*.{}", format.extension()));
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title(format!("Export {} Theme", format.display_name()))
            .accept_label("Export")
            .initial_name(exported.suggested_file_name)
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build();
        let contents = exported.contents;
        let weak = Rc::downgrade(self);
        dialog.save(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(file) => {
                        if let Some(path) = file.path() {
                            if let Some(source) = this.model.borrow().current_path.clone() {
                                match paths_refer_to_same_file(&source, &path) {
                                    Ok(true) => {
                                        this.toast(
                                            "An export cannot overwrite the theme you are editing",
                                        );
                                        return;
                                    }
                                    Ok(false) => {}
                                    Err(error) => {
                                        this.toast(&format!(
                                            "Could not verify the export path: {error}"
                                        ));
                                        return;
                                    }
                                }
                            }
                            match write_atomically(&path, contents.as_bytes()) {
                                Ok(()) => {
                                    this.toast(&format!("Exported {} theme", format.display_name()))
                                }
                                Err(error) => this.toast(&format!("Export failed: {error}")),
                            }
                        } else {
                            this.toast("Only local files can be exported");
                        }
                    }
                    Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                    Err(error) => this.toast(&format!("Could not export: {error}")),
                }
            },
        );
    }

    fn choose_open(self: &Rc<Self>) {
        self.choose_design_open();
    }

    fn confirm_discard<F>(self: &Rc<Self>, action: F)
    where
        F: FnOnce(Rc<Self>) + 'static,
    {
        self.settle_active_edit();
        if (!self.model.borrow().dirty && !self.has_draft()) || self.workspace_is_clean() {
            action(Rc::clone(self));
            return;
        }

        let dialog = gtk::AlertDialog::builder()
            .message("Discard unsaved changes?")
            .detail("Changes made since the last save will be lost.")
            .buttons(["Cancel", "Discard Changes"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if result != Ok(1) {
                    return;
                }
                if let Some(this) = weak.upgrade() {
                    action(this);
                }
            },
        );
    }

    fn confirm_close_discard(self: &Rc<Self>) {
        self.settle_active_edit();
        if !self.has_unsaved_setup() {
            self.window().close();
            return;
        }
        let dialog = gtk::AlertDialog::builder()
            .message("Discard unsaved setup changes?")
            .detail("Unsaved colors, typography, layout, prompt or greeting edits will be lost. Save Workspace to keep the complete setup. Saved files and applied terminal settings are kept.")
            .buttons(["Cancel", "Discard Changes"])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                if result != Ok(1) {
                    return;
                }
                if let Some(this) = weak.upgrade() {
                    this.model.borrow_mut().dirty = false;
                    this.discard_drafts();
                    let current_prompt = this.prompt_settings.borrow().clone();
                    *this.last_exported_prompt.borrow_mut() = current_prompt;
                    this.starship_editor.discard_warning();
                    *this.typography_baseline.borrow_mut() = this.typography_settings();
                    this.layout_baseline.set(this.layout_settings());
                    this.workspace_baseline.borrow_mut().take();
                    this.greeting.replace(this.greeting.settings(), false);
                    *this.greeting.baseline.borrow_mut() = this.greeting.settings();
                    this.window().close();
                }
            },
        );
    }

    fn show_appearance_source(&self, appearance: &CurrentTerminalAppearance) {
        self.appearance_source.set_text(&appearance.source_label);
        let typography = &appearance.typography;
        let layout = &appearance.layout;
        self.appearance_details.set_text(&format!(
            "{} · {} pt · {} × {}\nProfile snapshot, not an attached terminal tab.",
            typography.family, typography.size, layout.columns, layout.rows,
        ));
        self.appearance_details
            .set_tooltip_text(Some(&appearance.notices.join("\n")));
    }

    fn reload_terminal_appearance(self: &Rc<Self>) {
        let mut appearance = import_current_appearance();
        let Some(palette) = appearance.palette.take() else {
            self.toast("Terminal palette unavailable. Your current theme is unchanged.");
            return;
        };
        self.show_appearance_source(&appearance);
        self.finish_active_edit();
        let preferred = appearance.preferred_variant.unwrap_or(Variant::Light);
        let variant = if palette.variant(preferred).is_some() {
            preferred
        } else if palette.variant(Variant::Light).is_some() {
            Variant::Light
        } else {
            Variant::Dark
        };
        *self.model.borrow_mut() =
            Model::new(palette, variant, None, appearance.source_label.clone());
        self.history.borrow_mut().clear();
        *self.selected_color_key.borrow_mut() = "Foreground".to_owned();
        self.updating.set(true);
        let typography = &appearance.typography;
        let (names, index) = monospace_font_choices(&self.preview_terminal, typography);
        let names: Vec<_> = names.iter().map(String::as_str).collect();
        self.font_family_selector
            .set_model(Some(&gtk::StringList::new(&names)));
        self.font_family_selector.set_selected(index);
        self.font_size_input.set_value(typography.size);
        self.font_weight_selector
            .set_selected(typography.weight.index());
        self.line_height_input.set_value(typography.line_height);
        self.cell_width_input.set_value(typography.cell_width);
        let layout = &appearance.layout;
        self.content_padding_input
            .set_value(f64::from(layout.content_padding));
        self.column_count_input.set_value(layout.columns as f64);
        self.row_count_input.set_value(layout.rows as f64);
        self.cursor_shape_selector
            .set_selected(layout.cursor_shape.index());
        self.cursor_blink_selector
            .set_selected(layout.cursor_blink.index());
        self.tab_bar_switch.set_active(layout.tab_bar);
        self.scrollbar_switch.set_active(layout.scrollbar);
        self.window_spacing_input
            .set_value(f64::from(layout.window_spacing));
        self.preview_terminal
            .set_bold_is_bright(appearance.bold_is_bright);
        self.preview_terminal
            .set_cjk_ambiguous_width(appearance.cjk_ambiguous_width);
        self.updating.set(false);
        self.preview_input.borrow_mut().reset();
        self.refresh_all();
        self.toast("Terminal appearance reloaded. Prompt design is unchanged.");
    }

    fn choose_preview_folder(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Choose Preview Folder")
            .accept_label("Preview")
            .initial_folder(&gio::File::for_path(&*self.preview_directory.borrow()))
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        dialog.select_folder(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(file) => {
                        let Some(path) = file.path() else {
                            this.toast("Choose a local folder for preview");
                            return;
                        };
                        *this.preview_directory.borrow_mut() = path;
                        this.current_preview_context.borrow_mut().take();
                        this.reset_prompt_preview();
                        let navigating = this.navigating_preview.replace(true);
                        this.preview_selector
                            .set_selected((PreviewScenario::ALL.len() - 1) as u32);
                        this.prompt_preview_selector.set_selected(0);
                        this.navigating_preview.set(navigating);
                        this.refresh_current_context();
                    }
                    Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                    Err(error) => this.toast(&format!("Could not open folder: {error}")),
                }
            },
        );
    }

    fn refresh_current_context(self: &Rc<Self>) {
        self.greeting_official_result.borrow_mut().take();
        self.ensure_official_greeting_preview();
        let generation = self.preview_generation.get().wrapping_add(1);
        self.preview_generation.set(generation);
        // Coalesce repeated refreshes into one follow-up instead of spawning
        // unbounded probes while the previous folder is still being read.
        if self.preview_loading.replace(true) {
            return;
        }
        self.preview_context_source
            .set_text("Reading folder context…");
        self.prompt_import_status.set_text("Reading starship.toml…");
        let directory = self.preview_directory.borrow().clone();
        let columns = usize::try_from(self.preview_terminal.column_count()).unwrap_or(80);
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(CurrentPreviewContext::load_for_width(directory, columns));
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(40), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            match receiver.try_recv() {
                Ok(context) => {
                    this.preview_loading.set(false);
                    if this.preview_generation.get() != generation {
                        this.refresh_current_context();
                        return glib::ControlFlow::Break;
                    }
                    this.preview_context_source.set_text(&format!(
                        "{} · {}\n{}",
                        context.shell,
                        context.path,
                        context.imported_prompt.label(),
                    ));
                    this.preview_context_source.set_tooltip_text(Some(&format!(
                        "{}\n{}\n{}",
                        context.detail,
                        context.imported_prompt.path.display(),
                        context.imported_prompt.detail
                    )));
                    let notice = if context.imported_prompt.ansi.is_some() {
                        context
                            .imported_prompt
                            .detail
                            .split_once("\nSkipped")
                            .map(|(_, rest)| format!("\n\nSkipped{rest}"))
                            .unwrap_or_default()
                    } else {
                        format!("\n\n{}", context.imported_prompt.detail)
                    };
                    this.prompt_import_status.set_text(&format!(
                        "{}\n{}{}",
                        context.imported_prompt.label(),
                        context.imported_prompt.path.display(),
                        notice
                    ));
                    this.prompt_import_status
                        .set_tooltip_text(Some(&context.imported_prompt.detail));
                    *this.current_preview_context.borrow_mut() = Some(context);
                    if this.prompt_module_button.is_active() {
                        this.sync_prompt_page();
                    }
                    if this.starship_editor.draft.borrow().is_some() {
                        this.schedule_copy_preview();
                    }
                    this.redraw_preview_contents();
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    this.preview_loading.set(false);
                    if this.preview_generation.get() != generation {
                        this.refresh_current_context();
                        return glib::ControlFlow::Break;
                    }
                    this.preview_context_source
                        .set_text("Folder context unavailable. You can still use sample scenes.");
                    this.prompt_import_status.set_text(
                        "Starship preview unavailable. Try reloading or switch to Designer.",
                    );
                    glib::ControlFlow::Break
                }
            }
        });
    }

    fn save(self: &Rc<Self>) {
        self.save_design(false);
    }

    fn save_theme(self: &Rc<Self>) {
        if !self.require_document_action("save-theme") {
            return;
        }
        self.settle_active_edit();
        if !self.require_valid_name() {
            return;
        }
        let path = self.model.borrow().current_path.clone();
        if let Some(path) = path {
            self.write_to(&path);
        } else {
            self.choose_theme_save_as();
        }
    }

    fn choose_save_as(self: &Rc<Self>) {
        self.save_design(true);
    }

    fn choose_theme_save_as(self: &Rc<Self>) {
        if !self.require_document_action("export-theme") {
            return;
        }
        self.settle_active_edit();
        if !self.require_valid_name() {
            return;
        }
        let model = self.model.borrow();
        let suggested_name = model
            .current_path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| palette_slug(model.palette.name()));
        drop(model);
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Ptyxis palette"));
        filter.add_pattern("*.palette");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title("Save Ptyxis Theme")
            .accept_label("Save")
            .initial_name(suggested_name)
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build();
        let weak = Rc::downgrade(self);
        dialog.save(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(file) => {
                        if let Some(path) = file.path() {
                            this.write_to(&path);
                        } else {
                            this.toast("Only local files can be saved");
                        }
                    }
                    Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                    Err(error) => this.toast(&format!("Could not save: {error}")),
                }
            },
        );
    }

    fn write_to(&self, path: &Path) {
        self.settle_active_edit();
        if !self.require_valid_name() {
            return;
        }
        let serialized = self.model.borrow().palette.to_palette_string();
        match write_atomically(path, serialized.as_bytes()) {
            Ok(()) => {
                let mut model = self.model.borrow_mut();
                model.current_path = Some(path.to_owned());
                model.palette.set_source(Some(path.to_owned()));
                model.source_label = "Local File".to_owned();
                model.mark_saved();
                drop(model);
                self.refresh_titles();
                self.refresh_history_actions();
                self.toast("Theme saved");
            }
            Err(error) => self.toast(&format!("Save failed: {error}")),
        }
    }

    fn toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(4);
        self.toast_overlay.add_toast(toast);
    }

    fn require_valid_name(&self) -> bool {
        if !self.name_valid.get() {
            self.toast("Fix the theme name before saving, exporting, or installing");
            self.name_entry.grab_focus();
            return false;
        }
        if self.color_picker.has_invalid_draft() {
            self.toast("Fix the invalid HEX or RGB value, or press Ctrl+Z to discard it");
            return false;
        }
        true
    }
}

fn set_name_entry_error(entry: &gtk::Entry, message: Option<&str>) {
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

fn save_button_state(dirty: bool, has_path: bool, inputs_valid: bool) -> (bool, bool) {
    let enabled = inputs_valid && (dirty || !has_path);
    let emphasized = enabled && dirty;
    (enabled, emphasized)
}

#[cfg(test)]
fn has_unexported_prompt_changes(current: &PromptSettings, last_exported: &PromptSettings) -> bool {
    current != last_exported
}

fn set_metric(value_label: &gtk::Label, ratio: Option<f64>, threshold: f64, relationship: &str) {
    remove_status_classes(value_label);
    let description = if let Some(ratio) = ratio {
        value_label.set_text(&format!("{ratio:.2}:1"));
        if ratio >= threshold {
            value_label.add_css_class("status-good");
            format!("{relationship} · Pass · Minimum {threshold:.1}:1")
        } else {
            value_label.add_css_class("status-error");
            format!("{relationship} · Low · Minimum {threshold:.1}:1")
        }
    } else {
        value_label.set_text("Missing");
        value_label.add_css_class("status-warning");
        format!("{relationship} · Color missing")
    };
    value_label.set_tooltip_text(Some(&description));
    value_label.update_property(&[gtk::accessible::Property::Description(&description)]);
}

fn remove_status_classes(widget: &impl IsA<gtk::Widget>) {
    widget.remove_css_class("status-note");
    widget.remove_css_class("status-good");
    widget.remove_css_class("status-warning");
    widget.remove_css_class("status-error");
}

fn color_display_name(key: &str) -> String {
    if let Some((_, title, _)) = BASIC_COLORS
        .iter()
        .find(|(candidate, _, _)| *candidate == key)
    {
        if *title == key {
            return (*title).to_owned();
        }
        return format!("{title} · {key}");
    }
    if let Some(index) = key
        .strip_prefix("Color")
        .and_then(|index| index.parse::<usize>().ok())
        .filter(|index| *index < ANSI_NAMES.len())
    {
        return format!("Color{index} · {}", ANSI_NAMES[index]);
    }
    key.to_owned()
}

fn rgba_from_rgb(color: Rgb) -> gdk::RGBA {
    gdk::RGBA::new(
        f32::from(color.red()) / 255.0,
        f32::from(color.green()) / 255.0,
        f32::from(color.blue()) / 255.0,
        1.0,
    )
}

fn diagnostic_title(issue: &Issue) -> String {
    if is_font_issue(issue) {
        return issue
            .message
            .lines()
            .next()
            .unwrap_or("Prompt compatibility")
            .to_owned();
    }
    match issue.code {
        "variant-passed" => "All checks passed".to_owned(),
        "missing-required-color" => issue
            .message
            .strip_prefix("missing required color ")
            .map_or_else(
                || "Missing color".to_owned(),
                |key| format!("Missing {key}"),
            ),
        "body-text-low-contrast" => "Text contrast".to_owned(),
        "cursor-low-contrast" => "Cursor contrast".to_owned(),
        "ansi-color-low-contrast" => issue
            .message
            .split_once(" may be hard to read")
            .map_or_else(
                || "ANSI contrast".to_owned(),
                |(key, _)| color_display_name(key),
            ),
        "codex-composer-low-contrast" => "Codex input contrast".to_owned(),
        code => code.to_owned(),
    }
}

fn diagnostic_detail(issue: &Issue) -> String {
    if is_font_issue(issue) {
        return issue
            .message
            .split_once('\n')
            .map_or_else(|| issue.message.clone(), |(_, detail)| detail.to_owned());
    }
    match issue.code {
        "variant-passed" => "No readability issues were found for this variant".to_owned(),
        "missing-required-color" => format!("{}. Required for previews and exports", issue.message),
        "ansi-color-low-contrast" => {
            format!("{}. Recommended contrast is at least 3.0:1", issue.message)
        }
        "body-text-low-contrast" => {
            "Foreground and Background need a contrast ratio of at least 4.5:1".to_owned()
        }
        "cursor-low-contrast" => {
            "Cursor and Background should have a contrast ratio of at least 3.0:1".to_owned()
        }
        "codex-composer-low-contrast" => {
            "Codex uses Foreground text on a Color0 background; contrast must be at least 4.5:1"
                .to_owned()
        }
        _ => issue.message.clone(),
    }
}

fn is_starship_toml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
}

fn palette_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut separator_pending = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if separator_pending && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character.to_ascii_lowercase());
            separator_pending = false;
        } else if !slug.is_empty() {
            separator_pending = true;
        }
    }
    if slug.is_empty() {
        slug.push_str("termimochi");
    }
    slug.push_str(".palette");
    slug
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt_diagnostics::is_rust_issue;

    #[test]
    #[ignore = "requires an X11 GTK/VTE session; run with --ignored --test-threads=1"]
    fn typography_font_selection_reaches_live_vte() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.TypographyTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let preset_path = fixture.path().join(typography_preset::PRESET_NAME);
        present_advanced_with_preset(&app, None, preset_path.clone());
        let window = app.active_window().unwrap();
        let this = greeting::tests::project_controller(&window);
        let settle = || {
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_millis(350) {
                while glib::MainContext::default().pending() {
                    glib::MainContext::default().iteration(false);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        settle();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-typography", None);
        assert!(
            !this.typography_dirty(),
            "startup must not create a font edit"
        );
        this.font_size_input.set_value(11.0);
        let model = this.font_family_selector.model().unwrap();
        let mut families: Vec<_> = (0..model.n_items())
            .map(|index| {
                let family = model
                    .item(index)
                    .unwrap()
                    .downcast::<gtk::StringObject>()
                    .unwrap();
                (index, family.string().to_string())
            })
            .filter(|(_, family)| {
                matches!(
                    family.as_str(),
                    "DejaVu Sans Mono" | "Iosevka Nerd Font Mono" | "JetBrainsMono Nerd Font Mono"
                )
            })
            .collect();
        if families.len() < 2 {
            families = (0..model.n_items().min(3))
                .map(|index| {
                    let family = model
                        .item(index)
                        .unwrap()
                        .downcast::<gtk::StringObject>()
                        .unwrap();
                    (index, family.string().to_string())
                })
                .collect();
        }
        assert!(
            families.len() >= 2,
            "install at least two monospace families for this test"
        );
        let palette = this.model.borrow().palette.clone();
        fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
            let mut result = vec![widget.clone()];
            let mut child = widget.first_child();
            while let Some(current) = child {
                result.extend(descendants(&current));
                child = current.next_sibling();
            }
            result
        }
        for (index, family) in families {
            let toggle = this
                .font_family_selector
                .first_child()
                .unwrap()
                .downcast::<gtk::ToggleButton>()
                .unwrap();
            toggle.set_active(true);
            settle();
            let nodes = descendants(this.font_family_selector.upcast_ref());
            let search = nodes
                .iter()
                .find_map(|node| node.clone().downcast::<gtk::SearchEntry>().ok())
                .unwrap();
            search.set_text(&family);
            settle();
            let list = nodes
                .iter()
                .find_map(|node| node.clone().downcast::<gtk::ListView>().ok())
                .unwrap();
            let filtered = list.model().unwrap();
            assert!(filtered.n_items() > 0, "font search must find {family}");
            assert_eq!(
                filtered
                    .item(0)
                    .unwrap()
                    .downcast::<gtk::StringObject>()
                    .unwrap()
                    .string(),
                family
            );
            let label = descendants(list.upcast_ref())
                .into_iter()
                .find_map(|node| {
                    node.downcast::<gtk::Label>()
                        .ok()
                        .filter(|label| label.text() == family)
                })
                .unwrap();
            let bounds = label.compute_bounds(&window).unwrap();
            let (dx, dy) = window.surface_transform();
            let scale = f64::from(window.scale_factor());
            let x = ((f64::from(bounds.x() + bounds.width() / 2.0) + dx) * scale).round() as i32;
            let y = ((f64::from(bounds.y() + bounds.height() / 2.0) + dy) * scale).round() as i32;
            window.set_title(Some("TermiMochi point-to-edit test"));
            let mut child = std::process::Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../scripts/preview-pointer-driver.py"
                ))
                .args(["click", &x.to_string(), &y.to_string()])
                .spawn()
                .unwrap();
            while child.try_wait().unwrap().is_none() {
                settle();
            }
            assert!(child.wait().unwrap().success());
            settle();
            assert!(!toggle.is_active(), "selecting a font closes its popup");
            assert_eq!(this.font_family_selector.selected(), index);
            assert_eq!(this.typography_settings().family, family);
            let applied = this.preview_terminal.font().unwrap();
            assert_eq!(applied.family().as_deref(), Some(family.as_str()));
            let resolved = this
                .preview_terminal
                .pango_context()
                .load_font(&applied)
                .unwrap();
            assert_eq!(resolved.face().unwrap().family().name(), family);
            eprintln!(
                "selected={family}; VTE={applied}; cell={}x{}",
                this.preview_terminal.char_width(),
                this.preview_terminal.char_height()
            );
            if let Some(directory) = std::env::var_os("TERMIMOCHI_FONT_SCREENSHOT_DIR") {
                std::fs::create_dir_all(&directory).unwrap();
                window.set_title(Some("TermiMochi point-to-edit test"));
                let mut child = std::process::Command::new("python3")
                    .arg(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../scripts/preview-pointer-driver.py"
                    ))
                    .args(["capture", "0", "0"])
                    .env(
                        "TERMIMOCHI_INSPECT_SCREENSHOT",
                        PathBuf::from(directory).join(format!("font-{index}.png")),
                    )
                    .spawn()
                    .unwrap();
                while child.try_wait().unwrap().is_none() {
                    settle();
                }
                assert!(child.wait().unwrap().success());
            }
            for action in ["show-prompt", "show-palette", "show-typography"] {
                gio::prelude::ActionGroupExt::activate_action(&this.window(), action, None);
                settle();
                assert_eq!(this.preview_terminal.font().unwrap(), applied);
            }
        }
        let height = this.preview_terminal.char_height();
        this.font_size_input.set_value(24.0);
        settle();
        assert!(this.preview_terminal.char_height() > height);
        assert!(this.typography_dirty());
        this.save_typography_preset(); // Explicit preset action, not main Save.
        let saved = this.typography_settings();
        assert_eq!(
            PresetStore::open(preset_path.clone())
                .unwrap()
                .settings()
                .unwrap(),
            Some(saved.clone())
        );
        assert!(!this.typography_dirty());
        assert!(
            this.typography_status
                .text()
                .contains("restored on next launch")
        );
        this.undo_action.activate(None);
        assert!(this.typography_dirty());
        this.redo_action.activate(None);
        assert!(!this.typography_dirty());
        assert_eq!(this.typography_settings(), saved);
        this.font_size_input.set_value(13.5);
        this.font_weight_selector
            .set_selected(PreviewFontWeight::Semibold.index());
        this.line_height_input.set_value(1.25);
        this.cell_width_input.set_value(1.1);
        this.save_typography_preset(); // Explicit preset action, not main Save.
        let saved = this.typography_settings();
        let respond = |label: &str| {
            settle();
            let button = gtk::Window::list_toplevels()
                .into_iter()
                .flat_map(|widget| descendants(&widget))
                .find_map(|widget| {
                    widget
                        .downcast::<gtk::Button>()
                        .ok()
                        .filter(|button| button.label().as_deref() == Some(label))
                })
                .unwrap_or_else(|| panic!("dialog button missing: {label}"));
            button.emit_clicked();
            settle();
        };
        this.font_size_input.set_value(15.0);
        window.close();
        respond("Cancel");
        assert!(window.is_visible());
        assert!(this.typography_dirty());
        this.reload_typography_preset();
        respond("Cancel");
        assert_eq!(this.font_size_input.value(), 15.0);
        this.reload_typography_preset();
        respond("Replace Changes");
        assert_eq!(this.typography_settings(), saved);
        assert!(!this.typography_dirty());
        // Opening an Apply confirmation is read-only; this test only cancels.
        if TypographyTarget::discover().is_ok() {
            let before = import_current_appearance().typography;
            this.request_typography_apply();
            respond("Cancel");
            assert_eq!(import_current_appearance().typography, before);
        }
        let mut external = saved.clone();
        external.size = 14.0;
        PresetStore::open(preset_path.clone())
            .unwrap()
            .save(&external)
            .unwrap();
        this.font_size_input.set_value(16.0);
        this.save_typography_preset(); // Explicit preset action, not main Save.
        assert!(
            this.typography_dirty(),
            "conflicted save must not mark the draft clean"
        );
        assert_eq!(
            PresetStore::open(preset_path.clone())
                .unwrap()
                .settings()
                .unwrap(),
            Some(external.clone())
        );
        this.reload_typography_preset();
        respond("Replace Changes");
        assert_eq!(this.typography_settings(), external);
        assert!(!this.typography_dirty());
        let saved = external;
        assert_eq!(this.model.borrow().palette, palette);
        window.destroy();
        present_advanced_with_preset(&app, None, preset_path);
        let restored_window = app.active_window().unwrap();
        let restored = greeting::tests::project_controller(&restored_window);
        settle();
        assert_eq!(
            restored.typography_settings(),
            saved,
            "reopening the app restores all saved font fields"
        );
        assert_eq!(
            restored.preview_terminal.font().unwrap(),
            saved.font_description()
        );
        assert!(!restored.typography_dirty());
        restored_window.destroy();
    }

    #[test]
    #[ignore = "requires a graphical GTK session; run with --ignored --test-threads=1"]
    fn light_chrome_pages_and_native_controls() {
        adw::init().unwrap();
        for modern in [false, true] {
            if modern && gtk::check_version(4, 16, 0).is_some() {
                continue;
            }
            let errors = Rc::new(RefCell::new(Vec::new()));
            let captured = errors.clone();
            let provider = gtk::CssProvider::new();
            provider.connect_parsing_error(move |_, _, error| {
                captured.borrow_mut().push(error.to_string())
            });
            provider.load_from_data(&chrome_css(modern));
            assert!(
                errors.borrow().is_empty(),
                "CSS parse errors: {:?}",
                errors.borrow()
            );
        }
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.ChromeTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        present(&app, None);
        let window = app.active_window().unwrap();
        let this = greeting::tests::project_controller(&window);
        let settle = || {
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_millis(400) {
                while glib::MainContext::default().pending() {
                    glib::MainContext::default().iteration(false);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        settle();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        let palette = this.model.borrow().palette.clone();
        let screenshots = std::env::var_os("TERMIMOCHI_CHROME_SCREENSHOT_DIR").map(PathBuf::from);
        let capture = |name: &str| {
            let Some(directory) = &screenshots else {
                return;
            };
            std::fs::create_dir_all(directory).unwrap();
            window.set_title(Some("TermiMochi point-to-edit test"));
            settle();
            let mut child = std::process::Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../scripts/preview-pointer-driver.py"
                ))
                .args(["capture", "0", "0"])
                .env(
                    "TERMIMOCHI_INSPECT_SCREENSHOT",
                    directory.join(format!("{name}.png")),
                )
                .spawn()
                .unwrap();
            while child.try_wait().unwrap().is_none() {
                settle();
            }
            assert!(child.wait().unwrap().success());
        };
        this.palette_module_button.set_active(true);
        this.name_entry.grab_focus();
        capture("palette");
        let scene_button = this
            .preview_selector
            .first_child()
            .unwrap()
            .downcast::<gtk::ToggleButton>()
            .expect("native scene dropdown toggle");
        scene_button.set_active(true);
        capture("preview-scene-menu");
        assert!(scene_button.is_active());
        scene_button.set_active(false);
        this.dark_button.set_active(true);
        this.name_entry.grab_focus();
        capture("palette-dark");
        this.light_button.set_active(true);
        gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-typography", None);
        settle();
        capture("typography");
        let font_button = this
            .font_family_selector
            .first_child()
            .unwrap()
            .downcast::<gtk::ToggleButton>()
            .expect("native font dropdown toggle");
        font_button.set_active(true);
        capture("font-menu");
        font_button.set_active(false);
        gio::prelude::ActionGroupExt::activate_action(&this.window(), "show-layout", None);
        this.row_count_input.grab_focus();
        capture("layout");
        this.prompt_module_button.set_active(true);
        // Editing a module no longer selects its observation scene. The A/B
        // checks below explicitly exercise the dedicated Prompt scene.
        this.preview_scene_selector.set_selected(2);
        settle();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while this.copy_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        this.starship_editor.symbol.grab_focus();
        capture("prompt");
        if this.starship_editor.draft.borrow().is_some() {
            this.starship_editor.symbols[1].emit_clicked();
            settle();
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while this.copy_loading.get() {
                assert!(std::time::Instant::now() < deadline);
                settle();
            }
            capture("prompt-edited");
            this.prompt_compare_selector.set_selected(1);
            capture("prompt-original");
            this.prompt_compare_selector.set_selected(0);
            this.starship_editor.undo();
            settle();
        }
        this.output_bar.more_button().popup();
        capture("save-menu");
        this.output_bar.more_button().popdown();
        this.prompt_source_selector.set_selected(1);
        capture("designer");
        let designer_original = this.prompt_settings.borrow().clone();
        this.update_prompt_settings(|settings| {
            settings.set_add_newline(!settings.add_newline());
        });
        let designer_edited = this.prompt_settings.borrow().clone();
        assert_ne!(designer_original, designer_edited);
        capture("designer-edited");
        let compare_button = this
            .prompt_compare_selector
            .first_child()
            .unwrap()
            .downcast::<gtk::ToggleButton>()
            .expect("native compare dropdown toggle");
        compare_button.set_active(true);
        capture("preview-compare-menu");
        assert!(compare_button.is_active());
        compare_button.set_active(false);
        this.prompt_compare_selector.set_selected(1);
        assert_eq!(this.preview_prompt_settings(), designer_original);
        assert_eq!(*this.prompt_settings.borrow(), designer_edited);
        capture("designer-original");
        this.prompt_compare_selector.set_selected(0);
        assert_eq!(this.preview_prompt_settings(), designer_edited);
        this.restore_prompt_settings(designer_original);
        this.prompt_add_button.popup();
        capture("module-menu");
        this.prompt_add_button.popdown();
        #[allow(deprecated)] // Verify the GTK 4.10 named-color fallback too.
        let accent = this
            .starship_editor
            .symbol
            .style_context()
            .lookup_color("accent_bg_color")
            .unwrap();
        assert!(
            (accent.red() - accent.blue()).abs() < 0.08
                && (accent.green() - accent.blue()).abs() < 0.08,
            "accent is not neutral: {accent}"
        );
        let color = this.starship_editor.symbol.color();
        assert!(
            color.red() < 0.25 && color.green() < 0.25 && color.blue() < 0.25,
            "entry text must remain legible on white: {color}"
        );
        assert_eq!(
            this.model.borrow().palette,
            palette,
            "chrome must never modify the terminal palette"
        );
        window.destroy();
    }

    #[test]
    #[ignore = "requires a graphical GTK/VTE session; run with --ignored --test-threads=1"]
    fn point_to_edit_real_vte_navigation() {
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        gtk::IconTheme::for_display(&gdk::Display::default().unwrap())
            .add_resource_path(&format!("{}/icons", crate::RESOURCE_BASE));
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.InspectTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        present(&app, None);
        let window = app.active_window().unwrap();
        // present stores exactly this Rc under this private key.
        let this = greeting::tests::project_controller(&window);
        let settle = || {
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_millis(350) {
                while glib::MainContext::default().pending() {
                    glib::MainContext::default().iteration(false);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        settle();
        // Wait for the bounded context worker before installing each scene.
        while this.preview_loading.get() {
            settle();
        }
        let terminal = &this.preview_terminal;
        let snapshot = || {
            terminal
                .text_range_format(
                    vte::Format::Text,
                    0,
                    0,
                    terminal.cursor_position().1,
                    terminal.column_count(),
                )
                .0
                .unwrap_or_default()
        };
        let at = |col: i64, row: i64| {
            let (top, fraction) =
                preview_visible_origin(terminal).expect("unique visible VTE range");
            let native = terminal.native().unwrap();
            let (dx, dy) = native.surface_transform();
            let native = native.dynamic_cast::<gtk::Widget>().unwrap();
            let p = terminal
                .compute_point(
                    &native,
                    &gtk::graphene::Point::new(
                        (col as f32 + 0.5) * terminal.char_width() as f32,
                        ((row as f64 - top as f64 - fraction) as f32 + 0.5)
                            * terminal.char_height() as f32,
                    ),
                )
                .unwrap();
            this.preview_target_at(f64::from(p.x()) + dx, f64::from(p.y()) + dy)
        };
        this.select_preview_scene(preview_scene::PreviewScene::Terminal);
        this.preview_selector.set_selected(0);
        settle();
        let first_row = terminal.cursor_position().1 - 7;
        terminal.vadjustment().unwrap().set_value(first_row as f64);
        settle();
        let cell = terminal
            .text_range_format(vte::Format::Text, first_row, 0, first_row, 1)
            .0
            .unwrap_or_default();
        assert_eq!(cell, "m", "VTE range endpoints are exclusive");
        assert!(matches!(
            at(0, first_row),
            Some(PreviewTarget::Ansi(2 | 10))
        ));
        assert_eq!(at(0, first_row + 7), Some(PreviewTarget::Typography));
        let before = snapshot();
        let dirty = this.model.borrow().dirty;
        let position = terminal.vadjustment().unwrap().value();
        let generation = this.copy_generation.get();
        for action in [
            "show-prompt",
            "show-typography",
            "show-layout",
            "show-palette",
        ] {
            gio::prelude::ActionGroupExt::activate_action(&this.window(), action, None);
            settle();
            assert_eq!(
                snapshot(),
                before,
                "Activity Rail must not replace the preview"
            );
            assert_eq!(terminal.vadjustment().unwrap().value(), position);
            assert_eq!(
                this.copy_generation.get(),
                generation,
                "navigation must not queue a prompt scene"
            );
        }
        assert!(
            !this.inspect_button.is_active(),
            "point-to-edit must be opt-in"
        );
        if std::env::var_os("TERMIMOCHI_POINTER_TEST").is_some() {
            window.set_title(Some("TermiMochi point-to-edit test"));
            let pointer = |mode: &str| {
                // The canvas now scrolls independently; this specimen targets
                // its first row, not the caret-following viewport at the end.
                this.preview_terminal_viewport.vadjustment().set_value(0.0);
                settle();
                window.set_title(Some("TermiMochi point-to-edit test"));
                gtk::prelude::WidgetExt::display(&window).flush();
                let native = terminal.native().unwrap();
                let (dx, dy) = native.surface_transform();
                let native = native.dynamic_cast::<gtk::Widget>().unwrap();
                let p = terminal
                    .compute_point(
                        &native,
                        &gtk::graphene::Point::new(
                            if mode == "blank" {
                                terminal.width() as f32 - 5.0
                            } else {
                                terminal.char_width() as f32 * 0.5
                            },
                            terminal.char_height() as f32 * 0.5,
                        ),
                    )
                    .unwrap();
                let mut child = std::process::Command::new("python3")
                    .arg(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../scripts/preview-pointer-driver.py"
                    ))
                    .arg(if mode == "blank" { "click" } else { mode })
                    .arg(
                        (((f64::from(p.x()) + dx) * f64::from(terminal.scale_factor())) as i32)
                            .to_string(),
                    )
                    .arg(
                        (((f64::from(p.y()) + dy) * f64::from(terminal.scale_factor())) as i32)
                            .to_string(),
                    )
                    .spawn()
                    .unwrap();
                while child.try_wait().unwrap().is_none() {
                    settle();
                }
                assert!(child.wait().unwrap().success());
                settle();
                settle();
            };
            this.inspect_preview_target(PreviewTarget::Typography);
            settle();
            pointer("hover");
            assert!(
                !this.inspect_label.is_visible(),
                "disabled mode has no hover layer"
            );
            pointer("click");
            assert!(
                !this.palette_module_button.is_active(),
                "disabled mode cannot navigate"
            );
            this.inspect_button.set_active(true);
            pointer("hover");
            assert_eq!(
                this.inspect_hit.borrow().as_ref().map(|hit| hit.target),
                Some(PreviewTarget::Ansi(2))
            );
            assert!(this.inspect_label.is_visible());
            assert!(
                this.inspect_label.width() > 100,
                "the hover label must be readable, not just an ellipsis"
            );
            assert_eq!(this.inspect_label.text(), "Palette · Green / Color2");
            let hint_position = this
                .inspect_label
                .compute_bounds(&this.inspect_layer)
                .unwrap();
            let hint_moves = Rc::new(Cell::new(0));
            let hint_hidden = Rc::new(Cell::new(0));
            let hint_updates = Rc::new(Cell::new(0));
            let updates = hint_updates.clone();
            let hint_notify =
                this.inspect_label
                    .connect_notify_local(Some("label"), move |_, _| {
                        updates.set(updates.get() + 1);
                    });
            let terminal_bounds = terminal.compute_bounds(&this.inspect_layer).unwrap();
            let terminal_moves = Rc::new(Cell::new(0));
            let grid_moves = terminal_moves.clone();
            let grid = terminal.clone();
            let moves = hint_moves.clone();
            let hidden = hint_hidden.clone();
            let layer = this.inspect_layer.clone();
            let hint = this.inspect_label.clone();
            let monitor = this.inspect_layer.add_tick_callback(move |_, _| {
                if !hint.is_visible() {
                    hidden.set(hidden.get() + 1);
                }
                if hint.compute_bounds(&layer).unwrap() != hint_position {
                    moves.set(moves.get() + 1);
                }
                if grid.compute_bounds(&layer).unwrap() != terminal_bounds {
                    grid_moves.set(grid_moves.get() + 1);
                }
                glib::ControlFlow::Continue
            });
            pointer("jitter");
            monitor.remove();
            this.inspect_label.disconnect(hint_notify);
            assert_eq!(hint_hidden.get(), 0, "steady hover must never flicker off");
            assert_eq!(hint_moves.get(), 0, "hint must not chase pointer jitter");
            assert_eq!(
                hint_updates.get(),
                0,
                "same target must not relayout hint text"
            );
            assert_eq!(
                terminal_moves.get(),
                0,
                "hover must not move the terminal grid"
            );
            assert_eq!(
                snapshot(),
                before,
                "hover must not redraw terminal contents"
            );

            // Long/short target names and a miss all share one stable anchor;
            // none of them participates in the terminal's size request.
            let hovered = this.inspect_hit.borrow().clone().unwrap();
            let minimum = this.inspect_layer.measure(gtk::Orientation::Horizontal, -1);
            for target in [
                Some(PreviewTarget::Prompt),
                Some(PreviewTarget::Cursor),
                None,
            ] {
                this.show_inspection_feedback(target.map(|target| PreviewHit {
                    target,
                    bounds: hovered.bounds,
                }));
                settle();
                let bounds = this
                    .inspect_label
                    .compute_bounds(&this.inspect_layer)
                    .unwrap();
                assert_eq!(bounds.x(), hint_position.x());
                assert_eq!(bounds.y(), hint_position.y());
                assert!(bounds.x() >= 0.0 && bounds.y() >= 0.0);
                assert!(bounds.x() + bounds.width() <= this.inspect_layer.width() as f32);
                assert!(bounds.y() + bounds.height() <= this.inspect_layer.height() as f32);
                assert_eq!(
                    this.inspect_layer.measure(gtk::Orientation::Horizontal, -1),
                    minimum
                );
                assert_eq!(
                    terminal.compute_bounds(&this.inspect_layer).unwrap(),
                    terminal_bounds
                );
            }
            assert!(
                !this.palette_module_button.is_active(),
                "hover alone never navigates"
            );
            pointer("blank");
            assert!(this.inspect_hit.borrow().is_none());
            assert_eq!(this.inspect_label.text(), "No editable detail");
            assert!(
                !this.palette_module_button.is_active(),
                "blank space must not select Background"
            );
            pointer("click");
            assert!(
                this.palette_module_button.is_active(),
                "actual click must switch the editor"
            );
            this.inspect_preview_target(PreviewTarget::Typography);
            settle();
            pointer("double");
            assert!(
                !this.palette_module_button.is_active(),
                "double-click must not switch the editor"
            );
            assert!(
                terminal.has_selection(),
                "double-click must retain native word selection"
            );
            terminal.unselect_all();
            pointer("drag");
            assert!(
                !this.palette_module_button.is_active(),
                "drag must not switch the editor"
            );
            assert!(
                terminal.has_selection(),
                "drag must retain native selection"
            );
            terminal.unselect_all();
            assert_eq!(snapshot(), before);
            this.inspect_button.set_active(false);
            assert!(!this.inspect_label.is_visible());
            assert!(this.inspect_hit.borrow().is_none());
            // Turning off before the next frame must discard a queued hover.
            this.inspect_button.set_active(true);
            this.schedule_inspection_hover(0.0, 0.0);
            this.inspect_button.set_active(false);
            settle();
            assert!(!this.inspect_label.is_visible());
            assert!(!this.inspect_hover_pending.get());
            pointer("click");
            assert!(
                !this.palette_module_button.is_active(),
                "turning Inspect off restores normal clicks"
            );
            this.inspect_button.set_active(true);
            pointer("click_escape");
            assert!(!this.inspect_button.is_active(), "Escape exits Inspect");
            assert!(!this.inspect_label.is_visible());
            assert!(
                !this.palette_module_button.is_active(),
                "Escape cancels a delayed click instead of navigating later"
            );
            assert_eq!(
                snapshot(),
                before,
                "exiting Inspect must not reset the scene"
            );
        }
        this.inspect_preview_target(PreviewTarget::Ansi(2));
        settle();
        assert_eq!(*this.selected_color_key.borrow(), "Color2");
        assert_eq!(snapshot(), before, "navigation must preserve the scene");
        assert_eq!(this.model.borrow().dirty, dirty);

        this.prompt_module_button.set_active(true);
        this.prompt_source_selector.set_selected(1);
        this.prompt_preview_selector.set_selected(
            PromptPreviewScenario::ALL
                .iter()
                .position(|s| *s == PromptPreviewScenario::Projects)
                .unwrap() as u32,
        );
        settle();
        let before = snapshot();
        let mut found_segment = None;
        let top = preview_visible_origin(terminal).unwrap().0;
        for row in top..top + terminal.row_count() {
            for col in 0..terminal.column_count() {
                if let Some(PreviewTarget::PromptSegment(kind)) = at(col, row) {
                    found_segment = Some(kind);
                    break;
                }
            }
            if found_segment.is_some() {
                break;
            }
        }
        let kind = found_segment.expect("designer prompt must expose a semantic module");
        this.inspect_preview_target(PreviewTarget::Typography);
        settle();
        assert_eq!(
            snapshot(),
            before,
            "leaving Prompt through inspection preserves its scene"
        );
        this.inspect_preview_target(PreviewTarget::PromptSegment(kind));
        settle();
        assert_eq!(this.selected_prompt_kind.get(), Some(kind));
        assert_eq!(snapshot(), before);

        this.preview_selector
            .set_selected((PreviewScenario::ALL.len() - 1) as u32);
        this.preview_input.borrow_mut().commit("local input");
        this.redraw_preview_contents();
        settle();
        let before = snapshot();
        let (col, row) = terminal.cursor_position();
        assert_eq!(at(col, row), Some(PreviewTarget::Cursor));
        this.inspect_preview_target(PreviewTarget::Prompt);
        settle();
        assert_eq!(this.prompt_source_selector.selected(), 0);
        assert!(this.output_bar.root.is_visible());
        assert_eq!(
            this.window()
                .lookup_action("save-starship")
                .unwrap()
                .is_enabled(),
            this.starship_editor.draft.borrow().is_some() && !this.starship_editor.invalid()
        );
        assert_eq!(this.preview_input.borrow().text(), "local input");
        assert_eq!(
            snapshot(),
            before,
            "opening prompt controls through Inspect preserves input"
        );
        assert_eq!(
            at(terminal.column_count() - 1, row),
            None,
            "empty canvas has no implicit color target"
        );
        this.inspect_preview_target(PreviewTarget::Cursor);
        settle();
        assert_eq!(snapshot(), before);

        // Real VTE wrapping and scrollback, not a guessed character-width map.
        terminal.reset(true, true);
        this.preview_feed.borrow_mut().clear();
        *this.preview_map.borrow_mut() = PreviewMap::default();
        this.feed_preview(PREVIEW_HOME_AND_CLEAR);
        for line in 0..40 {
            this.feed_scoped_preview(
                &format!("你好 e\u{301} 🌸 (≧◡≦) {line} {}\r\n", "中文".repeat(50)),
                Some(PreviewTarget::Prompt),
            );
        }
        this.feed_preview(b"\r\n");
        settle();
        let adjustment = terminal.vadjustment().unwrap();
        adjustment.set_value((adjustment.upper() - adjustment.page_size()).max(0.0));
        settle();
        let row = preview_visible_origin(terminal).unwrap().0;
        assert_eq!(at(0, row), Some(PreviewTarget::Prompt));
        assert_eq!(
            at(1, row),
            Some(PreviewTarget::Prompt),
            "both halves of a wide character must resolve"
        );
        if std::env::var_os("TERMIMOCHI_COPY_TEST").is_some() {
            let fixture = tempfile::tempdir().unwrap();
            let original = fixture.path().join("original.toml");
            let marker = fixture.path().join("must-not-run");
            let source = format!(
                "# Keep my layout\npalette = 'cute'\nformat = '''\n[╭─](bold pink)$directory$rust\n[╰─](bold pink)$character\n'''\n[palettes.cute]\npink = '#f275a0'\nred = '#eb6f92'\n[rust]\nsymbol = ' rs ' # my symbol\nstyle = 'bold red'\nformat = '[$symbol$version]($style)'\n[custom.marker]\ncommand = \"touch {}\"\n",
                marker.display()
            );
            std::fs::write(&original, &source).unwrap();
            {
                let mut context = this.current_preview_context.borrow_mut();
                let imported = &mut context.as_mut().unwrap().imported_prompt;
                imported.path = original.clone();
                imported.source = Some(source.clone());
            }
            // Emulate the first successful configuration load in this window.
            this.starship_editor.draft.borrow_mut().take();
            this.prompt_module_button.set_active(true);
            this.prompt_source_selector.set_selected(0);
            this.sync_prompt_page();
            assert_eq!(
                this.prompt_source_selector.selected(),
                0,
                "Your Starship opens directly, without creating a copy"
            );
            this.starship_editor.scenario.set_selected(1);
            let wait_for_copy = || {
                let deadline = std::time::Instant::now() + Duration::from_secs(8);
                // Let the edit debounce expire before waiting on its worker.
                settle();
                while this.copy_loading.get() {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "copy worker must finish"
                    );
                    settle();
                }
                // VTE consumes the queued replay after the worker publishes
                // its result; a retained transcript can span many frames.
                settle();
                assert!(
                    matches!(this.copy_preview.borrow().as_ref(), Some(Ok(_))),
                    "{:?}",
                    this.copy_preview.borrow()
                );
            };
            wait_for_copy();
            let editor = &this.starship_editor;
            assert!(editor.root.is_visible());
            assert_eq!(editor.symbol.text(), " rs ");
            assert!(snapshot().contains(" rs "));
            assert!(!editor.dirty(), "opening a copy must not modify it");
            let retained_base: Vec<_> = this
                .prompt_preview_base
                .borrow()
                .as_ref()
                .unwrap()
                .iter()
                .map(|chunk| chunk.text.clone())
                .collect();
            let initial_sample = this.copy_initial_sample.borrow().clone();
            assert_eq!(initial_sample.as_ref().unwrap().0.3, 1);
            editor.symbols[1].emit_clicked();
            wait_for_copy();
            assert_eq!(
                *this.copy_initial_sample.borrow(),
                initial_sample,
                "the original starting sample is shared and frozen across edits"
            );
            assert!(snapshot().contains(" 🦀 "));
            let edited = snapshot();
            let generation = this.copy_generation.get();
            let frame_count = this.copy_scene_history.borrow().len();
            let document = editor
                .draft
                .borrow()
                .as_ref()
                .unwrap()
                .contents()
                .to_owned();
            this.preview_input.borrow_mut().commit("comparison scratch");
            this.redraw_preview_contents();
            settle();
            let with_input = snapshot();
            let position = terminal.vadjustment().unwrap().value();
            this.prompt_compare_selector.set_selected(1);
            settle();
            let original_view = snapshot();
            assert!(original_view.contains(" rs "));
            assert!(
                !this
                    .copy_scene
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .original_ansi
                    .as_ref()
                    .unwrap()
                    .contains(" 🦀 ")
            );
            assert!(original_view.contains("~/projects/rust-app"));
            assert!(original_view.contains("v1.89.0") && edited.contains("v1.89.0"));
            assert!(original_view.contains("comparison scratch"));
            assert_eq!(terminal.vadjustment().unwrap().value(), position);
            assert_eq!(
                this.copy_generation.get(),
                generation,
                "comparison must not start a renderer"
            );
            assert_eq!(this.copy_scene_history.borrow().len(), frame_count);
            assert_eq!(editor.draft.borrow().as_ref().unwrap().contents(), document);
            this.prompt_compare_selector.set_selected(0);
            settle();
            let compared = snapshot();
            // VTE's absolute ring origin can advance across a reset. Compare
            // retained text, not blank rows preceding that origin.
            assert_eq!(
                compared.trim_start_matches('\n'),
                with_input.trim_start_matches('\n'),
                "A/B must replay the exact same scene"
            );
            for action in [
                "show-palette",
                "show-typography",
                "show-layout",
                "show-prompt",
            ] {
                gio::prelude::ActionGroupExt::activate_action(&this.window(), action, None);
                settle();
                assert_eq!(snapshot(), compared);
                assert_eq!(this.preview_input.borrow().text(), "comparison scratch");
                assert_eq!(this.copy_generation.get(), generation);
            }
            assert_eq!(
                this.prompt_preview_base
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|chunk| chunk.text.clone())
                    .collect::<Vec<_>>(),
                retained_base
            );
            this.preview_input.borrow_mut().reset();
            this.redraw_preview_contents();
            assert!(this.undo_action.is_enabled());
            this.prompt_source_selector.set_selected(1);
            assert!(!editor.root.is_visible());
            this.prompt_source_selector.set_selected(0);
            assert_eq!(
                editor.symbol.text(),
                " 🦀 ",
                "returning to Your Starship keeps edits"
            );
            this.undo_edit();
            assert_eq!(
                editor.symbol.text(),
                " rs ",
                "one undo restores the whole preset edit"
            );
            this.redo_edit();
            assert_eq!(editor.symbol.text(), " 🦀 ");
            editor.symbol.grab_focus();
            editor.symbol.set_text(" crab ");
            editor.symbol.set_text(" crabby ");
            this.undo_edit();
            assert_eq!(editor.symbol.text(), " 🦀 ", "typing is one undo group");
            editor.style.set_text("not-a-color");
            assert!(editor.invalid());
            assert!(!this.save_action.is_enabled());
            assert!(
                !this
                    .window()
                    .lookup_action("export-starship")
                    .unwrap()
                    .is_enabled()
            );
            assert!(
                this.output_bar.more_button().is_sensitive(),
                "recovery menu remains accessible"
            );
            this.undo_edit();
            assert!(!editor.invalid());
            assert!(this.output_bar.more_button().is_sensitive());
            editor.color.set_rgba(&gdk::RGBA::parse("#123456").unwrap());
            editor.bold.set_active(false);
            editor.version.set_selected(2);
            editor.layout.set_selected(1);
            wait_for_copy();
            let contents = editor
                .draft
                .borrow()
                .as_ref()
                .unwrap()
                .contents()
                .to_owned();
            let mut expected: toml::Table = toml::from_str(&source).unwrap();
            let mut edited: toml::Table = toml::from_str(&contents).unwrap();
            assert_eq!(edited["rust"]["style"].as_str(), Some("fg:#123456"));
            assert_eq!(
                edited["rust"]["version_format"].as_str(),
                Some("v${major}.${minor}")
            );
            expected.remove("rust");
            edited.remove("rust");
            assert_eq!(expected, edited);
            assert!(snapshot().contains('╭') && snapshot().contains('╰'));
            assert!(
                !marker.exists(),
                "custom commands must not run in the copy renderer"
            );
            assert_eq!(std::fs::read_to_string(&original).unwrap(), source);
            this.inspect_preview_target(PreviewTarget::PromptCopy);
            assert_eq!(this.prompt_source_selector.selected(), 0);
            // A stale renderer must not replace the most recent edit.
            editor.symbols[0].emit_clicked();
            this.start_copy_preview();
            editor.symbols[1].emit_clicked();
            wait_for_copy();
            assert!(snapshot().contains(" 🦀 "));
            if let Some(path) = std::env::var_os("TERMIMOCHI_COPY_SCREENSHOT") {
                window.set_title(Some("TermiMochi point-to-edit test"));
                let mut child = std::process::Command::new("python3")
                    .arg(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../scripts/preview-pointer-driver.py"
                    ))
                    .args(["capture", "0", "0"])
                    .env("TERMIMOCHI_INSPECT_SCREENSHOT", path)
                    .spawn()
                    .unwrap();
                while child.try_wait().unwrap().is_none() {
                    settle();
                }
                assert!(child.wait().unwrap().success());
            }
            // Test compatibility with an actual missing glyph, not the font's
            // name or the generic five-glyph Nerd Font scene score.
            editor.symbol.set_text(" \u{10fffd} ");
            wait_for_copy();
            settle();
            let findings = this.reported_issues.borrow().clone().unwrap();
            assert!(
                findings
                    .iter()
                    .any(|issue| issue.code == "rust-symbol-missing"
                        && issue.message.contains("U+10FFFD"))
            );
            let row = this.diagnostics.first_child();
            this.refresh_all_diagnostics();
            assert_eq!(
                this.diagnostics.first_child(),
                row,
                "unchanged findings must not rebuild or jiggle the report"
            );
            let before_font_navigation = snapshot();
            gio::prelude::ActionGroupExt::activate_action(&this.window(), "diagnostic-fonts", None);
            settle();
            assert_eq!(
                snapshot(),
                before_font_navigation,
                "Fonts action preserves the current prompt"
            );
            assert_eq!(this.prompt_source_selector.selected(), 0);
            gio::prelude::ActionGroupExt::activate_action(
                &this.window(),
                "diagnostic-module",
                Some(&"rust".to_variant()),
            );
            assert!(editor.root.is_visible());
            assert_eq!(
                editor.symbol.text(),
                " \u{10fffd} ",
                "diagnostic action must not silently replace the symbol"
            );
            if let Some(path) = std::env::var_os("TERMIMOCHI_DIAGNOSTICS_SCREENSHOT") {
                window.set_title(Some("TermiMochi point-to-edit test"));
                let mut child = std::process::Command::new("python3")
                    .arg(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../scripts/preview-pointer-driver.py"
                    ))
                    .args(["capture", "0", "0"])
                    .env("TERMIMOCHI_INSPECT_SCREENSHOT", path)
                    .spawn()
                    .unwrap();
                while child.try_wait().unwrap().is_none() {
                    settle();
                }
                assert!(child.wait().unwrap().success());
            }
            editor.symbols[0].emit_clicked();
            wait_for_copy();
            settle();
            assert!(
                !this
                    .reported_issues
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(is_rust_issue),
                "plain rs clears the obsolete glyph warning"
            );
            editor.symbols[1].emit_clicked();
            wait_for_copy();
            settle();
            assert!(
                !this
                    .reported_issues
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(is_rust_issue),
                "available emoji fallback is not an incompatibility"
            );
            editor.symbols[2].emit_clicked();
            wait_for_copy();
            settle();
            let description = this.typography_settings().font_description();
            let primary = terminal.pango_context().load_font(&description).unwrap();
            assert_eq!(
                this.reported_issues
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(is_rust_issue),
                !primary.has_char('\u{e7a8}')
            );
            editor.enabled.set_active(false);
            wait_for_copy();
            settle();
            assert!(
                !this
                    .reported_issues
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(is_rust_issue),
                "a disabled module must not be reported, including old simulated frames"
            );
            assert!(
                this.copy_scene
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .scene
                    .title
                    .contains("disabled in draft")
            );
            editor.reset.emit_clicked();
            assert_eq!(editor.draft.borrow().as_ref().unwrap().contents(), source);
            assert!(!editor.dirty());

            // A common editor, not a collection of Rust-only special cases.
            let multi_source = format!(
                "{}\n[python]\nsymbol = 'py '\nformat = '[$symbol$version]($style)'\n",
                source.replace("$directory$rust", "$directory$rust$python")
            );
            editor
                .begin(original.clone(), multi_source.clone())
                .unwrap();
            wait_for_copy();
            let generation = this.copy_generation.get();
            let original_prompt = this.copy_preview.borrow().clone();
            for (index, spec) in crate::starship_modules::MODULES.iter().enumerate() {
                editor.module.set_selected(index as u32);
                assert_eq!(editor.draft.borrow().as_ref().unwrap().spec().id, spec.id);
                assert_eq!(
                    editor.symbol.parent().unwrap().is_visible(),
                    !spec.symbols.is_empty()
                );
                assert_eq!(editor.version.parent().unwrap().is_visible(), spec.version);
            }
            assert!(!editor.dirty(), "module navigation must not edit the copy");
            assert!(
                this.copy_generation.get() > generation,
                "navigation must refresh the selected scene"
            );
            wait_for_copy();
            assert_eq!(
                *this.copy_preview.borrow(),
                original_prompt,
                "module navigation preserves the full prompt"
            );
            assert!(snapshot().contains("date +%T"));
            assert!(snapshot().contains("14:32:08"));
            editor.select_module("rust");
            editor.symbols[1].emit_clicked();
            editor.select_module("python");
            editor.scenario.set_selected(3);
            editor.symbol.set_text("python ");
            wait_for_copy();
            assert!(snapshot().contains("python "));
            editor.select_module("rust");
            this.undo_edit();
            assert_eq!(editor.draft.borrow().as_ref().unwrap().spec().id, "python");
            assert_eq!(editor.symbol.text(), "py ");
            this.redo_edit();
            assert_eq!(editor.symbol.text(), "python ");
            editor.style.set_text("invalid-color");
            editor.select_module("git_branch");
            assert_eq!(
                editor.draft.borrow().as_ref().unwrap().spec().id,
                "python",
                "don't lose invalid input on navigation"
            );
            assert!(editor.invalid());
            assert!(!this.save_action.is_enabled());
            this.undo_edit();
            editor.symbol.set_text(" \u{10fffd} ");
            wait_for_copy();
            settle();
            let finding = this
                .reported_issues
                .borrow()
                .as_ref()
                .unwrap()
                .iter()
                .find(|issue| issue_module(issue).is_some_and(|m| m.id == "python"))
                .cloned()
                .expect("Python missing-glyph finding");
            editor.select_module("rust");
            let target = format!("python\n{}", finding.message).to_variant();
            gio::prelude::ActionGroupExt::activate_action(
                &this.window(),
                "diagnostic-module",
                Some(&target),
            );
            assert_eq!(editor.draft.borrow().as_ref().unwrap().spec().id, "python");
            assert_eq!(editor.symbol.text(), " \u{10fffd} ");
            editor.symbols[0].emit_clicked();
            wait_for_copy();
            settle();
            assert!(
                !this
                    .reported_issues
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(|issue| issue_module(issue).is_some_and(|m| m.id == "python"))
            );
            editor.reset.emit_clicked();
            let doc: toml::Table =
                toml::from_str(editor.draft.borrow().as_ref().unwrap().contents()).unwrap();
            assert_eq!(
                doc["rust"]["symbol"].as_str(),
                Some(" 🦀 "),
                "reset Python keeps Rust edits"
            );
            editor.select_module("git_status");
            editor.symbol_field.set_selected(2);
            editor.symbol.set_text("+${count}");
            assert_eq!(
                editor
                    .draft
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .field("staged")
                    .as_deref(),
                Some("+${count}")
            );
            wait_for_copy();
            assert!(snapshot().contains("git add README.md src/"));
            assert!(
                snapshot().contains("+2"),
                "an omitted module appears in the prompt after the simulated command"
            );
            assert!(!snapshot().contains("Git Status · Staged"));
            let latest_prompt = this
                .copy_scene
                .borrow()
                .as_ref()
                .unwrap()
                .ansi
                .clone()
                .unwrap();
            assert!(latest_prompt.contains("+2"));
            assert!(latest_prompt.contains("TermiMochi"));
            let unchanged_prompt = this.copy_preview.borrow().clone();
            editor.symbol_field.set_selected(1);
            editor.symbol.set_text("U${count}");
            wait_for_copy();
            assert!(snapshot().contains("touch notes.txt todo.txt"));
            assert!(snapshot().contains("U2"));
            let history = this.copy_scene_history.borrow();
            let last = history.len() - 1;
            assert_eq!(history[last - 1].scene.command, "git add README.md src/");
            assert_eq!(history[last].scene.command, "touch notes.txt todo.txt");
            assert!(history[last].ansi.as_ref().unwrap().contains("U2"));
            let frames_before = history.len();
            drop(history);
            // Selection appends a command; typing updates that command's prompt.
            editor.symbol.set_text("new${count}");
            wait_for_copy();
            assert_eq!(
                this.copy_scene_history.borrow().len(),
                frames_before,
                "typing must not flood the terminal with one command per character"
            );
            assert!(
                this.copy_scene
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .ansi
                    .as_ref()
                    .unwrap()
                    .contains("new2")
            );
            assert_eq!(*this.copy_preview.borrow(), unchanged_prompt);
            editor.symbol_field.set_selected(2);
            if let Some(path) = std::env::var_os("TERMIMOCHI_MODULES_SCREENSHOT") {
                wait_for_copy();
                window.set_title(Some("TermiMochi point-to-edit test"));
                let mut child = std::process::Command::new("python3")
                    .arg(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../scripts/preview-pointer-driver.py"
                    ))
                    .args(["capture", "0", "0"])
                    .env("TERMIMOCHI_INSPECT_SCREENSHOT", path)
                    .spawn()
                    .unwrap();
                while child.try_wait().unwrap().is_none() {
                    settle();
                }
                assert!(child.wait().unwrap().success());
            }
            editor.select_module("username");
            editor.style_field.set_selected(1);
            editor.color.set_rgba(&gdk::RGBA::parse("#334455").unwrap());
            assert_eq!(
                editor
                    .draft
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .field("style_root")
                    .as_deref(),
                Some("bold fg:#334455")
            );
            assert!(
                editor
                    .draft
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .field("style")
                    .is_none()
            );
            editor.select_module("character");
            editor.symbol_field.set_selected(1);
            editor.symbol.set_text("[\u{10fffd}](red)");
            editor.select_module("git_status");
            editor.open_finding("character", "Prompt Symbol · Missing glyph (U+10FFFD)");
            assert_eq!(
                editor.symbol_field.selected(),
                1,
                "repair opens the affected field, not always the first symbol"
            );
            assert_eq!(editor.symbol.text(), "[\u{10fffd}](red)");
            wait_for_copy();
            assert_eq!(
                this.copy_scene.borrow().as_ref().unwrap().scene.command,
                "false"
            );
            editor.symbol.set_text("[FAIL](pink)");
            wait_for_copy();
            assert!(
                snapshot().contains("FAIL"),
                "the error symbol must be visible even when the real prompt is successful"
            );
            editor.select_module("git_status");
            this.start_copy_preview();
            editor.select_module("python");
            editor.select_module("username");
            wait_for_copy();
            assert!(
                this.copy_scene
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .scene
                    .title
                    .starts_with("User ·"),
                "a stale worker cannot bring back a previously selected module"
            );
            assert_eq!(std::fs::read_to_string(&original).unwrap(), source);
            for id in ["rust", "python", "git_status", "username", "character"] {
                editor.select_module(id);
                editor.reset.emit_clicked();
            }
            assert_eq!(
                editor.draft.borrow().as_ref().unwrap().contents(),
                multi_source
            );
            assert!(!editor.dirty());
            // Exercise actual GTK confirmation buttons and on-disk writes in
            // a disposable directory only, never the user's configuration.
            fn find_button(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
                if let Some(button) = widget.downcast_ref::<gtk::Button>()
                    && button.label().is_some_and(|text| text == label)
                {
                    return Some(button.clone());
                }
                let mut child = widget.first_child();
                while let Some(widget) = child {
                    if let Some(button) = find_button(&widget, label) {
                        return Some(button);
                    }
                    child = widget.next_sibling();
                }
                None
            }
            let respond = |label: &str| {
                settle();
                let button = gtk::Window::list_toplevels()
                    .into_iter()
                    .find_map(|widget| find_button(&widget, label))
                    .unwrap_or_else(|| panic!("dialog button missing: {label}"));
                button.emit_clicked();
                settle();
            };
            editor.begin(original.clone(), source.clone()).unwrap();
            editor.symbol.set_text("saved-rust ");
            let saved_contents = editor
                .draft
                .borrow()
                .as_ref()
                .unwrap()
                .contents()
                .to_owned();
            let theme_before = this.model.borrow().palette.clone();
            this.window()
                .lookup_action("save-starship")
                .unwrap()
                .activate(None);
            respond("Cancel");
            assert_eq!(std::fs::read_to_string(&original).unwrap(), source);
            assert!(editor.dirty());
            assert!(
                editor
                    .file
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .latest_backup()
                    .is_err()
            );
            this.window()
                .lookup_action("save-starship")
                .unwrap()
                .activate(None);
            respond("Back Up and Save");
            assert_eq!(std::fs::read_to_string(&original).unwrap(), saved_contents);
            assert!(!editor.dirty());
            assert_eq!(
                editor
                    .file
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .latest_backup()
                    .unwrap()
                    .contents,
                source
            );
            assert_eq!(this.model.borrow().palette, theme_before);
            this.undo_edit();
            assert_eq!(editor.symbol.text(), " rs ");
            assert!(editor.dirty());
            assert_eq!(
                std::fs::read_to_string(&original).unwrap(),
                saved_contents,
                "Undo edits the draft, not disk"
            );
            this.redo_edit();
            assert!(!editor.dirty());
            this.request_starship_restore();
            respond("Cancel");
            assert_eq!(std::fs::read_to_string(&original).unwrap(), saved_contents);
            this.request_starship_restore();
            respond("Back Up and Restore");
            assert_eq!(std::fs::read_to_string(&original).unwrap(), source);
            assert_eq!(editor.symbol.text(), " rs ");
            assert!(!editor.dirty());
            assert_eq!(
                editor
                    .file
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .latest_backup()
                    .unwrap()
                    .contents,
                saved_contents
            );
            this.undo_edit();
            assert_eq!(editor.symbol.text(), "saved-rust ");
            this.request_starship_reload();
            respond("Cancel");
            assert_eq!(editor.symbol.text(), "saved-rust ");
            this.request_starship_reload();
            respond("Reload");
            assert_eq!(editor.symbol.text(), " rs ");
            assert!(!editor.dirty());
            editor.symbol.set_text("pending ");
            this.request_starship_save();
            let external = format!("{source}\n# edited outside TermiMochi\n");
            std::fs::write(&original, &external).unwrap();
            respond("Back Up and Save");
            respond("Close");
            assert_eq!(std::fs::read_to_string(&original).unwrap(), external);
            assert!(editor.dirty());
            assert_eq!(editor.symbol.text(), "pending ");
            this.request_starship_reload();
            respond("Reload");
            assert_eq!(editor.draft.borrow().as_ref().unwrap().contents(), external);
            assert!(!editor.dirty());
        }
        window.destroy();
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ContextualState {
        content: i32,
        context: i32,
    }

    impl HistorySnapshot for ContextualState {
        type Context = i32;

        fn context(&self) -> Self::Context {
            self.context
        }

        fn set_context(&mut self, context: Self::Context) {
            self.context = context;
        }
    }

    fn snapshot(
        palette: &PtyxisPalette,
        active_variant: Variant,
        selected_color_key: &str,
    ) -> EditorSnapshot {
        let mut palette = palette.clone();
        palette.set_source(None);
        EditorSnapshot {
            palette,
            active_variant,
            selected_color_key: selected_color_key.to_owned(),
        }
    }

    fn restore_model(model: &mut Model, snapshot: EditorSnapshot) -> String {
        let EditorSnapshot {
            mut palette,
            active_variant,
            selected_color_key,
        } = snapshot;
        palette.set_source(model.current_path.clone());
        model.palette = palette;
        model.active_variant = active_variant;
        model.refresh_dirty();
        selected_color_key
    }

    fn palette_color(palette: &PtyxisPalette, variant: Variant, key: &str) -> Rgb {
        palette.variant(variant).unwrap().get(key).unwrap()
    }

    #[test]
    fn palette_names_get_safe_file_names() {
        assert_eq!(
            palette_slug("雾纸白 · Fog Paper（Codex 专用）"),
            "fog-paper-codex.palette"
        );
        assert_eq!(palette_slug("麻薯"), "termimochi.palette");
        assert_eq!(palette_slug("  Soft / Blue  "), "soft-blue.palette");
    }

    #[test]
    fn palette_selection_has_compact_names() {
        assert_eq!(color_display_name("Foreground"), "Text · Foreground");
        assert_eq!(color_display_name("Background"), "Background");
        assert_eq!(color_display_name("Color0"), "Color0 · Black");
        assert_eq!(color_display_name("Color15"), "Color15 · Bright White");
    }

    #[test]
    fn editor_modules_have_stable_navigation_metadata() {
        assert_eq!(EditorModule::ALL.len(), 5);
        assert_eq!(EditorModule::Palette.stack_name(), "palette");
        assert_eq!(EditorModule::Typography.stack_name(), "typography");
        assert_eq!(EditorModule::Layout.stack_name(), "layout");
        assert_eq!(EditorModule::Prompt.stack_name(), "prompt");
        assert_eq!(EditorModule::Greeting.stack_name(), "greeting");
        assert_eq!(EditorModule::Greeting.shortcut(), "Control+5");
        assert_ne!(
            EditorModule::Palette.stack_name(),
            EditorModule::Typography.stack_name()
        );
        assert_ne!(
            EditorModule::Palette.action_name(),
            EditorModule::Typography.action_name()
        );
        assert_eq!(
            EditorModule::Palette.icon_name(),
            "preferences-color-symbolic"
        );
        assert_eq!(
            EditorModule::Typography.icon_name(),
            "font-x-generic-symbolic"
        );
        assert_eq!(
            EditorModule::Layout.icon_name(),
            "termimochi-layout-symbolic"
        );
        assert_eq!(
            EditorModule::Prompt.icon_name(),
            "termimochi-prompt-symbolic"
        );
        assert_eq!(EditorModule::Palette.shortcut(), "Control+1");
        assert_eq!(EditorModule::Typography.shortcut(), "Control+2");
        assert_eq!(EditorModule::Layout.shortcut(), "Control+3");
        assert_eq!(EditorModule::Prompt.shortcut(), "Control+4");
    }

    #[test]
    fn edit_history_undoes_and_redoes_a_complete_transaction() {
        let mut history = EditHistory::new(100);
        history.begin(10);
        history.mark_changed();
        assert!(history.commit(40));

        assert_eq!(history.undo(40), Some(10));
        assert_eq!(history.redo(10), Some(40));
    }

    #[test]
    fn prompt_presets_have_an_independent_undo_history() {
        let developer = PromptSettings::default();
        let essential = PromptSettings::from_preset(PromptPreset::Essential);
        let mut history = EditHistory::new(100);
        history.begin(developer.clone());
        history.mark_changed();
        assert!(history.commit(essential.clone()));

        assert_eq!(history.undo(essential), Some(developer.clone()));
        assert_eq!(
            history.redo(developer),
            Some(PromptSettings::from_preset(PromptPreset::Essential))
        );
    }

    #[test]
    fn prompt_export_savepoint_tracks_edits_reexport_and_undo() {
        let initial_export = PromptSettings::default();
        let mut current = initial_export.clone();
        assert!(!has_unexported_prompt_changes(&current, &initial_export));

        assert!(current.set_character(PromptCharacter::Arrow));
        assert!(has_unexported_prompt_changes(&current, &initial_export));

        let reexported = current.clone();
        assert!(!has_unexported_prompt_changes(&current, &reexported));
        assert!(current.set_tone(PromptSegmentKind::Directory, PreviewTone::Green));
        assert!(has_unexported_prompt_changes(&current, &reexported));

        current = reexported.clone();
        assert!(!has_unexported_prompt_changes(&current, &reexported));
    }

    #[test]
    fn live_prompt_contexts_cover_every_conditional_module() {
        let contexts = prompt_preview_contexts();
        for kind in [
            PromptSegmentKind::GitBranch,
            PromptSegmentKind::GitStatus,
            PromptSegmentKind::Rust,
            PromptSegmentKind::NodeJs,
            PromptSegmentKind::Python,
            PromptSegmentKind::Go,
            PromptSegmentKind::CommandDuration,
            PromptSegmentKind::ExitStatus,
            PromptSegmentKind::Jobs,
            PromptSegmentKind::Time,
        ] {
            let mut settings = PromptSettings::from_preset(PromptPreset::Blank);
            assert!(settings.add_module(kind));
            assert!(
                contexts.iter().any(|context| settings
                    .preview_parts(context)
                    .iter()
                    .any(|part| part.kind == kind)),
                "{kind:?} is absent from every live preview context"
            );
        }
    }

    #[test]
    fn prompt_structure_and_appearance_edits_remain_atomic_history_steps() {
        let developer = PromptSettings::default();
        let mut with_jobs = developer.clone();
        assert!(with_jobs.add_module(PromptSegmentKind::Jobs));
        let mut reordered = with_jobs.clone();
        assert!(reordered.move_module_up(PromptSegmentKind::Jobs));
        let mut recolored = reordered.clone();
        assert!(recolored.set_tone(PromptSegmentKind::Jobs, PreviewTone::Magenta));

        let mut history = EditHistory::new(100);
        for (before, after) in [
            (developer.clone(), with_jobs.clone()),
            (with_jobs.clone(), reordered.clone()),
            (reordered.clone(), recolored.clone()),
        ] {
            history.begin(before);
            history.mark_changed();
            assert!(history.commit(after));
        }

        assert_eq!(history.undo(recolored), Some(reordered.clone()));
        assert_eq!(history.undo(reordered), Some(with_jobs.clone()));
        assert_eq!(history.undo(with_jobs.clone()), Some(developer.clone()));
        assert_eq!(history.redo(developer), Some(with_jobs));
    }

    #[test]
    fn redo_restores_the_context_where_the_edit_finished() {
        let before = ContextualState {
            content: 1,
            context: 10,
        };
        let after = ContextualState {
            content: 2,
            context: 20,
        };
        let mut history = EditHistory::new(10);
        history.begin(before.clone());
        history.mark_changed();
        history.commit(after.clone());

        let mut viewed_elsewhere = after;
        viewed_elsewhere.context = 99;
        assert_eq!(history.undo(viewed_elsewhere), Some(before.clone()));
        assert_eq!(
            history.redo(before),
            Some(ContextualState {
                content: 2,
                context: 20,
            })
        );
    }

    #[test]
    fn edit_history_coalesces_continuous_updates() {
        let mut history = EditHistory::new(100);
        history.begin(0);
        // A drag or a focused text edit can update the live document many
        // times; only its final value is committed.
        history.mark_changed();
        history.mark_changed();
        history.mark_changed();
        assert!(history.commit(3));

        assert_eq!(history.undo(3), Some(0));
        assert_eq!(history.undo(0), None);
    }

    #[test]
    fn invalid_draft_undo_skips_valid_intermediate_values() {
        let mut history = EditHistory::new(100);
        history.begin(48);
        // Typing 999 temporarily changes the live model to 9 and then 99;
        // the final 999 is invalid and remains only in the Entry draft.
        history.mark_changed();
        history.mark_changed();

        assert_eq!(history.undo_pending(99), Some(48));
        assert!(!history.can_undo());
        assert!(history.can_redo());
        assert_eq!(history.redo(48), Some(99));
    }

    #[test]
    fn invalid_color_draft_undo_restores_a_real_palette_snapshot() {
        let palette = PtyxisPalette::from_text(SAMPLE_PALETTE).unwrap();
        let before = snapshot(&palette, Variant::Light, "Foreground");
        let original = palette_color(&palette, Variant::Light, "Foreground");
        let changed = Rgb::new(9, 99, 199);
        let mut live_palette = palette.clone();
        live_palette
            .variant_mut(Variant::Light)
            .unwrap()
            .set("Foreground", changed)
            .unwrap();
        let live = snapshot(&live_palette, Variant::Light, "Foreground");

        let mut history = EditHistory::new(100);
        history.begin(before.clone());
        // This represents valid intermediate RGB input before the active Entry
        // becomes an invalid draft such as 999. The invalid text never enters
        // the palette, but the complete focused edit remains one transaction.
        history.mark_changed();

        let restored = history.undo_pending(live.clone()).unwrap();
        assert_eq!(
            palette_color(&restored.palette, Variant::Light, "Foreground"),
            original
        );
        assert_eq!(history.redo(restored).unwrap(), live);
    }

    #[test]
    fn invalid_only_draft_does_not_undo_earlier_history() {
        let mut history = EditHistory::new(100);
        history.begin(10);
        history.mark_changed();
        assert!(history.commit(20));

        // An immediately-invalid replacement never changes the live model.
        history.begin(20);
        assert_eq!(history.undo_pending(20), None);
        assert_eq!(history.undo(20), Some(10));
    }

    #[test]
    fn invalid_draft_returning_to_its_start_preserves_redo_branch() {
        let mut history = EditHistory::new(100);
        history.begin(10);
        history.mark_changed();
        assert!(history.commit(20));
        assert_eq!(history.undo(20), Some(10));

        history.begin(10);
        history.mark_changed();
        assert_eq!(history.undo_pending(10), None);
        assert!(history.can_redo());
        assert_eq!(history.redo(10), Some(20));
    }

    #[test]
    fn no_op_transaction_preserves_redo_branch() {
        let mut history = EditHistory::new(100);
        history.begin(0);
        history.mark_changed();
        history.commit(1);
        assert_eq!(history.undo(1), Some(0));

        history.begin(0);
        history.mark_changed();
        assert!(!history.commit(0));
        assert!(history.can_redo());
        assert_eq!(history.redo(0), Some(1));
    }

    #[test]
    fn new_edit_after_undo_clears_redo_branch() {
        let mut history = EditHistory::new(100);
        history.begin(0);
        history.mark_changed();
        history.commit(1);
        assert_eq!(history.undo(1), Some(0));

        history.begin(0);
        history.mark_changed();
        history.commit(2);
        assert!(!history.can_redo());
        assert_eq!(history.undo(2), Some(0));
    }

    #[test]
    fn edit_history_discards_oldest_entries_at_its_limit() {
        let mut history = EditHistory::new(2);
        for value in 0..3 {
            history.begin(value);
            history.mark_changed();
            history.commit(value + 1);
        }

        assert_eq!(history.undo(3), Some(2));
        assert_eq!(history.undo(2), Some(1));
        assert_eq!(history.undo(1), None);
    }

    #[test]
    fn editor_history_keeps_light_and_dark_values_isolated_and_restores_context() {
        let palette = PtyxisPalette::from_text(SAMPLE_PALETTE).unwrap();
        let original_light = palette_color(&palette, Variant::Light, "Foreground");
        let original_dark = palette_color(&palette, Variant::Dark, "Foreground");
        let changed_light = Rgb::new(12, 34, 56);
        assert_ne!(changed_light, original_light);

        let before = snapshot(&palette, Variant::Light, "Foreground");
        let mut edited_palette = palette.clone();
        edited_palette
            .variant_mut(Variant::Light)
            .unwrap()
            .set("Foreground", changed_light)
            .unwrap();
        let after = snapshot(&edited_palette, Variant::Light, "Foreground");

        let mut history = EditHistory::new(100);
        history.begin(before.clone());
        history.mark_changed();
        assert!(history.commit(after));

        // Merely viewing another variant is not an edit. Undo and redo return
        // to the variant and swatch where the color transaction occurred.
        let viewed_dark = snapshot(&edited_palette, Variant::Dark, "Color3");
        let undone = history.undo(viewed_dark).unwrap();
        assert_eq!(undone.active_variant, Variant::Light);
        assert_eq!(undone.selected_color_key, "Foreground");
        assert_eq!(
            palette_color(&undone.palette, Variant::Light, "Foreground"),
            original_light
        );
        assert_eq!(
            palette_color(&undone.palette, Variant::Dark, "Foreground"),
            original_dark
        );

        let redone = history.redo(undone).unwrap();
        assert_eq!(redone.active_variant, Variant::Light);
        assert_eq!(redone.selected_color_key, "Foreground");
        assert_eq!(
            palette_color(&redone.palette, Variant::Light, "Foreground"),
            changed_light
        );
        assert_eq!(
            palette_color(&redone.palette, Variant::Dark, "Foreground"),
            original_dark
        );
    }

    #[test]
    fn dirty_state_tracks_saved_document_contents() {
        let palette = PtyxisPalette::from_text(SAMPLE_PALETTE).unwrap();
        let mut model = Model::new(palette, Variant::Light, None, "test".to_owned());
        assert!(!model.dirty);

        model.palette.set_name("Changed").unwrap();
        model.refresh_dirty();
        assert!(model.dirty);

        model.mark_saved();
        assert!(!model.dirty);
    }

    #[test]
    fn undo_and_redo_track_the_exact_savepoint_without_changing_the_document_path() {
        let palette = PtyxisPalette::from_text(SAMPLE_PALETTE).unwrap();
        let path = PathBuf::from("/virtual/termimochi-test.palette");
        let mut model = Model::new(
            palette,
            Variant::Light,
            Some(path.clone()),
            "test".to_owned(),
        );
        model.palette.set_source(Some(path.clone()));
        let before = snapshot(&model.palette, Variant::Light, "Foreground");
        let changed_color = Rgb::new(3, 45, 67);

        let mut history = EditHistory::new(100);
        history.begin(before.clone());
        model
            .palette
            .variant_mut(Variant::Light)
            .unwrap()
            .set("Foreground", changed_color)
            .unwrap();
        history.mark_changed();
        let edited = snapshot(&model.palette, Variant::Light, "Foreground");
        assert!(history.commit(edited.clone()));
        model.refresh_dirty();
        assert!(model.dirty);

        let selected = restore_model(&mut model, history.undo(edited).unwrap());
        assert_eq!(selected, "Foreground");
        assert!(!model.dirty, "undoing to the original savepoint is clean");
        assert_eq!(model.current_path.as_deref(), Some(path.as_path()));
        assert_eq!(model.palette.source(), Some(path.as_path()));

        let clean = snapshot(&model.palette, model.active_variant, &selected);
        let selected = restore_model(&mut model, history.redo(clean).unwrap());
        assert!(model.dirty, "redoing away from the savepoint is dirty");
        assert_eq!(
            palette_color(&model.palette, Variant::Light, "Foreground"),
            changed_color
        );

        model.mark_saved();
        assert!(!model.dirty);
        let saved = snapshot(&model.palette, model.active_variant, &selected);
        restore_model(&mut model, history.undo(saved).unwrap());
        assert!(
            model.dirty,
            "undoing after Save differs from the new savepoint"
        );
        assert_eq!(model.current_path.as_deref(), Some(path.as_path()));
        assert_eq!(model.palette.source(), Some(path.as_path()));
    }

    #[test]
    fn save_button_distinguishes_snapshots_changes_and_invalid_input() {
        assert_eq!(save_button_state(false, true, true), (false, false));
        assert_eq!(save_button_state(false, false, true), (true, false));
        assert_eq!(save_button_state(true, true, true), (true, true));
        assert_eq!(save_button_state(true, false, true), (true, true));
        assert_eq!(save_button_state(true, true, false), (false, false));
        assert_eq!(save_button_state(false, false, false), (false, false));
    }

    #[test]
    fn fitting_preview_width_preserves_configured_grid_and_handles_missing_metrics() {
        assert_eq!(fitted_preview_columns(109, 600, 10, 6, true), 58);
        assert_eq!(fitted_preview_columns(109, 2000, 10, 6, true), 109);
        assert_eq!(fitted_preview_columns(109, 600, 10, 6, false), 109);
        assert_eq!(fitted_preview_columns(109, 0, 10, 6, true), 109);
        assert_eq!(fitted_preview_columns(109, 600, 0, 6, true), 109);
        assert_eq!(fitted_preview_columns(109, 4, 10, 6, true), 12);
    }

    #[test]
    fn terminal_grid_extent_preserves_cells_without_overflowing_gtk_sizes() {
        assert_eq!(terminal_grid_extent(8, 58, 2, 1), 466);
        assert_eq!(terminal_grid_extent(0, 58, 2, 195), 195);
        assert_eq!(terminal_grid_extent(-8, 58, -2, 195), 195);
        assert_eq!(
            terminal_grid_extent(std::os::raw::c_long::MAX, usize::MAX, i64::MAX, 1),
            i32::MAX
        );
        assert_eq!(add_widget_padding(466, 5), 476);
        assert_eq!(add_widget_padding(0, -5), 1);
        assert_eq!(add_widget_padding(i32::MAX, i32::MAX), i32::MAX);
    }

    #[test]
    fn editor_modules_have_unique_navigation_metadata() {
        use std::collections::HashSet;

        assert_eq!(EditorModule::ALL.len(), 5);
        for values in [
            EditorModule::ALL.map(EditorModule::stack_name),
            EditorModule::ALL.map(EditorModule::action_name),
            EditorModule::ALL.map(EditorModule::shortcut),
        ] {
            assert_eq!(values.into_iter().collect::<HashSet<_>>().len(), 5);
        }
        assert_eq!(EditorModule::Prompt.label(), "Prompt");
        assert_eq!(
            EditorModule::Prompt.icon_name(),
            "termimochi-prompt-symbolic"
        );
        assert_eq!(EditorModule::Prompt.shortcut_hint(), "Ctrl+4");
    }

    #[test]
    fn starship_export_accepts_only_toml_destinations() {
        assert!(is_starship_toml_path(Path::new("starship.toml")));
        assert!(is_starship_toml_path(Path::new("Starship.TOML")));
        assert!(is_starship_toml_path(Path::new("profiles/cute.toml")));
        assert!(!is_starship_toml_path(Path::new(".bashrc")));
        assert!(!is_starship_toml_path(Path::new(".zshrc")));
        assert!(!is_starship_toml_path(Path::new("config.fish")));
        assert!(!is_starship_toml_path(Path::new("starship")));
    }

    #[test]
    fn font_choice_prefers_exact_then_resolved_then_system_family() {
        let names = vec![
            "DejaVu Sans Mono".to_owned(),
            "JetBrains Mono".to_owned(),
            "Ubuntu Sans Mono".to_owned(),
        ];
        assert_eq!(
            preferred_font_index(
                &names,
                "JetBrains Mono",
                Some("DejaVu Sans Mono"),
                Some("Ubuntu Sans Mono")
            ),
            1
        );
        assert_eq!(
            preferred_font_index(
                &names,
                "Missing Mono",
                Some("DejaVu Sans Mono"),
                Some("Ubuntu Sans Mono")
            ),
            0
        );
        assert_eq!(
            preferred_font_index(&names, "Missing Mono", None, Some("Ubuntu Sans Mono")),
            2
        );
        assert_eq!(
            preferred_font_index(&names, "Missing Mono", None, Some("Missing System")),
            0
        );
    }
}
