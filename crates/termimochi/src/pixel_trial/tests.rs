use super::*;
use crate::greeting_image::{
    self, Options, Style,
    source::{EditableArtwork, SourceImage},
};

pub(crate) fn fixture(animated: bool) -> (GreetingSettings, PixelImage) {
    let source = if animated {
        greeting_image::animation::tests::fixture()
    } else {
        let mut pixels = image::RgbaImage::from_pixel(48, 32, image::Rgba([0; 4]));
        for y in 4..28 {
            for x in 8..40 {
                pixels.put_pixel(
                    x,
                    y,
                    image::Rgba(if x < 24 {
                        [220, 45, 70, 255]
                    } else {
                        [20, 150, 220, 255]
                    }),
                );
            }
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(pixels)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        EditableArtwork::new(
            SourceImage::new(bytes.into_inner()).unwrap(),
            Options {
                columns: 24,
                style: Style::Detail,
                cell_ratio: 0.5,
                foreground: [255; 3],
                background: [0; 3],
                invert: false,
                edits: Default::default(),
            },
        )
        .unwrap()
    };
    let mut settings = GreetingSettings {
        enabled: true,
        ..Default::default()
    };
    settings
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
    settings.editable_artwork = Some(source.clone());
    (settings, pixel_export::prepare(&source).unwrap())
}

pub(crate) fn options(protocol: Protocol, ansi: bool) -> TrialOptions {
    TrialOptions {
        protocol,
        terminal: Terminal::Kitty,
        ansi,
        columns: 24,
        cells: [10, 20],
    }
}

pub(crate) fn confirm(trial: &Trial) {
    let asset = if trial.ansi {
        "config-ansi.jsonc"
    } else {
        trial.protocol.source_name()
    };
    let assessment = Assessment {
        done: true,
        visual: Visual::Confirmed,
        asset_hash: Some(hash(&read(&trial.directory.join(asset)).unwrap())),
        config_hash: Some(hash(
            &read(&trial.directory.join(if trial.ansi {
                "config-ansi.jsonc"
            } else {
                "config.jsonc"
            }))
            .unwrap(),
        )),
        ..Default::default()
    };
    typography_preset::write_private(
        &trial.directory.join("result.json"),
        &serde_json::to_vec(&assessment).unwrap(),
    )
    .unwrap();
}

#[test]
fn query_ack_does_not_verify_animation_and_timeouts_remain_unknown() {
    let response = runner::parse_response(b"\x1b_Gi=19;OK\x1b\\\x1b[?62;4;22c\x1b[6;22;11t", 19);
    assert_eq!(response.kitty, Support::Advertised);
    assert_eq!(response.sixel, Support::Advertised);
    assert_eq!(response.cells, Some([11, 22]));
    assert_eq!(response.animation, Visual::Unverified);
    assert_eq!(response.visual, Visual::Unverified);
    assert_eq!(runner::parse_response(b"", 19).kitty, Support::Unverified);
    assert_eq!(
        runner::parse_response(b"\x1b[?65;1;22c", 19).kitty,
        Support::Unavailable
    );
    assert_eq!(
        runner::parse_response(b"\x1b[?4;6c", 19).sixel,
        Support::Unverified
    ); // VT132 ID != Sixel feature.
    assert_eq!(
        runner::parse_response(b"\x1b_Gi=29;OK\x1b\\", 19).kitty,
        Support::Unverified
    );
    assert_eq!(
        runner::parse_response(b"\x1b_Gi=19;ENOTSUP\x1b\\", 19).kitty,
        Support::Unavailable
    );
    assert_eq!(runner::parse_response(b"\x1b[6;99999;0t", 19).cells, None);
}

#[test]
fn trial_is_temporary_absolute_and_cancellation_never_touches_daily_config() {
    let root = tempfile::tempdir().unwrap();
    let (settings, image) = fixture(false);
    let daily = root.path().join("config.jsonc");
    std::fs::write(&daily, "// daily\n{}").unwrap();
    let trial = Trial::prepare(
        root.path(),
        &settings,
        &image,
        options(Protocol::Kitty, false),
    )
    .unwrap();
    validate_directory(&trial.directory).unwrap();
    let config = crate::fastfetch_document::value(
        &String::from_utf8(read(&trial.directory.join("config.jsonc")).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        config["logo"]["source"],
        trial.directory.join("logo.png").to_str().unwrap()
    );
    assert!(trial.directory.join("config-ansi.jsonc").is_file());
    let path = trial.directory.clone();
    drop(trial);
    assert!(!path.exists());
    assert_eq!(std::fs::read_to_string(daily).unwrap(), "// daily\n{}");
}

#[test]
fn safe_trial_never_runs_imported_commands_or_external_logo_files() {
    let root = tempfile::tempdir().unwrap();
    let (mut settings, image) = fixture(false);
    settings.imported_source = Some(
        r#"{"modules":[{"type":"command","text":"touch NEVER"}],"display":{"pipe":true}}"#.into(),
    );
    settings.gap = 7;
    let trial = Trial::prepare(
        root.path(),
        &settings,
        &image,
        options(Protocol::Kitty, false),
    )
    .unwrap();
    let source = String::from_utf8(read(&trial.directory.join("config.jsonc")).unwrap()).unwrap();
    let safe = runner::safe_config(&source, &trial.directory, Protocol::Kitty, false).unwrap();
    assert_eq!(
        crate::fastfetch_document::value(&source).unwrap()["display"]["pipe"],
        false
    );
    assert_eq!(
        crate::fastfetch_document::value(&safe).unwrap()["logo"]["padding"]["right"],
        7
    );
    assert!(!safe.contains("NEVER"));
    assert!(!safe.contains("command"));
    assert!(!safe.contains("pipe"));
    assert!(safe.contains("TermiMochi"));
    assert!(
        runner::safe_config(
            r#"{"logo":{"type":"raw","source":"/etc/passwd"}}"#,
            root.path(),
            Protocol::Sixel,
            false
        )
        .is_err()
    );
    assert!(
        runner::safe_config(
            r#"{"logo":{"type":"file-raw","source":"/etc/passwd"}}"#,
            root.path(),
            Protocol::Kitty,
            true
        )
        .is_err()
    );
    let safe = runner::safe_config(
        "{\"logo\":{\"type\":\"data-raw\",\"source\":\"x\\u001b]52;c;AA==\\u0007\"}}",
        root.path(),
        Protocol::Kitty,
        true,
    )
    .unwrap();
    assert!(!safe.contains("52;c"));
}

#[test]
fn install_requires_visual_confirmation_and_rejects_changed_assets() {
    let root = tempfile::tempdir().unwrap();
    let (settings, image) = fixture(false);
    let trial = Trial::prepare(
        root.path(),
        &settings,
        &image,
        options(Protocol::Kitty, false),
    )
    .unwrap();
    let target = fastfetch_apply::Target::open(root.path().join("config.jsonc")).unwrap();
    let managed = root.path().join("managed");
    assert!(trial.install_plan(&managed, target.clone()).is_err());
    assert!(!managed.exists());
    confirm(&trial);
    let plan = trial.install_plan(&managed, target.clone()).unwrap();
    assert!(!plan.directory.exists()); // Reviewing/cancelling never installs assets.
    let config_path = trial.directory.join("config.jsonc");
    let before = read(&config_path).unwrap();
    std::fs::write(&config_path, b"{}").unwrap();
    assert!(trial.install_plan(&managed, target.clone()).is_err());
    std::fs::write(&config_path, before).unwrap();
    std::fs::write(trial.directory.join("logo.png"), b"changed").unwrap();
    assert!(trial.install_plan(&managed, target).is_err());
    assert!(!managed.exists());
}

#[test]
fn managed_install_survives_trial_cleanup_and_restores_exact_config() {
    let root = tempfile::tempdir().unwrap();
    let (settings, image) = fixture(false);
    let trial = Trial::prepare(
        root.path(),
        &settings,
        &image,
        options(Protocol::Sixel, false),
    )
    .unwrap();
    confirm(&trial);
    let daily = root.path().join("config.jsonc");
    let original = "// preserve comments\n{\"modules\":[\"os\"]}";
    std::fs::write(&daily, original).unwrap();
    let plan = trial
        .install_plan(
            &root.path().join("managed"),
            fastfetch_apply::Target::open(daily.clone()).unwrap(),
        )
        .unwrap();
    let assets = plan.directory.clone();
    let state = root.path().join("state");
    let installed = plan.apply(&state).unwrap();
    drop(trial);
    let config = crate::fastfetch_document::value(&installed.source().unwrap()).unwrap();
    assert_eq!(
        config["logo"]["source"],
        assets.join("logo.sixel").to_str().unwrap()
    );
    assert!(assets.join("logo.sixel").is_file());
    assert!(assets.join("config-ansi.jsonc").is_file());
    assert_eq!(assets.metadata().unwrap().mode() & 0o777, 0o700);
    assert_eq!(
        assets.join("logo.sixel").metadata().unwrap().mode() & 0o777,
        0o600
    );
    fastfetch_apply::restore(&fastfetch_apply::prepare_restore(&state).unwrap(), &state).unwrap();
    assert_eq!(std::fs::read_to_string(&daily).unwrap(), original);
    assert!(assets.join("logo.sixel").exists()); // Backups can still reference it.
}

#[test]
fn late_config_conflict_blocks_install_before_creating_assets() {
    let root = tempfile::tempdir().unwrap();
    let (settings, image) = fixture(false);
    let trial = Trial::prepare(
        root.path(),
        &settings,
        &image,
        options(Protocol::Kitty, false),
    )
    .unwrap();
    confirm(&trial);
    let target = fastfetch_apply::Target::open(root.path().join("config.jsonc")).unwrap();
    let plan = trial
        .install_plan(&root.path().join("managed"), target.clone())
        .unwrap();
    let directory = plan.directory.clone();
    std::fs::write(&target.path, "// external\n{}").unwrap();
    assert!(plan.apply(&root.path().join("state")).is_err());
    assert!(!directory.exists());
    assert_eq!(
        std::fs::read_to_string(target.path).unwrap(),
        "// external\n{}"
    );
}

#[test]
fn ansi_fallback_does_not_require_sixel_generation_and_can_be_installed() {
    let root = tempfile::tempdir().unwrap();
    let (settings, image) = fixture(false);
    let trial = Trial::prepare(
        root.path(),
        &settings,
        &image,
        TrialOptions {
            cells: [256, 256],
            ..options(Protocol::Sixel, true)
        },
    )
    .unwrap();
    assert!(!trial.directory.join("logo.sixel").exists());
    confirm(&trial);
    let target = fastfetch_apply::Target::open(root.path().join("config.jsonc")).unwrap();
    let installed = trial
        .install_plan(&root.path().join("managed"), target)
        .unwrap()
        .apply(&root.path().join("state"))
        .unwrap();
    assert_eq!(
        installed.source().unwrap(),
        settings.fastfetch_config().unwrap()
    );
}

#[test]
fn target_launch_uses_fixed_argv_and_no_shell_or_cwd_dependency() {
    for terminal in Terminal::ALL {
        let command = terminal.command(
            Path::new("/usr/bin/terminal"),
            Path::new("/tmp/helper 'literal'"),
            Path::new("/tmp/trial $(literal)"),
        );
        let args = command.get_args().collect::<Vec<_>>();
        assert_eq!(args[args.len() - 3], "/tmp/helper 'literal'");
        assert_eq!(args[args.len() - 2], WORKER_ARG);
        assert_eq!(args[args.len() - 1], "/tmp/trial $(literal)");
        assert!(!args.contains(&std::ffi::OsStr::new("bash")));
        assert_eq!(command.get_current_dir(), Some(Path::new("/")));
    }
}

#[test]
#[ignore = "requires dedicated X11, real Kitty/Ptyxis/Sixel xterm and trial helper; visual output and managed cwd independence"]
fn real_terminal_trial_capabilities_visual_confirmation_and_managed_install() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    let root = tempfile::tempdir().unwrap();
    let run = |path: &Path, terminal: &str, mode: &str| {
        let output = Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/test-pixel-trial.py"
            ))
            .arg(path)
            .arg(terminal)
            .arg(mode)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{terminal} {mode}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        println!("{}", String::from_utf8_lossy(&output.stdout));
    };
    for (terminal, protocol, ansi, mode) in [
        ("kitty", Protocol::Kitty, false, "static"),
        ("kitty", Protocol::Kitty, false, "rejected"),
        ("kitty", Protocol::KittyAnimation, false, "animation"),
        ("xterm", Protocol::Sixel, false, "static"),
        ("kitty", Protocol::Sixel, false, "unverified"),
        ("ptyxis", Protocol::Kitty, false, "unsupported"),
        ("ptyxis", Protocol::Kitty, true, "ansi"),
    ] {
        let (settings, image) = fixture(protocol == Protocol::KittyAnimation);
        let trial =
            Trial::prepare(root.path(), &settings, &image, options(protocol, ansi)).unwrap();
        run(&trial.directory, terminal, mode);
        let target = fastfetch_apply::Target::open(root.path().join("config.jsonc")).unwrap();
        if matches!(mode, "unsupported" | "unverified" | "rejected") {
            assert!(
                trial
                    .install_plan(&root.path().join("managed"), target)
                    .is_err()
            );
        } else {
            assert_eq!(trial.assessment().unwrap().visual, Visual::Confirmed);
            if terminal == "kitty" && protocol == Protocol::Kitty {
                let plan = trial
                    .install_plan(&root.path().join("managed"), target)
                    .unwrap();
                let directory = plan.directory.clone();
                plan.apply(&root.path().join("state")).unwrap();
                drop(trial);
                run(&directory, "kitty", "installed");
            }
        }
    }
}

