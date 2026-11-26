# TickerBar 发布检查表

## 1. 构建候选包

确认 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock` 和 `src-tauri/tauri.conf.json` 的应用版本一致。只打包准备发布的提交，不上传用户配置、调试构建或本地分析目录。

```bash
pnpm install --frozen-lockfile
pnpm test:coverage
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pnpm release -- --locked
```

在 macOS 本机运行打包，DMG 步骤需要能挂载和卸载磁盘镜像。若 Finder 美化阶段导致镜像无法卸载，可以先卸载打包留下的临时卷，再用 `CI=true pnpm release -- --locked` 跳过 Finder 布局脚本；验证 DMG 里仍有「应用程序」链接。产物在 `src-tauri/target/universal-apple-darwin/release/bundle/`。验证 DMG 完整性；再以只读方式挂载，检查 DMG 内的 App（而不只是构建目录里的副本）：

- `hdiutil verify`：镜像校验通过；
- `lipo -archs`：主二进制包含 `arm64` 与 `x86_64`；
- `codesign --verify --deep --strict`：签名结构通过验证；ad-hoc 验证通过不等于已公证；
- Info.plist 中版本、最低 macOS 版本与发布说明一致；
- 对最终 DMG 用 `shasum -a 256` 生成 `SHA256SUMS.txt`，和安装包一起提供。下载者可在两文件所在目录用 `shasum -a 256 -c SHA256SUMS.txt` 校验。

## 2. 人工验收

| 场景 | 通过标准 |
| --- | --- |
| 干净环境首次安装 | 从下载的候选包拖入「应用程序」，能启动并显示首次引导。记录 macOS 版本、芯片、签名状态和实际提示 |
| 设置和菜单栏入口 | 搜索名称/代码、选择股票、填写演示持仓、保存；关闭设置后菜单栏仍在运行 |
| 小屏和速览 | 不外接显示器时，菜单栏项目被挤走仍可用快捷键；弹窗四角不露矩形边或双描边；Esc 与右上角设置入口有效 |
| 覆盖升级、重启 | 先备份旧配置。替换旧 App 后，持仓、显示顺序、提醒仍在；开启登录启动，重启后只运行新版，且一次普通重启不轮换备份 |
| 行情和提醒 | 断网后重连、睡眠后唤醒、跨交易日都能恢复；试发通知可到达，真实阈值在合适交易时段触发 |

至少记录一台干净安装环境和实际测试的系统/芯片组合。通用二进制通过构建不是 Intel 实机或 macOS 13 实机通过。另按[待办清单](TODO.md)观察 5 个交易日；未经验证的组合标为「未实测」。

## 3. 发布到 GitHub

先确认测试分支的 CI 通过，再确定对应的提交和版本号。没有 Developer ID 时，只发布明确标记为**预发布**的测试包，不设为最新稳定版。上传最终 DMG 和 SHA-256 校验文件，发布说明须注明公证状态、已实测系统和剩余限制。先创建草稿核对附件和下载说明，确认后再公开。

正式分发使用项目维护者的 Developer ID Application 签名身份和 Apple 公证凭证。按 [Tauri 的签名与公证文档](https://v2.tauri.app/distribute/sign/macos/)配置环境变量，不把证书、密码、私钥或配置文件提交到仓库。发布前对最终包执行 `xcrun stapler validate`，并验证干净系统能通过 Gatekeeper。不要为了通过验收而关闭系统保护。
