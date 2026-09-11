use super::*;

fn workspace() -> Workspace {
    Workspace::new(
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
    )
}
fn plan(root: &Path, id: &str, revision: u64) -> Plan {
    prepare(
        root,
        id,
        "中文 'quoted' project",
        revision,
        &workspace(),
        Ownership {
            palette: true,
            typography: true,
            layout: true,
            ..Default::default()
        },
        None,
        [8, 16],
    )
    .unwrap()
}

// Deterministic backend tests intentionally use a local executable identity,
// not a pretend real Kitty success. Actual target rendering is a separate
// ignored/native check, so CI without Kitty still verifies the write contract.
#[allow(clippy::too_many_arguments)]
fn prepare(
    root: &Path,
    id: &str,
    name: &str,
    revision: u64,
    workspace: &Workspace,
    owned: Ownership,
    native: Option<&str>,
    cells: [u32; 2],
) -> Result<Plan, String> {
    let executable = Executable::capture("/usr/bin/bash".into())?;
    Plan::prepare_inner(
        root,
        id,
        name,
        revision,
        workspace,
        owned,
        native,
        cells,
        Some(Dependencies {
            kitty: executable.clone(),
            bash: executable,
            helper: None,
            starship: None,
            fastfetch: None,
        }),
    )
}

#[test]
fn independent_entries_conflicts_repeat_open_and_restore_preserve_sentinels() {
    // Backend creation does not start Kitty. Presence of a target executable is
    // required for planning, independent from display-server availability.
    let root = tempfile::tempdir().unwrap();
    let sentinel = root.path().join("daily.rc");
    fs::write(&sentinel, "sentinel unchanged").unwrap();
    let managed = root.path().join("中文 'spaces' managed");
    let a = plan(&managed, "project-a", 1);
    let stale = plan(&managed, "project-a", 1);
    let first = a.publish().unwrap();
    assert!(stale.publish().is_err());
    let b = plan(&managed, "project-b", 1).publish().unwrap();
    assert_ne!(first.directory, b.directory);
    let second = plan(&managed, "project-a", 2).publish().unwrap();
    assert_eq!(list(&managed).unwrap().len(), 2);
    let reopened = current(&managed, "project-a").unwrap().unwrap();
    assert_eq!(reopened.version, second.version);
    let command = reopened.command().unwrap();
    let args: Vec<_> = command.get_args().map(|s| s.to_string_lossy()).collect();
    assert!(args.windows(2).any(|p| p == ["--config", "NONE"]));
    assert!(args.iter().any(|s| s == "--norc"));
    assert!(!args.iter().any(|s| s == "--hold" || s == "--rcfile"));
    assert!(!second.hashes.contains_key("starship.toml"));
    assert!(!second.hashes.contains_key("fastfetch.jsonc"));
    let restored = restore(&managed, "project-a", &second.version)
        .unwrap()
        .unwrap();
    assert_eq!(restored.version, first.version);
    assert_eq!(
        current(&managed, "project-b").unwrap().unwrap().version,
        b.version
    );
    assert!(second.directory.exists());
    assert!(first.directory.exists());
    assert_eq!(fs::read_to_string(sentinel).unwrap(), "sentinel unchanged");
    assert!(restore(&managed, "project-a", &second.version).is_err());
}

#[test]
fn tampered_artifact_blocks_open_and_recovery_without_overwriting() {
    let root = tempfile::tempdir().unwrap();
    let first = plan(root.path(), "a", 1).publish().unwrap();
    let second = plan(root.path(), "a", 2).publish().unwrap();
    fs::write(second.directory.join("kitty.conf"), "external change").unwrap();
    assert!(second.command().is_err());
    assert!(restore(root.path(), "a", &second.version).is_err());
    assert_eq!(
        fs::read_to_string(second.directory.join("kitty.conf")).unwrap(),
        "external change"
    );
    assert!(first.command().is_ok());
}

#[test]
fn first_publication_recovery_deactivates_without_removing_referenced_assets() {
    let root = tempfile::tempdir().unwrap();
    let first = plan(root.path(), "a", 1).publish().unwrap();
    assert!(restore(root.path(), "a", &first.version).unwrap().is_none());
    assert!(list(root.path()).unwrap().is_empty());
    assert!(first.command().is_ok());
    let second = plan(root.path(), "a", 2).publish().unwrap();
    assert!(second.previous.is_none());
    assert!(second.command().is_ok());
    assert!(first.directory.exists());
}

