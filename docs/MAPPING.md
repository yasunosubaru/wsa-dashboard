# MAPPING.md — wsl-dashboard 0.12.0 → wsa-dashboard 标识符与文案映射

> **总原则（spec Q12）**：**标识符、i18n 键名、文件名一律保留参照原名**，只替换**显示文案**。
> 这样 50 种语言 × 563 键的翻译可以几乎零成本复用，且迁移链、回调接线代码可逐行对照。
>
> 所有 `file:line` 指向固化在 `docs/reference/` 的参照副本（v0.12.0），不是 Downloads 里的原件。

---

## 1. 为什么保留原名

| 收益 | 说明 |
|---|---|
| 逐行对照 | 改 `src/ui/handlers/*` 时能一眼看出我们抄的是哪一段 |
| 50 语翻译复用 | `en.toml` → `zh.toml` 等 49 份的值里，只有少量"WSL/distro"字样需要替换 |
| 迁移链可复刻 | `config/migration.rs` 的 v0→v10 逐步迁移按字段名逐行照搬 |
| i18n 校验不炸 | `build.rs::verify_translations` 以 `en.toml` 为基准，键名不变则零缺键警告 |

**唯一例外**：值里的领域词必须改（"WSL"→"WSA"、"distribution"→"Android app"），
但**键不动**。改动量见 §3。

---

## 2. 标识符映射

### 2.1 硬字符串（运行时身份，必须一致才能复用 1:1 逻辑）

| 参照（原名，保留） | 参照取值 | 本项目取值 | 出处 |
|---|---|---|---|
| `APP_NAME` | `"WSL Dashboard"` | `"WSA Dashboard"` | `src/app/constants.rs:6` |
| `APP_ID` | `"wsldashboard"` | `"wsadashboard"` | `src/app/constants.rs:8` |
| 根组件 `title` | `"WSL_DASHBOARD_WINDOW_UI"` | `"WSA_DASHBOARD_WINDOW_UI"` | `src/ui/app.slint:46` |
| 运行时窗口标题 | `"WSL Dashboard Main"` / `"WSL Dashboard Tray"` | `"WSA Dashboard Main"` / `"WSA Dashboard Tray"` | `src/app/window.rs:400,409` |
| 单实例互斥体 | `wsldashboard-v0.3-lock` | `wsadashboard-v0.3-lock` | `src/app/lifecycle.rs:12` |
| 单实例唤醒管道 | `\\.\pipe\wsldashboard_wakeup_pipe_v0.3` | `\\.\pipe\wsadashboard_wakeup_pipe_v0.3` | `src/app/tasks/mod.rs:128` |
| 配置目录 | `~/.wsldashboard/` | `~/.wsadashboard/` | `src/config/mod.rs:31,36,41,343,538` |
| 日志/临时 | `~/.wsldashboard/logs`、`.../temp` | 同名换根 | `src/config/models.rs:167-168` |
| 脚本目录 | `~/.wsldashboard/scripts` | `~/.wsadashboard/scripts` | `src/network/scheduler.rs:18` |
| 主 exe | `wsldashboard.exe` | `wsadashboard.exe` | `Cargo.toml [[bin]]` |
| 计划任务根路径 | 含 `.wsldashboard` | 含 `.wsadashboard` | `src/app/scheduler_task/schtasks.rs:354` |
| elevation 临时日志 | `wsldashboard_elevated_<ts>.log` | `wsadashboard_elevated_<ts>.log` | `src/utils/system.rs:140` |
| 卸载临时脚本 | `wsldashboard_uninstall_tasks.ps1` | `wsadashboard_uninstall_tasks.ps1` | `src/app/uninstall.rs:145` |

> **`window.rs:397` 的匹配条件**是 `title.contains("WINDOW_UI") || title == "WSL Dashboard Main" || class_name.contains("Slint"`。
> 我们把 `title` 换成 `WSA_DASHBOARD_WINDOW_UI`，`contains("WINDOW_UI")` 仍命中 ⇒ 逻辑无需改动。

### 2.2 类型 / 函数名（保留原名，改字段语义）

