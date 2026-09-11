use super::*;

fn references() -> Workspace {
    Workspace::new(
        &PtyxisPalette::from_text(include_str!("../../resources/themes/fog-paper.palette"))
            .unwrap(),
        Variant::Light,
        TypographySettings::default(),
        LayoutSettings::default(),
        PromptSettings::default(),
        Some("# reference only\nformat = '$directory$character'\n".into()),
        false,
    )
}

#[test]
fn theme_color_only_size_edit_is_sparse_and_roundtrips() {
    let reference = references();
    let source = "# exact source\nbackground #123456\nforeground #abcdef\ninclude private.conf\n";
    let native = DesignDocument::from_kitty_source(source.into()).unwrap();
    let id = native.id.clone();
    let theme = native.into_theme(TargetHint::Kitty, "My Kitty").unwrap();
    assert_eq!(theme.id, id);
    let opening = theme.project_preview(&reference);
    assert_eq!(theme.capture_theme(&opening, &opening).unwrap(), theme);
    assert!(!theme.scope().typography);
    let mut now = opening.clone();
    now.typography.size = 19.0;
    let edited = theme.capture_theme(&opening, &now).unwrap();
    assert_eq!(
        edited
            .theme
            .as_ref()
            .unwrap()
            .typography
            .keys()
            .collect::<Vec<_>>(),
        vec!["size"]
    );
    let output = edited.theme_kitty_configuration(&now).unwrap();
    assert!(output.contains("font_size 19"));
    assert!(output.contains("include private.conf"));
    for forbidden in [
        "font_family",
        "modify_font",
        "cursor_shape",
        "initial_window",
    ] {
        assert!(!output.contains(forbidden), "{forbidden}");
    }
    let reopened = import(&document_store::encode(&edited).unwrap(), &reference).unwrap();
    assert_eq!(reopened, edited);
    assert_eq!(reopened.project_preview(&reference).typography.size, 19.0);
    assert_eq!(theme.capture_theme(&opening, &opening).unwrap(), theme);
}

#[test]
fn theme_inheritance_removes_only_owned_native_override_and_copy_retains_appearance() {
    let reference = references();
    let source = "background #123456\nfont_size 18\n# retained source\n";
    let mut theme = DesignDocument::from_kitty_source(source.into())
        .unwrap()
        .into_theme(TargetHint::Kitty, "Native")
        .unwrap();
    let copy = theme
        .convert_theme_copy(TargetHint::Ptyxis, &reference)
        .unwrap();
    assert_ne!(theme.id, copy.id);
    assert_eq!(copy.theme.as_ref().unwrap().colors["Background"], "#123456");
    assert_eq!(copy.theme.as_ref().unwrap().typography["size"], 18.0);
    theme
        .theme
        .as_mut()
        .unwrap()
        .inherit
        .insert("typography.size".into());
    let preview = theme.project_preview(&reference);
    assert_eq!(preview.typography.size, reference.typography.size);
    assert!(
        !theme
            .theme_kitty_configuration(&preview)
            .unwrap()
            .contains("font_size")
    );
    assert_eq!(theme.theme.as_ref().unwrap().sources[0].text, source);
}

#[test]
fn empty_theme_owns_no_reference_values_and_validates_sparse_keys() {
    let mut theme = DesignDocument::new_theme(TargetHint::Kitty, "Empty", true);
    assert_eq!(theme.scope(), Scope::default());
    let reference = references();
    let projection = theme.project_preview(&reference);
    let output = theme.theme_kitty_configuration(&projection).unwrap();
    assert!(output.lines().all(|l| l.starts_with('#')));
    assert!(!projection.greeting.enabled);
    theme
        .theme
        .as_mut()
        .unwrap()
        .colors
        .insert("Typo".into(), "#ffffff".into());
    assert!(theme.validate().is_err());
}

