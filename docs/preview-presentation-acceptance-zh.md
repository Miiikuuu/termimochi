# 本轮手动验收卡

普通启动（不会自动应用主题）：

```bash
cd /home/xiaozhouchao/Projects/personal/TermiMochi
./target/release/termimochi
```

想先在临时 App 状态里验收：`bash scripts/run-isolated.sh`。
隔离启动只能读到隔离配置，不能用于证明已捕获你的日常 Ptyxis；
可用文件选择器打开测试副本。脚本保留临时会话并打印重开方式。
本轮不需要安装，不要为了验收点击 Use/Apply。

| 操作 | 应看到的结果 |
| --- | --- |
| 新建 Kitty 或 Ptyxis 主题，不调整分隔条 | 默认 Full，有简短安全样例；终端窗口四周有余量，阴影和圆角完整 |
| 把 App 调到普通窗口、再扩大 | Actual size 文字不变小；大窗口中终端居中，不铺满整个右栏 |
| 输入 `help`、`git diff`，再输入一段中文但不回车 | 输入紧贴 Prompt；改背景、字号和左侧页不丢掉输入或历史 |
| 切换 Actual size、Fit window、100% / fixed-grid 观察 | Fit 连标题、标签、正文、图像和输入一起缩放；外面的编辑工具不缩放；不会把主题变成未保存状态 |
| 点预览的 `+`，在新标签输入 `cd src`，切回原标签 | 两个标签目录、输入和输出独立；顶部只有一个样例菜单；没有会关闭整个 App 的假按钮 |
| 多次 `help` 后滚动，再 `clear`，然后改颜色 | 只滚动窗口内部；外框不变高；clear 后不重新灌入欢迎样例 |
| Actual 与 Fit 下用拼音输入中文、选候选、Esc 取消，再复制粘贴 | 候选和输入位置对应，提交/取消正常；不能只粘贴中文就算输入法通过 |
| 保留一个未保存主题；新建菜单选 From My Terminal…（底部 ⋮ 也有） | 选择 Kitty/Ptyxis；多来源才选文件/profile；摘要说明是已保存配置，不是运行窗口抓取 |
| 展开 Sources, inherited values and limits | 看到 Read / Inherited / Unparsed / Unrecognized，以及实际来源；不是所有项目都宣称成功 |
| 若发现 Starship/Fastfetch 候选，先不勾选，再 Create Theme | 新窗口进入相应目标工作区；原主题不变；Prompt/Greeting 不会因为找到文件而自动加入 |
| 再导入一次，明确勾选候选 | 只在新主题加入所选 Prompt/Greeting；素材带入设计，不执行配置里的命令 |
| 新窗口 Ctrl+S 保存到新位置，再打开 | 保存完整副本；原终端、原文件不变；保存不是应用 |

`From My Terminal…` 可以读取可解析的日常已保存设置，但不保证与正在运行的
终端逐项一致。Kitty 的动态 include、命令行覆盖、即时设置会明确列为未读取。
隔离环境没有 Ptyxis profile 时应提示范围限制，不能绕过隔离访问日常设置。

默认 Full 是交互设计样例，不执行本机命令，也不证明 Kitty/Sixel 支持。
原生实验入口、Try Greeting、Use Theme 和恢复保持原有独立安全流程。

自动化已完成：相关默认 GUI 30/30、补充导入 2/2、原生/真实输入法 8/8；
完整计数、前后截图、几何和不写回证据见
[本轮报告](preview-presentation-system-import-qa.md)。日常 Wayland、分数缩放、
极窄窗口、外部用户体验仍待验证，不计为通过。
