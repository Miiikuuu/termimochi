# 这一版怎么验收

这张卡只验收本次“整套方案 + Greeting 输出”流程。先用隔离版；不要点日常终端的安装脚本，也不需要改 `.bashrc`。

## 先启动隔离版

在项目目录运行：

```bash
bash scripts/run-isolated.sh
```

它会打印 `Isolated session: .../target/qa/session-...`。保存好这个路径。
这是新 HOME、独立配置目录和独立 D-Bus 会话；字体/主题初始值与日常终端不同是正常的。
它不是文件访问沙箱：请只打开、保存测试副本，不要手动选择真实配置作覆盖目标。
测试文件保留，关闭不会删掉你保存的方案；GSettings 测试值在重启后重置。

如果提示需要构建，先运行 `cargo build --workspace --release --locked`。

## 操作卡

| 步骤 | 你怎么做 | 应该看到什么 |
| --- | --- | --- |
| 1. 放入作品 | 左侧 Greeting → 打开启用开关 → **Add image / GIF…**，选一张自己的图片/GIF；调整裁剪、去背景，点 **Use Artwork** | 作品进入草稿，原文件不变；日常终端不变。重新点 **Edit Artwork…** 还能编辑原稿。 |
| 2. 选择样子 | 在 **Display** 选 Image / Character；GIF 还可选 Animation。用 Compact / Standard / Wide 或 Columns 调占位 | 改的是方案里的显示方式。Character 的列数越多通常越细、也越占地方；不是免费的“提高清晰度”。 |
| 3. 区分缩放 | 在右侧 **Fit preview / 100% occupancy** 间切换；GIF 点 **Play animation** | Columns 和保存状态不变。这里是 GTK 设计预览，不是终端协议兼容性证明。布局/文字的最终效果以真实试用为准。 |
| 4. 保存整套 | 随便切到 Palette、Typography、Layout 或 Prompt，按 **Ctrl+S**，在测试会话目录保存为 `my-demo.termimochi.json` | 不管在哪个模块，保存的都是整套。移动原图片后，方案里的原稿仍可编辑。Save 不会应用终端配置。 |
| 5. 关闭再打开 | 关闭后，运行终端打印的 `bash scripts/run-isolated.sh --session '…'`，用 Open 打开刚保存的方案 | Display、Columns、原稿及其他模块保留；旧的“已验证”不会自动恢复。目标可以记住，共享替换勾选不能记住。 |
| 6. 真正试用 | 底部 **Greeting target** 选已安装的 Kitty（图片/GIF），点 **Try** | Kitty 展示临时配置，不读写日常 Fastfetch。回到应用点 **Looks correct**；若空白点 **Nothing displayed**，不动点 **Animation broken**。不需要记终端按键。 |
| 7. 应用测试副本 | **保持 Replace shared greeting with pixels 不勾选** → **Apply Scheme…** → 只勾 Greeting → **Back Up & Apply Selected** | 路径是本次隔离会话 `data/termimochi/targets/kitty/config.jsonc`；结果为 **Installed · not enabled**。这不是失败：素材和独立配置已装好，但没有修改共享配置或开启自启动。 |
| 8. 重复与恢复 | 改一项信息字段再审阅 Greeting；然后在 **⋮ → Last Application & Recovery…** 中恢复相应应用记录 | 字段修改不会悄悄把图片改为 ANSI。每次结果分项显示；恢复要另一次确认，外部冲突不会被强行覆盖。图片资源会保留，避免破坏备份引用。 |

应用结果会显示准确的 `fastfetch --config '…'` 命令。若要检查已安装的副本，可在支持的目标终端手动运行它；它会运行完整配置中的自定义命令/网络字段，只有确认这些内容安全后才运行。**Try** 则始终使用安全示例信息，不能证明自定义命令正确。

## 再做四个防呆动作

- 点应用审阅后 **Cancel**：终端配置不变。
- 成功试用后再试一次，并点 **Animation broken / Nothing displayed**：不能继续沿用上次“已验证”安装。
- 换目标终端、改 Columns 或改目标字体配置：旧验证应过期；未验证不能冒充支持。
- 真想改回字符：明确选 **Display → Character**，再审阅 Greeting。仅切目标不会修改已安装文件；也可以用恢复记录回到上一次配置。

## 目前不能当成“已通过”的部分

- 当前桌面合成器上的 Wayland 分数缩放、临时终端缩放/外部 include、远程和复用器环境。
- 多位新用户是否能不看说明独立完成。现在没有外部测试者，自动化不能代替他们。
- GTK 图片合成与每一种真实终端布局的逐像素一致性、未测试终端版本的普遍兼容性。

遇到问题请留：方案副本、Display、Columns、目标、报错或结果页、实际终端与应用各一张截图。方案包含原始图片和导入文本，分享前检查隐私。