#[test]
fn theme_name_and_both_palette_variants_survive_save_without_borrowing_defaults() {
    let theme = DesignDocument::new_theme(TargetHint::Kitty, "Original", true);
    let opening = theme.project_preview(&references());
    let mut changed = opening.clone();
    let mut palette = changed.palette().unwrap();
    palette.set_name("Renamed Theme").unwrap();
    palette
        .variant_mut(Variant::Light)
        .unwrap()
        .set("Background", "#123456".parse().unwrap())
        .unwrap();
    palette
        .variant_mut(Variant::Dark)
        .unwrap()
        .set("Background", "#abcdef".parse().unwrap())
        .unwrap();
    changed.palette = palette.to_palette_string();
    changed.light = false;
    let saved = theme.capture_theme(&opening, &changed).unwrap();
    let intent = saved.theme.as_ref().unwrap();
    assert_eq!(intent.name, "Renamed Theme");
    assert_eq!(intent.colors.len(), 1);
    assert_eq!(intent.dark_colors.len(), 1);
    let reopened = import(&document_store::encode(&saved).unwrap(), &references()).unwrap();
    let preview = reopened.project_preview(&references());
    assert_eq!(preview.palette().unwrap().name(), "Renamed Theme");
    assert!(
        reopened
            .theme_kitty_configuration(&preview)
            .unwrap()
            .contains("background #ABCDEF")
    );
    assert_eq!(reopened.theme.as_ref().unwrap().typography.len(), 0);
}

#[test]
fn palette_save_serializes_only_owned_colors_not_visible_references() {
    let mut workspace = references();
    workspace.greeting.message = "reference-greeting-must-not-save".into();
    workspace.typography.family = "reference-font-must-not-save".into();
    // Even an invalid unowned value cannot prevent saving this Palette.
    workspace.layout.rows = 0;
    let document = DesignDocument::from_workspace(
        Kind::Palette,
        Scope::for_kind(Kind::Palette),
        &workspace,
        None,
    )
    .unwrap();
    let bytes = document_store::encode(&document).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let components = value["components"].as_object().unwrap();
    assert_eq!(components.len(), 1);
    assert!(components.contains_key("palette"));
    let text = String::from_utf8(bytes).unwrap();
    for forbidden in [
        "reference-",
        "starship",
        "typography",
        "greeting",
        "layout",
        "approved",
        "profile_id",
        "document_id",
    ] {
        assert!(!text.contains(forbidden), "leaked {forbidden}");
    }
    assert_eq!(document.target_hint, Some(TargetHint::Ptyxis));
    assert_eq!(document.kind.label(), "Ptyxis Palette");
}

#[test]
fn projection_and_save_cannot_promote_changed_references_into_ownership() {
    let base = references();
    let doc =
        DesignDocument::from_workspace(Kind::Prompt, Scope::for_kind(Kind::Prompt), &base, None)
            .unwrap();
    let mut projection = doc.project_preview(&base);
    projection.typography.family = "another reference".into();
    projection.greeting.enabled = true;
    projection.layout.columns = 119;
    let mut saved =
        DesignDocument::from_workspace(doc.kind, doc.scope(), &projection, doc.native.clone())
            .unwrap();
    saved.id.clone_from(&doc.id);
    assert_eq!(
        document_store::encode(&doc).unwrap(),
        document_store::encode(&saved).unwrap()
    );
    assert!(saved.scope().require(Action::Prompt).is_ok());
    assert!(saved.scope().require(Action::Palette).is_err());
    assert!(saved.scope().require(Action::Greeting).is_err());
    assert!(saved.scope().require(Action::Typography).is_err());
    assert!(saved.scope().require(Action::Layout).is_err());
    assert!(saved.scope().require(Action::Artwork).is_err());
}