| 参照标识符 | 保留 | 语义替换 |
|---|---|---|
| `DistroOpGuard`（`src/wsl/dashboard/operation_guard.rs`） | 保留结构，**类型名改为 `WsaOpGuard`** | op 名 `Starting/Stopping/Restarting/Deleting/Exporting/Importing/Moving` → `Starting/Stopping/Launching/Installing/Uninstalling` |
| `ApplicationConfig` / `UserSettings` / `TraySettings` / `SidebarConfig` | 全保留 | `distro` 字段 → `subsystem`（**字段名保留 `distro`，见下注**） |
| `CachedDistro`（`src/config/models.rs:330`） | 保留 | 存的是 APK 包缓存元数据 |
| `UsbConfigFile` / `UsbDeviceConfig` | 保留 | 存 adb 设备配置 |
| `NetworkConfig` / `NetworkRule` | 保留 | 规则后端 netsh → `adb forward`/`reverse` |

> **注意**：迁移链依赖**字段名**逐行对齐，所以 `Config` 内部的 `distro` 等字段名**保留不改**，
> 只在读写处做 WSA 语义解释。真正的命名替换只发生在**新写的类型**上（`AppInfo`、`SubsystemStatus`）。

### 2.3 侧栏 9 页签（键名保留，值替换）

`src/assets/i18n/en.toml` 的 `[sidebar]` 表（9 个页签键全保留）：

| 键名（保留） | 参照值 | 本项目值 |
|---|---|---|
| `home` | Home | Home |
| `add` | Add an instance | Add an app |
| `usb` | USB devices | Devices |
| `network` | Network | Network |
| `scheduler` | Task scheduler | Task scheduler |
| `mount` | Mount disk | Storage |
| `settings` | Settings | Settings |
| `about` | About | About |
| `donate` | Donate | Donate |
| `toggle` | Toggle | Toggle |

> 页面顺序、折叠宽度（54/200px）、拖拽重排**全部照搬**（spec §3）。

---

## 3. i18n：27 表 × 563 键

`docs/reference/assets/i18n/` 共 **50 个语言文件**，每个 **精确 563 键**、**27 个表**：

```
common  sidebar  network  settings  settings.languages  settings.log_levels
settings.log_retention_options  settings.update_options  dialog  operation
install  about  donate  add  add.sources  distro  tray  wsl_conf
usb  usb.detail  main  toast  task  debug  mail  scheduler  mount_disk
```

**替换规则**：
1. **键名一个都不改** ⇒ `build.rs::verify_translations` 零缺键警告（`build.rs:145-197`，只 `cargo:warning`，从不 fail）。
2. **值里改领域词**，词表固定（下节）。
3. 表名 `distro`、`wsl_conf` **保留**（键名不改原则）；`[distro]` 的值全部改写为 WSA 语义。
4. **本项目新增的键**在 §4 单独列出，不混入既有表。

### 3.1 值替换词表（50 语言 × 563 键的改动面）

| 参照词（各语言对应译法） | 本项目 |
|---|---|
| WSL | WSA |
| distribution / distro | app |
| instance | app |
| WSL instance | Android app |
| mount disk | storage |
| `api1.wslui.com` 相关值 | 删除（见 §5） |

> 具体各语言的替换在 P5 用脚本批量做，先靠 `verify_translations` 的 warning 驱动。

---

## 4. 语义不适用项（结构保留，隐藏或改数据源）

| 参照项 | 处理 | 阶段 |
|---|---|---|
| `handlers/distro/` 的 `install_logic` / `export` / `clone` / `compress` / `move_distro` / `mirror_install` | **隐藏对应按钮**（`visible: false`），**回调全保留**（Q22），i18n 键全留 | P2 |
| `vscode_distro` / `terminal_distro` 动作 | 隐藏 | P2 |
| 磁盘页 `wsl --mount` 语义 | 改"WSA 存储管理"：VHDX 元数据 + `adb shell df` + 应用占用 Top N（Q32） | P3 |
| 网络页 netsh portproxy / 防火墙规则 | 改 `adb forward` / `adb reverse`；netsh 段隐藏（Q33） | P3 |
| HTTP 代理块 | **保留并实现**（实测 `settings put global http_proxy` shell 可写） | P3 |
| 10 个 `api1.wslui.com` 端点（`src/api/*`） | **全部删除**，`src/api/` 只留 GitHub releases 更新检查 | P5 |
| 关于页更新区块 | 显示但灰掉，"更新源未配置"（Q15） | P6 |
| 捐赠页收款内容 | 保留布局 + 灰色占位块（Q5/Q16） | P6 |
| 计划任务两套（应用内 + Windows 任务计划） | **都保留**（Q34） | P7 |

