# GithubDeck 统一仓库迁移与 v0.2.0 发布实施计划

## 目标与约束

依据 `docs/superpowers/specs/2026-07-22-githubdeck-repository-migration-release-design.md`，把当前工作区整理为统一 monorepo，删除旧公开仓库 `FeiJiZai/ghdeck`，创建并推送公开仓库 `FeiJiZai/GithubDeck`，重写根 README，并发布同时包含 Python 与 Rust 构建产物的稳定版 `v0.2.0`。

必须遵守：

- 使用当前工作区根目录作为新 Git 仓库根目录；
- 默认分支为 `master`，不创建额外功能分支；
- 删除旧仓库前完成备份、本地提交、测试、生产构建、发布资产和安全审查；
- 不使用 force push、不跳过 hooks、不上传凭据、用户配置、依赖目录或构建目录；
- 不执行真实 GitHub 登录、账号切换或登出；
- Release 恰好包含四个区分实现的二进制资产和一个校验和文件。

## 阶段 1：记录并备份旧仓库

### 1.1 保存旧远程元数据

使用 GitHub CLI 读取并保存到根目录之外的临时备份位置：

- `FeiJiZai/ghdeck` 基本信息、可见性、默认分支和描述；
- `master` 最新提交 SHA；
- `v0.1.0` Release 的标题、正文、标签、状态和资产清单；
- 旧 Release 资产名称、大小和 GitHub 摘要。

备份文件不得进入新 Git 仓库。

### 1.2 创建 Git bundle

在 `python-version` 现有 Git 仓库运行：

```powershell
git bundle create "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager-migration-backup\ghdeck-before-migration.bundle" --all
git bundle verify "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager-migration-backup\ghdeck-before-migration.bundle"
git bundle list-heads "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager-migration-backup\ghdeck-before-migration.bundle"
```

验收：

- bundle 存在且非空；
- `verify` 成功；
- 包含旧 `master` ref 和提交；
- bundle 位于新根仓库之外或被根 `.gitignore` 明确排除。

任一步失败立即停止，禁止删除旧远程仓库。

## 阶段 2：准备统一根仓库文件

### 2.1 创建根 `.gitignore`

新增：`.gitignore`

必须排除：

```gitignore
# Python
**/.venv/
**/__pycache__/
**/.pytest_cache/
**/*.py[cod]
python-version/build/
python-version/dist/

# Node / frontend
**/node_modules/
rust-version/dist/

# Rust / Tauri
rust-version/src-tauri/target/

# Release staging and backups
release-staging/
*.bundle
*release-metadata*.json

# Test / coverage / logs
.coverage
htmlcov/
coverage/
*.log

# Local settings and secrets
.env
.env.*
!.env.example
*.local.json

# Editor / OS
.vscode/
.idea/
.DS_Store
Thumbs.db
```

若现有子目录 `.gitignore` 有额外有效规则，根规则与其并存，不删除安全性更强的子目录规则。

### 2.2 重写根 README

新增：`README.md`

内容按照设计规范固定为：

1. 项目标题、共享 GD 图标和简介；
2. 功能摘要；
3. Python 与 Rust 版本对比表；
4. `v0.2.0` 四种下载资产及推荐顺序；
5. Windows 10+ 与 GitHub CLI 前置要求；
6. 使用方法；
7. 安全边界；
8. monorepo 目录说明；
9. Python 测试、运行、构建命令；
10. Rust 前端测试、后端测试、运行、构建命令；
11. SmartScreen、未签名程序、SHA-256 校验；
12. “暂未附带开源许可证”的准确说明。

README 中不得：

- 声称已有代码签名、CI、自动更新或许可证；
- 链接旧仓库 `FeiJiZai/ghdeck`；
- 使用旧产品名 `GhDeck` 作为当前用户可见名称；
- 引用不存在的 Release 资产名称。

### 2.3 调整子 README 的仓库级说明

修改：

- `python-version/README.md`
- `rust-version/README.md`

只补充统一仓库导航和正确的 `v0.2.0` 资产名称；保留各自实现细节、安全边界和构建命令。Rust README 不再假设自己单独作为仓库根目录。

### 2.4 清理仅用于脚手架的无关资产

审查并仅删除明确未引用的默认 Vite/Tauri/React 占位资源，例如：

- `rust-version/public/vite.svg`
- `rust-version/public/tauri.svg`
- `rust-version/src/assets/react.svg`

