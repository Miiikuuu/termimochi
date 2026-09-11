# 中文手动验收卡：一套主题，一个目标

这是操作卡，不代表真人易用性测试已通过。建议先用测试主题；点击 Save 不会
应用配置。Kitty 的独立入口不会更改默认终端、日常配置或 `.bashrc`。

| 步骤 | 怎么操作 | 应该看到什么 |
| --- | --- | --- |
| T02 打开颜色主题 | Open Theme 打开仅有颜色的 `kitty.conf` | 左上显示名称 · Kitty，右侧 Full；左侧五页均可进入 |
| T03 字号与继承 | Typography 只改 Font Size，Ctrl+Z，再 Ctrl+Shift+Z | 只记录字号；撤销回继承。Theme Settings & Inheritance 可查看/取消具体覆盖 |
| T05 加小猫和 Prompt | Greeting → Import Artwork 导入 GIF，开启 Greeting，选图像/动画；Prompt 原地编辑或导入，使用 Enable Prompt 开关 | 不要求 Create Project/Convert；名称和 Kitty 目标不变。Full 可播放 GIF；切左侧页不切右侧场景 |
| T06 保存重开 | Ctrl+S 选择 `.termimochi-design.json`，关闭重开 | 配色、字号、Prompt、GIF、原稿配方、名称、目标都保留；默认仍 Full |
| T08 在 Kitty 使用 | Use Theme → Try in Kitty；检查真实窗口，再勾选外观和 GIF 确认 → Create / Update Independent Entry | 先审阅实际文件和不支持项；没有真实试用不能发布。Try Greeting 只测 Greeting，不等于试整套 |
| T15 再次打开与恢复 | 结果页 Open in Kitty；重启 App → ⋮ → Open Independent Kitty Scheme；需要时 Restore / Deactivate Entry | 能重新打开同一入口；恢复只处理有记录的受管理版本，外部冲突会阻止覆盖 |
| Ptyxis | 打开 `.palette`，原地改字号/布局，再 Use Theme | 明确标出 profile/全局影响；未改模块不在写集中。没有完整隔离试用，GIF 不会借 Kitty 冒充通过 |
| 换终端 | Convert Terminal — Create Copy | 明确新副本和映射损失，原主题及已发布入口不变 |

注意：Ptyxis 的“取消覆盖”当前只撤掉主题意图，不会自动逐字段撤回已经写入的
共享设置；需要通过 Last Application & Recovery 审阅恢复。只指定零散颜色的
Ptyxis 主题需明确导入完整 palette，或确认将当前预览色表纳入主题基底。

Kitty 未指定的外观沿用受控会话默认，不是你的日常 `kitty.conf`；App 的未指定
字体/布局仅是预览参考。受控 Bash 不加载个人别名、Conda/ROS 或旧 Greeting 钩子。
