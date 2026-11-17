# GithubDeck (Rust + Tauri)

GithubDeck 是一款面向 Windows 的 GitHub CLI 多账号桌面管理器。本目录包含两种 Rust 实现：**Tauri 2 + React**（`src-tauri/` 与 `src/`）和 **GPUI**（`gpui-version/`），两者共用 `ghdeck-core/`。：读取本机已安装的 GitHub CLI，并以图形界面完成状态刷新、账号切换、网页登录和本地登出，同时展示每个账号的令牌权限、绑定账号所在的浏览器，并识别在 GitHub 上改过名的账号。

统一项目说明和 Python/PySide6 实现请参阅仓库根目录的 [`README.md`](../README.md)。

## 功能

- 按 GitHub 主机显示本机 GitHub CLI 已保存的账号，标识当前活动账号、需要重新认证的账号和状态检查超时的账号。
- **令牌权限**：每个账号显示其令牌的 OAuth 权限（scopes），并附中文说明；`delete_repo`、`admin:*` 等高风险权限在卡片上直接标红，并给出移除命令。
- **浏览器绑定**：自动识别已安装的浏览器（Chrome / Edge / Brave / Vivaldi 识别到具体用户配置，Firefox 识别到 profile），可以为每个账号绑定一个，并一键在该浏览器中打开 GitHub。
- **在指定浏览器中授权**：登录新账号时先选浏览器，授权页直接在该浏览器打开，登录成功后新账号自动绑定这个浏览器；重新认证时使用账号已绑定的浏览器。
- **改名识别**：记录每个账号的 GitHub 用户 ID（ID 不随改名变化），按 ID 查询当前用户名；改过名的账号显示新名字，并标出本机记录的旧名字。“同步改名”会重新授权，确认是同一个 GitHub 账号后，再移除旧名下的记录。
- 切换账号、精确登出（二次确认）、取消等待中的登录。
- 通过系统文件对话框选择并校验自定义 `gh.exe` 路径。
## 技术栈

- `ghdeck-core/`：账号操作、浏览器探测、本地状态，被两种 UI 共用
- `src-tauri/` + `src/`：Rust / Tauri 2 后端与 React 19 + TypeScript + Vite 前端
- `gpui-version/`：Rust / GPUI 0.2 单可执行文件界面

## 系统要求

