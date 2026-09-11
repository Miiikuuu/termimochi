//! Portable typed documents. Workspace is only a transient preview bridge:
//! reference components never enter this envelope or its output scope.
use crate::{
    document_store::{self, Document},
    greeting::{GreetingPreset, GreetingSettings},
    layout::{LayoutPreset, LayoutSettings},
    prompt::PromptSettings,
    typography::TypographySettings,
    workspace::Workspace,
};
use serde::{Deserialize, Serialize};
use termimochi_core::{PtyxisPalette, Variant};
mod strict;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    #[default]
    Palette,
    KittyAppearance,
    Prompt,
    Greeting,
    Typography,
    Layout,
    Artwork,
    Project,
    Legacy,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Palette => "Ptyxis Palette",
            Self::KittyAppearance => "Kitty Appearance",
            Self::Prompt => "Starship Prompt",
            Self::Greeting => "Fastfetch Greeting",
            Self::Typography => "Typography Preset",
            Self::Layout => "Layout Preset",
            Self::Artwork => "Artwork",
            Self::Project => "Explicit Project",
            Self::Legacy => "Legacy Workspace",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TargetHint {
    Ptyxis,
    Kitty,
}

/// Exact native bytes are provenance, never a destination or execution grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeSource {
    pub format: NativeFormat,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NativeFormat {
    Ptyxis,
    Kitty,
    Starship,
    Fastfetch,
    LegacyWorkspace,
    TypographyPreset,
    LayoutPreset,
    GreetingPreset,
}

/// Derived from present components, not separately serialized permission bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Scope {
    pub palette: bool,
    pub typography: bool,
    pub layout: bool,
    pub prompt: bool,
    pub greeting: bool,
    pub artwork: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Palette,
    Typography,
    Layout,
    Prompt,
    Greeting,
    Artwork,
}

/// Operation support is separate from component ownership. Local dependencies,
/// file snapshots and visual confirmation are deliberately not serialized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    Edit,
    Preview,
    NativeExport,
    Trial,
    Write,
    Enable,
    Open,
    Restore,
}

impl Operation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Edit => "Edit owned content",
            Self::Preview => "Design preview",
            Self::NativeExport => "Export a native file / preset",
            Self::Trial => "Real target trial",
            Self::Write => "Write reviewed output",
            Self::Enable => "Activate / create independent entry",
            Self::Open => "Open in the correct target",
            Self::Restore => "Restore a recorded application",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Capability {
    Available,
    NeedsRequirement(String),
    Unsupported(String),
}

impl Capability {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Available => "Available",
            Self::NeedsRequirement(_) => "Needs preparation / review",
            Self::Unsupported(_) => "Not implemented for this operation",
        }
    }
    pub fn detail(&self) -> &str {
        match self {
            Self::Available => {
                "Implemented for this document's owned content. No execution or write permission is implied."
            }
            Self::NeedsRequirement(detail) | Self::Unsupported(detail) => detail,
        }
    }
}

impl Scope {
    pub fn for_kind(kind: Kind) -> Self {
        match kind {
            Kind::Palette => Self {
                palette: true,
                ..Self::default()
            },
            Kind::KittyAppearance => Self {
                palette: true,
                typography: true,
                layout: true,
                ..Self::default()
            },
            Kind::Prompt => Self {
                prompt: true,
                ..Self::default()
            },
            Kind::Greeting => Self {
                greeting: true,
                ..Self::default()
            },
            Kind::Typography => Self {
                typography: true,
                ..Self::default()
            },
            Kind::Layout => Self {
                layout: true,
                ..Self::default()
            },
            Kind::Artwork => Self {
                artwork: true,
                ..Self::default()
            },
            // Project creation supplies its own explicit component selection.
            Kind::Project => Self::default(),
            Kind::Legacy => Self {
                palette: true,
                typography: true,
                layout: true,
                prompt: true,
                greeting: true,
                artwork: false,
            },
        }
    }

    pub fn allows(self, action: Action) -> bool {
        match action {
            Action::Palette => self.palette,
            Action::Typography => self.typography,
            Action::Layout => self.layout,
            Action::Prompt => self.prompt,
            Action::Greeting => self.greeting,
            Action::Artwork => self.greeting || self.artwork,
        }
    }

    pub fn require(self, action: Action) -> Result<(), String> {
        if self.allows(action) {
            Ok(())
        } else {
            Err("This is preview reference content, not part of the current document. Create an explicit project or open that document to edit or use it.".into())
        }
    }

