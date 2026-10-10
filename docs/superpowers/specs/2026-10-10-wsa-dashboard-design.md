# WSA Dashboard — 设计规格

- 日期：2026-10-10
- 状态：**待用户过目**（过目后进入 `writing-plans`）
- 参照程序：`wsl-dashboard-0.12.0`（`wsldashboard`，GPL-3.0）
- 目标仓库：`<repo>`
- 许可证：GPL-3.0（与参照一致）

---

## 0. 一句话

**照 `wsl-dashboard` 的架构、目录、并发、视觉、配置、i18n 桥 1:1 重写一个 WSA 管理面板**，把领域层从 `wsl/*` 换成 `wsa/*`，UI 层与交互保持一致。

不是 fork 改造——参照源码只做**结构与视觉的权威参照**，领域逻辑按 WSA 重写。

**明确不做的事**：不调用 `api1.wslui.com`（那是别人的后端）。`src/api/` 只保留 GitHub releases 更新检查。

---

## 1. 实测事实（本机 WSA 环境，2026-10-10 全部实跑验证）

> 这一节是本设计的地基。凡标注 ✅ 的都已实跑；标注 ⚠️ 的是待验项。

### 1.1 环境

| 项 | 值 | 来源 |
|---|---|---|
| WSA 版本 | 2407.40000.4.0（Release-Nightly + Magisk 30.6 + GApps 侧载版） | `Get-AppxPackage` |
| 安装位置 | `C:\Users\reader\Downloads\Compressed\WSA_..._\WSA_2407.40000.4.0_x64` | `InstallLocation` |
| 包身份 | `MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe` | 同上 |
| 注册方式 | **Development Mode（侧载）**，`IsDevelopmentMode=True` | 同上 |
| Android | **13 / SDK 33 / x86_64** | `adb shell getprop` |
| 构建类型 | `ro.build.type=user`、`ro.debuggable=0`、安全补丁 2023-09-01 | 同上 |
| 设备型号串 | `Subsystem for Android(TM)`（`ro.product.manufacturer` = Microsoft Corporation） | 同上 |
| adb | 37.0.1（WinGet platform-tools），指纹 MD5 `19:2E:21:14:1B:EA:97:FF:23:60:AC:EB:20:CE:C1:68` | `adb version` + certutil 独立复算 |

**⚠️ 关键推论：安装路径在 `Downloads\Compressed` 下。绝对不可硬编码。**
一切路径必须 `Get-AppxPackage MicrosoftCorporationII.WindowsSubsystemForAndroid` 取 `InstallLocation`。

### 1.2 启动安卓应用 —— `/launch` 是死路，`wsa://` 是正解

`AppxManifest.xml` 里声明了：

```xml
<uap3:Protocol Name="wsa"          Parameters="/launch %1" />
<uap3:Protocol Name="wsa-client"   Parameters="/deeplink %1" />
<uap3:Protocol Name="wsa-settings" />
<uap3:AppExecutionAlias>  <desktop:ExecutionAlias Alias="WsaClient.exe" />
<Application Id="App"          Executable="WsaClient\WsaClient.exe" EntryPoint="Windows.FullTrustApplication" />
<Application Id="SettingsApp"  Executable="WsaSettings.exe"          EntryPoint="WsaSettings.App" />
```

**`WsaClient.exe /launch <pkg>` 直接调用 —— 7 种机制全部失败：**

| # | 机制 | 结果 |
|---|---|---|
| 1 | raw `CreateProcess`（绝对路径） | `0xC0000135` 缺 DLL |
| 2 | raw + 包根目录进 `PATH` | `0xC0000409`，WER 记录**模块 `ucrtbase.dll`**（CRT 参数校验→abort） |
| 3 | 执行别名（分参 `'/launch','pkg'`） | exit 0，**零动作**，8s 内 160 次采样无新进程 |
| 4 | 执行别名（单参 `'/launch pkg'`） | exit 0，零动作 |
| 5 | COM `IApplicationActivationManager::ActivateApplication('…!App', '/launch pkg')` | hr 0、**返回了 pid**，进程秒退，应用未起 |
| 6 | 冷状态（无常驻实例）经别名 `/launch` | **子系统起来了**（t=3s vmmem / t=16s 端口），但**应用没起**，boot 完成后 `mCurrentFocus=null`、目标包仍 stopped |
| 7 | 有常驻实例时 warm `/launch` | exit 0、**不派生新进程**、24s 后仍 stopped |

