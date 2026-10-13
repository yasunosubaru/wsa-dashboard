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

1. **用 `PrintWindow` 不用 `CopyFromScreen`。** 后者拍的是桌面，不是窗口：会返回
   *窗口移动之前*的帧（三张拍成别的应用，首页拍成自己的首帧，而模型说它已经加载完了），
   还会被置顶窗口和 f.lux 污染。`PrintWindow` 直接向窗口要内容，上面三件事都不存在。
2. **窗口用枚举 + 尺寸下限选，不用 `Process.MainWindowHandle`。** 本二进制的单实例握手
   窗口会让后者在多次运行中返回那个 158×26 的隐藏 shim，于是拍的是 shim。

验证方式：`cargo test --release --bin wsadash` → **189 passed / 0 failed / 3 ignored**；
GUI `cargo build --release --bin wsadashboard` 零 error 零 warning；
四态截图 `docs/screenshots/p2-{ready,booting,offline,stopped}.png`（1200×900，
四张同尺寸可直接并排）。
