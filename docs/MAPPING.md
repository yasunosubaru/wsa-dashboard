# MAPPING.md — wsl-dashboard 0.12.0 → wsa-dashboard 标识符与文案映射

> **脱敏说明（公开仓库）**：本文件引用真实命令输出时，机器名、用户名与本地绝对路径已替换为 `DESKTOP-EXAMPLE` / `reader` / `<repo>`；数值、状态、退出码与结论一律未改动，替换只在标识符上。

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
| `src/wsa/app/wsa_path.rs` | 参照不需要"定位宿主"（WSL 内置于 Windows），WSA 侧载包必须运行时解析（spec §1.1 / R6） |
| `src/wsa/dashboard/` | 参照为 `wsl/dashboard/mod.rs` + `wsl/ops/*.rs`；见 §4.1.2 |

#### 4.1.1 `WsaCommand` 变体命名（spec §2.3 意图名 → 实际变体名）

spec §2.2 散文与 §2.3 表格用了两套名字，且 §2.3 把**非 adb 操作**也列进了同一张表。落地时：

| spec §2.3 意图 | 落地位置 | 说明 |
|---|---|---|
| `ConnectAdb` | `WsaCommand::ConnectAdb { port }` | |
| `DeviceProps` | `WsaCommand::GetProps` | §2.2 散文作 `GetProp` |
| `ListPackages` | `WsaCommand::ListPackages { include_system }` | |
| `PackageInfo` | `WsaCommand::PackageInfo { package }` | |
| `StopApp` | `WsaCommand::AmForceStop { package }` | 命令名而非意图名，避免与 `StopSubsystem` 混淆 |
| `InstallApk` / `UninstallApk` | 同名变体 | |
| `ForwardPorts` / `ReversePorts` | `ForwardAdd`/`ForwardRemove`/`ReverseAdd`/`ReverseRemove` | 一条命令一个变体 |
| **`LaunchApp`** | ❌ **不进枚举** | 落地为 `Start-Process wsa://<pkg>`，无 adb argv；Task 11 用 `open::that`（spec §1.2）。§2.2 散文的 `AmStart` 同步取消 |
| **`StartSubsystem`** | ❌ 不进枚举 | 同上，`wsa://<任一已装包>` |
| **`StopSubsystem`** | ❌ 不进枚举 | 终止 `WsaClient.exe` 进程，非 adb |
| **`OpenSettings`** | ❌ 不进枚举 | `explorer shell:AppsFolder\…!SettingsApp` |
| **`VhdxInfo`** | ❌ 不进枚举 | 读本地 `LocalState\*.vhdx`，不调 adb |
| — | `WsaCommand::ListAdbDevices` | 新增：`adb devices`，probe 三态判定的输入 |
| — | `WsaCommand::Shell { args }` | 新增：逃生口；`argv()` 会补前缀 `shell` |

**判据**：`WsaCommand` 是 **adb 命令面**。不经过 adb 子进程的操作没有 argv，也不该过 executor（spec §2.2 的"唯一出口"约束的是 adb 调用），由 ops 直接实现。

#### 4.1.2 ops 层 `src/wsa/dashboard/{mod,ops}.rs`（Task 11）

参照对应物是 `src/wsl/dashboard/mod.rs` + `src/wsl/ops/{lifecycle,transfer,info,…}.rs`。
本项目把九个操作合在**一个** `ops.rs`：spec §2.2 的操作本就是一张表，按参照那样拆成
七个文件只会增加跳转而没有边界收益（参照拆分的 `compress`/`sparse` 在 WSA 侧不存在）。
`WsaDashboard` 与参照同名同形状（executor + 路径），但**不带**参照的 `distros` 缓存、
`Notify`、`refresh_interval`——参照需要它们是因为 `wsl -l -v` 是快照命令，本项目所有读
都过 `WsaCommandExecutor`，再存一份缓存只是把同一状态放两处。

| 项 | 参照 / 计划原文 | 本项目落地 | 依据 |
|---|---|---|---|
| 状态枚举 | `WslStatus` 三态 | `SubsystemState` 增 **`Offline`**（Task 3 已定 `Unauthorized`） | spec §5 把 `offline`/`unauthorized` 归 **T2**（"重连"按钮），不能与 T3"未运行/启动"同渲染；spec §1.3 实测杀 `WsaClient` 后 adb 先变 `offline` |
| 探活 | `wsl -l -v`，只读 | `probe()` 在**条目缺失时自动 `adb connect` 一次** | spec §1.4：WSA 停止后 adb server 把条目**整个清掉**，系统里没有任何东西会再连回去——只读的 probe 会永远报 `Stopped`。实测：`adb disconnect 127.0.0.1:58526` 后 `adb devices` 确实空行，`adb connect` 后恢复 `device` |
| 启动 | 参照 `instance.rs` 的启动 handler | `open::that("wsa://<pkg>")` / `wsa://com.android.settings` | spec §1.2：`WsaClient /launch` 七种机制 **exit 0 零动作**，`wsa://` 是唯一有效路径 |
| 停止子系统 | `wsl --terminate` | `taskkill /F /IM WsaClient.exe` → 轮询 `vmmemWSA` 消失（30s） | spec §1.3：`/terminate` 零动作；杀进程才让 `vmmemWSA` 干净下线 |
| 轮询目标 | — | `tasklist /FI "IMAGENAME eq vmmemWSA*"` | **实测坑**：进程名是 `vmmemWSA` **不带 `.exe`**；`IMAGENAME eq vmmemWSA.exe` 匹配 0 行，而 **`tasklist` 匹配与否都 exit 0** —— 过滤器写错时退出码救不回来，只能读 stdout。`*` 兼容两种拼写 |
| 错误类型 | 计划只写 `Result<T>` | `OpsError { Adb, Activate, Helper, Rejected, NoData }` | 九个操作里有两个不跑 adb（`wsa://` 激活）、两个跑 Windows helper（`taskkill`/`tasklist`），塞进 `ExecError` 会让调用方靠字符串猜原因 |
| 成功判定 | 计划："检查 exit code 与 stdout 含 `Success`" | **两者都要** | adb 有 exit 0 却打印 `Failure [...]` 的先例（Task 10 实测，且它打在 **stdout**）。失败关闭：假"失败"会重试，假"成功"会丢安装 |
| 操作守卫 `WsaOpGuard` | `DistroOpGuard`：`async create` 按 distro 名存 map、**无条件插入**、`Drop` **spawn 后台任务** unregister | **同步** `create -> Result<_, OpBusy>`（拒绝而非覆盖）、单槽位 `Mutex<Option<&'static str>>`、`Drop` **同步**清空 | spec §2.2"注册过则 UI 弹 `toast.distro_busy`"＝拒绝。参照 `Drop` 没有 runtime 时只 `error!` 一句，**槽位会永久留在忙碌且无人发现**；同步锁让 `Drop` 不可失败。参照先 drop 的 guard 会清掉后注册的标签（last-out-wins）；单槽位没有顺序可错 |
| `heavy_op_lock` | dashboard 自带一把，靠每个 UI handler 自己记得 `try_lock()` | `WsaDashboard::heavy_op_lock()` **委托**到 executor 的同一把 | spec §2.2 要求存在；但 Task 10 的 `run` 已对所有 `is_heavy()` 命令持有它**一整个往返**。再加一把＝同一个不变量的两份真相，会在最坏时刻分叉 |
| `stop_app` 守卫 | 参照无对应物（没有"停单个应用"） | **不拿 guard** | spec §2.2 只授权五个 op 名，`stop_app` 不在其中；复用 `Stopping` 会让 busy 提示谎称是子系统操作，新增第六名又需 spec 没要求的 i18n 键。`am force-stop` 也无状态可破坏 |

**三项故意不做单测**：`launch_app` / `start_subsystem` / `stop_subsystem`。
唯一能验证它们的办法就是**真启动、真杀掉用户自己的子系统**，任何单测都会在每次
`cargo test` 时把读者机器上的 WSA 关一遍。可测的部分已被抽出来钉住：
`launch_uri`/`start_uri` 的字符串（spec §1.2 的核心结论）、`wait_until` 的超时分支
（`stop_subsystem` 里唯一会出错的地方）。三项由 Task 13 的 CLI 手工验收。

**假 adb**：`ops.rs` 的测试用一个 `.cmd` 冒充 adb——executor 只决定程序和 argv，不知道
程序做什么，所以让它指向一个从自身目录读"应答文件"的批处理，每个测试就能决定 adb 说了
什么。写这个之前先实证过：`std::process::Command` **能**在 Windows 上直接执行 `.cmd`
（走 `cmd.exe /c`），且 `adb devices` 的 **tab 分隔**能原样穿过往返。批处理是**纯 ASCII**
且状态目录取自 `%~dp0`（不是烤进去的路径），所以临时目录带空格或非 ASCII 也不会坏。

### 4.1.3 CLI（`src/wsadash.rs`）与端到端验证中测到的事实

参照**没有 CLI**：它的九个动词是进程内的 `/` 斜杠命令。所以 §4.1 的"1:1 映射"在这里
不适用，下面记的是本项目自己定的接口，以及真机验收时才暴露出来的事实。

| 事项 | 结论 | 依据 |
|---|---|---|
| 参数解析 | **不引 `clap`**，手写匹配 | 参照也没用；一个只为读九个动词的依赖会成为 CLI 重新编译的唯一理由 |
| 退出码 | `0` 就绪/成功、`1` 用法、`2` Booting、`3` Stopped、`4` 未安装、`5` 缺 adb、`6` 操作失败、`7` Unauthorized、`8` Offline | 计划只列了四个状态，但 `SubsystemState` 有六个。缺码会让"需要用户去 WSA 点同意"和"真的坏了"撞成同一个数 |
| `WSADASH_ADB` | 环境变量，覆盖 PATH 里的 `adb` | Review Focus #1 说 adb 不保证在 PATH；也是 Q8 要求的"不依赖实机"唯一能落地的方式——把 CLI 指向替身 adb |
| 面向设备的动词 | 先跑一次 `probe`（`require_device`），非 `Ready`/`Booting` 就用**该状态自己的退出码**返回并提示怎么办 | 否则 `wsadash list` 打一个停着的子系统会拿到 adb 那句谁也看不懂的失败，而不是"Stopped —— 请先 `wsadash start`"。也复用了 Task 11 已测过的自愈重连 |
| 读不懂的包列表 | 判定 `adb/decoder.rs::listing_is_unparseable`（纯函数），警告在 `ops.rs::list_apps` 里 `tracing::warn!` | Review Focus #3。放在 ops 层而非 CLI：那里是唯一同时握着"原始回复"和"解析结果"的地方，GUI 从 P2 起免费拿到同一条警告，不必再跑一次 `pm list packages`。判定**关于回复本身**而不是解析结果——`filter_system` 合法滤空或真的没有用户应用时，必须保持沉默 |

**端到端跑出的真实缺陷（已修，Task 10 的盲区）**：`ExecError::reason()` 原先优先取
stdout。真机拿一个 4 KB 假 apk 跑 `install`，adb 把 `Performing Incremental Install` /
`Performing Streamed Install` 打在 **stdout**，而把真正的
`INSTALL_PARSE_FAILED_NOT_APK` 打在 **stderr**——用户于是只看到两行进度加一个退出码，
完全不知道哪里错了。但也不能简单改成只看 stderr：Task 10 记录的"`Failure [...]` 在
stdout"同样是实测。定论是 **stderr 在前、stdout 跟在后面，两者都留**；3 个单测把两种
形态和"两个流都空"钉住。

**验收替身的延时用 `for /L` 而不是 `ping`**：`ping` 是孙进程，父进程被杀后必然还活着，
而 Review Focus #5 要验证的恰恰是"杀完还剩什么"。留一个注定残留的孙进程会污染结论。
`for /L` 由 cmd.exe 自己执行，杀掉 cmd.exe 就什么都没有了。

**杀掉 `WsaClient.exe` 之后 `probe` 报 `Offline` 而不是 `Stopped`**：这是 Task 11 记下的
实测事实（陈旧条目要老化才消失），不是 bug。spec §5 把陈旧条目归 T2"重连"、`Stopped`
归 T3"启动"，两者本来就必须渲染成不同的东西。P2 的重连按钮要处理的正是这一态。

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

---

## 8. P2 首页页面的逐组件差异

### 8.1 零新增组件（P9 裁定）

P2 **没有新增任何 `.slint` 组件文件**。参照首页需要的每一样东西，参照都已有对应组件，
差异只在属性和取值，逐条列在 8.2。"参照没有的组件"按 P9 的硬裁定算缺陷，
所以三态降级全部**复用**参照的现有组件：`LoadingOverlay` 用参照 `main_view.slint:277`
的加载覆盖层，`NotFound` 用 `DistroNotFoundView`，`Skeleton` 用空 `Distro` 行。
`StatusCard` / `GridMode` 两个中间产物在 Task 8 前废弃。

### 8.2 组件 delta（对照参照同名文件）

| 组件 | 差异 | 依据 |
|---|---|---|
| `main_view.slint` | 新增**搜索框**（`CustomLineEdit`），`search_query` 双向绑定 | 参照靠侧栏导航，Android 应用列表长得多，Q24 |
| `main_view.slint` | 新增 `greyed` / `buttons_disabled` 两个入参，透传到卡片 | 三态降级要有"有旧快照但非 Ready"的置灰态 |
| `distro_card.slint` | `enabled` 改由 `buttons_disabled` 驱动；`text` 取本地化后的状态名 | 同上 |
| `distro_not_found_view.slint` | 文案拆成 `distro_not_found_text` / `guide_text` 两个入参，不再组件内查 i18n | `HomeViewModel` 已经在算文案，查两遍会出现两个真相 |
| `theme.slint` | `Distro` 结构体新增 `initial_color_index`（`u8`）与 `badge_tints` 数组 | Q29：色号按包名哈希取 8 个 Catppuccin 色之一 |
| `distro_card.slint` | 删除参照的 `show_shield` 条件块（属性保留） | P2 时写的是"盾牌表示 WSL 发行版来源，WSA 侧无对应概念"——**错了**。量过参照：42 处 `show_shield: true` 全在 `mount_disk_card`/`network_rule_card`/`scheduler_card`/`usb_card`/`port_forwarding` 及其对话框，加上 `mount_disk_view`/`scheduler_view`，都是 P5–P7 的页面；`distro_card` 自己一处也没设。所以理由是"这页不设它"，块随第一个会设它的组件（P5）回来 |
| `theme.slint` | `GUIDE_URL` 改 `wsa://com.android.settings` | spec §1.2：设置页只能经 `wsa://` 激活 |
| `form_widgets.slint` | **整文件照抄**，未改一字 | 搜索框直接复用参照的输入控件 |
| `task_status_toast.slint` | **未改一字**；命名 `task_status_*` 而非 `home_toast_*` 保留 | 参照里 toast 是窗口级浮层（在所有对话框之上），P3 加对话框后仍是 |
| `app.slint` | 新增 `window_title` 入参属性，`title: window_title` | Slint 1.17 的 `Window` **只有 title 的 getter 没有 setter**，从 Rust 命名窗口的唯一通路 |
| `app.slint` | `export { Theme, Distro, AppInfo, AppI18n, AppApi }` | 参照 `src/ui/app.slint:43` 同样从根重导出；没有它 `app.global::<AppI18n>()` 解析不了 |
| `main_view.slint` | `start_distro` / `stop_distro` 的 action 参数由 `d.name` 改 `d.distro_display_name` | 参照 `name` 是 WSL 发行版名，本项目 `name` 是包名而显示名另存 |
| `refresh.rs` | `repush_needed(previous: Option<(Option<&HomeSnapshot>, &str, usize)>, …, placeholders: usize)` | 见 8.3 |