**根因（Phase 1–3 结论）**：
- `0xC0000135` = `gfxstream_backend.dll` 在**包根目录**，而 `WsaClient.exe` 在 `WsaClient\` 子目录，raw spawn 的 DLL 搜索路径不含包根。经执行别名/AppX 激活即消失。
- `0xC0000409` = 缺**包身份**（`EntryPoint=Windows.FullTrustApplication`，需 `GetCurrentPackageFullName` 成功）。经执行别名即消失。
- 修掉崩溃后 `/launch` 仍无效果：**常驻单实例吞掉命令行请求，不转发**；而协议激活（`wsa://`）是同一份 WsaClient 代码的另一条入口，常驻实例会处理它。

**结论：`WsaClient.exe /launch` 在本构建上不可用，且不可修（第三方二进制行为）。**
功能由其**上游入口** `wsa://` 承载——manifest 里 `Parameters="/launch %1"` 说明协议激活本身就是走 `/launch` 那段代码。

**`wsa://` 实测（全部 ✅）：**

| 场景 | 结果 |
|---|---|
| 热启动（子系统在跑） | **1–2 秒**起目标包，焦点正确（`com.topjohnwu.magisk/.ui.MainActivity`），复现 4 次 |
| **完全冷启动**（vmmem/WsaClient/WsaService 全无）→ `wsa://com.topjohnwu.magisk` | **t=3s vmmem → t=13s 端口 58526 → t=17s `sys.boot_completed=1` + 应用 RUNNING + adb `device`** |
| **完全冷启动 → `wsa://com.android.settings`（系统包）** | ✅ **t=3s vmmem → t=8s 端口 → t=12s boot 完成 + settings RUNNING**。**⇒ 启动按钮不依赖是否装了第三方应用**，总有可用的兜底包 |
| 指定 Activity：`wsa://com.android.settings/.Settings$DevelopmentSettingsDashboardActivity` | ❌ 被降级为默认 launcher activity（焦点落在 `.Settings`）→ **只能到包级** |

### 1.3 停止 WSA

| 机制 | 结果 |
|---|---|
| `WsaClient.exe /terminate`（执行别名） | ❌ exit 0，30s 内 `vmmemWSA` 仍是 12780、端口照听、adb 照连 → **零动作** |
| **杀掉常驻 `WsaClient.exe` 进程** | ✅ `vmmemWSA` 退出、端口 58526 关闭、adb 变 offline；无崩溃事件 |

**结论：dashboard 的"关闭子系统"= 终止 `WsaClient.exe` 进程。** 这不是 hack 的猜测——已实测子系统随之干净下线。

> ⚠️ 常驻 `WsaClient` 就是维持子系统活着的那个东西（实测：杀掉它，整个子系统下线）。所以**首页状态卡不能把"WsaClient 进程在"等同于"WSA 在跑"**，两者其实是同一件事；真正的健康信号是 adb 探活 + `sys.boot_completed`。

### 1.4 adb 链路

| 事实 | 影响 |
|---|---|
| 端口 **58526**，中继由 `WsaClient.exe` 持有（manifest 声明见下） | 网络页的"端口转发"要与这个原生规则区分 |

manifest 中该规则的真实形态（`AppxManifest.xml:209-213`，2026-10-10 实读）：

```xml
<desktop2:Extension Category="windows.firewallRules">
  <desktop2:FirewallRules Executable="WsaClient\WsaClient.exe">
    <desktop2:Rule Direction="in" IPProtocol="TCP"
                   LocalPortMin="58526" LocalPortMax="58526" Profile="private" />
  </desktop2:FirewallRules>
</desktop2:Extension>
```