#[test]
fn each_type_roundtrips_and_scope_cannot_be_forged() {
    let workspace = references();
    for kind in [
        Kind::Palette,
        Kind::KittyAppearance,
        Kind::Prompt,
        Kind::Greeting,
        Kind::Typography,
        Kind::Layout,
        Kind::Artwork,
        Kind::Project,
        Kind::Legacy,
    ] {
        let scope = if kind == Kind::Project {
            Scope::for_kind(Kind::Palette)
        } else {
            Scope::for_kind(kind)
        };
        let doc = DesignDocument::from_workspace(kind, scope, &workspace, None).unwrap();
        let loaded = import(&document_store::encode(&doc).unwrap(), &workspace).unwrap();
        assert_eq!(doc, loaded);
        assert_eq!(doc.scope(), scope);
    }
    let mut forged = DesignDocument::from_workspace(
        Kind::Legacy,
        Scope::for_kind(Kind::Legacy),
        &workspace,
        None,
    )
    .unwrap();
    forged.kind = Kind::Palette;
    assert!(document_store::encode(&forged).is_err());
    assert!(
        DesignDocument::from_workspace(Kind::Prompt, Scope::default(), &workspace, None).is_err()
    );
    let mut scope = Scope::for_kind(Kind::Greeting);
    scope.artwork = true;
    assert!(DesignDocument::from_workspace(Kind::Project, scope, &workspace, None).is_err());
}

#[test]
fn standalone_artwork_does_not_save_system_fields_or_imported_commands() {
    let mut workspace = references();
    workspace.greeting.imported_source = Some("{\"modules\":[{\"type\":\"command\",\"text\":\"must-not-run\"}],\"preRun\":\"must-not-run-either\"}".into());
    workspace.greeting.message = "reference-system-fields".into();
    let doc = DesignDocument::from_workspace(
        Kind::Artwork,
        Scope::for_kind(Kind::Artwork),
        &workspace,
        None,
    )
    .unwrap();
    let encoded = String::from_utf8(document_store::encode(&doc).unwrap()).unwrap();
    for forbidden in [
        "modules",
        "preRun",
        "must-not-run",
        "reference-system-fields",
        "imported_source",
        "field_styles",
    ] {
        assert!(!encoded.contains(forbidden));
    }
    assert!(doc.scope().allows(Action::Artwork));
    assert!(!doc.scope().allows(Action::Greeting));
}

#[test]
fn own_schema_rejects_unknown_versions_duplicate_keys_and_nested_unknown_fields() {
    let doc = DesignDocument::from_workspace(
        Kind::Palette,
        Scope::for_kind(Kind::Palette),
        &references(),
        None,
    )
    .unwrap();
    let valid = String::from_utf8(document_store::encode(&doc).unwrap()).unwrap();
    for invalid in [
        valid.replace("\"version\": 2", "\"version\": 99"),
        valid.replace("\"version\": 2", "\"version\": 2, \"version\": 2"),
        valid.replace("\"light\": true", "\"light\": true, \"light\": false"),
        valid.replace("\"light\": true", "\"light\": true, \"execute\": true"),
        valid.replace(
            "\"kind\": \"palette\"",
            "\"kind\": \"palette\", \"profile_id\": \"foreign\"",
        ),
        valid.replace("termimochi-design", "unknown-schema"),
    ] {
        assert_ne!(invalid, valid);
        assert!(
            import(invalid.as_bytes(), &references()).is_err(),
            "accepted {invalid}"
        );
    }
}

