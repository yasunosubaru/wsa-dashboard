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