# P4 计划：安装 APK 页 + 设备页

依据：spec §10 的 P4 行「安装 APK 页 + 设备页」，验收「安装/卸载回环可用」；
spec §3 的 view 映射第 2、3 行。前置：P0–P3 已完成（`7c8115f`）。

## 0. 先纠正 P3 计划里的一条隐含假设：P4 不是搬，是重写

P3 是纯移植（`sidebar`/`title_bar`/`common` 三个组件逐行照抄）。**P4 两个 view 都不是**，
读过源码之后确认：

| 参照 | 体量 | 它真正做的是 | spec §3 的映射 |
|---|---|---|---|
| `views/add_view.slint` | 520 行 | **装一个 WSL 实例**：选发行版（`installable_distro_names`）、选安装源（`install_sources`）、发行版镜像（`is_mirror_loading`/`mirror_distro_names`/`mirror_source_url`）、rootfs 帮助、`distro_location`、跑完拿到 `terminal_output` | → 安装 APK：「拖拽/选择 APK → 进度 → 结果；**对照"添加实例"的位置与交互**」 |
| `views/usb_view.slint` | 258 行 | **管 usbip**：`usbipd_version`、`is_usbipd_outdated`、bind/unbind/attach/detach | → 设备页：「**adb 目标列表、连接/断开、设备属性表**」 |

两者的**属性、回调、数据源**都换了，参照里没有一样东西能留。能照搬的是**位置与交互**，
以及 `components/form_widgets.slint`（P2 已整份拷过来，这一页就用它）。

所以 P4 的"1:1"落在**结构层**而不是逐字层：view 放哪、表单怎么排、卡片怎么展开、
进度怎么显示，按参照；字段和命令按 spec §3。

## 1. 域层先备齐（P1 给了什么、P4 缺什么）

`src/wsa/dashboard/ops.rs` 已有：`probe`/`list_apps`/`running_apps`/`app_info`/
`launch_app`/`stop_app`/**`install_apk`**/**`uninstall_apk`**/`start_subsystem`/`stop_subsystem`。
`WsaCommand` 已有 `InstallApk`/`UninstallApk`/`ListAdbDevices`/`GetProps`/`Shell`。

**缺的只有两样**：

1. **断开一个目标**：`DisconnectAdb { serial }` → `adb disconnect <serial>`。
   走**新变体**而不是 `Shell { args }`，因为 spec §2.2 的判据是"它是 adb 命令面"——
   disconnect 就是 adb 命令，有 argv、该过 executor，`Shell` 是给"不值得一个变体的"逃生口用的
   （这正是 P2 写在 §4.1.1 表末的规则）。
2. **把 `adb devices` 的结果按目标讲出来**：现在 `find_entry` 只为 probe 判"我们这个端口是
   device/offline/unauthorized/absent"。设备页要的是**所有目标**的 `{serial, state, ...}`，
   是同一个解析往前进一步，不是第二套解析。

## 2. Task 分解

每个 task：先红后绿、独立提交、`cargo test --release --bin wsadash` 全绿、
`cargo build --release --bin wsadashboard` 零 warning。

| # | Task | 交付 | 可测性 |
|---|---|---|---|
| 1 | **`DisconnectAdb` + 目标列表解析** | `command.rs` 新变体 + `argv()`；`decoder.rs` 把 `adb devices` 解析成 `Vec<AdbTarget{serial,state}>`，`find_entry` 改为基于它 | 纯函数，替身 adb 输出可直接钉住 |
| 2 | **`ops::adb_targets()`** | 返回全部目标；连接/断开两个 op。**不做**"自动连接全部"——只有首页 probe 那条路才自动连（§4.1.2 已记理由） | 替身 adb |
| 3 | **`device_view.slint`** | `device_view.slint` + 目标卡片；照参照 `usb_view` 的分组与展开交互（`expanded_xxx` 单开单合）；属性表**在页内**，不弹窗 | 编译 + 截图 |
| 4 | **`handlers/device.rs`** | 按 spec §3 命名（参照 `usb.rs`）；`select_tab(2)` 刷新目标列表 | 手动 |
| 5 | **接进外壳** | `app.slint` 加 `AddInstanceView` 的对应物与 `DeviceView`、页签绑定；翻 `sidebar_usb`；`TABS_WITH_A_VIEW += 2`；补 i18n `sidebar.*` 已在 P3 备好 | 编译 |
| 6 | **`add_view.slint`（APK 版）** | 位置与交互照参照：表单自上而下、`CustomButton`/`CustomLineEdit`、结果区与 `terminal_output` 的位置换成安装输出；**字段换掉**：APK 路径（文件选择器，`rfd` 已是依赖）、安装状态、成功/失败 | 编译 + 截图 |
| 7 | **`handlers/app.rs`** | 按 spec §3（参照 `instance.rs`→`app.rs`）；`on_install_apk` / `on_uninstall_apk`，busy toast 沿用 P2 的 `Action`/`Busy` 形状 | 手动回环 |
| 8 | **验收** | 真机**安装/卸载回环**：`wsadash install` 装一个真 APK → 首页出现 → `wsadash uninstall` → 消失；四张以上截图；MAPPING §11 | 实机 |

