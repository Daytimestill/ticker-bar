# Windows 版移植提示词

把下面整段交给 AI coding agent（Claude Code / Cursor / Codex 等）即可开工。

本文档由 macOS 版作者整理，所有平台断言都来自实际读依赖源码，不是凭印象写的。核实方式附在各条后面，你可以自己复核。

---

## 复制以下内容给 agent

我要把 TickerBar 移植到 Windows。这是一个 Tauri 2 应用，Rust 后端 + Vue 3 前端，原本只面向 macOS。

### 先建立认知

按顺序读这几个文件，不要跳：

| 文件 | 为什么要读 |
|------|-----------|
| `README.md` | 产品是什么、数据边界在哪 |
| `src-tauri/src/runtime/tray.rs` | 托盘/菜单栏的全部实现，移植的主战场 |
| `src-tauri/src/runtime/mod.rs` | 启动流程、插件注册、唯一的 `cfg(target_os = "macos")` 分支 |
| `src-tauri/src/storage.rs` | 配置读写与文件权限 |
| `src-tauri/tauri.conf.json` + `src-tauri/Info.plist` | 打包配置与 macOS 专有声明 |

### 唯一的硬阻塞：Windows 托盘不显示文字

这个应用的核心形态是「在菜单栏直接显示股价文字」。macOS 支持给托盘项设置标题，Windows **不支持**。

`tray-icon` crate 源码里对 `set_title` 的平台说明写得很明确：

```
/// ## Platform-specific:
/// - **Windows:** Unsupported
pub fn set_title<S: AsRef<str>>(&self, title: Option<S>)
```

核实方式：`cargo tree -p tray-icon` 找到版本后读它的 `src/lib.rs`。

所以 `tray.rs` 里这段在 Windows 上是空操作：

```rust
tray.set_title(Some(&title))
```

**不要试图绕过它，也不要假装它能用。** 你需要在下面三条路里选一条，动手前先向用户说明取舍：

**方案 A — 动态渲染图标（最接近原形态）**
把价格文字画进一张位图，用 `set_icon()` 每次刷新时替换。托盘图标在 100% 缩放下约 16×16，高 DPI 下可到 32×32，实际只能塞下 4–5 个字符。适合只显示涨跌幅（如 `+3.2%`），塞不下完整价格。必须处理 DPI 缩放，否则高分屏上糊成一团。

**方案 B — 图标 + 悬停提示（最省事，体验最弱）**
托盘只放一个随涨跌变色的图标，完整文字放进 tooltip 和下拉菜单。`set_tooltip()` 在 Windows 上是支持的，而且代码里已经在用了（`tray.rs` 的 `refresh_tooltip`），改动量最小。代价是不悬停就看不见数字，等于丢掉了产品的核心卖点。

**方案 C — 独立浮窗小组件（体验最好，工作量最大）**
做一个无边框、置顶、可拖动的小窗口常驻桌面角落，托盘只留右键菜单。要自己处理位置记忆、多显示器、全屏应用时的避让。

**建议：先做 B 保证能跑通全流程，再叠加 A 或 C。** 不要一上来就啃 A/C，否则调半天渲染却发现别处编译不过。

### 逐项平台差异