### 8.3 `repush_needed` 多出的 `placeholders` 维度

`repush_needed` 原本只比较"上次的快照 / 这次的快照 / 上次的查询串"。**有快照时的离线态**
能正确重推（快照变了），但**两侧都没有快照**时（首次轮询、adb 从来没通过），
它判定"没什么可推"→ `set_distros` 从不执行 → 列表空、`has_error` 假 → **整页空白**，
违反 P9。三态里 `Skeleton` 恰恰就是"无快照 + 有占位行"，所以占位行数必须是一个独立维度：
两个相邻维度（快照、查询串）都不能因为快照缺失而消失，否则占位行的数量记不住，
下一轮又会被判成"没变"。先写三个红测试再改实现。

### 8.4 `handlers/home.rs` 为什么没有测试

参照是 bin-only crate，`#[cfg(test)] mod tests` 内联。带 `AppWindow` 的模块测试跑不到
（GUI 的 `mod ui` 不进 `wsadash` 这条测试链），所以可测的判断逻辑一律上移到纯模块：
`build()` / `busy_text()` / `placeholder_count()` / `repush_needed()` 都在 `home_model.rs`
或 `refresh.rs` 里，`data.rs` 与 `handlers/*.rs` 按 `data.rs` 的先例只写一行注释说明
"本文件无测试"。`home.rs` 的两个测试因此改测 `home_model::busy_text`——抽它出来是因为
忙碌文案本来就是模型的一部分，不该埋在 handler 里。

### 8.5 三态降级的实测取值

| `SubsystemState` | `Degraded` | 渲染 |
|---|---|---|
| `Ready` | `None` | 参照的列表 |
| `Booting` | `LoadingOverlay` | 参照的加载覆盖层（`main_view.slint:277`） |
| `Unauthorized` / `Offline` | `Skeleton` | 空 `Distro` 行骨架 + toast |
| `Stopped` / `NotInstalled` | `NotFound` | `DistroNotFoundView` + `wsa://com.android.settings` 链接 |
| 非 `Ready` 但有 `last_good` | `Skeleton` | 行非空 + `greyed = true` + toast |

**两处对 spec §5 的放宽**（计划 `2026-10-11-p2-home-page.md` 已记）：
①骨架占位与状态卡进度互斥，取"不空白"这个实质；②"重连"按钮取消——
`probe()` 在条目缺失时自动 `adb connect` 一次并由 5 秒轮询复查（§4.1.2 已记实测依据）。

**`ps -A` 保留**：spec §1.3 只说 `ps` 在 WSA 上"输出格式与 WSL 一致"，实机确认可用，
所以 `running_apps()` 保留该命令面，返回 `HashSet<String>`（不返回 `Result`——
它只是过滤条件，失败时视为"没有运行中的应用"更接近真值）。

### 8.6 真机跑出来的缺陷：`Stopped` 与 `Booting` 被合并了

**实测**（2026-10-12，本机，WSA 确实在跑）：`WsaClient.exe` 与 `vmmemWSA` 两个进程都在，
但 58526 端口**无人监听**，`adb connect` 被直接拒绝（"目标计算机积极拒绝"）。
此时 `probe()` 报 `Stopped`，首页于是显示 `DistroNotFoundView`：
**"WSA is not running" + 一个 "Start WSA" 按钮**——对一台正在启动的机器说它没在运行，
还让用户去启动它。

根因："adb 里没有我们这个端口的条目"是**两种**情况：

| 情形 | 该报 | 该渲染 |
|---|---|---|
| 子系统没跑 | `Stopped` | `DistroNotFoundView` + 启动链接 |
| 子系统在跑，Android 还没开 adb 端点 | `Booting` | 加载覆盖层 |

§4.1.2 记的"停止后条目被整个清掉"只覆盖了第一种（所以才需要自动 `adb connect`），
**没有覆盖第二种**。两者的区别只能问 hypervisor 的进程，也就是 `stop_subsystem`
已经用着的那个 `vm_worker_present()`。

改法：抽出 `state_for_absent_endpoint(vm_worker_alive: bool)`，并把
`WsaDashboard::probe()` 拆成 `probe()`（真实现）与 `probe_with(vm_worker_alive)`
（供替身测试注入答案）。**必须是参数**：那个判断是一次跑在**读者机器**上的
`tasklist`，测试直接调它就是在断言"跑测试时 Windows 正在跑什么"。

**方向是刻意选错的**：刚死的子系统可能还有 `vmmemWSA` 残留一瞬，那一跳报 `Booting`
只是转圈；反过来报一次错的 `Stopped` 会让用户去启动一个已经启动过的子系统。

先写红测试（`an_absent_endpoint_is_booting_when_the_vm_worker_is_alive`）再改实现。
改完真机复跑：`probe` 从 `Stopped`（exit 3）变成 `Booting`（exit 2，hint "still booting"），
`wsadash start` 冷启动后到 `Ready`。

### 8.7 真机截图

`capture.ps1 -UseRealAdb` 走**真 adb**（不设 `WSADASH_ADB`，让它落到 PATH），
产出 `docs/screenshots/p2-real.png`：Magisk / Ham / Launcher 三张真卡，
首字母色块按包名哈希取色。替身只能证明替身自己，**这一张才证明 `WSADASH_ADB`
这条通路和真实 adb 都工作**。

---

## 9. 构建/验收环境实测（本机，2026-10-12）

这三条不是代码问题，是"在这台机器上截图"的问题，但每一条都曾被误判成代码缺陷，
所以写下来：

| 事项 | 事实 |
|---|---|
| **显示缩放 150%** | `GetDpiForMonitor` = **144**。捕获脚本若进程 DPI-unaware，`GetClientRect` 返回**逻辑** 800×600 而窗口实际渲染 **1200×900**，`PrintWindow` 于是**裁剪**而非缩放。首页 toast 在逻辑 y=534＝物理 y=801，落在画面之外，于是每一张截图里它都不存在——不是透明、不是没画，是**在框外**。同一根因也让"放到 (300,200) 的窗口每次都出现在 (464,310)"看起来像窗口拒绝移动。修法：`SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`，必须在碰任何窗口/DC 之前 |
| **f.lux 常驻** | 用户主动开的暖色滤镜（2300K）。经 DWM/WCS 合成级作用于**整屏**，所以任何走屏幕的截图都会把白底拍成 `#FFA250`。参照的 `Theme.content_bg` 是 `#FFFFFF`，两者差得远 |
| **图标字体** | 参照 `components/common.slint:5` 有 `import "../../../assets/font/icons.ttf";`。那个 ttf 是**微软 Segoe Fluent Icons v1.47 的 74 字形子集**（由参照的 `assets/font/subset_font.js` 从它自己的 `.slint` 用到的码位切出来），`name` 表 family = `Segoe Fluent Icons`，与 `theme.slint` 写死的 `Theme.icon_font` 同名。**已照搬**（含 ttf 文件本身）。旁边的 462 KB `segoeicons.ttf` 参照**没有任何文件 import**，是死资源，**不搬** |
| **但子集仍然不生效（渲染器限制）** | 上面那行 import 在本机上**不生效**：`\u{E769}`（停止）仍显示为豆腐块，而 `\u{E768}`（播放）正常。逐项量过，结论是**这不是移植缺陷，是 `renderer-skia` 的限制**：<br>① 子集的 `cmap` 里 U+E769→glyph 9、U+E768→glyph 8，**字形都在**；② `name` 表 family 名对得上；③ 用 GDI `PrivateFontCollection` 加载**成功**并报出该 family；④ 在 `app.slint` 里再加一次 import 无变化；⑤ 把 ttf 写出来用 `AddFontResourceExW(FR_PRIVATE)` 注册——**调用成功**（日志有 `icon font registered bytes=24560`）——字形**仍是豆腐块**。<br>第 ⑤ 条是关键：Slint 的 Skia 后端经 **DirectWrite** 解析字体族，DirectWrite 的系统字体集合独立于 GDI 字体表构建，所以**内嵌字体和进程私有注册的字体都到不了它**。参照**没有任何运行期注册**（全 `src` 里没有 `AddFontResource`/`fontdb`），`Cargo.toml` 的 renderer feature 与我们完全相同 ⇒ **参照在同一台机器上表现一致**，这是渲染器属性，不是两个程序的差异。<br>**能落地的动作只有一个：装 Segoe Fluent Icons 字体**（Windows 11 的可选功能）。装上后两个程序都画出图标，没装都画方框。<br>→ 曾为此写过 `AddFontResourceExW` 的运行期注册并**删掉了**：它成功了却什么都没改变，交付一份证明自己无效的代码比不交付更糟 |

**截图工具链的两条结论**（`tools/standin-adb/capture.ps1` 重写）：

1. **取图方式在 P3 反过来了，见 §10。** 当时的结论是"用 `PrintWindow` 不用
   `CopyFromScreen`"，理由成立——`CopyFromScreen` 确实会返回*窗口移动之前*的帧
   （三张拍成别的应用，首页拍成自己的首帧，而模型说它已经加载完了），
   还会被置顶窗口和 f.lux 污染。**P3 换成 `no-frame` 之后，`PrintWindow` 变成静默
   不完整**：标题栏、侧栏、搜索框都在，轮询之后才出现的卡片全没了——dxcam 拍同一窗口
   卡片齐全，所以不是应用的问题。现在回到屏幕取图，但这次**先 `SetProcessDpiAwarenessContext`**：
   之前那次"返回旧帧"的怪相里，相当一部分其实是 DPI 坐标错位（窗口被要求移到 (300,200)
   实际落在 (464,310)），修掉 DPI 之后命中测试和像素才对得上，归属校验也就可信了。
2. **窗口用枚举 + 尺寸下限选，不用 `Process.MainWindowHandle`。** 本二进制的单实例握手
   窗口会让后者在多次运行中返回那个 158×26 的隐藏 shim，于是拍的是 shim。这条没变。

---

## 10. P3 外壳阶段的逐组件差异

### 10.1 新增的都是参照里的组件，一个不多

| 新增文件 | 对照参照 | 说明 |
|---|---|---|
| `components/sidebar.slint` | 210 行 → 照搬 | 九条目 + `show_*` 开关 + 54/200 折叠 |
| `components/title_bar.slint` | 257 行 → 照搬 | 无边框窗口的替代标题栏 |
| `components/common.slint` 补齐 | 参照 318 行 | P2 只搬了 `ActionIcon`，P3 补 `SidebarItem`/`ThemedIcon`/`WindowControlButton` |
| `assets/font/icons.ttf` `assets/logo/mini.png` | 同 hash | 参照自带的图标字体子集与标题栏左上角的小图标 |
| `handlers/common.rs` `handlers/window.rs` | 同名保留（spec §3） | |
| `app/window.rs` | 参照 441 行的**一个切片** | 只有 `find_main_window` + `set_always_on_top`；其余（`set_skip_taskbar`/`hide_window_completely`/`set_window_opacity*`/`show_and_center`/`activate_window_by_hwnd`/`rename_app_windows`）全属托盘与单实例，P7 一并取回 |
| `ui/tabs.rs` | 参照没有 | **判断**，不是组件。见 10.3 |

### 10.2 九个页签只做了一个：用参照自己的 `show_*` 开关

参照 `sidebar.slint` 每个条目都带 `show_add`/`show_usb`/`show_network`/`show_scheduler`/
`show_mount`/`show_donate`/`show_about`，入参来自 `app.slint` 的 `in-out property <bool>
sidebar_*: true`，由设置页 `save_sidebar_state()` 持久化。P3 **不发明任何新机制**：把还没做的
八个页对应的 `show_*` 置 `false`，每做完一页翻成 `true`。属性名、类型、默认值全与参照一致，
所以 P4–P7 只是"翻一个已有的开关"。

**注意 Settings 没有开关**：参照里设置项不带 `if (root.show_*)` 守卫（它九页齐备，
没什么可关的），所以 P3 侧栏**是会显示 Settings 的**，而设置页要到 P6 才有。

### 10.3 于是需要 `ui/tabs.rs`：`select_tab` 的守卫

点一个"条目存在、页不存在"的入口，会把 `selected_tab` 设到没有 `opacity` 绑定匹配的索引上，
参照的页签切换靠 `x: selected_tab == N ? 0px : 10000px` 把非当前页推出屏幕——**推走之后
屏幕上什么都没有**，内容区空白，正是 spec §5 禁止的现象。

所以 `handlers/common.rs::on_select_tab` 先问 `has_a_view(tab)` 再切。这个**判断**
最初写在 `handlers/common.rs` 里，紧接着意识到那是个错误：`handlers` 属于 GUI 树
（`mod ui;`），`cargo test --bin wsadash` 根本不编译它，**三个测试一次都不会跑，
却在 diff 里和真能跑的测试长得一模一样**。项目已在 `data.rs` 和 `handlers/home.rs`
记过这条规则，这次又踩到同一处。挪到 `src/ui/tabs.rs` 并用 `#[path]` 挂进 CLI 后
测试数从 190 变 195。

### 10.4 一处明知的偏离：标题栏的邮件图标

参照 `title_bar.slint` 的 `mail_icon_always` 默认 `true`，标题栏左侧有一个信封，
点开是参照自己的系统消息面板（`mail_messages`/`show_mail_popup`，wslui.com 用来公告）。
WSA 侧**永远不会有**这个数据源，所以 `app.slint` 传 `mail_icon_always: false`：
属性、回调、分支全在组件里一字未动，缺的只是一个点了没反应的按钮。

这是 P3 全阶段唯一一处"参照有、我们没有"，与 P9 的硬裁定方向相反（P9 禁的是**多出**参照
没有的东西），记在这里供对照时认领。

### 10.5 拖动窗口：计划写错了，按参照做

P3 计划的 Review Focus #6 写着"必须走 `start_drag()`，不能自己 `set_position`"。
**那是错的**。参照 `handlers/window.rs:51-59` 就是 `app.window().set_position(pos + delta)`，
而且必须如此：`start_drag()` 会把鼠标事件交给操作系统吞掉，标题栏 `TouchArea` 的
`moved` 此后不再触发。参照的 `TitleBar` 报的是**相对按下点的位移**，窗口跟着光标走会让
这个位移每次回到常量附近，累加式地 `pos + delta` 因此不会发散。照抄参照的写法。

### 10.6 顺带补上 P2 计划里漏掉的 `home_clicked`

