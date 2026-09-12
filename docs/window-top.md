# Window Top

The current theme's **Layout → Window Top** page groups window-title and tab-bar
controls. This is the terminal's chrome, not TermiMochi's own application header.

## Capabilities and ownership

| Setting | Kitty output | Availability |
| --- | --- | --- |
| Title background: Follow System / Theme Background / Custom Color | `wayland_titlebar_color system / background / #RRGGBB` | Kitty on GNOME Wayland, with client-side decorations; exact native appearance still needs verification |
| Show Tab Bar | `tab_bar_style hidden` or the selected style | Kitty |
| Minimum Tabs | `tab_bar_min_tabs` (1–16) | Kitty; 1 means always, 2 means at least two tabs |
| Position | `tab_bar_edge top / bottom` | Kitty |
| Style | `tab_bar_style fade / slant / separator / powerline` | Kitty; executable custom tab scripts remain outside the safe adapter |
| Active/inactive text and background | `active_tab_foreground`, `active_tab_background`, `inactive_tab_foreground`, `inactive_tab_background` | Kitty |
| Ptyxis header background/text | `TitlebarBackground` / `TitlebarForeground` in the active palette variant | Ptyxis; edited in Window Top or Palette → Ptyxis Window Top, one shared value |

The supported native subset was checked against installed Kitty 0.45.0 and the
[official configuration documentation](https://sw.kovidgoyal.net/kitty/conf/).
Newer vertical edges and custom scripts are not advertised as supported.
Unsupported imported directives remain in the preserved source, not executable
trial output. Ptyxis now exposes its header colors in Window Top and shows a
Ptyxis-specific header/tab design preview in all four scenes. Its visibility
switch remains preview-only; native tab shape and selection shading follow
Adwaita. Kitty's three titlebar modes, tab positions, styles and four independent
tab colors are not falsely offered as Ptyxis capabilities. Ptyxis's absent
optional header colors use native computed defaults, not a system-color mode;
the App uses palette background/foreground as an explicitly approximate fallback.

These are fields in the existing `LayoutSettings`, projected through the existing
theme's sparse field ownership. There is no second mutable theme model. Opening
a color-only Kitty configuration uses Kitty chrome defaults as **preview
references**, not the current Ptyxis tab visibility. Untouched chrome fields do
not enter the theme patch or native write set. Explicit imported `hidden` remains
hidden. Legacy layout presets receive backward-compatible field defaults and
retain their explicit visibility setting.

Ptyxis header colors are authored directly into the existing Palette, not copied
into LayoutSettings. Palette and Window Top controls synchronize both ways;
light/dark variants remain independent. A theme with only header overrides still
requires the existing explicit complete-palette-base inclusion before installation.

The existing color picker supplies HEX/RGB/HSV editing. Invalid color drafts
block Save. Undo/Redo, theme save/reopen, native source reconciliation, layout
presets and reviewed Kitty publication use the same fields. Save does not apply.
Kitty trial, independent publication and reopening use the existing adapter and
retain its conflict, backup and recovery boundaries. Unsupported titlebar output
is removed at the adapter boundary with a result note, not merely disabled in UI;
the saved design retains its value for a supported environment.

The preview draws a title bar and two example tabs inside the existing terminal
shell; no new terminal or Greeting renderer is introduced. Left-side navigation
does not change Full / Terminal / Prompt / Greeting selection. Palette and font
refreshes redraw this window-local chrome. Top/bottom changes reorder only the
tab widget, and repeated refreshes do not reorder it again. Inspect navigates
the title and tab regions to their Layout controls. The drawing is an
**approximation**, not a Kitty/Sixel protocol test or exact system-decoration
capture. At a minimum above two, both sample tabs disappear by design.

## 中文验收卡

先保存工作并关闭旧版 App，运行仓库中的 `./target/release/termimochi`。

1. 新建 Kitty 主题，或打开只有配色的 `kitty.conf`。进入 **Layout → Window
   Top**（左侧向下滚动）。不用创建项目或转换副本。
2. 在 **Tab Bar** 开启 **Show Tab Bar**，把 **Minimum Tabs** 设为 `1`。
   切换 Top/Bottom、四种 Style；右侧示例标签应同步变化。
3. 点击四个颜色按钮，分别修改选中／未选中标签的文字和背景。试输入
   无效 HEX，再 Save，应要求修正；合法颜色可以保存。
4. 切到 Palette 或 Typography 编辑，再回 Layout：观察场景不应被切走。
   Ctrl+Z/Ctrl+Shift+Z 应撤销／重做编辑。
5. Save 后关闭重开：主题目标、位置、样式和颜色保留。再打开另一个主题
   并修改它，原窗口不能跟着变色或移动标签栏。
6. **Try in Kitty** 查看真实标签。Minimum Tabs 为 `2` 时，需要在 Kitty
   按 `Ctrl+Shift+T` 新建第二个标签才会显示；为 `1` 时单标签即显示。
   再通过 **Use Theme…** 审阅并发布独立入口，使用结果页的打开按钮。
   本轮自动验证只在临时目录中发布，未安装到你的日常环境。
7. **Title Bar** 仅在 Kitty + GNOME Wayland 环境开放：依次试 Follow
   System、Theme Background、Custom Color，再用 Try in Kitty 看真实窗口。
   X11/其他未确认环境应禁用并说明原因，不应假称成功。
8. 切换 Ptyxis 主题：Window Top 应显示 **Background / Text** 两个顶部
   配色按钮，右侧显示 Ptyxis 标题栏。开启 Show Tab Bar 可观察标签示例，
   此开关仍只影响预览；Kitty 专属样式不可编辑。
9. 在 **Palette → Ptyxis Window Top** 选择同一颜色修改，再回 Layout：
   按钮、顶部预览和保存值应一致。切换 Light/Dark 应各自保留其顶部色。
   Inspect 点标题栏应导航回 Window Top。
10. Ptyxis 使用 **Use Theme…** 的配色安装／启用流程，顶部颜色跟随该配色
    生效；不要用 Layout 单项 Apply 代替配色应用。仅编辑顶部色的新主题
    仍会要求明确包含完整配色基底。Save 本身不改终端。

实际 GNOME Wayland 标题栏、其他合成器、分数缩放和用户日常桌面操作仍需
单独验收；Xvfb 截图不能替代这些结果。自动化证据见
[本轮 QA 记录](window-top-qa.md)。
