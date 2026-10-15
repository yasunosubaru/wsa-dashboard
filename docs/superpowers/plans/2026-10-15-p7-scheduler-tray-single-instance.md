# P7 — 计划任务页、托盘、单实例 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 侧栏页签 4 打开一个真的页面；托盘、单实例、开机自启三件 app 级的事到位。

**Tech Stack:** Rust 2021、Slint 1.17、`tokio`、`windows` crate。

**Spec:** `docs/superpowers/specs/2026-10-10-wsa-dashboard-design.md` §3 九页签表（索引 4）、§10 阶段表（P7）。

## Global Constraints

与 P6 相同：GPL-3.0-only、每个新文件带 SPDX 两行头、标识符/i18n 键名/配置文件名保留参照原名、
**可测的判断必须在纯模块**、不新增参照没有的组件、每 task 先红后绿独立提交。

测试命令：
```
$env:SKIA_BINARIES_URL = 'file://C:/Users/reader/AppData/Local/Temp/opencode/skia-cache/0.99.0/skia-binaries-{key}.tar.gz'
cargo test --release --bin wsadash
```
工作目录 `<repo>`。**基线：276 passed / 0 failed / 3 ignored。**

---

## 0. 先读这一节：P6 留下的、约束 P7 的东西

这几条都是 P6 里实测出来的，不是从参照读来的。

1. **`offered(wanted, tab)` 已经把侧栏开关和 `has_a_view` 绑在一起了**
   （`handlers/settings.rs`）。P7 只要把 `4` 加进 `TABS_WITH_A_VIEW`，Task scheduler 那一行就会自己
   亮起来——`SidebarConfig` 里 `scheduler` 默认已经是 `true`，不需要改文件、不需要迁移。

2. **侧栏底部那组的坐标是靠图量的，不是靠算的。** `go-settings-page` 现在 526、`go-about-page`
   现在 566。加 scheduler 那一行会把两者**都往上推 40**（底部那组贴底边对齐）。MAPPING 15.9 记了
   为什么这个方向和直觉相反、错了三次的代价。

3. **`is_helper_installed` 早就在等真正的活。** P5b 留下它的时候写的是"由'有无计划任务'驱动"，
   而那个判据恰好就是 `schtasks /Query` 的退出码。`network_model` 和 `handlers/network.rs` 都不用改，
   只要这个字段有值。

4. **`AppWindow` 上已经有 `check_update_clicked`。** P6 为关于页接的，当前会 toast 一句
   "这个 build 里没有更新检查"。**P8 的更新检查要把它换掉，不是再加一个。**

5. **托盘三个开关已经在存盘里**（`TraySettings`，P6 的 `save_interface_settings` 会写）。P7 的托盘
   读的就是这三个字段。

6. **`config::Config` 的 `[system]` 段已经有 `timezone` 字段**（P6 加的，当前恒为空）。
   参照的 `about.rs` 用它做群组图片的地区匹配，我们没有群组图片，所以它继续空着。

---

## 1. 参照的规模（P6 结束时实测）

| 文件 | 行数 | 说明 |
|---|---|---|
| `src/app/scheduler_task/schtasks.rs` | 758 | `schtasks` 的解析器，占 P7 的三分之一 |
| `src/app/scheduler_task/models.rs` | 90 | `RawSchtasksRecord` 等 |
| `src/app/scheduler.rs` | 187 | 计划的增删改 |
| `src/app/task_scheduler.rs` | 151 | 启动时/关机时的钩子 |
| `src/ui/handlers/scheduler.rs` | 468 | UI 侧 |
| `src/ui/views/scheduler_view.slint` | 141 | |
| `src/ui/components/scheduler_card.slint` | 257 | |
| `src/ui/components/dialogs/scheduler_dialog.slint` | 299 | |
| `src/app/single_instance.rs` | 120 | `CreateMutexW` + `FindWindowW` |
| `src/app/autostart.rs` | 144 | 开机自启（注册表 Run 键） |
| `src/app/startup_info.rs` | 89 | |
| `src/app/tray.rs` | 100 | |

`handlers/scheduler.rs` 是单文件不是目录（`scheduler_task/` 才是目录）。

---

## Task 1：`schtasks` 解析器 —— 纯模块，先写测试

**Files:** Create `src/app/scheduler_task/{mod,models,schtasks}.rs`

**Interfaces:**
- `pub struct RawSchtasksRecord`（参照 `models.rs` 逐字段）
- `pub struct SchedulerTask`
- `pub fn parse_query_output(stdout: &str) -> Vec<RawSchtasksRecord>`
- `pub fn to_task(record: &RawSchtasksRecord) -> Option<SchedulerTask>`

**这是整个 P7 唯一真正值得测的东西。** `schtasks /Query /FO CSV /V` 的输出是**本地化的**：
列名是系统语言，日期格式跟着区域设置。参照用固定列号解析——那在非英文 Windows 上是坏的。
我们的做法：按**表头**找列，而不是按位置，并在表头不认识时返回空而不是猜。

- [ ] **Step 1: 写失败的测试**（至少八条）
  英文 CSV 表头能解析；表头顺序变了仍能解析；**德文表头**（`TaskName` → `Auftragsname`）能解析；
  缺列返回空；空输出返回空；`/Query` 失败时（stdout 有 "ERROR"）返回空而不是一条假记录；
  一次 `--query` 多条任务；末行没有换行符不吞掉最后一条。
