<p align="center">
  <img src="assets/githubdeck-icon.svg" width="112" height="112" alt="GithubDeck GD 图标">
</p>

# GithubDeck

GithubDeck 是一款面向 Windows 的 GitHub CLI 多账号桌面管理器。它读取本机 GitHub CLI 已认证账号，并提供账号状态刷新、活动账号切换、浏览器设备登录、精确本地登出和自定义 `gh.exe` 路径设置。

仓库包含三个桌面实现：

- **Rust / Tauri 2 / React**（主实现）：提供便携 EXE、MSI 和 NSIS 安装程序，推荐普通用户使用。
- **Rust / GPUI**：Zed 的 GPU 渲染 UI 框架，单个 EXE 免安装即可运行。账号逻辑与 Tauri 版共用同一个 `ghdeck-core`。
- **Python / PySide6**：早期实现，提供单文件便携 EXE。它没有下面标注“Rust 版”的功能。

## 功能

- 显示本机 GitHub CLI 保存的多个账号及认证状态。
- 标识当前活动账号，并切换到已登录的其他账号。
- 通过 GitHub CLI 官方设备授权流程登录新账号。
- 显示和复制一次性设备码，支持取消等待中的登录。
- 二次确认后只登出明确选中的账号。
- 配置并验证自定义 GitHub CLI 可执行文件。
- 异步运行 GitHub CLI 操作，避免阻塞桌面界面。
- Rust 版：显示每个账号令牌的权限（scopes），高风险权限标红。
- Rust 版：为账号绑定所在的浏览器（精确到 Chrome/Edge 的用户配置或 Firefox 的 profile）；授权页在该浏览器中打开。
- Rust 版：识别在 GitHub 上改过名的账号，显示新名字，并可把 GitHub CLI 的本机记录同步到新名字。

## 下载

从项目的 [Releases](https://github.com/just-liuyi/GithubDeck/releases) 页面选择对应资产。`v0.3.0` 提供 Rust 版：

| 资产 | 用途 |
| --- | --- |
| `GithubDeck-Rust-v0.3.0-x64-setup.exe` | Rust 版 NSIS 安装程序，推荐普通用户使用 |
| `GithubDeck-Rust-Portable-v0.3.0.exe` | Rust 版免安装便携程序 |
| `GithubDeck-Rust-v0.3.0-x64.msi` | Rust 版 Windows Installer 安装包 |
| `SHA256SUMS.txt` | 上述二进制资产的 SHA-256 校验值 |

Python/PySide6 单文件便携程序 `GithubDeck-Python-Portable-v0.2.0.exe` 在 `v0.2.0` Release 中提供。

两个实现均未进行商业代码签名，Windows SmartScreen 可能显示“未知发布者”。请只从本项目官方 Release 下载，并核对 SHA-256。

```powershell
Get-FileHash .\GithubDeck-Rust-v0.3.0-x64-setup.exe -Algorithm SHA256
```

## 系统要求

- Windows 10 或更高版本
- [GitHub CLI](https://github.com/cli/cli/releases/)

默认 GitHub CLI 路径：

```text
C:\Program Files\GitHub CLI\gh.exe
```

也可以在应用的“GitHub CLI 设置”中选择其他合法的 `gh.exe`。保存前会执行 `gh.exe --version` 校验。

## 使用方法

1. 启动 GithubDeck，等待账号状态刷新。
2. 选择非活动账号后，点击“切换到所选账号”。
3. 点击“登录新账号”（Rust 版会先让你选择浏览器），在 GitHub 官方设备授权页输入界面显示的一次性验证码。
4. 如需移除本机认证，选择账号并点击“登出所选账号”，然后完成二次确认。

登出只执行精确的本地 GitHub CLI 操作：

```text
gh auth logout --hostname HOST --user LOGIN
```

这不会撤销 GitHub 端的 OAuth 授权。`gh auth switch` 修改的是指定主机的全局活动账号，因此其他终端后续执行的 `gh` 命令也会使用新账号。

## 安全边界

GithubDeck 不自行管理 GitHub 凭据：

- 不读取、显示、保存或请求访问令牌。
- 不读取 Windows Credential Manager。
- 不调用 `gh auth token` 或 `--show-token`。
- 不向界面或日志输出 GitHub CLI 原始标准输出、错误输出或凭据。
- 登录期间只提取严格格式的一次性设备授权短码，并在结束、失败或取消后清除。
- 网址和授权码只在用户点击复制按钮后写入剪贴板。
- 每次启动 GitHub CLI 子进程都移除继承的 GitHub Token 环境变量和 `GH_BROWSER`。
- 所有 GitHub CLI 调用都使用程序路径与参数数组，不使用 shell 字符串拼接。

## 仓库结构

```text
assets/                      共享 GD 图标母图
docs/                        设计规范与实施计划
python-version/              Python 3.13 + PySide6 实现
rust-version/
  ghdeck-core/               共享核心：GitHub CLI 调用、浏览器、账号信息
  src-tauri/                 Tauri 2 后端（薄壳）
  src/                       Tauri 版的 React 前端
  gpui-version/              GPUI 版（薄壳）
```

## Python 版本开发

需要 Python 3.13 和 [uv](https://docs.astral.sh/uv/)。

```powershell
cd python-version
uv sync --dev
uv run python main.py
```

测试：

```powershell
uv lock --check
$env:QT_QPA_PLATFORM="offscreen"
uv run pytest -q
```

构建单文件 EXE：

```powershell
uv run pyinstaller --clean --noconfirm GithubDeck.spec
```

## Rust 版本开发

需要 Node.js 18+、Rust stable 和 Visual Studio Build Tools（MSVC）。

```powershell
cd rust-version
npm install
npm run tauri dev
```

测试与构建：

```powershell
npm test -- --run
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run tauri -- build
```

自动化测试使用替身，不执行真实账号切换、登录或登出。

## 当前限制

- 不自动下载、安装或升级 GitHub CLI。
- 不管理个人访问令牌。
- 不撤销 GitHub 端 OAuth 授权。
- 不管理 Git 推送使用的 HTTPS 凭据或 SSH 密钥。
- 不提供自动更新或商业代码签名。

## 许可证

本仓库暂未附带开源许可证。在许可证明确之前，源代码默认受版权所有者保留的权利约束。