#[test]
fn migration_keeps_exact_v1_bytes_without_binding_or_changing_original() {
    let workspace = references();
    let bytes = document_store::encode(&workspace).unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("old.termimochi.json");
    std::fs::write(&path, &bytes).unwrap();
    let migrated = import(&bytes, &workspace).unwrap();
    assert_eq!(migrated.kind, Kind::Legacy);
    assert_eq!(migrated.target_hint, None);
    assert_eq!(
        migrated.native.as_ref().unwrap().format,
        NativeFormat::LegacyWorkspace
    );
    assert_eq!(migrated.native.as_ref().unwrap().text.as_bytes(), bytes);
    assert_eq!(migrated.project_preview(&workspace), workspace);
    let reopened =
        document_store::decode::<DesignDocument>(&document_store::encode(&migrated).unwrap())
            .unwrap();
    assert_eq!(migrated, reopened);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn old_presets_import_as_independent_documents() {
    let workspace = references();
    for (bytes, kind) in [
        (
            crate::typography_preset::encode(&workspace.typography).unwrap(),
            Kind::Typography,
        ),
        (
            document_store::encode(&LayoutPreset::new(workspace.layout)).unwrap(),
            Kind::Layout,
        ),
        (
            document_store::encode(&GreetingPreset::new(workspace.greeting.clone())).unwrap(),
            Kind::Greeting,
        ),
    ] {
        let doc = import(&bytes, &workspace).unwrap();
        assert_eq!(doc.kind, kind);
        assert_eq!(doc.scope(), Scope::for_kind(kind));
        assert_eq!(doc.native.as_ref().unwrap().text.as_bytes(), bytes);
    }
}

#[test]
fn native_detection_uses_content_without_executing_or_guessing_extensions() {
    let workspace = references();
    for (source, kind) in [
        (
            "# Starship\nformat = '$directory$character'\n[custom.once]\ncommand='must-not-run'\n",
            Kind::Prompt,
        ),
        (
            "// Fastfetch\n{\"modules\":[\"os\",\"os\",{\"type\":\"command\",\"text\":\"must-not-run\"}],}",
            Kind::Greeting,
        ),
        (&workspace.palette, Kind::Palette),
    ] {
        assert_eq!(detect(source.as_bytes()).unwrap(), Detected::Native(kind));
        let doc = import(source.as_bytes(), &workspace).unwrap();
        assert_eq!(doc.kind, kind);
        assert_eq!(doc.native.as_ref().unwrap().text, source);
        assert_eq!(doc.scope(), Scope::for_kind(kind));
    }
    assert_eq!(
        detect(b"font_family Monospace\nbackground #121212\n").unwrap(),
        Detected::Native(Kind::KittyAppearance)
    );
    for source in [
        "# empty\n",
        "{\"$schema\":\"https://unrelated/config.json\",\"modules\":[]}",
        "{broken",
        "[broken",
    ] {
        assert!(detect(source.as_bytes()).is_err(), "guessed {source}");
    }
    for (source, kind) in [
        ("{}", Kind::Greeting),
        ("name = 'unclear'", Kind::Prompt),
        ("include preserve.conf\n", Kind::KittyAppearance),
    ] {
        assert_eq!(
            detect(source.as_bytes()).unwrap(),
            Detected::Ambiguous(kind)
        );
        assert!(import(source.as_bytes(), &workspace).is_err());
    }
    let confirmed = import_as(b"name = 'unclear'", Kind::Prompt, &workspace).unwrap();
    assert_eq!(confirmed.scope(), Scope::for_kind(Kind::Prompt));
    assert!(import_as(b"name = 'unclear'", Kind::Greeting, &workspace).is_err());
    assert!(import_as(b"{\"schema\":\"unknown\"}", Kind::Greeting, &workspace).is_err());
}

#[test]
fn native_fastfetch_preserves_legal_duplicate_modules_unknowns_and_comments() {
    let source = "// retain\n{\"modules\":[\"os\",\"os\"],\"preRun\":\"touch forbidden\",\"unknown\":{\"future\":42},}\n";
    let doc = import(source.as_bytes(), &references()).unwrap();
    let reopened = import(&document_store::encode(&doc).unwrap(), &references()).unwrap();
    assert_eq!(
        reopened
            .components
            .greeting
            .as_ref()
            .unwrap()
            .imported_source
            .as_deref(),
        Some(source)
    );
    assert_eq!(reopened.native.as_ref().unwrap().text, source);
}

#[test]
fn copy_conversion_has_new_id_real_loss_report_and_no_inherited_target() {
    let original = import(
        &document_store::encode(&references()).unwrap(),
        &references(),
    )
    .unwrap();
    let original_bytes = document_store::encode(&original).unwrap();
    let (copy, notes) = original
        .convert_copy(Kind::Palette, Scope::for_kind(Kind::Palette), &references())
        .unwrap();
    assert_ne!(copy.id, original.id);
    assert_eq!(copy.kind, Kind::Palette);
    assert!(copy.components.prompt.is_none());
    for name in ["Typography", "Layout", "Prompt", "Greeting"] {
        assert!(notes.iter().any(|n| n.starts_with(name)));
    }
    assert!(copy.native.is_none());
    assert_eq!(document_store::encode(&original).unwrap(), original_bytes);
}

#[test]
fn native_kitty_type_never_grants_prompt_or_greeting_and_allows_sparse_ownership() {
    let source = "font_size 14\ninclude unchanged.conf\n";
    let doc = DesignDocument::from_kitty_source(source.into()).unwrap();
    assert_eq!(doc.target_hint, Some(TargetHint::Kitty));
    assert!(doc.components.palette.is_none());
    assert!(doc.components.typography.is_none());
    assert!(doc.scope().allows(Action::Typography));
    assert!(!doc.scope().allows(Action::Prompt));
    assert!(!doc.scope().allows(Action::Greeting));
    let bytes = document_store::encode(&doc).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("font_family"));
    assert!(!String::from_utf8_lossy(&bytes).contains("Monospace"));
    let reopened = import(&bytes, &references()).unwrap();
    assert_eq!(reopened, doc);
    assert_eq!(
        reopened.project_preview(&references()).typography.size,
        14.0
    );
    assert_eq!(reopened.native.as_ref().unwrap().text, source);
    assert!(
        DesignDocument::from_workspace(
            Kind::KittyAppearance,
            Scope::for_kind(Kind::Legacy),
            &references(),
            None
        )
        .is_err()
    );
}