#[test]
fn one_corrupt_entry_does_not_hide_other_projects() {
    let root = tempfile::tempdir().unwrap();
    let broken = plan(root.path(), "broken", 1).publish().unwrap();
    let healthy = plan(root.path(), "healthy", 1).publish().unwrap();
    fs::write(broken.directory.join(MANIFEST), "broken manifest").unwrap();
    let (entries, issues) = list_with_issues(root.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, healthy.id);
    assert_eq!(issues.len(), 1);
    assert!(issues[0].starts_with("broken:"));
}

#[test]
fn replacement_review_names_the_previous_entry_and_freezes_its_manifest() {
    let root = tempfile::tempdir().unwrap();
    let first = plan(root.path(), "a", 1).publish().unwrap();
    let replacement = plan(root.path(), "a", 2);
    let review = replacement.review_text();
    assert!(review.contains(&format!(
        "Update existing independent entry: {}",
        first.name
    )));
    assert!(review.contains(&format!("Current version to replace: {}", first.version)));
    assert!(review.contains(root.path().join("a/current.json").to_str().unwrap()));
    assert!(review.contains(root.path().join("a/versions").to_str().unwrap()));
    let path = first.directory.join(MANIFEST);
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b'\n');
    fs::write(&path, bytes).unwrap();
    assert!(
        replacement
            .publish()
            .unwrap_err()
            .contains("manifest changed")
    );
    assert_eq!(
        current(root.path(), "a").unwrap().unwrap().version,
        first.version
    );
}

#[test]
fn imported_execution_is_retained_as_inert_source_not_approved_by_safe_trial() {
    let root = tempfile::tempdir().unwrap();
    let mut design = workspace();
    design.starship=Some("# preserve\nformat='$directory$custom$character'\n[custom.unsafe]\ncommand='touch should-never-run'\nwhen=true\n".into());
    design.use_designer = false;
    design.greeting.enabled = true;
    design.greeting.imported_source=Some("// preserve too\n{\"general\":{\"preRun\":\"touch pre-run-sentinel\"},\"modules\":[\"os\",{\"type\":\"command\",\"text\":\"touch command-sentinel\"},\"publicip\"]}".into());
    let plan = prepare(
        root.path(),
        "safe",
        "Safe",
        1,
        &design,
        Ownership {
            prompt: true,
            greeting: true,
            ..Default::default()
        },
        None,
        [8, 16],
    )
    .unwrap();
    let prompt = String::from_utf8_lossy(&plan.files["starship.toml"]);
    let greeting = String::from_utf8_lossy(&plan.files["fastfetch.jsonc"]);
    assert!(!prompt.contains("should-never-run"));
    assert!(
        !greeting.contains("pre-run-sentinel")
            && !greeting.contains("command-sentinel")
            && !greeting.contains("publicip")
    );
    assert_eq!(
        plan.files["starship.source.toml"],
        design.starship.unwrap().as_bytes()
    );
    assert!(
        String::from_utf8_lossy(&plan.files["fastfetch.source.jsonc"]).contains("pre-run-sentinel")
    );
    assert!(plan.notes.iter().any(|n| n.contains("custom command")));
    let deployed = plan.publish().unwrap();
    assert!(deployed.command().is_ok());
    for name in ["should-never-run", "pre-run-sentinel", "command-sentinel"] {
        assert!(!root.path().join(name).exists());
    }
}

#[test]
fn symlink_destination_is_rejected_and_unowned_references_do_not_leak() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("managed");
    std::os::unix::fs::symlink(root.path(), &destination).unwrap();
    assert!(
        prepare(
            &destination,
            "a",
            "a",
            1,
            &workspace(),
            Ownership::default(),
            None,
            [8, 16]
        )
        .is_err()
    );
    let ordinary = prepare(
        root.path(),
        "native",
        "Native",
        1,
        &workspace(),
        Ownership {
            typography: true,
            ..Default::default()
        },
        Some("font_size 14\nwatcher /sentinel.py\n"),
        [8, 16],
    )
    .unwrap();
    assert_eq!(ordinary.files.len(), 2);
    assert_eq!(
        String::from_utf8(ordinary.files["kitty.conf"].clone()).unwrap(),
        "font_size 14\nshell_integration disabled\nallow_remote_control no\n"
    );
    assert!(ordinary.review_text().contains("watcher /sentinel.py"));
    let trial = ordinary.temporary_trial(root.path()).unwrap();
    assert!(trial.deployment.command().is_ok());
    assert!(!root.path().join("native/current.json").exists());
}