侧栏的 Home 条目同时发 `tab_selected(0)` 和 `home_clicked`。参照把后者注册在
`handlers/distro/manage.rs`，语义是**手动刷新**（`should_refresh_wsl("manual trigger")`，
绕过 4 秒闸门）。P2 计划列了它但没实现——`data.rs` 里 `MANUAL_TRIGGER` 常量和测试都在，
`refresh_home` 却写死 `PERIODIC_TRIGGER`。这次把入口拆成
`refresh_home_with(reason, ...)`，`refresh_home` 成为周期触发的包装。

### 10.7 i18n 新增 9 键（键名照参照 `en.toml`，文案按 spec §3 换）

`sidebar.{home,add,usb,network,scheduler,mount,settings,about,donate}`。
`add` 参照是 "Add an instance"（加个 WSL 实例）→ 我们是 **Install APK**；
`usb` 参照 "USB devices" → 我们是 **Devices**（adb 目标）；
`mount` 参照 "Mount disk" → 我们是 **Storage**（spec §3 语义重定义）。其余同参照。
`common.app_name` P2 已有，值为 "WSA Dashboard"。

### 10.8 截图脚本这一阶段的三处改动

1. **`PrintWindow` → 屏幕取图**（§9 已记原因）。换回屏幕取图要重新解决遮挡，
   于是加了 `Raise()`（每次取图前 `HWND_TOPMOST` + `BringWindowToTop` + `SetForegroundWindow`）
   和 `Get-ForeignPoint()`（四角 + 中心五点问 `WindowFromPoint`）。P2 曾因只查中心而连拍四张
   压着 QQ 的图——**只有 DPI 修好之后，命中测试才和像素在同一个坐标系里，这条校验才可信**。
2. **等待从倒计时改成等应用自己的信号**：脚本设 `RUST_LOG=wsadashboard=debug` 给子进程，
   轮询 `~/.wsadashboard/logs/` 新增的 `home model applied` 行。改之前连拍五张空图——
   替身首轮轮询落在约 6 秒，而固定睡 5 秒，**页面自己的多样性判据又分不出来**，
   因为光是外壳（标题栏 + 侧栏 + 搜索框）就已经超过 8 种颜色。
3. **新增 `-Act collapse-sidebar | dark-theme`**：折叠和主题都是 `in-out` 属性、
   要到 P6/P8 才落盘，没有文件可预设，只能像用户那样点一下。坐标从布局推导
   （`Sidebar` 汉堡 4+13+10px 宽、标题栏四按钮 46px 右对齐、开关 40px）再乘显示器倍率。
   深色态还要把 `-ExpectedBackground` 一起改（`Theme.content_bg` 亮 #FFFFFF 暗 #1E1E1E，
   而主面积其实是侧栏 `#2D2D30`）——首次报错信息把"主题变了"误判成 f.lux 染色，
   文案已分开这两种可能。

### 10.9 验证

`cargo test --release --bin wsadash` → **195 passed / 0 failed / 3 ignored**；
`-- --ignored --nocapture` 三项真机全过；GUI `cargo build --release --bin wsadashboard`
零 error 零 warning。

截图（均 1200×900，可并排）：

| 文件 | 内容 |
|---|---|
| `docs/screenshots/p2-ready.png` | P2 四态之一，**在 P3 外壳下重拍** |
| `docs/screenshots/p2-booting.png` | 同上，加载覆盖层 |
| `docs/screenshots/p2-offline.png` | 同上，骨架行 + toast |
| `docs/screenshots/p2-stopped.png` | 同上，`DistroNotFoundView` |
| `docs/screenshots/p3-sidebar-collapsed.png` | 侧栏折叠到 54px（图中 81px），只剩图标 |
| `docs/screenshots/p3-theme-dark.png` | 深色主题，全外壳生效 |
| `docs/screenshots/p3-real.png` | `-UseRealAdb`，真 adb 与真 WSA |
| `docs/screenshots/p2-real.png` | 上一条的 P2 外壳版本，保留作对照 |

**P2 的四张仍在 `p2-*` 名下**：它们记的是 spec §5 的四个渲染态，P3 只是换了外壳，
改名会让 P2 计划里的引用断掉。重拍日期见 git。

---

## 11. P4 安装 APK 页与设备页

### 11.1 不是移植，是重写

P3 是纯移植，P4 不是。读过源码后确认两个 view 都不是照抄的对象：

| 参照 | 它真正做的是 | spec §3 的映射 |
|---|---|---|
| `views/add_view.slint`（520 行） | 从发行版 / 镜像 / rootfs **创建一个 WSL 实例** | → 安装 APK：位置与交互照它，字段全换 |
| `views/usb_view.slint`（258 行） | **管 usbip**：`usbipd` 的 bind / unbind / attach | → 设备页：adb 目标 + 连接/断开 + 属性表 |

所以"1:1"落在**结构层**——页面坐在哪、表单怎么叠、卡片怎么展开、进度怎么显示，
以及 `form_widgets.slint`（P2 已整份照抄，这页直接用）。字段与命令按 spec。

### 11.2 刻意没搬的（P9 逐页对照时看这里，不是漏做）

| 没搬 | 理由 |
|---|---|
| `usb_card.slint` 的 15 个字段 | 一个 adb 目标只有 `serial` 与 `state`；第三样（属性）是**问设备**要的。新写 `device_card.slint`，保留参照的徽标 / 药丸 / 单开单合交互 |
| `usb_settings_dialog`（317 行） | 把物理盘绑进 WSL 的 usbipd 操作。spec §3 对网络页已明说参照的 netsh 段"不适用、保留结构但隐藏"，这页同理 |
| `usb_device_info_dialog`（210 行） | spec §3 要的是"设备属性表"，没说它必须是弹窗。放页内 ⇒ 顺带把 P5 才要建的 `modal_manager`（790 行，只为仲裁对话框而存在）再往后推 |
| `distro/install_logic.rs`（713 行） | 下载校验和、解压 rootfs、配镜像、逐行写 `terminal_output`。`adb install -r` 是一条命令一次往返，P1 已按 §4.1.1 的"exit code 与 stdout 两者都要"实现 |
| 源选择器 / `[add.sources]` 四项 / 镜像整块 | APK 只有一个来源"一个文件"；一个选项的下拉是用不起来的控件 |
| `instance_name` / `install_dir` / `name_error` | 没有被创建的东西，也没有目录；一个字段只有一条错误，参照的 `path_error` 就是那条 |
| `bind_clicked` / `unbind_clicked` | adb 没有 bind 这一步 |
| `usbipd_version` / `is_usbipd_outdated` | 没有 usbipd 可报版本。"工具缺失"对应 `device_error == "not_found"`，**不带下载按钮**——参照的按钮指向 `AppConstants.usbipd_releases`，WSA 侧没有能安全写死的地址 |

### 11.3 改了签名的、换了名字的

- **`ConnectAdb { port: u16 }` → `{ target: String }`**。名字按 Q12 不动，改的是参数装多少：
  原来 `argv()` 里焊死 `127.0.0.1:`，只有首页那条路能用；设备页要连的是用户选的目标
  （Wi-Fi 的 `192.168.1.5:5555`、USB 序列号）。**没有加第二个 `adb connect` 变体**——
  那会让同一条命令在"唯一书写点"里出现两种拼法。
- **回调名**按 Q12 保留参照拼写：`attach_clicked` / `detach_clicked` /
  `info_clicked(DeviceTarget)` / `select_install_file(int)`。
  **`install_distro` 没有保留**，它的四参数签名是选择器的形状（源序号、名字、路径…），
  和"装一个 apk"是两件事；设备页的 attach/detach 是同一件事换个叫法，这里不是。
- **i18n 键** `usb.*` / `add.*` 全按参照原键名，值按 spec §3 换
  （`usb.attach` = "Connect"、`add.create` = "Install APK"）。
  没有参照对应物的新键四个：`usb.subsystem`、`add.apk_file`、`add.installed`、`add.file_missing`。
- **`adb devices` 只有一处解析**：`parse_targets` 同时供首页 `find_entry` 与设备页
  `adb_targets` 使用，并有一个专门钉住"两者对每个状态结论一致"的测试（Review Focus 2）。
- **`is_session_state(state)`** 收成一处判断，`find_entry` 与设备页共用，
  免得 `state == "device"` 这个字面量散在两个文件里。

### 11.4 计划里没预见的三处

1. **layout 会吞掉父级高度。** `HorizontalLayout` 作为 `Rectangle` 的直接子元素时拿到的是
   **父级**高度而不是自己声明的 `height`：头行于是拉满整卡、内容居中到中点，正好压在属性表上。
   属性区从一开始就对，因为它本来就在一个显式 `height` 的 `Rectangle` 里——头行照同样方式包一层即可。
2. **`assets/feat/microsoft-shield.svg` 得在 P4 就拷。** P2 从没编译过 `CustomButton`
   （那页只用了 `CustomLineEdit`），所以它 `show_shield` 块里的 `@image-url` 一直没被检查过。
   P3 的计划写的是"盾牌块随第一个会设它的组件（P5）回来"——**时间点说早了**。
3. **`VerticalLayout` 的余量分配。** 没有自身高度的子项会吞掉页面剩余像素并居中：
   标签贴顶、输入框低了 230px、中间那段空隙不归任何人。每行给显式高度（照参照的
   `Text { height: 18px }` / 控件 32px）之后，余量落到页面底部，那里既看不见，
   又正好是结果块要长进去的地方。

### 11.5 侧栏坐标在多了一行之后会变

`capture.ps1` 的点击坐标**从布局推导**而非照截图抄，所以 P4 打开 `sidebar_add`
插入一行时，`go-device-page` 的 `y` 必须从 154 变成 194。抄旧值不会报错，只会安静地点到
下一条上——第一次就是这么把安装页截成了设备页。`tabs.rs` 的测试同时钉住
`TABS_WITH_A_VIEW` 与 `is_sidebar_entry` 两边一致（Review Focus 4）。

### 11.6 截图脚本在 P4 期间的四处改动

1. **`-Act` 改成逗号分隔的序列。** 有些状态要连点两下才到（页藏在侧栏里，属性表藏在
   chevron 里），一个动作的取景拍不到两者。也因为 `powershell -File` **跨进程不解析
   PowerShell 的逗号数组**：父 shell 里 `a,b` 到子进程是**一个**字符串，`ValidateSet`
   拿整串去比对必然失败。去掉 `ValidateSet`，改成脚本内 `Split(',')`，
   `switch` 的 `default` 负责报出可用值。
2. **点击前等 250ms。** `Raise()` 之后前台切换没完成时，第一个点击被 Windows 消费去做激活。
   症状是日志里**完全看不到** `tab selected`——除非有人去读日志。
3. **点完把光标挪出窗口。** 停在按钮上会把该按钮的 tooltip 连同窗口边缘的截断一起拍进图，
   读起来像是应用没有的布局 bug（参照的 tooltip 定位一样，同样会截）。
4. **`-BypassDisplayFilter` 回来**（P2 有、P3 重写时删掉、P4 又需要）。f.lux 经 DWM 对
   **整个桌面**做暖色合成，凡读屏幕的取图都会被染色。脚本检出色偏后暂停它、`finally` 里恢复，
   AUMID 从运行中的进程路径反推（`F.luxSoftwareLLC.f.lux_4.137.0.0_x86__sw1dyjdkns7gt`
   → 包名 + 发布者）。这里踩过一个 PowerShell 陷阱：**`"$name_$publisher"` 被解析成变量
   `${name_}`**，推出 `sw1dyjdkns7gt!f.lux`——看着像成功、实际永远恢复不了；
   写成 `${name}_${publisher}` 才对。

### 11.7 验证

`cargo test --release --bin wsadash` → **202 passed / 0 failed / 3 ignored**；
GUI `cargo build --release --bin wsadashboard` 零 error 零 warning。

| 文件 | 内容 |
|---|---|
| `docs/screenshots/p4-install.png` | 安装 APK 页 |
| `docs/screenshots/p4-device.png` | 设备页：一个目标 + 徽标 + 断开 + chevron |
| `docs/screenshots/p4-device-properties.png` | 同上展开，7 行 `getprop`，值与真机实测一致 |

（截图尺寸随机器当时的显示缩放走：150% 时 1200×900，125% 时 1000×750，
脚本每次都现测倍率，所以四张之间**不必**同尺寸——P2 那一批之所以要求同尺寸，
是为了并排比对，这里比的是每一页的内容。）

### 11.8 真机回环：卸载一半的替代验证，与一个环境变化

spec §10 的 P4 验收是"安装/卸载回环可用"，用户选定按**方案 2** 落地。

**卸载：在替身 adb 上验证完成。**
`ops::tests::install_then_uninstall_round_trip_through_the_stand_in` 走完整的
"装 → 列表多一个 → 卸 → 列表回到原样"，203 passed。

它不是"断言收到了命令"那种假验证：原先替身的 `:op` 对 install 和 uninstall
用**同一份** fixture 作答，`list_apps` 两侧看到的永远一样，回环无物可观察。
为此改了替身——`:apply` 在 exit 0 时把 `install.package` 这个 fixture 写进
`packages.txt`、按行删掉卸载的包，且 `install -r` 对已存在的包只留一行
（真机 `pm list packages` 就是这么表现的）。没装机的环节，就在替身上把状态机建出来。

过程中踩到两个 cmd 的坑，都写在 `BATCH` 的注释里：
- **括号块里的 `%VAR%` 在整块解析时展开**，那时 `NEWPKG` 还是空的，于是
  `findstr "package:"` 匹配到"任意含 package: 的行"，**静默什么都不加**——
  一个接受命令、报告成功、却什么都没改的假 adb，比报告失败的更坏。
  改成无括号的 `goto` 结构。
- **替身 `.cmd` 是纯 LF**：`goto <label>` 在 LF 文件里找得到标签，`call <label>` 找不到，
  报"系统找不到指定的批处理标签"后继续走。六年没人踩到只是因为此前没有 `call`。
  写文件时改成**双向归一化成 CRLF**。

**真机卸载没跑**：`WsaCommand::UninstallApk` 是 spec §2.3 原样的
`adb uninstall <package>`，**没有 `-k`**，会清掉所卸应用的数据；
而磁盘上没有多余的 APK——实测 `$env:USERPROFILE\Downloads`、`<dev>`、
`AppData\Local\Packages` 三个树里 `.apk` 命中 **0 个**，
所以无法"先装一个自己创建的包再卸掉"。用户选定此方案，未做，如实记下。

**真机安装那一半：本会话早些时候确实跑通过**（`wsadash probe`/`start`/`list`、
设备页与属性表截图都取自真 adb），但现在**无法复验**——机器状态在这次会话期间变了：

```
Get-AppxPackage *WindowsSubsystemForAndroid*
  IsDevelopmentMode : True          ← 之前是普通安装
  InstallLocation   : (空)          ← 直接 Register 清单的开发模式注册
  PackageUserInformation : {}       ← 没有关联用户
WindowsApps\…\AppxManifest.xml      → 不存在
58526 端口无监听 / 无 WsaClient / 无 vmmemWSA / `adb devices` 空
```

`resolve()` 只按 spec §1.1 的 `Get-AppxPackage … .InstallLocation` 取路径，拿到空值就判
"未安装"——**按规范它答得没错**（这台机器上 WSA 确实起不来），但开发模式注册的清单本来
就不在 `WindowsApps` 下，spec 没覆盖这种注册方式。

