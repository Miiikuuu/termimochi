# Compact theme application — 2026-09-13

Follow-up: the previously unverified Ptyxis Apply → actual Open → Restore
path now has [isolated Release/system-Ptyxis evidence](ptyxis-apply-acceptance.md).
The original test scope below is retained; daily systemd/Wayland remains unverified.

This patch simplifies the final application flow on `main` / `13faee83`, on
top of the existing uncommitted release-acceptance work. It does not replace
the theme model or preview architecture. No commit, push, daily installation,
default-terminal change or daily configuration/startup write was performed.

## Behavior and retained safety

| Entry | Normal flow | Retained advanced behavior |
| --- | --- | --- |
| Kitty theme | Use Theme → review owned modules → real Try in Kitty → one explicit visual confirmation → Apply & Open in Kitty | Exact generated files, safe projection details, app-menu launcher/environment choice, version recovery |
| Ptyxis theme | Use Theme → review supported owned settings, preselected → Apply & Open Profile when appearance/profile supports it | Component deselection, install-only palette, file destinations/diffs, existing result/recovery actions |
| Legacy single-item/advanced project | Existing adapter and scope checks; legacy advanced project remains opt-in | Native imports/exports, per-item application and source preservation remain |

Default selection uses the existing scoped plan: a font-size-only theme does
not apply reference palette, layout, Prompt or Greeting. Unavailable effects,
global font effects, shared files and export-only/not-enabled outcomes remain
visible before confirmation. Save still only saves the design.

Kitty still requires a real temporary trial. The single checkbox explicitly
mentions a moving GIF for animated themes; process launch is not visual or
animation verification. Publication rechecks document identity and snapshot.
Automatic opening occurs only for the still-current, visible confirmed review.
Opening runs in a background worker, preserves conflict checks and reports
publication success separately from opening failure. Retry Open does not
publish another version. It does not automatically create a desktop launcher.

Application records now retain their target adapter (`serde(default)` keeps
older records readable). Reopening a record never borrows another editor's
current target. Old target-less records still ask explicitly. Executing an
exact imported Fastfetch file, including preRun/command/network modules, still
needs the separate existing review and confirmation in Advanced.

## Evidence scope

- Build gates: [10/10](../target/qa/native-preview/build-checks-96lbbd82/results.json),
  including both strict Clippy checks, both workspace suites (420 ordinary
  tests each), both Release builds, private installer and desktop metadata.
- Optimized workspace suite: [420 passed](../target/qa/release-acceptance/compact-apply-release-tests.log),
  89 ignored GUI tests are not counted as passes.
- Actual final standalone Release: [passed](../target/qa/release-acceptance/blackbox-hesro7rw/result.json).
  Real AT-SPI/XTest navigation, font edit, Ctrl+S, different-process reopen,
  visible external-save conflict, exact design comparison and protected-byte
  comparison; memory GSettings isolation. This black-box does **not** Apply.
- Expanded optimized GTK matrix: [22/24](../target/qa/release-acceptance/termimochi-regression-l19ta4pu/results.json),
  12 cases at 1×/2×, no missing fixtures. The two image-import failures were
  stale target-dropdown expectations; the corrected [2/2 rerun passed](../target/qa/release-acceptance/termimochi-regression-q3uy3fio/results.json).
  This is 22 initial passing cases plus two corrected retests, not a claim of
  a single final 24/24 run. The retest pins the final production worker hash.
  Optimized GTK tests use controller access; they are not falsely described
  as pointer-only black-box application tests.
- Enabled native execution: [4/4](../target/qa/native-preview/termimochi-regression-native-wkwxhy8g/results.json),
  real Kitty and private patched Ptyxis embedding at both scales. This is a
  development harness with `native-preview` enabled, not an optimized native
  application matrix. It does not certify stock Ptyxis session isolation.

Default Release executable: `target/release/termimochi`, SHA256
`e791f6ac4a7e0072ff604a3ac0ebed955456fa45ac5437334d91c60bcb121fb9`.
The standalone black-box uses an immutable copy with this same hash.
Both strict Clippy modes and the optimized workspace suite were rerun after
the final test corrections. The earlier dual-build gates and 24-case matrix
precede those test-only corrections; their original build hashes are retained,
not presented as one identical final executable.

### Native screenshots

