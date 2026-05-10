# 桌面端更新发布说明

Liberty 的桌面端更新链路分为两个平台：

- macOS：Sparkle + GitHub Releases
- Windows：Tauri updater + GitHub Releases

应用内提供统一的“检查更新”入口，启动后也会在后台自动检查一次。

## 应用内行为

- 系统设置页展示当前更新状态、平台、技术栈、最新版本、最近检查时间和发布说明
- 应用菜单中提供“检查更新”入口
- 启动后会静默执行一次后台检查
- macOS 上发现新版本时由 Sparkle 接管下载和安装
- Windows 上发现新版本时可直接在应用内下载并安装

## 需要配置的密钥

### Windows

- `LIBERTY_TAURI_UPDATER_PUBLIC_KEY`
- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

### macOS

- `LIBERTY_SPARKLE_PUBLIC_KEY`
- `LIBERTY_SPARKLE_PRIVATE_KEY`

### GitHub

发布工作流依赖仓库的 `GITHUB_TOKEN`，并从 GitHub Releases 读取更新产物。

## CI 产物

发布流程会生成并附带这些文件：

- `appcast.xml`：macOS Sparkle 更新源
- `latest.json`：Windows updater 更新源
- `.sig`：Windows 更新包签名文件

## 发布流程

1. 打标签或手动触发发布工作流
2. CI 构建 macOS 和 Windows 产物
3. macOS 阶段下载 Sparkle 工具并生成 `appcast.xml`
4. Windows 阶段生成并附加 `.sig`
5. 发布阶段把 `appcast.xml`、`latest.json` 和安装包一起上传到 GitHub Releases

## Sparkle 侧说明

macOS 包内通过 `Info.plist` 指向：

- `SUFeedURL` -> `https://github.com/<owner>/<repo>/releases/latest/download/appcast.xml`
- `SUPublicEDKey` -> 仓库配置的 Sparkle 公钥

CI 会在 macOS 机器上从 Sparkle 官方 Release 下载工具并调用 `generate_appcast` 生成 appcast。

## Windows 侧说明

Windows updater 读取：

- `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`

`latest.json` 由发布脚本基于本次 Release Tag、发布说明和 Windows 安装包签名生成。

## 本地验证

- `pnpm desktop:build:web`
- `SPARKLE_FRAMEWORK_PATH="$PWD/apps/desktop/src-tauri/vendor" cargo check -p liberty`（macOS 需要提供本地 `Sparkle.framework` 路径）
