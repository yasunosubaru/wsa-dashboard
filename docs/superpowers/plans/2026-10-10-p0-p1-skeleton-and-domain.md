# WSA Dashboard P0+P1 — 骨架与领域层 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付仓库骨架 + 可运行的空窗口 + WSA 领域层（`src/wsa/`、`src/adb/`）+ `wsadash` CLI，CLI 能在本机真实 adb 上列出应用、启动、安装、卸载、停止子系统。

**Architecture:** 照 `wsl-dashboard-0.12.0` 的三层结构重写领域层——UI/CLI 只调 `WsaDashboard` 门面，门面分派到 `ops/*`，`ops` 经 `WsaCommandExecutor` 单闸门执行 `WsaCommand`。所有 adb 调用串行过闸（adb server 是全局单例）。`src/ui/` 在本阶段只保留最小窗口，P2+ 才铺开。

**Tech Stack:** Rust edition 2024 / rust-version 1.92，Slint 1.17.1（`backend-winit` + `renderer-skia`），tokio 1.53.1 `rt-multi-thread`。

**Spec:** `docs/superpowers/specs/2026-10-10-wsa-dashboard-design.md`（计划从 spec 论证，两份必须一起读）

## Global Constraints

- package name `wsadashboard`，version `0.1.0`，edition `2024`，`rust-version = "1.92"`
- 依赖与 `Cargo.lock` 一律照搬参照，**不升级版本**（spec Q17）
- release profile 照搬：`opt-level = "z"`、`lto`、`codegen-units = 1`、`strip`、`panic = "abort"`
- Skia 构建必须设 `SKIA_BINARIES_URL` 且 **`file://` 两个斜杠**（三个斜杠会 `strip_prefix("file://")` 得 `/C:/` → os error 123）
- adb 端口从 WSA 包 `AppxManifest.xml` 动态读 `LocalPort`，**解析失败回退 `58526`**（spec Q27）
- 超时分级：**读 10s / 写 45s / 重活 1800s**；信号量 `16` + 后台 `4`；获取超时 `20s`（流式 `10s`）
- 配置目录 `~/.wsadashboard/`
- 标识符与 i18n 键名保留参照原名，只改显示文案（spec Q12）；本阶段不引入 i18n
- GPL-3.0-only，文件头 `SPDX-FileCopyrightText` + `SPDX-License-Identifier` 照搬格式

## Review Focus

spec 描述愿景，不列全它会遇到的输入。以下五类最可能咬到使用者，每类都指定了归属任务与钉住它的测试：

1. **adb 完全不存在**（用户没装 platform-tools）——期望是明确的安装引导，不是 panic 或空窗口。钉在 Task 12（`wsadash probe` 必须返回可读诊断，不崩）。
2. **WSA 已安装但子系统没启动**——期望 `probe` 报"未运行"而非"未安装"，两者报错文案不同。钉在 Task 8。
3. **`pm list packages` 输出格式漂移**（不同 Android 版本前缀/空行差异）——期望解析不 panic 且不静默返回空表。钉在 Task 5。
4. **端口 58526 被别的进程占用或 WSA 改端口**——期望动态解析生效，回退只在 manifest 不可读时发生。钉在 Task 8。
5. **重活超时（`adb install` 1800s 上限）被触发**——期望子进程被杀而非泄漏，且错误可区分于"命令失败"。钉在 Task 11。

---

## File Structure

```
<repo>\
├── Cargo.toml                      # 照搬参照依赖；[[bin]] wsadashboard + wsadash
├── Cargo.lock                      # 照搬参照后按本 crate 名重解析
├── build.rs                        # slint_build
├── build.ps1                       # 封装 SKIA_BINARIES_URL 双斜杠坑
├── LICENSE                         # GPL-3.0
├── .gitignore
├── docs/
│   ├── reference/                  # 精简参照副本（ui/ assets/ Cargo.toml build.rs）
│   └── MAPPING.md                  # distro→app 等标识符语义映射表
├── src/
│   ├── main.rs                     # GUI 入口（本阶段只起空窗口）
│   ├── wsadash.rs                  # CLI 入口
│   ├── adb/
│   │   ├── mod.rs
│   │   ├── decoder.rs              # 纯函数：stdout → 结构
│   │   └── port.rs                 # manifest LocalPort 解析 + 回退
│   └── wsa/
│       ├── mod.rs
│       ├── command.rs              # WsaCommand 枚举 → argv
│       ├── executor.rs             # 单闸门 + 超时分级 + kill_on_drop
│       ├── models.rs               # AppInfo / DeviceInfo / SubsystemStatus …
│       ├── dashboard/
│       │   ├── mod.rs              # WsaDashboard 门面
│       │   ├── op_guard.rs         # WsaOpGuard（create + Drop 反注册）
│       │   └── ops.rs              # 具体操作
│       └── app/
│           ├── constants.rs        # APP_NAME/APP_ID/…（build.rs 用 #[path] 引入）
│           └── wsa_path.rs         # Get-AppxPackage → InstallLocation
└── ui/                             # ← 不存在；.slint 在 src/ui/，见下
```

