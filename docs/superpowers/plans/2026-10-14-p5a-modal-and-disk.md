# P5a — ModalManager 子集 + 磁盘页 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搬参照的 `modal_manager.slint` 外壳与两个对话框，做"WSA 存储管理"页，并把首页卡片上三个点了没反应��按钮接上线。

**Architecture:** 磁盘页的数据源是 `LocalCache\*.vhdx` 的本地文件（不调 adb），所以这一页在 WSA 停止时照常可用——spec §3 第 6 行把它称为"三态降级的关键"。判断逻辑全部放纯模块 `src/wsa/app/storage.rs`（只碰文件系统，`tempfile` 可测），GUI 侧只剩绘制。对话框按参照的写法照搬：`ModalManager` 不是抽象容器，是每个对话框各写一份"遮罩 + 居中卡片 + 点击取消"，P5a 只搬用得上的那几份。

**Tech Stack:** Rust 2021、Slint 1.17（renderer-skia）、tokio、`tempfile`。

**Spec:** `docs/superpowers/specs/2026-10-10-wsa-dashboard-design.md` §2（第 226 行 `VhdxInfo`）、§3（第 241 行磁盘页、第 246 行 `modal_manager` 照搬、第 248 行 handler 更名）、§5、§10（"WSA 停止时磁盘页可用"）、§11（第 348 行）。

## 为什么 P5 拆成两个计划

spec §10 把网络页和磁盘页放进同一个 P5，但它们是两套独立的数据源、两套独立的数据面：
磁盘页读本地 VHDX 文件，网络页要 `adb forward` / `adb reverse` 和对话框。它们各自能独立
产出可运行、可验收的软件，所以按 writing-plans 的 scope check 拆开。P5a 先做，因为它
**现在就能真机验收**（`userdata.2.vhdx` / `metadata.2.vhdx` 还在磁盘上，WSA 停止正是验收条件）；
P5b（网络页）等 modal_manager 落地后再做，那时对话框机制已经验证过了。

## Global Constraints

- **GPL-3.0-only**，每个新文件带 SPDX 两行头。
- **标识符 / i18n 键名 / 组件名保留参照原名**，只改显示文案；例外要记 `docs/MAPPING.md` §12。
- **可测的判断必须在纯模块**：`handlers/` 只在 GUI 树里，`cargo test --release --bin wsadash` 不编译它。本项目已因此踩坑三次（`data.rs`、`handlers/home.rs`、`handlers/common.rs`）。
- **单 task 一个提交**，先红后绿。
- **不新增参照没有的组件**（P9 硬裁定）：本计划不引入 `DialogHost` 之类的抽象壳。
- 测试命令：`$env:SKIA_BINARIES_URL='file://C:/Users/reader/AppData/Local/Temp/opencode/skia-cache/0.99.0/skia-binaries-{key}.tar.gz'` 然后 `cargo test --release --bin wsadash`。
- 工作目录 `<repo>`。
- `.ps1` 与替身 `.cmd` 一律纯 ASCII + CRLF（`src/wsa/dashboard/ops.rs` 里的 `BATCH` 常量同理）。

## 两处必须先解决的事实问题

