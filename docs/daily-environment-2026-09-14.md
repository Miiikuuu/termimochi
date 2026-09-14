# Daily environment acceptance — 2026-09-14 (partial)

The user explicitly authorized installation of the candidate and execution of
their local Bash/Conda initialization. This is a real desktop check, distinct
from the earlier isolated X11 matrix. It is **not a full acceptance pass**.

## Candidate and installation

- Ubuntu 26.04.1, GNOME Wayland, Kitty 0.45.0, stock Ptyxis 50.1.
- External DP-3: 2560×1440, scale 1; built-in eDP-1: 2560×1440,
  scale approximately 1.3333. These are observed monitor settings, **not** a
  successful cross-monitor movement/IME/geometry test.
- Candidate: `termimochi-0.2.0-rc.1-linux-x86_64.tar.gz`.
- Archive SHA256: `e4e684361bc79dc8a24f7401b602bd47c4c5f7f7a461e8bc3b89d702320fc56b`.
- Installed GUI SHA256: `f00fb3a2fdd09230ce865f344ac339204a91abe74cea1121592979795c79724e`.
- Package integrity/dependency checks and installer dry run passed; installation
  under `~/.local` completed. Fifteen previous App files were copied before
  replacement. No source build, commit, push, publication or default-terminal
  change was performed.
- GIO's desktop application index finds the installed entry with the expected
  executable and icon. Actual GNOME overview search/click has **not** been
  verified. An initial `gio launch` invocation returned without a retained App
  process; direct installed-binary launch succeeded. Do not count the former as
  a successful menu launch.
- `termimochi-cli --version` is not a supported CLI command; it returns usage.
  Candidate identity was checked by hashes and package metadata, not that flag.

## Checks completed

| Check | Result and evidence boundary |
| --- | --- |
| Installed GTK App on daily Wayland | Opened; accessible native controls and Full scene present. No screenshot obtained. |
| Complex Kitty theme import and real trial | Opened a renamed/new-ID copy of the existing 小猫测试 fixture; original theme untouched. Use Theme → Try in Kitty launched the actual target with the owned colors and font size 16. Loaded Wayland client library observed. The user subsequently confirmed “显示正常，GIF 在动”. This is user-observed appearance/motion evidence, not an automated screenshot or pixel comparison. |
| Local Bash initialization | Interactive Bash loads the user's actual initialization; aliases and functions available; Conda is a function, Starship/Fastfetch resolve. |
| Conda base activation/deactivation | Both return 0; active base verified. Only the disposable process environment was changed by the probe. No other Conda environments, ROS/custom hooks or long-running workflows certified. |
| Stock Ptyxis normal session path | A new window in the existing daily Ptyxis instance ran the same personal-Bash probe, including successful Conda activate/deactivate. No private bus, patched binary or systemd masking. Test window closes after the finite probe. This does not verify App Apply/profile restoration. |
| Startup and terminal configuration files | Enumerated files retain their original hashes, including Bash startup, Kitty/Fastfetch/Starship and default-application files. |
| Desktop interface and Shell settings | Recorded subtree hashes unchanged. |
| Ptyxis settings | **Change attributed: only `window-size`, `(uint32 132, uint32 41)` → `(uint32 120, uint32 36)`.** A read-only reconstruction changing this field alone exactly matched the original complete subtree SHA256. This establishes that font, palette and profile settings did not change in the compared snapshot. The first search incorrectly tried pixel-pair serialization and did not match; the successful search used the actual typed terminal-grid serialization. No settings were written back. See `ptyxis-window-state-attribution.json`. |
| Previous App recovery preparation | All 15 previous files retained and restore dry run passed. Actual rollback has not been performed; the candidate remains installed. |

## Screenshot and interaction limits

Direct GNOME Screenshot D-Bus access was refused. The interactive screenshot
portal returned response code 2 (`InteractiveScreenshot didn't return a file`).
A non-interactive request displayed a permission for taking screenshots at any
time, broader than the intended single screenshot. No Allow action was taken.
The requesting process was stopped; a subsequent cross-connection Close call
timed out. The user was asked to choose Deny if the dialog remained visible.
No automated desktop screenshot or automated GIF-motion evidence was obtained
through those requests. Later the user supplied a single system screenshot;
the separately reported live GIF motion is user-observed evidence.