---

### Task 1: 仓库初始化、参照副本、`MAPPING.md`

**Files:**
- Create: `.gitignore`, `LICENSE`, `docs/MAPPING.md`, `docs/reference/`
- Create: `Cargo.toml`, `Cargo.lock`

**Interfaces:**
- Consumes: 参照源码 `C:\Users\reader\Downloads\Compressed\wsl-dashboard-0.12.0\wsl-dashboard-0.12.0`
- Produces: git 仓库；`docs/reference/` 内可 diff 的只读参照；`MAPPING.md` 的标识符映射约定

- [x] **Step 1: 建目录并 `git init`**

Run: `git -C <repo> init`
Expected: `Initialized empty Git repository`

- [x] **Step 2: 复制精简参照到 `docs/reference/`**

只取参照的 `src/ui/`、`assets/`、`Cargo.toml`、`Cargo.lock`、`build.rs`。不复制 `src/wsl/`、`src/app/` 等领域代码——那是我们要重写的部分。

Run: `robocopy <参照根>\src\ui docs\reference\src\ui /E`（同理 `assets`、单文件 `Cargo.toml`/`Cargo.lock`/`build.rs`）
Expected: `docs/reference/` 存在且 `docs/reference/Cargo.toml` 含 `name = "wsldashboard"`

- [x] **Step 3: 写 `docs/MAPPING.md`**

内容：标识符保留表，逐行列出 `distro → app`、`wsl → wsa`、`Distro → AppInfo`、`usb → device`、`terminal_distro → （隐藏）`、`vscode_distro → （隐藏）` 等映射，并注明"标识符保留原名，只改 i18n 值"（spec Q12）。首页搜索框标注为"本项目新增"（spec Q24）。

- [x] **Step 4: 写 `.gitignore`**

必须包含：`/target`、`*.pdb`、`/.vs`、`*.user`。**不包含** `docs/reference/`——它是被固化的验收基线。

- [x] **Step 5: 写 `LICENSE`**

从参照复制 `LICENSE` 原文（GPL-3.0）。同时写 `REUSE.toml` 头格式约定进 `MAPPING.md`，供后续所有源文件照用。

- [x] **Step 6: 写 `Cargo.toml`**

照搬参照的 `[dependencies]` 全部条目与版本，但：`name = "wsadashboard"`、`version = "0.1.0"`、`description` 换 WSA 语义、`license = "GPL-3.0"`、删掉 `repository`/`homepage`/`documentation`（Q2=A 未建远程）、`[package.metadata] expire = 0`（Q40）。加 `[[bin]] name = "wsadash", path = "src/wsadash.rs"`。删掉参照的 `dhat` 相关（Q9）。

- [x] **Step 7: 复制 `Cargo.lock` 并重解析**

Run: `copy <参照>\Cargo.lock .` 然后 `cargo metadata --format-version 1 > $null`
Expected: lock 文件里出现 `wsadashboard`，其余依赖版本与参照一致

- [x] **Step 8: 首次提交**

```bash
git add -A
git commit -m "chore(P0): init repo, vendor reference, map identifiers"
```

---

### Task 2: Skia 构建通道 + 最小窗口

> **修正（执行时发现）**：参照的 `build.rs:13` 是 `slint_build::compile("src/ui/app.slint")`——`.slint` 位于 **`src/ui/`**，不是顶层 `ui/`。且 `build.rs:4` 用 `#[path = "src/app/constants.rs"] mod constants;`，所以 `src/app/constants.rs` 必须先于 `build.rs` 存在，否则构建失败。

**Files:**
- Create: `build.ps1`, `build.rs`, `src/app/constants.rs`, `src/ui/app.slint`, `src/main.rs`, `src/wsadash.rs`（占位）

**Interfaces:**
- Consumes: Task 1 的 `Cargo.toml`
- Produces: `cargo build --release` 通过；`wsadashboard.exe` 启动出窗口；`build.ps1` 是后续所有构建的唯一入口

- [x] **Step 1: 写 `build.ps1`**

必须做三件事：① 校验 `C:\Users\reader\AppData\Local\Temp\opencode\skia-cache\0.99.0\` 下是否存在 `skia-binaries-*.tar.gz`，缺失则用 `Invoke-WebRequest` 手工下载并报错退出（官方 curl 通道在本机 300s 超时）；② 对每个依赖的 key 计算 `SKIA_BINARIES_URL = "file://C:/Users/reader/AppData/Local/Temp/opencode/skia-cache/0.99.0/skia-binaries-$key.tar.gz"`——**必须是两个斜杠**；③ `cargo build --release @args`。

- [x] **Step 2: 写 `src/app/constants.rs`**（**必须先于 build.rs**）

照搬参照的 `src/app/constants.rs` 的符号，只改值：`APP_NAME = "WSA Dashboard"`、`APP_ID = "wsadashboard"`、`COMPANY_NAME` / `LEGAL_COPYRIGHT` / `PROJECT_REPOSITORY` 按本项目填（无远程仓库时 `PROJECT_REPOSITORY` 留空串）。`build.rs` 通过 `#[path]` 直接 `mod` 引入该文件，符号名不得改动。