#[test]
fn native_kitty_explicit_missing_field_edit_is_owned_but_other_defaults_remain_references() {
    let source = "# exact\nfont_size 50\ninclude preserved.conf\n";
    let original = crate::kitty_document::KittyDocument::parse(source).unwrap();
    let opening = original.project_workspace(&references()).unwrap();
    let unchanged = original
        .reconcile_with_reference(&opening, &opening)
        .unwrap();
    assert_eq!(
        unchanged, source,
        "clamped display must not replace authored font_size 50"
    );
    let mut edited = opening.clone();
    edited.typography.family = "Explicit Font".into();
    let updated_source = original
        .reconcile_with_reference(&edited, &opening)
        .unwrap();
    let saved = DesignDocument::from_kitty_source(updated_source).unwrap();
    let bytes = document_store::encode(&saved).unwrap();
    let reopened = import(&bytes, &references()).unwrap();
    let text = &reopened.native.as_ref().unwrap().text;
    assert!(text.contains("font_family Explicit Font"));
    assert!(text.contains("font_size 50\n"));
    assert!(text.contains("include preserved.conf\n"));
    assert!(!text.contains("background"));
    assert!(!text.contains("window_padding"));
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["components"], serde_json::json!({}));
}

#[test]
fn native_copy_reports_actual_unmapped_and_approximated_fields() {
    let source = "font_size 50\nbold_font Custom Bold\ninitial_window_width 800\nwindow_padding_width 8 12\ninclude original.conf\n";
    let document = DesignDocument::from_kitty_source(source.into()).unwrap();
    let (_, notes) = document
        .convert_copy(
            Kind::Project,
            Scope {
                typography: true,
                layout: true,
                ..Scope::default()
            },
            &references(),
        )
        .unwrap();
    for expected in [
        "native 50",
        "bold_font",
        "uses pixels",
        "per-edge",
        "include",
    ] {
        assert!(
            notes.iter().any(|note| note.contains(expected)),
            "missing {expected}: {notes:?}"
        );
    }
    assert_eq!(document.native.as_ref().unwrap().text, source);
}

