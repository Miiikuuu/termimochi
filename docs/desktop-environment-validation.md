# Real desktop/environment validation — separate open task

Status: **partially exercised; full acceptance remains open**, not “feature absent”.
The user-authorized [2026-09-14 daily checks](daily-environment-2026-09-14.md)
installed the exact candidate, opened the real Wayland App/Kitty trial and ran
personal Bash/Conda successfully, including a normal stock-Ptyxis window.
The user subsequently confirmed the Kitty trial appearance and GIF motion,
completed personal-Bash trial/launcher creation and supplied a native screenshot.
Actual installed desktop Exec reopening twice without an editor, plus a bounded
missing-startup → error GUI → targeted-repair review check, also passed.
Repair publication, actual repaired-entry execution, GUI Undo and restored-entry
execution without an editor subsequently passed. The user then confirmed real
GNOME menu search/click, opening and GIF appearance. The user also confirmed
pinyin input, bidirectional clipboard use and movement between the 100% and
~133% displays in the App and Kitty. These are human observations on this
desktop, not automated IME/pixel tests or certification of other environments.
The Ptyxis subtree change was attributed by exact baseline-hash reconstruction
to saved `window-size` alone (132×41 → 120×36); font/palette/profile settings
were unchanged in the compared snapshot. No settings were written back.
This task remains open after
isolated X11 tests pass. It requires the user's desktop and shell setup, with
their explicit consent for any installation or local startup-code execution.
No external testers are currently available.

| Environment | Existing feature to exercise | Required evidence | Status |
| --- | --- | --- | --- |
| GNOME application menu | App/Kitty theme desktop entry, search and launch | Search by theme name, open twice after editor exit; favorite and logout/login reindex | Partial: indexing and actual desktop Exec/reopen/repair/Undo passed; overview search/click and appearance confirmed by user; favorites and logout/login pending |
| User's Conda / custom Bash | My Bash Environment, explicit daily-session trial | `conda` activation, aliases/functions, PATH, prompt coexistence, no duplicate Greeting; before/after startup-file hashes | Partial: actual local startup, basic Conda activate/deactivate, user trial/screenshot and unchanged startup files verified; other environments/custom hooks not certified |
| Daily Wayland session | App, native target, clipboard/IME, GIF | Session/terminal versions, actual windows, focus/input, live motion, resize/reopen | Passed for the exercised App/Kitty flow on this desktop: automated startup checks plus user-confirmed GIF, pinyin, clipboard and cross-screen behavior; not universal certification |
| Multiple monitors / fractional scale | Window movement and target rendering | Each monitor's scale, move windows between monitors, font/image geometry and input position | User-confirmed on the recorded 100% / ~133% display pair; no additional paired screenshots or automated geometry proof supplied |
| Native Ptyxis normal systemd launch | Actual reviewed profile open | UUID, appearance/font, shell, recovery; no private-bus systemd masking | Partial: stock normal window and personal Bash/Conda probe passed; daily App Apply/profile recovery not tested |
| Other machines/distributions | Installable preview and dynamic dependencies | Package SHA, OS/library versions, installation/uninstallation and launch | Not verified |
| External usability | Theme → trial → use → reopen → recovery | Human-observed outcome without developer coaching | Not verified |

Run with the exact candidate archive/binary SHA from the release record. Retain
failed observations as failures, environmental prerequisites as blockers, and
unattempted cases as unverified. A launched PID, exported config or screenshot
does not prove GIF motion, menu indexing, shell compatibility or successful use.

The isolated stock-Ptyxis harness masks `systemd-run` only inside its namespace;
the experimental embedded-Ptyxis harness uses a private patched build. Neither
certifies the daily systemd/Wayland path. These are distinct evidence boundaries,
not reasons to describe the supported application flow as nonexistent.

中文执行卡（需要用户同意后才实际操作）：

1. 记录安装包校验和、系统版本、终端版本、会话类型和显示器缩放。
2. 显式安装候选版；审阅并创建一个测试主题入口，不更换默认终端。
3. 从真实桌面菜单搜索并打开两次；关闭 App 后再次打开，记录结果。
4. 只有选择并同意 **My Bash Environment** 后才测试个人 Conda／Bash。
5. 检查中文输入、复制粘贴、GIF 动作和跨显示器移动，记录失败或限制。
6. 撤回测试入口，核对日常配置／启动文件前后哈希；记录未完成项目。

Do not close this task using the [isolated prerelease checks](prerelease-qa.md).
