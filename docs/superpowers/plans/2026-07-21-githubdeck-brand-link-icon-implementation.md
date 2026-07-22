# GithubDeck 品牌、发布页链接与图标更新实施计划

## 目标与边界

依据 `docs/superpowers/specs/2026-07-21-githubdeck-brand-link-icon-design.md`，将 Rust/Tauri 和 Python/PySide6 两个桌面版本的用户可见品牌统一为 `GithubDeck`，让两个版本共用同一套“GD 专业字标”图标，并让 Rust 设置页通过系统默认浏览器打开固定的 GitHub CLI 官方发布页。

本次只修改品牌呈现、固定发布页链接、图标资产、构建配置、文档和相关测试。不得修改账号刷新、切换、登录、取消、登出、GitHub CLI 路径校验、命令参数、输出过滤或凭据安全边界；验证中不得执行真实 GitHub 登录、账号切换或登出。

## 必须保留的兼容标识

不得对 `GhDeck` / `ghdeck` 做全局替换。以下内部标识必须原样保留：

- Rust Cargo 包名：`ghdeck`；
- Rust 库名：`ghdeck_lib`；
- Tauri identifier：`com.feijizai.ghdeck`；
- Rust 设置目录：`GhDeck/settings.json`；
- Python 包和导入名：`github_cli_manager`；
- Python 项目分发标识：`ghdeck`，因为它不控制本次用户下载的 EXE 名称；
- Python QSettings 组织/应用存储身份：`VibeDev/GhDeck`；
- 现有 GitHub 仓库地址和默认分支。

用户可见窗口标题、应用标题、当前 README、构建产物名称和新图标统一使用 `GithubDeck`。历史设计文档中用于描述旧版本的 `GhDeck` 不做回溯改写。

## 实施原则与依赖顺序

1. 先添加会失败的兼容性、安全策略和用户可见行为测试。
2. 再创建一份路径化 SVG 品牌母图，由它生成所有 Rust 和 Python 图标资产。
3. 先完成 Rust/Tauri 依赖、权限、界面与构建配置，再完成 Python/PySide6 运行时和 PyInstaller 配置。
4. 每一阶段先运行聚焦测试，全部修改完成后再运行完整测试和生产构建。
5. 多文件实现完成后必须由独立 verification agent 验证；只有 PASS 才能按“全部完成”报告，PARTIAL 必须准确列出无法验证项。
6. 本计划不创建提交、分支、推送、Release，也不提交生成的 EXE、MSI、NSIS 或 PyInstaller 构建目录。

## 任务 1：锁定 Rust 兼容设置目录和 Tauri 构建身份

修改：

- `rust-version/src-tauri/src/settings.rs`
- `rust-version/src-tauri/tauri.conf.json`
- 新增 `rust-version/src-tauri/tests/branding_config.rs`

测试先行：

1. 在 `settings.rs` 将旧目录名提取为私有常量 `LEGACY_SETTINGS_DIRECTORY`，值保持 `"GhDeck"`。
2. 增加单元测试，断言设置路径尾部为 `GhDeck/settings.json`。为避免依赖真实用户配置目录，可抽取只负责把根目录与旧目录、文件名拼接的纯函数；生产路径仍由 `dirs::config_dir()` 提供。
3. 增加配置回归测试，读取 `tauri.conf.json` 并断言：
   - `productName` 为 `GithubDeck`；
   - `mainBinaryName` 为 `GithubDeck`；
   - 主窗口标题为 `GithubDeck`；
   - `identifier` 仍为 `com.feijizai.ghdeck`。
4. 测试应先因新品牌配置和纯路径断言尚未实现而失败。

实现：

- 将 `tauri.conf.json` 的 `productName` 和主窗口 `title` 改为 `GithubDeck`。
- 添加 `mainBinaryName: "GithubDeck"`，使 Cargo 包名仍为 `ghdeck` 时 release 主程序仍输出 `GithubDeck.exe`。
- 不修改 identifier、版本号、窗口尺寸、CSP 或 bundle targets。

聚焦验证，在 `rust-version/src-tauri` 运行：

```powershell
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --locked settings
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --locked --test branding_config
```

## 任务 2：锁定 Rust 固定发布页权限策略

修改：