#[test]
fn native_export_uses_current_owned_source_not_original_provenance_or_references() {
    let references = references();
    let mut prompt = import(b"# initial\nformat='$directory'\n", &references).unwrap();
    prompt.components.prompt.as_mut().unwrap().starship =
        Some("# changed\nformat='$rust$character'\n".into());
    let (filename, text) = prompt.native_export().unwrap();
    assert_eq!(filename, "starship.toml");
    assert!(text.contains("# changed"));
    assert!(!text.contains("# initial"));
    assert!(!text.contains("reference"));
    prompt.components.prompt.as_mut().unwrap().use_designer = true;
    assert!(
        prompt
            .native_export()
            .unwrap()
            .1
            .contains("Generated by TermiMochi")
    );

    let mut greeting = import(b"{\"modules\":[\"os\"]}", &references).unwrap();
    greeting
        .components
        .greeting
        .as_mut()
        .unwrap()
        .imported_source = Some("// changed\n{\"modules\":[\"cpu\",\"cpu\"]}".into());
    assert!(greeting.native_export().unwrap().1.contains("// changed"));
    assert!(
        greeting
            .native_export()
            .unwrap()
            .1
            .contains("\"cpu\",\"cpu\"")
    );

    let source = "font_size 14\ninclude unexecuted.conf\n";
    let kitty = DesignDocument::from_kitty_source(source.into()).unwrap();
    assert_eq!(
        kitty.native_export().unwrap(),
        ("kitty.conf", source.into())
    );
    let font_only = DesignDocument::from_workspace(
        Kind::KittyAppearance,
        Scope {
            typography: true,
            ..Scope::default()
        },
        &references,
        None,
    )
    .unwrap();
    let (_, native) = font_only.native_export().unwrap();
    assert!(native.contains("font_family"));
    assert!(!native.contains("background "));
    assert!(!native.contains("initial_window_width"));
    assert!(!native.contains("fastfetch"));
    for kind in [Kind::Typography, Kind::Layout, Kind::Palette] {
        let doc =
            DesignDocument::from_workspace(kind, Scope::for_kind(kind), &references, None).unwrap();
        let (_, text) = doc.native_export().unwrap();
        assert!(!text.contains("reference only"));
        assert_eq!(import(text.as_bytes(), &references).unwrap().kind, kind);
    }
    for kind in [Kind::Artwork, Kind::Legacy, Kind::Project] {
        let scope = if kind == Kind::Project {
            Scope::for_kind(Kind::Palette)
        } else {
            Scope::for_kind(kind)
        };
        let doc = DesignDocument::from_workspace(kind, scope, &references, None).unwrap();
        assert!(doc.native_export().is_err());
    }
}

#[test]
fn empty_projects_and_duplicate_nested_maps_are_rejected_without_deduplicating_native_modules() {
    assert!(
        DesignDocument::from_workspace(Kind::Project, Scope::default(), &references(), None)
            .is_err()
    );
    assert!(
        DesignDocument::from_workspace(
            Kind::KittyAppearance,
            Scope::default(),
            &references(),
            None
        )
        .is_err()
    );
    let greeting = import(b"{\"modules\":[\"os\",\"os\"]}", &references()).unwrap();
    let bytes = document_store::encode(&greeting).unwrap();
    assert!(import(&bytes, &references()).is_ok());
    let text = String::from_utf8(bytes).unwrap();
    let duplicate = text.replace(
        "\"field_styles\": {}",
        "\"field_styles\": {\"imported:0\":{},\"imported:0\":{}}",
    );
    assert_ne!(duplicate, text);
    assert!(
        import(duplicate.as_bytes(), &references())
            .unwrap_err()
            .contains("Duplicate key")
    );
    let mut malformed = greeting;
    malformed.kind = Kind::KittyAppearance;
    malformed.native = Some(NativeSource {
        format: NativeFormat::Kitty,
        text: "font_size 14\n".into(),
    });
    assert!(
        document_store::encode(&malformed).is_err(),
        "native and generated fields cannot have dual ownership"
    );
}

