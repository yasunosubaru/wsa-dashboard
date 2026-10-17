# WSA Dashboard

[![Rust](https://img.shields.io/badge/Rust-1.90+-3776AB?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Slint](https://img.shields.io/badge/Slint-1.17-1C3C3C?style=flat)](https://slint.dev/)
[![Windows](https://img.shields.io/badge/Windows-11-0078D4?style=flat&logo=windows&logoColor=white)](#requirements)
[![tests](https://img.shields.io/badge/tests-417%20passing-brightgreen?style=flat)](#testing)
[![license](https://img.shields.io/badge/license-GPL--3.0--only-blue?style=flat)](#license)

给 Windows Subsystem for Android 的一个桌面控制台：开停子系统、看应用与磁盘、装 APK、
转端口、排计划任务。原生 Rust + Slint，一个 exe，除 `adb` 外不需要运行时。

[设计规格](docs/superpowers/specs/2026-10-10-wsa-dashboard-design.md) ·
[逐项差异记录](docs/MAPPING.md) ·
[汇报缺陷](../../issues/new) ·
[与上游的关系](PROVENANCE.md)

---

## 项目概况

WSL 有 WSL Dashboard 那样的图形控制台，WSA 没有。Windows 上能用的只有
`Windows Subsystem for Android™ Settings` 那个设置页，它管不到应用层：装了哪些包、哪个在跑、
`adb` 通不通、端口怎么转、VHDX 涨到多大了、能不能定时跑一条 `adb` 命令——这些都得开终端。

这个仓库把那些终端里的操作做成九个页签，并且**在子系统关着的时候也不空白**：每条命令走
`adb`，`adb` 三档状态（就绪 / 启动中 / 不可用）各有自己的渲染，而不是一个空白页加一句
"未连接"。

一个刻意的选择：**不内置 `adb`**，也不替用户装 WSA。工具只驱动已经装好的那套东西，
找不到就直说缺什么、怎么装。

| | |
| --- | --- |
| 语言 / 框架 | Rust 2024 + [Slint](https://slint.dev/) 1.17（`renderer-skia`） |
| 平台 | Windows 11（x64）。WSA 自 2025 年起已停止支持，本工具面向仍在使用它的环境 |
| 产物 | 单个 `wsadashboard.exe`，约 24 MB，内嵌图标与版本资源 |
| 外部依赖 | `adb`（`PATH` 或 `WSADASH_ADB`）、Windows 自带的 `schtasks` |
| 代码规模 | 77 次提交，599 个入库文件，11.8 MB |
| 测试 | 417 条单元测试 + 10 条实机测试（`#[ignore]`，需真机） |
| 许可证 | GPL-3.0-only（**因为它是 WSL Dashboard 的衍生作品**，见[许可证](#license)） |

---

## 功能

### 九个页签

| # | 页签 | 做什么 |
| --- | --- | --- |
| 0 | 首页 | 子系统状态、启动/停止、应用网格、三档降级渲染 |
| 1 | 安装 APK | 选文件、装、看结果；替身上有完整回环测试 |
| 2 | 设备 | adb 目标列表、属性表（`getprop`） |
| 3 | 网络 | `adb forward`/`reverse` 规则、HTTP 代理三个 global |
| 4 | 计划任务 | 增删改查、启停、立即运行；**执行前自动把子系统叫起来** |
| 5 | 存储 | VHDX 元数据、应用占用 Top N；子系统关着也能用 |
| 6 | 捐赠 | 上游有这一页；本仓库没有收款内容，故未启用 |
| 7 | 设置 | 通用/高级/界面三段、语言选择器（50 个包）、`settings.toml` |
| 8 | 关于 | 版本、许可证、更新检查（GitHub releases） |

### 外壳

- **侧栏 54 / 200 px 折叠**，页签靠 `opacity` 切换（与上游同一套做法），不用 `if` 换组件。
- **托盘**：关闭到托盘、开机自启（写 `HKCU\…\Run`）、启动时最小化。
- **单实例**：第二次启动按进程 id 找到已有窗口，`SW_RESTORE` + `SetForegroundWindow`。
- **主题**：浅色/深色，token 与上游逐条对齐。
- **i18n**：50 个语言包，中文/英文无缺键由测试扫描全部 `.slint` 保证。

### 计划任务的"自动"

一条计划任务在 WSA 没起来时会**先把它叫起来再执行**，这不是 `schtasks` 的能力，是这个程序
补的：每条任务的 `/TR` 指向一段本程序生成的 VBS 启动器，第一行动作是
`wsadashboard.exe /scheduler`（探测 → 已就绪立刻返回 → 否则启动并等到就绪），第二行才是用户
写的命令。启动器的退出码被忽略，所以不需要子系统的任务在子系统起不来时照样跑。

---

## 技术构成

- **Rust 2024**，`tokio` 跑 adb（`tokio::process`）与阻塞调用（`spawn_blocking` / 专用线程）。
- **[Slint](https://slint.dev/) 1.17** + `renderer-skia`。选它而不是 Tauri/Electron：一个 exe、
  没有 WebView、冷启动不闪白；代价是图标字体要自己带（见下）。
- **不需要第三方服务**。上游的 `api1.wslui.com` 客户端整个没有进这个仓库；唯一的外网调用是
  GitHub releases 的更新检查。
- **`schtasks` 用 `/XML` 读**，不解析 `/FO CSV` 的列号——那张表的表头随界面语言变（中文 33 列
  对英文 29 列），按列号取字段会静默拿错，本地化 Windows 上任务名会变成状态字符串。
- **配置**：`~/.wsadashboard/settings.toml`，带 `setting-version` 与迁移链；读永不失败、读永不写盘。

---

## 开始使用

### 前置条件

- Windows 11 x64，[WSA 已安装](https://learn.microsoft.com/windows/android/wsa/)并至少启动过一次
- [Android platform-tools](https://developer.android.com/tools/releases/platform-tools)（`adb`）
- Rust 1.90+ 与 MSVC 工具链（Visual Studio 生成工具，含 C++ 桌面开发负载）

`adb` 找不到时程序会在日志与提示里说明，并给出 `winget install Microsoft.PlatformTools`。

### 构建

```powershell
git clone https://github.com/yasunosubaru/wsa-dashboard.git
cd wsa-dashboard
powershell -File build.ps1            # 或用 cargo build --release --bin wsadashboard
.\target\release\wsadashboard.exe
```

`build.ps1` 把 Skia 预编译包的下载与缓存包好了（首次构建要下几十 MB，之后走本地缓存）。
仓库里不留 `target/`。

### 测试

```powershell
cargo test --release --bin wsadash                      # 单元测试
cargo test --release --bin wsadash -- --ignored --nocapture   # 实机测试
```

第二条会真的读本机的 `schtasks`、本机的 `settings.toml`、本机的 WSA。它**会写**：建一条任务、
跑一次 `Run Now`、再删掉，并用一个临时目录证明命令确实执行了。`running_apps_sees_a_real_running_app`
在没有运行中的应用时会打印 `skipping:` 然后报 ok——**跳过和通过长得一样**，看到它时要读那行输出。

```powershell
cargo test --release --bin wsadash -- --ignored --nocapture 2>&1 | Select-String skipping
```

---

## 用法

### 子系统的三档状态

首页不假设"连不上 = 没装"。四档各有自己的画面，且判定是纯函数、逐条有测试：

| 状态 | 触发条件 | 首页 |
| --- | --- | --- |
| `Ready` | adb 有端点且 `sys.boot_completed=1` | 应用网格 |
| `Booting` | 端点缺失但 VM 进程活着，或启动标志未置位 | 启动中，可停止 |
| `Offline` / `Unauthorized` | 端点在但连不上 / 未授权 | 提示重连 / 授权 |
| `Stopped` / `NotInstalled` | 端点缺失且 VM 不活 / 包未注册 | 启动按钮 / 安装指引 |

### 命令行

GUI 之外还有一个 CLI，用来在脚本里驱动同一套领域层：

```
wsadash probe                 报告状态（退出码见下）
wsadash list [--all]          已装包；--all 含系统包
wsadash info <package>        单包详情
wsadash launch <package>      启动一个应用
wsadash stop-app <package>    强制停止
wsadash install <apk>         安装
wsadash uninstall <package>   卸载（不带 -k，会清应用数据）
wsadash start / stop          冷启动并等待 / 关机
wsadash vhdx                  WSA 的存储镜像（不需要设备）
```

退出码：`0` 就绪 / `1` 用法 / `2` 启动中 / `3` 已停止 / `4` 未安装 / `5` 缺 adb /
`6` 操作失败 / `7` 未授权 / `8` 离线。

`wsadashboard.exe /scheduler` 是给计划任务用的第四条入口：不开窗口，把子系统叫起来就退出。

---

## 与上游的关系

这个仓库是 **[wsl-dashboard](https://github.com/owu/wsl-dashboard) 0.12.0 的衍生作品**：结构、
九个页签的 `.slint` 组件、i18n 包、计划任务的 cron 解析与提权原语来自上游，领域层
（`src/wsa/`、`src/adb/`）与配置/更新检查是为 WSA 重写的。逐件登记在
[`PROVENANCE.md`](PROVENANCE.md)。

上游是 **GPL-3.0-only**，所以这个仓库也是 GPL-3.0-only。这不是偏好问题：把衍生作品改成 MIT
会侵权，因此 `LICENSE` 不会换。

`docs/reference/` 是上游源码树的**只读基线副本**（规格 Q18），用途只有一个——让"与上游逐项
对齐"这件事可以被 diff 验证，而不是靠记忆。它同样是 GPL-3.0-only，版权归上游作者。

图标与托盘图用的是上游那张 256×256（规格 Q6 允许开发期复用），换图只需改 `assets/logo/logo.png`
一个文件。

---

## 验收状态

规格里有一份 11 条的验收清单，**只勾有证据的**，并且把三种"没通过"分开记：

| 状态 | 条数 | 说明 |
| --- | --- | --- |
| 通过 | 10 | 9 页签与切换、侧栏 54/200、主题 token、首页三态、冷/热启动计时、关子系统后磁盘/设置/关于可用、配置迁移链、i18n 无缺键、计划任务冷启动、单实例+托盘+关闭行为 |
| 部分通过 | 1 | 安装/卸载回环：替身回环通过，真机卸载会清应用数据，没拿在用的应用做实验 |
| 未通过 | 0 | — |

**跳过项**（能做但没做，各带原因）：真机卸载回环（缺一个可弃的 APK）。

逐条证据、实测输出与"为什么这样判"都在
[`docs/MAPPING.md`](docs/MAPPING.md)。那份文件也记着这一路被实机推翻的判断，比如：

- `schtasks /Query /XML` 对**启用**的任务根本不写 `<Enabled>`，读成"关"会让每个任务显示为停用、
  开关变成死键；
- `/TR` 会被 `schtasks` **拆成 `<Command>` + `<Arguments>`**，只看前一半会把任何脚本类任务读成
  `wscript.exe`；
- `cmd /c "<powershell -Command "…">"` 的嵌套引号会把命令撕碎，而失败被记成一条 warn。

---

## 路线图

| 阶段 | 交付 | 状态 |
| --- | --- | --- |
| P0 | 仓库骨架、`build.ps1`、空窗口 | 完成 |
| P1 | 领域层 `src/wsa/` + `src/adb/` | 完成 |
| P2 | 首页与三态降级 | 完成 |
| P3 | 侧栏折叠、页签切换、主题、标题栏 | 完成 |
| P4 | 安装 APK、设备页 | 完成 |
| P5 | 网络页、存储页 | 完成 |
| P6 | 设置页、关于页、i18n 桥与语言包 | 完成 |
| P7 | 计划任务、托盘、单实例 | 完成 |
| P8 | 配置迁移、更新检查、打包 | 完成 |
| P9 | 与上游逐页对照验收 | 10/11 |

待办（按优先级）：

- **网络页的"自动转发"任务**：`adb forward` 重启后不保留，现在没有规则存储，所以这个按钮仍是
  隐藏的。需要先有规则的持久化，再有一条 ONLOGON 任务跑恢复模式。
- **捐赠页**：上游有，本仓库没有收款内容，等有内容再启用。
- **真机卸载回环**：需要一个可弃的 APK。
- **`/scheduler` 写日志**：它在日志初始化之前返回，所以冷启动失败时日志里没有痕迹。

---

## 参与贡献

有想法就提 [issue](../../issues/new)，也可以 fork 后发 PR。

**提交纪律**（不是形式主义，每一条都被踩过）：

1. 先写一条会红的测试，再让它变绿。修 bug 前先有一个能复现它的用例。
2. **不要 `git add -A`**，逐个列路径。
3. 改 `.slint` 就要跑 i18n 测试：它扫描**全部** `.slint` 文件，复制了却没接的键会被点出来。
4. 涉及数值/路径的判断，先看 `docs/MAPPING.md` 里有没有实测记录——那里写着哪些"显然的做法"在
   本地化 Windows 上是错的。

**脱敏纪律**：这个仓库是公开的。机器名、用户名、绝对个人路径不进仓库（引用真实输出时换成
`reader` / `DESKTOP-EXAMPLE` / `<repo>`），也不放邮箱与学号。协作走 issue。

---

## 许可证

[GPL-3.0-only](LICENSE)，版权归各自作者：本仓库自有部分归 WSA Dashboard 贡献者，衍生自上游的
部分归 wsl-dashboard 作者。

一句话版本：这是 WSL Dashboard 的衍生作品，上游是 GPL-3.0-only，所以这里也只能是
GPL-3.0-only；再分发时要保留同样的许可与源码可得性。

---

## Notices（开源许可登记）

| 项目 | 用在本仓库的哪里 | 许可 |
| --- | --- | --- |
| [wsl-dashboard](https://github.com/owu/wsl-dashboard) 0.12.0 | 结构、九个页签的 `.slint`、i18n 包、cron 解析、提权原语；`docs/reference/` 是它的只读基线副本 | GPL-3.0-only |
| [Slint](https://slint.dev/) 1.17 | 全部 UI 与渲染（`renderer-skia`） | GPL-3.0 / 商业双许可；此处按 GPL-3.0 使用 |
| [Skia](https://skia.org/)（经 `skia-bindings`） | Slint 的渲染后端 | BSD-3-Clause |
| [Segoe Fluent Icons](https://learn.microsoft.com/windows/apps/design/style/segoe-fluent-icons-font) 子集 | `assets/font/icons.ttf`，图标字形（74 字形子集，来自上游） | Microsoft 字体许可（随 Windows 分发） |
| [Rust 标准库](https://www.rust-lang.org/) 及 `tokio` / `serde` / `toml` / `chrono` / `ureq` / `windows` / `tray-icon` / `rfd` / `roxmltree` / `ico` / `embed-resource` / `image` / `md-5` | 运行时与构建 | MIT / Apache-2.0（逐包见 `Cargo.lock`） |
| [Android platform-tools](https://developer.android.com/tools/releases/platform-tools) | 外部依赖，**不随仓库分发** | Android SDK 许可 |

徽章与许可标志只是速记，以本表与 `Cargo.lock` 为准。

---

## 致谢

- [wsl-dashboard](https://github.com/owu/wsl-dashboard)：上游。没有它，这个仓库现在只是一份方案。
- [Slint](https://slint.dev/)：一个 exe、无 WebView 的原生 UI。
- [Best-README-Template](https://github.com/othneildrew/Best-README-Template) 与同事项目
  [field-station-agent](https://github.com/yasunosubaru/field-station-agent)：本页的结构与语气。