- [x] **Step 3: 写 `build.rs`**

本阶段只做三件事（与参照 `build.rs` 同序，逐块后续补回）：① `slint_build::compile("src/ui/app.slint").expect("Slint compilation failed");`；② 读 `Cargo.toml` 的 `[package.metadata] expire`，**`expire = 0` 时按参照 `build.rs:31-40` 转成"现在 + 365 天"**，`cargo:rustc-env=APP_EXPIRE_TIME=…`（Q40）；③ `cargo:rerun-if-changed` 三项。

**本阶段省略**参照的 icon/rc 生成（Task 1 的占位图标与 `embed-resource` 留 P6/P8）、i18n 校验（`verify_translations`，P5 引入 i18n 时整块搬回）。

- [x] **Step 4: 写 `src/ui/app.slint` 最小窗口**

`export component AppWindow inherits Window { title: "WSA_DASHBOARD_WINDOW_UI"; preferred-width: 1000px; preferred-height: 700px; }`。标题字符串照搬参照的命名约定（Q38），后续单实例 `FindWindowW` 靠它。

- [x] **Step 5: 写 `src/main.rs` 起窗口**

`fn main() -> Result<(), slint::PlatformError> { AppWindow::new()?.run() }`。`wsadash.rs` 暂时 `fn main(){}`。

- [x] **Step 6: 构建并验证窗口**

Run: `powershell -File build.ps1`
Expected: `Finished release [optimized] target(s)`，无 `os error 123`、无 `Unable to locate LLVM installation`

- [x] **Step 7: 验证 exe 依赖集（Q14 的判定依据）**

Run: `dumpbin /dependents target\release\wsadashboard.exe`
Expected: 记录非系统 DLL 清单进 `MAPPING.md`。若存在系统外 DLL → 交付形态从"单 exe"改为 zip（Q14=A，不回来等用户）。

- [x] **Step 8: 提交**

```bash
git add -A
git commit -m "feat(P0): skia build channel and empty window"
```

---

### Task 3: 领域模型 `src/wsa/models.rs`

> **修正（执行时发现）**：计划原写 `tests/models.rs` 集成测试，**走不通**——集成测试只能 import **lib target** 的公开 API，而参照是 **bin-only crate**（`src/lib.rs` 不存在、`tests/` 不存在、无 `[lib]`，见 `docs/reference`）。参照自己用的是**内联 `#[cfg(test)] mod tests`**（`api/client.rs:209`、`app/scheduler_task/schtasks.rs:773`、`config/mod.rs:544`、`utils/device_id.rs:16`、`utils/machine.rs:80` 共 5 处）。
>
> **不加 lib target**：加了会把 `crate::app::…` 这类路径全部改写成 `wsadashboard::app::…`，破坏"逐行对照参照"的前提（Q12）。改为内联单测，同时更贴合 Q8 的"单测"。

**Files:**
- Create: `src/wsa/models.rs`, `src/wsa/mod.rs`
- Modify: `src/wsadash.rs`（加 `mod wsa;` —— 领域层暂挂在 CLI bin 下，GUI 在 P2 需要时同样加一行；两个 bin 各自编译一次，代价可忽略）

**Interfaces:**
- Consumes: 无
- Produces: `AppInfo`、`DeviceInfo`、`SubsystemStatus`、`StorageInfo`、`PortRule` —— 后续 decoder/ops/CLI 全部消费这些类型

- [x] **Step 1: 写失败测试（内联在 `models.rs`）**

```rust
#[cfg(test)]
mod tests {
    use super::AppInfo;

    #[test]
    fn display_name_from_last_package_segment() {
        let a = AppInfo { package: "com.topjohnwu.magisk".into(), version_name: "30.6".into(),
                          version_code: 30600, apk_size_bytes: 54389465, is_system: false };
        assert_eq!(a.display_name(), "Magisk");
        assert!(!a.is_system);
    }

    #[test]
    fn display_name_splits_underscores() {
        let a = AppInfo { package: "com.example.my_app".into(), ..blank() };
        assert_eq!(a.display_name(), "My App");
    }
}
```

规则见 **spec §1.7**（`rsplit('.')` 取末段 → 在 `_/.'` 上分词 → 每词首字母大写 → 空格连接）。

- [x] **Step 2: 运行确认失败**

Run: `cargo test --bin wsadash`
Expected: FAIL，`cannot find struct AppInfo in this scope` / `no method named display_name`

