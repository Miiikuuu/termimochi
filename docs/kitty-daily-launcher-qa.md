# 日常 Kitty 入口：本地验收记录

本轮基于 `main` 的 `e3d12dd` 工作区实施；包含此前未提交的窗口颜色隔离修复，
保留用户任务书。测试在 2026-09-12 本地会话执行。未 commit / push / 安装到日常环境。

## 自动化结果

| 检查 | 本轮结果 | 本机原始记录 |
| --- | --- | --- |
| Rust 1.92.0 fmt | 通过 | `target/qa/theme-workspace/daily-fmt.log` |
| Clippy，workspace / all-targets / locked / -D warnings | 通过 | `daily-clippy.log` |
| Workspace tests | **401 passed，0 failed；70 native ignored** | `daily-tests.log` |
| Release workspace build | 通过 | `daily-release.log` |
| Release 隔离原生启动 | 10 秒存活，预期 timeout 124，无 fatal / panic | `daily-smoke.log`、`smoke-78MKsYoq/gui.log` |
| 隔离安装 / 卸载 | 通过；只处理独立临时前缀 | `daily-installer.log` |
| 桌面资源测试 | **9/9** | `daily-resources-tests.log` |
| Desktop / AppStream / 嵌入资源 / 打包副本 | 通过，17 个嵌入资源 | `daily-resources.log` |
| 新生成主题 `.desktop` | 原生案例中 `desktop-file-validate` 通过，实际 GIO 启动成功 | 下述矩阵 |

以上省略前缀的日志都位于 `target/qa/theme-workspace/`。
本轮未重跑线上 CI、RustSec 网络审计、全部 70 个原生案例或完整 Cargo 打包矩阵。
常规 Rust 测试中忽略的原生案例不算通过。

Release：`target/release/termimochi`。
SHA-256：`078ce583de5ec4f58c78fdfa3a9c458c1629117c9ab85cae6ca201eb6456f6af`。

## 隔离原生矩阵：10/10

私有 HOME、XDG 数据 / 配置 / 缓存 / 状态、短路径 runtime、独立 D-Bus、memory GSettings、
专用 Xvfb `:96`（2560×1800）；每个案例独立进程，1× / 2× 各一遍。

| 案例 | 1× | 2× |
| --- | --- | --- |
| 实际 `.desktop` → 留存生产程序 → Kitty；个人 Bash + 字体 / 配色 / 当前 Prompt / GIF；关闭再打开 | 通过 | 通过 |
| 原有 controlled Kitty 临时试用 / 发布 / 字体 / 配色 / Prompt / GIF / 重开 | 通过 | 通过 |
| 原生 GUI 环境选择 / 禁止绕过试用确认 / 创建 / 结果页 / 停用后的库内撤回 | 通过 | 通过 |
| 多窗口预览颜色独立、切场景、关闭重开 | 通过 | 通过 |
| 原有 Use Theme 审阅 / 未验证发布与过期试用防护 | 通过 | 通过 |

主记录：
[`results.json`](../target/qa/theme-workspace/termimochi-regression-32q68dck/results.json)，
[`构建哈希`](../target/qa/theme-workspace/termimochi-regression-32q68dck/build.json)，
[`矩阵日志`](../target/qa/theme-workspace/daily-native-acceptance.log)。

新的真实启动案例通过 GIO 解析并启动**实际生成的桌面文件**，没有用拼装的近似命令代替。
启动时没有编辑器运行；关闭后再次启动。测试读取 Kitty 实际背景、字体字号、终端文本，
验证 alias、函数、PATH、HISTFILE、Starship 路径，逐帧检查测试 GIF 中红块位置变化。
测试素材是噪点背景加移动红块，专门用于判断是否播放，不是主题 Logo 预设。
只在隔离 fixture 中添加 Kitty remote-control 配置进行观测；生产安全配置不因此开启它。

GUI 自动勾选用于验证流程门槛，不等于真实用户确认外观或可用性。
额外后端覆盖：重复发布与撤回、外部文件冲突、审阅后 `.bashrc` 变化、过期主题、
符号链接、收据 / 脚本篡改、参数转义、未拥有 Prompt 的保持、运行程序去重与原编辑器移除。

### 原生截图

- [1× 创建前的审阅与禁用状态](../target/qa/theme-workspace/termimochi-regression-32q68dck/1x-window-daily_launcher-tests-launcher_gui_review_install_and_deactivated_recovery/cache/daily-launcher-review.png)
- [1× 创建成功及菜单搜索名称](../target/qa/theme-workspace/termimochi-regression-32q68dck/1x-window-daily_launcher-tests-launcher_gui_review_install_and_deactivated_recovery/cache/daily-launcher-installed.png)
- [1× 停用后仍可撤回的入口库](../target/qa/theme-workspace/termimochi-regression-32q68dck/1x-window-daily_launcher-tests-launcher_gui_review_install_and_deactivated_recovery/cache/daily-launcher-deactivated-library.png)
- [1× 实际桌面文件启动 Kitty](../target/qa/theme-workspace/termimochi-regression-32q68dck/1x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/daily-launcher-evidence/first-0.png)
- [1× 关闭后重开](../target/qa/theme-workspace/termimochi-regression-32q68dck/1x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/daily-launcher-evidence/reopen-0.png)
- [2× 创建成功](../target/qa/theme-workspace/termimochi-regression-32q68dck/2x-window-daily_launcher-tests-launcher_gui_review_install_and_deactivated_recovery/cache/daily-launcher-installed.png)
- [2× 实际桌面文件启动 Kitty](../target/qa/theme-workspace/termimochi-regression-32q68dck/2x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/daily-launcher-evidence/first-0.png)

同目录有 8 帧截图和实际颜色 / 字体 / shell / 终端文本。静态截图本身不能证明动画，
动画通过结论来自逐帧像素位置断言；该结果针对测试素材，不代表所有 GIF 都已兼容。

## 初轮失败与修正

没有把初轮失败隐藏为通过。`daily-native*.log`、`daily-real-debug.log` 保留了中间记录。

- 自动化最初用窗口标题定位 GTK AlertDialog，实际标题来自内容标签；改为识别对应标签。
- Kitty 固定 OS 标题不被窗口内部标题命令替换，且 legacy WM_NAME 使用 Latin-1；测试驱动
  只增加确定的隔离测试标题及编码处理，没有扩大到用户桌面窗口。
- 一轮专用 Xvfb 服务退出，导致 GTK 初始化失败；改为测试进程管理显示服务并启用 noreset。
- 原生测试输入长诊断命令，把 Greeting 滚出过小的窗口；测试使用 100 列 × 32 行，
  保留可见 GIF 再采集。没有把正常滚动解释成产品图片渲染错误。
- 以上修正后完整选定矩阵重新运行，最终为 10/10。

## 待验证 / 未实现

- 待验证：真实 GNOME 应用菜单索引、搜索 / 收藏、注销重登、Wayland / 多显示器。
- 待验证：用户自己的 Conda / 特殊 `.bashrc` / 启动时改色或替换 shell 的代码。
- 待验证：实际断电恢复、异常启动错误窗的原生交互、所有真实用户 GIF 和外部可用性。
- 未实现：Zsh / Fish / login-profile 日常环境适配；本轮是明确选择的 Bash 路径。
- 未实现：旧版本、运行程序及日志的自动垃圾回收；撤回会保留可恢复文件。
- 不属于本轮：替换普通 Kitty 图标、修改系统默认终端、为 Ptyxis 冒充整套独立 Kitty 会话。

操作说明：[中文验收卡及边界](kitty-daily-launcher.md)。
