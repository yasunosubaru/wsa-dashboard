# P6 — 设置页、关于页、真实语言包 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 页签 7（设置）与页签 8（关于）各有一个页面并真的保存东西；i18n 从一张手写英文表换成 50 个真实语言包的加载器。

**Architecture:** 领域层新增 `src/config/`（settings 模型 + TOML 读写 + 路径解析，纯逻辑，可测）；UI 层新增 `views/settings/{mod,sections}.slint`、`views/about_view.slint`、`components/language_data.slint`、`components/dialogs/language_select_dialog.slint`，全部照搬参照；handler 新增 `handlers/settings.rs`、`handlers/about.rs`。i18n 模块的**函数签名一个不改**（P1 就固定好了），只换实现。

**Tech Stack:** Rust 2021、Slint 1.17、`toml`、`rust-embed`、`once_cell`。

**Spec:** `docs/superpowers/specs/2026-10-10-wsa-dashboard-design.md` §3 九页签表（索引 7 设置 / 8 关于）、§10 阶段表（P6）、§11 验收清单。

## Global Constraints

- **GPL-3.0-only**，每个新文件带 SPDX 两行头。
- **标识符 / i18n 键名保留参照原名**，只改显示文案；例外记 `docs/MAPPING.md`。
- **可测的判断必须在纯模块**（`config/` 和 `i18n/` 的纯函数都能被 CLI 的 `#[path]` 挂进去跑测试）。
- **不新增参照没有的组件**（P9 硬裁定）。
- 每 task 一个提交，先红后绿。测试命令：
  `$env:SKIA_BINARIES_URL='file://C:/Users/reader/AppData/Local/Temp/opencode/skia-cache/0.99.0/skia-binaries-{key}.tar.gz'`
  然后 `cargo test --release --bin wsadash`。
- 工作目录 `<repo>`。

## 这个 phase 特有的一个判断

**语言包不能无条件覆盖英文。** 参照的做法是"英文打底、目标语言覆盖"，因为它的英文就是英文。我们的英文表里有 MAPPING 记下的一批**故意改写过的措辞**（`operation.deleting` 是 "Uninstalling ..." 而不是 "Deleting ..."，`sidebar.mount` 是 "Storage"，`network.proxy_error_port` 的范围是 65535 而不是 65534）。语言包是**照着参照的英文翻译的**，把它们盖在这些键上，得到的译文描述的是另一个软件。

所以：`OURS` 列一张键名清单（措辞是我们的、语言包别碰），`lookup` 先看目标语言有没有这个键、有的话再看它是不是我们的键——是就用我们的英文。

**这张表必须自己验**：一条测试拿参照的 `en.toml` 和我们的英文表逐键比对，凡是我们改过措辞的键必须都在 `OURS` 里、且凡是没改的键必须都不在。多一个键是错（白白放弃一个能翻译的键），少一个键是更糟的错（我们自己的句子被盖成别的东西）。

## Review Focus

1. **清空语言包时退回自己的英文**，而不是显示键名——`t()` 的缺键路径是"显示键名并 warn"，那在"这个语言没翻译这个键"时会变成满屏 `network.no_rules`。
2. **`normalize_language_code` 要把 `zh-CN` 归到 `zh`**，否则系统语言是中文的用户会看到英文，而 50 个包里有 `zh-CN.toml` 和 `zh-TW.toml` 两个中文——这是两个不同的选择，不是同一个。
3. **配置文件写坏了不能让程序起不来**：读失败退回默认值并 `warn`，写失败要告诉用户。
4. **关于页的版本号是唯一不能编造的字段**：从 `Cargo.toml` 的 `version` 取（编译期 `env!("CARGO_PKG_VERSION")`），不是从某个 toml 里读。
5. **`available_languages()` 的顺序就是下拉框的顺序**，所以它得是稳定的——按文件名排序，不按文件系统枚举顺序。

---

### Task 1: 语言包 + 加载器

**Files:**
- Create: `assets/i18n/*.toml`（49 个语言包照搬参照，`en.toml` 由脚本从我们的表生成）
- Modify: `src/i18n/mod.rs`（`lookup` 换成加载器；函数签名不动）
- Modify: `src/ui/language_data.slint`（新建，照搬参照）

**Interfaces:**
- 新增 `pub fn load_resources(lang_code: &str)`
- 新增 `pub fn normalize_language_code(code: &str) -> String`
- `t` / `tr` / `is_rtl` / `current_lang` / `available_languages` 签名**不变**

