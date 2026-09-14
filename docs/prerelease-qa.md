# Installable preview / targeted launcher repair / field recovery

2026-09-14 · candidate **0.2.0-rc.1**, Ubuntu 26.04.1 x86_64, Rust **1.92.0**
(the CI toolchain, reused from the existing private toolchain directory).
Implementation based on main `eb4e24b`, with local changes. No commit, tag,
push, publication or daily installation is performed by this task.

## Functional changes

- `scripts/package-preview.py` builds an optimized default-feature archive with
  GUI/CLI binaries, icon/desktop data, installer, integrity manifest and build
  provenance. It is an unsigned dynamically linked Linux preview, not a portable
  AppImage, distro repository package or stable release. See [installation](prerelease-install.md).
- **Repair This Launcher…** identifies one managed entry by receipt/checksum,
  verifies its theme artifacts, reviews the active version and rebinds required
  executables. It reuses daily trial, acknowledgement, checked publication and
  rollback. Missing runtime/startup artifacts can be regenerated; a modified
  runtime gets a separate retained copy rather than being overwritten.
- Repair materializes an immutable session projection under the new launcher
  receipt, regenerating absolute asset/config/startup paths. It does not create
  a second editable theme or mutate the original published theme version.
- The launcher error page forwards the exact receipt/checksum into repair.
  **Open Independent Kitty Scheme…** also exposes repair. If an older retained
  runtime predates this UI, use the newly opened App's entry library instead.
- Altered/missing theme artifacts, receipt corruption, removed entries, inactive
  themes, unavailable dependencies and external desktop edits remain explicit
  blockers. Missing theme data needs reviewed re-publication from a saved theme;
  repair does not manufacture trust or silently fall back to ordinary Kitty.
- Ptyxis font, spacing, layout, palette selection and interface style recover
  only fields whose recorded before/after values differ. Unowned/reference
  values never enter the write set. Each safe field restores independently;
  external edits or locks are retained and reported. Removed profiles never
  redirect to the default profile.
- Completion is persisted per receipt fingerprint. Reopening/retrying partial
  recovery cannot overwrite fields already restored, even when their new value
  happens to equal the old applied value. Unset is restored with GSettings reset.
  Palette-file recovery checks actual profile use instead of being blocked by
  an unrelated interface-style conflict. Existing backups remain available.
- Every new application receipt has its own ID, including repeated applications
  of identical values through advanced font/layout tools. Old receipts without
  an ID remain readable. Completion from an older application cannot suppress
  recovery of a new one.

## Evidence

Default Release binary SHA256:
`f00fb3a2fdd09230ce865f344ac339204a91abe74cea1121592979795c79724e`.
The candidate archive is `target/preview-packages/termimochi-0.2.0-rc.1-linux-x86_64.tar.gz`;
its adjacent `.sha256` records local byte identity, not publisher authenticity.

| Check | Result / evidence |
| --- | --- |
| CI-toolchain default + enabled build boundaries | [10/10 passed](../target/qa/native-preview/build-checks-12lw1jlm/results.json): fmt, strict Clippy, workspace tests, Release, installer and metadata |
| Ordinary workspace tests | 431 passed per build: 395 App + 4 CLI + 5 integration + 27 core. 89 default / 101 enabled opt-in tests ignored, **not passes** |
| Exact default Release GTK / Kitty / Full regression | [4/4 passed](../target/qa/release-acceptance/termimochi-regression-hm9aq586/results.json): GUI trial gate and repair, actual daily GIF entry/repair/update/reopen/recovery, error-page route, Full composition/navigation, settings conflict recovery |
| Exact default Release + stock Ptyxis complete recovery | [Passed](../target/qa/release-acceptance/ptyxis-apply-iedayekk/result.json): reviewed UUID, native background/font, unchanged keyfile bytes after full restore, missing-profile refusal |
| Exact default Release + stock Ptyxis partial recovery | [Passed](../target/qa/release-acceptance/ptyxis-apply-7juw7yui/result.json): safe font/height restored, external width kept; restart App, reopen recovery, retry only remaining fields; later font/Dark edits kept |
| Actual enabled Release Kitty/Ptyxis interaction | [2/2 passed](../target/qa/native-preview/termimochi-regression-native-qqm8dkrf/results.json): real shell/tabs, Chinese input, clipboard, hot update and lifecycle at 1× |
| Crate packages | `cargo package --workspace --allow-dirty --locked` passed after the receipt-ID fix; raw log retained with the candidate checks |
| Installable archive | `test-preview-package.py` verifies the exact archive, private installation, CLI, desktop file, uninstall, sentinels and tamper refusal; final archive results are retained under `target/qa/preview-package/` |

