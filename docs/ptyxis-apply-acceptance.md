# Ptyxis Apply → Open → Restore acceptance

2026-09-13 · main `13faee83` plus the preserved uncommitted compact-apply patch.
This follow-up adds executable native acceptance, not a new application adapter.
No production Rust changes, commits, pushes or daily configuration writes.

## Actual scope

`scripts/test-ptyxis-apply.py` launches an immutable copy of the actual Release
App and invokes its visible GTK actions through AT-SPI. The production
`open_profile()` executes **unmodified `/usr/bin/ptyxis` 50.1**, not a mock,
test-only Rust branch, patched private Ptyxis, or replacement terminal. The
profile runs an isolated real Bash with a small test-only probe in its private
rc file. The probe queries VTE's actual background via OSC 11 and records the
actual `PTYXIS_PROFILE` and PTY grid.

Isolation: owned Xvfb `:96`, private D-Bus, HOME, startup files and shared
keyfile GSettings (not per-process memory settings). Bubblewrap makes the host
filesystem read-only; only the evidence tree and private temporary files are
writable. It masks `systemd-run` within this mount namespace so stock Ptyxis
does not require a host user manager on the private bus. `/sys` is hidden to
avoid read-only AppArmor policy-query errors in the private D-Bus services.
No global service, policy, environment or binary is changed. This does **not**
certify the normal systemd launch path, daily dconf, Wayland or fractional scale.
All namespace descendants terminate when the test ends.

## Checks

1. Set two private profiles, with the deliberately different profile as default.
   Launch the reviewed profile and measure its native baseline.
2. Open Use Theme and Cancel: settings unchanged, no new shell.
3. Use Theme → Apply & Open Profile: the production App automatically opens
   the exact reviewed UUID, not the configured default. Real OSC background
   changes from `#B9D8F2` to `#E7E8E5`; global font changes from Liberation Mono
   12 to 20. Actual twenty-M ink bounds change from **197×11 to 316×17 pixels**.
   These are measured pixels, not screenshot similarity scores or config-only
   assertions. GTK4 VTE's AT-SPI geometry was unsuitable, so the test locates
   the actual native background and first text row in the captured window.
4. Restore through the GUI confirmation, then Open Profile Tab: same reviewed
   UUID, original background and **197×11** ink bounds. Both complete effective
   settings snapshots and original keyfile bytes match, preserving unset keys.
   The newly installed palette is removed by recovery; decoy profile and
   default-profile selection remain unchanged.
5. Remove the private reviewed profile from the profile list. Open Profile Tab
   reports that it no longer exists and opens no extra shell. Restore the
   test list and recheck the original settings bytes.
6. Private startup sentinels remain unchanged; no application-menu launcher.

The applied record has separate installation and activation receipts: its
install row remains `NotEnabled`, while the activation row is `Applied`.
This test requires activation success **and** checks the real terminal color;
it does not mistake successful file installation for successful activation.

Grid is recorded, not artificially held equal: the initial/applied tabs are
80×24. After font restoration, a tab in the existing larger window is 128×40.
The theme contains no layout override; font restoration does not resize an
already existing native window. Configuration dimensions remain unchanged.

## Results and evidence

- Initial complete positive flow: [passed](../target/qa/release-acceptance/ptyxis-apply-hpsiomrz/result.json).
- Extended flow with byte checks and missing-target refusal:
  [passed](../target/qa/release-acceptance/ptyxis-apply-lvszk2wn/result.json).
- Final screenshot-forwarding run:
  [passed](../target/qa/release-acceptance/ptyxis-apply-_binwhno/result.json).
  [Review](../target/qa/release-acceptance/ptyxis-apply-_binwhno/02-review.png),
  [actual applied Ptyxis](../target/qa/release-acceptance/ptyxis-apply-_binwhno/03-applied.png),
  [restored native window](../target/qa/release-acceptance/ptyxis-apply-_binwhno/05-restored.png),
  [visible missing-profile refusal](../target/qa/release-acceptance/ptyxis-apply-_binwhno/06-missing-profile-blocked.png),
  [before settings](../target/qa/release-acceptance/ptyxis-apply-_binwhno/keyfile-before.ini)
  and [byte-identical restored settings](../target/qa/release-acceptance/ptyxis-apply-_binwhno/keyfile-restored.ini).
- Python compilation and `git diff --check` pass. This turn does not recount
  the earlier 420 Rust tests as newly rerun; production Rust was unchanged.

Release SHA256: `e791f6ac4a7e0072ff604a3ac0ebed955456fa45ac5437334d91c60bcb121fb9`.
System Ptyxis SHA256: `d896539ba35478c4fc307be5f0fc2f3b2c6997114b90189810233e450d11d1f3`.

Earlier attempts exposed test-environment/harness issues: read-only `/tmp`,
AppArmor query access, unavailable/misreported VTE AT-SPI text geometry, and
an incorrect lowercase/combined receipt-status assumption. These failures are
retained in `target/qa/release-acceptance/ptyxis-apply-*`; they are not passes.

Known environment warnings are **not** erased: the stock terminal emits an
initial-allocation `VTE-CRITICAL: columns >= 1` under this WM-less Xvfb;
its shell then starts, renders and passes the measured flow. Private portal
FUSE/PipeWire services are unavailable. This is a functional-flow pass, not
an assertion of a warning-free desktop or a fix to those native warnings.

## Run / 中文验收卡

Run from the repository: `python3 scripts/test-ptyxis-apply.py`.
It requires existing system Ptyxis, bubblewrap, Python GI/AT-SPI/Pillow,
X11 tools and the existing private Xvfb. It does not install dependencies.
Evidence stays in `target/qa/release-acceptance/ptyxis-apply-*`.

日常手动验收（本轮未修改你的配置）：

1. 打开 Ptyxis 主题 → **Use Theme…**，核对显示的配置档案和全局影响。
2. 愿意实际应用时点 **Apply & Open Profile**，看自动打开的终端背景、字号。
3. 需要撤回时，在结果页点 **Restore This Application… → Restore Changes**。
4. 点 **Open Profile Tab** 检查恢复。字体是 Ptyxis 全局设置；默认终端和
   默认档案不应改变。既有窗口不会因为恢复字号而自动恢复物理窗口尺寸。

结论：原来缺失的 **Ptyxis 实际应用出口** 已有隔离 Release/native 证据；
日常 GNOME/systemd/Wayland 环境仍属于手动验证范围，不能以本测试代替。
