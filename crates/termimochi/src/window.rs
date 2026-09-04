use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
    rc::Rc,
};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use termimochi_core::{
    ExportFormat, Issue, PtyxisPalette, Rgb, Severity, Target, Variant, contrast_ratio,
    export_palette, lint_palette, paths_refer_to_same_file, write_atomically,
};
use vte::prelude::*;

use crate::{
    color_picker::{ColorPicker, ColorSwatch},
    preview::{PREVIEW_COLUMNS, PREVIEW_HOME_AND_CLEAR, PREVIEW_ROWS, PreviewScenario},
    ptyxis::{InstallOutcome, PtyxisInstaller, RollbackOutcome, import_current_palette},
    style::BASE_CSS,
};

const SAMPLE_PALETTE: &str = include_str!("../../../themes/fog-paper.palette");
const EDIT_HISTORY_LIMIT: usize = 64;

const BASIC_COLORS: [(&str, &str, &str); 4] = [
    ("Foreground", "Text", "Default terminal text"),
    ("Background", "Background", "Terminal canvas"),
    ("Cursor", "Cursor", "Input position"),
    ("CursorForeground", "Cursor Text", "Text beneath the cursor"),
];

const ANSI_NAMES: [&str; 16] = [
    "Black",
    "Red",
    "Green",
    "Yellow",
    "Blue",
    "Magenta",
    "Cyan",
    "White",
    "Bright Black",
    "Bright Red",
    "Bright Green",
    "Bright Yellow",
    "Bright Blue",
    "Bright Magenta",
    "Bright Cyan",
    "Bright White",
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

struct Workbench {
    window: glib::WeakRef<adw::ApplicationWindow>,
    toast_overlay: adw::ToastOverlay,
    brand_title: gtk::Label,
    save_button: adw::SplitButton,
    save_action: gio::SimpleAction,
    undo_action: gio::SimpleAction,
    redo_action: gio::SimpleAction,
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
    terminal_title: gtk::Label,
    preview_terminal: vte::Terminal,
    preview_selector: gtk::DropDown,
    body_ratio: gtk::Label,
    composer_ratio: gtk::Label,
    summary_icon: gtk::Box,
    summary_title: gtk::Label,
    diagnostic_header: gtk::Box,
    diagnostic_surface: gtk::Box,
    diagnostics: gtk::ListBox,
    ptyxis_installer: Option<PtyxisInstaller>,
    model: RefCell<Model>,
    history: RefCell<EditHistory<EditorSnapshot>>,
    name_valid: Cell<bool>,
    updating: Cell<bool>,
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
    let (palette, current_path, source_label, preferred_variant, startup_notice) = if let Some(
        path,
    ) =
        initial_path.as_deref()
    {
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
        match import_current_palette() {
            Ok(current) => {
                let (palette, preferred_variant) = current.into_parts();
                (
                    palette,
                    None,
                    "Current Terminal · Ptyxis".to_owned(),
                    preferred_variant,
                    None,
                )
            }
            Err(error) => {
                eprintln!("TermiMochi: could not read the current Ptyxis palette: {error}");
                let (palette, path, source, variant) = fallback_document();
                (
                    palette,
                    path,
                    source,
                    variant,
                    Some(format!(
                        "Could not read the current Ptyxis palette; using the default template: {error}"
                    )),
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

    // Keep the editor chrome in the warm Fog Paper family while the selected
    // palette continues to control the VTE preview itself.
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);

    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("TermiMochi")
        .icon_name(crate::APPLICATION_ID)
        .default_width(1080)
        .default_height(780)
        .width_request(760)
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

    let open_content = adw::ButtonContent::builder()
        .icon_name("document-open-symbolic")
        .label("Open Theme")
        .build();
    let open_button = gtk::Button::builder()
        .child(&open_content)
        .tooltip_text("Open a Ptyxis .palette file  Ctrl+O")
        .action_name("win.open")
        .css_classes(["tool-button", "open-button"])
        .build();
    open_button.update_property(&[
        gtk::accessible::Property::Label("Open Theme"),
        gtk::accessible::Property::Description("Choose a Ptyxis .palette file"),
    ]);
    header_bar.pack_start(&open_button);

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
    header_bar.pack_start(&history_controls);

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

    let save_menu = gio::Menu::new();
    let save_section = gio::Menu::new();
    let save_as_item = gio::MenuItem::new(Some("Save As…"), Some("win.save-as"));
    set_menu_verb_icon(&save_as_item, "document-save-as-symbolic");
    save_section.append_item(&save_as_item);
    save_menu.append_section(None, &save_section);

    let export_menu = gio::Menu::new();
    for format in ExportFormat::ALL {
        export_menu.append(
            Some(&format!("{}…", format.display_name())),
            Some(&format!("win.export-{}", format.as_str())),
        );
    }
    let export_section = gio::Menu::new();
    let export_item = gio::MenuItem::new_submenu(Some("Export Current Variant"), &export_menu);
    set_menu_verb_icon(&export_item, "document-send-symbolic");
    export_section.append_item(&export_item);
    save_menu.append_section(None, &export_section);

    let save_content = adw::ButtonContent::builder()
        .icon_name("media-floppy-symbolic")
        .label("Save")
        .build();
    let save_button = adw::SplitButton::builder()
        .child(&save_content)
        .tooltip_text("Save  Ctrl+S")
        .dropdown_tooltip("Save As and Export")
        .menu_model(&save_menu)
        .action_name("win.save")
        .css_classes(["save-split"])
        .build();
    if let Some(popover) = save_button.popover() {
        popover.add_css_class("save-popover");
    }
    save_button.update_property(&[
        gtk::accessible::Property::Label("Save Theme"),
        gtk::accessible::Property::Description(
            "Save the current theme, or open the adjacent menu for Save As and export options",
        ),
        gtk::accessible::Property::KeyShortcuts("Control+S"),
    ]);

    let deployment_menu = gio::Menu::new();
    let install_item = gio::MenuItem::new(Some("Install to Ptyxis…"), Some("win.install-ptyxis"));
    set_menu_verb_icon(&install_item, "system-software-install-symbolic");
    deployment_menu.append_item(&install_item);
    let rollback_item = gio::MenuItem::new(
        Some("Roll Back Last Installation…"),
        Some("win.rollback-ptyxis"),
    );
    set_menu_verb_icon(&rollback_item, "document-revert-symbolic");
    deployment_menu.append_item(&rollback_item);
    let more_button = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text("More Actions: Install and Roll Back")
        .menu_model(&deployment_menu)
        .css_classes(["tool-menu", "overflow-menu"])
        .build();
    more_button.update_property(&[
        gtk::accessible::Property::Label("More Actions"),
        gtk::accessible::Property::Description("Install or roll back a Ptyxis theme"),
    ]);

    // Keep output actions together while preserving their hierarchy: saving
    // is primary, file variants live in its dropdown, and deployment stays in
    // a separate low-frequency overflow menu.
    header_bar.pack_end(&more_button);
    header_bar.pack_end(&save_button);

    toolbar_view.add_top_bar(&header_bar);
    toolbar_view.set_content(Some(&toast_overlay));
    window.set_content(Some(&toolbar_view));

    let main_paned = gtk::Paned::builder()
        .orientation(gtk::Orientation::Horizontal)
        .position(430)
        .wide_handle(false)
        .build();
    main_paned.add_css_class("workbench-split");
    toast_overlay.set_child(Some(&main_paned));

    let editor = build_editor();
    let preview = build_preview(&variant_switch);
    main_paned.set_start_child(Some(&editor.root));
    main_paned.set_end_child(Some(&preview.root));
    main_paned.set_resize_start_child(false);
    main_paned.set_shrink_start_child(false);
    main_paned.set_shrink_end_child(false);

    let base_css_provider = gtk::CssProvider::new();
    base_css_provider.load_from_data(BASE_CSS);
    let terminal_css_provider = gtk::CssProvider::new();
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
    }

    let window_ref = glib::WeakRef::new();
    window_ref.set(Some(&window));
    let workbench = Rc::new(Workbench {
        window: window_ref,
        toast_overlay,
        brand_title: brand_name,
        save_button,
        save_action,
        undo_action,
        redo_action,
        install_action,
        rollback_action,
        name_entry: editor.name_entry,
        light_button,
        dark_button,
        controls: editor.controls,
        selected_color_key: RefCell::new("Foreground".to_owned()),
        selected_color_title: editor.selected_color_title,
        color_picker: editor.color_picker,
        terminal_css_provider,
        terminal_title: preview.terminal_title,
        preview_terminal: preview.terminal,
        preview_selector: preview.selector,
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
        history: RefCell::new(EditHistory::new(EDIT_HISTORY_LIMIT)),
        name_valid: Cell::new(true),
        updating: Cell::new(false),
    });

    Workbench::install_actions(&workbench);
    Workbench::connect_signals(&workbench);
    workbench.refresh_all();
    if let Some(notice) = startup_notice {
        workbench.toast(&notice);
    }
    // The window owns the controller. The controller only keeps a weak window
    // reference, so closing the window releases the complete object graph.
    unsafe {
        window.set_data("termimochi-workbench", workbench);
    }
    window.present();
}

struct PreviewWidgets {
    root: gtk::ScrolledWindow,
    terminal_title: gtk::Label,
    terminal: vte::Terminal,
    selector: gtk::DropDown,
    body_ratio: gtk::Label,
    composer_ratio: gtk::Label,
    summary_icon: gtk::Box,
    summary_title: gtk::Label,
    diagnostic_header: gtk::Box,
    diagnostic_surface: gtk::Box,
    diagnostics: gtk::ListBox,
}

struct EditorWidgets {
    root: gtk::ScrolledWindow,
    name_entry: gtk::Entry,
    controls: BTreeMap<String, ColorControl>,
    selected_color_title: gtk::Label,
    color_picker: ColorPicker,
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
    for (index, (key, title, detail)) in BASIC_COLORS.into_iter().enumerate() {
        let tile = gtk::Box::new(gtk::Orientation::Vertical, 5);
        let swatch = ColorSwatch::new(false, &format!("{key} · {detail}"));
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
        .min_content_width(380)
        .css_classes(["editor-scroll"])
        .child(&content)
        .build();
    EditorWidgets {
        root: scroll,
        name_entry,
        controls,
        selected_color_title,
        color_picker,
    }
}

fn build_preview(variant_switch: &gtk::Box) -> PreviewWidgets {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.add_css_class("termimochi-preview-pane");
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let heading = gtk::Label::new(Some("Live Preview"));
    heading.set_xalign(0.0);
    heading.set_valign(gtk::Align::Center);
    heading.set_hexpand(true);
    heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
    heading.add_css_class("preview-heading");
    variant_switch.set_valign(gtk::Align::Center);
    variant_switch.set_halign(gtk::Align::End);
    variant_switch.set_hexpand(false);
    let preview_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    preview_header.set_hexpand(true);
    preview_header.set_halign(gtk::Align::Fill);
    preview_header.set_valign(gtk::Align::Center);
    preview_header.add_css_class("preview-title-row");
    preview_header.append(&heading);
    preview_header.append(variant_switch);
    content.append(&preview_header);

    let terminal = gtk::Box::new(gtk::Orientation::Vertical, 8);
    terminal.set_widget_name("termimochi-terminal");
    terminal.add_css_class("terminal-shell");
    terminal.set_margin_bottom(4);

    let terminal_header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    terminal_header.add_css_class("terminal-header");
    let terminal_title = gtk::Label::new(Some("bash"));
    terminal_title.set_hexpand(true);
    terminal_title.set_xalign(0.0);
    terminal_title.add_css_class("terminal-chrome");
    let scenario_labels = PreviewScenario::ALL.map(PreviewScenario::label);
    let selector = gtk::DropDown::from_strings(&scenario_labels);
    selector.set_tooltip_text(Some("Switch VTE preview scenario"));
    selector.add_css_class("preview-scenario");
    terminal_header.append(&terminal_title);
    terminal_header.append(&selector);
    terminal.append(&terminal_header);

    let vte_terminal = vte::Terminal::builder()
        .audible_bell(false)
        .bold_is_bright(false)
        .cursor_blink_mode(vte::CursorBlinkMode::Off)
        .input_enabled(false)
        .scrollback_lines(0)
        .build();
    vte_terminal.set_hexpand(true);
    vte_terminal.set_vexpand(false);
    vte_terminal.set_height_request(195);
    vte_terminal.set_size(
        PREVIEW_COLUMNS
            .try_into()
            .expect("preview column count fits c_long"),
        PREVIEW_ROWS
            .try_into()
            .expect("preview row count fits c_long"),
    );
    // This is a read-only preview inside another scrolled window. Keeping VTE
    // out of pointer hit-testing lets wheel/touchpad events reach that parent.
    vte_terminal.set_can_target(false);
    vte_terminal.set_focusable(false);
    vte_terminal.set_font(Some(&gtk::pango::FontDescription::from_string(
        "Monospace 10.5",
    )));
    vte_terminal.add_css_class("vte-preview");
    terminal.append(&vte_terminal);

    content.append(&terminal);

    let quality = gtk::Box::new(gtk::Orientation::Vertical, 4);
    quality.add_css_class("quality-section");

    let metrics = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    metrics.add_css_class("quality-row");
    let quality_heading = gtk::Label::new(Some("Contrast"));
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

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        // Hide the large pane scrollbar without making the content dictate
        // the window's minimum height. Unlike `Never`, `External` keeps the
        // viewport scrollable by wheel and touchpad when the window is short.
        .vscrollbar_policy(gtk::PolicyType::External)
        .min_content_width(360)
        .css_classes(["preview-scroll"])
        .child(&content)
        .build();

    PreviewWidgets {
        root: scroll,
        terminal_title,
        terminal: vte_terminal,
        selector,
        body_ratio,
        composer_ratio,
        summary_icon,
        summary_title,
        diagnostic_header,
        diagnostic_surface,
        diagnostics,
    }
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
        let has_draft = self.has_draft();
        let history = self.history.borrow();
        self.undo_action
            .set_enabled(has_draft || history.can_undo());
        self.redo_action
            .set_enabled(!has_draft && history.can_redo());
    }

    fn undo_edit(&self) {
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

    fn redo_edit(&self) {
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

    fn install_actions(this: &Rc<Self>) {
        let window = this.window();

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
        let weak = Rc::downgrade(this);
        this.window().connect_close_request(move |_| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            this.settle_active_edit();
            if !this.model.borrow().dirty && !this.has_draft() {
                return glib::Propagation::Proceed;
            }
            this.confirm_discard(|this| {
                this.model.borrow_mut().dirty = false;
                this.discard_drafts();
                this.window().close();
            });
            glib::Propagation::Stop
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
                this.refresh_preview();
            }
        });

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
        for control in self.controls.values() {
            control.swatch.set_selected(false);
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
        self.refresh_preview();
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
            "#termimochi-terminal {{ background-color: {background}; color: {foreground}; }}"
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
        self.preview_terminal.reset(true, true);
        self.preview_terminal.set_colors(
            Some(&foreground_rgba),
            Some(&background_rgba),
            &terminal_palette_refs,
        );
        self.preview_terminal.set_color_cursor(Some(&cursor_rgba));
        self.preview_terminal
            .set_color_cursor_foreground(Some(&cursor_foreground_rgba));
        let scenario = PreviewScenario::from_index(self.preview_selector.selected());
        self.terminal_title.set_text(scenario.terminal_title());
        // VTE's reset is queued; explicitly home the cursor and clear the
        // viewport in the same feed stream before drawing the next scenario.
        self.preview_terminal.feed(PREVIEW_HOME_AND_CLEAR);
        self.preview_terminal.feed(scenario.transcript().as_bytes());

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

    fn refresh_diagnostics(&self, issues: &[Issue]) {
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

        remove_status_classes(&self.summary_icon);
        self.diagnostic_header.set_visible(issue_count != 1);
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
        } else {
            self.summary_icon.add_css_class("status-good");
            self.summary_title.set_text("All checks passed");
        }

        self.diagnostic_surface.set_visible(issue_count > 0);

        for issue in visible_issues {
            let (icon_name, class, severity_name) = match issue.severity {
                Severity::Error => ("dialog-error-symbolic", "status-error", "Error"),
                Severity::Warning => ("dialog-warning-symbolic", "status-warning", "Warning"),
                Severity::Note => ("emblem-ok-symbolic", "status-good", "Note"),
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

    fn choose_export(self: &Rc<Self>, format: ExportFormat) {
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
        self.settle_active_edit();
        self.confirm_discard(|this| this.show_open_dialog());
    }

    fn show_open_dialog(self: &Rc<Self>) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Ptyxis palette"));
        filter.add_pattern("*.palette");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title("Open Theme")
            .accept_label("Open")
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build();
        let weak = Rc::downgrade(self);
        dialog.open(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |result| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(file) => {
                        if let Some(path) = file.path() {
                            this.open_path(&path);
                        } else {
                            this.toast("Only local files can be opened");
                        }
                    }
                    Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                    Err(error) => this.toast(&format!("Could not open: {error}")),
                }
            },
        );
    }

    fn confirm_discard<F>(self: &Rc<Self>, action: F)
    where
        F: FnOnce(Rc<Self>) + 'static,
    {
        self.settle_active_edit();
        if !self.model.borrow().dirty && !self.has_draft() {
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

    fn open_path(&self, path: &Path) {
        match PtyxisPalette::from_file(path) {
            Ok(palette) => {
                // File dialogs can briefly restore focus to the previous
                // picker field before completing. End that old transaction
                // before replacing the document and clearing its history.
                self.finish_active_edit();
                let preferred_variant = preferred_ui_variant();
                let active_variant = if palette.variant(preferred_variant).is_some() {
                    preferred_variant
                } else if palette.variant(Variant::Light).is_some() {
                    Variant::Light
                } else {
                    Variant::Dark
                };
                *self.model.borrow_mut() = Model::new(
                    palette,
                    active_variant,
                    Some(path.to_owned()),
                    "Local File".to_owned(),
                );
                self.history.borrow_mut().clear();
                *self.selected_color_key.borrow_mut() = "Foreground".to_owned();
                self.refresh_all();
                self.toast("Theme opened");
            }
            Err(error) => self.toast(&format!("Invalid theme format: {error}")),
        }
    }

    fn save(self: &Rc<Self>) {
        self.settle_active_edit();
        if !self.require_valid_name() {
            return;
        }
        let path = self.model.borrow().current_path.clone();
        if let Some(path) = path {
            self.write_to(&path);
        } else {
            self.choose_save_as();
        }
    }

    fn choose_save_as(self: &Rc<Self>) {
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
    fn edit_history_undoes_and_redoes_a_complete_transaction() {
        let mut history = EditHistory::new(100);
        history.begin(10);
        history.mark_changed();
        assert!(history.commit(40));

        assert_eq!(history.undo(40), Some(10));
        assert_eq!(history.redo(10), Some(40));
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
    fn save_button_distinguishes_snapshots_changes_and_invalid_input() {
        assert_eq!(save_button_state(false, true, true), (false, false));
        assert_eq!(save_button_state(false, false, true), (true, false));
        assert_eq!(save_button_state(true, true, true), (true, true));
        assert_eq!(save_button_state(true, false, true), (true, true));
        assert_eq!(save_button_state(true, true, false), (false, false));
        assert_eq!(save_button_state(false, false, false), (false, false));
    }
}