1. **spec §2 第 226 行写的 `LocalState\*.vhdx` 在这台机器上不存在。** 真值是
   `LocalCache\`（`LocalCache\userdata.2.vhdx` 2,857 MB、`LocalCache\metadata.2.vhdx` 72 MB）。
   Task 1 按 `LocalCache` 实现并把更正记进 MAPPING——和 spec 第 111 行
   `LocalPortMin`/`LocalPortMax` 那条事实更正同一类。
2. **文件名带版本后缀**（`userdata.2.vhdx`，不是 `userdata.vhdx`）。按前缀 glob，不硬编码全名；
   否则 WSA 自己升一次版本这页就空了。

## 一处故意的交互分歧

参照的 `mount_disk_card` 有三个回调：`unmount_clicked` / `delete_clicked` / `mount_clicked`，
对应"把盘挂进 WSL 后卸载它 / 删掉它"。WSA 侧**没有对应操作**，而最近似的两个是破坏性的：
`userdata.vhdx` 删掉等于抹掉整个子系统，WSA 也没有 unmount API。

**决定：三个回调签名全部保留（Q12），只渲染 `mount_clicked` 一个图标，它的显示文案从
"挂载磁盘"改成"打开目录"（`explorer /select,` 定位到该文件）。另外两个不渲染。**
不渲染的理由是本项目一贯的：渲染出来的控件必须做点诚实的事。已在 MAPPING §12 记录，
P9 逐页对照时这一处会被看到。

## Review Focus

spec 暗示、但没有 task 的测试会自然覆盖的五条，按"最容易咬人"排序。每一行在下面
标了由哪个 task 补测试。

1. **`LocalCache` 目录不存在**（包没装、或 `%LOCALAPPDATA%` 被重定向）时磁盘页必须显示空状态
   而不是报错或白屏 —— Task 1 `a_missing_cache_directory_is_empty_not_an_error`。
2. **VHDX 文件正被 WSA 占用**时读长度可能失败（共享冲突）—— 单个文件读不出来不能连带
   整页失败，Task 1 `one_unreadable_image_does_not_hide_the_others`。
3. **`userdata` 和 `metadata` 都匹配不到**（版本后缀变了，比如 `userdata.3.vhdx` 之外的命名）
   也不能崩 —— Task 1 `no_matching_glob_is_an_empty_list_not_a_error`。
4. **首页卡片的卸载确认框必须真的拦一道**：spec §11 要"安装/卸载回环可用"，而
   `adb uninstall` 不带 `-k` 会清数据，确认框被 Enter 键或点击遮罩外的空白处误关掉，
   就等于没有确认 —— Task 6 `the_delete_confirmation_defaults_to_cancel`。
5. **磁盘页在 WSA 停止时打开**（spec §11 第 348 行）—— 这是本阶段唯一的真机验收项，
   Task 7 用实机截图证明，不得用替身数据代替。

---

### Task 1: `storage` 领域模块 + `wsadash vhdx` 子命令

**Files:**
- Create: `src/wsa/app/storage.rs`
- Modify: `src/wsa/app/mod.rs`（加 `pub mod storage;`）
- Modify: `src/wsadash.rs`（CLI 入口 `wsadash`；已核对存在）

**Interfaces:**
- Produces:
  ```rust
  // src/wsa/app/storage.rs
  pub const PACKAGE_FAMILY: &str =
      "MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe";
  pub const CACHE_DIR_NAME: &str = "LocalCache";

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum StorageRole { UserData, Metadata }

  #[derive(Debug, Clone, PartialEq)]
  pub struct StorageImage {
      pub file_name: String,
      pub path: std::path::PathBuf,
      pub size_bytes: u64,
      pub modified: String,   // "%Y-%m-%d %H:%M"
      pub role: StorageRole,
  }

  /// `None` when `%LOCALAPPDATA%` is not set. Does not touch the filesystem.
  pub fn cache_dir() -> Option<std::path::PathBuf>;

  /// Glob `userdata*.vhdx` and `metadata*.vhdx` under `dir`.
  /// `Ok(vec![])` for a missing `dir`. A file whose length cannot be read is
  /// skipped, not fatal. Sorted UserData first, then Metadata, then by name.
  pub fn images_in(dir: &std::path::Path) -> std::io::Result<Vec<StorageImage>>;
  ```

- [ ] **Step 1: 写失败的测试**

在 `src/wsa/app/storage.rs` 末尾建 `#[cfg(test)] mod tests`，三个测试：

```rust
#[test]
fn a_missing_cache_directory_is_empty_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let gone = dir.path().join("LocalCache");
    assert!(images_in(&gone).unwrap().is_empty());
}

#[test]
fn one_unreadable_image_does_not_hide_the_others() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("userdata.2.vhdx"), b"x").unwrap();
    std::fs::write(dir.path().join("metadata.2.vhdx"), b"y").unwrap();
    // 把其中一个换成目录：读长度会失败
    std::fs::create_dir(dir.path().join("metadata.2.vhdx")).unwrap();
    let images = images_in(dir.path()).unwrap();
    assert_eq!(images.len(), 1, "{images:?}");
    assert_eq!(images[0].role, StorageRole::UserData);
}

#[test]
fn no_matching_glob_is_an_empty_list_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("readme.txt"), b"x").unwrap();
    std::fs::write(dir.path().join("userdata.vhdx.bak"), b"x").unwrap();
    assert!(images_in(dir.path()).unwrap().is_empty());
}

#[test]
fn the_version_suffix_is_not_hardcoded() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("userdata.7.vhdx"), vec![0u8; 10]).unwrap();
    std::fs::write(dir.path().join("metadata.7.vhdx"), vec![0u8; 4]).unwrap();
    let images = images_in(dir.path()).unwrap();
    assert_eq!(images.len(), 2, "{images:?}");
    assert_eq!(images[0].size_bytes, 10);
    assert_eq!(images[1].size_bytes, 4);
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --release --bin wsadash storage::`
Expected: 编译失败，`cannot find function images_in`。

