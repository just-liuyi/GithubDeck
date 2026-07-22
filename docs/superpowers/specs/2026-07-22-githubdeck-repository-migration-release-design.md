# GithubDeck 统一仓库迁移与 v0.2.0 发布设计

## 目标

删除现有公开仓库 `FeiJiZai/ghdeck`，将当前工作区整理为同时包含 Python/PySide6 与 Rust/Tauri 两种实现的统一源码仓库，创建新的公开仓库 `FeiJiZai/GithubDeck`，重写根 README，并发布包含两种实现的 `v0.2.0` Release。

本次迁移不新增应用功能，不修改 GitHub CLI 账号读取、切换、登录、取消、登出或凭据安全边界。

## 已确认决策

- 采用全新统一 monorepo，不直接重命名旧仓库。
- 删除旧公开仓库前创建本地 Git bundle 和旧 Release 元数据备份。
- 新仓库名称为 `FeiJiZai/GithubDeck`，可见性为 Public。
- 使用 `master` 作为默认分支，不创建额外功能分支。
- 新仓库以当前工作区根目录作为 Git 根目录。
- 新仓库首次 Release 为稳定版 `v0.2.0`，不是 draft 或 prerelease。
- 发布前将 Python project/QApplication 版本，以及 Rust package/Tauri/package.json 版本统一升级为 `0.2.0`，锁文件和安装包内部元数据必须同步。
- Python 和 Rust 版本在同一 Release 中使用可区分的资产名称。
- 不保留旧仓库的远程提交历史、Release、Issue 或下载统计；本地备份仅用于灾难恢复，不推送到新仓库。

## 新仓库结构

```text
GithubDeck/
├── README.md
├── .gitignore
├── assets/
│   └── githubdeck-icon.svg
├── docs/
│   └── superpowers/
├── python-version/
│   ├── github_cli_manager/
│   ├── tests/
│   ├── assets/
│   ├── GithubDeck.spec
│   ├── pyproject.toml
│   ├── uv.lock
│   └── README.md
└── rust-version/
    ├── src/
    ├── public/
    ├── src-tauri/
    ├── package.json
    ├── package-lock.json
    └── README.md
```

根仓库只跟踪源码、锁文件、测试、文档、构建配置和图标资产。以下内容必须由根 `.gitignore` 排除：

- Python `.venv/`、`__pycache__/`、`.pytest_cache/`、`build/`、`dist/`；
- Rust `target/`；
- Node `node_modules/` 和前端 `dist/`；
- IDE、操作系统、日志、覆盖率和临时文件；
- Release 暂存目录、Git bundle、旧 Release 元数据备份；
- 用户设置、本机 GitHub CLI 配置、Token、凭据或环境文件。

现有 `python-version/.git` 不直接作为新仓库继续使用。准备统一根仓库时先保留旧 Git 历史备份，再移除嵌套 Git 元数据并从根目录初始化新仓库，避免 `python-version` 被记录为嵌套仓库或 gitlink。

## 根 README 设计

根 `README.md` 使用简体中文，服务于下载用户和开发者，不复制两份子目录 README 的全部内容。结构如下：

1. GithubDeck 简介和 GD 图标；
2. 功能摘要；
3. Python 与 Rust 版本对比；
4. 推荐下载方式；
5. GitHub CLI 与 Windows 前置要求；
6. 使用方法；
7. 安全边界；
8. 源码目录说明；
9. 两端开发、测试和构建命令；
10. SmartScreen、未签名程序和 SHA-256 校验说明；
11. License 状态说明。

下载建议：

- 普通用户优先选择 Rust NSIS 安装程序；
- 免安装用户选择 Rust Portable；
- 需要 Python/PySide6 实现的用户选择 Python Portable；
- MSI 提供给偏好 Windows Installer 或集中部署的用户。

README 不声称程序已进行代码签名，不添加不存在的截图、CI、许可证、自动更新或安全认证声明。若项目当前没有 LICENSE，README 明确说明“暂未附带开源许可证”，不得擅自选择许可证。

## v0.2.0 Release 资产

Release 标题：

```text
GithubDeck v0.2.0
```

Release 必须包含且只包含以下五个资产：

```text
GithubDeck-Python-Portable-v0.2.0.exe
GithubDeck-Rust-Portable-v0.2.0.exe
GithubDeck-Rust-v0.2.0-x64.msi
GithubDeck-Rust-v0.2.0-x64-setup.exe
SHA256SUMS.txt
```

资产来源：

- Python Portable 来自内部版本已更新为 `0.2.0` 的 `python-version/dist/GithubDeck.exe`；
- Rust Portable 来自内部版本已更新为 `0.2.0` 的 `rust-version/src-tauri/target/release/GithubDeck.exe`；
- Rust MSI 来自 `GithubDeck_0.2.0_x64_en-US.msi`；
- Rust Setup 来自 `GithubDeck_0.2.0_x64-setup.exe`。