> **事实更正（2026-10-10，执行 Task 4 时发现）**：属性名是 **`LocalPortMin` / `LocalPortMax`**，**不存在 `LocalPort` 属性**。元素名是 `<desktop2:Rule>`，外层是 `<desktop2:FirewallRules Executable="...">`（带 `Executable` 属性，容易被"紧接着 `>`"的正则漏掉）。解析见 `src/adb/port.rs`。
| **WSA 停止后 adb server 会把设备条目整个清掉**（不是留 offline） | 每次启动后必须显式 `adb connect 127.0.0.1:58526`，不能假设条目还在 |
| 连接中会短暂出现 `unauthorized`（等 RSA 授权） | 探活要容忍三态：`absent` / `offline` / `unauthorized` / `device`，只有 `device` 才算就绪 |
| `adb root` 不可用：`adbd cannot run as root in production builds` | ❌ 不要用 `adb root` |
| **`adb shell su -c id` → `uid=0(root) ... context=u:r:magisk:s0`** | ✅ **Magisk 给 adb shell 授了 root，立即可用（无超时）**。首页"是否 root"用这条探测，不要用 `adb root` |
| **`settings put global http_proxy <host:port>` / `global_http_proxy_host` / `_port` 在 user 构建上 shell 直接可写**（读回一致，已测后清理复原） | ✅ **网络页的"HTTP 代理"块可实现**，不必隐藏 |
| **`adb reverse tcp:A tcp:B` create/list/remove 全通**（`adb reverse --list` 输出 `host-16 tcp:18080 tcp:18080`），且**数据面确认**：host 侧 loopback listener 接受了来自安卓的 TCP 连接 | ✅ `adb forward` / `adb reverse` 均可用 |
| Android 上的探针工具：`/system/bin/nc`、`/system/bin/toybox` 有；`curl`/`wget`/`python`/`busybox` **都没有** | 排查命令要用 `nc`/`toybox`，别用 `curl` |
| 设备端 `nc` 是 toybox 实现，参数格式与 GNU nc 不同（`nc 127.0.0.1 18080` 报 `bad argument count`） | 写探针脚本时注意 |
| `adb install -r` / `adb uninstall` 回环 | ✅ 实测 pull 54MB → uninstall `Success` → install `Success` → 包仍在 |
| 健康判定 | ✅ 用 `adb shell getprop sys.boot_completed` == `1`。**不能用 `mCurrentFocus`**——实测 boot 完成后它可长时间为 `null`（home 窗口未附着） |
| 拿不到安卓应用 label | 平台限制（`pm list`/`dumpsys` 均不返回）。维持既有方案：包名规则化 + `LocalState` 图标缓存，有 `aapt` 才解析真名 |

### 1.5 相关路径

| 用途 | 路径 |
|---|---|
| 图标缓存 | `%LOCALAPPDATA%\Packages\MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe\LocalState\<包名>.png/.ico` |
| 用户数据 VHDX | 同上 `LocalState\userdata.2.vhdx`（3.33 GB）+ `metadata.2.vhdx`（74 MB） |
| 执行别名 | `%LOCALAPPDATA%\Microsoft\WindowsApps\WsaClient.exe`（0 字节 reparse point） |
| 打开子系统设置 | `explorer shell:AppsFolder\MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe!SettingsApp`，或 `wsa-settings://` |
| adb 密钥 | `%USERPROFILE%\.android\adbkey` / `adbkey.pub`（生成于 2026-09-22，注释 `reader@DESKTOP-EXAMPLE`） |

### 1.6 已知的参照环境坑（与设计相关）

- `WsaSettings.exe` 今日崩 2 次（`Windows.UI.Xaml.dll`，`0xc0000409`）→ **dashboard 不得依赖设置面板存活**；子系统设置页必须是"深链 + 打不开时给出手动路径"的降级。
- `WsaClient` 曾崩一次留 4 个 tombstone，随后停在 `/partiallyrunning` → 半运行状态**不影响 adb**，所以**启动状态不能看 `WsaClient` 命令行**。

### 1.7 包名 → 显示名规则（新增，2026-10-10 补）

安卓平台不返回应用 label（`pm list packages` / `dumpsys` 均只给包名，见 §1.4 与 R3）。
显示名由包名规则化推导，**规则固定为**：

1. 取包名 **`rsplit('.')` 的最后一段**（`com.topjohnwu.magisk` → `magisk`）；
2. 在 **`_` 与 `.`** 上分词（防最后一段仍含 `_`，如 `com.example.my_app`）；
3. 每个非空词**首字母大写**，其余保持原样（不转小写）；
4. 以单个空格连接。

示例：`com.topjohnwu.magisk` → `Magisk`；`com.example.my_app` → `My App`；单段 `magisk` → `Magisk`。

实现锚点：`src/wsa/models.rs::AppInfo::display_name`。这条规则同时驱动 Q29 的图标占位块（首字母取自该显示名，颜色按完整包名哈希）。

---

## 2. 参照架构 → 目标架构（1:1 映射）

### 2.1 目录