- [x] **Step 3: 实现 `models.rs`**

精确签名：`pub struct AppInfo { pub package: String, pub version_name: String, pub version_code: i64, pub apk_size_bytes: u64, pub is_system: bool }`；`impl AppInfo { pub fn display_name(&self) -> String }`。
另定义 `pub enum SubsystemState { NotInstalled, Stopped, Booting, Ready, Unauthorized }`（spec §5 三态降级的细分）、`pub struct SubsystemStatus { pub state: SubsystemState, pub adb_port: u16, pub boot_completed: bool }`、`pub struct DeviceInfo`、`pub struct StorageInfo`、`pub struct PortRule { pub local_port: u16, pub remote: String, pub direction: PortDirection }`，`pub enum PortDirection { Forward, Reverse }`。

- [x] **Step 4: 运行确认通过**

Run: `cargo test --bin wsadash`
Expected: PASS

- [x] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(P1): domain models"
```

---

### Task 4: adb 端口动态解析 `src/adb/port.rs`

**Files:**
- Create: `src/adb/mod.rs`, `src/adb/port.rs`, `tests/fixtures/AppxManifest.xml`
- Modify: `src/wsadash.rs`（加 `mod adb;`）

> 同 Task 3 的修正：**测试内联**（bin-only crate 无法做集成测试）。fixture 放在 `tests/fixtures/` 供 P5–P7 复用，用 `include_str!("../../tests/fixtures/AppxManifest.xml")` 编译期引入，路径错了会立刻编译失败而不是运行时 `None`。

**Interfaces:**
- Consumes: 无
- Produces: `pub fn parse_local_port(manifest_xml: &str) -> Option<u16>`、`pub const FALLBACK_PORT: u16 = 58526` —— executor 与 probe 消费

- [x] **Step 1: 写失败测试（内联在 `port.rs`）**

三条断言：① fixture 含 `LocalPort="58526"` 时返回 `Some(58526)`；② fixture 改成 `LocalPort="60000"` 返回 `Some(60000)`；③ 无 `LocalPort` 属性返回 `None`（**不返回 fallback**——回退由调用方做，这样才可测）。fixture 只读一份，②③ 用字符串改写生成。

- [x] **Step 2: 运行确认失败**

Run: `cargo test --release --bin wsadash`
Expected: FAIL，`no function or associated item named parse_local_port`

- [x] **Step 3: 写 `tests/fixtures/AppxManifest.xml`**

从本机真实 manifest 截取含 `<firewallRules>` 的最小片段，保留 `Executable="WsaClient\WsaClient.exe" LocalPort="58526" Profile="private"`。

- [x] **Step 4: 实现 `parse_local_port`**

`pub fn parse_local_port(manifest_xml: &str) -> Option<u16>`：找 `LocalPort="..."` 属性并解析 u16。用字符串查找而非完整 XML 解析——manifest 是 WSA 自己的产物，格式稳定，引入 XML 依赖不划算。

- [x] **Step 5: 运行确认通过**

Run: `cargo test --release --bin wsadash`
Expected: PASS

- [x] **Step 6: 提交**

```bash
git add -A
git commit -m "feat(P1): dynamic adb port resolution from WSA manifest"
```

---

### Task 5: decoder — `pm list packages` 解析

**Files:**
- Create: `src/adb/decoder.rs`（测试**内联**在该文件的 `#[cfg(test)] mod tests` 里；无 `tests/*.rs`）

**Interfaces:**
- Consumes: `AppInfo`（Task 3）
- Produces: `pub fn parse_package_list(stdout: &str, filter_system: bool) -> Vec<String>` —— ops 层消费

- [x] **Step 1: 写失败测试（内联 `src/adb/decoder.rs`）**

覆盖四种输入：① 正常 `package:com.android.settings\npackage:com.topjohnwu.magisk\n` → 2 项；② 带 `-3` 过滤时系统包被排除；③ 空输入返回空 `Vec`（**不是 panic**）；④ 含 `package:android.auto_generated_rro_vendor__` 这类下划线包名不被误删。

关键断言：`parse_package_list("package:a\n\npackage:b\n", false) == vec!["a", "b"]`（空行被跳过）。

- [x] **Step 2: 运行确认失败**

Run: `cargo test --release --bin wsadash`
Expected: FAIL，`unresolved import super::parse_package_list`

- [x] **Step 3: 实现 `parse_package_list`**

`pub fn parse_package_list(stdout: &str, filter_system: bool) -> Vec<String>`：逐行 trim，跳过空行，剥 `package:` 前缀。`filter_system` 时排除 `is_system_package(name)` 判定为系统的包——`is_system_package` 判据：包名以 `com.android.`、`android.`、`com.google.android.`、`com.amazon.` 开头，或 `com.android.*.auto_generated_rro_vendor__` 后缀。

- [x] **Step 4: 运行确认通过**