#[test]
fn capabilities_separate_implemented_editing_from_unverified_target_operations() {
    let palette = DesignDocument::from_workspace(
        Kind::Palette,
        Scope::for_kind(Kind::Palette),
        &references(),
        None,
    )
    .unwrap();
    let rows = palette.capabilities();
    assert_eq!(rows.len(), 8);
    assert!(
        rows.iter()
            .any(|(op, value)| *op == Operation::Edit && *value == Capability::Available)
    );
    assert!(
        rows.iter()
            .any(|(op, value)| *op == Operation::Trial
                && matches!(value, Capability::Unsupported(_)))
    );
    for (operation, capability) in rows {
        assert!(!operation.label().is_empty());
        assert!(!capability.label().is_empty());
        assert!(!capability.detail().is_empty());
    }
    let kitty = DesignDocument::from_kitty_source("font_size 14\n".into()).unwrap();
    for (operation, capability) in kitty.capabilities() {
        if matches!(
            operation,
            Operation::Trial
                | Operation::Write
                | Operation::Enable
                | Operation::Open
                | Operation::Restore
        ) {
            assert!(
                matches!(capability, Capability::NeedsRequirement(_)),
                "portable metadata must not claim local readiness"
            );
        }
    }
    let legacy = import(
        &document_store::encode(&references()).unwrap(),
        &references(),
    )
    .unwrap();
    assert!(
        legacy
            .capabilities()
            .iter()
            .any(|(op, value)| *op == Operation::Write
                && matches!(value, Capability::Unsupported(_)))
    );
}

