# 背景随主题保存与使用：验收卡

## 本轮实现

复用现有 `ThemeIntent`、Workspace 投影、窗口编辑器与主题撤销记录。
可选的 `background` 只包含共享不可变 PNG 素材和位置、缩放、覆盖比例；
不保存原文件路径、运行时路径、试用通过状态或执行权限。
旧主题没有此字段时仍没有背景，不会借用其他窗口的素材。

| 路径 | 本轮结果与边界 |
| --- | --- |
| 主题编辑 / Save | 图片、Cover/Contain、10–300% 缩放、X/Y、Overlay 随主题保存；重开不依赖原图；移除/撤销恢复 |
| Kitty 使用 | 复用现有临时试用、审阅、版本发布、入口重开和恢复；背景 PNG 纳入文件完整性检查 |
| Kitty 未指定 Background | 合成覆盖色使用实际 Kitty 输出的 Background；未指定时用受控 Kitty 的黑色默认值，不把预览参考配色写进输出 |
| 系统 Ptyxis 应用 | 不支持图片背景；已明确设置的背景在主审阅与结果记录中保持 Unsupported，不能报成已应用 |
| 增强版 Ptyxis | GUI 临时试用，或 Apply & Open 保存版本并创建独立菜单入口；App 重启后从入口库再开、更新、恢复；已应用终端不随编辑器退出 |
| 单项文件 | 配色、字体、布局文件不拥有背景；背景只作参考，转换副本明确提示遗漏；跨目标的主题副本保留素材 |

Kitty 与增强版 Ptyxis 使用同一份有界构图合成算法。
原生输出是按设计网格生成的静态画布，随窗口缩放；改变原生窗口宽高比可能拉伸。
它不是能在目标终端内重新拖拽的编辑器，也不表示 GTK 预览通过了真实协议验证。

## 中文操作卡

在仓库目录运行：

```sh
bash scripts/run-background-trial.sh --workbench
```

1. 打开或新建主题，进入左侧 **Background**（Ctrl+6）。
2. 点 **Choose Background Image…**。调 **Image zoom %**；打开 **Move Image**
   后可以在右侧拖动、Ctrl+滚轮缩放。普通滚轮仍滚动终端内容。
3. **Ctrl+S** 保存主题。关闭重开，检查图片、位置、缩放仍在。
   保存后的主题不依赖原 PNG/JPG/WebP 的路径。
4. Kitty：**Use Theme… → Try in Kitty**，确认真实窗口效果后
   **Apply & Open in Kitty**。以后从 **⋮ → Open Independent Kitty Scheme…** 打开。
5. Ptyxis：**Background → Open in Enhanced Ptyxis…**，阅读缺项与真实 shell 提示。
   **Open Real Terminal** 仅临时试用；要长期使用，点 **Apply & Open**，会创建
   `TermiMochi · 主题名 (Enhanced Ptyxis)` 独立应用菜单入口并打开终端，不再另勾知情框。
   后续从 **⋮ → Enhanced Ptyxis — Saved Entries… → Open** 或该菜单入口打开。
   修改主题后再次 Apply 会生成新版本；入口库 **Restore Previous…** 恢复旧版本，
   首版的 **Remove Entry…** 只移除快捷入口，保留版本素材、不停止现有终端。
   普通系统 Ptyxis 图标仍打开原版，不会被替换。
6. 移除背景后可用 **Ctrl+Z** 恢复。Kitty 更新入口后，也可使用现有恢复入口恢复上一版本。

保存、编辑和打开审阅不会执行本机命令。真正打开独立终端需要原有明确确认；
其私有配置不是文件系统安全沙箱，用户在真实 shell 中输入的命令仍有本机权限。

## 未实现、未验证与已知限制

- **未实现**：增强版 Ptyxis 的完整 Prompt/Greeting、个人 Bash/Conda 启动集成、其他布局字段、实时更新；
  系统原版 Ptyxis 的背景开关；动态 GIF 背景、SVG 背景、画笔和模糊。
- **未包含在发布包**：增强版 Ptyxis 仍是项目内可选运行时。没有安装到日常环境，也没有打包发布它。
  已应用入口会保留自己的 TermiMochi 启动辅助程序，但仍依赖审阅时的增强版运行时路径；
  不要删掉 `target/background-runtime/prefix`。运行时移动/变更会报错并要求重新审阅，不回退到系统 Ptyxis。