- `rust-version/src-tauri/capabilities/default.json`
- 新增 `rust-version/src-tauri/tests/capability_policy.rs`
- `rust-version/src-tauri/Cargo.toml`

测试先行：

1. 新增集成测试，用 `serde_json` 读取 `capabilities/default.json`。
2. 测试必须断言 capability 只作用于 `main` 窗口，并保留 `core:default`。
3. 测试必须找到且只找到一个 URL opener 权限，其内容为：

```json
{
  "identifier": "opener:allow-open-url",
  "allow": [
    {
      "url": "https://github.com/cli/cli/releases/"
    }
  ]
}
```

4. 明确断言不存在以下宽权限：
   - `opener:default`；
   - URL 中的 `*` 通配符；
   - `http://` 地址；
   - shell、path、reveal 或任意协议权限。
5. 测试应先因固定 opener 权限缺失而失败。

实现：

- 在 `default.json` 中只添加上述固定 HTTPS URL 权限。
- 不添加通用 opener、shell 或文件系统能力。
- 测试仅使用现有 `serde_json` 开发可用依赖，不引入额外 Rust 测试库。

聚焦验证：

```powershell
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --locked --test capability_policy
```

## 任务 3：添加 Rust opener 与前端交互测试基础

修改：

- `rust-version/package.json`
- `rust-version/package-lock.json`
- `rust-version/src-tauri/Cargo.toml`
- `rust-version/src-tauri/Cargo.lock`
- `rust-version/src-tauri/src/lib.rs`
- 新增 `rust-version/src/App.test.tsx`
- `rust-version/vite.config.ts`

步骤：

1. 添加与 Tauri 2 兼容的前端依赖 `@tauri-apps/plugin-opener`。
2. 添加最小前端测试依赖和 `test` 脚本，使用 Vitest、jsdom 和 Testing Library；在现有 `vite.config.ts` 中加入 `test.environment = "jsdom"`，不新增第二份 Vite 配置，也不引入端到端浏览器框架。
3. 在 Rust 依赖中添加 `tauri-plugin-opener`，更新两个锁文件，不手工编辑锁文件内容。
4. 在 `tauri::Builder::default()` 上注册：

```rust
.plugin(tauri_plugin_opener::init())
```

5. 测试环境 mock `@tauri-apps/api/core`、`@tauri-apps/api/event` 和 `@tauri-apps/plugin-opener`，禁止测试连接真实 Tauri 运行时或打开浏览器。
6. 测试应覆盖：
   - 顶部品牌文本为 `GithubDeck`；
   - 设置页显示完整固定 URL 和清晰的“官方发布页”可访问名称；
   - 点击只调用一次 `openUrl("https://github.com/cli/cli/releases/")`；
   - `openUrl` 拒绝时设置窗口保持打开；
   - 失败时现有状态区域显示简体中文错误且 `isError` 为真。
7. 失败测试不得通过允许处理函数接收任意 URL 来修复。

依赖安装在 `rust-version` 运行：

```powershell
$env:NODE_ENV="development"
& "C:\nvm4w\nodejs\npm.cmd" install
```

聚焦验证：

```powershell
& "C:\nvm4w\nodejs\npm.cmd" test -- --run
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --locked
```

如果 npm 所在绝对路径不同，只使用机器上已验证的 Node/npm 安装路径；不得修改全局 npm 配置来绕过环境问题。

## 任务 4：实现 Rust 固定官方发布页链接和品牌界面

修改：

- `rust-version/src/App.tsx`
- `rust-version/src/App.css`
- `rust-version/index.html`

实现：

1. 在 `App.tsx` 添加唯一固定常量：

```ts
const GITHUB_CLI_RELEASES_URL = "https://github.com/cli/cli/releases/";
```

2. 从 `@tauri-apps/plugin-opener` 导入 `openUrl`。
3. 实现无参数的 `openGithubCliReleases()`；它只能引用上述常量，不接收 URL 参数。
4. 点击链接时阻止 WebView 自行导航，再调用 `openUrl(GITHUB_CLI_RELEASES_URL)`。
5. 捕获 opener 错误并通过现有 `AppState.message` 显示固定简体中文错误“无法打开 GitHub CLI 官方发布页，请稍后重试”；不得展示插件原始异常。
6. 将顶部 `GH` 文字标记替换为共享 favicon 图像，品牌文本改为 `GithubDeck`。图像提供非重复的可访问文本策略：若相邻文本已说明品牌，图标使用空 `alt`。
7. 设置页保留完整可识别 URL，文案统一为“官方发布页：URL”。
8. `App.css` 只调整图像尺寸、圆角和链接的 hover/focus-visible 样式，不用 CSS 再绘制另一套 GD 图标。
9. `index.html` 标题改为 `GithubDeck`，favicon 改为 `/favicon.png`。
10. 不修改设备登录固定 URL、账号操作调用、设置路径校验或现有 CSP。

