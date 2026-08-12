# TickerBar

在 macOS 菜单栏显示股票价格、涨跌和持仓收益的本地工具。没有常驻主窗口，也不显示 Dock 图标。

行情来自腾讯免费接口，无需注册、无需密钥、不登录券商账户。

## 功能

- 菜单栏显示置顶股票，最多添加 8 只，一次请求批量刷新（固定 3 秒）
- 点击菜单栏文字弹出股票列表，点任意行切换置顶；列表顺序可拖动调整
- 按交易时段与行情时间戳区分 交易中 / 休市 / 延迟（A 股与港股分别判定）
- 每只股票独立配置本地持仓，逐股显示收益，并按币种分组给出合计
- 提醒通知：价格、涨跌幅、持仓收益到达阈值时发 macOS 系统通知
  - 穿越触发（回落后再次越过才响），不会反复轰炸
  - 通知文案可完全自定义，支持静默
  - 设置窗口停在最前台时 macOS 不弹横幅（系统对自家 App 的固定行为），此时改由窗口内卡片提示
  - 休市期间可用「试发」验证通知链路，无需等开盘
- 菜单栏显示项可任意勾选、排序、调整精度与格式，含实时预览

## 安装

> ⚠️ **本应用未经 Apple 公证（notarization）**，从网上下载后 macOS 会拦截。这是所有未付费加入 Apple 开发者计划的开源 macOS 应用的共同处境，不是软件有问题。

1. 从 [Releases](https://github.com/Daytimestill/tickerbar/releases) 下载 `TickerBar.dmg`，把 TickerBar 拖入「应用程序」
2. 首次打开会提示无法验证开发者，此时**不要**反复双击，按以下任一方式放行：

   **方式一（图形界面）**：打开 系统设置 → 隐私与安全性，滚动到底部会看到「已阻止 TickerBar」，点「仍要打开」并输入密码。只需做一次。

   **方式二（命令行）**：
   ```bash
   xattr -dr com.apple.quarantine /Applications/TickerBar.app
   ```

3. 首次启动会自动打开设置窗并显示引导：搜索添加股票、按需配置持仓与提醒

**不放心可执行文件的话，直接从源码构建**（见下方「开发」），自己编出来的包不带 quarantine 标记，不会被拦。

要求 macOS 13 及以上，Apple Silicon 与 Intel 均支持。

## 数据与隐私

这是一个纯本地工具，**没有账号体系、没有服务端、没有遥测、没有崩溃上报、没有第三方 SDK**。

**留在本机的数据**——股票列表、持仓数量与成本、提醒规则、显示偏好，明文 JSON 存放于：

```
~/Library/Application Support/com.neza.tickerbar/config.json
```

文件权限为 `0600`（仅当前系统用户可读写）。持仓数据不加密，仅靠文件系统权限保护——对单用户本机小工具是合理取舍，但请知悉。

**会发出去的请求**——只有两类，都发往腾讯公开行情接口：

| 用途 | 地址 | 携带内容 |
|------|------|----------|
| 拉取行情 | `https://qt.gtimg.cn/q=...` | 股票代码 |
| 搜索股票 | `https://smartbox.gtimg.cn/s3/...` | 你输入的关键词 |

请求不带 Cookie、不带用户标识、不需要登录。User-Agent 只包含应用名与版本号，不含用户名或设备信息。**持仓数量与成本永远不会离开本机**——收益全部在本地计算。

## 免责声明

行情来自腾讯免费公开接口，可能存在延迟、中断或错误，**仅供参考，不构成任何投资建议**。本工具不对数据准确性作任何担保，据此产生的任何投资决策与损失由使用者自行承担。

该接口为腾讯提供的公开服务，本项目仅作个人查询用途、未做任何规避或高频请求（固定 3 秒间隔、批量合并、失败指数退避）。接口的可用性与使用条款由腾讯决定，随时可能变化。

## 开发

```bash
pnpm install
pnpm tauri dev                # 开发模式
pnpm release                  # 构建 Universal 包（Apple Silicon + Intel）
```

构建产物在 `src-tauri/target/universal-apple-darwin/release/bundle/`。本地构建使用 ad-hoc 签名，仅供自用；对外分发需替换为 Apple Developer ID 签名并公证。

## 质量检查

```bash
pnpm test:coverage            # 前端测试 + 覆盖率
pnpm build                    # 类型检查 + 构建

cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo audit
```

## Windows 版？

目前只有 macOS 版，我也没有 Windows 机器，短期不会自己做。

如果你想做，[`docs/WINDOWS_PORT_PROMPT.md`](docs/WINDOWS_PORT_PROMPT.md) 是一份可以直接喂给 AI coding agent 的移植提示词：整理了全部平台差异、哪些代码碰不得、以及验收清单。

里面写清楚了最大的那道坎——**Windows 系统托盘不支持显示文字**（`tray-icon` crate 明确标注 `Windows: Unsupported`），所以「在菜单栏直接看到股价」这个核心形态需要重新设计，文档里给了三条路和各自的取舍。

## 技术栈

Tauri 2（Rust 后端 + Vue 3 前端），行情计算使用 `rust_decimal` 避免浮点误差。

## 许可

[MIT](LICENSE)
