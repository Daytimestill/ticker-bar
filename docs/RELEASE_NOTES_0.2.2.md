# TickerBar 0.2.2 · 公开测试版

适合想试用 macOS 菜单栏行情和持仓速览的用户。它不会连接券商账号，也不会下单。行情来自腾讯公开接口，可能延迟或暂时不可用。

## 相对 v0.1.0-beta

- 菜单栏被其他应用挤掉时，用 `⌃⌘T` 打开速览；在「设置 → 系统」可换绑快捷键和调整排序。
- 菜单栏可显示总持仓收益和收益率；不同币种分别统计。显示项可暂停、恢复或删除。
- 设置页重新排版，最小窗口也能使用股票行的排序、删除按钮；修正速览窗口重复描边。
- 简化行情网络错误提示。保存时保留上一版配置，普通重启不会覆盖备份。
- 更新 Vue 及 Rust 网络依赖，纳入本轮安全修复。

## 安装与反馈

候选安装包目标为 macOS 13 及以上，包含 Apple Silicon 和 Intel 二进制。已在 Apple Silicon / macOS 26.5.2 上校验镜像和包内签名结构，未完成干净账户首装、Intel 实机或 macOS 13 实机验证。测试包未获 Apple 公证，不适合不允许安装未公证软件的设备。

请从本项目的 Releases 下载 DMG 及对应的 `SHA256SUMS.txt`。安装、校验和升级说明见 [README](https://github.com/Daytimestill/ticker-bar#安装)，维护者的人工验收要求见[发布检查表](https://github.com/Daytimestill/ticker-bar/blob/main/docs/RELEASING.md)。

有问题请反馈 macOS 版本、芯片、应用版本和复现步骤，不要上传自己的股票持仓、配置文件或账号信息。