- **未验证**：真实 Wayland、硬件 GPU 差异、实际中文输入法预编辑、多显示器、
  分数缩放、桌面菜单和注销重登录。本轮隔离 X11 结果不能替代它们。
- 现有高级 Casilda 嵌入预览没有加入背景热更新；本轮不宣称重跑了完整嵌入原生交互矩阵。
- 对比度检查仍只检查纯色主题，不保证图像每个像素后面的文字都清楚。
- 原生图片画布会随窗口宽高比变化而拉伸；默认终端、用户启动文件与日常配置未改。

## 保存/应用接入验收记录（2026-09-26，确认流程精简前）

- CI 同版本 Rust 1.92.0，`--release --locked`，本地未提交工作区；基础 HEAD
  `a5755b1976492c194b619a309d412aa4f1da6627`，不是把新改动冒充为已发布提交。
- 工作区测试：**443 通过、0 失败、92 ignored**（App 407、CLI 4、CLI 合约 5、core 27）。
  ignored 不计为通过，下面的原生/GUI 用例另外实际运行。
- 默认 Release workspace Clippy、启用 `native-preview` 的 Release all-targets
  Clippy 均以 `-D warnings` 通过；格式与 diff 空白检查通过。
- 原生安全脚本 **8/8 通过**：私有 keyfile、批量校验、远程权限限制、supervisor 错误路径等。
- 最终同一优化构建的隔离原生/GTK 检查：**7 个不同用例、8 次执行、全部通过**。
  1×：背景编辑/保存重开/删除原图/撤销/迟到结果、Ptyxis 背景真实显示及再次打开、
  Kitty 背景+字体+当前 Prompt+动态 GIF+输入+发布入口重开、Full/GIF、独立场景、精简应用页。
  2×：背景编辑与保存重开、Inspect 实际指针导航。
- 背景 GUI 用例检查了“Open in Enhanced Ptyxis”确实取得当前图片并省掉第二次选图，
  并检查未完成图片处理会阻止 Save/Use。真实 Ptyxis 显示由独立原生用例验证；
  两者不冒充外部用户可用性反馈。
- 原生检查采样真实终端显示的双色背景；Kitty 另采样连续帧的 GIF 运动，检查真实
  字体查询、Prompt、输入与 Greeting 仅运行一次。截图已人工式视觉复核，不只检查字符串。
- 运行环境为独立 Xvfb、私有 HOME/XDG、软件渲染；增强版 Ptyxis 使用私有 schema、
  keyfile、D-Bus。没有修改日常配置、安装到日常环境、commit、push 或发布。

最终 App：`target/background-trial-build/release/termimochi`

SHA-256：`46181e10480f9793dd889d129e209f6f418c6e24bc260bfacde74f46b5e03b1c`

1× / 2× runner 记录的 production worker 与该文件 hash 一致；测试 harness 是另一个
可执行文件，hash 为 `2485a443fe605bbbbc382fe0b3646eba302df43384db016a1aaa6f5dc8254252`。
增强版 Ptyxis 二进制 hash：`24b54b8f61209865c6adb5bf9cd88df90002d3bd100c25e472722b137a0376ec`。

本机证据目录：`target/qa/background-theme-2026-09-26/`

- `1x/`、`2x/`：`background-saved-reopened.png`、`background-moved-0.png`、
  `background-drag-geometry.json`、runner `results.json` / `build.json`。
- `kitty/published-2.png`、字体查询、实际终端文字和测试日志。
- `ptyxis/saved-background.png`、`reopened-background.png` 和测试日志。
- Release、Clippy、安全脚本和 GUI 汇总日志在同一目录。

原始完整矩阵分别位于 `/tmp/termimochi-regression-urerzfxg/`（1×）和
`/tmp/termimochi-regression-rm8kze8d/`（2×），可与保留副本核对。
这些本地证据没有自动上传。日志仍含隔离环境的 portal/PipeWire、辅助功能总线、
软件 EGL 警告，以及已有 Full 切换时的 GtkScrollbar 分配警告；本轮没有声称修复这些问题。

