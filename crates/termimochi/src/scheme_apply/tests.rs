use super::*;

const BEFORE: &str = "# personal comment\n[rust]\nsymbol = 'rs '\n";
const AFTER: &str = "# personal comment\n[rust]\nsymbol = 'rust '\n";
const FETCH: &str = "{\"logo\":{\"type\":\"none\"},\"modules\":[]}";

fn item(id: &'static str, action: Action) -> Item {
    Item {
        id,
        title: id.into(),
        detail: "Reviewed target".into(),
        versions: None,
        action: Some(action),
    }
}

fn fixture(root: &Path) -> (Plan, PathBuf, PathBuf) {
    let starship = root.join("starship.toml");
    let fastfetch = root.join("config.jsonc");
    std::fs::write(&starship, BEFORE).unwrap();
    let mut plan = Plan::new(&root.join("state"), "Ptyxis test target".into(), None);
    plan.items.push(item(
        "starship",
        Action::Starship {
            file: FileSnapshot::read(&starship).unwrap(),
            contents: AFTER.into(),
        },
    ));
    plan.items.push(item(
        "fastfetch",
        Action::Fastfetch {
            target: fastfetch_apply::Target::open(fastfetch.clone()).unwrap(),
            source: FETCH.into(),
        },
    ));
    plan.items
        .push(item("export", Action::ExportStarship(AFTER.into())));
    (plan, starship, fastfetch)
}

#[test]
fn review_cancel_empty_selection_and_validation_never_write_external_state() {
    let root = tempfile::tempdir().unwrap();
    let (plan, starship, fastfetch) = fixture(root.path());
    let directory = plan.directory.clone();
    assert!(!directory.exists());
    assert!(plan.apply(&[false, false, false]).is_err());
    assert!(!directory.exists());
    assert!(!root.path().join("state").exists());
    assert_eq!(std::fs::read_to_string(starship).unwrap(), BEFORE);
    assert!(!fastfetch.exists());
}

#[test]
fn selected_modules_partial_failure_restart_and_restore_are_independent() {
    let root = tempfile::tempdir().unwrap();
    let (plan, starship, fastfetch) = fixture(root.path());
    std::fs::write(&starship, "# external edit\n").unwrap();
    let (directory, report) = plan.apply(&[true, true, true]).unwrap();
    assert_eq!(
        report.items.iter().map(|r| r.status).collect::<Vec<_>>(),
        [Status::Failed, Status::Applied, Status::Exported]
    );
    assert!(report.items[0].undo.is_none());
    assert_eq!(std::fs::read_to_string(&fastfetch).unwrap(), FETCH);
    let (last, mut restored) = Report::latest(&root.path().join("state")).unwrap();
    assert_eq!(last, directory);
    restored.restore(&last).unwrap();
    assert!(!restored.can_restore());
    assert!(!fastfetch.exists());
    assert_eq!(
        std::fs::read_to_string(starship).unwrap(),
        "# external edit\n"
    );
    assert!(last.join("starship.toml").is_file()); // Exports are explicitly kept.
    restored.restore(&last).unwrap(); // Idempotent: never repeat a completed undo.
}

#[test]
fn restore_blocks_external_changes_but_restores_other_successes() {
    let root = tempfile::tempdir().unwrap();
    let (plan, starship, fastfetch) = fixture(root.path());
    let (directory, mut report) = plan.apply(&[true, true, false]).unwrap();
    std::fs::write(&fastfetch, "{\"modules\":[\"os\"]}").unwrap();
    report.restore(&directory).unwrap();
    assert_eq!(report.items[0].status, Status::Restored);
    assert_eq!(report.items[1].status, Status::RestoreBlocked);
    assert_eq!(report.items[2].status, Status::Skipped);
    assert!(report.can_restore());
    assert_eq!(std::fs::read_to_string(starship).unwrap(), BEFORE);
    assert_eq!(
        std::fs::read_to_string(fastfetch).unwrap(),
        "{\"modules\":[\"os\"]}"
    );
}

