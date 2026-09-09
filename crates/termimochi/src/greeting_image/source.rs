//! Versioned, bounded editable source. Cloned history entries share image bytes.
use super::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub(crate) const DOCUMENT_LIMIT: u64 = 24 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SourceImage(Arc<Vec<u8>>);

impl std::fmt::Debug for SourceImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceImage")
            .field("bytes", &self.0.len())
            .finish()
    }
}

impl Serialize for SourceImage {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&gtk::glib::base64_encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for SourceImage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        if encoded.len() as u64 > FILE_LIMIT.div_ceil(3) * 4 {
            return Err(serde::de::Error::custom("Editable image exceeds 16 MiB."));
        }
        let bytes = gtk::glib::base64_decode(&encoded);
        if gtk::glib::base64_encode(&bytes).as_str() != encoded {
            return Err(serde::de::Error::custom("Invalid editable image encoding."));
        }
        Self::new(bytes).map_err(serde::de::Error::custom)
    }
}

impl SourceImage {
    pub fn new(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() as u64 > FILE_LIMIT || bytes.is_empty() {
            return Err("Editable image must be nonempty and at most 16 MiB.".into());
        }
        let svg = super::svg::looks_like_svg(&bytes);
        if svg && bytes.len() > super::svg::SVG_LIMIT {
            return Err("Editable SVG sources are limited to 2 MiB.".into());
        }
        if !svg
            && !matches!(
                image::guess_format(&bytes),
                Ok(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP)
            )
        {
            return Err("Editable sources support PNG, JPG, WebP and SVG only.".into());
        }
        Ok(Self(Arc::new(bytes)))
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        if !is_image(path) {
            return Err("Choose the original PNG, JPG, WebP or SVG image.".into());
        }
        Self::new(
            crate::typography_preset::read_private_with_limit(path, FILE_LIMIT)?
                .ok_or("Original image is missing.")?,
        )
    }
    pub fn decode(&self) -> Result<DecodedImage, String> {
        decode(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditableArtwork {
    version: u8,
    pub image: SourceImage,
    // JSON numbers are finite and Eq, unlike arbitrary public f64 values.
    recipe: serde_json::Value,
}

impl EditableArtwork {
    pub fn new(image: SourceImage, options: Options) -> Result<Self, String> {
        let this = Self {
            version: 1,
            image,
            recipe: serde_json::to_value(options).map_err(|e| e.to_string())?,
        };
        this.options()?;
        Ok(this)
    }
    pub fn options(&self) -> Result<Options, String> {
        if self.version != 1 {
            return Err("Unsupported editable artwork version.".into());
        }
        let options: Options =
            serde_json::from_value(self.recipe.clone()).map_err(|e| e.to_string())?;
        options.edits.validate()?;
        if !(8..=grid_limits(options.style).0).contains(&options.columns)
            || !options.cell_ratio.is_finite()
            || !(0.1..=2.0).contains(&options.cell_ratio)
        {
            return Err("Invalid editable artwork dimensions.".into());
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document_store,
        greeting::{GreetingPreset, GreetingSettings},
    };

    #[test]
    fn editable_source_round_trips_without_original_and_never_leaks_into_fastfetch() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("private-original.png");
        super::super::tests::fixture().save(&path).unwrap();
        let source = SourceImage::read(&path).unwrap();
        let clone = source.clone();
        assert!(
            Arc::ptr_eq(&source.0, &clone.0),
            "Undo must share large source bytes"
        );
        let options = Options {
            columns: 96,
            style: Style::Ascii,
            cell_ratio: 0.5,
            background: [255; 3],
            foreground: [20; 3],
            invert: true,
            edits: Adjustments {
                crop: [5, 10, 15, 20],
                exposure: 0.3,
                contrast: 1.2,
                saturation: 0.7,
                smoothing: 0.3,
                edges: 1.1,
                trim: true,
                ink: Ink::Gray,
                structure: Structure::Tone,
                removal: Removal {
                    enabled: false,
                    color: Some([254, 253, 252]),
                    tolerance: 9.0,
                    softness: 3.0,
                    connected: false,
                },
            },
        };
        let artwork = source.decode().unwrap().convert(options).unwrap().artwork;
        let mut settings = GreetingSettings::starter();
        settings.import_artwork(artwork.clone()).unwrap();
        settings.editable_artwork = Some(EditableArtwork::new(source, options).unwrap());
        let encoded = document_store::encode(&GreetingPreset::new(settings.clone())).unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains("private-original.png"));
        std::fs::remove_file(&path).unwrap();
        let restored = document_store::decode::<GreetingPreset>(&encoded)
            .unwrap()
            .greeting;
        assert_eq!(restored, settings);
        let editable = restored.editable_artwork.unwrap();
        assert_eq!(editable.options().unwrap(), options);
        assert_eq!(
            editable
                .image
                .decode()
                .unwrap()
                .convert(editable.options().unwrap())
                .unwrap()
                .artwork,
            artwork
        );
        let config = settings.fastfetch_config().unwrap();
        assert!(
            !config.contains("editable_artwork")
                && !config.contains("recipe")
                && !config.contains("iVBOR")
        );
        assert_eq!(
            crate::fastfetch_document::value(&config).unwrap()["logo"]["source"],
            artwork.ansi
        );
        settings.import_artwork(artwork).unwrap();
        assert!(
            settings.editable_artwork.is_none(),
            "unrelated replacement must detach the old source"
        );
    }

    #[test]
    fn editable_source_validation_bounds_and_legacy_documents() {
        assert!(SourceImage::new(vec![0; FILE_LIMIT as usize + 1]).is_err());
        assert!(serde_json::from_str::<SourceImage>("\"not base64!\"").is_err());
        let preset = GreetingPreset::new(GreetingSettings::starter());
        let mut value = serde_json::to_value(&preset).unwrap();
        value["greeting"]
            .as_object_mut()
            .unwrap()
            .remove("editable_artwork");
        assert!(
            document_store::decode::<GreetingPreset>(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .greeting
                .editable_artwork
                .is_none()
        );
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("image.png");
        super::super::tests::fixture().save(&path).unwrap();
        let source = SourceImage::read(&path).unwrap();
        let options = Options {
            columns: 64,
            style: Style::Detail,
            cell_ratio: 0.5,
            background: [255; 3],
            foreground: [0; 3],
            invert: false,
            edits: Adjustments::default(),
        };
        let mut editable = EditableArtwork::new(source, options).unwrap();
        editable.recipe["columns"] = serde_json::json!(u32::MAX);
        assert!(editable.options().is_err());
        editable.recipe["columns"] = serde_json::json!(64);
        editable.recipe["edits"]["removal"]["tolerance"] = serde_json::json!(1000);
        assert!(editable.options().is_err());
        editable.version = 2;
        assert!(editable.options().is_err());
    }
}