#[test]
fn controlled_shell_ignores_user_rc_and_inherited_environment() {
    let root = tempfile::tempdir().unwrap();
    let deployed = plan(root.path(), "env", 1).publish().unwrap();
    let rc = root.path().join(".bashrc");
    fs::write(&rc, "printf RC_WAS_READ\n").unwrap();
    fs::write(
        root.path().join(".bash_profile"),
        "printf PROFILE_WAS_READ\n",
    )
    .unwrap();
    let command = deployed.command().unwrap();
    let env: BTreeMap<_, _> = command
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    assert!(!env.contains_key("BASH_ENV"));
    assert!(!env.contains_key("ENV"));
    assert!(env.keys().all(|k| !k.starts_with("BASH_FUNC_")));
    // Run the exact controlled Bash/hook contract non-GUI, with interactive
    // input. No inherited rc and a real second command prompt must survive.
    let mut shell = Command::new("/usr/bin/bash");
    clean_environment(&mut shell);
    shell
        .env("HOME", root.path())
        .env("PROMPT_COMMAND", env["PROMPT_COMMAND"].as_ref().unwrap());
    shell
        .args(["--noprofile", "--norc", "-i"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = shell.spawn().unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"printf 'CONTROLLED_OK\\n'\nprintf 'SECOND_COMMAND\\n'\nexit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("CONTROLLED_OK") && text.contains("SECOND_COMMAND"),
        "{output:?}"
    );
    assert!(!text.contains("RC_WAS_READ") && !text.contains("PROFILE_WAS_READ"));
}

#[test]
#[ignore = "dedicated X11, real Kitty/Fastfetch/Starship, Pillow/GStreamer; verifies live font/colors/prompt/GIF/input and durable reopen"]
fn controlled_kitty_real_gif_prompt_and_reopen() {
    assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
    assert!(!matches!(
        std::env::var("DISPLAY").as_deref(),
        Ok(":0" | ":1") | Err(_)
    ));
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    fs::create_dir(&home).unwrap();
    fs::write(
        home.join(".bashrc"),
        "printf RC_SHOULD_NOT_RUN\ntouch rc-sentinel\n",
    )
    .unwrap();
    fs::write(home.join(".bash_profile"), "touch profile-sentinel\n").unwrap();
    let mut design = workspace();
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
    let plan = Plan::prepare(
        &root.path().join("managed"),
        "native-a",
        "Native A",
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
    let trial = plan.temporary_trial(root.path()).unwrap();
    let deployment = plan.publish().unwrap();
    let capture_root = PathBuf::from(std::env::var_os("XDG_CACHE_HOME").unwrap())
        .join("controlled-kitty-evidence");
    fs::create_dir_all(&capture_root).unwrap();
    let driver =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/preview-pointer-driver.py");
    let expected_bg = design
        .palette()
        .unwrap()
        .variant(design.variant())
        .unwrap()
        .get("Background")
        .unwrap()
        .to_hex()
        .to_lowercase();
    for (phase, entry) in [("trial", &trial.deployment), ("published", &deployment)] {
        let command = entry.command().unwrap();
        let mut args: Vec<_> = command
            .get_args()
            .map(|v| v.to_string_lossy().into_owned())
            .collect();
        let socket = root.path().join(format!("{phase}.sock"));
        let insert = args.len() - 4;
        args.splice(
            insert..insert,
            [
                "--override".into(),
                "allow_remote_control=yes".into(),
                "--override".into(),
                "linux_display_server=x11".into(),
                "--listen-on".into(),
                format!("unix:{}", socket.display()),
                "--title".into(),
                "TermiMochi point-to-edit test".into(),
            ],
        );
        let mut env: BTreeMap<String, String> = command
            .get_envs()
            .filter_map(|(k, v)| {
                v.map(|v| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned(),
                    )
                })
            })
            .collect();
        env.insert("HOME".into(), home.to_string_lossy().into_owned());
        env.insert("LIBGL_ALWAYS_SOFTWARE".into(), "1".into());
        // Deliberately do not add hostile startup vars back to the already
        // cleaned production command; separate tests inspect their removal.
        let request = serde_json::json!({"program":command.get_program().to_string_lossy(),"args":args,"env":env,"socket":format!("unix:{}",socket.display()),"phase":phase,"capture":capture_root,"driver":driver,"background":expected_bg,"home":home});
        let python = r#"
import json,os,pathlib,subprocess,sys,time
from PIL import Image
r=json.loads(sys.argv[1]); folder=pathlib.Path(r['capture']); phase=r['phase']; logfile=folder/(phase+'.log')
def remote(*args):
    return subprocess.run([r['program'],'@','--to',r['socket'],*args],capture_output=True,text=True,timeout=5)
with logfile.open('w') as log:
    child=subprocess.Popen([r['program'],*r['args']],env=r['env'],cwd=r['home'],stdout=log,stderr=subprocess.STDOUT)
    try:
        deadline=time.monotonic()+20
        while True:
            assert child.poll() is None, logfile.read_text()
            text=remote('get-text','--extent','all')
            if text.returncode==0 and 'CURRENT_PROMPT_MARKER' in text.stdout: break
            assert time.monotonic()<deadline,(text.stdout,text.stderr,logfile.read_text())
            time.sleep(.1)
        assert text.stdout.count('GREETING_ONCE')==1,text.stdout
        assert 'RC_SHOULD_NOT_RUN' not in text.stdout
        colors=remote('get-colors'); assert colors.returncode==0,colors.stderr
        assert any(line.split()==['background',r['background']] for line in colors.stdout.lower().splitlines()),colors.stdout
        (folder/(phase+'-colors.txt')).write_text(colors.stdout)
        font=pathlib.Path(r['home'])/(phase+'-font.txt')
        # Only the known test shell on a dedicated display receives input.
        send=remote('send-text',"kitty +kitten query_terminal font_family font_size > "+str(font)+"; printf 'INTERACTIVE_INPUT_OK\\n'\n"); assert send.returncode==0,send.stderr
        deadline=time.monotonic()+10
        while time.monotonic()<deadline:
            text=remote('get-text','--extent','all').stdout
            if font.exists() and font.stat().st_size and 'INTERACTIVE_INPUT_OK' in text:break
            time.sleep(.1)
        font_text=font.read_text(); (folder/(phase+'-font.txt')).write_text(font_text)
        # Kitty reports the resolved face/PostScript name (LiberationMono), not
        # necessarily the user's spaced family label. Check the actual face and
        # numeric size separately instead of a brittle display-label substring.
        font_values=dict(line.split(':',1) for line in font_text.splitlines() if ':' in line)
        assert font_values['font_family'].replace(' ','').strip()=='LiberationMono',font_text
        assert float(font_values['font_size'].strip())==15.0,font_text
        assert text.count('GREETING_ONCE')==1,text
        assert text.count('CURRENT_PROMPT_MARKER')>=2,text
        (folder/(phase+'-text.txt')).write_text(text)
        positions=[]
        for i in range(6):
            shot=folder/(phase+'-'+str(i)+'.png')
            subprocess.run(['/usr/bin/python3',r['driver'],'capture','0','0'],env=dict(os.environ,TERMIMOCHI_INSPECT_SCREENSHOT=str(shot)),check=True,timeout=8,stdout=subprocess.DEVNULL)
            image=Image.open(shot).convert('RGB')
            red=[x for y in range(image.height) for x in range(image.width) if (p:=image.getpixel((x,y)))[0]>150 and p[1]<70 and p[2]<110]
            assert red, 'Missing actual GIF image'
            positions.append(min(red)); time.sleep(.07)
        assert len(set(positions))>1,('GIF did not move',positions)
        assert not (pathlib.Path(r['home'])/'rc-sentinel').exists()
        assert not (pathlib.Path(r['home'])/'profile-sentinel').exists()
        print('PASS',phase,'real palette, Liberation Mono 15, current Starship, one Greeting, interactive input, moving GIF',positions,flush=True)
    finally:
        child.terminate()
        try:child.wait(timeout=5)
        except subprocess.TimeoutExpired:child.kill();child.wait()
"#;
        let output = Command::new("/usr/bin/python3")
            .arg("-c")
            .arg(python)
            .arg(request.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{phase}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        println!("{}", String::from_utf8_lossy(&output.stdout));
    }
    drop(trial);
    assert!(
        current(&root.path().join("managed"), "native-a")
            .unwrap()
            .unwrap()
            .command()
            .is_ok()
    );
    println!("Evidence: {}", capture_root.display());
}