上传前把四个二进制复制到独立的本地 Release 暂存目录并重命名，不修改构建目录内的原始产物。`SHA256SUMS.txt` 使用小写 SHA-256，逐行记录四个二进制文件名；发布后重新下载或查询 GitHub 资产摘要并与本地摘要核对。

Release 正文先写入本地 `release-staging/v0.2.0/RELEASE_NOTES.md`，该文件只作为 `gh release create --notes-file` 的输入，不上传为 Release 资产，并由根 `.gitignore` 排除。正文包含：

- GithubDeck 重命名；
- Python 与 Rust/Tauri 双实现；
- 四个资产的用途和推荐顺序；
- GitHub CLI 前置要求；
- 未签名与 SmartScreen 提示；
- SHA-256 校验命令；
- 安全边界摘要；
- 不包含凭据、Token 或用户配置的声明。

## 构建与发布前验证

在任何远程删除发生前完成以下检查：

### Python

- `uv lock --check`；
- 完整 pytest，使用 offscreen Qt；
- PyInstaller `GithubDeck.spec` 干净构建；
- `dist/GithubDeck.exe` 存在、非空、可启动，窗口标题为 `GithubDeck`；
- 正常关闭验证实例，不执行真实账号操作。

### Rust/Tauri

- 前端 Vitest；
- TypeScript/Vite production build；
- `cargo test --locked`；
- Tauri production build；
- GithubDeck EXE、`GithubDeck_0.2.0_x64_en-US.msi`、`GithubDeck_0.2.0_x64-setup.exe` 存在且非空；
- EXE 可启动，窗口标题为 `GithubDeck`，并正常关闭；
- 固定 GitHub CLI Releases URL capability 测试继续通过。

### 源码与安全审查

- 根 `.gitignore` 能排除所有构建产物、依赖目录和备份目录；
- `git status` 中没有 `.env`、Token、凭据、本机路径配置、用户设置或缓存；
- 没有把旧 `python-version/.git` 作为嵌套仓库提交；
- 新提交不包含 EXE、MSI、NSIS、`node_modules`、`.venv` 或 `target`；
- 当前用户可见名称均为 GithubDeck；兼容性内部标识按已有设计保留。

## 旧仓库备份与破坏性操作顺序

删除旧仓库是不可逆的共享状态操作，必须使用以下顺序：

1. 读取并保存旧仓库基本元数据、默认分支、最新提交、Release 正文和资产清单；
2. 从现有 `python-version` Git 仓库创建包含所有 refs 的本地 Git bundle；
3. 验证 bundle 可被 Git 列出并验证完整性；
4. 完成新根仓库文件准备、测试、构建、Release 暂存和安全审查；
5. 初始化根 Git 仓库，创建 `master` 首次提交；
6. 在远程删除前再次确认 `FeiJiZai/GithubDeck` 不存在，防止覆盖同名仓库；
7. 删除 `FeiJiZai/ghdeck`；
8. 立即创建公开 `FeiJiZai/GithubDeck`；
9. 将新仓库 `origin` 设置为新的 HTTPS 地址并推送 `master`；
10. 验证远程仓库可见性、默认分支和提交；
11. 创建 `v0.2.0` Release 并上传五个资产；
12. 验证 Release 非草稿、非预发布，资产名称、数量、大小和摘要正确。

不使用 force push，不重写新仓库历史，不跳过 Git hooks。删除旧仓库时只使用 GitHub 官方 CLI/API，并以精确 owner/name 定位，禁止模糊匹配其他仓库。

## 失败处理

- 若备份或 bundle 验证失败：停止，不删除旧仓库。
- 若构建、测试或安全审查失败：停止，不删除旧仓库。
- 若 `FeiJiZai/GithubDeck` 已存在：停止，不删除旧仓库，先报告冲突。
- 若删除旧仓库后新仓库创建失败：保留本地根仓库、bundle 和 Release 暂存资产，报告失败；不得删除其他仓库或强制覆盖。
- 若源码推送失败：保留新远程空仓库和本地提交，修复认证或网络问题后正常重试，不 force push。
- 若 Release 创建或上传失败：保留已推送源码和本地资产，修复后继续同一 `v0.2.0` 发布；不得创建多个同版本 Release。
- 若部分资产上传成功：查询现有 Release 资产，只补传缺失或明确替换错误资产，不盲目重复上传。

## 完成标准

只有以下条件全部满足才可报告完成：

- `FeiJiZai/ghdeck` 已不存在；
- `FeiJiZai/GithubDeck` 存在且为 Public；
- 默认分支为 `master`；
- 根仓库包含 Python、Rust、共享资产、文档和新的根 README；
- 远程源码不包含依赖目录、构建产物、备份、Token、凭据或用户配置；
- `v0.2.0` 为已发布稳定 Release；
- Release 恰好包含四个区分版本的二进制资产和一个 `SHA256SUMS.txt`；
- 远程资产大小非零且摘要与本地一致；
- Python 与 Rust 的测试、生产构建和窗口烟雾测试均通过；
- 未执行真实 GitHub 账号登录、切换或登出；
- 返回新仓库与 Release 的公开地址。