#[test]
fn artwork_output_wrapper_never_borrows_reference_fields_position_or_commands() {
    let mut reference = references();
    reference.greeting.position = crate::greeting::Position::Right;
    reference.greeting.gap = 8;
    reference.greeting.message = "borrowed-message-sentinel".into();
    reference.greeting.imported_source = Some("{\"preRun\":\"borrowed-command-sentinel\",\"modules\":[\"os\",{\"type\":\"command\",\"text\":\"borrowed-command-sentinel\"}]}".into());
    let artwork = DesignDocument::from_workspace(
        Kind::Artwork,
        Scope::for_kind(Kind::Artwork),
        &reference,
        None,
    )
    .unwrap();
    let output = artwork.greeting_output().unwrap();
    let default = GreetingSettings::default();
    assert!(output.enabled);
    assert_eq!(output.position, default.position);
    assert_eq!(output.gap, default.gap);
    assert!(output.message.is_empty());
    assert!(output.items.iter().all(|item| !item.enabled));
    assert!(output.official_preset.is_none());
    assert!(output.official_items.is_empty());
    assert!(output.imported_source.is_none());
    assert!(output.field_styles.is_empty());
    let config = output.fastfetch_config().unwrap();
    assert!(!config.contains("borrowed-message-sentinel"));
    assert!(!config.contains("borrowed-command-sentinel"));
    let value = crate::fastfetch_document::value(&config).unwrap();
    assert!(
        !value["modules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|module| module == "os" || module["type"] == "command")
    );
    let greeting = DesignDocument::from_workspace(
        Kind::Greeting,
        Scope::for_kind(Kind::Greeting),
        &reference,
        None,
    )
    .unwrap();
    assert_eq!(greeting.greeting_output().unwrap(), reference.greeting);
    let palette = DesignDocument::from_workspace(
        Kind::Palette,
        Scope::for_kind(Kind::Palette),
        &reference,
        None,
    )
    .unwrap();
    assert!(palette.greeting_output().is_none());
}

#[test]
fn standalone_source_logo_materializes_actual_ansi_colors_without_changing_source_document() {
    let art = crate::greeting_art::Artwork::parse("\x1b[38;2;12;200;90mMOCHI\x1b[0m").unwrap();
    let descriptor = serde_json::json!({"type":"file-raw", "source":"original.ans"});
    let mut reference = references();
    reference.greeting.source_logo = Some(crate::greeting_art::LogoSnapshot {
        descriptor: descriptor.clone(),
        artwork: art.clone(),
        marked_source: None,
    });
    reference.greeting.imported_source = Some(serde_json::json!({"logo": descriptor, "preRun":"never-execute-sentinel", "modules":["os"]}).to_string());
    let document = DesignDocument::from_workspace(
        Kind::Artwork,
        Scope::for_kind(Kind::Artwork),
        &reference,
        None,
    )
    .unwrap();
    let before = document_store::encode(&document).unwrap();
    let output = document.greeting_output().unwrap();
    output.validate().unwrap();
    assert_eq!(output.logo, crate::greeting::Logo::Custom);
    assert_eq!(output.custom_art.as_ref(), Some(&art));
    assert_eq!(output.artwork_ansi(), art.ansi);
    assert!(output.source_logo.is_none());
    assert!(output.imported_source.is_none());
    assert!(output.items.iter().all(|item| !item.enabled));
    let config = output.fastfetch_config().unwrap();
    let value = crate::fastfetch_document::value(&config).unwrap();
    assert_eq!(value["logo"]["source"], art.ansi);
    assert!(!config.contains("original.ans"));
    assert!(!config.contains("never-execute-sentinel"));
    assert_eq!(document_store::encode(&document).unwrap(), before);
    assert!(
        document
            .components
            .artwork
            .as_ref()
            .unwrap()
            .source_logo
            .is_some()
    );
    let mut plain_document = document;
    plain_document
        .components
        .artwork
        .as_mut()
        .unwrap()
        .source_logo
        .as_mut()
        .unwrap()
        .artwork = crate::greeting_art::Artwork::parse("PLAIN").unwrap();
    let plain = plain_document.greeting_output().unwrap();
    assert_eq!(
        plain.accent, 16,
        "plain native artwork must follow foreground, not acquire the template accent"
    );
    plain.validate().unwrap();
    assert_eq!(plain.custom_art.as_ref().unwrap().plain, "PLAIN");
}

#[test]
fn ptyxis_project_cannot_route_pixels_to_another_terminal_but_standalone_shared_greeting_remains_available()
 {
    use crate::greeting_image::pixel_export::Protocol;
    let mut project = DesignDocument::from_workspace(
        Kind::Project,
        Scope::for_kind(Kind::Greeting),
        &references(),
        None,
    )
    .unwrap();
    project.target_hint = Some(TargetHint::Ptyxis);
    let before = document_store::encode(&project).unwrap();
    for protocol in [Protocol::Kitty, Protocol::Sixel, Protocol::KittyAnimation] {
        let error = project.require_greeting_target(Some(protocol)).unwrap_err();
        assert!(error.contains("Kitty project copy"));
        assert!(error.contains("Character"));
    }
    assert!(project.require_greeting_target(None).is_ok());
    assert_eq!(
        document_store::encode(&project).unwrap(),
        before,
        "a rejected target does not silently convert or mutate the design"
    );
    project.target_hint = Some(TargetHint::Kitty);
    assert!(
        project
            .require_greeting_target(Some(Protocol::KittyAnimation))
            .is_ok()
    );
    let mut standalone = DesignDocument::from_workspace(
        Kind::Greeting,
        Scope::for_kind(Kind::Greeting),
        &references(),
        None,
    )
    .unwrap();
    standalone.target_hint = Some(TargetHint::Ptyxis);
    assert!(
        standalone
            .require_greeting_target(Some(Protocol::KittyAnimation))
            .is_ok(),
        "standalone shared-file flow makes its own explicit target and pixel-scope checks"
    );
}

#[test]
fn checked_storage_preserves_conflicts_and_two_windows_cannot_overwrite() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("colors.termimochi-design.json");
    let doc = DesignDocument::from_workspace(
        Kind::Palette,
        Scope::for_kind(Kind::Palette),
        &references(),
        None,
    )
    .unwrap();
    document_store::DocumentStore::open(path.clone())
        .unwrap()
        .save(&doc)
        .unwrap();
    let mut first = document_store::DocumentStore::<DesignDocument>::open(path.clone()).unwrap();
    let mut second = document_store::DocumentStore::<DesignDocument>::open(path.clone()).unwrap();
    let mut changed = doc.clone();
    changed
        .components
        .palette
        .as_mut()
        .unwrap()
        .source
        .push_str("\n# outside change\n");
    first.save(&changed).unwrap();
    assert!(second.save(&doc).is_err());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        document_store::encode(&changed).unwrap()
    );
    let alias = root.path().join("alias.termimochi-design.json");
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    assert!(document_store::DocumentStore::<DesignDocument>::open(alias).is_err());
}
