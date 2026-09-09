//! Portable data-only workspace. No local destination, profile ID or executable
//! action is serialized; imported Starship contents always reopen detached.
use crate::{
    document_store::Document, layout::LayoutSettings, prompt::PromptSettings,
    starship_draft::StarshipDraft, typography::TypographySettings,
};
use serde::{Deserialize, Serialize};
use termimochi_core::{PtyxisPalette, Variant};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Workspace {
    kind: String,
    version: u8,
    pub palette: String,
    pub light: bool,
    pub typography: TypographySettings,
    pub layout: LayoutSettings,
    pub designer: PromptSettings,
    pub starship: Option<String>,
    pub use_designer: bool,
    #[serde(default)]
    pub greeting: crate::greeting::GreetingSettings,
}

impl Workspace {
    pub fn new(
        palette: &PtyxisPalette,
        variant: Variant,
        typography: TypographySettings,
        layout: LayoutSettings,
        designer: PromptSettings,
        starship: Option<String>,
        use_designer: bool,
    ) -> Self {
        Self {
            kind: "termimochi-workspace".into(),
            version: 1,
            palette: palette.to_palette_string(),
            light: variant == Variant::Light,
            typography,
            layout,
            designer,
            starship,
            use_designer,
            greeting: Default::default(),
        }
    }
    pub fn variant(&self) -> Variant {
        if self.light {
            Variant::Light
        } else {
            Variant::Dark
        }
    }
    pub fn palette(&self) -> Result<PtyxisPalette, String> {
        PtyxisPalette::from_text(&self.palette).map_err(|error| error.to_string())
    }
}

