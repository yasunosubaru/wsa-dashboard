# 来源登记（PROVENANCE）

这个仓库是 [wsl-dashboard](https://github.com/owu/wsl-dashboard) **0.12.0** 的衍生作品。这张表
逐件登记"哪部分来自上游、改了什么"，因为 GPL-3.0 的义务不是一句"本仓库采用 GPL"就能交代清楚的，
而且读者有权知道哪些代码不是为 WSA 写的。

**口径**：_"照搬"_ = 逻辑与结构未动（可能换了标识符注释与显示文案）；_"改写"_ = 领域含义变了，
代码随之重写；_"自有"_ = 本仓库为 WSA 新写。

---

## 1. 来自上游（照搬或轻度改写）

| 路径 | 上游对应 | 程度 | 说明 |
| --- | --- | --- | --- |
| `src/ui/**`（除下述三个） | `src/ui/**` | 照搬 | 九个页签的组件、侧栏、标题栏、主题 token、对话框。绑定换成 WSA 的领域对象，形状不变 |
| `src/ui/views/scheduler_view.slint` | 同名 | 照搬 | 空态文案按 WSA 语义改写，其余逐行一致 |
| `src/ui/components/scheduler_card.slint` | 同名 | 照搬 | 只读六个字段 |
| `src/ui/components/dialogs/scheduler_dialog.slint` | 同名 | 照搬 | 一处文案改动：命令占位符 |
| `src/ui/theme.slint` | 同名 | 改写 | 结构体的**名字**保留，**字段语义**换成 WSA（`Distro` → 应用行，`PortProxyRuleUI` 的两端都是端口） |
| `src/app/constants.rs` | `src/app/constants.rs` | 照搬 | 符号名全部保留（Q12），值换成 WSA 的；`API1_URL` 恒为空 |
| `src/app/scheduler_task/cron.rs` | `schtasks.rs` 的 cron 部分 | 照搬 | cron 解析与"cron → `schtasks` 参数"的映射，连同上游的十条测试 |
| `src/app/scheduler_task/elevate.rs` | `utils/system.rs:61-169` | 照搬 | 一对 UAC 原语（`ShellExecuteExW` + 等待 + 日志回读），只搬计划任务用得到的那一半 |
| `src/app/scheduler_task/schtasks.rs` | `schtasks.rs` + `network/scheduler.rs` | 改写 | 增删改查的参数构造与 VBS 启动器来自上游；**读**换成 XML 解析器（上游按 CSV 列号，本地化 Windows 上是错的） |
| `src/app/scheduler_task/models.rs` | 同名 | 改写 | 结构保留，四个字段在本平台没有来源（`status`/`last_run`/`next_run`/`trigger_summary`） |
| `src/config/models.rs`、`src/config/mod.rs` | 同名 | 改写 | 键名照上游；字段按 WSA 增删（去实例/备份/临时目录，加回两个重定义过的开关） |
| `assets/i18n/*.toml` | 同名 | 照搬 | 49 个翻译包逐字照搬；`en.toml` 由本仓库的英文表生成 |
| `assets/font/icons.ttf` | 同名 | 照搬 | Segoe Fluent Icons 的 74 字形子集，由上游的 `subset_font.js` 切出 |
| `assets/logo/logo.png` | `assets/logo/logo.png` | 照搬 | 256×256 图标。规格 Q6 允许开发期复用；换图只需替换这一个文件 |
| `docs/reference/**` | 整棵源码树 | 照搬（只读） | 见第 3 节 |

## 2. 本仓库自有

| 路径 | 为什么必须有 |
| --- | --- |
| `src/wsa/**` | WSA 的定位、启停、状态机、VHDX 元数据。没有上游对应物——上游驱动的是 WSL 发行版 |
| `src/adb/**` | `adb` 的执行器、解码器、解析器、路径发现。上游用的是 `wsl.exe` |
| `src/app/scheduler_task/xml.rs` | `schtasks /Query /XML` 的解析器。上游按 `/FO CSV` 的**列号**取值，那张表头随界面语言变 |
| `src/app/scheduler_task/{mod,models}` 中 WSA 特有的部分 | `ActionKind`（`<Exec>` vs `<ComHandler>`）、文件夹边界过滤 |
| `src/app/immediate.rs` | `/scheduler`：让计划任务能把子系统叫起来 |
| `src/app/{window,single_instance,autostart,startup_info}.rs` | 按**进程 id** 找窗口（上游拿三个标题猜）、唤醒管道、Run 键、启动信息 |
| `src/config/migration.rs` | 带版本戳的迁移链（上游有十步，因为发布过十次；此处一步） |
| `src/api/**` | 只有 GitHub releases 的更新检查。上游那棵树是 `api1.wslui.com` 的客户端，**没有进本仓库** |
| `src/ui/handlers/*`（除共用的通知条） | 每个页签的接线；上游多半走同一个形状，但每个回调的**语义**都是重写的 |
| `src/ui/{disk,home,network,scheduler}_model.rs` | 纯函数视图模型，可测（`handlers/` 只在 GUI 树里编译，测不到） |
| `tools/standin-adb/**` | 替身 adb 与截图工具链。上游没有等价物，因为 WSL 不需要"断连"的可复现状态 |
| `docs/superpowers/{specs,plans}/**`、`docs/MAPPING.md` | 设计决定与逐项差异记录 |

## 3. `docs/reference/` 是什么

上游源码树的**逐字只读副本**（规格 Q18）。存在理由只有一个：让"与上游逐项对齐"可以被 diff
验证。规范与实现里几次出现"上游这里是怎么写的"，答案必须能当场查，而不是靠记忆。

它**不参与构建**，只有一个例外：`src/i18n/mod.rs` 的一条测试用
`include_str!("../../docs/reference/assets/i18n/en.toml")` 来核对"我们的英文和上游的英文差在
哪些键"，`tools/derive-ours-keys.py` 与 `tools/apply-ours-keys.py` 同理。也就是说这套基线是
**测试输入**，删掉它会让 i18n 的那条测试与两个工具失效。

版权与许可归上游作者，GPL-3.0-only。不要改这里的任何文件。

## 4. 许可结论

上游是 **GPL-3.0-only**，本仓库包含它的实质性部分（`src/ui/`、i18n 包、cron 解析、提权原语），
因此**整体只能是 GPL-3.0-only**。把衍生作品改成 MIT 会侵权，所以 `LICENSE` 不会换；能换的只有
"把上游的部分全部重写"——那等于另一个项目。

每个源文件头部都有

```
SPDX-FileCopyrightText: Copyright (c) 2026
SPDX-License-Identifier: GPL-3.0-only
```

上游文件在自己的 `docs/reference/` 里保留原头部（含上游作者的版权行）。
