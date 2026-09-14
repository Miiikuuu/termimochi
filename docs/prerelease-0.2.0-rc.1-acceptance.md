# 0.2.0-rc.1 交付与验收卡

2026-09-14。可安装候选版，尚未发布 Release。初次交付后的用户授权安装与
真实桌面验收见[后续记录](daily-environment-2026-09-14.md)。

## 安装包

[下载／打开安装包](../target/preview-packages/termimochi-0.2.0-rc.1-linux-x86_64.tar.gz)
（约 5.5 MB），[校验和](../target/preview-packages/termimochi-0.2.0-rc.1-linux-x86_64.tar.gz.sha256)。

以上是交付机器上的本地文件，不是 GitHub Release 下载；`target/` 原始证据和
安装包不随源码提交。压缩包中保留打包时的文档，后续验收状态以仓库记录为准。

适用本次构建环境 **Ubuntu 26.04.1 / x86_64**。这是带安装脚本的压缩包，
不是免依赖 AppImage 或 deb；不包含 Kitty、Ptyxis、Starship、Fastfetch 和系统动态库。
包内 `PREVIEW.json` 记录工具链、平台、源码指纹及依赖。安装包未签名。

解压后，在解压目录打开终端：

```bash
bash scripts/check-preview.sh
./target/release/termimochi
```

上面只是检查和运行。确定要安装时再执行：

```bash
bash scripts/install.sh install --dry-run
bash scripts/install.sh install
```

卸载用 `bash scripts/install.sh uninstall`，指定过路径时需保持相同参数。
安装会替换旧 App 文件，不修改终端配置或 shell 启动文件；主题及受管理入口有各自的恢复流程。
详见[安装边界](prerelease-install.md)。

## 已完成与测试

| 内容 | 结果 |
| --- | --- |
| 两种构建、Clippy、普通测试、Release、安装器、元数据 | [10/10 检查通过](../target/qa/native-preview/build-checks-12lw1jlm/results.json)；每种构建 431 项普通测试通过，忽略项不计通过 |
| Kitty 启动器定向修复、试用确认、GIF、重开、更新／恢复、错误页跳转，Full 回归 | [Release 4/4 通过](../target/qa/release-acceptance/termimochi-regression-hm9aq586/results.json) |
| Ptyxis 逐字段撤回、App 重启后继续恢复、保留后来修改的字段 | [完整恢复通过](../target/qa/release-acceptance/ptyxis-apply-iedayekk/result.json)，[部分恢复通过](../target/qa/release-acceptance/ptyxis-apply-7juw7yui/result.json) |
| 实验原生 Kitty／Ptyxis：标签、中文输入、剪贴板、热更新及生命周期 | [实际启用的 Release 2/2 通过](../target/qa/native-preview/termimochi-regression-native-qqm8dkrf/results.json)，1× 隔离 X11 |
| 最终压缩包的校验、隔离安装／卸载、CLI、桌面文件、损坏包拒绝 | [通过](../target/qa/preview-package/install-br1ib8dc/result.json) |
| 从最终包取出的 GUI：编辑、保存、重开、外部文件冲突、配置哨兵 | [黑盒通过](../target/qa/release-acceptance/blackbox-exg330bj/result.json)，只编辑复杂主题的隔离副本 |

以上默认 Release 使用同一 GUI SHA256：
`f00fb3a2fdd09230ce865f344ac339204a91abe74cea1121592979795c79724e`。
安装包 SHA256：`e4e684361bc79dc8a24f7401b602bd47c4c5f7f7a461e8bc3b89d702320fc56b`。

实际截图：[启动器错误](../target/qa/release-acceptance/termimochi-regression-hm9aq586/1x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/repair-route/01-error.png)、
[定向修复审阅](../target/qa/release-acceptance/termimochi-regression-hm9aq586/1x-kitty_session-launcher-tests-launcher_native_daily_gif_and_desktop_reopen/cache/repair-route/02-targeted-review.png)、
[逐字段结果](../target/qa/release-acceptance/ptyxis-apply-7juw7yui/07-partial-restore.png)、
[再次打开真实 Ptyxis](../target/qa/release-acceptance/ptyxis-apply-7juw7yui/08-partial-retry-native.png)。

## 不用读源码的操作卡

| 你操作什么 | 应看到什么 |
| --- | --- |
| 正常编辑主题 → Save | 保存设计，不偷偷改系统 |
| ⋮ → Open Independent Kitty Scheme… → 失效入口 → Repair This Launcher… | 明确该主题和 Kitty 目标；不是让你猜配置路径 |
| Try Daily Session → 检查实际外观 → 勾选 → Create / Update App Launcher | 试用、确认前不能更新；只换当前入口，其他入口不受影响 |
| 撤回启动器更新 | 返回旧入口、保留旧素材；旧入口本来损坏时不会承诺能启动 |
| Ptyxis → Restore This Application… | 安全字段恢复，外部改动保留；未全部恢复时展开具体字段结果 |
| 重启 App → ⋮ → Last Application & Recovery… → 重试 | 只处理剩余字段，不覆盖已经恢复后你又修改的字段 |

不要为了验收手动破坏日常配置或删除日常运行程序；故障注入已经在隔离测试中完成。
收据／主题素材被外部修改或缺失时，修复会阻止覆盖并提示重新发布，而不是冒充成功。

## 明确保留的验证缺口

**Ptyxis 日常全局应用／撤回、收藏与注销重登、真实策略锁、其他机器和外部
用户可用性仍待验证。** 用户授权后，当前机器的 Kitty 个人 Bash／Conda、
桌面入口、修复及撤回已做实际检查；中文输入、剪贴板和双屏显示由用户确认通过。
这些不是隔离 X11 的替代推断。准确范围见[单独保留的验证任务](desktop-environment-validation.md)。

隔离环境的 portal/systemd/VTE 警告、测试驱动修正和失败尝试完整保留在
[实现与证据说明](prerelease-qa.md)。本轮不宣称线上 CI 或最新 RustSec 审计已通过。
