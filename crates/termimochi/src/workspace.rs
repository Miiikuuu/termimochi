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
}