- [Compact Kitty review](../target/qa/release-acceptance/termimochi-regression-l19ta4pu/1x-window-typed_kitty-tests-typed_kitty_compact_apply_opens_real_target_and_retries/cache/compact-kitty-review.png).
- [Actually applied Kitty window](../target/qa/release-acceptance/termimochi-regression-l19ta4pu/1x-window-typed_kitty-tests-typed_kitty_compact_apply_opens_real_target_and_retries/cache/compact-kitty-applied.png).
- [Owned-font-only Ptyxis review](../target/qa/release-acceptance/termimochi-regression-l19ta4pu/1x-window-scheme-tests-scheme_theme_compact_review_selects_only_owned_settings/cache/theme-compact-review.png).
- [Final standalone Release saved/reopened](../target/qa/release-acceptance/blackbox-hesro7rw/reopened.png).

The new Kitty test opens actual Kitty, checks reviewed background pixels,
closes its private trial with XTest Ctrl+D, applies/opens, deliberately tampers
with the private published configuration and confirms opening is blocked,
then restores the test damage and retries without another version. Startup
sentinels and absence of automatic app-menu installation are asserted.
The merged GIF consent is checked by the existing GIF vertical workflow;
programmatically setting consent is not claimed to be human approval.

The Ptyxis compact test uses memory GSettings and checks before/cancel/apply/
restore values, scope and document equality. Its production profile-opening
function is suppressed under `cfg(test)`; this is **not** evidence that a real
Ptyxis profile was opened after Apply. Real native embedding tests are a
separate scope and do not fill that gap.

### Failure ledger

- `rlnrfg3g`: 2/10; collapsed GTK Expander content was omitted by the test
  walker, and the new Kitty fixture had not been converted to a theme. Fixed
  test introspection/fixture; no safety gate removed.
- `gzdv8_cn`: 8/10; new Ptyxis capture ran before GTK produced a render node.
  Wait for the next frame after the checkbox toggle.
- `meluv05g`: 18/20; old GIF workflow still expected two confirmation widgets.
  Updated to one and explicitly asserts the moving-GIF text.
- Initial black-box `8r_wfl8r` failed when AT-SPI returned a removed child as
  None. The traversal now skips vanished children; `mzykpmw2` passed on the
  same production binary. Application behavior was not changed for this.
- The expanded matrix initially found an old image-import test looking for
  another target dropdown. It now checks the retained Ptyxis target and keeps
  cancellation/explicit execution checks intact. Both 1×/2× retests passed in
  `q3uy3fio`; original failures remain recorded in `l19ta4pu`.

## Remaining / not claimed

- No human usability acceptance, daily Wayland/fractional-scale/IBus matrix,
  or real daily profile writes were performed.
- Real Ptyxis Apply → normal-profile open remains unverified here; the UI
  explicitly warns that its normal shell may run existing startup commands.
  It is not described as an isolated full-scheme trial.
- Existing native-preview limitations are not erased: stock Ptyxis private
  D-Bus/systemd environment restrictions and complex-Greeting clipping in
  native Kitty remain as documented in the prior release report.
- SVG import from an already running, replaced/deleted executable remains an
  outstanding helper-path bug, not fixed by this Apply simplification. Save,
  exit the old process and launch the current binary before retesting import.

## 中文手动验收卡

1. 保存旧窗口的工作并退出，运行 `./target/release/termimochi`。
2. 打开 Kitty 主题，点 **Use Theme…**。核对主题名、Kitty 和包含的设置；
   正常使用不需要展开 **Advanced**。
3. 点 **Try in Kitty**，在真实窗口看效果；有 GIF 时确认它确实在动。
   回到审阅页勾选唯一的效果确认，点 **Apply & Open in Kitty**。
4. 应自动打开应用后的独立 Kitty。普通 Kitty 图标和默认终端不改变。
   以后从独立方案库再打开；需要桌面入口时才展开 Advanced 创建。
5. 打开只改字号的 Ptyxis 主题：审阅只应应用 Typography，不能顺便
   应用参考配色/Prompt/Greeting。注意字体是 Ptyxis 全局设置。
6. 只想应用部分设置时展开 **Advanced · components & files**；取消后
   不应写配置。恢复仍在结果/方案入口的 Advanced 中。

手动应用会真实写入审阅的目标：只在你愿意应用时确认。自动化检查
使用隔离配置，未替你完成上述日常环境验收。