删除前用源码搜索确认没有引用。不得删除 GithubDeck 共享或生成图标。

## 阶段 3：验证源码与版本一致性

### 3.1 名称和 URL 审查

搜索当前源码与当前 README：

- 用户可见名称必须为 `GithubDeck`；
- 允许保留的兼容标识仅包括：Cargo `ghdeck`、库 `ghdeck_lib`、Tauri identifier `com.feijizai.ghdeck`、Rust `GhDeck/settings.json`、Python project name `ghdeck`、Python `QSettings("VibeDev", "GhDeck")`；
- GitHub CLI 官方发布页固定为 `https://github.com/cli/cli/releases/`；
- 不存在 `webbrowser`、任意 URL opener、宽 capability、shell 拼接或 token 读取能力。

### 3.2 将两端内部版本统一升级为 0.2.0

修改并验证以下版本来源：

- `python-version/pyproject.toml`：`project.version = "0.2.0"`；
- 在 `python-version` 运行 `uv lock`，同步 `python-version/uv.lock` 中本地 `ghdeck` 包版本为 `0.2.0`，随后 `uv lock --check` 必须通过；
- `python-version/github_cli_manager/app.py`：QApplication version 为 `0.2.0`；
- `python-version/tests/test_app.py`：版本断言为 `0.2.0`；
- `rust-version/package.json`：`version = "0.2.0"`，通过 npm 更新 `package-lock.json` 根包版本；
- `rust-version/src-tauri/Cargo.toml`：`package.version = "0.2.0"`，通过 Cargo 更新 `Cargo.lock` 中本地 `ghdeck` 包版本；
- `rust-version/src-tauri/tauri.conf.json`：`version = "0.2.0"`；
- 两份子 README 和根 README 的当前构建产物示例使用 `0.2.0`。

不得修改兼容标识 `ghdeck`、`ghdeck_lib`、`com.feijizai.ghdeck`、`GhDeck/settings.json` 或 `VibeDev/GhDeck`。完成后搜索所有当前版本元数据，确认不存在仍会写入 EXE/MSI/NSIS 的 `0.1.0`。

### 3.3 Python 完整验证

在 `python-version` 运行：

```powershell
uv lock --check
$env:QT_QPA_PLATFORM="offscreen"
uv run pytest -q
uv run pyinstaller --clean --noconfirm GithubDeck.spec
```

验收：

- 锁文件检查通过；
- 64 项或更新后的完整测试全部通过；
- `dist/GithubDeck.exe` 存在且非空；
- 启动真实 GUI 窗口，确认标题为 `GithubDeck`，随后正常关闭；
- 不点击登录、切换或登出。

### 3.4 Rust/Tauri 完整验证

在 `rust-version` 运行：

```powershell
$env:NODE_ENV="development"
npm test -- --run
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run tauri -- build
```

使用机器上已验证的绝对 npm/Cargo 路径执行。

验收：

- React/Vitest 全部通过；
- TypeScript/Vite build 通过；
- Rust 单元和集成测试全部通过；
- 以下产物存在且非空：
  - `src-tauri/target/release/GithubDeck.exe`
  - `src-tauri/target/release/bundle/msi/GithubDeck_0.2.0_x64_en-US.msi`
  - `src-tauri/target/release/bundle/nsis/GithubDeck_0.2.0_x64-setup.exe`
- Rust EXE 启动后标题为 `GithubDeck` 并正常关闭；
- 不执行真实账号操作。

`cargo fmt --check` 当前存在两个历史格式差异；如果本次没有修改这些文件，单独记录但不为发布制造无关格式化 diff。若本次触及这些文件，则必须格式化本次改动。

## 阶段 4：创建 Release 暂存资产

### 4.1 建立本地暂存目录

在被 `.gitignore` 排除的 `release-staging/v0.2.0/` 中复制并重命名：

```text
python-version/dist/GithubDeck.exe
  -> GithubDeck-Python-Portable-v0.2.0.exe

rust-version/src-tauri/target/release/GithubDeck.exe
  -> GithubDeck-Rust-Portable-v0.2.0.exe

rust-version/src-tauri/target/release/bundle/msi/GithubDeck_0.2.0_x64_en-US.msi
  -> GithubDeck-Rust-v0.2.0-x64.msi

rust-version/src-tauri/target/release/bundle/nsis/GithubDeck_0.2.0_x64-setup.exe
  -> GithubDeck-Rust-v0.2.0-x64-setup.exe
```