聚焦验证：

```powershell
& "C:\nvm4w\nodejs\npm.cmd" test -- --run
& "C:\nvm4w\nodejs\npm.cmd" run build
```

## 任务 5：创建共享 GD 专业字标母图并生成全部图标

新增或修改：

- 新增 `assets/githubdeck-icon.svg`
- `rust-version/src-tauri/icons/*`
- 新增 `rust-version/public/favicon.png`
- 新增 `python-version/assets/githubdeck-icon.png`
- 新增 `python-version/assets/githubdeck-icon.ico`

母图要求：

- 建立一份高分辨率、方形、深色圆角底的 SVG 母图。
- 使用白色 `G` 和 GitHub 蓝色 `D` 的路径，不依赖系统字体、外部 URL、脚本、滤镜或嵌入位图。
- 不使用 Octocat 或其他 GitHub 商标图形。
- 字形、边距和对比度须保证 16×16 与 32×32 仍可识别。

生成步骤：

1. 在 `rust-version` 目录使用 Tauri 图标命令从同一母图重新生成 `src-tauri/icons` 下所有 PNG、ICO、ICNS 和 Windows Store Logo：

```powershell
& "C:\nvm4w\nodejs\npm.cmd" run tauri -- icon "..\assets\githubdeck-icon.svg" --output "src-tauri\icons"
```

2. 从生成结果复制适合前端显示的 PNG 为 `rust-version/public/favicon.png`。
3. 将同一生成结果复制为 Python 的 `assets/githubdeck-icon.png` 和 `assets/githubdeck-icon.ico`；不得重新手绘第二套 Python 图标。
4. 检查所有预期 Tauri 图标存在且非空：`icon.png`、`icon.ico`、`icon.icns`、`32x32.png`、`128x128.png`、`128x128@2x.png` 和 Store Logo 系列。
5. 视觉抽查 SVG 母图、16×16/32×32 PNG、favicon 和 ICO，确认透明度、圆角、字母间距和小尺寸识别性。
6. 不提交工具生成的 EXE、MSI、NSIS、`dist/` 或 PyInstaller `build/`。

## 任务 6：锁定 Python 品牌、链接和旧 QSettings 身份

修改：

- 新增 `python-version/tests/test_app.py`
- `python-version/tests/test_main_window.py`
- `python-version/tests/test_gh_settings_dialog.py`
- `python-version/tests/test_gh_settings.py`

测试先行：

1. 应用元数据测试验证：
   - Qt organization name 仍为 `VibeDev`；
   - application name 和 display name 为 `GithubDeck`；
   - version 仍为 `0.1.0`；
   - QApplication/window 使用共享 GithubDeck 图标。
2. 主窗口测试将标题断言更新为 `GithubDeck`，并验证顶部品牌文本和图标 pixmap 已加载。
3. 设置对话框测试验证：
   - `ghSourceLabel` 使用 RichText；
   - `openExternalLinks` 为真；
   - HTML 只包含固定 `https://github.com/cli/cli/releases/`；
   - 可见文案以“官方发布页：”开头。
4. 设置兼容测试验证默认 `GhExecutableSettings()` 显式构造 `QSettings("VibeDev", "GhDeck")`，即使当前 QApplication 的可见 applicationName 已是 `GithubDeck`，仍能读写原有 `githubCli/executablePath`。
5. 测试须隔离 QSettings 路径，避免读写真实用户配置。可用 `QSettings.setDefaultFormat(QSettings.Format.IniFormat)` 和临时用户作用域路径，并在测试结束后恢复全局设置；不得删除真实配置。
6. 注入自定义 `QSettings` 的现有测试语义必须继续通过。

聚焦验证，在 `python-version` 运行：

```powershell
$env:QT_QPA_PLATFORM="offscreen"
uv run pytest tests/test_app.py tests/test_main_window.py tests/test_gh_settings_dialog.py tests/test_gh_settings.py -q
```