```
wsl-dashboard                          wsa-dashboard
├── src/main.rs                     →  同（launcher→lifecycle→runner 12 步保持不变）
├── src/app/                        →  同名保留
│   ├── launcher.rs lifecycle.rs runner.rs
│   ├── single_instance.rs tray.rs window.rs state.rs
│   ├── scheduler.rs task_scheduler.rs autostart.rs
│   ├── updater.rs popup.rs uninstall.rs startup_info.rs
│   ├── cli.rs constants.rs initialize.rs
│   └── mount_disk/ scheduler_task/ tasks/
├── src/wsl/                        →  src/wsa/      ★ 领域层重写
│   ├── command.rs                  →  wsa/command.rs    命令定义
│   ├── executor.rs                 →  wsa/executor.rs   执行与并发闸门
│   ├── decoder.rs                  →  wsa/decoder.rs    stdout→结构
│   ├── parser.rs                   →  wsa/parser.rs     结构→模型
│   ├── models.rs                   →  wsa/models.rs
│   ├── ops/                        →  wsa/ops/          具体操作
│   └── dashboard/                  →  wsa/dashboard/    Dashboard 门面 + AppOpGuard
├── src/usb/                        →  src/adb/      ★ USB 设备页 → adb 设备页
├── src/network/                    →  同名保留（语义换成 WSA 的 forward/reverse/代理）
├── src/config/                     →  同名保留（`~/.wsadashboard/*.toml` + 版本迁移）
├── src/i18n/                       →  同名保留
├── src/api/                        →  同名保留但**只留 GitHub releases**
├── src/utils/                      →  同名保留
└── src/ui/                         →  同名保留（app.slint / theme.slint / data.rs / views / components / handlers）
```

### 2.2 三层调用关系（照搬）

```
UI handler  →  WsaDashboard (src/wsa/dashboard/)
                    ↓
              ops/*（原子操作，如 ListApps / InstallApk / StopSubsystem）
                    ↓
              WsaCommandExecutor（唯一出口，串行化 + 超时 + 取消）
                    ↓
              WsaCommand（枚举：AdbConnect / PmList / AmStart / GetProp / …）
```

**`WsaOpGuard`**（对应参照 `src/wsl/dashboard/operation_guard.rs` 的 `DistroOpGuard`——**注意参照里没有叫 AppOpGuard 的东西**）：`create` 时注册操作名、`Drop` 时反注册；注册过则 UI 弹 `toast.distro_busy`。参照的 op 名为 `"Starting"/"Stopping"/"Restarting"/"Deleting"/"Exporting"/"Importing"/"Moving"`，我们换成 `"Starting"/"Stopping"/"Launching"/"Installing"/"Uninstalling"`。

**其余并发原语照搬**（`src/wsl/dashboard/mod.rs`、`executor.rs`）：

- `heavy_op_lock: Arc<Mutex<()>>` —— 重活互斥
- `manual_operation` —— `AtomicI32` 计数器
- executor 信号量 **`Semaphore::new(16)`** + 后台 **`Semaphore::new(4)`**
- 超时分级：**读 10s / 写 45s / 重活 1800s**（`install`/`uninstall` 归重活）
- 信号量获取超时 **20s**（流式 10s）
- 取消 = tokio timeout future 上的 **`kill_on_drop(true)`**

### 2.3 `WsaCommand` 初版枚举（按 1.2/1.4 实测确定）

| 变体 | 落地命令 | 备注 |
|---|---|---|
| `ConnectAdb` | `adb connect 127.0.0.1:58526` | |
| `ProbeReady` | `adb shell getprop sys.boot_completed` | **健康唯一真值** |
| `ListPackages` | `adb shell pm list packages -3` | 首页应用列表 |
| `PackageInfo` | `adb shell dumpsys package <pkg>` | 版本/大小/权限 |
| `LaunchApp` | `Start-Process wsa://<pkg>` | **不走 `WsaClient /launch`** |
| `StopApp` | `adb shell am force-stop <pkg>` | |
| `InstallApk` | `adb install -r <path>` | |
| `UninstallApk` | `adb uninstall <pkg>` | |
| `StartSubsystem` | `Start-Process wsa://<任一已装包>` | ⚠️ 冷启动 17s，UI 要有进度语义 |
| `StopSubsystem` | 终止 `WsaClient.exe` 进程 | ✅ 实测干净下线 |
| `OpenSettings` | `explorer shell:AppsFolder\…!SettingsApp` | |
| `ForwardPorts` / `ReversePorts` | `adb forward` / `adb reverse` | 网络页 |
| `DeviceProps` | `adb shell getprop`（批量） | 设备页 |
| `VhdxInfo` | 读 `LocalState\*.vhdx` 元数据（不调 adb） | 磁盘页，**WSA 停止时也必须能用** |

---

## 3. 九页签映射（照搬 `views/` 结构与切换方式）

参照用 `app.slint` 里 **opacity 切换**（不是条件销毁）保持页间状态。照搬。

| # | 参照 view | 目标 view | 内容 |
|---|---|---|---|
| 1 | `main_view.slint` | `main_view.slint` 首页 | WSA 状态卡（三态降级，见 §5）+ 应用卡片网格 |
| 2 | `add_view.slint` | `add_view.slint` 安装 APK | 拖拽/选择 APK → 进度 → 结果；对照"添加实例"的位置与交互 |
| 3 | `usb_view.slint` | `device_view.slint` 设备 | adb 目标列表、连接/断开、设备属性表 |
| 4 | `network/` | `network/` 网络 | 三块结构照搬：**端口转发**（改 `adb forward` / `adb reverse`，参照的 netsh portproxy 语义不适用）+ **HTTP 代理**（改 `settings put global http_proxy` + `adb reverse` 把设备 `127.0.0.1` 桥到宿主）+ 组件说明。netsh 防火墙规则段不适用，保留结构但隐藏 |
| 5 | `scheduler_view.slint` | `scheduler_view.slint` 计划任务 | 定时启动/停止/安装，走同一 TaskScheduler |
| 6 | `mount_disk_view.slint` | `disk_view.slint` 磁盘 | **语义重定义：参照的"把物理盘/VHD 挂进 WSL"（`wsl --mount`）在 WSA 侧不存在**。改为"WSA 存储管理"：`userdata.vhdx` / `metadata.vhdx` 元数据（大小、路径、占用）+ `adb shell df` 的 Android 存储用量 + 应用占用 Top N。卡片结构与交互照搬，只换数据源。**此页不依赖 adb**（VHDX 读本地文件），是三态降级的关键 |
| 7 | `donate_view.slint` | `donate_view.slint` 捐赠 | 1:1 |
| 8 | `settings/mod.slint` | `settings/mod.slint` 设置 | 通用设置 + **WSA 子系统设置深链**（含打不开降级） |
| 9 | `about_view.slint` | `about_view.slint` 关于 | 版本/更新检查（GitHub releases）/许可证 |

侧栏 **54px ↔ 200px** 折叠、主题 token（`theme.slint`）、`title_bar`、`scrollbar`、`modal_manager` 全部照搬。组件命名保持一致（`*_card.slint`、`form_widgets.slint`、`dialogs.slint`、`task_status_toast.slint`…）。

`handlers/` 12 个按语义替换：`instance.rs`→`app.rs`、`usb.rs`→`device.rs`、`wsl_guide.rs`→`wsa_guide.rs`、`mount_disk.rs`→`disk.rs`，其余（about/common/donate/mail/scheduler/settings/update/window）同名保留。

---

## 4. 并发与数据流（照搬）

- **4 秒防抖 + 快照去重**（对应 `src/ui/data.rs`）：数据变更不直接推 UI，先在内存里比对快照，变了才 `invoke`。
- **`WsaCommandExecutor` 单一出口**：所有 adb 调用串行过闸，避免并发 adb 命令互相踢（adb server 是全局单例）。
- **TaskScheduler 优先级链**（计划任务 + 专注模式 DND）照搬。
- **单实例**（`single_instance.rs`）、**托盘**（`tray.rs`）、**启动信息**（`startup_info.rs`）照搬。
- adb 探活循环与 UI 线程隔离，走 tokio `rt-multi-thread`。

---

## 5. 首页三态降级（用户硬要求：adb 不通时不得空白）

| 态 | 判定 | 渲染 |
|---|---|---|
| **T1 就绪** | `adb devices` 有 `58526 device` 且 `sys.boot_completed=1` | 状态卡正常 + 应用网格 |
| **T2 部分** | 端口在听但 `offline`/`unauthorized`，或 boot 未完成 | 状态卡显示进度/等待授权，应用网格显示**骨架占位**（不是空白），"重连"按钮可用 |
| **T3 断开** | 端口不通 / WSA 未启动 | 状态卡显示"子系统未运行"+ **启动按钮**（`wsa://` 冷启动 17s，按钮进入带进度的忙碌态）；应用网格显示**上次快照 + 置灰**，并标注"数据可能过期"；磁盘页/设置页/关于页**完全不受影响仍可用** |

**T3 下磁盘页必须可用**——VHDX 信息读的是本地文件，不依赖 adb。这是降级设计里最容易漏的一条。

---

## 6. 配置与迁移

- 配置目录 **`~/.wsadashboard/`**，文件名与参照一一对应照搬：`settings.toml`（`SETTINGS_VERSION=10`）、`instances.toml`（`INSTANCES_VERSION=2`）、`network.toml`（`NETWORK_VERSION=1`）、`usb.toml`（`USB_CONFIG_VERSION=1`）、`mount.toml`（`MOUNT_DISK_CONFIG_VERSION=1`）、`debug.toml`、`sync.toml`（`SYNC_VERSION=1`）。
- **文件名一律照搬，不改名**（`usb.toml` 里存的是 adb 设备配置、`sync.toml` 建空结构占位）——同构才能让迁移链代码逐行复用。
- 迁移链照搬 `config/migration.rs`（v0→v10 逐步迁移）+ `config/mod.rs:472-532` 的一次性迁移。
- `config/instances.rs`、`mount.rs`、`debug.rs`、`models.rs` 同构保留，字段语义替换为 WSA（如 `distro` → `subsystem`），**保留版本号字段名以便逐行对照**。

---

## 7. i18n

- **参照实际有 50 个语言文件，每个精确 563 键、27 个表**（含 `fil` 等），全部照搬。
- `build.rs::verify_translations`（`build.rs:145-197`）：以 `en.toml` 摊平后的键集为基准，逐个比对其他语言，缺键**只 `cargo:warning`，从不 fail**。数组值键按 `key[i]` 展开（`build.rs:199-223`）。照搬。
- 运行时 `src/i18n/mod.rs:199-253` 加载 `assets/i18n/{lang}.toml`，缺省回落 `en`。
- Slint ↔ Rust 的 i18n 桥走 `AppI18n.t(string, [string])` 回调（`theme.slint:279`，在 `app/runner.rs:78` 接线）。照搬。
- **键名一律保留原名**，只替换值里的领域词 → 50 种语言的翻译几乎零成本复用。

---

## 8. 构建（已验证的坑，写进 `build.ps1`）

| 坑 | 正确做法 |
|---|---|
| Skia 预编译包下载 | 官方 `curl` 通道在本机 **300s 超时**。必须手工 `Invoke-WebRequest` 下载 24.5 MB 到本地缓存 |
| `SKIA_BINARIES_URL` | 必须 **`file://C:/...`（两个斜杠）**。三个斜杠 `file:///C:/` 会 `strip_prefix("file://")` 得 `/C:/...` → `os error 123` |
| 不设该变量的后果 | 300s 超时 → 转全量编译 → 死在 `Unable to locate LLVM installation` |
| 本机已验证探针 | `slint 1.17.1 + skia` 构建通过（13.92s），探针工程留存于 `%TEMP%\opencode\slint-probe` |

**依赖照搬 `Cargo.toml`**：`slint =1.17.1 (backend-winit, renderer-skia)`、`tokio 1.53.1`、`tracing*`、`serde/toml/uuid/rfd/open/image/ureq/reqwest/futures-util/semver/chrono/rand/dirs/windows 0.62.2/rust-embed/once_cell/tray-icon/scopeguard/rust-ini/if-addrs/urlencoding/async-trait/native-tls/md5`；`edition=2024`、`rust-version=1.92`、release `opt-level=z + lto + codegen-units=1 + strip + panic=abort`；`build.rs` 照搬（slint_build + ico 生成 + embed-resource + i18n 完整性校验 + `APP_EXPIRE_TIME`）。

**本机工具链**：cargo 1.98.1 + MSVC 可用。

---

## 9. 风险与降级

| # | 风险 | 结论 |
|---|---|---|
| R1 | 启停 WSA 的命令 | ✅ **已解决**：启动 `wsa://`（冷 17s）、停止杀 `WsaClient`。`/launch`、`/terminate` 均不可用，已彻底证伪 |
| R2 | 冷启动是否必须指定某个应用？ | ✅ **已验证**：第三方包冷启动 17s、**系统包 `com.android.settings` 冷启动 12s**。启动按钮固定用 `wsa://com.android.settings` 兜底，无需探测是否有第三方应用 |
| R3 | adb 拿不到应用 label | 已有方案：包名规则化 + `LocalState` 图标缓存 + 可选 `aapt` |
| R4 | 首页 adb 不通不得空白 | §5 三态降级，T3 下磁盘/设置/关于页仍可用 |
| R5 | `WsaSettings.exe` 不稳定（今日崩 2 次） | 子系统设置页只做深链 + 手动路径降级，不依赖其存活 |
| R6 | 侧载包可能被卸载/移动 | 路径一律 `Get-AppxPackage` 动态解析；包不存在 → T3 态 + 明确提示 |
| R7 | adb 命令并发互相踢 | `WsaCommandExecutor` 单闸门串行 |
| R8 | 计划任务在 WSA 未启动时触发 | TaskScheduler 要能接受"先冷启动再执行"的两段式，且冷启动有 17s 预期 |

---

## 10. 阶段划分（P0–P9，每阶段末尾产出可运行 exe）

| 阶段 | 交付 | 验收 |
|---|---|---|
| **P0** | 仓库骨架 + `build.ps1`（Skia 坑已封装）+ 空窗口 | `cargo run` 出窗口 |
| **P1** | 领域层 `src/wsa/` + `src/adb/`：`WsaCommand` 枚举、executor、decoder、parser、models、ops（启停/安装/探活命令均已实测确定，见 §1.2–1.4） | 命令行可列出包、可冷启动、可停止 |
| **P2** | 首页：状态卡 + 应用网格 + 三态降级 | 断 adb 时首页不空白 |
| **P3** | 侧栏 54/200 折叠 + 页签 opacity 切换 + 主题 token + 标题栏 | 与参照逐页对照视觉 |
| **P4** | 安装 APK 页 + 设备页 | 安装/卸载回环可用 |
| **P5** | 网络页 + 磁盘页 | forward/reverse 可用；WSA 停止时磁盘页可用 |
| **P6** | 设置页 + 关于页 + i18n 桥 + 语言包 | 中英文切换，缺 key 有 warning |
| **P7** | 计划任务 + TaskScheduler + 托盘 + 单实例 | 任务在子系统未启动时能冷启动后执行 |
| **P8** | 配置迁移 + 更新检查（GitHub releases）+ 打包 exe | 全流程可复现 |
| **P9** | 与参照程序逐页对照验收 | 见 §11 |

---

## 11. P9 验收清单

- [ ] 9 页签与参照一一对应，切换方式（opacity）一致
- [ ] 侧栏折叠宽度 54/200，折叠态图标一致
- [ ] 主题 token 覆盖一致，深浅色切换一致
- [ ] 首页在 adb 断开时不空白，三态渲染符合 §5
- [ ] 安装 APK / 卸载回环可用
- [ ] 冷启动子系统 ≤ 30s，热启动应用 ≤ 3s
- [ ] 关闭子系统后磁盘页/设置页/关于页仍可用
- [ ] 配置目录 `~/.wsadashboard/` 与迁移链生效
- [ ] i18n 中英文无缺 key
- [ ] 计划任务在 WSA 未启动时能自动冷启动后执行
- [ ] 单实例 + 托盘 + 关闭行为一致

---

## 12. 附：本次调试的完整证据索引

- `WsaClient.exe` raw spawn `0xC0000135` → 缺包根的 `gfxstream_backend.dll`
- raw + PATH → `0xC0000409`，WER 记录**出错模块 `ucrtbase.dll`**（不是 WsaClient 自身）
- `AppxManifest.xml` 全文已读（16278 bytes），协议/别名/COM server/防火墙规则/启动任务全部列出
- `com.android.settings` 存在且 `.Settings` 可解析 → 排除"包名不存在"
- 依赖包 `Microsoft.UI.Xaml.2.8`、`Microsoft.VCLibs.140.00(.UWPDesktop)` **全部已安装** → 排除依赖缺失
- 冷启动时间线、warm 对照组、`wsa://` 4 次复现、`adb install/uninstall` 回环，均已实跑

---

## 13. 决策记录（2026-10-10 grilling，40 问全定）

设计树已走完，无分支依赖假设。逐条记录，供实现期随时回查。

| # | 决策 | 结果 |
|---|---|---|
| Q1 | UI `.slint` 处理 | **复制全部 `ui/`**，只换领域绑定 |
| Q2 | 仓库 / 更新检查 | git init 本地，**暂不建远程**；更新检查显示"未配置" |
| Q3 | 交付形态 | **只要 `cargo build --release` 的 exe** |
| Q4 | i18n 语言 | 见 Q36 修正（原答"四语"基于我给错的事实） |
| Q5 | 捐赠页 | 1:1 照搬结构，**收款内容留空占位** |
| Q6 | 应用图标 | 开发期复用参照 ico，P8 打包前换新 |
| Q7 | P1 先出 CLI 验证器 | **要**，`[[bin]]` 单独 target |
| Q8 | 自动化测试 | `decoder`/`parser`/`models` 单测；executor 不写（依赖实机） |
| Q9 | 日志 / 崩溃 | 日志全保留；**去掉 `dhat` 与 `crash_reports/`** |
| Q10 | 阶段汇报 | 每阶段跑起来 + 截图 + 逐条对照 |
| Q11 | 项目定位 | **打算发布** → P9 按发布标准验收 |
| Q12 | 标识符与 i18n 键名 | **一律保留原名**，只改显示文案；写 `MAPPING.md` 记录 `distro→app` 映射 |
| Q13 | ru / vi 翻译来源 | 复用参照官方翻译 |
| Q14 | exe 外发现 DLL | **破例做 zip**，不回来等 |
| Q15 | 关于页更新区块 | 显示但灰掉，标"更新源未配置" |
| Q16 | 捐赠页留空呈现 | 保留原版布局 + 灰色占位块 |
| Q17 | 依赖版本 | **照搬 `Cargo.toml` + `Cargo.lock`** |
| Q18 | 参照源码固化 | 复制精简参照到 `docs/reference/` |
| Q19 | 命名 | package/GUI bin = `wsadashboard`，CLI bin = `wsadash` |
| Q20 | P0 内容 | **git init + 首次提交**（含 spec 与参照副本）+ 骨架 + 空窗口 |
| Q21 | 配置目录 | `~/.wsadashboard/` |
| Q22 | 卡片无对应动作 | **16 个回调全保留，无对应的按钮 `visible: false`**，i18n 键全留 |
| Q23 | 新增 WSA 特有动作 | **不新增**，P9 验收通过后再统一加 |
| Q24 | 首页布局 | **照搬竖排 48/96px 卡片 + 拖拽排序，顶部加搜索框**（搜索框标进 `MAPPING.md`） |
| Q25 | GUI 命令行开关 | **六个全照搬**（`/version` `/help` `/silent` `/scheduler` `/clean` `/initialize`） |
| Q26 | adb 来源 | 配置可指定路径，未配置走 PATH，找不到给安装引导 |
| Q27 | adb 端口 | **从 WSA 包 manifest 动态读**，失败回退 58526。读的是 `LocalPortMin`/`LocalPortMax`（见 §1.4 事实更正）；两者不等时返回 `None` 不猜，回退由调用方做 |
| Q28 | WSA 未安装 | 照搬 `DistroNotFoundView` 结构，换文案与链接 |
| Q29 | 无图标占位 | 包名首字母纯色块（按包名哈希取色） |
| Q30 | 配置目录文件名 | 见 §6 |
| Q31 | 应用信息呈现 | **面板内展示 `dumpsys package` 摘要**，不跳转安卓窗口 |
| Q32 | 磁盘页语义 | **改"WSA 存储管理"**（VHDX 元数据 + `df` + 应用占用 Top N） |
| Q33 | 网络页切分 | 端口转发 `adb forward`/`reverse`；**HTTP 代理块做**（实测 `settings put global http_proxy` shell 可写）；netsh 防火墙段隐藏 |
| Q34 | 计划任务落地 | **应用内 TaskScheduler + Windows 计划任务两套都照搬** |
| Q35 | 配置文件映射 | **文件名全照搬**，`usb.toml` 存 adb 设备配置，`sync.toml` 空结构占位 |
| Q36 | 语言包 | **全部 50 个文件照搬**（更正：参照不是 4 语而是 50 语 × 563 键） |
| Q37 | 关于页数据来源 | 结构照搬，字段留空占位 |
| Q38 | 单实例命名 | 三件套结构照搬全换 WSA 变体：`wsadashboard-v0.3-lock` / `wsadashboard_wakeup_pipe_v0.3` / 窗口标题 `WSA_DASHBOARD_WINDOW_UI` |
| Q39 | 托盘菜单 | **照搬 2 项**（显示窗口 / 退出）+ 分隔符 + 双击切换 |
| Q40 | 版本过期机制 | 照搬结构，`expire = 0`（永不过期） |

### 事实更正（我先前写错，已修正到对应章节）

1. **`AppOpGuard` 不存在**，实为 `DistroOpGuard`（`src/wsl/dashboard/operation_guard.rs`）——见 §2.2
2. **配置文件不是** `config.toml`/`app.toml`/`language.toml`/`mount_disk.toml`，实为 §6 所列七份
3. **i18n 不是 4 种语言，是 50 种**，每份精确 563 键 —— 见 §7

### 本轮新增实测（Q33 相关）

| 事实 | 结论 |
|---|---|
| `adb shell settings put global http_proxy 127.0.0.1:7890` 读回一致 | ✅ user 构建上 shell 可写，代理块**可实现**，不用隐藏 |
| `global_http_proxy_host` / `_port` 同样可写 | ✅ 兼容旧版读取方式 |
| `adb reverse tcp:18080 tcp:18080` → `host-16 tcp:18080 tcp:18080`；host 侧 listener **接受了来自安卓的 TCP 连接** | ✅ 隧道数据面确认（安卓 `127.0.0.1` → 宿主回环） |
| `adb forward tcp:18081 tcp:18081` → `127.0.0.1:58526 tcp:18081 tcp:18081` | ✅ |
| **`adb shell su -c id` → `uid=0(root) ... context=u:r:magisk:s0`** | ✅ **root 立即可用**（`adb root` 不可用，但 `su -c` 可用）——首页 root 探测用这条 |