#[test]
fn no_op_does_not_restore_previous_module_transactions() {
    let root = tempfile::tempdir().unwrap();
    let (mut plan, starship, fastfetch) = fixture(root.path());
    plan.items[0].action = Some(Action::Starship {
        file: FileSnapshot::read(&starship).unwrap(),
        contents: BEFORE.into(),
    });
    std::fs::write(&fastfetch, FETCH).unwrap();
    plan.items[1].action = Some(Action::Fastfetch {
        target: fastfetch_apply::Target::open(fastfetch).unwrap(),
        source: FETCH.into(),
    });
    let (directory, report) = plan.apply(&[true, true, false]).unwrap();
    assert_eq!(report.items[0].status, Status::Unchanged);
    assert_eq!(report.items[1].status, Status::Unchanged);
    assert!(!report.can_restore());
    assert!(!directory.join("last-fastfetch-apply.json").exists());
}

#[test]
fn per_application_receipts_survive_later_module_apply() {
    let root = tempfile::tempdir().unwrap();
    let (plan, _, fastfetch) = fixture(root.path());
    let (directory, mut report) = plan.apply(&[false, true, false]).unwrap();
    let mut target = fastfetch_apply::Target::open(fastfetch.clone()).unwrap();
    let standalone = root.path().join("other-apply");
    fastfetch_apply::apply(&mut target, "{\"modules\":[\"cpu\"]}", &standalone).unwrap();
    report.restore(&directory).unwrap();
    assert_eq!(report.items[1].status, Status::RestoreBlocked);
    fastfetch_apply::restore(
        &fastfetch_apply::prepare_restore(&standalone).unwrap(),
        &standalone,
    )
    .unwrap();
    Report::load(&directory)
        .unwrap()
        .restore(&directory)
        .unwrap();
    assert!(!fastfetch.exists());
}

#[test]
fn palette_review_rejects_late_edits_and_symlinks_without_installing() {
    let root = tempfile::tempdir().unwrap();
    let palettes = root.path().join("palettes");
    std::fs::create_dir(&palettes).unwrap();
    let path = palettes.join("test.palette");
    std::fs::write(&path, "old").unwrap();
    let installer = PtyxisInstaller::new(palettes.clone(), root.path().join("unused"));
    let mut plan = Plan::new(&root.path().join("state"), "test".into(), None);
    plan.items.push(item(
        "palette",
        Action::Palette {
            installer: installer.clone(),
            name: "test.palette".into(),
            before: Some(b"old".to_vec()),
            contents: "new".into(),
        },
    ));
    std::fs::write(&path, "external").unwrap();
    let (_, report) = plan.apply(&[true]).unwrap();
    assert_eq!(report.items[0].status, Status::Failed);
    assert!(!report.can_restore());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "external");
    std::os::unix::fs::symlink(&path, palettes.join("alias.palette")).unwrap();
    assert!(
        installer
            .install_reviewed("alias.palette", b"new", &Some(b"external".to_vec()))
            .is_err()
    );
    assert!(!root.path().join("unused").exists());
}

#[test]
fn corrupted_latest_pointer_cannot_escape_the_transaction_directory() {
    let root = tempfile::tempdir().unwrap();
    let pointer = root.path().join("scheme-applies/latest");
    typography_preset::write_private(&pointer, b"../../unrelated").unwrap();
    assert!(Report::latest(root.path()).is_err());
}

#[test]
fn interrupted_apply_journal_can_recover_only_its_attempted_module() {
    let root = tempfile::tempdir().unwrap();
    let (plan, _, fastfetch) = fixture(root.path());
    let (directory, mut report) = plan.apply(&[false, true, false]).unwrap();
    // Simulate a process exiting after target write but before result status update.
    report.items[1].status = Status::Pending;
    report.save(&directory).unwrap();
    let mut reopened = Report::load(&directory).unwrap();
    reopened.restore(&directory).unwrap();
    assert!(!fastfetch.exists());
    assert_eq!(reopened.items[1].status, Status::Restored);
}