**这一条要特别注意，别读成绿色**：此时
`cargo test --release --bin wsadash -- --ignored` 的输出是 **2 passed / 1 failed**，
而那 2 个"passed"是**跳过**——`probe_against_the_live_subsystem` 在 `resolve()` 失败时
`eprintln!("skipping…"); return;`，`running_apps_sees_a_real_running_app` 同理。
跳过和通过长得一模一样。唯一真正失败的是 `resolves_the_live_install`。
修好注册之前，任何"真机通过"的陈述都不成立。

---

## 12. P5a：ModalManager 子集 + WSA 存储页（磁盘页）

spec §10 把"网络页 + 磁盘页"放进同一个 P5。两者是两套独立数据源、两套独立数据面，
各自能独立产出可运行、可验收的软件，所以按 writing-plans 的 scope check 拆成
**P5a（本节）** 与 P5b（网络页）。先做 P5a 还有个实际理由：磁盘页的核心读本地
文件，**现在就能真机验收**，而 WSA 停止正是 spec §11 第 348 行要的验收条件。

### 12.1 事实更正：目录是 `LocalCache`，不是 `LocalState`

spec §2 第 226 行写 `VhdxInfo` 读 `LocalState\*.vhdx`。**该目录不存在**。真值：

```
%LOCALAPPDATA%\Packages\MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe\LocalCache\
    userdata.2.vhdx    2,995,781,632 bytes   2026-10-13 23:12
    metadata.2.vhdx      75,497,472 bytes   2026-10-13 19:51
```

与 spec 第 111 行 `LocalPortMin`/`LocalPortMax` 那条事实更正同一类。

**文件名带版本号**（`.2`）。按前缀 + 精确 `.vhdx` 后缀匹配，不硬编码全名——
否则 WSA 自己升一次版本号这页就空了；也**不是**子串匹配，`userdata.vhdx.bak`
这类备份副本不算数（一个回答"还剩多少空间"的页面把备份算进去就是在撒谎）。

### 12.2 真机验收（spec §11 第 348 行）

此刻这台机器 WSA **不可用**（见 §11.8），所以"关闭子系统后磁盘页可用"不是模拟出来的：

```
wsadash probe   → exit 4  "WSA is not installed"
wsadash vhdx    → exit 0  两行，字节数与文件系统完全一致
```

`docs/screenshots/p5a-disk.png` 就是这个状态下拍的（`probe` 与 `vhdx` 同一时刻）。
`docs/screenshots/p5a-disk-empty.png` 把 `LocalCache` 临时改名后拍的空状态，
拍完在同一次 PowerShell 调用的 `finally` 里改名回去并核对过（2 个 vhdx 都在）。
改名只是元数据操作，不碰数据。

### 12.3 语义重定义：`MountDiskCard` 三个回调只渲染一个

spec §3 第 241 行把这一页从"把物理盘/VHD 挂进 WSL"改为"WSA 存储管理"。卡片结构与
交互照搬，只换数据源——但**交互照搬在这三个按钮上做不到**：

| 参照 | WSA 侧 | 决定 |
|---|---|---|
| `mount_clicked`（挂载） | 没有挂载这回事 | 保留，改为 `explorer /select,`，文案"打开目录" |
| `unmount_clicked`（卸载） | WSA 没有 unmount API | **不渲染** |
| `delete_clicked`（删除记录） | 删 `userdata.vhdx` = 抹掉整个子系统 | **不渲染** |

签名按 Q12 全留。**这是 P5a 唯一的故意交互分歧**，P9 逐页对照时会看到。
不渲染的理由是本项目一贯的那条：渲染出来的控件必须做点诚实的事，一个渲染出来
却什么都不做的控件比没有更糟。

底栏的 primary 按钮在参照里是打开"选盘"对话框；这里没有盘可选，改为打开
`LocalCache` 目录本身，与卡片共用一个 `open_folder(string)`。

### 12.4 一个测试抓到的设计错误：不能用文件系统判断意图

底栏要"打开目录"、卡片要"选中文件"，第一版写成 `explorer_arg_for(path)` 靠
`path.is_dir()` 区分。测试立刻红了——**因为这台机器上 `LocalCache` 根本不存在**
（WSA 从没跑过），目录被当成文件、加上 `/select,`、Explorer 报错。恰好是在这一页
唯一要服务的状态上失效。改为 `explorer_arg_for_dir` / `explorer_arg_for_file`
两个函数，由调用方说明意图，文件系统不参与。

### 12.5 `Information` 结构体：名字保留，字段换成 WSA 语义

参照的 `Information` 有八个 WSL **发行版**字段（`distro_name` / `wsl_version` /
`install_location` / `vhdx_path` / `vhdx_size` / `actual_used` / `ip` /
`is_sparse`）。结构体名按 Q12 保留，字段换成 `AppInfo` 的五个：包名、版本名、
版本号、APK 大小、是否系统应用。

三个参照字段在 Android 应用上永远是空——空占位不是中立，是这个框答不了的问题。
参照唯一的 `status` 行（发行版是否 Running）换成"来源：系统应用 / 用户安装"，
因为后者决定"卸载"这个动作该不该被提供。用 `bool` 而不是字符串：参照拿字面量
`"Running"` 比较，差一个字母就会静默显示停用那一份。

### 12.6 `ModalManager` 只搬两块

计划里把它写成"60 份重复的遮罩"，读源码后发现**不是**：它是纯调度器，声明每个
对话框一个 `show_*`，按 100%/100% 实例化，遮罩由各对话框自己带。所以既没有值得
抽成 `DialogHost` 的东西（P9 硬裁定：参照没有的组件＝缺陷），加对话框也很便宜。

只搬 `show_delete` 与 `show_information`。其余二十个对话框的显示数据分别属于 P6
的设置/语言、P7 的计划任务、P8 的更新/导出，现在搬就得先造那些页面的数据模型，
真页面落地时再全部推倒。各阶段按参照的名字往里加，文件朝参照那份长齐。

### 12.7 两个截图当场抓出来的缺陷

1. **卡片上没有大小。** 右栏照搬参照只有一行 `mount_time`，而 spec §3 第 241 行
   要求这一页显示"大小、路径、占用"。参照可以不画——它的大小在信息框里，这一页
   没有。改成大小在上、时间在下。
2. **空状态长路径横向溢出，把图标顶到窗口外。** 参照那句是 "No mounted disks"
   （11 字符，必然放得下），我们那句以路径结尾（这台机器 90 字符）；`Text` 没有
   宽度约束会把纵向布局撑到比窗口还宽。补 `wrap: word-wrap` + `width: flick.width - 120px`。
   用 `flick.width` 而不是 `parent.width`：Flickable 容器里 `parent.width` 就是
   `viewport-width`，而它由本布局的 min-width 算出，Slint 直接报绑定环。

### 12.8 顺带修正的四处失实注释

- `tabs.rs`：三段互相矛盾的旧阶段注释叠在一起，表格里的 P4 行还停在 `0, 2`。
- `distro_card.slint` / `handlers/home.rs`：都说展开条那三个按钮"直到 P3 接上"，
  而 P3 交付的是外壳，一个都没接。实际在 P5a 才接上。
- `i18n/mod.rs` 的 `operation.deleting`：**原地**改文案而不是新增一条——`lookup`
  是 `find`，第一条命中，重复键会静默沿用旧文案。
- 计划里 Task 5 写的 `Vec<StorageImageUI>`：`StorageImageUI` 由 Slint 生成、只存在
  于 GUI 二进制，纯模块里点名它会让 `#[path]` 挂载失败、测试一次都跑不了。
  改为 `StorageRow`，和 `home_model::AppRow` → `data::row` → `Distro` 同形。
---

## 13. WSA 恢复（2026-10-14）与一处被自己推翻的判断

### 13.1 起因：注册还在，文件没了

某一阶段 `resolve()` 开始报 `NotInstalled`（exit 4）。查下来是：

```
Get-AppxPackage *WindowsSubsystemForAndroid*
  Status              : Ok
  IsDevelopmentMode   : True
  InstallLocation     : （空）
HKCU\...\AppModel\Repository\Packages\<PFN>\PackageRootFolder
  = C:\Users\reader\Downloads\Compressed\WSA_..._3\WSA_2407.40000.4.0_x64   → 目录不存在
```

包**注册还在**，指向的目录被删了。同批被删的还有 `Microsoft.UI.Xaml.2.8`
与两个 `Microsoft.VCLibs`。

### 13.2 我先写错了一句，已改

第一版 `resolve()` 的注释说：*"开发模式注册的 `InstallLocation` 本来就是空的"*，
并把 `PackageRootFolder` 回退说成是修复本机问题的原因。**这是错的**，实测推翻了它：

| 状态 | `InstallLocation` | `PackageRootFolder` | 两者相同 |
|---|---|---|---|
| 文件夹已删 | 空 | 指向已删路径 | — |
| 文件夹还原后 | 正常有值 | 同一路径 | 是 |

即：**空值是缺目录的连带结果，不是开发模式注册的属性**——这个包恰恰是开发模式注册，
目录回来后它照样报 `InstallLocation`。而 `PackageRootFolder` 与 `InstallLocation`
**指向同一个路径**，所以回退救不了被删的文件夹，真正修好它的是把文件放回去。

回退仍然保留（两个答案来自不同子系统，不必永远一致；多一次注册表读的代价，
落在本来就为解析而启动的那个 PowerShell 里），但注释已改成只陈述实测到的内容。

### 13.3 下载：GitHub 与 SourceForge 都被按主机限速

| 源 | 速度 |
|---|---|
| GitHub release-assets 直连 | 17 KB/s（ETA 12.5 h） |
| SourceForge 七个镜像（master / netix / phoenixnap / netcologne / deac-riga / versaweb / main） | 7–15 KB/s |
| Cloudflare speedtest（同机对照） | **2,035 KB/s** |

机器带宽没问题，是被服务方按主机限速。GitHub 代理 `gh-proxy.com` 跑出
1.2–8 MB/s，最终 **740.0 MiB / 775,994,382 bytes** 下载完整，7z 魔数
`37 7A BC AF 27 1C` 正确，bsdtar 3.8.8（libarchive）可直接读，无需装 7-Zip。

**以后 >100 MB 的下载先问用户或走 IDM**（已写入团队记忆）。

### 13.4 恢复步骤与两个拦路错误

包名从残留的 `PackageRootFolder` 反查，逐字对上：
`WSA_2407.40000.4.0_x64_Release-Nightly-with-magisk-30.6.30600.-stable-GApps-13.0.7z`，
MustardChef/WSABuilds 的 `Windows_11_2407.40000.4.0_LTS_7_HOTFIX_1`。

1. 解压到**旧注册的同一路径**（313 文件 / 2.1 GB / 30 s），一个注册表键都没改。
   此时 `InstallLocation` 立刻恢复，`resolve()` 从 exit 4 变成 exit 3 `state=Stopped`。
2. 但 `wsa://` 仍打不开：`HKCU\Classes\wsa\shell\open\command` 为**空**。
3. 直接重注册 → `0x80073CFB`（已注册版本高于要注册的版本）。
4. `Install.ps1` 第 173–174 行的回退分支就是答案：
   `Remove-AppxPackage -PreserveApplicationData` 后再
   `Add-AppxPackage -ForceUpdateFromAnyVersion -Register`。
   **这一步需要管理员权限**，非提权会得到 `0x80073D28`。

### 13.5 备份：唯一拷不到的那个文件

`Remove-AppxPackage` 之前先把整个包数据目录 `robocopy /E` 备份到
`<dev>\apps\_wsa-pkgdata-backup-20261014`（138 文件 / 3,090,681,424 字节）。

**137 个拷成功，唯一失败的是 `Settings\settings.dat`（32,768 字节）**，报错
"正被另一进程使用"。这不是拷贝不稳，是**已注册包的 `settings.dat` 被当作注册表
配置单元挂载，任何进程都读不了它**——它在这个状态下必然锁死。

而丢掉的正是坏掉的那份：该文件里装的是包的 `URLAssociations`
（`wsa://` 的关联就在这个键下，13.4 第 2 步实测为空），重新注册会从
`AppxManifest.xml` 重建。用户的应用与 2.9 GB 镜像逐字节一致。

### 13.6 恢复结果

```
wsadash probe → state=Ready  adb_port=58526  boot_completed=true
wsadash start → Booting → Ready，exit 0，用时 17s
wsadash list  → com.topjohnwu.magisk 30.6  Magisk
                com.nowcent.ham 1.7.6       Ham
                com.microsoft.launcher 6.260402.0.1178950  Launcher
cargo test --release --bin wsadash              → 222 passed / 0 failed / 3 ignored
cargo test --release --bin wsadash -- --ignored → 3 passed / 0 failed
                                                 live probe: state=Ready port=58526 boot_completed=true
```

**17 秒与 spec §1.2 当初实测的冷启动时间一致。** 用户的三个应用一个没丢。

`-- --ignored` 那三条此前一直是"跳过冒充通过"（§11.8），这次是真通过——
`resolves_the_live_install` 不再返回 `NotInstalled`，
`probe_against_the_live_subsystem` 打出了实测值而不是 `skipping`。

截图：`docs/screenshots/p5a-real-home.png`（真 adb、真包）与
`p5a-real-restored.png`（真机存储页，VHDX mtime 已随本次启动更新）。
---

## 14. P5b：网络页（端口转发 + HTTP 代理）

### 14.1 与参照的语义差异

参照 `src/ui/handlers/network/` 是四个文件（`mod` / `rules` / `proxy` / `service` / `utils`），
绝大部分在驱动 `netsh interface portproxy` 和一个替它跑提权工作的 Windows 服务。逐条：

| 参照 | 我们 | 依据 |
|---|---|---|
| `netsh interface portproxy add/delete/show all` | `adb forward` / `adb reverse` / `--list` | spec §3 第 239 行 |
| `netsh advfirewall firewall`（新增/删除规则） | 保留 netsh 这一条 | 同上（防火墙那半照搬） |
| 服务 `WslDashboardService` 提权跑 netsh | 不需要：adb 命令不提权 | — |
| 给 WSL shell 导 `HTTP_PROXY` 环境变量（`manage.rs:95`） | `settings put/delete global` | Android 没有 shell 环境变量这回事 |
| 发行版列表 + 解析出的 IP（`distro_names` / `local_ips`） | 属性保留但没人读：`adb forward` 不需要地址 | — |

**暂存模型整个没有对应物。** 参照是"先改、后应用"：规则先落盘，卡片显示未应用（`is_active == false`）
并给一个绿色播放键，"Apply all rules" 一次性推全组。它必须这样，是因为 `netsh` 要提权，不能随手改。
`adb forward` 是一条普通命令，加进去那一刻已经在服务端的表里。

于是：`NetworkRuleCard` 保留三个回调签名但只渲染删除（同 P5a 的 `MountDiskCard`，同一个取舍：
一个什么都不做的按钮比没有按钮更糟）；`process_network_rule` / `cancel_network_rule` /
`apply_all_network_rules` / `cancel_all_network_rules` / `initialize_task_clicked` 声明了但没有
handler，Slint 里那就是 no-op。**`is_helper_installed` 传 `true`**，让参照那个"Auto-Start
Forwarding"按钮（它装的是 netsh 需要的提权 helper）不渲染；P7 的计划任务会真正接手它——开机后
重建转发，因为 adb 自己不会。

