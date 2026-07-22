# GhDeck Windows 打包与首次发布实施计划

## 目标

按照 `docs/superpowers/specs/2026-07-20-ghdeck-windows-release-design.md`，将 `python-version` 中的应用整理为 GhDeck `v0.1.0`，构建并验证单文件 Windows GUI 程序，然后在当前默认分支创建首次提交，推送到新的公开仓库 `FeiJiZai/ghdeck`，最后发布 Release。

## 任务 1：更新产品与项目元数据

涉及文件：

- `python-version/pyproject.toml`
- `python-version/github_cli_manager/app.py`
- `python-version/github_cli_manager/main_window.py`
- 必要的现有测试文件

步骤：

1. 将项目名、版本和描述更新为 GhDeck `0.1.0`。
2. 将 PyInstaller 加入开发依赖并更新 `uv.lock`。
3. 将 Qt 应用名及窗口标题更新为 GhDeck，同时保留稳定的组织名，避免无关配置迁移。
4. 调整依赖应用名称的测试断言。

## 任务 2：补全发布忽略规则与 README

涉及文件：

- `python-version/.gitignore`
- `python-version/README.md`

步骤：

1. 排除 `.venv/`、缓存、覆盖率文件、`build/`、`dist/`、PyInstaller spec 中间文件、IDE 和系统临时文件。
2. 将 README 改为 GhDeck 项目首页，准确描述下载、前置要求、使用方式、安全边界、SmartScreen 提示、开发测试和构建命令。
3. 不添加不存在的截图、签名或安全认证声明。

## 任务 3：添加可复现的单文件构建配置

涉及文件：

- `python-version/GhDeck.spec`

步骤：

1. 以 `main.py` 为入口配置 PyInstaller。
2. 使用 `onefile` GUI 模式，关闭控制台窗口，输出 `GhDeck.exe`。
3. 让 PyInstaller 自动收集 PySide6 所需插件，不嵌入用户配置或 GitHub CLI 数据。
4. 将 spec 文件作为源码提交，但不提交 `build/`、`dist/` 或 EXE。

## 任务 4：本地验证与构建

步骤：

1. 运行 `uv lock --check`。
2. 运行完整 pytest 套件。
3. 通过固定命令执行 PyInstaller 干净构建。
4. 检查 `dist/GhDeck.exe` 是唯一预期程序资产。
5. 启动 EXE，确认窗口进程能够运行；随后正常关闭验证实例，不触发真实登录、切换或登出。
6. 生成 `dist/GhDeck.exe.sha256`，重新计算并比对摘要。
7. 检查构建日志，不把未知的原始 GitHub CLI 输出或凭据写入发布文件。

## 任务 5：提交前审查与独立验证

步骤：

1. 审查全部待提交文件和 diff。
2. 搜索明显的 Token、凭据、本机配置和构建产物，确认它们未进入版本控制。
3. 对非平凡多文件改动执行独立验证，覆盖测试、构建、启动、校验和发布配置。
4. 修复验证发现的问题，直至通过。

## 任务 6：创建首次提交并公开推送

步骤：

1. 再次确认 `FeiJiZai/ghdeck` 不存在，避免覆盖远程内容。
2. 仅暂存源码、测试、README、锁文件和构建配置。
3. 在当前默认分支创建首次提交，不新建分支、不改写历史。
4. 使用 GitHub CLI 创建公开仓库 `FeiJiZai/ghdeck`，设置描述并推送当前分支。
5. 查询远程仓库，确认可见性、默认分支和提交均正确。

## 任务 7：发布并验证 v0.1.0

步骤：

1. 使用已验证的 `dist/GhDeck.exe` 和 `dist/GhDeck.exe.sha256` 创建 `v0.1.0` Release。
2. Release 正文包含功能摘要、GitHub CLI 前置要求、安全边界、SmartScreen 说明和校验命令。
3. 查询 Release 元数据，确认不是草稿或预发布版本。
4. 确认 Release 资产只有 `GhDeck.exe` 与 `GhDeck.exe.sha256`，且大小非零。
5. 返回公开仓库和 Release 地址。

## 完成约束

- 不执行真实 GitHub 账号切换、登录或登出。
- 不提交或发布 Token、凭据、QSettings 用户数据、本机 GitHub CLI 配置、缓存或构建临时目录。
- 不创建额外分支。
- 不强制推送、不覆盖同名仓库、不跳过 Git hooks。
- 若仓库创建、推送或 Release 发布失败，保留本地已验证产物并准确报告失败阶段。