impl Document for Workspace {
    const SUFFIX: &'static str = ".termimochi.json";
    const MAX_BYTES: u64 = crate::greeting_image::source::DOCUMENT_LIMIT;
    fn validate(&self) -> Result<(), String> {
        if self.kind != "termimochi-workspace" || self.version != 1 {
            return Err("Unsupported TermiMochi workspace.".into());
        }
        if self.palette()?.variant(self.variant()).is_none() {
            return Err("The workspace's selected palette variant is missing.".into());
        }
        self.typography.validate()?;
        self.layout.validate()?;
        self.designer.validate()?;
        self.greeting.validate()?;
        if let Some(source) = &self.starship {
            StarshipDraft::new("workspace-starship.toml".into(), source.clone())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_store::{DocumentStore, decode, encode};
    #[test]
    fn workspace_roundtrip_preserves_all_modules_and_literal_starship_contents() {
        let palette =
            PtyxisPalette::from_text(include_str!("../resources/themes/fog-paper.palette"))
                .unwrap();
        let source = "# keep comments\nformat='$directory$rust$character'\n[rust]\nsymbol=' rs '\n[custom.safe]\ncommand='must-not-run'\n";
        let mut workspace = Workspace::new(
            &palette,
            Variant::Light,
            TypographySettings::default(),
            LayoutSettings::default(),
            PromptSettings::default(),
            Some(source.into()),
            false,
        );
        workspace.layout.columns = 97;
        workspace.typography.size = 13.5;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("portable.termimochi.json");
        DocumentStore::open(path.clone())
            .unwrap()
            .save(&workspace)
            .unwrap();
        let reopened = DocumentStore::<Workspace>::open(path)
            .unwrap()
            .document()
            .unwrap()
            .unwrap();
        assert_eq!(workspace, reopened);
        assert_eq!(reopened.starship.as_deref(), Some(source));
        assert!(!root.path().join("must-not-run").exists());
    }
    #[test]
    fn hostile_numeric_settings_fail_before_any_document_write() {
        let palette =
            PtyxisPalette::from_text(include_str!("../resources/themes/fog-paper.palette"))
                .unwrap();
        let workspace = Workspace::new(
            &palette,
            Variant::Light,
            TypographySettings::default(),
            LayoutSettings::default(),
            PromptSettings::default(),
            None,
            true,
        );
        let root = tempfile::tempdir().unwrap();
        let saved = root.path().join("saved.termimochi.json");
        DocumentStore::open(saved.clone())
            .unwrap()
            .save(&workspace)
            .unwrap();
        let before = std::fs::read(&saved).unwrap();
        let reference = serde_json::to_value(&workspace).unwrap();
        let imported = root.path().join("invalid.termimochi.json");
        for pointer in [
            "/layout/columns",
            "/layout/rows",
            "/layout/content_padding",
            "/layout/window_spacing",
            "/typography/size",
            "/typography/line_height",
            "/typography/cell_width",
            "/greeting/gap",
            "/greeting/accent",
            "/greeting/preview_columns",
        ] {
            for invalid in [
                serde_json::json!(-1),
                serde_json::json!(u64::MAX),
                serde_json::json!(1e100),
                serde_json::Value::Null,
                serde_json::json!("NaN"),
                serde_json::json!([]),
                serde_json::json!({}),
            ] {
                let mut value = reference.clone();
                *value.pointer_mut(pointer).unwrap() = invalid;
                let bytes = serde_json::to_vec(&value).unwrap();
                assert!(decode::<Workspace>(&bytes).is_err(), "accepted {pointer}");
                std::fs::write(&imported, &bytes).unwrap();
                assert!(DocumentStore::<Workspace>::open(imported.clone()).is_err());
                assert_eq!(std::fs::read(&imported).unwrap(), bytes);
                assert_eq!(std::fs::read(&saved).unwrap(), before);
            }
        }
        assert!(!root.path().join("termimochi-backups").exists());
    }

    #[test]
    fn whole_document_validation_rejects_invalid_nested_values_and_versions() {
        let palette =
            PtyxisPalette::from_text(include_str!("../resources/themes/fog-paper.palette"))
                .unwrap();
        let workspace = Workspace::new(
            &palette,
            Variant::Light,
            TypographySettings::default(),
            LayoutSettings::default(),
            PromptSettings::default(),
            None,
            true,
        );
        let valid = String::from_utf8(encode(&workspace).unwrap()).unwrap();
        for invalid in [
            valid.replace("\"version\": 1", "\"version\": 99"),
            valid.replace("10.5", "-1.0"),
            valid.replace("\"starship\": null", "\"starship\": \"[bad\""),
        ] {
            assert_ne!(invalid, valid);
            assert!(decode::<Workspace>(invalid.as_bytes()).is_err());
        }
        let mut invalid = workspace;
        invalid.layout.rows = 0;
        assert!(encode(&invalid).is_err());
    }

    #[test]
    fn workspace_rejects_duplicate_modules_unknown_actions_and_nested_fields() {
        let palette =
            PtyxisPalette::from_text(include_str!("../resources/themes/fog-paper.palette"))
                .unwrap();
        let workspace = Workspace::new(
            &palette,
            Variant::Light,
            TypographySettings::default(),
            LayoutSettings::default(),
            PromptSettings::default(),
            None,
            true,
        );
        let valid = serde_json::to_value(&workspace).unwrap();
        let mut duplicate = valid.clone();
        duplicate["designer"]["segments"][1] = duplicate["designer"]["segments"][0].clone();
        let mut missing = valid.clone();
        missing["designer"]["segments"]
            .as_array_mut()
            .unwrap()
            .pop();
        let mut destination = valid.clone();
        destination["starship_path"] = serde_json::json!("/tmp/foreign.toml");
        let mut action = valid.clone();
        action["apply"] = serde_json::json!(true);
        let mut nested = valid;
        nested["layout"]["command"] = serde_json::json!("must not run");
        for invalid in [duplicate, missing, destination, action, nested] {
            assert!(decode::<Workspace>(&serde_json::to_vec(&invalid).unwrap()).is_err());
        }
    }

    #[test]
    fn old_workspaces_default_to_disabled_greeting_and_new_ones_preserve_it() {
        let palette =
            PtyxisPalette::from_text(include_str!("../resources/themes/fog-paper.palette"))
                .unwrap();
        let mut workspace = Workspace::new(
            &palette,
            Variant::Light,
            TypographySettings::default(),
            LayoutSettings::default(),
            PromptSettings::default(),
            None,
            true,
        );
        let mut old = serde_json::to_value(&workspace).unwrap();
        old.as_object_mut().unwrap().remove("greeting");
        let loaded = decode::<Workspace>(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert!(!loaded.greeting.enabled);
        workspace.greeting.enabled = true;
        workspace.greeting.message = "Welcome 你好 🦀".into();
        workspace.greeting.items.reverse();
        assert_eq!(
            decode::<Workspace>(&encode(&workspace).unwrap()).unwrap(),
            workspace
        );
        workspace.greeting.custom_logo = "\x1b[2J".into();
        assert!(encode(&workspace).is_err());
    }
}