Run: `cargo test --release --bin wsadash`
Expected: PASS

- [x] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(P1): pm list packages decoder"
```

---

### Task 6: decoder — `dumpsys package` 解析

**Files:**
- Modify: `src/adb/decoder.rs`（测试**内联**在该文件的 `#[cfg(test)] mod tests` 里）
- Create: `tests/fixtures/dumpsys_package.txt`（仅 fixture，无 `tests/*.rs`）

**Interfaces:**
- Consumes: `AppInfo`（Task 3）
- Produces: `pub fn parse_package_info(stdout: &str) -> Option<AppInfo>` —— 首页应用卡片与 CLI `info` 消费

- [x] **Step 1: 采集真实 fixture**

Run: `adb shell dumpsys package com.topjohnwu.magisk > tests/fixtures/dumpsys_package.txt`
Expected: 文件非空，含 `versionName=` 与 `firstInstallTime=`

- [x] **Step 2: 写失败测试（内联 `src/adb/decoder.rs`）**

断言：从 fixture 解析出的 `AppInfo.package == "com.topjohnwu.magisk"`、`version_name` 非空、`version_code > 0`、`is_system == false`。另测无 `versionName=` 的输入返回 `None`（不 panic、不造默认值）。

- [x] **Step 3: 运行确认失败**

Run: `cargo test --release --bin wsadash`
Expected: FAIL，`unresolved import super::parse_package_info`

- [x] **Step 4: 实现 `parse_package_info`**

`pub fn parse_package_info(stdout: &str) -> Option<AppInfo>`：定位 `Packages:` 段下与目标包匹配的块，取 `versionName=`、`versionCode=`。`apk_size_bytes` 在本函数恒为 0——真实大小走 `adb shell pm path` + `stat`，属 ops 层职责，不让 decoder 依赖两次命令。

- [x] **Step 5: 运行确认通过**

Run: `cargo test --release --bin wsadash`
Expected: PASS

- [x] **Step 6: 提交**

```bash
git add -A
git commit -m "feat(P1): dumpsys package decoder"
```

---

### Task 7: decoder — `getprop` 批量解析

**Files:**
- Modify: `src/adb/decoder.rs`（测试**内联**在该文件的 `#[cfg(test)] mod tests` 里；无 `tests/*.rs`）

**Interfaces:**
- Consumes: `SubsystemStatus`、`SubsystemState`（Task 3）
- Produces: `pub fn parse_props(stdout: &str) -> HashMap<String,String>`、`pub fn subsystem_status(props: &HashMap<String,String>, adb_online: bool, port: u16) -> SubsystemStatus` —— probe 消费

- [x] **Step 1: 写失败测试（内联 `src/adb/decoder.rs`）**

两条链路断言：① `parse_props("[ro.build.type]: [user]\n[ro.debuggable]: [0]\n")` 得 2 键；② `subsystem_status` 状态机——`adb_online=false` → `Stopped`；`adb_online=true` 但 `sys.boot_completed != "1"` → `Booting`；两者齐备 → `Ready`。

**这条测试就是 Review Focus 第 2 条**（"WSA 已安装但子系统没启动"必须报"未运行"而非"未安装"）。

- [x] **Step 2: 运行确认失败**

Run: `cargo test --release --bin wsadash`
Expected: FAIL，`unresolved import super::{parse_props, subsystem_status}`

- [x] **Step 3: 实现两个函数**

签名照上。`parse_props` 逐行匹配 `[key]: [value]`。`subsystem_status` 只做状态推导，不发命令。

- [x] **Step 4: 运行确认通过**

Run: `cargo test --release --bin wsadash`
Expected: PASS

- [x] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(P1): getprop decoder and subsystem state machine"
```

---

### Task 8: 定位 WSA 包 `src/wsa/app/wsa_path.rs`

**Files:**
- Create: `src/wsa/app/mod.rs`, `src/wsa/app/wsa_path.rs`, `tests/fixtures/AppxManifest_full.xml`
- 测试**内联**在 `src/wsa/app/wsa_path.rs` 的 `#[cfg(test)] mod tests` 里（无 `tests/*.rs`）

**Interfaces:**
- Consumes: `parse_local_port`（Task 4）
- Produces: `pub struct WsaPaths { pub install_location: PathBuf, pub manifest_path: PathBuf, pub client_exe: PathBuf, pub settings_aumid: String, pub adb_port: u16 }`、`pub fn resolve() -> Result<WsaPaths, WsaPathError>` —— executor 与 CLI 全部消费

- [ ] **Step 1: 采集真实 manifest fixture**

Run: `Copy-Item "$((Get-AppxPackage MicrosoftCorporationII.WindowsSubsystemForAndroid).InstallLocation)\AppxManifest.xml" tests\fixtures\AppxManifest_full.xml`
Expected: 文件存在且含 `Package Name="MicrosoftCorporationII.WindowsSubsystemForAndroid"`

