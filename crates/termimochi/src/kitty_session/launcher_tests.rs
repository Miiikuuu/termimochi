use super::*;

fn fixture(data: &Path, revision: u64) -> LauncherPlan {
    let workspace = Workspace::new(
        &termimochi_core::PtyxisPalette::from_text(include_str!(
            "../../resources/themes/fog-paper.palette"
        ))
        .unwrap(),
        termimochi_core::Variant::Light,
        Default::default(),
        Default::default(),
        Default::default(),
        None,
        true,
    );
    let exe = Executable::capture("/usr/bin/bash".into()).unwrap();
    let deployment = Plan::prepare_inner(
        &data.join("termimochi/kitty-sessions"),
        "launcher-a",
        "中文 ' $ % theme",
        revision,
        &workspace,
        Ownership {
            palette: true,
            ..Default::default()
        },
        None,
        [8, 16],
        Some(Dependencies {
            kitty: exe.clone(),
            bash: exe,
            helper: None,
            starship: None,
            fastfetch: None,
        }),
    )
    .unwrap()
    .publish()
    .unwrap();
    LauncherPlan {
        before: read(&desktop_path(data, &deployment.id)).unwrap(),
        deployment,
        data: data.into(),
        mode: ShellMode::Controlled,
        binary: b"#!/bin/sh\nexit 0\n".to_vec(),
        sources: vec![],
    }
}

#[test]
fn launcher_publication_update_undo_keeps_files_and_references() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("中文 ' $ % data");
    let first = fixture(&data, 1).install().unwrap();
    assert_eq!(
        Installed::discover_at(&data, "launcher-a")
            .unwrap()
            .unwrap()
            .path,
        first.path
    );
    let before = fs::read(desktop_path(&data, "launcher-a")).unwrap();
    let second = fixture(&data, 2).install().unwrap();
    assert!(first.restore().is_err());
    second.restore().unwrap();
    assert_eq!(fs::read(desktop_path(&data, "launcher-a")).unwrap(), before);
    first.restore().unwrap();
    assert!(!desktop_path(&data, "launcher-a").exists());
    assert!(first.path.exists() && second.path.exists());
    assert!(first.record().unwrap().runtime.path.exists());
    assert!(first.restore().is_err());
}

#[test]
fn launcher_conflicts_preserve_external_desktop_and_startup_files() {
    let temp = tempfile::tempdir().unwrap();
    let mut plan = fixture(temp.path(), 1);
    let rc = temp.path().join(".bashrc");
    fs::write(&rc, "sentinel").unwrap();
    plan.sources.push((rc.clone(), Some(b"sentinel".to_vec())));
    fs::write(&rc, "external edit").unwrap();
    assert!(
        plan.install()
            .err()
            .unwrap()
            .contains("Bash startup changed")
    );
    assert!(!desktop_path(temp.path(), "launcher-a").exists());
    fs::write(&rc, "sentinel").unwrap();
    let first = plan.install().unwrap();
    let newer = fixture(temp.path(), 2);
    let path = desktop_path(temp.path(), "launcher-a");
    fs::write(&path, "external launcher").unwrap();
    assert!(newer.install().is_err());
    assert!(first.restore().is_err());
    assert!(Installed::discover_at(temp.path(), "launcher-a").is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "external launcher");
    assert_eq!(fs::read_to_string(rc).unwrap(), "sentinel");
}

#[test]
fn launcher_rejects_stale_theme_symlinks_and_tampered_receipt() {
    let temp = tempfile::tempdir().unwrap();
    let plan = fixture(temp.path(), 1);
    let new = fixture(temp.path(), 2);
    assert!(plan.install().err().unwrap().contains("version changed"));
    let installed = new.install().unwrap();
    fs::write(&installed.path, "{}").unwrap();
    assert!(
        installed
            .restore()
            .err()
            .unwrap()
            .contains("receipt changed")
    );
    let temp = tempfile::tempdir().unwrap();
    let victim = temp.path().join("victim");
    fs::write(&victim, "unchanged").unwrap();
    let plan = fixture(temp.path(), 1);
    fs::create_dir(temp.path().join("applications")).unwrap();
    std::os::unix::fs::symlink(&victim, desktop_path(temp.path(), "launcher-a")).unwrap();
    assert!(plan.install().is_err());
    assert_eq!(fs::read_to_string(victim).unwrap(), "unchanged");
}