The user confirmed the trial appearance/GIF. The GTK check box exposes zero
AT-SPI actions on this Wayland session and programmatic focus also fails.
Attempting Apply while unchecked correctly returned false. The user then checked
and applied it themselves; no internal state or backend bypassed the GUI gate.

**Applied successfully:** test theme `5699a90c-88e6-43a6-9835-9a47d4135f49`,
version `version-mqaXyY`, with `current.json` pointing to that version and the
eight managed configuration/source/artwork files present. The result page reports
the independent entry ready and Kitty started. Publication evidence is retained
in `published-version-evidence.json`.

Advanced → Add / Update App Launcher → **My Bash Environment** → **Try Daily
Session** was then executed through the actual GUI. The live Kitty process uses
the generated private `launcher-trials/trial-zhmpaB/startup.bash`, and the review
initially reported successful process startup with **no app-menu entry yet written**.
See `daily-launcher-review-ui.json` and
`kitty-processes-after-personal-trial.json`.

### User check, installed-entry reopening and targeted repair

The user then reported completion and supplied
`Screenshot From 2026-09-14 16-22-52.png`, retained privately as
`target/qa/daily-environment/rc1-mg8C6nz8/user-personal-kitty.png`.
Visual inspection shows the artwork, full system-information block, terminal
font DejaVuSansMono 16, palette and Prompt together; both `conda activate base`
and `conda deactivate` are visible, followed by normal prompts with no visible
error. This supports basic personal-shell coexistence with the user's report;
the screenshot alone does not measure GIF motion or prove the Conda environment
variable changed (the separate probe verified that separately).

The actual GUI confirmed **Theme App Launcher** installed:
`TermiMochi RC1 验收 — Kitty`, receipt `version-xapJAE/launcher.json`, mode
`personal-bash`. The desktop file selects the retained runtime whose hash matches
the candidate. No normal Kitty entry was replaced.

All three test-editor dialogs/windows were closed normally. With no editor
process present, GIO parsed and launched the actual installed desktop entry twice
(`entry-first.json`, `entry-reopen.json`). Each produced a new Kitty that remained
alive through the observation interval and used the exact receipt's generated
startup. Only those two probe-created Kitty processes were closed afterwards;
the user's trial windows were not closed. **Pass: actual desktop Exec and
independence from the editor. Not yet verified: GNOME overview search/click or
visual output in those two automatically opened windows.** The probe initially
expected an underscore in the serialized shell mode and stopped before launch;
correcting its expectation to the actual `personal-bash` allowed both runs.

A bounded fault check temporarily moved only this test receipt's generated
`startup.bash` to `startup.bash.qa-held`. The actual desktop launcher refused to
run it and showed **Theme launcher needs attention**. Its **Repair This Launcher…**
button led to **Repair: TermiMochi RC1 验收 — Kitty · My Bash Environment**.
The desktop file was byte-identical before confirmation. A `finally` block
restored the original script, with matching SHA256, so no broken entry was left.
The theme manifest was unchanged. See `repair-route-result.json`,
`repair-error-ui.json` and `repair-targeted-ui.json` (native accessibility
records, not screenshots). The short-lived probe's child GUI did not remain
after the probe ended; the same receipt was reopened via the supported repair
argument in a retained process for the next interactive step. An AT-SPI cache
warning was observed and retained separately from the successful route check.

**Repair publication and Undo completed.** The user completed the repair trial
confirmation and installed `version-FvKbai/launcher.json`; the result page's
Open App Launcher action reported successful launch. A subsequent actual-desktop
Exec probe verified that this repaired receipt's startup was selected and Kitty
remained running (`entry-repaired.json`; editor still present in this particular
probe). The repaired receipt's saved previous desktop bytes were checked against
the known-working original before recovery (`repair-installed.json`).

Through the actual GUI, Remove / Restore App Launcher → Undo Launcher Update
then succeeded. The desktop file exactly matched the original SHA256
`67962a9ae33247aa7ea86848eabc0ad1ce3599bd775dae2cdf9a81f83a3557a6`.
Both test-editor windows were closed, and the restored desktop entry started
Kitty successfully without an editor (`entry-restored.json`).
`undo-repair-ui.json` records the native recovery result.

