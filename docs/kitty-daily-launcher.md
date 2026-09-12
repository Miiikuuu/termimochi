# Kitty 日常使用入口

本轮补齐的是：把已经审阅、发布的 Kitty 主题变成系统应用菜单中的入口。
不把 Save 改成系统应用，不替换普通 Kitty 图标，不修改默认终端或 shell 启动文件。

## 中文验收卡

| 操作 | 应看到什么 |
| --- | --- |
| 在 Kitty 主题中完成设置并 Save | 保存设计；普通终端不应变化 |
| Use Theme → Try in Kitty → 确认外观及 GIF → Create / Update Independent Entry | 发布同一目标的独立主题版本 |
| 结果页 Add / Update App Launcher… | 选择 Isolated Bash 或 My Bash Environment |
| 选择 My Bash Environment | 明确提示将执行本机 `/etc/bash.bashrc` 和 `~/.bashrc`；不修改它们 |
| 审阅页 Try Daily Session | 打开实际 Kitty；检查主题、GIF、自己的 alias / 环境初始化是否正常 |
| 勾选确认 → Create / Update App Launcher | 未试用、未确认时不能创建；成功页显示可搜索的名称 |
| 关闭 TermiMochi，在系统应用菜单搜索“主题名 — Kitty” | 直接进入主题终端，无须再输入 fastfetch 命令 |
| 关闭终端，再次从该入口打开 | 同一主题、Prompt 和 Greeting 再次出现，GIF 应播放 |
| 再发布修改后的主题 | 旧入口明确要求 Update App Launcher，不悄悄切成新版本或普通黑色 Kitty |
| ⋮ → Open Independent Kitty Scheme… | 可重开独立版本，也能打开 / 撤回应用菜单入口 |
| Remove / Restore App Launcher… → Undo Launcher Update | 恢复上一次入口，或移除首次创建的入口；素材仍保留 |
| 停用没有前一版的主题 | 明确显示 Deactivate Entry，不再声称存在可恢复的前一版；应用菜单入口仍可从库中撤回 |

已有独立主题不必重新导入：从 **Open Independent Kitty Scheme…** 打开对应版本，
再选择 **Add / Update App Launcher…**。本次代码验证不会替用户完成日常安装。

## 架构与保存位置

复用现有 `kitty_session::Deployment`、目标命令生成、安全投影及版本检查。
新 `kitty_session::launcher` 是已发布版本的输出适配层，不是第二份可编辑主题状态。
`window/daily_launcher` 负责环境选择、审阅、实际试用、确认、安装和撤回。
启动程序在 GTK 编辑器启动前处理专用 launcher 参数，因此应用菜单启动不需要编辑器进程。

- `$XDG_DATA_HOME/applications/io.github.miiikuuu.termimochi.theme-<id>.desktop`：唯一发布切换点。
- `$XDG_DATA_HOME/termimochi/kitty-launchers/<id>/version-*/`：不可变审阅收据、生成的启动脚本、前一版桌面入口快照。
- `$XDG_DATA_HOME/termimochi/launcher-runtimes/<sha256>/termimochi`：按内容去重的运行程序副本；启动和 GIF settle helper 均使用它。
- 主题仍引用原有 `kitty-sessions/<id>/versions/` 中的同一套配置和素材。
- `$XDG_CACHE_HOME/termimochi/launcher-logs/`：独立启动日志。立即退出会提示错误及日志位置；较晚退出仍需检查日志。

未设置 XDG 时使用标准用户数据 / 缓存目录。先完整落盘收据与运行程序，最后原子发布
`.desktop`；中断留下的未发布素材不被当成成功。复用现有有界读取、受管理目录检查、
原子写入、备份与外部冲突检查。撤回只处理这次桌面入口，不删除可能被终端引用的文件。
保留的运行程序会占用磁盘；本轮不自动清除历史版本、运行程序或日志。

当前入口固定到审阅的主题版本。主题当前版本变化或停用、配置 / 启动脚本变化、依赖程序
变化、桌面入口被外部修改时会阻止使用或覆盖。由应用菜单启动时，前置错误有独立 GUI
提示，可打开编辑器修复，不以配置路径作为唯一终点。

## Bash 与安全边界

My Bash Environment 显式执行本机 Bash 初始化，保留 aliases、函数、PATH、历史等。
初始化的标准输出隐藏以避免重复 Greeting，标准错误仍可见；主题拥有 Prompt 时，
之后初始化当前 Starship。未拥有 Prompt 时不覆盖本机 PS1 / 提示符钩子。
不读取 login profile，不承诺 Zsh / Fish 兼容。

这是用户明确选择的本机代码执行，不是文件系统沙箱。修改终端颜色、直接写 `/dev/tty`、
`exec` 另一个 shell 的自定义启动代码仍可能冲突，必须在日常试用中检查。
审阅后、本次安装前，直接读取的 Bash 启动文件发生变化会要求重试；它们间接 source 的
文件不递归冻结。之后用户自行修改 Bash 初始化将影响之后的启动。

Kitty 继续只使用生成的安全配置，不读取日常 `kitty.conf`；导入配置中的任意命令仍不会
因此获得执行授权。Kitty、Bash，以及主题用到的 Starship / Fastfetch 仍须安装。
运行程序副本不包含系统动态链接库，因此并非跨系统免依赖安装包。

## 验证范围

自动化结果和隔离证据见 [本轮 QA](kitty-daily-launcher-qa.md)。
实际桌面菜单搜索 / 收藏、用户自己的 Conda / 自定义 Bash、真实 Wayland / 多显示器、
注销重登后的菜单索引和外部用户可用性测试单列为待验证，不能由隔离 X11 结果代替。