### 14.2 三个代理 global 互不推导（推翻 spec §1 第 116 行）

spec §1 第 116 行写：`settings put global http_proxy <host:port>` 之后
`global_http_proxy_host` / `_port` 会一致地读回来。**不成立。** 本机实测两个方向：

```
只写 http_proxy 10.0.2.2:8081      → host=[:0]  port=[:0]  combined=[10.0.2.2:8081]
只写 host=10.0.2.2 + port=8080      → host=[10.0.2.2] port=[8080] combined=[:0]
```

AOSP 源码解释了原因：`ProxyTracker.loadGlobalProxy()` 读的是 `GLOBAL_HTTP_PROXY_HOST` /
`_PORT` / `_EXCLUSION_LIST` 三件套，而 `HTTP_PROXY`（即 `http_proxy`）是**另一列**、被
`loadDeprecatedGlobalHttpProxy()` **优先**读取。第一版实现写的 `http_proxy_host` /
`http_proxy_port` / `http_proxy` 全在错的一列上——平台一个都不读。它能通过任何"写进去读回来"
的测试，因为**两列都能 round-trip**；只有把替身按两列分开建模才分得清"写对了 key"和"写了个 key"。

### 14.3 设置时必须 delete `http_proxy`，不能写它

`loadDeprecatedGlobalHttpProxy()` 命中非空 `http_proxy` 就
`setGlobalProxy(ProxyInfo(host, port, Collections.emptyList()))`——**空排除列表**——新版本还直接
`return true` 短路，三件套根本不读。所以设代理时要**删掉**它；清除时才写 `:0`：
`":0".split(':')` = `["", "0"]`，得到 `ProxyInfo("", 0, "")`，host 为空正好落进
`setGlobalProxy` 的 else 分支把 `mGlobalProxy` 置 null。

`http_proxy` **没有默认值**（`settings delete` 后读回 `null` 而不是 `:0`）——此前看到的 `:0`
是我们自己先前写进去的。

### 14.4 `settings put` 拒绝空串，清除只能 delete

```
settings put global global_http_proxy_exclusion_list ''
→ Bad arguments   (exit 1)
```

**没有任何一种写等于"没有值"**，所以清空排除列表、清除代理都必须走 `settings delete`。
替身 adb 现在也拒绝空 put——否则这条回归会在这儿过、在设备上炸。

### 14.5 端口范围 1..=65535，参照文案写 65534

取自 AOSP `ProxySelector.validate`（`portVal <= 0 || portVal > 0xFFFF`）与 `ProxyInfo.isValid()`。
参照的 `network.proxy_error_port` 写 65534（它那条 netsh 的范围）。键名照留（Q12），**值改成与本页
真正校验的代码一致**；`http_proxy_value` 因此拒绝 `0`、`65536` 和非数字，且在写之前拒绝——部分写入比
不写更糟。

### 14.6 认证字段没有平台对应物

`ProxyTracker.setGlobalProxy` 只写 host / port / exclusion / pac 四项；`Settings.Global` 公开 API
里没有用户名口令项；AOSP 里没有任何结构能存全局代理凭据。参照配的是 WSL shell（有 `HTTP_PROXY`
环境变量），Android 没有等价物。

所以 `proxy_auth` / `proxy_user` / `proxy_pass` 三个键**保留在页面上**（P9 要求组件对齐），保存时
代码里显式 `let _ = (auth, username, password)` 而不是不绑定——不绑定读起来像漏了。宁可
"保存时明确丢弃"，也不给系统一个永远不读的 key，让页面看起来配好了而其实没有。

### 14.7 adb forward/reverse 的真实形状（spec §1 第 117 行已确认）

```
adb forward tcp:18081 tcp:8081  → 打印 18081（不是 "Success"），exit 0
adb forward --list              → 127.0.0.1:58526 tcp:18081 tcp:8081
adb reverse tcp:18080 tcp:18080 → 打印 18080
adb reverse --list              → host-16 tcp:18080 tcp:18080
```

成功时**静默**（只打印端口号），所以不能复用给 install/uninstall 的 `require_success`——那个函数要
求 stdout 里有一行字面量 `Success`，而 `install` 恰恰会在 exit 0 的同时打印 `Failure [...]`，
那正是它存在的理由。给 forward/reverse 另写了 `silent_success`：exit code 说了算。
**`serial` 列按不透明字符串处理**：`forward` 放 `host:port`、`reverse` 放字面量 `host-N`，
一个按 `forward` 格式写的解析器会把每条 reverse 规则都丢掉，页面显示空白而规则其实存在。

### 14.8 规则 id 编码成 `<方向>:<端口>`

`delete_rule` 只收一个字符串（参照的签名），而删一条规则需要方向和端口两件事。参照的 id 是一个
netsh key，本来就装着同样的两个事实。我们的 `network_model::parse_id` 对 `""`、`"forward"`、
`"forward:"`、`"forward:abc"`、`"forward:0"`、`"forward:70000"`、以及参照自己的 `"18081-18081"`
一律返回 `None`——**不能退化成 `(Forward, 0)`**：`adb forward --remove tcp:0` 是一条什么都不指向
的命令，而 id 是从窗口里来的，可能是陈旧行。

### 14.9 截图环境：两件会让人把错页当成证据的事

**窗口在点击之前会被 Windows 移动。** 四次实测起点分别是 (113,114)、(37,38)、(265,266)、
(151,152)，每次都不一样；而 `capture.ps1` 原本在步骤开头读一次窗口位置就一直用。偏移正好够点到
隔壁那一行——一次本该点 Network 的点击选中了 **Storage**，画面看起来就是导航成功了。现在每次点击
前重读，并在位置变化时打印。

**这个桌面上有永远压在最上面的窗口。** `PrintWindow` 让窗口自己画自己，谁在上面都影响不了图；
但输入是发给最上面那个窗口的。按下去之前用 `WindowFromPoint` 问一句那个像素归谁：两个点当时都报
`'reader | OpenChamber'`。所以脚本现在会改用 `PostMessage` 把鼠标事件直接投进我们自己的队列。

**代理页那张图是用临时办法拍的。** 在上述环境下注入点击整体不可靠（同一个刚成功的步骤，下一步就
三连失败），页签切换没法稳定复现。于是把 `NetworkView` 的 `active_tab` 临时设成 1、重编、拍完复原、
重编——**验证的是那个视图本身**（真正有风险的部分：布局、占位文案、认证块的禁用态），而页签切换
的代码是参照逐字搬来的（`components.slint` 与参照 diff 除 SPDX 外完全一致）。
### 14.10 Task 5：两个对话框接上，第三个明确不接

| 对话框 | 状态 | 理由 |
|---|---|---|
| `NetworkAddRuleDialog` | 接上 | 添加规则那个按钮一直活着，没有它页面就不完整 |
| `ProxyHelpDialog` | 接上 | 纯文字，无数据依赖 |
| `CheckProxyDialog` | **不接** | 它要真的通过代理发一次请求并回报 HTTP 状态，需要一个 HTTP 客户端；这个 build 没有，P8 的更新检查会带来。挂上去只会得到一个"点了就关掉、什么也不说"的按钮。它的页面入口（Check Connection）在代理被配置之前本来就是禁用的，所以这不是一个会静默失效的控件。 |

**对话框的两个下拉框保留，各只有一项。** 参照的 `distro_names` / `local_ips` 是"转发到哪个发行版
的哪个 IP"；`adb forward` 不需要地址，所以 `network_distro_names = ["WSA"]`、
`network_local_ips = ["127.0.0.1"]`，`ip` 在使用处丢弃（和认证字段同一个处理）。空的下拉框是
一个没法用的控件，填一个不是选择的项则是明说这不是选择。

**防火墙勾选框目前没有效果，且不是"以后再说"。** `netsh advfirewall firewall add|delete rule`
要提权，参照解决这件事的办法是装一个 Windows 服务（它的 `service.rs`），这里没有对应物。
P7 拿到提权机制之后才谈得上。在那之前徽章读 "FW: Off"，`network_model` 里
`enable_firewall` 恒为 false——这是事实，不是占位。

**端口校验和代理页同一条规则**（1..=65535 之外拒绝），拒绝发生在 `forward_add` 之前：adb 接受
端口 0 并且什么都不转发，那会是一条页面上显示不出、也删不掉的规则。
---

## 15. P6：设置页、关于页，以及真正的语言包（2026-10-15）

### 15.1 这一阶段推翻了 §3 的一条

§3 写的是「键名一个都不改，值里改领域词，50 个语言各改一遍」。P6 实际做出来的是另一件事，而且
比那件更对：**我们自己的英文是权威，其余 49 个包是叠加层，叠加不得覆盖我们的措辞。**

理由是 §3 自己列的词表暴露出来的矛盾。`operation.deleting` 在参照里是 "Deleting ..."，我们改成
"Uninstalling ..."（卸载 APK）；`sidebar.mount` 是 "Mount"，我们写 "Storage"；`network.
proxy_error_port` 写 65534，而我们代码校验的是 65535（AOSP 的 1..=65535 才是真范围，参照那个
是它自己 `netsh` 的范围）。这 54 个键的句子是**照着参照的句子翻译的**——`ja.toml` 里
`operation.deleting` 那句 Japanese 描述的是删一个 WSL 发行版。让它盖上来，得到的不是一份翻译过
的面板，而是一份用另一种语言、彬彬有礼地描述另一个软件的说明书。

所以：

- `assets/i18n/en.toml` **由 `ENGLISH` 表生成**（`tools/install-language-packs.py`），一条测试
  断言两者一致，手改文件会被抓到。
- `OURS` 列出 54 个措辞是我们的键，`lookup` 拒绝对它们做覆盖。
- **`OURS` 不靠人手判断**：`ours_list_is_exactly_the_keys_whose_wording_changed` 拿参照自己的
  `en.toml` 和我们的表逐键比对，两个方向都要一致。多一个键是错（白白放弃一个能翻译的键），少一个
  键更糟（我们自己的句子被盖成别的东西）。`tools/apply-ours-keys.py` 从两个文件重新生成它。

顺带作废 §3.1 那张词表：它描述的是批量改 50 个语言包的做法，我们没有批量改任何一个包。

### 15.2 `normalize_language_code`：参照 160 行 if 链 → 匹配已发布的包名

参照用一百六十行的 `if lower == "..."` 链，一个包一行。那件事只有一个含义：**这个区域设置对应我们
发布了哪个包**——而发布的列表已经在二进制里了。所以链换成匹配器：精确 → 前面 → 两条真正的别名
（`fil` ← `tl`，`no` ← `nb`/`nn`）→ 两个中文包（Windows 给 `zh-CN` / `zh-TW`，而 `zh`、
`zh-Hans`、`zh-HK`、`zh-MO` 也都可达，`zh` 单独出现时按简体解，和参照一致）。一条测试对全部 50 个
包名逐个断言「能选中自己」。

### 15.3 设置页：五处语义改写，没有一个是死控件

参照的设置页有六行描述这个程序没有的东西。每一行都改成一件**真的会发生**的事，而不是"存一个没人
读的字符串"——后者比少一个复选框更糟：勾上它、保存、关掉、再回来看到没变化。

| 键（参照的） | 参照的含义 | 这里 | 真活在哪 |
|---|---|---|---|
| `distro_location` | WSL 发行版安装目录 | WSA 数据目录 | 从包注册读出的 `LocalCache`，按钮打开它 |
| `backup_location` | 发行版导出/备份目录 | 程序自己的数据目录 | `~/.wsadashboard`，按钮打开它 |
| `sparse_vhd` | 新建镜像默认稀疏 | 优先显示已分配大小 | `disk_model` 读 VHDX footer 的两个尺寸 |
| `mount_auto_probe` | 启动发行版以读挂载表 | 窗口打开时刷新存储 | 我们的磁盘页不需要子系统跑着 |
| `select_folder` 按钮 | `rfd` 弹选择器 | 打开它自己写的那行目录 | `open::that`，磁盘页已有同款 |
| `stop_wsl` 按钮 | 停止 WSL（要二次确认） | 停止子系统（不二次确认） | 确认已经在首页那个 Stop 按钮上了 |

`select_*_folder` 三个回调没有用 `rfd`：那两行是**事实**不是选择，没有东西可选，弹一个只有唯一
答案的选择框是白弹。标签也从 "Select Folder" 改成 "Open Folder"。

`logs_location` **故意不回写**：那一行显示的是日志器真正写的目录，接受编辑就等于允许页面声称一个
日志器根本不看的目录。

### 15.4 配置：三个字段去掉、两个加回来、一个段落照搬

`UserSettings` 去掉五个字段：实例目录、备份目录、临时目录、temp（我们不装东西，没有目录可选），
以及 mount 的"自动启动发行版以探测挂载表"（磁盘页按 spec §11 第 348 行要求在子系统停着时也能用，
所以它根本不探测）。

**`sparse-vhd` 和 `mount-auto-probe` 在 Task 2 被我当 WSL 专属删掉了，Task 3 给它们找到真活之后又
加回来**，键名照参照。留着这两个字段而不改名，是 15.3 那张表的另一半。

`[system]` 段（`system-language` / `timezone`）照搬参照加进来，系统语言用
`GetUserDefaultLocaleName` 读（为此给 `windows` crate 加了 `Win32_Globalization`）。`get_font_for`
需要它——非中文 Windows 上必须显式给 CJK 字体，否则 Skia 画不出中文。

两个字段留了但没人读：`show_drag`（参照的发行版列表有拖拽手柄 ours 是固定九项，顺序有意义，默认
**false** 而不是照搬 true，存一个 true 是在承诺一个不存在的控件）和 `auto_shutdown`（P7 回答"退出时
停不停 WSA"）。

读永远不失败：文件不存在、解析不了、是新版本写的键，三种情况答案一样——带默认值打开并在日志里说
一句。参照的 `load` 也是这么做的。

### 15.5 侧栏开关：配置默认值不能直接灌进窗口

`SidebarConfig::default()` 八个开关全 `true`（参照九个页面都做了）。P6 在启动时读配置直接灌进窗口
之后，Donate / Task scheduler / About 三个入口冒了出来，后面什么都没有——`tabs::has_a_view` 存在的
全部意义就是防这个。

改成 `offered(wanted, tab) = wanted && has_a_view(tab)`：**文件说读者要什么，`has_a_view` 说这个
build 有什么，两个都对上才显示。** 附带的好处是阶段落地时文件不用改，之前就写着的 `true` 会自己
开始生效。

### 15.6 语言选择器：P6 唯一一处签名改动

参照的 `selected(int)` 发的是**索引**，调用方再去 `LanguageData.lang-codes` 里查表。那张表是硬编码
在 `language_data.slint` 里的 51 项，而我们的列表来自 `available_languages()`（二进制里的文件列表），
所以改成 `selected(string)` 直接发代码。