#[test]
#[ignore = "requires dedicated X11 and Kitty; repeated immediate PNG/GIF startup without launcher sleeps"]
fn kitty_managed_images_survive_repeated_immediate_startup() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    let root = tempfile::tempdir().unwrap();
    for protocol in [Protocol::Kitty, Protocol::KittyAnimation] {
        let animated = protocol == Protocol::KittyAnimation;
        let (settings, image) = fixture(animated);
        let trial =
            Trial::prepare(root.path(), &settings, &image, options(protocol, false)).unwrap();
        confirm(&trial); // Only authorizes the isolated install fixture; pixels are checked below.
        let plan = trial
            .install_plan(
                &root.path().join("managed"),
                fastfetch_apply::Target::open(root.path().join("config.jsonc")).unwrap(),
            )
            .unwrap();
        let directory = plan.directory.clone();
        let config = crate::fastfetch_document::value(&plan.config).unwrap();
        assert!(
            config["general"]["preRun"]
                .as_str()
                .unwrap()
                .contains(startup::ARG)
        );
        plan.apply(&root.path().join("state")).unwrap();
        drop(trial);
        for iteration in 0..8 {
            let output = Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../scripts/test-pixel-trial.py"
                ))
                .arg(&directory)
                .arg("kitty")
                .arg(if animated {
                    "installed-animation"
                } else {
                    "installed"
                })
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{protocol:?} startup {iteration}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            println!("PASS {protocol:?} immediate startup {iteration}");
        }
    }
}