## 3. 提前定的几处取舍（避免做到一半再决定）

### 3.1 不搬 `usb_card` / 两个 usb 对话框 / `install_logic.rs`

- `usb_card.slint`（402 行）装的是 `UsbDevice`（busid、vid:pid、绑定状态、usbip 任务）。
  我们的设备行是 adb target（serial + state），**没有一样字段对得上**，照搬等于搬一堆永远
  不读的属性。新写 `device` 行，结构照 `usb_card` 的**展开/折叠与图标列位置**。
- `usb_settings_dialog`（317 行）是"把物理 USB 盘绑进 WSL 的 usbipd"——spec §3 已明说
  参照的 netsh/防火墙段"不适用，保留结构但隐藏"，这一页同理。
- `usb_device_info_dialog`（210 行）是设备属性表。spec §3 写的是"**设备属性表**"，没说
  它必须是弹窗；**放在页内**。这省掉一个对话框，也就省掉 P5 才建的 `modal_manager`
  （参照的 `modal_manager.slint` 是 790 行，只为仲裁对话框而存在——P5 有对话框了再搬）。
- `distro/install_logic.rs`（713 行）跑的是 WSL：下载校验和、解压 rootfs、配置镜像、
  逐行往 `terminal_output` 里写。`adb install -r` 是一条命令、一次往返、exit code + stdout
  就是结论（P1 已按 §4.1.1 的"两者都要"实现）。**没有任何东西可搬**。

### 3.2 装 APK 的"进度"

参照的进度是一路写进 `terminal_output` 的日志。我们没有日志可写：`adb install` 是**一次**
同步往返，spec §2.3 给它的是 45s 的 write 超时。所以形态是**前置 busy toast**（P2 已有
`Action::Launch` 那套），成功后 toast 清掉、页面刷新，失败时 toast 留着显示错误。
`install_status` / `install_success` 两个属性仍然接，值由 toast 的状态驱动——
参照的属性名和类型保留在 `app.slint` 上，免得 P5 的设置页来对时对不上。

### 3.3 设备页不会自动重连

首页 probe 那条"条目缺失就 `adb connect` 一次"是 spec §1.4 的硬要求（不这么做会永远
报 `Stopped`）。设备页是**用户来看和断开目标**的地方，每秒自动把用户刚断开的东西连回去
就是和用户对着干。所以 `adb_targets()` 纯读，连接只在用户点"连接"时发生。

### 3.4 tab 索引

spec §3 表的顺序即索引：1 = add（安装 APK）、2 = usb（设备）。
`TABS_WITH_A_VIEW` 从 `&[0]` 变成 `&[0, 1, 2]`，同时 `app.slint` 翻 `sidebar_add`、
`sidebar_usb`。Settings 那条无开关的守卫仍然按 P3 的 `tabs.rs` 走。

## 4. Review Focus（做 task 时逐条对着看）

1. `DisconnectAdb` 是**新变体**不是 `Shell`——`Shell` 是逃生口，写了第一个就有第二个。
2. 目标列表的解析只有一处：`find_entry`（首页）和 `adb_targets()`（设备页）必须共用同一套
   解析，两套解析会在"offline 拼写"这类地方悄悄分叉。
3. `device_view` 不得出现 `usbipd` / `bind` / `attach` 字样——那是参照的领域，不是 WSA 的。
4. 页签推进必须是**同时**改 `TABS_WITH_A_VIEW` 和 `sidebar_*`，少改一个就是"看得见点不开"
   或"点得开一片空白"，正是 P3 守卫存在的理由。
5. 安装回环必须在**真机真 APK** 上跑一遍（spec §10 验收就一句话），替身 adb 只证明命令面。
6. 搬 `form_widgets` 而不是自造控件：P2 已整份拷过，且它带参照的主题化。
7. MAPPING 要写清**哪几样是刻意没搬**（3.1 的四条），否则 P9 逐页对照时看起来像漏做。
