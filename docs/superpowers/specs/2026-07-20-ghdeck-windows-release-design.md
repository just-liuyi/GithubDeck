# GhDeck Windows 打包与首次发布设计

## 目标

将现有 GitHub CLI 多账号管理器正式命名为 **GhDeck**，打包为无需安装 Python 的 Windows 单文件程序，并通过公开 GitHub 仓库发布首个 `v0.1.0` Release。

首版发布面向 Windows 10 及更高版本。用户下载 `GhDeck.exe` 后即可启动，但本机仍需安装 GitHub CLI；GhDeck 不捆绑、不下载也不静默安装 `gh.exe`。

## 产品与版本标识

- 产品名：`GhDeck`
- 仓库名：`ghdeck`
- GitHub 所有者：`FeiJiZai`
- 应用程序文件名：`GhDeck.exe`
- Python 包名：保留 `github_cli_manager`，避免与本次发布无关的内部重命名
- 首个版本：`0.1.0`
- Git 标签与 Release：`v0.1.0`
- 仓库可见性：公开
- 默认分支：沿用当前默认分支，不创建功能分支

应用窗口标题和 Qt 应用标识更新为 GhDeck，现有账号管理功能、行为和安全边界保持不变。

## 打包方案

使用 PyInstaller 生成 Windows 单文件、无控制台窗口的 `GhDeck.exe`。PyInstaller 作为开发依赖纳入 `pyproject.toml` 和 `uv.lock`，构建过程可通过固定命令复现。

打包入口继续使用 `main.py`。构建配置应明确：

- 单文件模式；
- GUI 模式，不显示命令行窗口；
- 输出文件名为 `GhDeck.exe`；
- 清理并隔离构建产物，避免提交临时目录；
- 包含 PySide6 运行所需的 Qt 插件；
- 不包含访问令牌、GitHub CLI 配置、Windows Credential Manager 数据、QSettings 用户数据、测试缓存或本机路径配置。

首版不制作 MSI/安装向导，不进行代码签名，也不提供目录版 ZIP。未签名 EXE 可能触发 Windows SmartScreen 提示，README 和 Release 说明应如实告知。

## 发布资产

Release `v0.1.0` 提供两个文件：

1. `GhDeck.exe`
2. `GhDeck.exe.sha256`

校验文件使用标准格式记录 EXE 的 SHA-256 摘要。Release 正文包含版本摘要、主要功能、安装要求、下载与校验方式、安全边界和已知限制。

## README

将现有 README 更新为 GhDeck 的公开项目首页，使用简体中文并包含：

- 产品定位和核心功能；
- Windows 10+ 与 GitHub CLI 前置要求；
- 从 GitHub Release 下载 `GhDeck.exe` 的步骤；
- 首次启动及 GitHub CLI 路径设置；
- 刷新、切换、登录和登出账号的使用说明；
- 登出仅修改本机 GitHub CLI 认证配置、不撤销 GitHub OAuth 授权的说明；
- 不读取、不保存、不显示 Token 的安全边界；
- Windows SmartScreen 对未签名程序可能出现的提示；
- 源码开发、测试与构建命令；
- 当前功能范围与限制。

README 不引用尚未生成的截图，也不声称 EXE 已签名或通过第三方安全认证。

## 仓库内容与忽略规则

当前 `python-version` 目录作为仓库根目录。首次提交包含应用源码、测试、依赖锁文件、README、构建配置和必要的发布元数据。

`.gitignore` 必须排除：

- `.venv/`
- `.pytest_cache/`
- `__pycache__/`
- `build/`
- `dist/`
- PyInstaller 中间文件
- IDE、本机缓存和临时文件
- 任何用户级 GitHub CLI 或 GhDeck 配置

构建后的 EXE 不直接提交进 Git 历史，只作为 GitHub Release 附件上传。

## 验证流程

发布前按以下顺序验证：

1. `uv lock --check` 通过；
2. 完整 pytest 测试通过；
3. PyInstaller 构建成功并只产生预期的单文件 EXE；
4. 对 EXE 计算 SHA-256 并生成校验文件；
5. 启动构建后的 EXE，确认窗口可以创建且进程不会因缺少 Python 环境而立即崩溃；
6. 验证仓库状态，确认没有 Token、凭据、本机配置或构建临时文件进入提交；
7. 创建首次提交并推送到公开仓库；
8. 创建 `v0.1.0` Release，上传 EXE 和校验文件；
9. 从 GitHub 查询 Release 元数据，确认标签、资产名称、资产数量和发布状态正确。

启动验证不得自动执行真实账号切换、登录或登出。测试也不得调用真实的账号变更命令。

## 发布操作边界

创建公开 GitHub 仓库、推送提交和发布 Release 都会产生外部可见状态。用户已明确要求执行这些操作；实现阶段仍应在推送前检查待提交文件，在发布前检查最终资产与版本信息。

不覆盖任何同名远程仓库。如果 `FeiJiZai/ghdeck` 在创建时已存在，应停止并报告冲突，而不是强制推送或删除远程内容。

## 完成标准

当以下条件全部满足时，首次发布完成：

- 本地测试和锁文件检查通过；
- `GhDeck.exe` 可独立启动；
- SHA-256 校验文件与 EXE 匹配；
- README 准确描述 GhDeck、安装要求、使用方式和安全边界；
- 公开仓库 `FeiJiZai/ghdeck` 已创建并包含首个提交；
- `v0.1.0` Release 已发布；
- Release 包含且仅包含 `GhDeck.exe` 与 `GhDeck.exe.sha256`；
- 未提交或发布任何凭据、Token、用户配置或无关构建文件。