#[test]
#[ignore = "requires dedicated X11 and Kitty; multi-packet GIF motion in trial and managed installation"]
fn kitty_chunked_animation_moves_in_trial_and_managed_install() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    let root = tempfile::tempdir().unwrap();
    let source = greeting_image::animation::tests::chunked_fixture();
    let image = pixel_export::prepare(&source).unwrap();
    let mut settings = GreetingSettings {
        enabled: true,
        ..Default::default()
    };
    settings
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
    settings.editable_artwork = Some(source);
    let trial = Trial::prepare(
        root.path(),
        &settings,
        &image,
        options(Protocol::KittyAnimation, false),
    )
    .unwrap();
    let stream = std::fs::read_to_string(trial.directory.join("logo.kitty")).unwrap();
    assert!(
        stream.matches("m=1;").count() > 3,
        "each noisy frame must span packets"
    );
    let run = |path: &Path, mode: &str| {
        let output = Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/test-pixel-trial.py"
            ))
            .arg(path)
            .arg("kitty")
            .arg(mode)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        println!("{}", String::from_utf8_lossy(&output.stdout));
    };
    run(&trial.directory, "animation");
    assert_eq!(trial.assessment().unwrap().animation, Visual::Confirmed);
    let plan = trial
        .install_plan(
            &root.path().join("managed"),
            fastfetch_apply::Target::open(root.path().join("config.jsonc")).unwrap(),
        )
        .unwrap();
    let directory = plan.directory.clone();
    plan.apply(&root.path().join("state")).unwrap();
    drop(trial);
    run(&directory, "installed-animation");
}