**这不是理论上的改动。** 第一版我只改了文档没改代码，Slint 把 int 强转成 string，
`normalize_language_code("5")` 回答 `en`，点一下看起来什么都没发生——而且 `t()` 退回英文让好几条
断言**看起来像**通过了一半。另外 `callback selected(string)` 是显式写出来的：Slint 的类型推断会拿
第一个调用点定类型，同一个错误会在三百行之外再冒一次，报成 "Cannot convert string to int"。

### 15.7 关于页：五分之四的 handler 不能存在

参照的 `handlers/about.rs` 五分之四是一次 fetch：向 `api1.wslui.com` 要文档地址、讨论区地址，再按
读者的语言和时区去列表里匹配一张群组图片。spec Q6 禁止调那个服务，所以这些一个都不存在。

| 参照 | 这里 |
|---|---|
| `group_clicked` 弹出群图片 | 什么都不做——正是参照自己在图片没就绪时的分支（`about.rs:15-22`） |
| `check_update_clicked` 查 GitHub | 一句「这个 build 里没有更新检查」，P8 换掉这句话 |
| `open_url` 打开服务器刚给的地址 | 打开配置了的地址，没有就说没有 |

`open_url` 的守卫放在 `handlers/home.rs` 而不是 about 里：窗口上只有**一个** `open_url`，主页的卡片
也走它。`openable_url` 拒空串和没见过的 scheme——`open::that("")` 不是错误，是一条没有参数的 shell
命令，点下去看起来就像坏了。`file:` 在白名单里，因为磁盘页就是靠它打开资源管理器的。

`PROJECT_REPOSITORY` 现在是空的，所以"Project website"和"Project repository"两行后面没有地址，
点它们会得到那句 toast。P3 记过这件事："宁可没有链接，也不要错的链接"。

### 15.8 实测推翻的两条

1. **语言包的加载顺序不是"英文打底 + 目标语言无条件覆盖"。** 见 15.1。
2. **`SystemConfig.system_language` 不是 Windows API 的实时查询**，是配置文件里的一个字段（参照
   `settings.rs:42` 读的是 `get_config().system.system_language`）。我们照搬了这个形状：本阶段读一次
   写进文件。理由和参照一样——`get_font_for` 的答案不该因为用户在会话中途翻了 Windows 设置就
   在运行中的窗口底下变掉。

### 15.9 截图工具链：这一阶段侧栏坐标变了四次

P6 里 `go-settings-page` / `go-about-page` 的坐标改了四次，其中三次的原因是**某一行出现或消失**，
不是布局变了：

| 值 | 为什么 |
|---|---|
| 540 | 从一张 Donate 还亮着的截图上数的行 |
| 488 | 猜「隐藏 Donate 少 40px」 |
| 526 / 566 | 真的：底部那组贴底边对齐，加一行会把**上面的行往上推**，不是把下面的往下挪 |

这一条值得写下来，因为它和这个工具链其余部分的注释是反的：`go-*` 的坐标一直是**从布局算出来
再验**的，而底部那一组算不出来——它同时取决于上面的行数和窗口高度，只有图知道。P7 加 scheduler
那一行时两个数字都要再动一次。

另外这一轮里 `capture.ps1` 报告的缩放在 125% 和 150% 之间来回跳（输出 1000x750 和 1200x900 两种），
`settings-language-de` 的点击坐标因此失效——它留在步骤表里但标了 UNVERIFIED，并从 `$setsState`
里拿掉，因为一个不能安全重复的步骤不该待在以可重复为唯一目的的运行里。

### 15.10 P6 验证到什么程度

- 三个页签各一张截图，标签和路径都是真值（`docs/screenshots/p6-settings-*.png`、`p6-about.png`）。
- `settings.toml` 落盘并回读正确，`[system] system-language = "zh-CN"` 是从 OS 读出来的。
- 语言对话框打开、列出 50 个包、当前语言高亮——**选了 `de` 之后整个窗口是否重算，没验成**（见
  15.9）。`i18n` 的加载、`OURS` 保护、缺键退回英文有 14 条测试；`AppI18n.version` 自增那一段是
  照参照 `settings.rs:52` 抄的三行。
- 276 passed / 0 failed / 3 ignored；GUI 零 error 零 warning。

## 16. P7：计划任务、托盘、单实例（2026-10-15）

本阶段到 Task 3 为止的记录；Task 4/5（计划任务页视图层与接线）与 Task 6 的验收勾选在本节末尾的
16.9 里标为未做。

### 16.1 `schtasks` 的输出是本地化的，固定列号在非英文 Windows 上静默出错

参照的计划任务读取用 `schtasks /Query /FO CSV /V`，然后**按列号**取字段（第 2 列是任务名、第 3 列是
下次运行时间……）。那张表的表头跟着系统界面语言走：

| 界面语言 | 表头 | 列数 |
|---|---|---|
| 英文 | `HostName,TaskName,Next Run Time,Status,...` | 29 |
| 中文（本机实测） | `主机名,任务名,下次运行时间,状态,...` | 33 |

按列号解析**不会报错**，只会静默拿错字段：任务名变成状态字符串、日期变成 `__/__/____`。换到
`/XML` 之后 `<Task>`、`<StartBoundary>`、`<RunLevel>`、`<Enabled>` 的拼写在所有 Windows 上一致——
那些是 task schema 的元素名，不是界面标签——日期也是 ISO 8601，不是区域格式。

三个只有跑到真实输出才看得见的问题：

1. **`schtasks /Query /XML` 的输出不是合法的 XML 文档。** 本机 219 个任务：一个 `<Tasks>` 外壳，
   219 个 `<?xml ?>` 声明，其中 **218 个嵌在外壳里面**。`roxmltree` 整份拒收，直接解析会在任何任务
   多的机器上返回 0 个。解析前先剥掉内部声明；`<Description>` 里出现 `<?xml` 的文本不碰。
2. **`<Exec>` 在 `<Actions>` 下，不在 `<Task>` 下。** 夹具写错时 219 个任务全部取不到命令；修完是
   **102 个 `<Exec>` + 119 个 `<ComHandler>`**。后 119 个是 Windows 自己的维护任务，**本来就没有
   命令行**。把这个区别做成 `ActionKind`：119 行空命令列是在展示自己的失败，不是在展示系统的样子。
3. **`*/4` 在星期字段从 0（周日）起步**，落在 0 和 4，加上折叠的 7，得 SUN/THU——不是直觉上的
   MON/FRI。参照的 cron 校验里 `1,5` 和 `1-5/2` 都被拒，所以"两个指定星期"在这个解析器里**根本
   无法表达**；这一点页面必须说出来，而不是悄悄建一个在错误两天跑的任务。

另外参照的文件夹过滤是 `starts_with("\WSA-Dashboard")`，而 `\WSA-Dashboard-Other\X` 也以它开头——
那会把别人的任务列成自己的、还能删掉。改成按文件夹边界比较。

`RawSchtasksRecord`（参照的 CSV 行中间类型）被删掉：它全部字段都是**位置**的，这正是问题本身。
`TaskXml` 取代它，`models.rs` 记了原因。

### 16.2 Run 键的值名是 `APP_ID`，不是参照的 `WSLDashboard`

参照把开机自启写成 `HKCU\...\Run\WSLDashboard`。`Run` 键是全机共享的命名空间，一个叫
`WSLDashboard` 的值会指向**这个** exe。后果是双向的：装过参照的人会在任务管理器的启动项里看到一个
没装的程序；而一个删除 `WSLDashboard` 的卸载器会漏掉我们的自启项。

值名改成 `constants::APP_ID`（`wsadashboard`），并且有一条测试断言它**不等于** `WSLDashboard`。
Q12 说标识符保留参照的名字；这个不是标识符，是参照的**产品名**，照搬恰好产生上面那个碰撞。

顺带：`delete_autostart` 用 `KEY_WOW64_64KEY` 打开键，参照没带这个标志——64 位构建写 64 位视图
才是任务管理器启动项里看得到的那个，写 32 位视图会造出一个自己都读不回来的项。

### 16.3 单实例按进程 id 找窗口，不是三个标题

参照用三个窗口**标题**（自己的 caption、一个内部常量、一个短形式）去猜正在跑的那个实例的窗口，
找不到就是一次没有解释的"双击没反应"。我们按**进程 id** 枚举顶层窗口（`find_main_window`），
所以那份标题表在我们的文件里不存在——`window_titles_for_lookup()` 返回空，并有一条测试断言它
保持为空。那条测试是唯一的守卫：谁把标题表加回来，参照那个静默失败就回来了。

`mutex` + 唤醒管道两套机制都保留（参照也是两套）：托盘里的窗口句柄可能已经不在 shell 的活动列表
里，而管道由进程自己应答。管道名不带版本号（参照的带 `_v0.3`），因为不存在一个更老的**本程序**
构建会来唤醒我们。

### 16.4 `ver` 的解析取括号里的版本形状 token，不是英文 `"Version "`

参照用字面量 `"Version "` 切分 `wsl --version` 的输出。本机实测 `cmd /c ver` 回：

```
Microsoft Windows [版本 10.0.26300.9457]
```

中文 Windows 上那句切分什么都找不到，然后**静默退成 `N/A`**——而 `N/A` 同时也是一个真正答不上来
的问题的样子，所以没有东西提示这里是坏的。不随语言变的是括号里的数字（Windows 自己格式化的），
所以改成：取最后一个方括号里的、版本形状的 token。有一条 `#[ignore]` 的实机测试跑真实 `ver` 并断言
解析结果不是 `N/A`（Q8 把依赖机器的活留在套件外）。

### 16.5 `show_and_center` 不在 `app/window.rs`

参照的 `show_and_center(&AppWindow, silent)` 是一个 120 行的函数：silent 分支把窗口停到 -32000、
minimized、`is_window_visible = false`；非 silent 分支起一条后台线程，opacity 0 起步、找句柄、
改扩展样式、按显示器居中、`rename_app_windows`、显示、恢复 opacity、恢复置顶。它做的其实是三件
从三个不同地方调用的事。

拆成 `park_in_tray`（启动时，`app.run` 之前）、`hide_to_tray`（关窗进托盘）、`show_and_center`
（从托盘恢复），放在 `ui/handlers/window.rs`，和其余点名 `AppWindow` 的回调在一起。这样
`app/window.rs` 保持**不含 Slint 类型**，CLI 才能挂载它（见 16.7）。`set_skip_taskbar` 留在
`app/window.rs`：它只需要句柄。参照那个函数有一个从未使用的 `&AppWindow` 参数，这里没有。

`set_window_opacity*` 与 `rename_app_windows` 仍然不做：它们服务于"从 opacity 0 渐显"的揭示流程，
我们通过 `activate_window_by_hwnd` 一步揭示，没有东西可渐显、也没有 helper 窗口要改名。

`activate_window_by_hwnd` 里一处改动：参照把停到屏外的窗口居中到 `MONITOR_DEFAULTTOPRIMARY`，
我们居中到 `MONITOR_DEFAULTTONEAREST`——把读者放在第二块屏上的窗口"恢复"到主屏，是这类程序被
记成坏掉的方式。

### 16.6 托盘：菜单是数据，图标是内嵌的

菜单项的 id 在参照里写两遍（`MenuItem::with_id("show", ...)` 一遍、`match event.id { "show" => ... }`
一遍）。改一处就是一条渲染正确、翻译正确、点了什么都不发生的菜单项。这里两处都用 `tray_model`
的常量，`entries()` 是唯一的来源，并有一个测试断言 id 恰好是那两个、顺序是 show→exit、两个都
可点。另有一个测试断言 `label_key` 真的在 `ENGLISH` 表里——一个拼错的键是空的菜单标签，
`tray-icon` 把它渲染成一行空白而不是报错。

图标用 `include_bytes!("assets/logo/mini.png")` 编译进来、运行时解码，而不是参照的
`Icon::from_resource(1, None)`。参照能那样做是因为它的 `build.rs` 嵌了图标；我们的 `build.rs`
把图标块留给了 P8（文件里写着），所以资源查找在 P8 之前每台机器都会失败，而失败的结果是托盘里
什么都没有。P8 落地资源后可以换回参照的顺序，其余不动。

`tray.show_window` / `tray.exit` 的措辞是参照的原句，所以 49 个翻译包带着它们，非英文机器上菜单
是翻译过的——这也意味着它们**不**在 `OURS` 里（参照的托盘同样显示窗口和退出，没有我们的句子要保护）。

### 16.7 两个测试目标的现实：`window.rs` / `single_instance.rs` 现在真的在跑

这个项目的测试运行只有 `cargo test --bin wsadash`；GUI 二进制的测试目标没人跑。Task 2 之前，
`single_instance.rs` 的 4 条纯测试（mutex 名、管道名、标题表为空、两名字不同）写在 GUI 二进制里，
**从来没有执行过**；而且 `cargo test --bin wsadashboard` 根本编译不过（三处路径错，其中两处是
`super::super::` 用错了层级）。

`app/window.rs` 是这个项目里唯一不含 Slint 的 GUI 文件（从上到下都是 Win32 句柄），所以它和
`single_instance.rs` 一起挂进 CLI 测试区是可行的。两处结构配合这个挂载：`single_instance` 用
`super::window::`（GUI 里 `super` = `app`，CLI 里 `super` = crate 根，两处都对），
`single_instance` 的测试用 `super::super::constants::APP_ID`。两个文件加 `#![allow(dead_code)]`，
理由和 `scheduler_task/models.rs`、`config/mod.rs` 一样：挂载只让测试可见，句柄那半边在这里
是"构造上不可达"，不是死代码。现在这 5 条测试在跑，330→336。

### 16.8 计划任务页的语义：没有发行版，所以"命令"是 adb 命令

参照的计划是"在某个 WSL 发行版里跑一条命令"，对应物是"在 WSA 里跑一条 adb 命令"——我们没有发行版
维度。参照的 cron 校验、`short_filename`、错误文案在 Task 4/5/6 里逐条对齐，结论是**它们基本原样成立**，
因为两边约束的是同一件事（Windows 的计划任务），而不是各自的产品：

- `short_filename`（末段路径、超 32 字符截到 29 加 `...`）原样搬，只把**字节切片改成按字符**：参照的
  `&name[..29]` 在非 ASCII 名字上 panic，而本程序带五十种语言。ASCII 名字上两者完全相同。
- 名称规则（≤60 字符、`A-Za-z0-9-_.`）原样搬。这不是风格规则：名字进 `schtasks /TN`，一个反斜杠
  就是一层文件夹、一个冒号就是驱动器。
- cron 方言的那五条错误句子原样搬，并且新增一条 `the_prefixes_still_match_what_the_parser_says`：
  它把 `parse_cron` **真实产出**的消息喂给 `cron::error_key`，所以改文案会先在测试里失败，而不是先
  变成德语窗口里的一句英文。
- `scheduler.error_create_failed` 等的 `{0}` 占位符是**用**的（`i18n::tr`），不是参照那样
  `format!("{}: {}", t(key), e)` 拼在占位符后面——那会读出 "Failed to create task: {0}: 原因"。
- 参照硬编码成英文的两句 `Toggle failed:` / `Run failed:` 在这里是键（`error_toggle_failed` /
  `error_run_failed`），文件选择器的标题也是（`dialog_select_command`，与参照同字面值）。