#[test]
fn launcher_desktop_quoting_keeps_literal_arguments() {
    let arg = "/data/中文 ' double\" dollar$ tick` slash\\ percent%f";
    let quoted = exec_arg(arg).unwrap();
    let key = glib::KeyFile::new();
    key.set_string("Desktop Entry", "Exec", &quoted);
    let decoded = glib::KeyFile::new();
    decoded
        .load_from_data(&key.to_data(), glib::KeyFileFlags::NONE)
        .unwrap();
    let expanded = decoded
        .string("Desktop Entry", "Exec")
        .unwrap()
        .replace("%%", "%");
    let parsed = glib::shell_parse_argv(expanded).unwrap();
    assert_eq!(parsed, vec![std::ffi::OsString::from(arg)]);
    assert!(exec_arg("bad\narg").is_err());
}

#[test]
fn launcher_personal_bash_does_not_discard_unowned_prompt_or_history() {
    let temp = tempfile::tempdir().unwrap();
    let plan = fixture(temp.path(), 1);
    let personal = startup(&plan.deployment, ShellMode::PersonalBash).unwrap();
    assert!(personal.contains(".bashrc") && !personal.contains(".profile"));
    assert!(!personal.contains("HISTFILE") && !personal.contains("PS1="));
    let controlled = startup(&plan.deployment, ShellMode::Controlled).unwrap();
    assert!(!controlled.contains(".bashrc") && controlled.contains("HISTFILE"));
    let command = session_command(
        &plan.deployment,
        ShellMode::PersonalBash,
        Path::new("/safe startup"),
    )
    .unwrap();
    let env: BTreeMap<_, _> = command
        .get_envs()
        .filter_map(|(k, v)| {
            v.map(|v| {
                (
                    k.to_string_lossy().to_string(),
                    v.to_string_lossy().to_string(),
                )
            })
        })
        .collect();
    assert_eq!(env["PROMPT_COMMAND"], "source '/safe startup'");
    assert!(!env.contains_key("BASH_ENV") && !env.contains_key("ENV"));
}

#[test]
fn launcher_runtime_is_retained_and_shared_by_content() {
    let temp = tempfile::tempdir().unwrap();
    let first = fixture(temp.path(), 1).install().unwrap();
    let second = fixture(temp.path(), 2).install().unwrap();
    let a = first.record().unwrap();
    let b = second.record().unwrap();
    assert_eq!(a.runtime.path, b.runtime.path);
    assert_eq!(
        fs::metadata(a.runtime.path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        read(&second.path.parent().unwrap().join("startup.bash"))
            .unwrap()
            .as_deref()
            .map(digest)
            .unwrap(),
        b.startup_hash
    );
}

#[test]
fn launcher_settle_helper_survives_original_editor_removal() {
    let temp = tempfile::tempdir().unwrap();
    let editor = temp.path().join("movable-editor");
    fs::write(&editor, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o700)).unwrap();
    let mut plan = fixture(temp.path(), 1);
    plan.deployment.dependencies.helper = Some(Executable::capture(editor.clone()).unwrap());
    let installed = plan.install().unwrap();
    fs::remove_file(&editor).unwrap();
    let receipt = installed.record().unwrap();
    assert_eq!(
        receipt
            .deployment
            .dependencies
            .helper
            .as_ref()
            .unwrap()
            .path,
        receipt.runtime.path
    );
    assert!(receipt.deployment.command().is_ok());
    assert!(
        !fs::read_to_string(installed.path.parent().unwrap().join("startup.bash"))
            .unwrap()
            .contains("movable-editor")
    );
}

