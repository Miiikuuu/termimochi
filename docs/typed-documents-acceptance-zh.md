# 中文手动验收卡：按文档编辑、按目标使用

先用隔离版本看界面：在仓库运行 `bash scripts/run-isolated.sh`。
隔离模式里的应用和发布只作用于其临时 HOME/XDG；它不是文件系统沙箱，
不要在文件选择器中选真实配置作为写入目标。本轮自动化没有改你的日常配置。

界面底部显示当前类型。Save / Ctrl+S 保存 `.termimochi-design.json` 设计稿，
不会应用。右侧 Full / Terminal / Prompt / Greeting 是观察场景，左侧是编辑范围。
`⋮` 菜单包含 New Document、Create Project / Convert Copy、Export Native Copy、
Document Capabilities、Choose Use Target 和独立 Kitty 方案库。

## 任务一：只改配色

1. Open 打开 `.palette`。左侧应只有配色，不应允许快捷键进入字体/Prompt/Greeting。
2. 改背景；切 Full 仍能参考完整画面，但参考字体和欢迎内容不能被编辑或保存。
3. Ctrl+S 保存设计稿。点 Use Design，核对 Ptyxis profile 和全局 Light/Dark 说明。
4. 审阅中应只有配色安装及启用，没有字体/布局/Starship/Fastfetch 写入。
5. 在你自己授权的环境确认后，用结果页 Open Profile Tab 验证；Restore This
   Application 恢复本次成功变化。隔离 Ptyxis 若提示用户总线不可用，记“环境受阻”，
   不连接真实总线绕过它。

## 任务二：Kitty 独立动画项目

1. 在已有设计里选 Create Project / Convert Copy，类型 Explicit Project，目标
   Kitty。明确勾选 Palette、Typography、Layout、Current Prompt、Greeting。
   阅读转换说明后 Create Copy；原文档应仍在原窗口，不被覆盖。
2. Greeting → Import Artwork 导入 GIF，在原有编辑器确认处理结果；Full 应能播放。
   调颜色/字体时 Full 场景不能被左侧切页切走。
3. Save 保存项目。Use Design → Try in Kitty：弹出的真实 Kitty 应使用这套颜色、
   字体、当前支持的 Prompt，欢迎界面只出现一次，GIF 在真实窗口中运动。
4. 只有亲眼确认后，勾选外观确认和 GIF 运动确认，再 Create / Update Independent Entry。
5. 点 Open in Kitty。能输入命令；不会再次出现原来的绿色角色/旧欢迎脚本。
   注意：这是独立受控 Bash，不加载原 `.bashrc`，不是原日常环境的完整副本。
6. 关闭 App 再打开，从 `⋮ → Open Independent Kitty Scheme` 再次打开同一入口。
   再创建另一个项目，更新 A 不应改变 B。结果页可恢复上一版本或停用首次入口，
   被引用的文件不删除。

## 任务三：独立 Prompt / Greeting

1. 分别 Open `starship.toml`、Fastfetch JSONC，确认各在独立窗口且只开放所属编辑器。
   模糊文件应询问一次类型；不能仅因为扩展名相同就强行识别。
2. 编辑并 Ctrl+S；原配置不应变化。Export Native Copy 应是当前对应内容，不夹带终端外观。
3. Use Design 选择独立 Kitty 入口，或显式选择要更新的原生文件。只有后一种审阅
   才能写入那个文件；它不等于自动安装启动钩子。
4. Greeting 缺真实验证时，在同一流程点 Try Greeting Now，完成后回来 Review。
   Kitty/Ptyxis/Xterm 的名字和实际目标必须相符，不能让我复制路径猜终端。
5. 原生更新后 Run Applied Greeting 先选目标，再审阅完整配置的执行内容。
   若含导入命令/网络模块，取消应完全不执行；已改配置可通过原有恢复记录恢复。

## 防呆抽查

- 原生 Kitty 只写 `font_size 14`：Save/原生导出不应凭空增加 font_family、Prompt 或 Greeting。
- 在另一个窗口修改同一设计文件，再保存旧窗口：应提示外部冲突，不覆盖。
- 审阅后换目标或修改设计：旧确认应被拒绝；重新审阅不丢设计。
- 打开 GIF 是素材，不自动变成完整 Kitty 项目；Inspect 系统字段应提示“预览参考”。
- 不支持的原生内容应保留并说明，不得把“进程已启动”写成“画面已验证”。

这些步骤是交给真人的验收卡，不是“真人可用性已通过”的声明。