## 任务 7：实现 Python 品牌和运行时图标，同时保留旧设置

修改：

- `python-version/github_cli_manager/app.py`
- `python-version/github_cli_manager/main_window.py`
- `python-version/github_cli_manager/gh_settings_dialog.py`
- `python-version/github_cli_manager/gh_settings.py`
- `python-version/github_cli_manager/__init__.py`

实现：

1. 在 `app.py` 将 application name 和 display name 改为 `GithubDeck`，organization 和 version 不变。
2. 从打包和源码运行均可解析的位置加载 `assets/githubdeck-icon.png`，对 QApplication 和 MainWindow 使用同一 `QIcon`。资源路径处理只解决 PyInstaller one-file 临时目录与源码目录两种真实边界，不建立通用资源框架。
3. 将主窗口标题和顶部品牌文字改为 `GithubDeck`，把 `GH` 文字徽标替换为共享 PNG；保持既有公开控件属性、对象名和业务信号不变。
4. 调整 QSS 中 `appMark` 的规则以适配 pixmap，不用 QSS 重画字标。
5. 在 `gh_settings_dialog.py` 保留 `_RELEASES_URL` 固定常量和 Qt `setOpenExternalLinks(True)`，只把链接文案统一为“官方发布页：URL”；不引入 `webbrowser`、shell 或接收任意 URL 的槽函数。
6. 在 `GhExecutableSettings.__init__` 无注入对象时显式使用：

```python
QSettings("VibeDev", "GhDeck")
```

7. 保留 `_SETTINGS_KEY = "githubCli/executablePath"`，不迁移、不双写、不删除旧键。
8. 更新包级用户可见说明为 `GithubDeck`；包目录和导入名保持不变。

聚焦验证：

```powershell
$env:QT_QPA_PLATFORM="offscreen"
uv run pytest tests/test_app.py tests/test_main_window.py tests/test_gh_settings_dialog.py tests/test_gh_settings.py -q
```

## 任务 8：更新 Python PyInstaller 配置和两端文档

重命名与修改：

- `python-version/GhDeck.spec` → `python-version/GithubDeck.spec`
- `python-version/README.md`
- `rust-version/README.md`
- `python-version/pyproject.toml` 的 description，保留 `[project].name = "ghdeck"`
- `rust-version/src-tauri/Cargo.toml` 的 description

步骤：

1. 将 PyInstaller EXE 名称改为 `GithubDeck`。
2. 设置 Windows 文件图标为 `assets/githubdeck-icon.ico`。
3. 将运行时 PNG 加入 `Analysis.datas`，目标路径保持应用资源加载逻辑可解析。
4. 删除旧 `GhDeck.spec`，只保留 `GithubDeck.spec`，避免两个构建入口产生不同名称。
5. 两份当前 README 的标题、正文、下载名、校验命令和构建命令统一为 `GithubDeck`。
6. Python README 构建命令改为：

```powershell
uv run pyinstaller --clean --noconfirm GithubDeck.spec
```

7. README 中发布产物使用 `GithubDeck.exe` 和 `GithubDeck.exe.sha256`；Rust 说明使用 `GithubDeck.exe`、`GithubDeck_0.1.0_x64_en-US.msi`、`GithubDeck_0.1.0_x64-setup.exe`。
8. Cargo 描述中的产品名改为 `GithubDeck`，但 package/lib 名保持原样。
9. 将 Python `[project].description` 更新为包含 `GithubDeck` 的用户可见描述，同时保持 `[project].name = "ghdeck"`；仓库 URL、版本号、安全声明和 GitHub CLI 行为说明不变。

## 任务 9：执行名称与安全边界审查

在构建前搜索源文件和当前文档：

- 应改名的位置不得残留用户可见 `GhDeck`。
- 以下残留是明确允许的：Rust package/lib/identifier、Rust `GhDeck` 设置目录、Python `QSettings("VibeDev", "GhDeck")`、Python project name、历史设计/实施文档中的旧名称。
- 搜索 opener 权限，确认只有固定 HTTPS Releases URL，没有 wildcard、HTTP、shell、path 或 generic opener default。
- 搜索 URL 调用，确认 Rust 点击处理函数不接受 URL 参数，Python 仍只使用 Qt 固定富文本链接。
- 搜索图标引用，确认 Rust 前端、Tauri bundle、Python 运行时和 PyInstaller 都源自同一母图生成资产。