#[test]
#[ignore = "private HOME/XDG/D-Bus/Xvfb; actual GIO desktop launch, Kitty, personal Bash, Starship and moving GIF"]
fn launcher_native_daily_gif_and_desktop_reopen() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    assert!(!matches!(
        std::env::var("DISPLAY").as_deref(),
        Ok(":0" | ":1") | Err(_)
    ));
    let home = glib::home_dir();
    let rc = home.join(".bashrc");
    assert!(!rc.exists(), "requires a fresh isolated HOME");
    let rc_bytes = b"printf 'LOCAL_RC_STDOUT\\n'\nalias daily_alias='printf alias-ok'\ndaily_function() { printf function-ok; }\nexport PATH=/daily-test-bin:$PATH\nHISTFILE=$HOME/.daily_history\nPS1='LOCAL_PROMPT ' \n";
    fs::write(&rc, rc_bytes).unwrap();
    fs::write(home.join(".bash_profile"), "touch ~/profile-must-not-run").unwrap();
    let data = glib::user_data_dir();
    let mut design = Workspace::new(
        &termimochi_core::PtyxisPalette::from_text(include_str!(
            "../../resources/themes/fog-paper.palette"
        ))
        .unwrap(),
        termimochi_core::Variant::Light,
        Default::default(),
        Default::default(),
        Default::default(),
        None,
        true,
    );
    design.typography.family = "Liberation Mono".into();
    design.typography.size = 15.0;
    design.layout.columns = 100;
    design.layout.rows = 32;
    design.starship = Some(
        "format='CURRENT_PROMPT_MARKER $character'\n[character]\nsuccess_symbol='[>](green)'\n"
            .into(),
    );
    design.use_designer = false;
    let mut greeting = crate::pixel_trial::tests::fixture(true).0;
    let source = crate::greeting_image::animation::tests::chunked_fixture();
    greeting
        .import_artwork(
            source
                .image
                .decode()
                .unwrap()
                .convert(source.options().unwrap())
                .unwrap()
                .artwork,
        )
        .unwrap();
    greeting.editable_artwork = Some(source);
    greeting.presentation.visual = crate::greeting_output::Visual::Animation;
    greeting.presentation.columns = 24;
    greeting.imported_source =
        Some("{\"modules\":[{\"type\":\"os\",\"key\":\"GREETING_ONCE\"}]}".into());
    design.greeting = greeting;
    let mut plan = Plan::prepare(
        &data.join("termimochi/kitty-sessions"),
        "native-daily",
        "Daily Launcher Native",
        1,
        &design,
        Ownership {
            palette: true,
            typography: true,
            layout: true,
            prompt: true,
            greeting: true,
        },
        None,
        [10, 20],
    )
    .unwrap();
    // Only this isolated fixture enables remote control for evidence queries.
    // Production safe projection never enables these directives.
    plan.files.get_mut("kitty.conf").unwrap().extend_from_slice(
        format!(
            "\nallow_remote_control yes\nlinux_display_server x11\nlisten_on unix:{}\n",
            PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap())
                .join("daily-kitty")
                .display()
        )
        .as_bytes(),
    );
    let deployment = plan.publish().unwrap();
    let launcher = LauncherPlan::prepare(deployment.clone(), ShellMode::PersonalBash).unwrap();
    let installed = launcher.install().unwrap();
    let desktop = desktop_path(&data, "native-daily");
    assert!(
        Command::new("desktop-file-validate")
            .arg(&desktop)
            .status()
            .unwrap()
            .success()
    );
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/daily-launcher-test-driver.py");
    let args = serde_json::json!({
        "desktop": desktop,
        "background": design.palette().unwrap().variant(design.variant()).unwrap().get("Background").unwrap().to_hex().to_lowercase(),
        "driver": Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/preview-pointer-driver.py"),
    });
    let result = Command::new("/usr/bin/python3")
        .arg(script)
        .arg(args.to_string())
        .output()
        .unwrap();
    println!("{}", String::from_utf8_lossy(&result.stdout));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(&rc).unwrap(), rc_bytes);
    let script = installed.path.parent().unwrap().join("startup.bash");
    let before = fs::read(&script).unwrap();
    fs::write(&script, "external modification").unwrap();
    assert!(installed.open().unwrap_err().contains("startup changed"));
    fs::write(&script, before).unwrap();
    crate::kitty_session::restore(
        &data.join("termimochi/kitty-sessions"),
        "native-daily",
        &deployment.version,
    )
    .unwrap();
    assert!(installed.open().unwrap_err().contains("deactivated"));
    assert_eq!(Installed::list().unwrap().len(), 1);
    installed.restore().unwrap();
    assert!(!desktop.exists());
    println!(
        "Evidence: {}",
        glib::user_cache_dir()
            .join("daily-launcher-evidence")
            .display()
    );
}
