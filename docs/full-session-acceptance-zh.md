# Full Session 中文手动验收卡

这是完整**设计预览**，不是“整套方案已在真实终端生效”。本轮不需要 Apply。

## 启动隔离版本

在项目目录运行：

```bash
bash scripts/run-isolated.sh
```

如果尚未构建：`cargo build --workspace --release --locked`。
启动器会打印 `target/qa/session-…` 的隔离目录及再次打开它的命令。
它隔离 HOME/XDG、D-Bus 和 GSettings，不是文件访问沙箱：只操作测试文件副本，
不要安装、启用开机欢迎或在本次验收中确认 Apply Scheme。

## 一张操作卡

| 操作 | 应看到的结果 |
| --- | --- |
| 左侧 Greeting → 开启 Greeting → **Add image / GIF…**，选自己的 GIF 副本；调整后点 **Use Artwork**，输出选择 **Animation** | 保留可编辑源稿；选输出方式不等于已验证 Kitty |
| 右上观察场景选择 **Full** | 同一画面出现 GIF、系统信息、当前 Prompt、`printf` 模拟命令及输出、后续 Prompt |
| 点击 **Play GIF** | 动画播放；点击暂停可停住。系统关闭动画时保持暂停，不改系统设置 |
| 左侧切 Palette，修改 Background 和 ANSI 色 | Full 保持选中；整体背景和相应文字颜色更新，GIF 不因切页重置 |
| 左侧切 Typography，换字体/字号，再调行高、字符宽度 | 信息、Prompt、命令使用同一终端字体；图案按新的单元格尺寸对齐 |
| 左侧切 Layout 调 Content Padding，再切 Prompt 调结构/单双行 | 仍观察 Full，间距和 Prompt 在完整组合中更新 |
| Greeting 修改 Columns，例如 24 → 40 | 图案实际占位改变。若图很高，会受原有 64 行上限约束 |
| Full 工具栏切 Fit / 100% / 75% / 50% | 整幅画面缩放；Columns 和字体设置不变，也不新增未保存修改 |
| 100% 时滚动，开启 Inspect 点图案/文字 | 图案和文字一起滚动；左侧定位对应编辑项，右侧仍是 Full |
| 依次选择 Terminal / Prompt / Greeting，每次再切左侧模块 | 右侧保持所选场景。Greeting 单独预览也仍可用 |
| 回到 Full，Save 到测试目录的 `demo.termimochi.json` | 保存整套设计，不改变外部终端，不自动应用 |
| 关闭，再用隔离启动器打开保存文件；手动再选 Full | 颜色、字体、布局、Prompt、Greeting/GIF 源稿仍在；Full/缩放/播放不是保存内容，真实输出验证不随文件恢复 |
| 阅读 **Try Greeting** 的提示，若要协议验证再明确点击 | 它只在目标终端临时验证 Greeting，不代表字体、配色、Prompt 整套真实试用 |

打开文件会沿用原有初始化规则：启用了 Greeting 就先看 Greeting，否则看文件中的
Prompt 或 Terminal；不是忘记保存。新开进程的 Full 默认 Fit、暂停。

## 如何判断边界

- Full 的 “Design simulation · not terminal verification” 是正常提示。
- Full 里 GIF 动，不代表 Ptyxis 支持 GIF，也不代表 Kitty 已验证。
- 图案不见时先看 Greeting 是否开启、是否选了 Minimal Card、是否有图像准备错误。
  Minimal Card 本来就不显示图案；失败时会明确提示并显示字符回退。
- 真正修改外部终端仍要单独进入 **Apply Scheme…** 审阅。Save 和本卡的设计预览
  不会替你完成这一步。
- 请反馈具体操作、窗口大小/缩放、截图和预期结果。真实用户易用性、本机 Wayland
  混合缩放及日常终端效果仍需人工验证，不用把它们当作已经通过的自动化项目。