不得移动或重命名原始构建目录中的文件。

### 4.2 生成 `SHA256SUMS.txt`

对四个二进制计算小写 SHA-256，按上述顺序写入：

```text
{64 位小写 SHA-256}  GithubDeck-Python-Portable-v0.2.0.exe
{64 位小写 SHA-256}  GithubDeck-Rust-Portable-v0.2.0.exe
{64 位小写 SHA-256}  GithubDeck-Rust-v0.2.0-x64.msi
{64 位小写 SHA-256}  GithubDeck-Rust-v0.2.0-x64-setup.exe
```

验证每行摘要可重新计算匹配，五个暂存文件均非空。

### 4.3 准备 Release 正文

创建 `release-staging/v0.2.0/RELEASE_NOTES.md`，正文固定包含：

- GithubDeck 重命名与 GD 新图标；
- Python/PySide6 和 Rust/Tauri 双实现；
- 四个下载资产的用途；
- 推荐普通用户使用 Rust NSIS Setup；
- Windows 10+ 与 GitHub CLI 要求；
- 未签名和 SmartScreen 提示；
- PowerShell SHA-256 校验示例；
- 不读取、显示、持久化 Token，不访问 Windows Credential Manager；
- 登录、切换、登出仍由本机 GitHub CLI 完成。

验证 `RELEASE_NOTES.md` 存在且非空；它是 Release 正文输入，不作为第六个资产上传。Release 暂存目录应有五个待上传资产和一个本地 notes 文件。

## 阶段 5：初始化并审查新本地根仓库

### 5.1 处理嵌套 Git 元数据

前提：阶段 1 bundle 已验证成功。

- 确认 `python-version/.git` 是唯一嵌套 Git 元数据；
- 将其移动到根目录之外的备份位置，而不是直接永久删除；
- 确认 `rust-version` 没有 `.git`；
- 在工作区根目录初始化 Git，并将默认分支设为 `master`。

这是本地仓库结构变更，不影响远程；若失败，可把备份的 `.git` 恢复到 `python-version`。

### 5.2 暂存前安全审查

运行：

```powershell
git status --short
git check-ignore -v python-version/dist/GithubDeck.exe python-version/.venv rust-version/node_modules rust-version/dist rust-version/src-tauri/target release-staging/v0.2.0
git ls-files
```

确认不会暂存：

- `.venv`、`node_modules`、`target`、`dist`、`build`；
- `release-staging`、bundle 和旧 Release 元数据；
- `.env`、Token、凭据、用户设置和本机 GitHub CLI 配置；
- 嵌套 `.git`。

使用明确路径暂存根 README、`.gitignore`、`assets`、`docs`、`python-version` 和 `rust-version`，不得使用会意外包含机密的盲目暂存方式。

### 5.3 创建首次提交

提交信息聚焦“为什么”，例如：

```text
Publish GithubDeck Python and Rust implementations

Unify both desktop clients under the GithubDeck identity so users can choose a portable Python build or native Tauri installers from one project.

Co-Authored-By: VibeDev <noreply@vibedev.ai>
```

提交后确认：

- 工作树仅剩被忽略的构建/暂存文件；
- `master` 有一个完整根提交；
- `git ls-tree -r HEAD` 不含禁止文件；
- commit 中没有 gitlink（mode `160000`）。

## 阶段 6：不可逆操作前最终门禁

删除旧远程仓库前必须再次全部确认：

1. Git bundle 验证成功；
2. 旧 Release 元数据已保存；
3. Python 测试、构建、窗口烟雾测试通过；
4. Rust 测试、构建、窗口烟雾测试通过；
5. 五个 Release 文件存在、非空、摘要匹配；
6. 根仓库首次提交完成且安全审查通过；
7. GitHub CLI 当前认证账号有删除和创建仓库权限；
8. `gh repo view FeiJiZai/GithubDeck` 明确返回不存在；
9. `gh repo view FeiJiZai/ghdeck` 仍精确指向要删除的 Public 仓库；
10. 没有其他同名或大小写冲突仓库。

任一项不满足就停止，不删除旧仓库。

## 阶段 7：删除旧仓库并创建新公开仓库

### 7.1 删除旧仓库

使用精确 owner/name：

```powershell
gh repo delete FeiJiZai/ghdeck --yes
```