- [ ] **Step 2: 写失败测试（内联 `src/wsa/app/wsa_path.rs`）**

用 fixture 解析出：`install_location` 非空、`adb_port == 58526`、`client_exe` 以 `WsaClient\WsaClient.exe` 结尾、`settings_aumid` 含 `!SettingsApp`。另测 manifest 读不到时返回 `Err(WsaPathError::NotInstalled)`。

**这条测试就是 Review Focus 第 4 条**：动态端口解析生效，回退只在 manifest 不可读时发生。

- [ ] **Step 3: 运行确认失败**

Run: `cargo test --release --bin wsadash`
Expected: FAIL

- [ ] **Step 4: 实现 `wsa_path.rs`**

签名照上。`resolve()` 走 `Get-AppxPackage`（用 `powershell -NoProfile -Command` 子进程拿 `InstallLocation`，不用 PowerShell SDK 依赖——避免引入 Windows 侧 crate 的版本风险）。`adb_port` = `parse_local_port(&manifest)` 得到的值，`None` 时用 `FALLBACK_PORT`。`settings_aumid` 由 manifest 的 `Identity Name` + `Application Id="SettingsApp"` 拼成 `{name}_8wekyb3d8bbwe!SettingsApp`——publisher hash 常量 `8wekyb3d8bbwe` 照本机实测写死（微软商店应用的 publisher hash 固定）。

- [ ] **Step 5: 运行确认通过**

Run: `cargo test --release --bin wsadash`
Expected: PASS

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "feat(P1): resolve WSA package location, manifest and adb port"
```

---

### Task 9: `WsaCommand` 枚举 `src/wsa/command.rs`

**Files:**
- Create: `src/wsa/command.rs`（测试**内联**在该文件的 `#[cfg(test)] mod tests` 里；无 `tests/*.rs`）

**Interfaces:**
- Consumes: `FALLBACK_PORT`（Task 4）
- Produces: `pub enum WsaCommand`、`impl WsaCommand { pub fn argv(&self) -> Vec<String>; pub fn is_write(&self) -> bool; pub fn is_heavy(&self) -> bool; pub fn display(&self) -> String }` —— executor 消费

- [ ] **Step 1: 写失败测试（内联 `src/wsa/command.rs`）**

断言枚举的三条分类（决定超时分级）：
```rust
assert!(!WsaCommand::ListPackages.is_write());
assert!(WsaCommand::InstallApk { path: "a.apk".into() }.is_heavy());
assert!(!WsaCommand::LaunchApp { package: "com.x".into() }.is_write());
assert!(WsaCommand::StopSubsystem.is_write());
```
另断言 `LaunchApp` 的 `argv()` 是 `["shell","am","start","-W","-n","com.x"]` 形状——**注意：这里故意用 `am start` 而非 `wsa://`**，因为 `wsa://` 走 `ShellExecute` 协议激活而非 `adb` 子进程，属 Task 11 的职责，不在 `argv()` 里。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --release --bin wsadash`
Expected: FAIL

- [ ] **Step 3: 实现 `command.rs`**

枚举变体（spec §2.3）：`ConnectAdb { port }`、`ListAdbDevices`、`ProbeReady`、`GetProps`、`ListPackages { include_system }`、`PackageInfo { package }`、`AmForceStop { package }`、`InstallApk { path }`、`UninstallApk { package }`、`ForwardAdd/ForwardRemove/ReverseAdd/ReverseRemove`、`Shell { args }`。

`argv()` 返回 adb 后的参数（不含 `adb` 本身，executor 拼 exe 路径）。分类规则：只读命令（`List*`/`Probe*`/`GetProps`/`PackageInfo`/`* --list`）→ `is_write=false`；`InstallApk`/`UninstallApk` → `is_heavy=true`；其余 → `is_write=true`。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test --release --bin wsadash`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(P1): WsaCommand enum with timeout classification"
```

---

### Task 10: 执行闸门 `src/wsa/executor.rs`

**Files:**
- Create: `src/wsa/executor.rs`（测试**内联**在该文件的 `#[cfg(test)] mod tests` 里；无 `tests/*.rs`）

**Interfaces:**
- Consumes: `WsaCommand`（Task 9）
- Produces: `pub struct WsaCommandExecutor { adb_path: PathBuf, heavy_lock: Arc<Mutex<()>>, read_sem: Arc<Semaphore>, write_sem: Arc<Semaphore> }`、`impl WsaCommandExecutor { pub async fn run(&self, cmd: &WsaCommand) -> Result<CommandOutput, ExecError>`、`pub struct CommandOutput { pub stdout: String, pub stderr: String, pub exit_code: i32 }`、`pub enum ExecError { TimedOut, NotFound, Failed { code: i32, stderr: String } }`

- [ ] **Step 1: 写失败测试（内联 `src/wsa/executor.rs`，用 `#[tokio::test]`）**