Final checks (`post-recovery-checks.json`) verify the installed candidate binary
and original published theme manifest unchanged, with terminal/startup/default
application files still matching their initial snapshots. Ptyxis still differs
only in the previously attributed remembered window size. No probe-created
terminal window was left running; previously user-operated trial windows were
not terminated. The installed test entry is intentionally left **usable at its
original launcher version `version-xapJAE`**; the repaired version and assets are
retained. Initial-entry removal and arbitrary theme-content update are not
claimed as tested by this repair-and-Undo cycle.

The repair host opens a blank
`Untitled Theme · Ptyxis` parent while the repair review correctly targets Kitty;
this is a potentially confusing UI detail, not a target-write mismatch, and is
recorded as a remaining usability finding.

## Evidence and Chinese continuation card

### Menu confirmation and next desktop checks

The user subsequently replied “没问题” after being asked to search GNOME's menu
for the test theme, open it and check appearance/GIF. Record this as **user-confirmed
real menu use**, not an automated menu click or an additional screenshot.

For the next round, the installed candidate reopened the same test document and
the actual installed Kitty desktop entry was launched in a transient user service
`termimochi-qa-ime-rc1.service` (`ExitType=cgroup`, collect on exit). This service
keeps the test window independent of the automation command's process lifetime;
it does not change global session environment or install a persistent unit.
The user's Fcitx5 reports `pinyin`; merely finding it active is not an IME pass.
`ime-multimonitor-baseline.json` records the unchanged saved document hash,
current two-monitor layout and native accessible UI before user input. Wayland
accessible screen positions are unavailable/zero, so they cannot certify actual
candidate-box positioning or cross-monitor geometry.

The user is asked to exercise real pinyin preedit/commit in both the App's sample
input and Kitty, copy/paste between them, and move both windows between the 100%
external and ~133% built-in display. Use a `#` comment prefix for Kitty test text
and do not press Enter, avoiding accidental local command execution. No clipboard
contents are read by the automation. The user subsequently replied “都没问题”
to the combined App/Kitty pinyin-input, bidirectional-clipboard and cross-screen
movement check. Record these as **user-confirmed passes on this desktop**.
No additional paired screenshots, candidate-box coordinates or automated input
injection evidence were supplied; do not relabel these as automated checks.

**Ptyxis daily Apply/recovery remains scope-blocked, not failed:** its
`use-system-font` and `font-name` writes are global even with a new profile
(`typography_apply::GLOBAL_KEYS`). A new profile alone does not isolate a full
application test from the user's existing terminals. The current authorization
explicitly preserves daily terminal configuration, so no such writes were made.
Earlier isolated restore tests and the normal daily shell-launch test remain
separate evidence. Logout/login is also not automated because it would terminate
the user's active desktop work.

Local evidence directory (not committed):
`target/qa/daily-environment/rc1-mg8C6nz8/`.

Contains installation logs, original configuration hashes, backup manifest and
previous App files, installed-file hashes, actual monitor details, GIO index
metadata, personal Bash/Ptyxis result files, GUI accessibility records and the
current configuration comparison. Startup output is private and not copied into
this report. The test copy is `daily-check.termimochi-design.json`.

1. 若截图授权框仍在，点击 **Deny／拒绝**；无需授予持续截图权限。
2. **已由用户确认显示正常、GIF 在动，并完成 Apply & Open。** 验收主题已保存
   为 Kitty 独立版本；原主题与普通 Kitty 入口不变。
3. **个人 Bash 试用与首次入口创建已由用户完成。** 截图已查看；关闭编辑器后的
   两次实际桌面入口执行通过，GNOME 菜单搜索点击仍需用户检查。
4. **定向修复、修复入口启动、Undo 和撤回后独立启动已通过；系统菜单搜索、
   打开及画面已由用户确认。** 当前保留的是可用的首次验收入口。
5. **已由用户确认通过：** App／Kitty 拼音输入、候选框及光标观察、双向复制
   粘贴、两屏往返移动及 GIF／缩放。没有额外两屏截图，不记为自动化几何检查。
   剩余：收藏、注销重登、其他机器、外部用户体验和 Ptyxis 日常全局 Apply／恢复。

如需恢复旧 App，先关闭候选版，检查备份并运行以下**只读预演**：

```sh
/usr/bin/python3 target/qa/daily-environment/rc1-mg8C6nz8/audit.py restore
```

只有明确决定回退才加 `--apply`。脚本先检查所有当前 App 文件仍匹配本次安装
以及备份哈希，遇到外部修改就停止；不会恢复或覆盖终端配置。尚未实际执行回退。