| 位置 | macOS 现状 | Windows 要做什么 |
|------|-----------|-----------------|
| `runtime/mod.rs` | `set_activation_policy(Accessory)` 隐藏 Dock 图标，已被 `cfg(target_os = "macos")` 包住 | 无需改这行。Windows 侧改为给窗口设 `skipTaskbar: true`，避免设置窗占任务栏 |
| `src-tauri/Info.plist` | `LSUIElement = true`，声明无 Dock 后台应用 | Windows 不读这个文件，保留即可，不要删 |
| `storage.rs:47` | `#[cfg(unix)]` 把配置文件设为 `0600` | **已经 cfg 隔离，能编过，但 Windows 上完全没有权限保护**。持仓数据明文躺在 `%APPDATA%`。要么用 ACL 收紧（`icacls` 或 `windows-acl` crate），要么在 README 里如实写明这个差异。⛔ 不要装作已经保护了 |
| `runtime/mod.rs:140` | `app_config_dir()` 解析到 `~/Library/Application Support/<id>/` | Tauri 自动解析为 `%APPDATA%\<id>\`，**代码不用改**。但要实测一次确认目录真的建出来了 |
| `tauri-plugin-autostart` | macOS 登录项 | 该插件自带 `#[cfg(windows)]` 分支，走注册表 Run 键，理论上直接可用。**必须实测**：装包后重启机器验证，别只看代码 |
| `tauri-plugin-notification` | 走 `NSUserNotificationCenter` | Windows 走 WinRT Toast。**关键前提：Toast 需要开始菜单快捷方式和 AppUserModelID，绿色免安装版收不到通知。** 必须用 NSIS/MSI 安装包测，不能只 `cargo run` |
| `AlertSection.vue` 的试发倒计时 | 倒数 3 秒是为了让用户切走窗口——macOS 不给前台 App 弹横幅 | **Windows 没有这个限制**，前台也照弹。倒计时和相关文案应当去掉或改写，否则是在解释一个不存在的问题 |
| `AlertToasts.vue` 窗口内 Toast | 为绕开 macOS 前台抑制而做 | Windows 上仍然有用（设置窗开着时更醒目），建议保留 |
| `tauri.conf.json` bundle | `targets: ["app", "dmg"]`、`macOS.minimumSystemVersion`、`signingIdentity` | 改为 `["nsis"]` 或 `["msi"]`。macOS 段落保留不动，Tauri 按平台各取所需 |
| 构建脚本 `package.json` | `pnpm release` 打 universal（Apple Silicon + Intel） | 加一条 Windows 构建命令，目标 `x86_64-pc-windows-msvc`，有余力再加 `aarch64-pc-windows-msvc` |
| `.gitignore` | 有一条 `Icon[$'\r']` 排除 macOS Finder 图标缓存 | 与 Windows 无关，保留。⛔ 不要改成 `Icon?`——`?` 是通配符，会把 `src-tauri/icons/icon.png` 一起吞掉，导致克隆后构建失败（这个坑踩过） |

### 绝对不要动的部分

这些是纯业务逻辑，和操作系统无关。改了就是在制造回归：

- `src-tauri/src/domain/`、`portfolio.rs`、`alerts.rs` —— 交易时段判定、持仓收益、提醒穿越语义
- `provider.rs` —— 腾讯行情接口的请求与解析
- `config.rs` 的 schema 与版本迁移 —— **配置格式必须与 macOS 版保持一致**，同一个人可能两台机器都用
- 所有金额计算里的 `rust_decimal` —— ⛔ 不要为了图省事换成 `f64`，那会引入浮点误差
- 多币种合计**不跨币种相加**的设计 —— 这是刻意为之（没有汇率数据源），不是遗漏

### 验收标准

代码写完不等于做完，逐条实测：

- [ ] `cargo clippy --all-targets -- -D warnings` 在 Windows 上干净
- [ ] `cargo test` 与 `pnpm vitest run` 全绿（现有 62 + 88 条测试**一条都不许删**，platform-specific 的用 `cfg` 隔离）
- [ ] 托盘图标出现在通知区域，数值随行情刷新而变化
- [ ] 右键菜单能列出全部股票、切换置顶、显示多币种合计
- [ ] 设置窗能开能关，关掉后进程仍在后台运行
- [ ] 提醒触发时弹出 Windows Toast（**用安装包测，不是 `cargo run`**）
- [ ] 「登录时启动」开关重启机器后真的生效
- [ ] 配置文件落在 `%APPDATA%`，删掉后能重建默认配置
- [ ] 断网后能恢复、休眠唤醒后能恢复
- [ ] 高 DPI 缩放（125% / 150% / 200%）下托盘和设置窗都不糊

### 工作方式要求

- 改代码前先说明改哪、为什么，尤其是托盘方案的选型，让用户拍板再动手
- 保持现有的中文注释风格：解释**为什么这么写**，不要写「设置标题」这种复述代码的废话
- 平台差异一律用 `#[cfg(target_os = "windows")]` 隔离，⛔ 不要删 macOS 分支——这个仓库要同时维护两个平台
- 遇到 Windows 平台行为拿不准的（Toast 送达条件、注册表自启、DPI），**去读依赖 crate 的源码或官方文档核实，不要凭印象断言**

---

## 给移植者的话

macOS 版的完整状态见 [`TODO.md`](TODO.md)。已知尚未完成的通用项（不分平台）：长期挂机稳定性验证、腾讯接口的可用性观察、备用数据源。

托盘不能显示文字这件事，是这个产品在 Windows 上最大的形态挑战。如果你最后选了方案 C 做出了好用的浮窗，欢迎提 PR 回来——那个形态在 macOS 上同样有人想要。
