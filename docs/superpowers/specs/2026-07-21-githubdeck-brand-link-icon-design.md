# GithubDeck 品牌、发布页链接与图标更新设计

## 目标

将项目中的 Rust/Tauri 与 Python/PySide6 两个桌面版本，从用户可见的 **GhDeck** 统一更名为 **GithubDeck**，采用新的“GD 专业字标”图标，并让设置页中的 GitHub CLI 官方发布页 URL 可通过系统默认浏览器打开。

本次只调整品牌呈现、发布页链接交互和构建资产，不改变账号刷新、切换、登录、取消、登出、GitHub CLI 路径校验或凭据安全边界。

## 已确认的产品决策

- 新产品名：`GithubDeck`
- 重命名范围：Rust 版、Python 版及相关项目文档全部统一
- 图标方向：A 版“GD 专业字标”
- 发布页行为：点击后使用系统默认浏览器打开
- 固定官方地址：`https://github.com/cli/cli/releases/`
- 兼容策略：用户可见名称和构建产物全部更新，内部包名、应用标识及旧配置位置保持兼容

## 品牌重命名范围

### Rust/Tauri 版本

下列用户可见名称改为 `GithubDeck`：

- React 顶部品牌名称；
- HTML 文档标题和 favicon；
- Tauri `productName` 与主窗口标题；
- Cargo 包描述中的产品名称；
- README 标题、正文和构建产物说明；
- Windows EXE、MSI 与 NSIS 安装包的产品名称和文件名；
- 相关自动化测试中的窗口标题及文件名断言。

Rust 内部 crate/lib 标识可继续使用 `ghdeck` / `ghdeck_lib`，避免对代码模块和构建入口进行无价值重命名。Tauri identifier 继续使用 `com.feijizai.ghdeck`，使 Windows 安装升级链和应用身份保持稳定。

### Python/PySide6 版本

下列用户可见名称改为 `GithubDeck`：

- Qt application name、display name 与窗口标题；
- PyInstaller EXE 名称和图标；
- README 标题、正文、下载文件名和校验示例；
- 测试中的标题及应用名称断言；
- 构建配置文件中的输出名称。

Python 包目录和导入名继续保留 `github_cli_manager`。项目包的分发标识可继续保留当前内部名称，除非它直接决定用户下载的产物名称。

### 兼容保留项

以下内容不改名：

- Rust Tauri identifier：`com.feijizai.ghdeck`；
- Rust crate/lib 内部标识：`ghdeck` / `ghdeck_lib`；
- Rust 已有配置目录：`GhDeck`；
- Python 包目录：`github_cli_manager`；
- 现有 GitHub 仓库地址与默认分支。

保留旧配置目录可确保升级到 GithubDeck 后仍能读取用户已经保存的 `gh.exe` 路径。界面和 README 不向用户暴露这一内部兼容细节，必要时仅在开发文档中说明。

## GD 专业字标图标

图标采用 GitHub 专业控制台视觉：

- 深色方形底，使用接近 GitHub 深色界面的黑灰色；
- 白色 `G` 与 GitHub 蓝色 `D` 形成紧凑字标；
- 圆角和留白适合 Windows 任务栏、小图标和安装器；
- 不使用 GitHub Octocat 或其他受保护商标图形；
- 不使用复杂细节，确保 16×16、32×32 尺寸仍可识别。

先生成一个透明背景或完整底色的高分辨率母图，再通过 Tauri 图标命令或等价的本地转换方式生成：

- `icon.png`；
- `icon.ico`；
- `icon.icns`；
- Tauri 所需各尺寸 PNG 与 Windows Store Logo；
- 前端 favicon；
- Python PyInstaller 使用的 `.ico`。

Rust 和 Python 版本必须使用同一套品牌母图，避免同名应用出现两套图标。

## 官方发布页链接

### Rust/Tauri

当前 React 设置窗口中的官方发布页是纯文本。更新后显示为清晰的可点击链接或链接按钮：