- [ ] **Step 1: 写失败的测试**（五条）
  `normalize_language_code` 的归并（含 `zh-CN`→`zh`、`zh-TW`→`zh-TW`、未知→`en`）、`is_rtl` 的五个语言、`available_languages` 有 50 项且以 `en` 打头（升序）、`OURS` 与参照英文表的差集完全一致、`t()` 在非英语下对一个未翻译键退回英文。
- [ ] **Step 2: 跑测试确认失败**
- [ ] **Step 3: 复制语言包 + 生成 `en.toml`**
- [ ] **Step 4: 实现加载器**（`rust-embed` 嵌 `assets/i18n`，`toml` 解析并拍平 `section.key`）
- [ ] **Step 5: 跑测试确认通过**
- [ ] **Step 6: 提交** `feat(i18n): 50 个语言包 + 加载器`

---

### Task 2: `src/config/` —— 设置的存储

**Files:** Create `src/config/mod.rs`、`src/config/models.rs`、`src/config/paths.rs`

**Interfaces:**
- `pub struct Settings`（serde）
- `impl Settings { pub fn defaults() -> Self }`
- `pub fn config_path() -> Option<PathBuf>` / `pub fn load() -> Settings` / `pub fn save(&Settings) -> Result<(), String>`
- `pub fn parse_port(text: &str) -> Option<u16>`、`pub fn clamp_log_days(days: i64) -> i64`

- [ ] **Step 1: 写失败的测试**（八条）
  默认值全对、写后再读回一致、**文件损坏时 `load` 退回默认值而不是报错**（这是最容易出事的一条）、`normalize_language_code` 的空值进 `Settings::defaults` 不 panic、port 解析拒绝 0 / 65536 / 非数字、`clamp_log_days` 的上下界。
- [ ] **Step 2–4:** 红 → 实现 → 绿
- [ ] **Step 5: 提交** `feat(config): 设置模型与 TOML 读写`

---

### Task 3: 设置页视图层（照搬）

**Files:** Create `src/ui/views/settings/{mod,sections}.slint`、`src/ui/components/dialogs/language_select_dialog.slint`

- [ ] **Step 1–3:** 逐字搬，改动只在数据源；图标码位逐个对照 `icons.ttf` 的 `cmap` 验。
- [ ] **Step 4:** 在 `app.slint` 实例化（和前几个 view 同一个理由）
- [ ] **Step 5:** 提交 `feat(ui): 设置页视图层`

---

### Task 4: 设置页接线

**Files:** Create `src/ui/handlers/settings.rs`；Modify `handlers/mod.rs`、`handlers/common.rs`、`src/ui/tabs.rs`、`src/ui/app.slint`

- [ ] **Step 1:** 写 handler：读配置填页、保存写回、语言切换后 `load_resources` + 重填窗口、侧栏九项开关落盘
- [ ] **Step 2:** `tabs.rs` 加 7，`settings_is_offered_but_not_yet_available` 改成它该断言的东西
- [ ] **Step 3:** `app.slint` 绑数据与回调
- [ ] **Step 4:** 跑测试 + 编译（GUI 零 warning）
- [ ] **Step 5:** 提交 `feat(ui): 设置页接线，页签 7 打开`

---

### Task 5: 关于页

**Files:** Create `src/ui/views/about_view.slint`（662 行照搬）、`src/ui/handlers/about.rs`

- [ ] **Step 1–5:** 搬 + 接线 + 截图 + 提交 `feat(ui): 关于页，页签 8 打开`
- 版本号走 `env!("CARGO_PKG_VERSION")`，参照那些指向它自己仓库的链接改成指向本项目

---

### Task 6: MAPPING §15 + 截图

- [ ] **Step 1:** 写 MAPPING §15（P6 的语义差异、`OURS` 清单怎么来的、配置损坏时的行为）
- [ ] **Step 2:** 截图两张（设置页、关于页），核对九个侧栏开关真的落盘（改一个 → 重启 → 看）
- [ ] **Step 3:** 提交 `docs: MAPPING 15 —— P6`

## Self-Review

1. **Spec 覆盖：** §3 页签 7/8 → Task 3/4/5；§10 P6 → 全部；§11 验收里与设置页相关的条目 → Task 4/6。
2. **Step 扫描：** 每个 step 给确切文件名/测试名/命令。`OURS` 清单的具体内容在 Task 1 的测试里由脚本生成，不手抄。
3. **类型一致：** `Settings` 的字段名在模型、`toml`、handler、`app.slint` 四处一致，参照的 `settings_*` 属性名一个不改。
4. **Review Focus：** 五条各指派到 task（1/2→T1，3→T2，4→T5，5→T1）。
5. **比例：** 计划约 130 行，主体是参照搬运 + 配置层增量，计划里不抄函数体。