## 确认流程精简复验（2026-09-26）

背景窗口移除重复知情勾选；**Open Real Terminal** 这一次明确点击授权启动。
主界面保留简短执行提示及 Prompt/Greeting 缺项，临时配置、权限边界、缩放限制折叠到详情。
未运行时不显示 Stop；运行中的停止/关闭确认仍保留，防止误杀任务。
Kitty 发布审阅、备份、冲突检查和恢复没有改动。

- Release 工作区 **443 通过、0 失败、92 ignored**；默认 Release Clippy、格式、diff 检查通过。
- 隔离 GTK 背景编辑/保存重开回归 **1/1 通过**，新增检查：没有知情复选框、详情默认折叠、
  明确的打开按钮可见、未运行时 Stop 不出现。
- 最终 Release App 的隔离原生端到端 **13 项检查通过**：打开审阅和选图不启动进程、
  点击按钮后实际启动、错误图片、真实输入、Cover/Contain、覆盖比例、正常退出、
  Stop 取消/确认和临时配置清理。原生截图已查看。
- GUI runner：`/tmp/termimochi-regression-9cj09608/`；真实 App/终端截图和结果：
  `target/qa/background-trial-xo1bzv78/`（含 `00-single-action-review.png`）。
  日志：`target/qa/background-single-action-{tests,clippy,build,gui,native}.log`。
- 当前可运行 App SHA-256：`15f1f0434d57159076a794c08b1cf406dda2a53a758b3f31b7f6ed85511ab14f`，
  与此次 GTK runner 的 production worker 相同。上节旧构建 hash 只代表上一阶段证据。
- 本次没有重跑完整 Kitty/Casilda/2× 矩阵，不把上节结果当成本次重跑；
  Wayland、硬件 GPU、真实中文 IME 等待验证边界不变。没有改日常配置或发布。

## Open Real Terminal 无响应修复（2026-09-26）

实际复现路径：**Use Theme → Open in Enhanced Ptyxis → Open Real Terminal**。
旧 Apply 审阅是模态窗口，新试用窗口却与它并列挂在工作台下，GTK 将真实鼠标输入
拦在旧模态窗口。按钮虽然可见、敏感，实际点击仍不会启动终端。
此前独立试用入口和程序触发按钮的测试没有覆盖这条路径，不能据此宣称它可用。

修复仅改变导航：进入试用前关闭旧审阅、释放模态输入。没有执行 Apply，未删除
真正打开 shell 的显式点击，也未改目标限制、文件审阅、备份、恢复或停止进程确认。

- 修复前 Release `15f1f043…`：实际鼠标点击失败，证据
  `target/qa/background-trial-_0f46mbt/`；`background-button-before.log` 记录等待原生窗口超时。
- 修复后同一 Release App 的工作台真实鼠标流程 **9 项检查通过**：从主题进入审阅、
  打开当前背景、真实 shell 输入、退出清理和再次点击打开。截图已查看，真实背景正常。
  证据 `target/qa/background-trial-rkg3g2yx/`，其中 `01-workbench-native-open.png` 为实际终端。
- 独立图片选择/原生启动/错误输入/停止及清理流程 **13 项检查通过**，证据
  `target/qa/background-trial-qn8spr9c/`。两条路径均使用只读宿主的 Bubblewrap、私有 HOME/XDG/D-Bus 和 Xvfb。
- Release 工作区 **443 通过、0 失败、92 ignored**；Release Clippy `-D warnings`、
  格式、diff 检查通过。另跑隔离 GTK **2/2 通过**：背景编辑/保存重开以及目标隔离，
  新增断言旧模态审阅确实关闭。原始记录 `/tmp/termimochi-regression-rnp_9t6y/`。
- 当前可运行 App SHA-256：
  `ee5fec74e696f7465e298b78d39b47112ec65689ceb36c61dccb80657e13055b`。
  两条真实 App 检查的 `build.json` 和 GTK runner 的 production worker 均对应此文件。
  日志 `target/qa/background-button-{build,tests,clippy,gui,after,standalone}.log`。
- 本次没有重跑 Kitty/Casilda；Wayland、物理 IME、硬件 GPU 等仍未验证。
  隔离环境仍有 portal/PipeWire 等告警，不能当成这些环境已通过。

