use super::*;

const TEST_ID: &str = "12345678-1234-4321-9876-0123456789ab";

#[test]
fn enhanced_startup_keyfile_does_not_double_escape_shell_quotes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("keyfile");
    fs::write(&path, "[other]\nkey=true\n").unwrap();
    let command = "/usr/bin/env 'PROMPT_COMMAND=source '\\''/tmp/path with space/startup.bash'\\''' /usr/bin/bash --noprofile --norc -i";
    set_custom_command(&path, command).unwrap();
    let key = glib::KeyFile::new();
    key.load_from_file(&path, glib::KeyFileFlags::NONE).unwrap();
    let raw = key
        .value(
            &format!("{SCHEMA_PATH}/Profiles/{PROFILE}"),
            "custom-command",
        )
        .unwrap();
    let value = glib::Variant::parse(None, &raw).unwrap();
    assert_eq!(value.str(), Some(command));
    let argv = glib::shell_parse_argv(command).unwrap();
    assert_eq!(
        argv[1],
        "PROMPT_COMMAND=source '/tmp/path with space/startup.bash'"
    );
    assert_eq!(key.value("other", "key").unwrap(), "true");
}
fn fixture() -> (tempfile::TempDir, PathBuf, Runtime) {
    let root = tempfile::tempdir().unwrap();
    let prefix = root.path().join("runtime");
    for dir in ["bin", "libexec", "share/glib-2.0/schemas"] {
        fs::create_dir_all(prefix.join(dir)).unwrap();
    }
    for path in ["bin/ptyxis", "libexec/ptyxis-agent"] {
        fs::write(prefix.join(path), "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(prefix.join(path), fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::write(
        prefix.join("share/glib-2.0/schemas/gschemas.compiled"),
        b"fixture",
    )
    .unwrap();
    fs::write(
        prefix.join(format!("share/glib-2.0/schemas/{ID}.gschema.xml")),
        ID,
    )
    .unwrap();
    fs::write(
        prefix.join("share/termimochi-background-canvas-version"),
        b"1\n",
    )
    .unwrap();
    let runtime = Runtime::capture(&prefix).unwrap();
    let data = root.path().join("data");
    (root, data, runtime)
}
fn appearance() -> Appearance {
    Appearance {
        background: Some(crate::background::tests::fixture()),
        palette: Some(include_str!("../../../resources/themes/fog-paper.palette").into()),
        canvas: [800, 400],
        light: true,
        ..Default::default()
    }
}
fn plan(data: &Path, runtime: &Runtime, name: &str) -> Plan {
    Plan::prepare_at(
        data,
        TEST_ID,
        name,
        appearance(),
        runtime.clone(),
        b"#!/bin/sh\nexit 0\n".to_vec(),
    )
    .unwrap()
}

#[test]
fn enhanced_publish_restart_update_restore_and_remove_retain_versions() {
    let (_root, data, runtime) = fixture();
    let first = plan(&data, &runtime, "First");
    assert!(!data.exists(), "preparation never publishes files");
    let one = first.publish().unwrap();
    one.active().unwrap();
    assert!(one.directory().unwrap().join("background.png").is_file());
    assert_eq!(
        fs::metadata(one.directory().unwrap().join("termimochi"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let reopened = current(&data, TEST_ID).unwrap().unwrap();
    assert_eq!(one.version(), reopened.version());
    assert_eq!(list(&data).unwrap().0.len(), 1);
    let two = plan(&data, &runtime, "Second").publish().unwrap();
    assert_ne!(one.version(), two.version());
    assert!(
        one.active().is_err(),
        "stale opens/recovery cannot act on a new version"
    );
    assert!(one.restore().is_err());
    let restored = two.restore().unwrap().unwrap();
    assert_eq!(restored.name(), "First");
    assert_eq!(
        current(&data, TEST_ID).unwrap().unwrap().version(),
        one.version()
    );
    assert!(restored.restore().unwrap().is_none());
    assert!(list(&data).unwrap().0.is_empty());
    assert!(one.directory().unwrap().is_dir() && two.directory().unwrap().is_dir());
    assert!(data.join(FOLDER).join(TEST_ID).join("backups").is_dir());
}

#[test]
fn enhanced_publication_conflicts_do_not_replace_newer_entries() {
    let (_root, data, runtime) = fixture();
    let a = plan(&data, &runtime, "A");
    let b = plan(&data, &runtime, "B");
    a.publish().unwrap();
    let before = fs::read(desktop_path(&data, TEST_ID)).unwrap();
    assert!(b.publish().is_err());
    assert_eq!(fs::read(desktop_path(&data, TEST_ID)).unwrap(), before);
    let pending = plan(&data, &runtime, "Pending");
    fs::write(desktop_path(&data, TEST_ID), "external").unwrap();
    assert!(pending.publish().is_err());
    assert_eq!(
        fs::read_to_string(desktop_path(&data, TEST_ID)).unwrap(),
        "external"
    );
    assert_eq!(list(&data).unwrap().1.len(), 1);
}

#[test]
fn enhanced_artifact_and_manifest_changes_block_open_update_and_restore() {
    let (_root, data, runtime) = fixture();
    let entry = plan(&data, &runtime, "Integrity").publish().unwrap();
    let dir = entry.directory().unwrap();
    let image = fs::read(dir.join("background.png")).unwrap();
    fs::write(dir.join("background.png"), b"external image").unwrap();
    assert!(entry.artifacts().is_err());
    assert!(entry.launch().is_err());
    assert!(entry.restore().is_err());
    assert!(
        Plan::prepare_at(
            &data,
            TEST_ID,
            "Changed",
            appearance(),
            runtime,
            b"helper".to_vec()
        )
        .is_err()
    );
    fs::write(dir.join("background.png"), image).unwrap();
    entry.artifacts().unwrap();
    fs::write(dir.join(RECORD), b"{}").unwrap();
    assert!(current(&data, TEST_ID).is_err());
    assert!(entry.restore().is_err());
}

#[test]
fn enhanced_aliases_and_traversal_are_rejected() {
    let (root, data, runtime) = fixture();
    assert!(current(&data, "../../daily").is_err());
    let alias = root.path().join("alias");
    fs::create_dir_all(&data).unwrap();
    std::os::unix::fs::symlink(&data, &alias).unwrap();
    assert!(
        Plan::prepare_at(
            &alias,
            TEST_ID,
            "Alias",
            appearance(),
            runtime.clone(),
            b"helper".to_vec()
        )
        .is_err()
    );
    let entry = plan(&data, &runtime, "Safe").publish().unwrap();
    let asset = entry.directory().unwrap().join("background.png");
    fs::hard_link(&asset, root.path().join("linked.png")).unwrap();
    assert!(entry.artifacts().is_err());
    assert!(entry.restore().is_err());
}

#[test]
fn enhanced_dependency_changes_never_fall_back_to_system_ptyxis() {
    let (_root, data, runtime) = fixture();
    let plan = plan(&data, &runtime, "Runtime");
    let entry = plan.publish().unwrap();
    fs::write(runtime.prefix.join("bin/ptyxis"), "#!/bin/sh\nexit 42\n").unwrap();
    assert!(entry.launch().is_err());
    assert!(plan.publish().is_err());
    assert_eq!(
        current(&data, TEST_ID).unwrap().unwrap().version(),
        entry.version()
    );
}

#[test]
fn enhanced_generated_shortcut_is_quoted_and_assets_are_frozen() {
    let (_root, data, runtime) = fixture();
    let unusual = data.join("space $percent% and 'quote'");
    let p = plan(&unusual, &runtime, "Preview % name");
    let expected = canvas(&appearance()).unwrap();
    let entry = p.publish().unwrap();
    let key = glib::KeyFile::new();
    key.load_from_data(
        std::str::from_utf8(&entry.desktop).unwrap(),
        glib::KeyFileFlags::NONE,
    )
    .unwrap();
    let exec = key.string("Desktop Entry", "Exec").unwrap();
    assert!(exec.contains("\\$") && exec.contains("%%") && exec.contains(ARG));
    assert_eq!(
        fs::read(entry.directory().unwrap().join("background.png")).unwrap(),
        expected
    );
    let a: Appearance = serde_json::from_slice(
        &fs::read(entry.directory().unwrap().join("appearance.json")).unwrap(),
    )
    .unwrap();
    assert!(
        a.background.is_none(),
        "only the frozen processed asset is reopened"
    );
    validate_appearance(&a).unwrap();
    assert!(!entry.details().contains("Kitty"));
}

#[test]
fn enhanced_readonly_pointer_failure_retains_active_version() {
    let (_root, data, runtime) = fixture();
    let old = plan(&data, &runtime, "Old").publish().unwrap();
    let next = plan(&data, &runtime, "New");
    let pointer = desktop_path(&data, TEST_ID);
    fs::set_permissions(&pointer, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(next.publish().is_err());
    assert_eq!(
        current(&data, TEST_ID).unwrap().unwrap().version(),
        old.version()
    );
    old.artifacts().unwrap();
}

#[test]
fn enhanced_backup_alias_cannot_redirect_publication_or_recovery() {
    let (root, data, runtime) = fixture();
    let old = plan(&data, &runtime, "Old").publish().unwrap();
    let outside = root.path().join("outside");
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, data.join(FOLDER).join(TEST_ID).join("backups")).unwrap();
    let next = plan(&data, &runtime, "Next");
    assert!(next.publish().is_err());
    assert!(old.restore().is_err());
    assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
    old.active().unwrap();
}