用 `WsaCommand::Shell { args }` 指向一个会 sleep 的假命令验证超时与杀进程：
```rust
let out = executor.run(&WsaCommand::Shell{args: vec!["sleep".into(),"9999".into()]}).await;
assert!(matches!(out, Err(ExecError::TimedOut)));
```
另测：`ExecError::NotFound` 在 adb 路径不存在时触发（**Review Focus 第 1 条**：adb 缺失要给可读诊断，不 panic）。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --release --bin wsadash`
Expected: FAIL

- [ ] **Step 3: 实现 `executor.rs`**

关键取值（照 spec §2.2）：超时读 `10s`/写 `45s`/重活 `1800s`；信号量读 `Semaphore::new(16)`、写 `Semaphore::new(4)`；信号量获取超时 `20s`（流式 `10s`）；取消靠 `kill_on_drop(true)`。
`heavy_lock` 只在 `is_heavy()` 时持有。命令用 `tokio::process::Command`，`.kill_on_drop(true)`，`.output()` 外套 `tokio::time::timeout`。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test --release --bin wsadash`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(P1): command executor with timeout tiers and kill_on_drop"
```

---

### Task 11: 具体操作 `src/wsa/dashboard/ops.rs`

**Files:**
- Create: `src/wsa/dashboard/mod.rs`, `src/wsa/dashboard/ops.rs`

**Interfaces:**
- Consumes: `WsaCommandExecutor`（Task 10）、decoders（Task 5–7）、`WsaPaths`（Task 8）
- Produces: `impl WsaDashboard { pub async fn probe(&self) -> Result<SubsystemStatus>; pub async fn list_apps(&self, include_system: bool) -> Result<Vec<AppInfo>>; pub async fn app_info(&self, package: &str) -> Result<AppInfo>; pub async fn launch_app(&self, package: &str) -> Result<()>; pub async fn stop_app(&self, package: &str) -> Result<()>; pub async fn install_apk(&self, path: &Path) -> Result<()>; pub async fn uninstall_apk(&self, package: &str) -> Result<()>; pub async fn start_subsystem(&self) -> Result<()>; pub async fn stop_subsystem(&self) -> Result<()> }` —— CLI 与 P2+ UI 消费

- [ ] **Step 1: 实现 `launch_app` 用 `wsa://`，不用 `WsaClient /launch`**

这是 spec §1.2 的核心结论。`launch_app` 必须走 `ShellExecuteW("wsa://{package}")`，**不是** spawn `WsaClient.exe /launch`——后者已被 7 种机制实测证伪（exit 0 零动作）。用 `open` crate（已在依赖里）：`open::that(format!("wsa://{package}"))`。

`start_subsystem` 同理：`open::that("wsa://com.android.settings")`（spec §1.2 实测 12s 冷启动，系统包为兜底，Q2 已定）。

- [ ] **Step 2: 实现 `stop_subsystem` 杀 `WsaClient` 进程**

spec §1.3 实测：`/terminate` 零动作，杀进程才让 `vmmemWSA` 干净下线。实现用 `taskkill /F /IM WsaClient.exe`，随后轮询 `vmmemWSA` 消失（最多 30s）。

- [ ] **Step 3: 实现 `probe` 三段判定**

顺序：① `ListAdbDevices` 无 `device` 行 → `Stopped`；② 有设备但 `sys.boot_completed != "1"` → `Booting`；③ 否则 `Ready`。`Unauthorized` 由 `adb devices` 输出含 `unauthorized` 判定（spec §1.4 要求容忍四态）。

**这条覆盖 Review Focus 第 2 条**（未运行 vs 未安装必须可区分）。

- [ ] **Step 4: 实现其余操作**

`install_apk`/`uninstall_apk` 走 `InstallApk`/`UninstallApk` 命令后检查 exit code 与 stdout 含 `Success`。`list_apps` 走 `ListPackages` + 逐包 `PackageInfo`（并发受 executor 信号量约束）。

- [ ] **Step 5: 手工验证（不写单测——依赖实机 adb，Q8 已定）**

Run: `cargo run --bin wsadash -- probe`（CLI 在 Task 13 才有，此处先用 `cargo test` 里的 `#[ignore]` 集成测试占位）

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "feat(P1): dashboard ops - launch via wsa://, stop by killing WsaClient"
```

---

### Task 12: 门面与操作锁 `src/wsa/dashboard/mod.rs` + `op_guard.rs`

**Files:**
- Create: `src/wsa/dashboard/op_guard.rs`

**Interfaces:**
- Consumes: `ops.rs`（Task 11）
- Produces: `pub struct WsaDashboard`、`pub struct WsaOpGuard<'a>`，`WsaOpGuard::create(dashboard: &'a WsaDashboard, op: &'static str) -> WsaOpGuard<'a>`，`pub fn get_active_op(&self) -> Option<&'static str>`；`Drop` 时反注册