    fn subset_of(self, other: Self) -> bool {
        (!self.palette || other.palette)
            && (!self.typography || other.typography)
            && (!self.layout || other.layout)
            && (!self.prompt || other.prompt)
            && (!self.greeting || other.greeting)
            && (!self.artwork || other.artwork)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PaletteComponent {
    pub source: String,
    pub light: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PromptComponent {
    pub designer: PromptSettings,
    pub starship: Option<String>,
    pub use_designer: bool,
}

/// A standalone artwork never owns system fields, preRun, or a Greeting hook.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArtworkComponent {
    pub logo: crate::greeting::Logo,
    pub custom_logo: String,
    pub custom_art: Option<crate::greeting_art::Artwork>,
    pub editable_artwork: Option<crate::greeting_image::source::EditableArtwork>,
    pub source_logo: Option<crate::greeting_art::LogoSnapshot>,
    pub presentation: crate::greeting_output::Presentation,
}

impl ArtworkComponent {
    fn from_greeting(greeting: &GreetingSettings) -> Self {
        Self {
            logo: greeting.logo,
            custom_logo: greeting.custom_logo.clone(),
            custom_art: greeting.custom_art.clone(),
            editable_artwork: greeting.editable_artwork.clone(),
            source_logo: greeting.source_logo.clone(),
            presentation: greeting.presentation.clone(),
        }
    }
    fn project(&self, greeting: &mut GreetingSettings) {
        greeting.logo = self.logo;
        greeting.custom_logo.clone_from(&self.custom_logo);
        greeting.custom_art.clone_from(&self.custom_art);
        greeting.editable_artwork.clone_from(&self.editable_artwork);
        greeting.source_logo.clone_from(&self.source_logo);
        greeting.presentation.clone_from(&self.presentation);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Components {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<PaletteComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typography: Option<TypographySettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<PromptComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub greeting: Option<GreetingSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork: Option<ArtworkComponent>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DesignDocument {
    schema: String,
    version: u8,
    /// Portable design identity, not a local binding or execution permission.
    pub id: String,
    pub kind: Kind,
    pub components: Components,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<NativeSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_hint: Option<TargetHint>,
}

impl DesignDocument {
    /// A project's components must share its explicit terminal family. This is
    /// distinct from the separately reviewed shared-file option of a standalone
    /// Greeting document; no protocol or target is silently changed here.
    pub fn require_greeting_target(
        &self,
        protocol: Option<crate::greeting_image::pixel_export::Protocol>,
    ) -> Result<(), String> {
        if self.kind == Kind::Project
            && self.target_hint == Some(TargetHint::Ptyxis)
            && protocol.is_some()
        {
            return Err("This is a Ptyxis project, but this Greeting requires pixel output. Create an explicit Kitty project copy to use images/animation, or explicitly choose Character output for this project. Selecting a different Greeting test terminal does not change the project's target.".into());
        }
        Ok(())
    }

    /// Owned Greeting content, or the explicit minimal display wrapper required
    /// to show a standalone artwork. The wrapper never borrows preview fields,
    /// position, commands, styles or layout from another document/environment.
    pub fn greeting_output(&self) -> Option<GreetingSettings> {
        if let Some(greeting) = &self.components.greeting {
            return Some(greeting.clone());
        }
        self.components.artwork.as_ref().and_then(|artwork| {
            let mut greeting = GreetingSettings {
                enabled: true,
                message: String::new(),
                ..GreetingSettings::default()
            };
            for item in &mut greeting.items {
                item.enabled = false;
            }
            artwork.project(&mut greeting);
            if let Some(snapshot) = greeting.source_logo.take() {
                // A standalone material has no imported Fastfetch descriptor
                // to bind this snapshot to. Materialize its already validated
                // ANSI bytes, keeping their real colors and the editable image
                // source, but no file reference or native executable modules.
                // Uncolored native text follows terminal foreground rather
                // than inheriting the Greeting designer's default cyan accent.
                greeting.accent = 16;
                if snapshot.artwork.plain.trim().is_empty() {
                    greeting.logo = crate::greeting::Logo::None;
                    greeting.custom_logo.clear();
                    greeting.custom_art = None;
                } else {
                    let editable = greeting.editable_artwork.take();
                    greeting.import_artwork(snapshot.artwork).ok()?;
                    greeting.editable_artwork = editable;
                }
            }
            Some(greeting)
        })
    }

    /// This table reports implementation, not the current machine's readiness.
    /// Invoking an operation still runs the real adapter's scope, dependency,
    /// snapshot, conflict and authorization checks at its business boundary.
    pub fn capabilities(&self) -> Vec<(Operation, Capability)> {
        use Capability::{Available, NeedsRequirement as Needs, Unsupported};
        use Operation::*;
        let mut rows = vec![(Edit, Available), (Preview, Available)];
        rows.push((NativeExport, match self.kind {
            Kind::Project | Kind::Legacy => Unsupported("There is no single native format for this combination. Convert an explicit component copy, or use the independent target entry.".into()),
            Kind::Artwork => Unsupported("There is no single native configuration for an artwork. Use the retained original/TXT/ANSI/image-animation package export actions and choose the intended format.".into()),
            _ => Available,
        }));
        if self.kind == Kind::Legacy {
            for operation in [Trial, Write, Enable, Open] {
                rows.push((operation, Unsupported("Legacy mixed targets are not executed together. Create an explicit component or target project copy; conversion shows retained and omitted content.".into())));
            }
            rows.push((Restore, Needs("Use Last Application & Recovery to select an existing local receipt. Portable legacy data grants no restore authority; external changes are checked again.".into())));
            return rows;
        }
        if self.target_hint == Some(TargetHint::Kitty) {
            rows.extend([
                (Trial, Needs("Use Design → Try in Kitty. The adapter checks Kitty/Bash and owned Starship/Fastfetch dependencies, prepares temporary files, and requires separate visual and GIF-motion confirmation. Personal shell startup is not loaded.".into())),
                (Write, Needs("Complete the real controlled trial, review exact generated files and mapping limits, then confirm publication. Only owned components enter this independent Kitty directory.".into())),
                (Enable, Needs("After successful visual review, Create / Update Independent Entry prepares a versioned local entry. It does not change the default terminal or add a daily shell hook.".into())),
                (Open, Needs("Publish an independent entry, then use Open in Kitty or Open Independent Kitty Scheme. The adapter rechecks local files and dependencies; process launch is not visual verification.".into())),
                (Restore, Needs("Choose a published entry and Restore / Deactivate Entry. Its version pointer and retained assets are checked; external changes block restoration.".into())),
            ]);
            return rows;
        }
        if self.target_hint == Some(TargetHint::Ptyxis) || self.kind == Kind::Palette {
            let scope = self.scope();
            let appearance = scope.palette || scope.typography || scope.layout;
            rows.extend([
                (Trial, Unsupported("The Ptyxis adapter does not provide an isolated complete appearance/shell trial. Design preview is not real terminal verification; Greeting has its separate temporary trial.".into())),
                (Write, Needs("Select and review the Ptyxis profile or explicit native component file. Dependencies, source snapshots and external conflicts are rechecked; only owned settings can be selected.".into())),
                (Enable, if scope.palette { Needs("Review Palette installation plus separate profile activation and global Light/Dark effects. No Prompt or Greeting hook is installed automatically.".into()) } else { Unsupported("This scope has no Palette to activate. Shared Prompt/Greeting writes do not create startup hooks; use an independent Kitty entry for a controlled session.".into()) }),
                (Open, if appearance { Needs("After a successful reviewed application, Open Profile Tab opens that exact Ptyxis profile. Existing shell startup may execute; no complete appearance trial is implied.".into()) } else { Needs("Use the result's explicit target action if available. A shared native update is not a complete session; choose an independent Kitty entry for a repeatable controlled launch.".into()) }),
                (Restore, Needs("Select this application's local recovery receipt. Successful owned changes are restored independently only if their current values still match the recorded result.".into())),
            ]);
            return rows;
        }
        rows.extend([
            (Trial, Needs("Use Design to choose an independent Kitty target and run the real controlled trial. Greeting also retains its target-specific temporary trial. No terminal dependency or visual result is assumed here.".into())),
            (Write, Needs("Use Design to select an independent Kitty entry, a supported Ptyxis appearance target, or an explicit Starship/Fastfetch file as offered for this type. Review its actual scope and conflicts before writing.".into())),
            (Enable, Needs("Choose Create Independent Kitty Entry for a repeatable controlled launch. Updating a shared native file alone does not enable shell startup or replace the default terminal.".into())),
            (Open, Needs("Create a reviewed independent entry, then use Open in Kitty; it remains available from the local entry library after restart. Existing native-file updates have a narrower, separately disclosed opening scope.".into())),
            (Restore, Needs("A successful local application/publication and its unchanged recovery record are required. Open or import alone never grants restoration authority.".into())),
        ]);
        rows
    }

    /// Current owned content only. Provenance is not substituted for a changed
    /// Prompt/Greeting; exporting never grants permission to execute it.
    /// The first tuple element is a suggested complete file name.
    pub fn native_export(&self) -> Result<(&'static str, String), String> {
        self.validate()?;
        match self.kind {
            Kind::Palette => Ok(("colors.palette", self.components.palette.as_ref().ok_or("Missing Palette component.")?.source.clone())),
            Kind::KittyAppearance => {
                if let Some(source) = &self.native {
                    return Ok(("kitty.conf", source.text.clone()));
                }
                // The adapter receives explicit ownership and reads only owned
                // fields. This scratch baseline is never saved or written.
                let reference = Workspace::new(
                    &PtyxisPalette::from_text(include_str!("../resources/themes/fog-paper.palette")).map_err(|e| e.to_string())?,
                    Variant::Light, TypographySettings::default(), LayoutSettings::default(),
                    PromptSettings::default(), None, true,
                );
                let scope = self.scope();
                let (text, notes) = crate::kitty_session::appearance_configuration(
                    &self.project_preview(&reference),
                    crate::kitty_session::Ownership { palette: scope.palette, typography: scope.typography, layout: scope.layout, prompt: false, greeting: false },
                    None,
                )?;
                let mut output = String::from("# Generated Kitty appearance. Mapping limitations:\n");
                for note in notes {
                    for line in note.lines() { output.push_str("# "); output.push_str(line); output.push('\n'); }
                }
                output.push_str(&text);
                Ok(("kitty.conf", output))
            }
            Kind::Prompt => {
                let prompt = self.components.prompt.as_ref().ok_or("Missing Prompt component.")?;
                let source = if prompt.use_designer { prompt.designer.to_starship_toml() } else {
                    prompt.starship.clone().ok_or("No current Starship source to export.")?
                };
                Ok(("starship.toml", source))
            }
            Kind::Greeting => Ok(("config.jsonc", self.components.greeting.as_ref().ok_or("Missing Greeting component.")?.fastfetch_config()?)),
            Kind::Typography => {
                let settings = self.components.typography.as_ref().ok_or("Missing Typography component.")?;
                let bytes = crate::typography_preset::encode(settings)?;
                Ok(("typography.termimochi-font.json", String::from_utf8(bytes).map_err(|e| e.to_string())?))
            }
            Kind::Layout => {
                let settings = self.components.layout.ok_or("Missing Layout component.")?;
                let bytes = document_store::encode(&LayoutPreset::new(settings))?;
                Ok(("layout.termimochi-layout.json", String::from_utf8(bytes).map_err(|e| e.to_string())?))
            }
            Kind::Artwork => Err("Choose an explicit artwork export: TXT, ANSI, original image or image/animation package. A material is not a Fastfetch configuration.".into()),
            Kind::Project | Kind::Legacy => Err("A combination has no single native file. Use the target's independent entry, or convert an explicit component copy and export it.".into()),
        }
    }

    /// A native Kitty appearance owns its lossless source, not the defaults
    /// borrowed while projecting it into the common terminal editor.
    pub fn from_kitty_source(text: String) -> Result<Self, String> {
        let document = Self {
            schema: "termimochi-design".into(),
            version: 2,
            id: gtk::glib::uuid_string_random().into(),
            kind: Kind::KittyAppearance,
            components: Components::default(),
            native: Some(NativeSource {
                format: NativeFormat::Kitty,
                text,
            }),
            target_hint: Some(TargetHint::Kitty),
        };
        document.validate()?;
        Ok(document)
    }

    pub fn from_workspace(
        kind: Kind,
        scope: Scope,
        workspace: &Workspace,
        native: Option<NativeSource>,
    ) -> Result<Self, String> {
        if kind == Kind::KittyAppearance && native.is_some() {
            return Err("Capture explicit Kitty edits through the native handler, then use from_kitty_source; preview reference defaults are not document settings.".into());
        }
        let result = Self {
            schema: "termimochi-design".into(),
            version: 2,
            id: gtk::glib::uuid_string_random().into(),
            kind,
            components: Components {
                palette: scope.palette.then(|| PaletteComponent {
                    source: workspace.palette.clone(),
                    light: workspace.light,
                }),
                typography: scope.typography.then(|| workspace.typography.clone()),
                layout: scope.layout.then_some(workspace.layout),
                prompt: scope.prompt.then(|| PromptComponent {
                    designer: workspace.designer.clone(),
                    starship: workspace.starship.clone(),
                    use_designer: workspace.use_designer,
                }),
                greeting: scope.greeting.then(|| workspace.greeting.clone()),
                artwork: scope
                    .artwork
                    .then(|| ArtworkComponent::from_greeting(&workspace.greeting)),
            },
            native,
            target_hint: match kind {
                Kind::Palette => Some(TargetHint::Ptyxis),
                Kind::KittyAppearance => Some(TargetHint::Kitty),
                _ => None,
            },
        };
        result.validate()?;
        Ok(result)
    }

    pub fn scope(&self) -> Scope {
        if self.kind == Kind::KittyAppearance
            && let Some(native) = &self.native
            && let Ok(kitty) = crate::kitty_document::KittyDocument::parse(&native.text)
        {
            let (palette, typography, layout) = kitty.ownership();
            return Scope {
                palette,
                typography,
                layout,
                ..Scope::default()
            };
        }
        Scope {
            palette: self.components.palette.is_some(),
            typography: self.components.typography.is_some(),
            layout: self.components.layout.is_some(),
            prompt: self.components.prompt.is_some(),
            greeting: self.components.greeting.is_some(),
            artwork: self.components.artwork.is_some(),
        }
    }

    /// References are cloned only into this disposable projection. Saving must
    /// use from_workspace with this document's scope, never serialize the bridge.
    pub fn project_preview(&self, references: &Workspace) -> Workspace {
        let mut workspace = references.clone();
        if self.kind == Kind::KittyAppearance
            && let Some(native) = &self.native
            && let Ok(kitty) = crate::kitty_document::KittyDocument::parse(&native.text)
            && let Ok(projected) = kitty.project_workspace(references)
        {
            workspace = projected;
        }
        if let Some(value) = &self.components.palette {
            workspace.palette.clone_from(&value.source);
            workspace.light = value.light;
        }
        if let Some(value) = &self.components.typography {
            workspace.typography = value.clone();
        }
        if let Some(value) = self.components.layout {
            workspace.layout = value;
        }
        if let Some(value) = &self.components.prompt {
            workspace.designer = value.designer.clone();
            workspace.starship.clone_from(&value.starship);
            workspace.use_designer = value.use_designer;
        }
        if let Some(value) = &self.components.greeting {
            workspace.greeting = value.clone();
        }
        if let Some(value) = &self.components.artwork {
            value.project(&mut workspace.greeting);
        }
        workspace
    }

    /// Conversion never mutates the original or inherits a local session stamp.
    /// Reports are computed from the actual ownership difference, not promises
    /// of target compatibility (the adapter must disclose that separately).
    pub fn convert_copy(
        &self,
        kind: Kind,
        scope: Scope,
        references: &Workspace,
    ) -> Result<(Self, Vec<String>), String> {
        let old = self.scope();
        let mut notes = Vec::new();
        for (label, owned, retained) in [
            ("Palette", old.palette, scope.palette),
            ("Typography", old.typography, scope.typography),
            ("Layout", old.layout, scope.layout),
            ("Prompt", old.prompt, scope.prompt),
            ("Greeting", old.greeting, scope.greeting),
            ("Artwork", old.artwork, scope.artwork || scope.greeting),
        ] {
            if owned && !retained {
                notes.push(format!(
                    "{label} is not included in the new copy; the original document is unchanged."
                ));
            }
            if !owned && retained {
                notes.push(format!(
                    "{label} is explicitly added from the current preview reference."
                ));
            }
        }
        let native = if kind == self.kind {
            self.native.clone()
        } else {
            None
        };
        if self.native.is_some() && native.is_none() {
            notes.push("The original native source remains in the original document. The converted copy owns only the selected components; target mapping is reviewed before use.".into());
            if let Some(source) = &self.native
                && source.format == NativeFormat::Kitty
            {
                let kitty = crate::kitty_document::KittyDocument::parse(&source.text)?;
                notes.extend(
                    kitty
                        .notices
                        .iter()
                        .map(|n| format!("Not transferred to the copy: {n}")),
                );
                let projected = self.project_preview(references);
                if scope.typography {
                    if let Some(size) = kitty.properties().get("font_size")
                        && size.parse::<f64>().ok() != Some(projected.typography.size)
                    {
                        notes.push(format!("Font size is approximated from native {size} to the editor's supported {} points.", projected.typography.size));
                    }
                    for key in ["bold_font", "italic_font", "bold_italic_font"] {
                        if kitty.properties().contains_key(key) {
                            notes.push(format!("Native {key} is not represented by the shared Typography preset and stays only in the original source."));
                        }
                    }
                }
                if scope.layout {
                    for key in ["initial_window_width", "initial_window_height"] {
                        if let Some(value) = kitty.properties().get(key)
                            && !value.ends_with('c')
                        {
                            notes.push(format!("Native {key}={value} uses pixels. The copy uses the current reference terminal grid; this is not an exact size conversion."));
                        }
                    }
                    for key in ["window_padding_width", "window_margin_width"] {
                        if let Some(value) = kitty.properties().get(key)
                            && value.split_whitespace().count() > 1
                        {
                            notes.push(format!("Native {key}={value} has per-edge spacing. The copy approximates it with one shared spacing value."));
                        }
                    }
                }
            }
        }
        let copy = match (kind, native) {
            (Kind::KittyAppearance, Some(native)) => {
                if scope != old {
                    return Err("A lossless native Kitty copy retains its authored fields. Extract an explicit component document instead.".into());
                }
                Self::from_kitty_source(native.text)?
            }
            (kind, native) => {
                Self::from_workspace(kind, scope, &self.project_preview(references), native)?
            }
        };
        Ok((copy, notes))
    }
}

impl Document for DesignDocument {
    const SUFFIX: &'static str = ".termimochi-design.json";
    const MAX_BYTES: u64 = crate::greeting_image::source::DOCUMENT_LIMIT * 2;
    fn migrate(&mut self, bytes: &[u8]) -> Result<(), String> {
        strict::unique_json_keys(bytes)
    }
    fn validate(&self) -> Result<(), String> {
        if self.schema != "termimochi-design" || self.version != 2 {
            return Err("Unsupported TermiMochi design schema or version.".into());
        }
        if self.id.len() != 36 || !self.id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
            return Err("Invalid portable document identity.".into());
        }
        let scope = self.scope();
        if scope == Scope::default()
            && (self.kind == Kind::Project
                || self.kind == Kind::KittyAppearance && self.native.is_none())
        {
            return Err(
                "Select at least one explicit component for this project or appearance document."
                    .into(),
            );
        }
        if self.kind == Kind::KittyAppearance
            && let Some(native) = &self.native
        {
            if self.components != Components::default() {
                return Err("Native Kitty settings must have one source owner, not duplicate preview components.".into());
            }
            crate::kitty_document::KittyDocument::parse(&native.text)?;
        }
        if self.kind != Kind::Project && !scope.subset_of(Scope::for_kind(self.kind)) {
            return Err("The document contains components outside its declared type.".into());
        }
        if scope.greeting && scope.artwork {
            return Err("Artwork cannot have two mutable owners; keep it inside Greeting.".into());
        }
        if !matches!(self.kind, Kind::Project | Kind::KittyAppearance)
            && scope != Scope::for_kind(self.kind)
        {
            return Err("The document is missing its required component.".into());
        }
        if self.kind == Kind::Palette && self.target_hint == Some(TargetHint::Kitty)
            || self.kind == Kind::KittyAppearance && self.target_hint == Some(TargetHint::Ptyxis)
        {
            return Err(
                "Native document target conflicts with its type. Convert a copy instead.".into(),
            );
        }
        if let Some(value) = &self.components.palette {
            let palette = PtyxisPalette::from_text(&value.source).map_err(|e| e.to_string())?;
            if palette
                .variant(if value.light {
                    Variant::Light
                } else {
                    Variant::Dark
                })
                .is_none()
            {
                return Err("The selected palette variant is missing.".into());
            }
        }
        if let Some(value) = &self.components.typography {
            value.validate()?;
        }
        if let Some(value) = &self.components.layout {
            value.validate()?;
        }
        if let Some(value) = &self.components.prompt {
            value.designer.validate()?;
            if let Some(source) = &value.starship {
                crate::starship_draft::StarshipDraft::new(
                    "document-starship.toml".into(),
                    source.clone(),
                )?;
            }
        }
        if let Some(value) = &self.components.greeting {
            value.validate()?;
        }
        if let Some(value) = &self.components.artwork {
            let mut greeting = GreetingSettings::default();
            value.project(&mut greeting);
            // A logo snapshot may reference the original Fastfetch descriptor;
            // standalone artwork validates the preserved snapshot independently.
            if let Some(source) = greeting.source_logo.take() {
                source.validate()?;
            }
            greeting.validate()?;
        }
        if self
            .native
            .as_ref()
            .is_some_and(|n| n.text.len() as u64 > crate::greeting_image::source::DOCUMENT_LIMIT)
        {
            return Err("Native source is too large.".into());
        }
        if let Some(native) = &self.native {
            let compatible = matches!(
                (self.kind, native.format),
                (Kind::Palette, NativeFormat::Ptyxis)
                    | (Kind::KittyAppearance, NativeFormat::Kitty)
                    | (Kind::Prompt, NativeFormat::Starship)
                    | (
                        Kind::Greeting,
                        NativeFormat::Fastfetch | NativeFormat::GreetingPreset
                    )
                    | (Kind::Typography, NativeFormat::TypographyPreset)
                    | (Kind::Layout, NativeFormat::LayoutPreset)
                    | (Kind::Legacy, NativeFormat::LegacyWorkspace)
            );
            if !compatible {
                return Err(
                    "Native provenance does not match the document type. Convert a copy instead."
                        .into(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Detected {
    Design,
    Legacy,
    TypographyPreset,
    LayoutPreset,
    GreetingPreset,
    Native(Kind),
    Ambiguous(Kind),
}

/// File names and extensions are deliberately not sufficient evidence. Schema
/// claims are authoritative but still undergo strict typed decoding on import.
pub(crate) fn detect(bytes: &[u8]) -> Result<Detected, String> {
    if bytes.len() as u64 > DesignDocument::MAX_BYTES {
        return Err("Document is too large.".into());
    }
    let source =
        std::str::from_utf8(bytes).map_err(|_| "Binary artwork must use Import Artwork.")?;
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) {
        if let Some(schema) = value.get("schema") {
            return if schema == "termimochi-design" {
                Ok(Detected::Design)
            } else {
                Err("Unknown design schema.".into())
            };
        }
        if let Some(kind) = value.get("kind").and_then(serde_json::Value::as_str) {
            return match kind {
                "termimochi-workspace" => Ok(Detected::Legacy),
                "termimochi-typography" => Ok(Detected::TypographyPreset),
                "termimochi-layout" => Ok(Detected::LayoutPreset),
                "termimochi-greeting" => Ok(Detected::GreetingPreset),
                _ => Err("Unknown design document kind.".into()),
            };
        }
    }
    if PtyxisPalette::from_text(source).is_ok() {
        return Ok(Detected::Native(Kind::Palette));
    }
    if let Ok(value) = crate::fastfetch_document::value(source) {
        if let Some(schema) = value.get("$schema").and_then(serde_json::Value::as_str) {
            if schema.contains("fastfetch") {
                return Ok(Detected::Native(Kind::Greeting));
            }
            return Err("The JSONC schema does not identify Fastfetch.".into());
        }
        if value.get("modules").is_some() || value.get("logo").is_some() {
            return Ok(Detected::Native(Kind::Greeting));
        }
        return Ok(Detected::Ambiguous(Kind::Greeting));
    }
    if let Ok(value) = source.parse::<toml::Table>() {
        if let Some(schema) = value.get("$schema").and_then(toml::Value::as_str) {
            return if schema.contains("starship") {
                Ok(Detected::Native(Kind::Prompt))
            } else {
                Err("The TOML schema does not identify Starship.".into())
            };
        }
        let keys = [
            "format",
            "right_format",
            "add_newline",
            "scan_timeout",
            "command_timeout",
            "character",
            "directory",
            "git_branch",
            "rust",
            "custom",
        ];
        if keys.iter().any(|key| value.contains_key(*key)) {
            crate::starship_draft::StarshipDraft::new("import.toml".into(), source.into())?;
            return Ok(Detected::Native(Kind::Prompt));
        }
        if !value.is_empty() {
            return Ok(Detected::Ambiguous(Kind::Prompt));
        }
    }
    // Kitty is whitespace-based, not TOML. Require an actual recognized scalar
    // directive; unknown-only files remain ambiguous instead of executing Kitty.
    if source.lines().any(|line| {
        let key = line.split_whitespace().next().unwrap_or("");
        matches!(
            key,
            "font_family"
                | "font_size"
                | "foreground"
                | "background"
                | "cursor"
                | "cursor_shape"
                | "window_padding_width"
                | "initial_window_width"
                | "initial_window_height"
        ) || key
            .strip_prefix("color")
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| n < 16)
    }) {
        return Ok(Detected::Native(Kind::KittyAppearance));
    }
    if source.lines().any(|line| {
        matches!(
            line.split_whitespace().next(),
            Some("include" | "globinclude" | "envinclude" | "geninclude")
        )
    }) {
        return Ok(Detected::Ambiguous(Kind::KittyAppearance));
    }
    Err("The content does not identify a unique supported document. Choose an explicit import type; the file was not executed or converted.".into())
}

/// Import is data-only. Native files are detached design copies, and v1 bytes
/// remain preserved verbatim rather than granting old local bindings authority.
pub(crate) fn import(bytes: &[u8], references: &Workspace) -> Result<DesignDocument, String> {
    let detected = detect(bytes)?;
    import_detected(bytes, detected, references)
}

/// Only a candidate returned by detect may be confirmed; this cannot reinterpret
/// a foreign schema, malformed file, or another unambiguous native type.
pub(crate) fn import_as(
    bytes: &[u8],
    kind: Kind,
    references: &Workspace,
) -> Result<DesignDocument, String> {
    match detect(bytes)? {
        Detected::Ambiguous(candidate) | Detected::Native(candidate) if candidate == kind => {
            import_detected(bytes, Detected::Native(kind), references)
        }
        _ => Err("The selected type conflicts with this file's content or schema.".into()),
    }
}

fn import_detected(
    bytes: &[u8],
    detected: Detected,
    references: &Workspace,
) -> Result<DesignDocument, String> {
    if detected == Detected::Design {
        return document_store::decode(bytes);
    }
    let source = std::str::from_utf8(bytes)
        .map_err(|e| e.to_string())?
        .to_owned();
    let mut workspace = references.clone();
    let (kind, format) = match detected {
        Detected::Legacy => {
            workspace = document_store::decode(bytes)?;
            (Kind::Legacy, NativeFormat::LegacyWorkspace)
        }
        Detected::TypographyPreset => {
            workspace.typography = crate::typography_preset::decode(bytes)?;
            (Kind::Typography, NativeFormat::TypographyPreset)
        }
        Detected::LayoutPreset => {
            workspace.layout = document_store::decode::<LayoutPreset>(bytes)?.layout;
            (Kind::Layout, NativeFormat::LayoutPreset)
        }
        Detected::GreetingPreset => {
            workspace.greeting = document_store::decode::<GreetingPreset>(bytes)?.greeting;
            (Kind::Greeting, NativeFormat::GreetingPreset)
        }
        Detected::Native(Kind::Palette) => {
            let palette = PtyxisPalette::from_text(&source).map_err(|e| e.to_string())?;
            workspace.light = palette.variant(Variant::Light).is_some();
            workspace.palette = source.clone();
            (Kind::Palette, NativeFormat::Ptyxis)
        }
        Detected::Native(Kind::Prompt) => {
            crate::starship_draft::StarshipDraft::new("import.toml".into(), source.clone())?;
            workspace.starship = Some(source.clone());
            workspace.use_designer = false;
            (Kind::Prompt, NativeFormat::Starship)
        }
        Detected::Native(Kind::Greeting) => {
            crate::fastfetch_document::parse(&source)?;
            workspace.greeting = GreetingSettings {
                enabled: true,
                imported_source: Some(source.clone()),
                ..GreetingSettings::default()
            };
            (Kind::Greeting, NativeFormat::Fastfetch)
        }
        Detected::Native(Kind::KittyAppearance) => {
            return DesignDocument::from_kitty_source(source);
        }
        Detected::Ambiguous(kind) => {
            return Err(format!(
                "Confirm whether to import this content as {}. It was not executed or converted.",
                kind.label()
            ));
        }
        _ => return Err("Unsupported native import.".into()),
    };
    DesignDocument::from_workspace(
        kind,
        Scope::for_kind(kind),
        &workspace,
        Some(NativeSource {
            format,
            text: source,
        }),
    )
}

#[cfg(test)]
mod tests;