- [ ] **Step 3: 实现 `storage.rs`**

按上面 Interfaces 的签名实现。要点：`images_in` 用 `std::fs::read_dir` 手工过滤
（不引 glob crate），前缀匹配 `userdata` / `metadata` 且后缀恰为 `.vhdx`（大小写不敏感），
`role` 由前缀决定；`size_bytes` 用 `std::fs::metadata(...).len()`，失败就 `continue`；
`modified` 用 `metadata.modified()` 转 `SystemTime` 再算 `%Y-%m-%d %H:%M`
（参照的 `MountedDiskInfo.mount_time` 就是这个格式），取不到就写空串。
排序：`UserData` 在前，然后 `Metadata`，组内按 `file_name`。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test --release --bin wsadash storage::`
Expected: `4 passed`。

- [ ] **Step 5: 加 CLI 子命令 `vhdx`**

先读 `src/wsadash.rs` 确认 CLI 的分派形状（子命令表 + `async fn` + `ExitCode`），
再加一个 `("vhdx", []) => vhdx().await` 分支，函数体：调 `storage::cache_dir()`，
`None` 时打印一行说明并返回 `ExitCode::from(4)`（与 `NotInstalled` 同码）；
`Some(dir)` 时对 `images_in(&dir)?` 每行打印
`{role}\t{size_bytes}\t{modified}\t{file_name}\t{path}`。
参照的 CLI 输出风格见同文件已有的 `list` 分支，照它写。

- [ ] **Step 6: 跑测试 + 真机验一次**

Run: `cargo test --release --bin wsadash` → Expected: `207 passed; 0 failed; 3 ignored`
Run: `.\target\release\wsadash.exe vhdx` → Expected: 两行，`userdata` 在前，`metadata` 在后，
`userdata` 的字节数应是 2,995,977,600 量级（2,857 MB）。**把真实输出抄进 MAPPING §12。**

- [ ] **Step 7: 提交**

```bash
git add src/wsa/app/storage.rs src/wsa/app/mod.rs src/wsadash.rs
git commit -m "feat(wsa): VhdxInfo 领域模块与 wsadash vhdx 子命令"
```

---

### Task 2: 磁盘页视图层（照搬骨架，换数据源）

**Files:**
- Create: `src/ui/components/mount_disk_card.slint`（从 `docs/reference/src/ui/components/mount_disk_card.slint` 搬）
- Create: `src/ui/views/disk_view.slint`（从 `docs/reference/src/ui/views/mount_disk_view.slint` 搬）
- Modify: `src/ui/theme.slint`（加 `StorageImageUI`）

**Interfaces:**
- Consumes: Task 1 的 `StorageImage { file_name, size_bytes, modified, role }`
- Produces:
  ```slint
  // src/ui/theme.slint
  export struct StorageImageUI {
      file_name: string,
      path: string,
      size_text: string,
      modified_text: string,
      role_text: string,
      usage_text: string,     // "" when no device; the adb-side line
      has_usage: bool,
  }
  ```
  ```slint
  // src/ui/components/mount_disk_card.slint
  export component MountDiskCard inherits Rectangle {
      in property <StorageImageUI> image;
      in property <bool> busy: false;
      callback unmount_clicked(string);
      callback delete_clicked(string);
      callback mount_clicked(string);
  }
  ```
  ```slint
  // src/ui/views/disk_view.slint
  export component DiskView inherits Rectangle {
      in property <[StorageImageUI]> images: [];
      callback refresh_list();
      callback open_folder(string);      // 参照的 open_mount_dialog 改名后的唯一动作
  }
  ```

- [ ] **Step 1: 从参照搬 `mount_disk_card.slint` 与 `mount_disk_view.slint`**

参照路径：`docs/reference/src/ui/components/mount_disk_card.slint`（9,029 B）与
`docs/reference/src/ui/views/mount_disk_view.slint`（5,032 B）。逐字搬，保留 `ActionIcon`、
`CustomButton`、`CustomScrollbar`、`Theme` 的用法与 RTL 分支（参照每个按钮都写了两遍
LTR/RTL，别"顺手清理"）。

- [ ] **Step 2: 换数据源**

`MountDiskCard` 里把 `disk.mount_point` / `disk.disk_path` / `disk.mount_time` /
`disk.disk_type` 的引用改成 `image.file_name` / `image.path` / `image.modified_text` /
`image.role_text`；图标按 `image.role_text` 在两个已验证的图标码位里选一个
（参照用 `\u{eda2}` / `\u{e958}`，这两个都在图标字体子集里，可直接沿用）。

**只渲染 `mount_clicked` 一个 `ActionIcon`**，`unmount_clicked` / `delete_clicked`
保留回调声明但不渲染，理由写在文件头的注释里（见上文"一处故意的交互分歧"）。

`DiskView` 的底栏：参照有两个按钮（刷新 + 挂载磁盘）。保留两个，`mount_btn` 那个的
显示文案改为"打开目录"，回调改为 `open_folder(path)`。空状态图标沿用参照的 `\u{e772}`。

- [ ] **Step 3: 加 `StorageImageUI` 到 `theme.slint`**

照文件里 `Distro` / `DeviceTarget` 的写法加，带一行文档注释说明 `usage_text` 为空
代表设备没连上。

- [ ] **Step 4: 编译**

Run: `cargo build --release --bin wsadashboard`
Expected: `Finished`，零 error 零 warning（`DiskView` 还没被 `app.slint` 实例化，
Slint 不会编译未引用的组件——**所以这一步只能证明语法，不能证明它能被画出来**，
Task 5 才验）。

- [ ] **Step 5: 提交**

```bash
git add src/ui/theme.slint src/ui/components/mount_disk_card.slint src/ui/views/disk_view.slint
git commit -m "feat(ui): 磁盘页视图层（WSA 存储管理，照搬卡片骨架换数据源）"
```

---

### Task 3: ModalManager 外壳 + 两个对话框

**Files:**
- Create: `src/ui/components/dialogs/confirmation_dialog.slint`（从参照搬，4,732 B）
- Create: `src/ui/components/dialogs/information_dialog.slint`（从参照搬，7,419 B）
- Create: `src/ui/components/modal_manager.slint`

**Interfaces:**
- Produces:
  ```slint
  // src/ui/components/modal_manager.slint
  export component ModalManager inherits Rectangle {
      in property <bool> show_delete: false;
      in property <bool> show_information: false;
      in-out property <string> delete_target_name: "";
      in-out property <Information> delete_info;
      in property <string> delete_message: "";
      callback confirm_delete();
      callback cancel_delete();
      in-out property <Information> information_info;
      callback close_information();
      callback open_information_path(string);
  }
  ```

- [ ] **Step 1: 搬两个对话框，字段改成 WSA 语义**

`confirmation_dialog.slint` 的 `title_text` / `message` / `dialog_height` / `confirm` /
`cancel` 全部照搬，它本来就是通用的。

`information_dialog.slint` 里那个行式 `RowInfo` 组件（`label` / `value` /
`value_color` / `clickable` / `is_pill` / `clicked`）照搬；但它吃的 `Information`
结构体在参照里是 WSL 发行版语义（`distro_name` / `wsl_version` / `vhdx_path` /
`is_sparse` …），在 WSA 上这些字段永远是空的。**结构体名保留 `Information`（Q12），
字段换成 `AppInfo` 的五项**：`package` / `version_name` / `version_code` /
`apk_size` / `is_system`。记 MAPPING §12。

- [ ] **Step 2: 写 `modal_manager.slint`**

从 `docs/reference/src/ui/components/modal_manager.slint` 搬**结构**：每个对话框一块
`if (show_X): Rectangle { width: 100%; height: 100%; background: #00000080; TouchArea { clicked => cancel }; Rectangle { /* 卡片 */ } }`。
P5a 只写两块：`show_delete`（复用 `ConfirmationDialog`，走参照的删除确认外壳）和
`show_information`（复用 `InformationDialog`）。

文件头注释必须写清：参照这一份有 60 个 `show_*`、22 个对话框，P5a 只搬用得上的两块；
其余的随各自页面所在的阶段加进来，并说明为什么不整搬（大半对话框的数据来自 P6–P8
才有的页面，提前搬进来就要提前造那些数据）。

**不引入 `DialogHost` 之类的抽象壳**——P9 硬裁定：参照没有的组件＝缺陷。

- [ ] **Step 3: 编译**

Run: `cargo build --release --bin wsadashboard`
Expected: 零 error 零 warning（同样，`ModalManager` 要到 Task 5/6 才被实例化）。

- [ ] **Step 4: 提交**

```bash
git add src/ui/components/dialogs src/ui/components/modal_manager.slint
git commit -m "feat(ui): ModalManager 外壳 + 确认框 + 信息框（P5a 子集）"
```

---

### Task 4: `OpenSettings` 命令与 op

**Files:**
- Modify: `src/wsa/command.rs`
- Modify: `src/wsa/dashboard/ops.rs`
- Modify: `src/wsadash.rs`

**Interfaces:**
- Produces:
  ```rust
  // src/wsa/command.rs —— 新变体，名字取自 spec §2 第 223 行
  OpenSettings { aumid: String },

  // src/wsa/dashboard/ops.rs
  pub async fn open_settings(&self) -> Result<(), OpsError>;
  ```

- [ ] **Step 1: 写失败的测试**

`ops.rs` 里加两个测试。`open_settings` 用 `open::that`（spec §2 第 223 行：
`explorer shell:AppsFolder\…!SettingsApp`），而 `open::that` 会真的拉起资源管理器——
所以**不能**对着真包名跑单元测试。测的是"aumid 从哪来"和"没有路径时拒绝"：

```rust
#[tokio::test]
async fn opening_settings_without_a_resolved_package_refuses() {
    // paths 的 settings_aumid 为空时，open_settings 不该去启动 explorer。
    let paths = WsaPaths { settings_aumid: String::new(), ..paths_fixture() };
    let dash = dashboard_for("adb", paths);
    let err = dash.open_settings().await.expect_err("no aumid, no explorer");
    assert!(err.to_string().contains("settings"), "{err}");
}
```

`paths_fixture()` 已在 `ops.rs` 的 `mod tests` 里（返回 `install_location`、
`manifest_path`、`client_exe`、`settings_aumid`、`adb_port` 五个字段齐全的
`WsaPaths`），`WsaPaths` 字段全是 pub，直接用结构体更新语法即可；
`dashboard_for(adb, paths)` 定义在 `src/wsa/dashboard/mod.rs`，同文件里已有测试用它。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --release --bin wsadash opening_settings`
Expected: 编译失败，`no function or associated item named open_settings`。

- [ ] **Step 3: 实现**

`command.rs`：加 `OpenSettings { aumid: String }`，`argv()` 产出
`["shell:AppsFolder\\", aumid]`（参照的 `explorer shell:AppsFolder\…` 实际就是
`ShellExecute` 一个 `shell:AppsFolder\<aumid>` URI，所以 argv 只有一个 URI）。
超时分类按 `ops.rs` 里 `launch_app` 那一档（`open::that` 走 `is_heavy` 之外的档，
照 `LaunchApp` 的现有分类抄）。加一条 `argv()` 的单测，断言 URI 拼接后的确切字符串。

`ops.rs`：`open_settings` 先读 `self.paths.settings_aumid`，空则
`Err(OpsError::Config("WSA settings aumid is unknown; is WSA installed?"))`；
非空则 `open::that(format!("shell:AppsFolder\\{aumid}"))`。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test --release --bin wsadash` → Expected: 全绿。
（新增测试数 = Task 1 的 4 + 本任务的 2 + `argv()` 1 条。）

- [ ] **Step 5: 提交**

```bash
git add src/wsa/command.rs src/wsa/dashboard/ops.rs src/wsadash.rs
git commit -m "feat(wsa): OpenSettings 命令与 op"
```

---

### Task 5: 磁盘页接线，页签打开

**Files:**
- Create: `src/ui/handlers/disk.rs`
- Modify: `src/ui/handlers/mod.rs`
- Modify: `src/ui/app.slint`
- Modify: `src/ui/tabs.rs`

**Interfaces:**
- Consumes: Task 1 `storage::images_in` / `cache_dir`，Task 2 的 `DiskView`，Task 3 的 `ModalManager`
- Produces: 侧栏"磁盘"入口可见，页签 5 有页面，`refresh_list` 与 `open_folder` 有 handler

- [ ] **Step 1: 写 `src/ui/disk_model.rs`（纯模块，测试挂进 CLI）**

把"怎么把 `StorageImage` 变成 `StorageImageUI`"这个判断放进纯模块，
因为 `handlers/` 里的代码 `cargo test --release --bin wsadash` 根本不编译（本项目第三次踩这个坑）。

```rust
// src/ui/disk_model.rs
/// A row as this module produces it: **plain Rust, not the Slint struct**.
/// `StorageImageUI` is generated by `theme.slint` and exists only in the GUI
/// binary, so a `Vec<StorageImageUI>` here would make this file un-mountable
/// into the CLI and its tests would never compile -- the same trap as
/// `handlers/`, one level up. `home_model::AppRow` -> `data::row` -> `Distro`
/// is the shape to copy, and `ui/handlers/disk.rs` does the conversion.
pub struct StorageRow {
    pub file_name: String,
    pub path: String,
    pub size_text: String,
    pub modified_text: String,
    pub role_text: &'static str,
}

pub struct StoragePage {
    pub rows: Vec<StorageRow>,
    /// `""` when there is nothing to add to the empty state.
    pub note: String,
}

/// `2.79 GB` style. Binary units, one decimal, because that is what the
/// Windows Explorer properties dialog shows for the same file and a reader
/// comparing the two should not have to do arithmetic.
pub fn size_text(bytes: u64) -> String;

/// `role` -> the label the card branches its icon and colour off.
pub fn role_text(role: StorageRole) -> &'static str;

/// The list the page renders. A missing cache directory is an empty list plus
/// a note, never an error: this page has to work with WSA stopped, which is
/// exactly the state where the directory may not exist.
pub fn model(dir: Option<&Path>) -> StoragePage;

/// The argument `open::that` gets, for either caller: the card hands over a
/// file (which Explorer should *select*), the bottom bar a directory (which it
/// should *open*).
pub fn explorer_arg_for(path: &Path) -> String;
```

先写四个测试：`size_text` 的进位（`0` / `1023` / `1024` / `2857 * 1024 * 1024`）、
`role_text` 的两个值、`model(None)` 返回空列表且 `note` 非空、
`explorer_arg_for` 对文件与目录给出不同的参数。

在 `src/wsadash.rs` 里加 `#[path = "ui/disk_model.rs"] mod disk_model;`，
照 `home_model` / `refresh` / `tabs` 已有的挂法。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --release --bin wsadash disk_model`
Expected: 编译失败。

- [ ] **Step 3: 实现 `disk_model.rs`，跑到绿**

- [ ] **Step 4: 写 `handlers/disk.rs`**

`refresh_list`：`cache_dir()` → `disk_model::model(...)` → 填 `app.set_disk_images(...)`。
`open_folder(path)`：`open::that(format!("/select,{path}"))`，失败时走已有的
`task_status_toast` 报出来。**两个 handler 都不碰 adb**，这是这一页存在的理由。

在文件头注释里写明：这里没有单元测试，因为本模块命名 `AppWindow`，只有 GUI bin 编译它；
可测的判断已经上移到 `disk_model`。

- [ ] **Step 5: `app.slint` 与 `tabs.rs` 接线**

`app.slint`：`sidebar_mount: true`；加 `in property <[StorageImageUI]> disk_images: [];`；
在内容区按参照的 z 规则插 `DiskView { opacity: selected_tab == 5 ? 1 : 0; x: ... }`，
z 必须是 8（页签 N 的 z 是 N+3，参照自己的编号）；绑定 `refresh_list` / `open_folder`；
顶层挂一个 `ModalManager`。

`tabs.rs`：`TABS_WITH_A_VIEW = &[0, 1, 2, 5]`；同步改
`home_install_and_device_have_a_page_and_the_rest_do_not` 这个测试——它断言
`has_a_view(tab) == tab <= 2`，现在要改成断言"恰好是这四个"，并把测试名从
`home_install_and_device_...` 改成 `home_install_device_and_disk_...`
（这个名字已经因为阶段推进改过两次，所以改成按事实命名）。

- [ ] **Step 6: 跑测试 + 编译**

Run: `cargo test --release --bin wsadash` → Expected: 全绿（比 Task 4 多 3 条）。
Run: `cargo build --release --bin wsadashboard` → Expected: 零 error 零 warning。

- [ ] **Step 7: 提交**

```bash
git add src/ui/disk_model.rs src/ui/handlers src/ui/app.slint src/ui/tabs.rs src/wsadash.rs
git commit -m "feat(ui): 磁盘页接线，页签 5 打开"
```

---

### Task 6: 首页卡片三个按钮接线

**Files:**
- Modify: `src/ui/handlers/home.rs`
- Modify: `src/ui/app.slint`
- Modify: `src/wsa/dashboard/ops.rs`（替身 adb 加 `dumpsys` 已有，检查是否够用）

**Interfaces:**
- Consumes: Task 3 的 `ModalManager`，既有的 `ops::app_info` / `ops::uninstall_apk`，Task 4 的 `ops::open_settings`

- [ ] **Step 1: 写失败的测试**

纯模块侧不需要新判断，所以这一步没有新单元测试——**但有一个必须手动确认的行为**：
删除确认框默认落在取消上。Task 6 的验收靠截图 + 手动点击，不靠测试。**在下面 Step 5
写明怎么点。**

- [ ] **Step 2: `app.slint` 补三个回调声明与透传**

`AppWindow` 加 `callback information_clicked(string);`、
`callback delete_confirmed(string);`、`callback settings_clicked(string);`，
并在 `MainView` 实例化处透传同名回调（参照的 `main_view.slint` 已经在调这三个了，
只是之前没人接）。把 `ModalManager` 的 `show_delete` / `show_information` / `delete_target_name`
绑上。

- [ ] **Step 3: `handlers/home.rs` 接三个动作**

- `information_clicked(pkg)`：`app_info(pkg)` → 填 `information_info` → `show_information = true`。
- `settings_clicked(_)`：`open_settings()`。**这个不需要对话框**，所以是三个里最先变活的。
- `delete_confirmed(pkg)`：`uninstall_apk(pkg)` → 关框 + 触发一次首页刷新。
- `delete_clicked`（确认框弹出）：只置 `show_delete = true` 和 `delete_target_name`。

取消路径**必须**把 `show_delete` 置回 `false` 且不动任何状态。

- [ ] **Step 4: 跑测试 + 编译**

Run: `cargo test --release --bin wsadash` → Expected: 全绿。
Run: `cargo build --release --bin wsadashboard` → Expected: 零 error 零 warning。

- [ ] **Step 5: 提交**

```bash
git add src/ui/handlers/home.rs src/ui/app.slint
git commit -m "feat(ui): 首页卡片的信息/卸载/设置三个按钮接线"
```

---

### Task 7: 真机验收、截图、MAPPING

**Files:**
- Modify: `docs/MAPPING.md`（新增 §12）
- Create: `docs/superpowers/plans/…`（本计划末尾追加完成情况）

- [ ] **Step 1: 真机验磁盘页（WSA 停止状态下）**

此刻这台机器上 WSA 是不可用的（注册还在，但 `InstallLocation` 为空、
`PackageRootFolder` 指向已删除的 `Downloads\Compressed\…`，`IsDevelopmentMode: True`）。
**这正好是 spec §11 第 348 行要的验收条件**："关闭子系统后磁盘页仍可用"。

启动 GUI（`cargo run --release --bin wsadashboard`，或用
`tools/standin-adb/capture.ps1` 截图），点侧栏"磁盘"，
确认：两行（userdata / metadata）、大小不是 0、路径是 `LocalCache` 下的真实路径、
点"刷新"不崩、点"打开目录"能拉起资源管理器并选中该文件。

- [ ] **Step 2: 截图**

用 `tools/standin-adb/capture.ps1`（`-BypassDisplayFilter`，拍完把光标挪出窗口）拍：
`p5a-disk.png`（磁盘页两行）、`p5a-disk-empty.png`（把 `LocalCache` 临时改名后的空状态，
**拍完必须改回来**）、`p5a-info-dialog.png`（信息框）、`p5a-delete-confirm.png`（删除确认框）。

- [ ] **Step 3: 写 MAPPING §12**

必须包含：
1. `LocalState` → `LocalCache` 的事实更正（附 Task 1 Step 6 的真实输出）。
2. 文件名版本后缀（`userdata.2.vhdx`）与 glob 决策。
3. `Information` 结构体字段从 WSL 发行版语义换成 WSA 应用语义。
4. `MountDiskCard` 三个回调只渲染一个的交互分歧与理由。
5. `ModalManager` 只搬两块的理由（22 个对话框的数据源在 P6–P8）。
6. **WSA 当前不可用的证据**（`IsDevelopmentMode: True`、`InstallLocation` 空、
   `PackageRootFolder` 指向已删除目录），以及"因此 P5b 的真机验收同样被挡"。

- [ ] **Step 4: 提交**

```bash
git add docs/MAPPING.md
git commit -m "docs(P5a): MAPPING §12 与真机验收记录"
```

---

## Self-Review

**1. Spec 覆盖：** §2 第 226 行 `VhdxInfo` → Task 1。§3 第 241 行磁盘页（语义重定义、
卡片骨架照搬、只换数据源、不依赖 adb）→ Task 2/5。§3 第 246 行 `modal_manager` 照搬
→ Task 3（带记录在案的子集理由）。§3 第 248 行 handler 更名 `mount_disk.rs` → `disk.rs`
→ Task 5。§10「WSA 停止时磁盘页可用」→ Task 7 Step 1。§11 第 348 行 → Task 7 Step 1。
§11 第 346 行「安装/卸载回环」→ Task 6 让首页的卸载按钮真的能用，但真机那一半仍受
§MAPPING §11.8 已记录的限制（`adb uninstall` 会清数据）——**这一条 P5a 不解决**。
P5a **不覆盖** §10 的「forward/reverse 可用」——那是 P5b。

**2. Step 扫描：** 每个 step 都给出确切签名、确切测试名与断言、或确切命令与期望输出。
写计划时留的两处占位已核实并填实：CLI 入口是 `src/wsadash.rs`（全部 16 个 `src/` 路径
已逐个 `Test-Path` 核对，7 改 9 新），`WsaPaths` 的测试构造用 `ops.rs` 里已有的
`paths_fixture()`，`dashboard_for` 在 `src/wsa/dashboard/mod.rs`。

**3. 类型一致：** `StorageImageUI` 的七个字段在 Task 2 定义、Task 5 的 `disk_model`
填充、Task 5 的 `app.slint` 绑定，名字一致。`StorageRole` 只在 `storage.rs` 定义一次，
`disk_model::role_text` 收它，`theme.slint` 不重复定义（Slint 侧只拿 `role_text: string`）。
`DiskView.open_folder(string)` 在 Task 2 声明、Task 5 绑定，三处同名。

**4. Review Focus：** 五条都指派到了 task（1→T1、2→T1、3→T1、4→T6 手动、
5→T7）。第 4 条是手动验收而非测试，已在 Task 6 Step 1/5 写明原因（`AppWindow` 只有
GUI bin 编译）和做法。

**5. 比例：** 本计划约 380 行，对应的代码大致是：`storage.rs` 120 行、
`disk_model.rs` 90 行、`command.rs`/`ops.rs` 增量 50 行、Slint 侧是参照文件的搬运
（900 行上下，但那部分计划里只描述"改哪几处"而不是抄正文）。代码块只出现在测试与
签名处，没有大段函数体。

---

## 完成情况（2026-10-14）

七个 task 全部落地，工作树干净。**218 passed / 0 failed / 3 ignored**，
GUI `cargo build --release --bin wsadashboard` 零 error 零 warning。

| task | 提交 | 结果 |
|---|---|---|
| 1 `storage` + `wsadash vhdx` | `8b34b90` | 5 条测试；真机在 `probe` 报 exit 4 的同一时刻给出两个镜像的准确字节数 |
| 2 磁盘页视图层 | `4579184` | 卡片与视图照搬换数据源；`DiskView` 在写它的同一个提交里就被 `app.slint` 引用 |
| 3 ModalManager 子集 + 两个对话框 | `0c17fa0` | 与 6 合并提交（拆开会留下"搬了对话框却没人能打开它"的中间态） |
| 4 `OpenSettings` op | `5755014` | **没有加 `WsaCommand` 变体**——见下 |
| 5 磁盘页接线 | `1b01d13` | 8 条 `disk_model` 测试；`p5a-disk.png` 是 WSA 不可用状态下拍的 |
| 6 首页三按钮接线 | `0c17fa0` | 与 3 合并；`p5a-info-dialog.png` / `p5a-delete-confirm.png` |
| 7 MAPPING §12 + 空状态截图 | `105c780` | 八条记录；空状态截图暴露两个缺陷并当场修掉 |

### 与计划不符之处（都是写之前读了源码才发现的）

1. **Task 4 不该加 `WsaCommand::OpenSettings`。** `command.rs` 的模块注释早已写明
   `LaunchApp` / `StartSubsystem` / `StopSubsystem` / `OpenSettings` / `VhdxInfo`
   都不进命令表——它们没有 adb argv、也不经 executor。照 `launch_app` 的形状重做。
2. **Task 5 的 `Vec<StorageImageUI>` 会让纯模块无法挂进 CLI。** `StorageImageUI`
   由 Slint 生成、只存在于 GUI 二进制；点名它 `#[path]` 挂载就失败、测试一次都
   跑不了。改成 `StorageRow`，与 `home_model::AppRow` → `data::row` → `Distro` 同形。
3. **Task 2 的"ModalManager 不引入 `DialogHost`"结论对，但描述错了。** 它不是
   "60 份重复的遮罩"，是纯调度器，遮罩由各对话框自己带。
4. **Task 3 与 Task 6 合并成一个提交。**
5. **Task 7 的空状态截图原本打算手动改名 `LocalCache`，改成单次调用内
   `try/finally` 改名并核对复原**——计划里那句"拍完必须改回来"是个许愿，
   `finally` 才是保证。

### 尚未做（属 P5b 或更后面）

- 网络页（页签 3）、HTTP 代理、`adb forward` / `adb reverse`。
- 磁盘页的 `adb shell df` 占用行与"应用占用 Top N"——卡片上 `usage_text` 字段
  已在，`has_usage` 恒为 false，等 P5b 的 `df` 到位后填。
- ModalManager 的其余二十个对话框，随各自页面所在阶段加。
- **P5b 的真机验收仍被同一个环境问题挡着**（§11.8）：WSA 的注册是开发模式且
  指向已删除的目录，`resolve()` 因此失败。