Native measurements are not screenshot-similarity scores. Full restoration
returned twenty-M ink bounds **197×11 → 316×17 → 197×11 pixels**. In partial
recovery the deliberate later 20pt font stays **316×17**, while the external Dark
choice selects the restored palette's **#171C24** background. Neither is a failure
to restore an owned field: they are later/unowned values that must be kept.

### Native screenshot evidence

- [Partial field results](../target/qa/release-acceptance/ptyxis-apply-7juw7yui/07-partial-restore.png),
  [recovery menu after App restart](../target/qa/release-acceptance/ptyxis-apply-7juw7yui/07b-recovery-menu-after-restart.png),
  [actual reopened Ptyxis](../target/qa/release-acceptance/ptyxis-apply-7juw7yui/08-partial-retry-native.png).
- [Complete restoration](../target/qa/release-acceptance/ptyxis-apply-iedayekk/05-restored.png).
- [Real launcher error](../target/qa/release-acceptance/termimochi-regression-hm9aq586/1x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/repair-route/01-error.png),
  [same-theme repair review](../target/qa/release-acceptance/termimochi-regression-hm9aq586/1x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/repair-route/02-targeted-review.png).
- [Repaired Kitty GIF](../target/qa/release-acceptance/termimochi-regression-hm9aq586/1x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/daily-launcher-evidence/repaired/first-0.png).
  Motion was separately checked across eight frames on both first open and reopen;
  this one still image is not the animation proof.

Raw evidence paths are local, ignored build artifacts, not remotely published or
bundled desktop certification. The compiled release itself, not a debug-only
controller, handles GIO launcher/error dispatch and stock-Ptyxis application.

### Failed attempts and limits (not counted as passes)

- Initial compiler namespace error and a nested-format Clippy error were fixed.
  Rust 1.98 also flagged a pre-existing four-byte chunk idiom; it was mechanically
  updated without changing the caret test. Final gates use CI's Rust 1.92.0.
- `ptyxis-apply-mdlyso9z`, `81q7nk8j`, `raah5jao`, `2ztd8bvp` exposed selector
  issues: the menu button needed an accessible label, its proxy has no action,
  and stock GTK exposes anonymous GMenu rows. The driver now uses the actual
  toggle and an explicitly checked 16-row Palette menu plus screenshot evidence,
  not a hidden production action. This is a known accessibility/harness limit.
- `ptyxis-apply-_axnq0tl` incorrectly expected Light after preserving the user's
  Dark choice. The assertion now uses the actual palette's Dark value and
  measures light-on-dark glyph ink. Subsequent complete reruns passed.
- `termimochi-regression-lwbghuc_` rejected the new `repaired` test phase; the
  finite driver allowlist was extended. `tup9_7_1` successfully repaired/animated
  the theme but incorrectly expected undo to revive a deliberately deleted old
  runtime identity. Tests now retain the working repaired baseline for subsequent
  update/restore and separately verify undo returns the old (possibly broken) entry.
- Isolated X11 portal/PipeWire/systemd warnings remain visible. Stock Ptyxis can
  emit its known initial-allocation VTE warning; the functional passes do not
  certify a warning-free desktop. No production workaround changes daily services.
- Locked-setting handling exists per field; a real daily policy-lock scenario
  is not certified. Missing-field recovery is covered by an ordinary regression.
- No daily desktop-menu, user Conda/custom Bash, Wayland, multi-monitor,
  fractional-scale or external-user result is claimed by these isolated runs.

## 中文验收卡

| 操作 | 应看到什么 |
| --- | --- |
| 解压预发布包，运行 `bash scripts/check-preview.sh` | 文件校验、动态库检查；不是声称系统环境全部兼容 |
| 运行包内 `./target/release/termimochi` | 无需安装就能打开编辑器；Save 只保存设计 |
| 入口库 → 指定 Kitty 入口 → Repair This Launcher… | 明确同一个主题、目标及当前版本，显示修复范围 |
| Try Daily Session → 检查实际窗口 → 勾选 → Create / Update App Launcher | 试用前不能更新；只替换这一个受管理入口 |
| Remove / Restore App Launcher… → Undo Launcher Update | 回到之前的入口，保留素材；旧入口原本失效时仍可能失效 |
| Ptyxis 应用后，在系统设置中改其中一项，再 Restore This Application… | 未冲突项恢复，外部改动保留，结果列出每个受阻字段 |
| 再次撤回或重开恢复记录 | 已完成字段不再写入；不能将部分完成标为全部恢复 |

Do not deliberately delete daily runtime files or corrupt real configurations
to perform this card. Fault injection belongs in the isolated automated tests.

Real desktop menus, personal Conda/custom Bash, Wayland and multiple monitors
remain the [separate environment-validation task](desktop-environment-validation.md).