- `cron_error_range` 被参照同时用于"越界"和"根本不是数字"两种失败，而句子写的是前者。我们保留同样的
  映射（`error_key` 的第五条分支），把句子改成两种都成立的说法，这条因此进了 `OURS`。

**`scheduler_error` 的第三个空态：不加。** 参照填了它、视图从不渲染它，于是"查询失败"和"没有任务"
在屏幕上是同一个画面（`app.slint` 那条属性的注释也这么记）。视图只有两个空态，加第三个就是**新增
参照没有的组件**，P9 明令不许。所以这里用现成的窗口级气泡：失败时同时填 `scheduler_error`（保持与
参照逐字段可对照）并弹一条带原因的提示，成功时把它清空。屏幕上于是分得清，而视图层一行没动。

**没照搬的死字段**：`trigger_summary` 参照声明了却从没填过，卡片也不渲染；`status`/`last_run`/
`next_run` 是 `N/A`，因为 XML 里根本没有任务状态（`<StartBoundary>` 是配置值，不是下次触发时刻）。

### 16.9 悬空项：网络页的"自动转发"辅助任务（P5b 留下的那个）

`network_is_helper_installed` 在 P5b 里被硬编码成 `true`，用来**隐藏**参照那个"Auto-Start
Forwarding"按钮，因为 `adb forward` 不需要参照那个提权 helper，于是没有东西可装。P5b 的注释说
"P7 的计划任务会给这个按钮一份真活：重启后重新加映射"。

**这一项没有实现，这是决定而不是遗漏。** 它需要的东西和计划任务页不共用一条代码路径：

1. GUI 二进制的一条 CLI 模式（Q25 的 `/scheduler`），它得先读网络规则、必要时**冷启动 WSA**、
   再逐条 `adb forward`/`reverse`。`main.rs` 目前完全没有参数解析，这半条也要一起写。
2. 一个 `\WSA-Dashboard\` 下固定名字的 ONLOGON + `/RL HIGHEST` 任务去跑它，以及
   `check_task_exists`。`/RL HIGHEST` 非提权时是 `ERROR: Access is denied.`（本轮实测），所以注册
   必须走提权批次，也就意味着**每次验证都要点一次 UAC**。
3. 网络页那两个回调（`check_network_task_status` / `initialize_task_clicked`）与把
   `network_is_helper_installed` 的默认值改回 `false`。

它自己的设计、测试和实机验证（登入触发器 + WSA 关机状态）是一整块工作，不是一行接线。**写一半比
记下来更糟**：一个注册了却不会冷启动的任务，会在每次登入时静默失败。所以它留在 §11 的第 10 条
上不勾，等下一轮。

### 16.10 实机往返抓到的两个缺陷（只有真机能看到）

`#[ignore]` 的 `the_real_task_store_takes_a_task_and_gives_it_back`（建→读→停→启→跑→删）第一次
跑就红了两处。两处都在**读写真实任务库**的路径上，任何单测都碰不到——单测读的是我们自己造的结构体。

**启用态读反了。** `schtasks /Query /XML` 对**启用**的任务根本不写 `<Enabled>`：`cmd /c` 原始字节
取过，`schtasks /Create` 造出来就是空 `<Settings>`，`Get-ScheduledTask` 说
`settings.Enabled=True, state=Ready`；`/Change /DISABLE` 才**加上** `<Enabled>false</Enabled>`，
`/ENABLE` 又去掉。而 `to_scheduler_task` 原先写 `unwrap_or(false)`，注释还说"什么都没写就等于关着，
Windows 也是这样"。**Windows 不是这样。** 后果：页面上每个启用任务画成关闭，卡片那个开关变成死键
（读到 false → 发 `!false=true` 的 `/ENABLE` → 本来就启用 → 什么都不变 → 永远显示关闭）。改成
`unwrap_or(true)`，三种状态各一条断言；原来那条把 bug 当规格的测试换掉了。

**cron 写不进任务描述（非提权路径）。** 描述由 `powershell -NoProfile -Command "<脚本>"` 写，而当时
外面套了一层 `Command::new("cmd.exe").args(["/c", 整条命令行])`：`Command` 按 `CommandLineToArgvW`
的规则给整条命令行加引号、把内部 `"` 转义成 `\"`，而 **cmd 不把 `\"` 当转义**——cmd 收到撕碎的命令行，
`powershell` 根本没跑，非零退出码被记成一条 `warn`。症状只是编辑对话框里日程是空的。现在
`powershell` 直接以参数向量收脚本。

**提权那条是好的，而且是量过的**：把 `build_elevated_shell` 的形状（单行、
`(A & powershell -NoProfile -Command "…") > "log" 2>&1`）原样写进 `.cmd` 跑了一遍，
`<Description>cron:0 3 * * *</Description>` 确实进了任务并能从 `/Query /XML` 读回来。也就是说 cmd 把
双引号里的 `|` 当文本——而 `description_script` **不含任何 `"`** 正是它的前提，所以这件事现在是测试
断言（`matches('"').count() == 2`、不含 `^&<>`），不是巧合：多一个引号，`$t | Set-ScheduledTask`
里的 `|` 就变成管道。

顺带量到的两条：`/Create … /RL HIGHEST` 非提权 = `ERROR: Access is denied.`（所以"以管理员身份运行"
只能由提权批次兑现，实机测试里 `admin_mode` 保持 false）；`Set-ScheduledTask` **不需要提权**（所以
描述往返是真量的）。

还有一条**不是缺陷但值得记**：`schtasks /Create /TR "C:\Program Files\WSA\adb.exe"` 不加内层引号时，
Windows 把命令存成 `C:\Program`、参数存成 `Files\WSA\adb.exe`——第一张截图里那张卡片因此显示
"Program"。页面如实呈现了任务库里的东西；这正是 `escape_tr_args` 存在的理由（它只在值含空格或引号时
才加引号——给不含空格的 `notepad.exe` 加引号会把引号写进命令里，实测确认）。

### 16.11 P7 验证到什么程度

- **Task 1**：`schtasks` 解析器在**本机真实的 219 个任务**上跑通（102 Exec + 119 ComHandler）。
- **Task 2**：本机真实 `ver` 输出解析成 `10.0.26300.9711`；单实例按进程 id 找窗口。
- **Task 3**：托盘三个开关生效（桌面检查，按 §8 的办法）。
- **Task 4**：视图层三个 slint 文件照搬，卡片只读六个字段；空态文案按 WSA 语义改写，命令占位符从
  参照的 `D:\linux\scripts\example.bat` 换成 `am start -n com.android.settings/.Settings`。
- **Task 5**：接线十个回调。图片三张：`docs/screenshots/p7-scheduler-list.png`（空态）、
  `p7-scheduler-tasks.png`（两条真实任务：一条启用=绿点、一条禁用=灰点+淡出）、
  `p7-scheduler-dialog.png`（对话框）。侧栏"Task scheduler"条目亮起、默认顺序在 Network 与 Storage
  之间——`tabs.rs` 加 4 之后 `settings.rs:237` 早就接好的 `offered(sidebar.scheduler, 4)` 才放行。
- **`is_helper_installed` 未做**：见 16.9。
- **沙箱限制**：本会话早前 `schtasks.exe` 被 `PROGRAM BLOCKED BY SECURITY POLICY` 拦过；现在不再受限，
  7 条实机测试全部跑过。
- **测试基线**：388 passed / 0 failed / 7 ignored，`-- --ignored` 7 passed / 0 failed；
  `cargo build --release --bin wsadashboard` 零 error 零 warning。

---

## 17. P9 验收状态（2026-10-17）

spec §11 的 11 条逐条回写在那份文件里，**每条带一个三分类状态和对应证据**；这里是汇总。

| 状态 | 含义 | 条数 | 条目 |
|---|---|---|---|
| **通过** | 条目要求的行为有可复现证据 | **10** | 1, 2, 3, 4, 6, 7, 8, 9, 10, 11 |
| **部分通过** | 一部分有证据，另一部分被跳过 | **1** | 5（替身通过；真机卸载跳过） |
| **未通过** | 条目要求的行为不存在（"没做"，不是"没测"） | **0** | — |

**跳过清单**（"能做但没做/没重做"，各自带原因，与上面的"未通过"不是一回事）：

| 跳过的东西 | 为什么 |
|---|---|
| 真机卸载回环（第 5 条后半） | `adb uninstall` 没有 `-k`，会清所卸应用的数据；本机磁盘上没有多余 APK，用户的三个应用都是有数据的，不允许拿它做实验。要闭合需要 Android SDK 造一个可弃的包，本仓库没有这条工具链 |
| 语言选择器的点选交互（与第 9 条相关但不同） | `capture.ps1` 的 `settings-language-de` 标着 `UNVERIFIED`：注入点击的落点随显示器缩放跳（125%/150%）。第 9 条问的是**键**，键的完整性由测试保证 |

第 10 条在 §19 里兑现（不建持久化，改成"任务自己把子系统叫起来"），并且**这一兑现顺带把两个跳过项
消掉了**：`/scheduler` 把子系统叫起来之后，热启动计时可以实测了，那条会 `skipping` 的实机测试也不再
跳过。原因是机器的状态变了——不是又等了几天。

**环境事实（已被上面那件事改掉）**：写上一版汇总时本机 WSA 卡在 Booting（`WsaClient` 连续运行 28.5
小时、端口 58526 拒绝连接）。**现在不是了**：`wsadash probe` 是 `state=Ready / boot_completed=true`，
三个应用都在，`running_apps_sees_a_real_running_app` 不再打印 `skipping`。把它修好的就是 §19 那个
开关——20 秒、退出码 0、全程没有窗口。

**测试基线（P8 + §19）**：`cargo test --release --bin wsadash` → **417 passed / 0 failed / 10 ignored**；
`-- --ignored` → **10 passed / 0 failed**，**没有任何 `skipping`**（`running_apps_sees_a_real_running_app`
现在真的在验东西）。其中做实事的几条：本机真实 `settings.toml` 的迁移（§18.2）、真的 `api.github.com`
回答 `rust-lang/rust` 的 `tag="1.99.1"`（§18.3）、真实任务库的建→读→停→启→跑→删往返、以及
`pressing_run_now_makes_the_command_run` 那条端到端（§19.4）。
`cargo build --release --bin wsadashboard` 零 error 零 warning，exe 带图标与版本资源（§18.5）。

---

## 19. 计划任务的自动化：不建持久化，让任务自己把子系统叫起来（2026-10-17）

用户裁定：**不追求规则持久化**，要的是"点一下就自动执行"。所以不做网络页那份规则存储，改做一件更小
也更直接的事：**每个任务的启动器先让子系统起来，再跑命令**。

### 19.1 每个 `/TR` 都是一条启动器

`tr_value` 以前只包脚本类型（为了不闪控制台），直接程序原样进 `/TR`。现在一律包，
因为**冷启动住在启动器里**，而一条 `adb` 命令需不需要它，与它是不是脚本无关。

启动器（`vbs_launcher_body`）现在有三块，每块都是承重的：

```
' wsadashboard:cmd=<读者写的命令>
' wsadashboard:args=<读者写的参数>
' Written by WSA Dashboard. ...
Set shell = CreateObject("WScript.Shell")
shell.Run """<本程序 exe>"" /scheduler", 0, True     ← 冷启动
shell.Run "<命令行>", 0, True                          ← 读者的命令
```

1. **两条标记**（VBS 注释）——`/TR` 存的是 `wscript.exe "<启动器>"`，读者写的东西在那里根本看不见，
   而解释器形态（`powershell.exe -File "..."`）也不是他写的。启动器就是任务对自己的记录，和
   `<Description>` 记日程是同一个道理：Windows 的任务库里没有别的地方放。**参数只活在这里**
   （连参照都没做到）。
2. **冷启动**——第一行跑本程序自己的 `/scheduler`（Q25 的开关，参照的辅助任务也是跑
   `wsldashboard.exe /scheduler`）：先探测，已经起来就立刻返回，否则启动并**等到就绪**（本机实测
   17s，spec §13.6）。**它的退出码被故意忽略**：不需要子系统的任务（备份脚本之类）在子系统起不来时
   也必须跑。
3. **隐藏并等待**——`wscript.exe` 是 GUI 子系统进程、`Run(..., 0, True)` 隐藏且等退出码。

**为什么不用 `wsadash start`**：那要求 `wsadash.exe` 装在 `wsadashboard.exe` 旁边，而 Q3 的交付是
**一个 exe**；靠第二个文件存在的启动器，会在有人只拷一个文件的那天悄悄不再启动子系统。所以
`src/app/immediate.rs` 实现了 `/scheduler`（在 `bootstrap` 之前、在窗口/托盘/日志/单实例握手之前——
那些是读者的事，而一个开窗口的任务看起来就是卡住了）。

### 19.2 实机往返抓到的第二个缺陷（P7 就带进来的）

**`schtasks` 会把 `/TR` 拆成 `<Command>` 和 `<Arguments>`**（在第一个空格处，除非程序被引号包住）。
所以 `wscript.exe "…launcher.vbs"` 存进去是：

```xml
<Command>wscript.exe</Command>
<Arguments>"C:\Users\reader\.wsadashboard\scripts\launcher\launcher-7a05….vbs"</Arguments>
```

而 `to_scheduler_task` 只看 `<Command>`——后果是**任何脚本类任务读回来都成了命令 `wscript.exe`**：
卡片第二行显示 `wscript.exe`，编辑对话框预填 `wscript.exe`。P7 之所以没发现，是因为真机上只建过
**直接程序**（`cmd.exe`），那种 `/TR` 恰好是 `cmd.exe /c …`，拆开之后两个字段恰好就是读者写的两半。
单测也看不见：它自己拼了未拆分的 `wscript.exe "<path>"` 整行。

修法：读回前把两半拼回去（`command_line(&task.command, &task.arguments)`）再判断/读启动器；并且补一条
**按 Windows 实际存储形状**构造的测试（两个字段 + 一个真实启动器文件），它才是能看见这件事的测试。

### 19.3 提权：只有任务自己需要时才要

以前每个动作都走提权批次（UAC 弹窗），因为参照那么写。实测（§16.10）`/RL HIGHEST` 非提权是
`ERROR: Access is denied.`，而**其余四个动作（/Create 不带 /RL、/Delete、/Change、/Run）非提权都能
跑**。所以创建/删除/启停/立即运行的提权判据改成"这个任务自己是不是以管理员运行"
（`params.admin_mode` / 任务读回来的 `admin_mode`）。**普通任务现在一次 UAC 都不弹**——这也是整条链
能被自动化验证的前提。

### 19.4 验证

- **单测 417 通过 / 0 失败 / 10 ignored**（+4：标记往返、换行不能结束标记、无标记的旧启动器、只认
  那一个开关字面量、按拆分形状读回）。
- **实机（端到端）**：`pressing_run_now_makes_the_command_run`——建任务 → `run_task` → 轮询直到命令
  **真的产生了可见结果**（`cmd.exe /c md "<dir>"` 建出一个目录），4.6s 内出现，然后清理。它同时断言
  `/TR` 里存的是启动器而不是裸命令（防止有人"顺手"把它改回去、连带把冷启动丢掉）。