```text
官方发布页：https://github.com/cli/cli/releases/
```

点击时通过 Tauri opener 插件调用系统默认浏览器。安全限制如下：

- URL 为代码内固定常量，不接受用户输入；
- capability 只授予打开该固定 HTTPS 地址所需的最小权限；
- 不使用 shell 字符串、命令拼接或任意协议；
- 打开失败时在现有状态栏显示简体中文错误信息；
- 链接仍保留完整文本，用户可以识别目标地址。

### Python/PySide6

Python 设置页已使用富文本链接和 `setOpenExternalLinks(True)` 打开相同官方地址。实现阶段只需确认链接文案符合“官方发布页：URL”的统一格式，并补充或更新测试，不引入新的浏览器调用代码。

## 构建产物

更新后预期主要产物为：

### Rust/Tauri

- `GithubDeck.exe`
- `GithubDeck_0.1.0_x64_en-US.msi`
- `GithubDeck_0.1.0_x64-setup.exe`

### Python/PyInstaller

- `GithubDeck.exe`
- 发布时对应的 `GithubDeck.exe.sha256`

旧的本地 `GhDeck` 构建产物不作为新版本验证目标。构建目录仍由忽略规则管理，不提交生成的 EXE、MSI 或安装程序。

## 数据流与错误处理

品牌名称与图标是静态构建资产，不参与运行时业务状态。

Rust 发布页链接的数据流为：

```text
用户点击固定链接
  → React 调用 Tauri opener API
  → capability 校验固定 URL
  → Windows 系统默认浏览器打开官方页面
```

若 opener 返回错误，React 捕获错误并更新现有 `UiMessage` 风格的状态提示。失败不会关闭设置窗口，也不会影响 `gh.exe` 路径输入、校验或保存。

Python 继续由 Qt 处理外部 HTTPS 链接，行为不变。

## 安全边界

本次更新必须保持现有安全约束：

- 不读取、保存或展示 GitHub Token；
- 不读取 Windows Credential Manager；
- 不调用 `gh auth token` 或 `--show-token`；
- 不把 GitHub CLI 原始输出展示到界面或写入日志；
- 不增加任意 URL、任意协议或任意命令打开能力；
- 不执行真实登录、切换或登出来验证品牌更新。

## 测试与验证

实现后按以下层次验证：

1. 搜索 Rust/Python 用户可见源码，确认应更名位置已使用 `GithubDeck`；允许内部兼容标识保留 `ghdeck`、`GhDeck`。
2. Rust：运行 `cargo test --locked`，覆盖现有后端行为。
3. Rust 前端：运行 TypeScript 与 Vite 生产构建。
4. Rust 链接：测试固定 URL 常量、点击调用与错误提示；检查 capability 不允许任意 URL。
5. Rust Tauri：完整 release 构建，检查 EXE、MSI、NSIS 名称和图标。
6. Python：运行完整 pytest 测试。
7. Python PyInstaller：构建单文件 EXE，检查名称和图标。
8. 图标：检查母图、favicon、ICO 以及各 Tauri 尺寸资产均已更新；抽查 16×16 与 32×32 可识别性。
9. 启动两个版本，确认窗口标题均为 `GithubDeck`，随后正常关闭。
10. 验证过程中不执行真实 GitHub 登录、账号切换或登出。

## 完成标准

以下条件全部满足时，本次更新完成：

- Rust 与 Python 的用户可见品牌统一为 `GithubDeck`；
- 两个版本使用同一套 GD 专业字标图标；
- Rust 设置页的官方发布页 URL 可由默认浏览器打开；
- Python 设置页继续提供同一官方发布页的可点击链接；
- 新构建产物使用 `GithubDeck` 文件名；
- 旧设置路径和安装身份仍兼容；
- Rust/Python 测试、前端构建及桌面应用构建通过；
- 不引入任意 URL 打开能力或其他安全边界回归。