不得用盲目的全局替换或删除兼容性断言来消除搜索结果。

## 任务 10：完整自动化验证

### Rust 后端和策略

在 `rust-version/src-tauri` 运行：

```powershell
& "$env:USERPROFILE\.cargo\bin\cargo.exe" fmt --all -- --check
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --locked
```

项目若未安装 `rustfmt`，准确报告该项无法执行，但仍运行测试；不得静默跳过。

### Rust 前端

在 `rust-version` 运行：

```powershell
& "C:\nvm4w\nodejs\npm.cmd" test -- --run
& "C:\nvm4w\nodejs\npm.cmd" run build
```

### Python

在 `python-version` 运行：

```powershell
uv lock --check
$env:QT_QPA_PLATFORM="offscreen"
uv run pytest -q
```

任何失败必须定位并修复根因；不得删除测试、降低断言或绕过锁文件检查来制造通过。

## 任务 11：构建并检查 Rust/Tauri 生产产物

在 `rust-version` 运行：

```powershell
& "C:\nvm4w\nodejs\npm.cmd" run tauri -- build
```

检查预期产物存在且非空：

- `rust-version/src-tauri/target/release/GithubDeck.exe`
- `rust-version/src-tauri/target/release/bundle/msi/GithubDeck_0.1.0_x64_en-US.msi`
- `rust-version/src-tauri/target/release/bundle/nsis/GithubDeck_0.1.0_x64-setup.exe`

若 Tauri/Windows 工具链生成的语言或架构后缀与上述不同，应核对实际配置和日志，而不是仅靠模糊文件匹配宣称通过。

验证：

- 文件名已更新，不把旧本地 `GhDeck` 构建残留当作新结果；
- EXE、MSI、NSIS 使用新 GD 图标；
- 文件元数据和窗口标题显示 `GithubDeck`；
- Tauri identifier 仍为 `com.feijizai.ghdeck`；
- 启动新 EXE，确认主窗口正常出现后正常关闭，不触发任何账号操作；
- 打开设置页，确认完整固定 URL 可见；手工点击仅用于确认默认浏览器行为，不输入或替换其他 URL。

## 任务 12：构建并检查 Python/PyInstaller 生产产物

在 `python-version` 运行：

```powershell
uv run pyinstaller --clean --noconfirm GithubDeck.spec
```

检查：

- `python-version/dist/GithubDeck.exe` 存在且非空；
- 运行时不出现控制台窗口；
- 文件和任务栏使用 GD 图标；
- 应用窗口标题和顶部品牌为 `GithubDeck`；
- 设置页显示可点击的“官方发布页：URL”；
- 原有 VibeDev/GhDeck 设置身份仍能读取已经保存的 gh.exe 路径；
- 启动后正常关闭，不执行登录、切换或登出。

若为发布准备校验文件，名称必须是 `GithubDeck.exe.sha256`；本任务仅验证构建，不创建 Release 或上传资产。

## 任务 13：独立验证与最终抽查

由于实现会修改三个以上文件，并涉及 Tauri 权限和桌面构建，完成后调用 `verification` agent，传入：

- 原始请求：两个版本统一改名为 GithubDeck、使用 A 版 GD 专业字标、官方发布页可点击并由默认浏览器打开；
- 设计规范路径；
- 本实施计划路径；
- 全部变更文件；
- 兼容标识和固定 URL 最小权限方案；
- 不得执行真实登录、切换或登出的限制。

verifier 必须实际运行测试和构建，并给出 PASS、FAIL 或 PARTIAL。若 FAIL，修复后继续同一 verifier 复验；若 PASS，主执行者重跑其报告中 2–3 个关键命令并核对输出。若 PASS 报告缺少命令与输出证据，继续要求 verifier 补充，不可直接报告完成。

最终人工抽查：

- SVG、favicon、ICO、16×16 和 32×32 图标能识别为同一 GD 字标；
- Rust 与 Python 窗口标题、顶部品牌和设置页文案一致；
- Rust 点击固定链接通过默认浏览器打开；失败路径显示中文错误且设置窗口不关闭；
- Python 链接继续由 Qt 打开固定 HTTPS 地址；
- 两个版本都未增加任意 URL、任意协议、shell 字符串或凭据读取能力；
- 构建目录和二进制产物不纳入源码变更清单。

