//! Portable background intent. Image bytes are shared by history snapshots;
//! paths, local runtime availability and execution permission are never saved.
use crate::greeting_image::source::SourceImage;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub contain: bool,
    pub zoom: f64,
    pub x: f64,
    pub y: f64,
    pub tint: f64,
}
impl Default for Placement {
    fn default() -> Self {
        Self {
            contain: false,
            zoom: 1.0,
            x: 0.5,
            y: 0.5,
            tint: 0.35,
        }
    }
}
impl Placement {
    pub fn validate(self) -> Result<(), String> {
        if !self.zoom.is_finite()
            || !(0.1..=3.0).contains(&self.zoom)
            || [self.x, self.y, self.tint]
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err("Invalid background zoom, position or overlay.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackgroundSettings {
    pub image: SourceImage,
    pub placement: Placement,
}
impl BackgroundSettings {
    pub fn from_png(png: Vec<u8>, placement: Placement) -> Result<Self, String> {
        let settings = Self {
            image: SourceImage::new(png)?,
            placement,
        };
        settings.validate()?;
        Ok(settings)
    }
    pub fn validate(&self) -> Result<(), String> {
        self.placement.validate()?;
        let reader = image::ImageReader::with_format(
            std::io::Cursor::new(self.image.bytes()),
            image::ImageFormat::Png,
        );
        let (w, h) = reader
            .into_dimensions()
            .map_err(|e| format!("Invalid background PNG: {e}"))?;
        if w == 0 || h == 0 || w > 1920 || h > 1920 {
            return Err("Background must be a prepared PNG of at most 1920 × 1920.".into());
        }
        Ok(())
    }
    /// Run in the bounded output worker, never in a slider callback. Both
    /// adapters use this same framing projection. It scales with the native
    /// window; changing aspect ratio there can stretch this prepared canvas.
    pub fn canvas(&self, size: [u32; 2], color: [u8; 3]) -> Result<Vec<u8>, String> {
        self.validate()?;
        if size.contains(&0) {
            return Err("Empty background canvas.".into());
        }
        let factor = (1920.0 / f64::from(size[0].max(size[1]))).min(1.0);
        let w = (f64::from(size[0]) * factor).round().max(1.0) as u32;
        let h = (f64::from(size[1]) * factor).round().max(1.0) as u32;
        let source = image::load_from_memory(self.image.bytes())
            .map_err(|e| e.to_string())?
            .into_rgba8();
        let (x, y, iw, ih) = bounds(
            size[0].into(),
            size[1].into(),
            source.width().into(),
            source.height().into(),
            self.placement,
        );
        let sx = f64::from(w) / f64::from(size[0]);
        let sy = f64::from(h) / f64::from(size[1]);
        let (x, y, iw, ih) = (x * sx, y * sy, iw * sx, ih * sy);
        let mut canvas =
            image::RgbaImage::from_pixel(w, h, image::Rgba([color[0], color[1], color[2], 255]));
        // Sample only the output canvas: 300% zoom cannot allocate a huge
        // intermediate image. Bilinear sampling also preserves transparency.
        for (cx, cy, pixel) in canvas.enumerate_pixels_mut() {
            let u = (f64::from(cx) + 0.5 - x) / iw;
            let v = (f64::from(cy) + 0.5 - y) / ih;
            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                continue;
            }
            if let Some(sample) = image::imageops::sample_bilinear(&source, u as f32, v as f32) {
                let alpha = f64::from(sample[3]) / 255.0 * (1.0 - self.placement.tint);
                for i in 0..3 {
                    pixel[i] = (f64::from(sample[i]) * alpha + f64::from(color[i]) * (1.0 - alpha))
                        .round() as u8;
                }
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        canvas
            .write_to(&mut out, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(out.into_inner())
    }
}

pub(crate) fn bounds(w: f64, h: f64, iw: f64, ih: f64, p: Placement) -> (f64, f64, f64, f64) {
    let scale = if p.contain {
        (w / iw).min(h / ih)
    } else {
        (w / iw).max(h / ih)
    } * p.zoom;
    let (iw, ih) = (iw * scale, ih * scale);
    (axis_origin(w, iw, p.x), axis_origin(h, ih, p.y), iw, ih)
}
pub(crate) fn axis_range(view: f64, image: f64) -> (f64, f64) {
    let visible = (view.min(image) * 0.25).min(32.0);
    (visible - image, (view + image - 2.0 * visible).max(1.0))
}
pub(crate) fn axis_origin(view: f64, image: f64, position: f64) -> f64 {
    let (start, span) = axis_range(view, image);
    start + span * position
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn fixture() -> BackgroundSettings {
        let mut pixels = image::RgbaImage::from_pixel(80, 40, image::Rgba([30, 80, 150, 255]));
        for (x, _, pixel) in pixels.enumerate_pixels_mut() {
            if x >= 40 {
                *pixel = image::Rgba([170, 130, 20, 255]);
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        pixels.write_to(&mut out, image::ImageFormat::Png).unwrap();
        BackgroundSettings::from_png(
            out.into_inner(),
            Placement {
                zoom: 0.5,
                x: 0.65,
                y: 0.35,
                tint: 0.0,
                ..Default::default()
            },
        )
        .unwrap()
    }
    #[test]
    fn framing_canvas_preserves_position_zoom_transparency_and_limits() {
        let bg = fixture();
        let png = bg.canvas([800, 400], [1, 2, 3]).unwrap();
        let img = image::load_from_memory(&png).unwrap().into_rgba8();
        let (x, y, w, h) = bounds(800.0, 400.0, 80.0, 40.0, bg.placement);
        assert_eq!(
            img.get_pixel((x + w * 0.25) as u32, (y + h * 0.5) as u32).0,
            [30, 80, 150, 255]
        );
        assert_eq!(
            img.get_pixel((x + w * 0.75) as u32, (y + h * 0.5) as u32).0,
            [170, 130, 20, 255]
        );
        assert_eq!(img.get_pixel(0, 0).0, [1, 2, 3, 255]);
        let mut tinted = bg.clone();
        tinted.placement.tint = 1.0;
        assert!(
            image::load_from_memory(&tinted.canvas([40, 20], [1, 2, 3]).unwrap())
                .unwrap()
                .into_rgba8()
                .pixels()
                .all(|p| p.0 == [1, 2, 3, 255])
        );
        let bounded =
            image::load_from_memory(&bg.canvas([u32::MAX, u32::MAX], [0, 0, 0]).unwrap()).unwrap();
        assert_eq!((bounded.width(), bounded.height()), (1920, 1920));
        assert!(bg.canvas([0, 0], [0, 0, 0]).is_err());
    }
    #[test]
    fn theme_roundtrip_owns_background_without_leaking_into_references_or_copies() {
        use crate::{
            design_document::{DesignDocument, Kind, Scope, TargetHint},
            document_store::{decode, encode},
        };
        let mut design = DesignDocument::new_theme(TargetHint::Kitty, "Background", false);
        let bg = fixture();
        design.theme.as_mut().unwrap().background = Some(bg.clone());
        let bytes = encode(&design).unwrap();
        let restored: DesignDocument = decode(&bytes).unwrap();
        assert_eq!(restored, design);
        let palette = termimochi_core::PtyxisPalette::from_text(include_str!(
            "../resources/themes/fog-paper.palette"
        ))
        .unwrap();
        let mut reference = crate::workspace::Workspace::new(
            &palette,
            termimochi_core::Variant::Light,
            Default::default(),
            Default::default(),
            Default::default(),
            None,
            false,
        );
        reference.background = Some(bg.clone());
        let empty = DesignDocument::new_theme(TargetHint::Ptyxis, "Empty", true);
        assert!(empty.project_preview(&reference).background.is_none());
        assert_eq!(design.project_preview(&reference).background, Some(bg));
        let (copy, notes) = design
            .convert_copy(
                Kind::Typography,
                Scope::for_kind(Kind::Typography),
                &reference,
            )
            .unwrap();
        assert!(copy.project_preview(&reference).background.is_none());
        assert!(
            notes
                .iter()
                .any(|n| n.contains("Background image is not included"))
        );
        let converted = design
            .convert_theme_copy(TargetHint::Ptyxis, &reference)
            .unwrap();
        assert_eq!(
            converted.theme.unwrap().background,
            design.theme.unwrap().background
        );
    }
    #[test]
    fn invalid_background_fields_and_legacy_absence_are_checked() {
        let bg = fixture();
        let mut invalid = bg.clone();
        for p in [
            Placement {
                zoom: f64::NAN,
                ..bg.placement
            },
            Placement {
                x: 1.1,
                ..bg.placement
            },
            Placement {
                y: -0.1,
                ..bg.placement
            },
            Placement {
                tint: f64::INFINITY,
                ..bg.placement
            },
        ] {
            invalid.placement = p;
            assert!(invalid.validate().is_err());
        }
        use crate::design_document::{DesignDocument, TargetHint};
        let old = DesignDocument::new_theme(TargetHint::Kitty, "Old", true);
        let mut json = serde_json::to_value(&old).unwrap();
        json["theme"].as_object_mut().unwrap().remove("background");
        let restored: DesignDocument =
            crate::document_store::decode(&serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(restored.theme.unwrap().background.is_none());
    }
}