手动复验：重启开发版，打开带背景的 Ptyxis 主题，依次点击上述三个入口。
审阅窗口应先退出，点 **Open Real Terminal** 后出现真实终端；退出后可再次点击打开。
此操作不会把背景启用到系统原版 Ptyxis，也没有安装、commit、push 或发布。

## 增强版 Ptyxis 长期入口验收（2026-09-26，资源打包修正前）

此前只有临时试用；这一节才是长期应用、App 退出后继续使用、再次打开及恢复的实现证据。
它是**背景外观的独立入口**，不是系统 Ptyxis 背景支持，也不是整套 Prompt/Greeting 已应用。

### 实现与安全边界

- 从当前主题/Workspace 捕获不可变输出快照，复用现有构图、颜色、字体和网格投影；
  没有第二套可变主题数据。审阅明确说明包含当前可见参考配色/字体和有界网格，
  不把这条路径冒充为原版 Ptyxis 逐字段应用。
- **Apply & Open** 放在已有审阅中，不新增知情勾选或第二张确认页。
  版本文件在 `$XDG_DATA_HOME/termimochi/ptyxis-background-sessions/<theme-id>/versions/`；
  快捷入口在 `$XDG_DATA_HOME/applications/`。仅显式点击 Apply 才发布。
- 桌面文件是唯一活动版本指针；先写完不可变版本，再复用现有受检写入、备份及冲突检查切换入口。
  素材/辅助程序/清单校验、外部修改、符号链接/硬链接、只读文件及迟到主题结果均有回归。
  恢复只恢复入口，首版撤回只移除自己的快捷方式；保留版本素材，不中断已运行的终端。
- 复制的辅助程序独立持有临时目录、私有 D-Bus 和 supervisor，因此编辑器退出不杀终端。
  增强版运行时仍依赖审阅时的固定路径；文件或 schema 改变后拒绝启动并提示重新审阅，
  不悄悄回退到系统 Ptyxis。启动失败保留入口并显示错误，不记为原生验证通过。
- Ptyxis 配置保持私有 HOME/XDG/keyfile/bus；实际 shell 恢复调用者的 HOME/XDG 并进入家目录。
  使用干净 Bash，不读取个人启动文件，不继承导出的 Bash 函数，也不自动初始化 Conda。
  **真实 shell 不是文件系统沙箱**：手动输入的命令仍有当前用户权限。

### 最终构建与检查

基础 HEAD 仍为 `a5755b1976492c194b619a309d412aa4f1da6627` 加本地修改，未提交或发布。
Rust 1.92.0，`--release --locked`；最终生产程序：

`target/background-trial-build/release/termimochi`

SHA-256：`dc469c490a6db8ef794dc7ac0efbf6ac985901c587e86ed1ecabca6036608cb0`

下列三个原生流程的 `build.json` 和 GTK runner 的 `worker_sha256` 都与此文件一致；
不是测试之后另换构建。

| 检查 | 实际结果 |
| --- | --- |
| Release workspace 单元/合约测试 | 452 通过，0 失败，93 ignored；App 416、CLI 4、CLI 合约 5、core 27；ignored 不计通过 |
| 默认 Release workspace Clippy | all-targets、`-D warnings` 通过 |
| `native-preview` feature Clippy | Release、all-targets、`-D warnings` 通过；不是完整嵌入功能实测 |
| 隔离 GTK/原生回归 | 4/4：背景编辑与保存重开、目标隔离、异步审阅换主题拦截，以及真实 Kitty 的背景/GIF/Prompt/发布重开 |
| 增强 Ptyxis 长期流程 | 16 项检查通过，含实际指针 Apply、真实输入、关闭 App 后继续输入、`gio launch`、App 重启入口库、更新、恢复、移除、配置及个人 rc 隔离 |
| 原工作台临时试用 | 9 项检查通过，包括真实鼠标穿过审阅并打开，不回归模态遮挡问题 |
| 独立图片试用 | 13 项检查通过，包括无图/错误图、真实输入、Contain/Overlay、停止取消与确认、清理 |
| 原生安全脚本 | 8/8 通过 |
| 格式、diff 空白、生成 desktop 元数据 | 通过 |