- **实机（开关本身）**：`wsadashboard.exe /scheduler` 手工跑（本轮新 exe）——**全程没有窗口**
  （每 500ms 查一次 `MainWindowHandle`，10 秒内始终为 0），20 秒后**退出码 0**，随后
  `wsadash probe` 是 `state=Ready / boot_completed=true`。**它是在这台机器卡住的状态上做成这件事的**：
  跑之前是 `state=Booting`（`WsaClient` 连续运行 28.5 小时、端口 58526 拒绝连接、`getprop` 取不到），
  跑之后 Ready，三个应用都在。也就是说"计划任务把子系统冷启动起来"这件事不需要模拟就能证明——这台
  机器当时正好就是那个场景。
- **顺带被这次状态改变消掉的两个跳过项**：`wsadash launch com.microsoft.launcher` 实测 **2.31s**、
  再跑一次 **2.03s**（spec §11 第 6 条的热启动半边，上限 3s）；实机套件现在 **10 passed / 0 failed
  且没有任何 `skipping`**（`running_apps_sees_a_real_running_app` 真的在验东西了）。
- **一处如实记下的短板**：`/scheduler` 在 `bootstrap` 之前返回，所以它**不写日志**——一个任务的
  冷启动失败时，日志里没有任何痕迹，只能从子系统状态看出来。要补的话是在那条分支里先起一个最小
  的 logger，而不是把它挪到 `bootstrap` 之后（那会把窗口和托盘也带进来）。

**这一条改动让 spec §11 第 10 条成立**，并且是**上面那台机器恢复的直接原因**：不是"实现了一个功能"，
而是"这个功能的第一次真实使用就修好了它"。


---

## 18. P8：配置迁移、更新检查（2026-10-17）

### 18.1 迁移链的第一条：为什么是"版本戳 + 一条步"，而不是照抄十步

参照的 `migration.rs` 有十步，因为**它发布过十次**，每一步都在补一个后来加进去的字段。我们的链只有
一条，所以要说清的是"为什么有一条"而不是"为什么只有一条"。

**`#[serde(default)]` 回答的是"文件里没有这个字段"，不是"文件里有这个字段，而这个字段的意思变了"。**
第二种才会坏：键能解析、值是一个完全合法的 `bool`、唯一的问题是它当初回答的是另一个问题。默认值修不
了它，因为没有任何东西是缺的。

这个项目**已经出现过一次**：P6 里 `sparse-vhd` 和 `mount-auto-probe` 保留了参照的键名和默认值，而
它们要回答的问题换了（参照的 WSL 语义 → 这里的存储页开关，见 §15.4）。一个老到还带着旧语义的文件，
新代码没法不做决定就读它，而这个决定属于一条带版本守卫的链，不属于一条注释。

于是：`[application] setting-version`（键名、serde 重命名、类型都是参照的），`SETTINGS_VERSION = 1`，
**没有这个字段的文件读作 0**（参照的约定，也是所有现存文件的样子，包括这台机器上那份）。v0→v1 这一
步做的事就是上面那件事：把两个意思变过的键重置为本 build 的默认值，而不是相信它们。

**在真实存在的文件上它是空操作**——两个字段出厂默认本来就是 `false`——这一点写进代码注释而不是藏
起来：一个永远什么都不做的步，和一个恰好在今天的输入上什么都不做的步，在 diff 里长得一样，而只有
后者值得留。

两条规则各有测试钉住：

- **更新的文件不动。** 版本高于本 build 的是未来的 build 写的，它的键不是我们重新解释的对象；读它
  原样、`warn` 一句、不重写。
- **迁移永不写盘。** `load_from` 有一条承诺：读设置不会失败、不会让人意外。读的时候写文件会同时破坏
  这两条（第一条在只读盘上，第二条在任何时候）。版本号在**下一次因为改设置而保存**时被盖上——这也是
  参照的安排。

`load_from` 现在把四个出口收成一个：`read_config` 读，`migrate_config` 带前，四条路径（不存在、
读不了、解析不了、正常）走同一段后处理。

**副作用（正确地）打破了两条既有测试**，它们断言 `load_from(p) == Config::default()`。那正是"读会
迁移"的反面，所以改的是测试而不是代码：加了一个 `migrated_default()` 助手，并新增一条"读只加版本戳"
的断言。

`[application]` 段我们只留一个字段。参照那段有七个（名字、两个链接、版本号、设备 id、启动时间），
六个描述的是"一次安装"，而这个程序需要的那部分在 `constants.rs` 和日志里——写进文件就是同一个事实
有两个来源，而文件里那份是会过期的那份。

### 18.2 实机验证：读者的真实文件

`#[ignore]` 的 `the_real_settings_file_loads_and_migrates` 读**本机真实的** `settings.toml`（只读，
不写）。实测输出：

```
C:\Users\reader\.wsadashboard\settings.toml (615 bytes)
mentions setting-version: false
schema v0 -> v1, sparse-vhd=false, mount-auto-probe=false, ui-language=en
```

也就是说：这份文件确实是预 P8 的（没有版本戳），链在它上面真的跑了，读者的语言没有丢。这比"我自己
写一个 v0 文件再读它"强的地方在于——后者只能证明我写的那个文件。

### 18.3 更新检查：`src/api/` 是一次**缩减**，不是移植

参照的 `src/api/` 是 `api1.wslui.com` 的客户端：带每请求元数据头的请求构造器、响应信封、一小时磁盘
缓存、以及十个端点包装（关于页链接、下载页、群聊、cron 帮助、rootfs 帮助……）。**本 build 永不调用
那个主机**（spec Q6、MAPPING §5），所以那些全部不在。留下的是唯一不在那台服务器上的东西：GitHub
releases 的更新检查。

两个"错了也不报错"的判断被拆成纯函数并测试：

- `parse_release`：**缺 `tag_name` 是错误，不是空 release**。GitHub 对私有/改名仓库的 404 响应体是
  `{"message":"Not Found"}`——合法 JSON、有 `message`、没有 `tag_name`。当成空 tag 的 release，它就
  会走"不比当前新"的分支，于是页面说"已是最新"——而页面在**真的**是最新时说的也是这句话。
- `version_is_newer`：逐段比较，**不是字符串比较**（`"0.10.0" < "0.9.0"` 作为文本，而作为版本更大；
  弄反了就会向所有用 0.10 的读者推荐降级）。接受三种真实会遇到的写法：前导 `v`、段数不同（`1.0` 与
  `1.0.0` 相等）、以及遇到非数字段就停下（`0.2.0-beta` 比 `0.1.9` 新、不比 `0.2.0` 新——这正是读者期
  待的答案，也是唯一不需要实现 semver 预发布规则就能得到的答案）。读不出来的候选**永远不是更新**。

`latest_for(repository)` 收参数而不是自己读 `PROJECT_REPOSITORY`：这个文件编进两个二进制，而那个常量
在 GUI 里是 `crate::app::constants::`、在 CLI 里是 `crate::constants::`——收参数就不必选边。

接线在 `handlers/about.rs`：**不用 `slint::spawn_local`**（这是这棵树里其它 handler 的写法），因为
`ureq` 是阻塞客户端、请求允许花 15 秒，跑在 UI 线程的执行器上就是让窗口冻住网络的时长。用的是
`handlers/scheduler.rs` 那套（`std::thread::spawn` + `slint::invoke_from_event_loop`）。三种答案各一句
话：有新版（`about.new_version`，带 tag）、已是最新（新键 `about.up_to_date`）、失败（新键
`about.update_check_failed`，带原因）。成功时写 `check-time`——这个字段此前存在但没有任何东西写它。

`PROJECT_REPOSITORY` 为空（Q2：还没有远程）时**不开线程、不开 socket**，直接说"本 build 没有配置项目
仓库"——这句话与"请求失败"是两回事，而且为此加了一条测试：空白仓库在打开任何 socket 之前就被拒。

### 18.4 侧栏状态落盘：§16.5 那条 `persisted in P8` 的日志行有了下文

`on_save_sidebar_state` 原先只 `debug!` 一句。现在走 `config::update_user_settings`：**读-改-写**，
不是一个 handler 自己攒一份 `Config`——后者会在设置页下一次保存时，把其它每个值写回成"这个 handler
跑的时候"的样子。`modify-time` 一并盖上，理由和设置页盖它的理由一样：一个不动时间戳的写入会让文件
对自己的年龄撒谎。

### 18.5 打包：图标与版本资源进 exe，而**图标还是参照的**

参照 `build.rs:45-139` 的图标块照搬进来（六个尺寸 16/32/48/64/128/256 + `Lanczos3`——Windows 按 DPI
挑条目，只有 256 没有 16 的 exe 在任务栏是糊的、在 Alt-Tab 里是尖的），写出 `assets/logo/logo.ico`
和 `assets/logo/icon.rc`，再用 `embed-resource` 编进 exe。同时补上参照第 4 行
`#[path = "src/app/constants.rs"] mod constants;`——那块要读 `APP_NAME` / `APP_ID` /
`COMPANY_NAME` / `LEGAL_COPYRIGHT` / `PROJECT_REPOSITORY`，这正是那份文件从 P0 就在的原因。

`FileDescription` 从参照的 `"{} - Management Tool for WSL"` 改成
`"{} - Management Tool for Windows Subsystem for Android"`。

**两处如实记下的账：**

1. **图标是参照的 artwork，不是新画的。** spec Q6 说开发期复用参照 ico、P8 打包前换新；**没有新图标
   可换**，所以 `assets/logo/logo.png` 是参照那张 256×256 的副本。许可上干净（上游 GPL-3.0，这里
   GPL-3.0），而且一旦有东西可换，改的就是这一个文件。在换掉之前，这个 exe 在任务栏里和参照长得一样。
2. `assets/logo/logo.ico` 与 `icon.rc` 是**构建产物**（每次 build 从 png 重新生成），已进 `.gitignore`；
   `logo.png` 本身**入库**，它是源。

托盘图标不共用这张：`mini.png`（64×64）是 `include_bytes!` 进 `tray-icon` 的那张，见 §16.6。
---

## 20. UI 线程满载：已排除的与仍成立的（2026-10-17）

v0.1.0 发布后实测：窗口一出现，单个线程就吃掉 80–90% 的单核，机器像卡死。`tools/cpu-budget.ps1`
是为此加的门禁（单线程 CPU / 新增控制台 / 窗口是否应答）。**修好之前不发新包。**

**已测到的（数字都是 `cpu-budget.ps1` 同一口径：最热线程的 % 单核）：**

| 条件 | 结果 | 说明 |
|---|---|---|
| 真 adb，3 张卡 | 88.6% | 发病状态 |
| 替身 adb，0 张卡（页面正常就绪） | 31.4% | `tools/standin-adb/adb-empty.cmd`：就绪但没有包 |
| 替身 adb，4 张卡 | 83.7% | 同一条替身，只多四行 `package:` |
| 窗口隐藏到托盘 | 89.1% | 与可见时一样，所以不是"失焦后该停的刷新" |
| `WSADASH_ADB` 指向不存在的路径 | **0.3%** | adb 一跑不起来就归零，所以不在启动路径、也不在定时器 |
| 15 秒内 `adb.exe` 进程数 | **1** | 不是 spawn 风暴/重试环 |
| 热线程 = 窗口线程 | 是 | `GetWindowThreadProcessId` 与热线程 tid 相同 |
| `SLINT_BACKEND=software` | 88.5% | 与默认 93.8% 同量级 ⇒ **与渲染器无关** |
| `SLINT_BACKEND=femtovg` | 49.2% | 只有它减半，仍远高于预算 |

**结论：每帧都在重做布局**，成本随屏幕上的卡数（即文本数）增长，并且与渲染器无关。

**试过并推翻的：**

- `distro_card.slint` 的 `animate height`（396 行那处）。摘掉后 4 卡仍是 85.3%，与摘下前的 83.7%
  在噪声内 ⇒ 不是它。改动已撤回，`.slint` 与 HEAD 逐字节一致。
- 托盘那个 100ms 定时器（只 `try_recv` 两个队列，微秒级）与 `refresh.rs` 里的 `1ms`（只是测试夹具）。

**下一个假设（未验证）**：`Theme.icon_font` 指向的 `Segoe Fluent Icons` 在未安装该字体的机器上解析
失败（§9 记过：内嵌 ttf 到不了 DirectWrite）。**一个解析不到字族的 `Text` 可能每帧重新走一遍字体
回退**，这正好解释"成本 ∝ 屏幕上的文本数"与"与渲染器无关"。验证方法只需一次构建：把
`Theme.icon_font` 改成 `"Segoe MDL2 Assets"`（Windows 11 自带）或 `"Segoe UI"`，再看 4 卡条件是否
从 ~85% 掉下来。若成立，正确修法不是换字体名，而是查清 Slint 在字体族缺失时的重排行为并以最小代价
规避（例如把图标字形改用可解析的族，或预先测量一次）。

**当前状态**：门禁红（单线程 80.3%，新增控制台 0）。Release v0.1.0 保持 pre-release + "不要使用"。
### 20.1 又否掉两条，并修正一个前提

- **字体族解析不到 —— 不成立。** 这台机器**两个字体都装着**：注册表里 `Segoe Fluent Icons` 与
  `Segoe MDL2 Assets` 都在，`SegoeIcons.ttf` 与 `segmdl2.ttf` 也都在 `C:\Windows\Fonts`。也就是说
  `Theme.icon_font` 在这里是**能解析**的，§9 里"没装该字体、图标是豆腐块"那条记录对这台机器已经
  过期。相应地"改 `Theme.icon_font` 为 `Segoe MDL2 Assets`"这个实验没有做——它验证的前提不存在。
  改动已撤回（`theme.slint` 与 HEAD 一致）。
- **`AppI18n.t` 不是 `pure` —— 不成立。** `theme.slint:368` 就是
  `pure callback t(string, [string]) -> string;`。若它不是 pure，每个带文本的 `Text` 会每帧重算
  `t()`，那会完美解释"成本 ∝ 文本数"——但它是 pure。

**还剩两件事要分开，才谈得上修：**

1. **"隐藏到托盘仍然烧 89%"那次，可能根本没隐藏。** 窗口是自绘标题栏，`CloseMainWindow()` 发的是
   `WM_CLOSE`，而隐藏走的是 Slint 的 `close-requested` 回调链。若那次窗口其实仍可见，89% 与可见时的
   84% 就是同一件事，结论不变；若它真隐藏了还在烧，就说明烧的不是绘制。**验证**：隐藏前后各读一次
   `IsWindowVisible`，或用托盘双击（走应用自己的路径）来隐藏。
2. **每帧一次的重绘从哪来。** 先做 `SLINT_DEBUG_PERFORMANCE=refresh_full_speed` 对照（强制全速刷新
   后数字不变 ⇒ 它本来就在全速）；再按 `.slint` 二分——把首页的 `Flickable` + `CustomScrollbar` 换成
   普通 `VerticalLayout`，一次构建。滚动条的高度依赖 `parent.height`、`y` 依赖 `travel-dist`，是这一层
   唯一会自我引用的地方。

没有新包。门禁仍是红的。
