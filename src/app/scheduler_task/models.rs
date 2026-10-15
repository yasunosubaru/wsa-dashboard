// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// `SchedulerTask`, `CreateTaskParams` and the `CronParsed` fields are read by
// P7 Task 2, which is the part that actually runs `schtasks`. Under the CLI's
// `#[cfg(test)] #[path]` mount of this module, only the tests see them -- so
// they are unreachable by construction from there, not dead. Same reason and
// same shape as `config/mod.rs` and `scheduler_task/mod.rs`.
#![allow(dead_code)]

//! What a scheduled task *is*, as far as this program is concerned.
//!
//! The counterpart of the reference's `src/app/scheduler_task/models.rs`, field
//! for field, with two additions and one removal.
//!
//! # The removal
//!
//! `RawSchtasksRecord` is the reference's intermediate for a CSV row, and it is
//! gone. Its fields are all positional -- a CSV row has no names -- and that is
//! the whole problem: the reference reads `schtasks /Query /FO CSV /V` by column
//! *number*, and that table's header is **localised**. On this machine, an
//! English Windows, the same command answers with `主机名,任务名,下次运行时间,...` and 33
//! columns where English has 29. A parser written for one of those silently
//! reads the wrong field on the other, and nothing errors: the task name comes
//! back as a status string, a date comes back as `N/A`.
//!
//! So the reference's record type has no counterpart here, and
//! [`TaskXml`](super::xml::TaskXml) replaces it -- see `xml.rs` for what that
//! gets us and `MAPPING 16.1` for the measurement.
//!
//! # The additions
//!
//! `next_run` and `last_run` are the reference's, kept. `status` is the
//! reference's too and **we cannot fill it** -- Windows does not export task
//! state in the XML (only via COM), and the CSV column that carries it is
//! localised like every other one. `SchedulerTask::status` stays in the struct
//! so the card renders the reference's layout, and [`NOT_REPORTED`] is what goes
//! in it. That is the same choice P2 made for an Android package that leaves
//! `versionName` empty: a fact about the system is a different claim from
//! `"Ready"`.

use serde::{Deserialize, Serialize};

/// What a task is running, and when.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchedulerTask {
    /// Unique task identifier, e.g. `"MyTask"`.
    pub task_id: String,
    /// Display name.
    pub display_name: String,
    /// Original cron expression, e.g. `"0 9 1,15 * *"`.
    pub cron_expr: String,
    /// Command or script path to execute.
    pub command: String,
    /// Command arguments.
    pub arguments: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Current status: `"Ready"`, `"Running"`, `"Disabled"`, `"Queued"`.
    ///
    /// Windows does not export this outside COM. See the header.
    pub status: String,
    /// Last run time. Windows does not export this either, in any localisation-
    /// independent form; empty when it is not in the task's own description.
    pub last_run: String,
    /// Next run time, ISO 8601 as Windows wrote it.
    pub next_run: String,
    /// Human-readable trigger summary, e.g. `"Monthly on 1st at 09:00"`.
    pub trigger_summary: String,
    /// Whether to run with admin privileges (UAC).
    pub admin_mode: bool,
    /// Task creator.
    pub author: String,
}

/// What Windows does not tell us.
///
/// Distinct from `""` and from `"Ready"`: those are claims, and a reader who
/// sees `N/A` knows the difference between "not running" and "not reported".
pub const NOT_REPORTED: &str = "N/A";

/// Task creation parameters.
#[derive(Debug, Clone)]
pub struct CreateTaskParams {
    /// Task display name.
    pub display_name: String,
    /// Command to execute.
    pub command: String,
    /// Command arguments.
    pub arguments: String,
    /// Cron expression.
    pub cron_expr: String,
    /// Whether to run with admin privileges (UAC).
    pub admin_mode: bool,
}

/// Parsed cron fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronFields {
    pub minutes: Vec<i32>,
    pub hours: Vec<i32>,
    pub days_of_month: Vec<i32>,
    pub months: Vec<i32>,
    pub days_of_week: Vec<i32>,
}

/// A cron expression that turned out to be expressible, and how.
#[derive(Debug, Clone)]
pub struct CronParsed {
    pub fields: CronFields,
    /// Whether it can be directly mapped to a single schtasks command.
    ///
    /// Always `true` today. The reference set it to `true` unconditionally as
    /// well, but it had a use for the flag -- its `false` path was where a cron
    /// too complex for `schtasks /Create` got **split into several native tasks**
    /// named `<id>_1`, `<id>_2`. We do not do that (see `cron.rs`), so the field
    /// is kept for the reference's sake and says the same thing every time.
    pub is_simple: bool,
    /// Converted schtasks parameters.
    pub schtasks: SchtasksParams,
}

/// `schtasks /Create` parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchtasksParams {
    /// `SC`: `MINUTE` / `HOURLY` / `DAILY` / `WEEKLY` / `MONTHLY`.
    pub sc: String,
    /// `MO`: interval value.
    pub mo: Option<String>,
    /// `ST`: start time `HH:MM`.
    pub st: Option<String>,
    /// `D`: day or weekday.
    pub d: Option<String>,
    /// `M`: month.
    pub m: Option<String>,
}