- [ ] **Step 2: 跑测试确认失败**
- [ ] **Step 3: 实现**（`schtasks` 调用 + 解析）
- [ ] **Step 4: 跑测试确认通过**
- [ ] **Step 5: 挂进 `wsadash.rs` 的 `#[cfg(test)]` 区，跑全套**
- [ ] **Step 6: 提交** `feat(scheduler): schtasks 解析器，按表头不按列号`

## Task 2：单实例 + 开机自启 + 启动信息

**Files:** Create `src/app/{single_instance,autostart,startup_info}.rs`；Modify `app/runner.rs`、`app/mod.rs`

- 单实例：照参照的 `CreateMutexW` + `FindWindowW`。**第二实例要把第一���的窗口带回来**——
  `ShowWindow(SW_RESTORE)` + `SetForegroundWindow`，这是参照的全部意义，否则单实例只是"关掉第二个"。
- 开机自启：注册表 `Run` 键，值指向当前 exe。**要用 `tray_autostart` 这个字段**，P6 已经在存它。
- 启动信息：参照的 `startup_info.rs` 给日志用。
- 这三个都是 GUI/系统侧，但**注册表路径和命令行拼装是纯的**——拆成纯函数 + 薄壳，纯函数挂进 CLI 测试。

- [ ] Step 1: 写失败的测试（注册表 Run 值的拼装、命令行转义、是否该注册的条件）
- [ ] Step 2–4: 红 → 实现 → 绿
- [ ] Step 5: 接进 `runner.rs`（窗口起来**之前**做单实例判断）
- [ ] Step 6: 提交 `feat(app): 单实例、开机自启、启动信息`

## Task 3：托盘

**Files:** Create `src/app/tray.rs`；Modify `app/window.rs`、`handlers/settings.rs`

- 托盘图标用 `tray-icon`（已在 Cargo.toml）。
- **三个开关现在会立刻生效**：`tray_autostart`（Task 2 写注册表）、`tray_start_minimized`（启动时
  不显示窗口）、`tray_close_to_tray`（关窗口而不是退出）。P6 把它们存了但没人读，P7 是它们的读者。
- 语言切换时托盘要重建（参照 `settings.rs:60-68` 干这件事）——`tray-icon` 的 tooltip 含翻译过的文本。

- [ ] Step 1–3: 托盘菜单项的纯逻辑（哪些项、什么顺序、什么条件下置灰）写测试再实现
- [ ] Step 4: 接到窗口的关闭/最小化
- [ ] Step 5: 提交 `feat(tray): 托盘，三个开关开始生效`

## Task 4：计划任务页视图层（照搬）

**Files:** Create `src/ui/views/scheduler_view.slint`、`src/ui/components/scheduler_card.slint`、`src/ui/components/dialogs/scheduler_dialog.slint`

- 逐字搬。图标码位写之前对照 `assets/font/icons.ttf` 的 cmap 验（P6 用的做法）。
- `scheduler_dialog` 接进 `ModalManager`（`z: 200`）。

## Task 5：计划任务页接线 + `is_helper_installed`

- `handlers/scheduler.rs`：增删改查四个动作。
- **给 `is_helper_installed` 找到活**：它的值就是"系统里有没有名为 `wsl-dashboard-scheduler`（参照的
  `TASK_NAME`）的计划任务"，一次 `schtasks /Query` 就知道。
- `tabs.rs` 加 4 → 侧栏那一行自动亮起。
- 截图三张（页签 4 打开、对话框打开、计划列表空态）。

## Task 6：MAPPING §16 + 验收勾选

- §16 记 P7 的语义差异：**WSL 发行版的定时任务 vs. WSA 没有发行版**——参照的计划是
  "在某个 WSL 发行版里跑一条命令"，我们的对应物是"在 WSA 里跑一条 adb 命令"。这是这个页面
  最难说清楚的一处，参照的 cron 校验、`short_filename`、错误文案全部要重新对齐。
- 回写 spec §11 的 11 条清单，**只勾有证据的**。

---

## Review Focus

1. **`schtasks` 的输出是本地化的。** 按列号解析在非英文 Windows 上静默拿到错字段——不报错，只是
   任务名变成空白或者日期变成 `__/__/____`。这是 Task 1 唯一真正要防的事。
2. **`/Query` 的失败输出也是 CSV 形状的**（`"ERROR: ..."` 一行）。当数据解析会得到一条假记录，
   而"没有计划任务"和"查询失败"在 UI 上必须说得清。
3. **单实例不是"关掉第二个"。** 没有 `SW_RESTORE` + `SetForegroundWindow` 的单实例会让读者以为
   双击没反应。
4. **托盘图标要在每次语言切换后重建**，否则 tooltip 是旧语言的——而这正是 P6 加 `AppI18n.version`
   自增要解决的同一类问题的另一半。
5. **`is_helper_installed` 一旦有了真正的判据，网络页的防火墙勾选框就有救了**（P5b 记下的悬空项）。
   判据要能区分"任务不存在"和"任务存在但失败"。

## Self-Review

1. **Spec 覆盖：** §3 页签 4 → Task 5；§10 P7 全部 → Task 1–5；§11 → Task 6。
2. **Step 扫描：** 每个 step 给了确切文件名与测试对象；Task 1 的测试清单列了八条具体场景。
3. **类型一致：** `SchedulerTask` 在 `models.rs` 定义、`schtasks.rs` 产出、`scheduler.rs` 消费、
   `scheduler_model`（纯模块，Task 1 建）测试。
4. **Review Focus：** 五条各指派到 task（1,2→T1；2→T1；3→T2；4→T3；5→T5）。
5. **比例：** 计划约 150 行，其中 60 行是"先读这一节"和参照规模表——那是 P6 实测出来的事实，
   留给下一次省下的时间比计划本身多。