本轮真实测试发现并修复生成 shell 命令的 GVariant 双重转义问题；修复前曾无法恢复 shell HOME，
增加了转义回归并重跑最终生产程序。另修正测试驱动对 GTK 菜单辅助名称为空的处理，
从可见菜单用真实键盘选择入口。失败记录保留，不记为通过。

### 原生证据与未验证项

日志：`target/qa/background-persistence/`，包括 `tests.log`、`clippy-final.log`、
`native-feature-clippy-final.log`、`gui-final.log`、`native-final.log`、
`trial-workbench-final.log`、`trial-standalone-final.log`、`native-safety-final.log`。

- 长期流程：`target/qa/background-trial-r846zhqk/`，含 `results.json`、`build.json`。
  已查看 `00-workbench-review-before-click.png`、`11-editor-closed-terminal-alive.png`、
  `13-updated-native.png`、`14-restored-native.png`：原生背景可见、更新/恢复尺寸对应、
  shell 输入可见，关闭编辑器后仍可操作。
- 工作台临时流程：`target/qa/background-trial-fg9ku1y1/`；独立试用：
  `target/qa/background-trial-dk4v8z_i/`，已查看后者 `03-native-shell.png`。
- GTK/Kitty 回归：`/tmp/termimochi-regression-x2knu12b/`，含每项日志、原生截图及构建记录。
- 修复前 shell 转义失败：`target/qa/background-trial-7qcor2qr/`；菜单驱动失败：
  `target/qa/background-trial-l09uv_u0/`。这些是失败调查证据，不是最终通过结果。

全部原生执行在私有 HOME/XDG、Xvfb、软件渲染中；长期/临时生产 UI 流程另以 Bubblewrap
只读宿主、仅放开 QA 目录。没有在日常应用目录创建入口，没有改个人配置、默认终端或启动文件。

**未实现**：本页前述增强 Ptyxis Prompt/Greeting、个人 Bash/Conda、完整布局、热更新等。
**未验证**：真实桌面菜单索引/发现（`gio launch` 不等于桌面菜单点击）、Wayland、物理 IME、
硬件 GPU、多显示器、分数缩放及注销重登录。完整 Casilda 嵌入矩阵本轮未重跑。
**环境告警**：隔离环境 portal/FUSE/PipeWire/辅助功能与软件 EGL 告警仍在，不能当作这些能力通过。
当前列明的最终检查无失败；原生已知边界没有因改入口而消失。

## 推送前资源打包复验（2026-09-26）

推送检查发现后台脚本原先位于 Rust crate 外，独立源码包不能依赖该仓库相对路径。
现将同一份脚本移至 `crates/termimochi/resources/native/session-supervisor.py`，
同步更新嵌入引用、原生构建脚本和安全测试；未改变进程生命周期逻辑。
增强版补丁的空白上下文规范化后，已对固定源码运行反向 `git apply --check` 验证可用。

- `cargo package --workspace --allow-dirty --locked` 三个包的构建验证全部通过；
  `.crate` 归档已确认包含 supervisor，不只是在工作区里能编译。
- Release workspace 构建、Clippy `-D warnings`、452 个测试（93 ignored）、
  原生安全 8/8、格式/暂存 diff 检查再次通过。
- 重新构建后的生产 App 再跑增强 Ptyxis 长期流程，16/16 通过；证据
  `target/qa/background-trial-yn60da3n/`，日志 `target/qa/background-persistence/native-push.log`。
  本次未重跑上节全部 GTK/Kitty/临时试用矩阵；上节对应其各自列出的旧构建。
- 当前 Release App SHA-256：
  `e0a3707aa33e5e6b3ff17797f8615e753598c55cf08f6cf494be00b2ebd1b7e7`，
  与此次原生流程 `build.json` 一致。上节 hash 保留为历史验收证据。
- 打包、构建、Clippy、测试日志分别是 `package-push.log`、`build-push.log`、
  `clippy-push.log`、`tests-push.log`，均位于 `target/qa/background-persistence/`。

这次用户授权的是源码提交和 push；不发布安装包，不改日常配置。
真实桌面/Wayland 等未验证范围与尚未实现的能力保持不变。