- [ ] **Step 1: 实现 `WsaOpGuard`**

照参照 `src/wsl/dashboard/operation_guard.rs`：`create` 注册 op 名并存入 dashboard 的 `active_op: Mutex<Option<&'static str>>`；`Drop` 清空。op 名取值 `Starting` / `Stopping` / `Launching` / `Installing` / `Uninstalling`（spec §2.2）。

- [ ] **Step 2: 让每个改变状态的 op 持有 guard**

`launch_app`/`stop_app`/`install_apk`/`uninstall_apk`/`start_subsystem`/`stop_subsystem` 开头 `let _guard = WsaOpGuard::create(self, "Launching");`。`heavy_op_lock` 只在 `install`/`uninstall` 持有。

- [ ] **Step 3: 写失败测试（内联 `src/wsa/dashboard/op_guard.rs`）**

断言：① 创建后 `get_active_op() == Some("Launching")`；② guard drop 后回到 `None`；③ 嵌套创建时第二个返回 `Err(OpBusy)`（不覆盖第一个）。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test --release --bin wsadash`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(P1): WsaOpGuard with Drop-based unregistration"
```

---

### Task 13: `wsadash` CLI 与端到端验证

**Files:**
- Modify: `src/wsadash.rs`

**Interfaces:**
- Consumes: `WsaDashboard`（Task 12）、`SubsystemStatus`（Task 3）
- Produces: 可执行的 `wsadash.exe`，子命令 `probe` / `list` / `info` / `launch` / `stop-app` / `install` / `uninstall` / `start` / `stop`（Q19）

- [ ] **Step 1: 实现子命令分派**

不引 `clap`（参照未用，保持依赖一致）；手写 `args[0]` 匹配。`probe` 无子命令时打印 `SubsystemStatus` 摘要并返回退出码：`Ready`→0、`Booting`→2、`Stopped`→3、`NotInstalled`→4。

**这条覆盖 Review Focus 第 1 条**：adb 缺失时 `probe` 打印"未找到 adb，可用 `winget install Microsoft.PlatformTools` 安装"并返回 5，不 panic。

- [ ] **Step 2: 真机端到端验证**

Run（按顺序）:
```
wsadash probe                       # 期望 Ready, 退出码 0
wsadash list                        # 期望列出第三方包，含 com.topjohnwu.magisk
wsadash info com.topjohnwu.magisk   # 期望 version_name 非空
wsadash stop com.android.settings   # 期望成功
wsadash launch com.android.settings # 期望 2s 内起来
wsadash stop                        # 期望 vmmemWSA 下线
wsadash start                       # 期望 12-17s 内 Ready
```
Expected: 每条退出码符合预期；`stop` 后 `adb devices` 无条目；`start` 后 `sys.boot_completed=1`。

- [ ] **Step 3: 验证 Review Focus 第 5 条（重活超时杀进程）**

临时把 heavy 超时改为 2s，跑 `wsadash install <大apk>`，确认返回 `TimedOut` 且 `Get-Process adb` 无残留子进程。

- [ ] **Step 4: 验证 Review Focus 第 3 条（解析不静默返回空表）**

`wsadash list` 在 adb 返回非预期格式时必须打印 stderr 警告，不能静默输出空列表。

- [ ] **Step 5: 提交**

```bash
git add -A
git commit -m "feat(P1): wsadash CLI with end-to-end verified commands"
```

---

## Self-Review

**1. Spec coverage (P0+P1 范围内):** §1.1–1.4 实测事实 → Task 4/5/6/7/8/11 全覆盖；§2.2 并发原语 → Task 10/12；§2.3 命令表 → Task 9/11；§8 构建坑 → Task 2。**缺口**：§1.2 的"冷启动 17s 需有进度语义"属 P2 UI 范围，本计划不涉及，已在 spec §10 P2 项下，非遗漏。

**2. Step scan:** 每步只做一件事。Task 11 Step 1–4 有意不拆成独立 task——它们共享 `WsaDashboard` 结构体，拆开会让每个 task 都重复定义同一 struct，反而更长。

**3. Type consistency:** `AppInfo` 在 Task 3 定义、Task 5/6/11 消费，字段名一致（`package`/`version_name`/`version_code`/`apk_size_bytes`/`is_system`）。`WsaCommandExecutor::run` 返回 `CommandOutput`，Task 11 消费同名类型。`WsaPaths` 在 Task 8 产出、Task 11 消费。已核对。

**4. Review Focus:** 五条全部有归属任务与测试——①Task 13 Step 1、②Task 7 Step 1 + Task 11 Step 3、③Task 13 Step 4、④Task 8 Step 2、⑤Task 10 Step 1 + Task 13 Step 3。非空。

**5. Proportion:** 计划 13 个 task，代码块只出现在测试断言与枚举形状处，函数体一律留给实现者。与 spec（约 21 KB）相比计划更短，符合"计划不应长于它描述的代码"。




