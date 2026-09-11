//! Portable intent is separate from local destinations and visual approval.
use crate::{
    greeting::GreetingSettings,
    greeting_image::{self, pixel_export::Protocol},
    pixel_trial::Terminal,
};
use serde::{Deserialize, Serialize};

pub(crate) mod background;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Visual {
    Auto,
    Image,
    Animation,
    #[default]
    Character,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Fallback {
    #[default]
    Ask,
    PortableCharacter,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Presentation {
    pub visual: Visual,
    pub columns: u32,
    pub character_style: greeting_image::Style,
    pub protocol: Option<Protocol>,
    pub fallback: Fallback,
}
impl Default for Presentation {
    fn default() -> Self {
        Self {
            visual: Visual::Character,
            columns: 32,
            character_style: greeting_image::Style::Detail,
            protocol: None,
            fallback: Fallback::Ask,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OutputSpec {
    pub protocol: Option<Protocol>,
    pub columns: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Destination {
    SharedCharacter,
    SharedImage,
    IndependentImage,
}

impl OutputSpec {
    /// Scope follows resolved output, never the raw Auto/Image/Character intent.
    /// Shared-pixel consent cannot change the destination of character output.
    pub fn destination(&self, binding: &TargetBinding) -> Destination {
        if self.protocol.is_none() {
            Destination::SharedCharacter
        } else if binding.shared_pixels {
            Destination::SharedImage
        } else {
            Destination::IndependentImage
        }
    }
}

impl Presentation {
    pub fn validate(&self) -> Result<(), String> {
        if !(8..=160).contains(&self.columns) {
            return Err("Artwork width must be 8–160 terminal columns (pixels: up to 120).".into());
        }
        Ok(())
    }
    pub fn resolve(
        &self,
        settings: &GreetingSettings,
        terminal: Terminal,
    ) -> Result<OutputSpec, String> {
        self.validate()?;
        let source = settings.editable_artwork.as_ref();
        let visual = match self.visual {
            Visual::Auto if source.is_none() => Visual::Character,
            Visual::Auto if source.is_some_and(|s| s.image.is_gif()) => Visual::Animation,
            Visual::Auto => Visual::Image,
            v => v,
        };
        let protocol = match visual {
            Visual::Character => None,
            Visual::Image | Visual::Animation => {
                if self.columns > 120 {
                    return Err("Pixel output supports up to 120 columns. Reduce Columns explicitly; the existing character artwork has not been resized.".into());
                }
                let source = source.ok_or("Add an image or GIF before selecting pixel output.")?;
                if visual == Visual::Animation && !source.image.is_gif() {
                    return Err(
                        "Animation requires a GIF source. Choose Image or Character.".into(),
                    );
                }
                let preferred = self.protocol.unwrap_or(if visual == Visual::Animation {
                    Protocol::KittyAnimation
                } else if terminal == Terminal::Xterm {
                    Protocol::Sixel
                } else {
                    Protocol::Kitty
                });
                if (visual == Visual::Animation) != (preferred == Protocol::KittyAnimation) {
                    return Err(
                        "The advanced protocol conflicts with the chosen display effect.".into(),
                    );
                }
                Some(preferred)
            }
            Visual::Auto => unreachable!(),
        };
        // A recommendation is not evidence that this terminal supports pixels.
        Ok(OutputSpec {
            protocol,
            columns: self.columns,
        })
    }
}

/// Old documents keep their existing character artwork and recipe dimensions.
/// Migration does not decode/re-render a source or grant pixel approval.
pub(crate) fn migrate(settings: &mut GreetingSettings, bytes: &[u8]) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if value
        .get("greeting")
        .is_some_and(|g| g.get("presentation").is_none())
    {
        settings.presentation = Presentation::default();
        sync_recipe(settings)?;
    }
    Ok(())
}

pub(crate) fn sync_recipe(settings: &mut GreetingSettings) -> Result<(), String> {
    if let Some(source) = &settings.editable_artwork {
        let options = source.options()?;
        settings.presentation.columns = options.columns;
        settings.presentation.character_style = options.style;
    }
    Ok(())
}

pub(crate) fn select_character(settings: &mut GreetingSettings) {
    settings.presentation.visual = Visual::Character;
    settings.presentation.protocol = None;
}

/// Explicit semantic edit: regenerate character artwork only when its size or
/// style changes, retaining the embedded original and every processing option.
pub(crate) fn edit(
    settings: &GreetingSettings,
    presentation: Presentation,
) -> Result<GreetingSettings, String> {
    presentation.validate()?;
    let mut next = settings.clone();
    if let Some(source) = &settings.editable_artwork {
        let mut options = source.options()?;
        if options.columns != presentation.columns || options.style != presentation.character_style
        {
            options.columns = presentation.columns;
            options.style = presentation.character_style;
            let art = source.image.decode()?.convert(options)?.artwork;
            next.import_artwork(art)?;
            next.editable_artwork = Some(greeting_image::source::EditableArtwork::new(
                source.image.clone(),
                options,
            )?);
        }
    }
    next.presentation = presentation;
    next.validate()?;
    Ok(next)
}

/// Local-only scope. Independent pixels never overwrite the shared greeting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TargetBinding {
    pub terminal: Terminal,
    pub shared_pixels: bool,
}
impl Default for TargetBinding {
    fn default() -> Self {
        Self {
            terminal: Terminal::Ptyxis,
            shared_pixels: false,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct VerificationKey {
    presentation: Presentation,
    source: Option<greeting_image::source::EditableArtwork>,
    logo: serde_json::Value,
    terminal: Terminal,
    environment: String,
}
impl VerificationKey {
    pub fn new(
        settings: &GreetingSettings,
        terminal: Terminal,
        environment: String,
    ) -> Result<Self, String> {
        let config = crate::fastfetch_document::value(&settings.fastfetch_config()?)?;
        Ok(Self {
            presentation: settings.presentation.clone(),
            source: settings.editable_artwork.clone(),
            logo: config["logo"].clone(),
            terminal,
            environment,
        })
    }
}

fn hash_configuration_roots(
    paths: Vec<std::path::PathBuf>,
    hash: &mut impl std::hash::Hasher,
) -> Result<(), String> {
    use std::{
        hash::{Hash, Hasher},
        path::Path,
    };
    fn visit(
        path: &Path,
        hash: &mut impl Hasher,
        budget: &mut u64,
        depth: u8,
    ) -> Result<(), String> {
        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(e) => {
                return Err(format!(
                    "Cannot verify target environment {}: {e}",
                    path.display()
                ));
            }
        };
        if metadata.is_dir() {
            if depth == 0 {
                return Err("Target configuration is too deeply nested to verify.".into());
            }
            let mut paths = std::fs::read_dir(path)
                .map_err(|e| e.to_string())?
                .map(|entry| entry.map(|e| e.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            if paths.len() > 512 {
                return Err("Too many target configuration files to verify.".into());
            }
            paths.sort();
            for path in paths {
                visit(&path, hash, budget, depth - 1)?;
            }
        } else if metadata.is_file() {
            path.hash(hash);
            use std::io::Read;
            if metadata.len() > *budget {
                return Err("Target configuration exceeds the 4 MiB verification limit.".into());
            }
            let mut bytes = Vec::new();
            std::fs::File::open(path)
                .map_err(|e| e.to_string())?
                .take(*budget + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            *budget = budget
                .checked_sub(bytes.len() as u64)
                .ok_or("Target configuration grew during verification.")?;
            bytes.hash(hash);
        } else {
            return Err("Target configuration is not a regular file or directory.".into());
        }
        Ok(())
    }
    // XDG, the home fallback and KITTY_CONFIG_DIRECTORY can all name the same
    // directory, including via symlinks. Charge each canonical root only once.
    let mut roots = std::collections::BTreeSet::new();
    for path in paths {
        match std::fs::canonicalize(&path) {
            Ok(root) => {
                roots.insert(root);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Cannot verify target environment {}: {error}",
                    path.display()
                ));
            }
        }
    }
    let mut budget = 4 * 1024 * 1024;
    for path in roots {
        visit(&path, hash, &mut budget, 6)?;
    }
    Ok(())
}

/// Read-only, bounded fingerprint of known target configuration. Not a claim
/// about compositor state, runtime zoom or includes outside these locations.
pub(crate) fn environment(terminal: Terminal) -> Result<String, String> {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let home = gtk::glib::home_dir();
    let config = gtk::glib::user_config_dir();
    let paths = match terminal {
        Terminal::Kitty => vec![
            config.join("kitty"),
            home.join(".config/kitty"),
            "/etc/xdg/kitty".into(),
            std::env::var_os("KITTY_CONFIG_DIRECTORY")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| config.join("kitty")),
        ],
        Terminal::Xterm => vec![
            home.join(".Xresources"),
            home.join(".Xdefaults"),
            "/etc/X11/app-defaults/XTerm".into(),
        ],
        Terminal::Ptyxis => {
            crate::ptyxis::verification_environment().hash(&mut hash);
            vec![]
        }
    };
    hash_configuration_roots(paths, &mut hash)?;
    for name in [
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XENVIRONMENT",
        "XCURSOR_SIZE",
        "GDK_SCALE",
        "GDK_DPI_SCALE",
        "QT_SCALE_FACTOR",
    ] {
        (name, std::env::var_os(name)).hash(&mut hash);
    }
    Ok(format!("{:016x}", hash.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configuration_fingerprint(paths: Vec<std::path::PathBuf>) -> Result<u64, String> {
        use std::hash::Hasher;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        hash_configuration_roots(paths, &mut hash)?;
        Ok(hash.finish())
    }

    #[test]
    fn configuration_roots_charge_duplicate_defaults_and_aliases_once() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("kitty");
        std::fs::create_dir(&config).unwrap();
        let file = config.join("kitty.conf");
        std::fs::write(&file, vec![b'#'; 2 * 1024 * 1024]).unwrap();
        let single = configuration_fingerprint(vec![config.clone()]).unwrap();
        assert_eq!(
            configuration_fingerprint(vec![config.clone(); 3]).unwrap(),
            single
        );
        let alias = root.path().join("alias");
        std::os::unix::fs::symlink(&config, &alias).unwrap();
        assert_eq!(
            configuration_fingerprint(vec![alias.clone(), config.join("."), config.clone()])
                .unwrap(),
            single
        );
        assert_eq!(
            configuration_fingerprint(vec![config.clone(), alias, root.path().join("missing")])
                .unwrap(),
            single
        );
        std::fs::write(&file, vec![b' '; 2 * 1024 * 1024]).unwrap();
        assert_ne!(configuration_fingerprint(vec![config]).unwrap(), single);
    }

    #[test]
    fn configuration_roots_still_share_the_size_limit() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a");
        let b = root.path().join("b");
        for path in [&a, &b] {
            std::fs::create_dir(path).unwrap();
            std::fs::write(path.join("kitty.conf"), vec![b'#'; 2 * 1024 * 1024]).unwrap();
        }
        let original = configuration_fingerprint(vec![a.clone(), b.clone()]).unwrap();
        assert_eq!(
            configuration_fingerprint(vec![b.clone(), a.clone()]).unwrap(),
            original
        );
        std::fs::write(b.join("extra.conf"), b"#").unwrap();
        assert!(
            configuration_fingerprint(vec![a.clone(), b])
                .unwrap_err()
                .contains("4 MiB")
        );
        std::fs::write(a.join("kitty.conf"), vec![b'#'; 4 * 1024 * 1024 + 1]).unwrap();
        assert!(
            configuration_fingerprint(vec![a])
                .unwrap_err()
                .contains("4 MiB")
        );
    }
    #[test]
    fn resolved_destination_uses_actual_output_and_explicit_pixel_scope() {
        for source in [None, Some(false), Some(true)] {
            let (mut settings, _) = crate::pixel_trial::tests::fixture(source == Some(true));
            if source.is_none() {
                settings.editable_artwork = None;
            }
            for visual in [
                Visual::Auto,
                Visual::Character,
                Visual::Image,
                Visual::Animation,
            ] {
                settings.presentation.visual = visual;
                for terminal in Terminal::ALL {
                    for shared_pixels in [false, true] {
                        let binding = TargetBinding {
                            terminal,
                            shared_pixels,
                        };
                        let resolved = settings.presentation.resolve(&settings, terminal);
                        let character = visual == Visual::Character
                            || (visual == Visual::Auto && source.is_none());
                        let invalid = !character
                            && (source.is_none()
                                || (visual == Visual::Animation && source == Some(false)));
                        if invalid {
                            assert!(resolved.is_err());
                        } else {
                            let expected = if character {
                                Destination::SharedCharacter
                            } else if shared_pixels {
                                Destination::SharedImage
                            } else {
                                Destination::IndependentImage
                            };
                            assert_eq!(resolved.unwrap().destination(&binding), expected);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn legacy_recipe_migration_and_reediting_do_not_resize_or_forget_style() {
        use crate::{
            document_store::{decode, encode},
            greeting::GreetingPreset,
        };
        let (settings, _) = crate::pixel_trial::tests::fixture(false);
        let mut intent = settings.presentation.clone();
        intent.columns = 144;
        intent.character_style = greeting_image::Style::Ascii;
        let settings = edit(&settings, intent).unwrap();
        let mut legacy = serde_json::to_value(GreetingPreset::new(settings.clone())).unwrap();
        legacy["greeting"]
            .as_object_mut()
            .unwrap()
            .remove("presentation");
        let migrated: GreetingPreset = decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(migrated.greeting, settings);
        let reopened: GreetingPreset = decode(&encode(&migrated).unwrap()).unwrap();
        assert_eq!(reopened, migrated);
        let mut next = reopened.greeting;
        let source = next.editable_artwork.as_ref().unwrap();
        let mut recipe = source.options().unwrap();
        recipe.columns = 40;
        recipe.style = greeting_image::Style::HalfBlocks;
        next.editable_artwork = Some(
            greeting_image::source::EditableArtwork::new(source.image.clone(), recipe).unwrap(),
        );
        next.presentation.visual = Visual::Image;
        sync_recipe(&mut next).unwrap();
        assert_eq!(next.presentation.columns, 40);
        assert_eq!(next.presentation.character_style, recipe.style);
        assert_eq!(next.presentation.visual, Visual::Image);
        next.editable_artwork = None;
        select_character(&mut next);
        assert_eq!(
            next.presentation
                .resolve(&next, Terminal::Kitty)
                .unwrap()
                .protocol,
            None
        );
    }
    #[test]
    fn intent_is_not_capability_and_unrelated_fields_keep_visual_evidence() {
        let (mut settings, _) = crate::pixel_trial::tests::fixture(true);
        settings.presentation.visual = Visual::Animation;
        assert_eq!(
            settings
                .presentation
                .resolve(&settings, Terminal::Ptyxis)
                .unwrap()
                .protocol,
            Some(Protocol::KittyAnimation)
        );
        let key = VerificationKey::new(&settings, Terminal::Kitty, "font 12 / cells 10x20".into())
            .unwrap();
        settings.items[0].enabled = !settings.items[0].enabled;
        assert!(
            key == VerificationKey::new(&settings, Terminal::Kitty, "font 12 / cells 10x20".into())
                .unwrap()
        );
        assert!(
            key != VerificationKey::new(
                &settings,
                Terminal::Ptyxis,
                "font 12 / cells 10x20".into()
            )
            .unwrap()
        );
        assert!(
            key != VerificationKey::new(&settings, Terminal::Kitty, "font 14 / cells 12x24".into())
                .unwrap()
        );
        settings.presentation.columns = 48;
        assert!(
            key != VerificationKey::new(&settings, Terminal::Kitty, "font 12 / cells 10x20".into())
                .unwrap()
        );
        settings.presentation.visual = Visual::Character;
        assert_eq!(
            settings
                .presentation
                .resolve(&settings, Terminal::Kitty)
                .unwrap()
                .protocol,
            None
        );
    }
    #[test]
    fn character_edits_retain_original_and_processing_options() {
        let (settings, _) = crate::pixel_trial::tests::fixture(true);
        let mut intent = settings.presentation.clone();
        intent.columns = 48;
        intent.character_style = greeting_image::Style::HalfBlocks;
        let edited = edit(&settings, intent).unwrap();
        assert_eq!(
            edited.editable_artwork.as_ref().unwrap().image,
            settings.editable_artwork.as_ref().unwrap().image
        );
        assert_eq!(
            edited
                .editable_artwork
                .as_ref()
                .unwrap()
                .options()
                .unwrap()
                .edits,
            settings
                .editable_artwork
                .as_ref()
                .unwrap()
                .options()
                .unwrap()
                .edits
        );
        assert_eq!(
            edited
                .editable_artwork
                .as_ref()
                .unwrap()
                .options()
                .unwrap()
                .columns,
            48
        );
        assert!(
            Presentation {
                columns: 0,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