- Windows 10 或更高版本
- [GitHub CLI](https://github.com/cli/cli/releases/)
- 开发时需要：Node.js 18+、Rust stable、Visual Studio Build Tools（MSVC）

GithubDeck 默认查找：

```text
C:\Program Files\GitHub CLI\gh.exe
```

开发时使用 GitHub CLI 2.96.0 测试。其他能够通过 `gh.exe --version` 校验的 GitHub CLI 版本也可以使用。

## 从源码运行

```powershell
cd rust-version
npm install
npm run tauri dev

# GPUI 版（无需 Node，直接 cargo run）
cd gpui-version
cargo run --release
```

生产构建（Tauri 版）：

```powershell
npm run tauri build
```

GPUI 版构建：

```powershell
cd gpui-version
cargo build --release
```

产物为 `target/release/GithubDeck-GPUI.exe`，单个文件即可运行。

`build.rs` 会通过 `winresource` 把 `assets/githubdeck.ico` 编进 exe：Windows 需要一个 GUI 子系统的程序，否则启动时会附带一个控制台窗口；GPUI 还会按资源 ID 1 从 exe 自身读取窗口图标。`tests/packaging.rs` 会检查构建产物确实是 GUI 子系统并且带有图标资源：

```powershell
cargo build --release
cargo test --release --test packaging
```

主要构建产物：

- `GithubDeck.exe`
- `GithubDeck_0.3.0_x64_en-US.msi`
- `GithubDeck_0.3.0_x64-setup.exe`

发布到 `v0.3.0` Release 时分别命名为：

- `GithubDeck-Rust-Portable-v0.3.0.exe`
- `GithubDeck-Rust-v0.3.0-x64.msi`
- `GithubDeck-Rust-v0.3.0-x64-setup.exe`

文件校验值收录在 `SHA256SUMS.txt`。

## 测试

```powershell
npm test -- --run                                   # 前端
cargo test --manifest-path ghdeck-core/Cargo.toml   # 账号逻辑（使用模拟的 gh，不改动真实账号）
cargo test --manifest-path gpui-version/Cargo.toml  # GPUI 界面逻辑
cargo test --manifest-path src-tauri/Cargo.toml     # Tauri 配置检查
# 只读冒烟测试：调用真实 gh 执行 auth status / api user，并扫描浏览器
cargo test --manifest-path ghdeck-core/Cargo.toml real_gh_smoke -- --ignored --nocapture
```
## 使用方法

启动后，GithubDeck 会刷新本机 GitHub CLI 保存的账号，并核对每个账号在 GitHub 上的当前用户名：

- 点击账号卡片可展开详情：权限、绑定的浏览器，以及“同步改名”或“重新认证”入口。
- 选择非活动账号后，点击“切换到所选账号”。
- 点击“登录新账号”，选择授权用的浏览器，然后在打开的官方设备授权页中输入一次性授权码。
- 选择任意账号后，点击“登出所选账号”，并在确认窗口中明确确认。
- 点击“GitHub CLI 设置”，可以选择其他 `gh.exe`；保存前会校验文件名和版本输出（5 秒超时）。

账号的用户 ID 只能在它作为活动账号时，通过 `gh api user` 用它自己的令牌获得。从未在 GithubDeck 中作为活动账号刷新过的账号，切换到它一次之后才能识别改名。

登出执行精确的本地命令：

```text
gh auth logout --hostname HOST --user LOGIN
```

这只会移除本机 GitHub CLI 保存的认证配置，不会撤销 GitHub 端的 OAuth 授权。

## 安全边界

GithubDeck 不自行管理 GitHub 凭据：

- 不读取、保存或展示访问令牌，不调用 `gh auth token` 或 `--show-token`，不读取 Windows Credential Manager。
- 页面只接收整理后的状态；GitHub CLI 的原始输出不会发送到页面，也不会写入日志。
- 登录期间只以严格格式提取一次性设备授权码，登录结束、失败或取消后立即清除；网址和授权码只在用户点击复制按钮后写入剪贴板。
- 启动每个 GitHub CLI 子进程前，都会从环境中**移除** `GH_TOKEN`、`GITHUB_TOKEN`、`GH_ENTERPRISE_TOKEN`、`GITHUB_ENTERPRISE_TOKEN` 和 `GH_BROWSER`。
- 所有命令都以“程序路径 + 参数数组”的方式执行，不拼接 shell 字符串；只对上一次刷新列出的账号执行操作。
- 浏览器只会打开由后端拼出的固定 GitHub 地址（`https://<host>/`、`https://<host>/login/device`），只能打开探测到的浏览器，页面不能指定任意程序或网址。
- 改名识别通过 `gh api user` 和 `gh api user/<id>` 完成，只读取 `id` 和 `login` 两个字段。
- 关闭窗口时会终止等待中的 `gh auth login`，避免关闭后仍能在浏览器中完成授权。
- 本地只保存 `%APPDATA%\GhDeck\settings.json`（gh 路径）和 `%APPDATA%\GhDeck\accounts.json`（用户 ID、GitHub 当前用户名、浏览器绑定），不包含任何凭据。
## 当前限制

- 不自动下载、安装或升级 GitHub CLI。
- 不管理或粘贴个人访问令牌。
- 不撤销 GitHub 端 OAuth 授权。
- 不将账号永久绑定到特定 Git 仓库。
- 不管理 `git push` 使用的 HTTPS 凭据或 SSH 密钥。

## 与 Python 版的关系

仓库中的 `python-version` 是早期的 PySide6 实现，不包含令牌权限展示、浏览器绑定和改名识别。Rust 版是主实现。