## 预计变更文件

共享资产：

- `assets/githubdeck-icon.svg`

Rust/Tauri：

- `rust-version/package.json`
- `rust-version/package-lock.json`
- `rust-version/vite.config.ts` 或最小测试配置
- `rust-version/src/App.tsx`
- `rust-version/src/App.css`
- `rust-version/src/App.test.tsx`
- `rust-version/index.html`
- `rust-version/public/favicon.png`
- `rust-version/README.md`
- `rust-version/src-tauri/Cargo.toml`
- `rust-version/src-tauri/Cargo.lock`
- `rust-version/src-tauri/src/lib.rs`
- `rust-version/src-tauri/src/settings.rs`
- `rust-version/src-tauri/tauri.conf.json`
- `rust-version/src-tauri/capabilities/default.json`
- `rust-version/src-tauri/tests/branding_config.rs`
- `rust-version/src-tauri/tests/capability_policy.rs`
- `rust-version/src-tauri/icons/*`

Python/PySide6：

- `python-version/github_cli_manager/app.py`
- `python-version/github_cli_manager/main_window.py`
- `python-version/github_cli_manager/gh_settings_dialog.py`
- `python-version/github_cli_manager/gh_settings.py`
- `python-version/github_cli_manager/__init__.py`
- `python-version/tests/test_app.py`
- `python-version/tests/test_main_window.py`
- `python-version/tests/test_gh_settings_dialog.py`
- `python-version/tests/test_gh_settings.py`
- `python-version/GhDeck.spec`（删除）
- `python-version/GithubDeck.spec`（新增）
- `python-version/assets/githubdeck-icon.png`
- `python-version/assets/githubdeck-icon.ico`
- `python-version/README.md`
- `python-version/pyproject.toml`（只改 description，不改内部 project name）

## 主要风险与控制

- **Python 配置看似丢失：** 可见应用名改为 GithubDeck 后，默认 QSettings 身份必须显式固定为 `VibeDev/GhDeck`，并由隔离测试证明能读取旧键。
- **Rust 产物仍叫 ghdeck.exe：** 保持 Cargo 包名兼容，同时通过 `mainBinaryName: "GithubDeck"` 控制用户产物；生产构建检查实际文件名。
- **opener 权限过宽：** 前端无参数处理函数和 capability 精确 URL 双重限制，并由 JSON 策略测试拒绝 wildcard、HTTP 和 generic opener。
- **两套图标漂移：** 只维护一份路径化 SVG 母图，Rust 与 Python 资产全部从同一次生成结果复制。
- **小尺寸图标不可辨识：** 除高分辨率检查外，必须抽查 16×16 和 32×32，而不是只看母图。
- **盲目改名破坏升级：** 禁止全局替换；名称审查使用允许列表区分用户可见残留和兼容标识。
- **前端测试误触真实桌面能力：** 所有 Tauri API 和 opener 在 jsdom 中 mock，测试不得打开真实浏览器或执行 GitHub CLI。
- **构建残留造成误判：** 根据构建日志和新文件时间检查 GithubDeck 产物，不把旧 GhDeck 二进制当成成功结果。
- **安全边界回归：** 不修改进程参数、环境清理、Token 防护、账号快照或真实账号操作代码；完整既有测试必须继续通过。

## 完成标准

只有以下全部满足才可报告完成：

- Rust 与 Python 用户可见品牌均为 `GithubDeck`；
- 两个版本使用同一母图生成的 GD 专业字标；
- Rust 只允许打开固定 GitHub CLI Releases HTTPS URL，失败有中文提示；
- Python 继续通过 Qt 提供同一固定可点击 URL；
- Rust 旧设置目录、Tauri identifier、Cargo 标识和 Python 旧 QSettings 身份均保持兼容；
- Rust 测试、前端测试、前端生产构建、Tauri production build、Python pytest 和 PyInstaller build 均通过；
- 新 EXE、MSI、NSIS 使用 `GithubDeck` 文件名和新图标；
- 独立 verifier 给出有命令证据的 PASS，或如实报告其 PARTIAL 项；
- 未执行真实账号操作，未创建提交、分支、推送或 Release。