### 4.1 本项目新增（参照没有）

| 新增项 | 说明 |
|---|---|
| **首页搜索框** | 用于在应用列表中检索；标注为本项目新增（Q24） |
| 无图标占位 | 包名首字母纯色块，颜色按包名哈希（Q29） |
| `wsadash` CLI bin | 参照没有独立 CLI 包（其 `/` 开关在 GUI 进程内）（Q19） |

---

## 5. 禁用项（硬性）

- **`API1_URL = "https://api1.wslui.com"`（`src/app/constants.rs:52`）及其 10 个端点一律不调用。**
  参照 `src/api/` 的端点：`/common/v1/releases/version`、`/desktop/v1/helper/{about,distro,install,donate,sync,bootstrap,scheduler,mount}`、`/utils/ping`。
- `PROJECT_WEBSITE` / `PROJECT_DOCS`（`wslui.com`）、`PROJECT_REPOSITORY`（`owu/wsl-dashboard`）改空串或本项目地址。
- `WSL_GITHUB_RELEASES`（`microsoft/WSL/releases/latest`）改 WSA 相关指引或删除。
- `WSL_INIT_SCRIPT = "/etc/init.wsl-dashboard"`（`src/app/constants.rs:108`）不适用，删除。

---

## 6. 构建产物基线（P9 验收用）

固化在 `docs/reference/` 的参照可作为视觉与行为基线：

- 58 个 `.slint`（`src/ui/{app,constants,theme}.slint` + `components/` + `views/`）
- 38 个 `.rs`（`src/ui/handlers/**` —— 127 个顶层回调的实现层）
- 50 个 i18n `.toml`
- `Cargo.toml` / `Cargo.lock` / `build.rs` / `rust-toolchain.toml`（**stable**，非 nightly）

参照本体**不编译**（缺 `src/` 其余部分），它是只读对照基线。

---

## 7. 运行时依赖与交付形态（spec Q3 / Q14，2026-10-10 实测定论）

`dumpbin /dependents target/release/wsadashboard.exe` → **35 个 DLL**，其中**只有 2 个不是 Windows 内置**：

| DLL | 来源 | 处理 |
|---|---|---|
| `MSVCP140.dll` | Skia 的 C++ 静态库 → C++ 运行库 | **声明为前置依赖，不打包** |
| `VCRUNTIME140.dll` | MSVC 运行库 | 同上 |
| 其余 33 个 | `dwrite` `dxgi` `d3d12` `D3DCOMPILER_47` `OPENGL32` `bcryptprimitives` `api-ms-win-crt-*` … | Windows 10+ 内置 |

### 7.1 为什么不静态链接 CRT

试过 `-C target-feature=+crt-static`，**已回退**，原因链记录在 `.cargo/config.toml`：

1. `+crt-static` 会让 skia-bindings 的缓存 key 追加 `-static`；
2. `rust-skia/skia-binaries` tag `0.99.0` 发布 13 个 `x86_64-pc-windows-msvc` 资产，**没有任何一个以 `-static` 结尾** → 404；
3. 回落到源码编译 → 死在 `Unable to locate LLVM installation`。

### 7.2 参照自己的答案（1:1 照抄这条）

参照面临**完全相同**的处境（slint 1.17.1 + renderer-skia），它的处理是**声明依赖**而不是打包或静态化：

| 出处 | 内容 |
|---|---|
| `build/portable/build.ps1:119-128` | zip 里**只放一个 exe**，不带任何 DLL |
| `build/winget/build.ps1` | `PackageIdentifier: Microsoft.VCRedist.2015+.x64` |
| `manual/README_uz.md` | 系统要求列 "Visual C++ Redistributable - 2015-2022" |

**结论**：Q3 的"只要 release exe"成立；Q14 的"发现 DLL 就破例做 zip"**不触发**——
那两个 DLL 是文档化的前置依赖，不是打包问题。P8 做 winget / 安装器清单时把这条依赖写进去。