该命令已由用户明确授权。执行后立即查询旧仓库，确认返回不存在。

### 7.2 创建新仓库

创建：

```powershell
gh repo create FeiJiZai/GithubDeck --public --description "GithubDeck — Windows GitHub CLI multi-account desktop manager"
```

不要让创建命令自动推送未审查目录；先创建空远程，再显式设置本地 remote。

验证：

- `nameWithOwner` 为 `FeiJiZai/GithubDeck`；
- visibility 为 `PUBLIC`；
- URL 为 GitHub 返回的官方 URL。

若创建失败，停止远程动作，保留本地提交、bundle 和资产，准确报告；不得删除其他仓库或改用近似名称。

## 阶段 8：推送 master 并验证源码

设置新 origin：

```powershell
git remote add origin https://github.com/FeiJiZai/GithubDeck.git
git push -u origin master
```

若 origin 已存在，只在确认它精确指向新仓库后更新；不得保留旧 `ghdeck` URL。

验证：

- 远程默认分支为 `master`；
- 远程 `master` SHA 与本地 HEAD 相同；
- 仓库为 Public；
- 根 README 正常渲染；
- 远程树包含 `python-version`、`rust-version`、`assets`、`docs`；
- 远程树不包含构建、依赖、备份或机密文件。

## 阶段 9：发布 v0.2.0

使用暂存目录五个文件创建稳定 Release：

```powershell
$assets = @(
    "release-staging/v0.2.0/GithubDeck-Python-Portable-v0.2.0.exe"
    "release-staging/v0.2.0/GithubDeck-Rust-Portable-v0.2.0.exe"
    "release-staging/v0.2.0/GithubDeck-Rust-v0.2.0-x64.msi"
    "release-staging/v0.2.0/GithubDeck-Rust-v0.2.0-x64-setup.exe"
    "release-staging/v0.2.0/SHA256SUMS.txt"
)
gh release create v0.2.0 @assets --repo FeiJiZai/GithubDeck --title "GithubDeck v0.2.0" --notes-file "release-staging/v0.2.0/RELEASE_NOTES.md"
```

不得使用 `--draft` 或 `--prerelease`。

若 Release 已因部分失败而存在：

- 先查询现有状态和资产；
- 只补传缺失资产；
- 仅在资产内容错误时使用精确名称替换；
- 不创建第二个 `v0.2.0` Release。

## 阶段 10：远程发布验证

查询并确认：

- tag 为 `v0.2.0`；
- title 为 `GithubDeck v0.2.0`；
- `isDraft = false`；
- `isPrerelease = false`；
- 资产数量恰好为 5；
- 资产名称与设计完全一致；
- 每个资产大小非零；
- GitHub 提供的摘要与本地 SHA-256 一致，或下载资产后重新计算一致；
- `SHA256SUMS.txt` 内容与本地一致；
- 旧仓库查询返回不存在；
- 新仓库和 Release URL 均可公开访问。

最终返回新仓库和 Release 的 GitHub 官方 URL，不手写或猜测 URL。

## 独立验证

这是基础仓库结构、远程删除、推送和发布的非平凡变更。完成本地提交后、删除旧仓库前先执行一次独立验证，检查：

- 根仓库树和 `.gitignore`；
- Python/Rust 测试与生产构建；
- Release 暂存资产和摘要；
- 机密与构建产物排除；
- bundle 可恢复性；
- 新仓库不存在和旧仓库精确身份。

远程发布后再执行一次独立验证，检查：

- 旧仓库不存在；
- 新仓库 Public、master 和 SHA；
- README 与源码树；
- v0.2.0 状态、五个资产及摘要。

验证者给出 FAIL 时必须修复并复验；PARTIAL 时准确报告无法验证项，不伪造 PASS。

## 完成标准

- 旧 `FeiJiZai/ghdeck` 已删除且本地 bundle 可恢复；
- 新 `FeiJiZai/GithubDeck` 是 Public monorepo；
- 默认分支为 `master`；
- 根 README 已重写并准确介绍双实现；
- Python、Rust、共享资产和文档均已推送；
- 不含依赖、构建、备份、凭据或用户配置；
- `v0.2.0` 稳定 Release 恰好有五项指定资产；
- 四个二进制摘要与 `SHA256SUMS.txt` 一致；
- 双端测试、生产构建和窗口烟雾测试通过；
- 返回经过 GitHub 查询获得的新仓库和 Release URL。
