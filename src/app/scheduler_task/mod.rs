// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// The three `pub use` lines below are re-exports for the GUI's convenience.
// The CLI mounts this module at the crate root under `#[cfg(test)] #[path]`, so
// from *there* they are unreachable by construction and rustc says so. Same
// reason and same shape as `config/mod.rs` and `i18n/mod.rs`, which do this for
// the same test runner.
#![allow(unused_imports)]

//! Scheduled tasks: what they are, what a cron means, and what Windows exports.
//!
//! The counterpart of the reference's `src/app/scheduler_task/`, which is a
//! directory holding a 758-line `schtasks.rs` and a 90-line `models.rs`.
//!
//! # What is here, and what is not
//!
//! | reference file | here | why |
//! |---|---|---|
//! | `models.rs` | [`models`] | carried over field for field, minus `RawSchtasksRecord` |
//! | `schtasks.rs:612-771` (cron) | [`cron`] | carried over verbatim, tests and all |
//! | `schtasks.rs:29-188` (the CSV query) | [`xml`] | **replaced** -- the CSV header is localised |
//! | `schtasks.rs:189-611` (create / delete / run / the VBS launcher) | [`schtasks`] | carried over, minus the CSV fast path |
//! | `utils/system.rs:61-169` (the UAC pair) | [`elevate`] | the elevation half only |
//!
//! [`xml`] says what the replacement is and what it costs. [`models`] says what we
//! cannot get from Windows at all, which is task state. [`schtasks`] says what the
//! write half had to change and what it deliberately did not.
//!
//! # Where the machine stops and the tests start
//!
//! Everything that can be decided by arithmetic over strings and XML is decided
//! where a test can run it. [`xml`], [`cron`] and the greater part of
//! [`schtasks`] are pure; [`elevate`] is pure except for the one `ShellExecuteExW`
//! call it exists to make. What is left touches `schtasks.exe` exactly five times,
//! one per action, and those five are named in [`schtasks`]'s header.

pub mod cron;
pub mod elevate;
pub mod models;
pub mod schtasks;
pub mod xml;

pub use cron::{parse_cron, validate_cron};
pub use models::{
    NOT_REPORTED, CreateTaskParams, CronFields, CronParsed, SchedulerTask, SchtasksParams,
};
pub use schtasks::{
    create_task, delete_task, list_tasks, run_task, toggle_task,
};
pub use xml::{TASK_FOLDER, TaskXml, Trigger, TriggerKind, our_tasks, parse_tasks_xml};

#[cfg(test)]
mod tests {
    use super::*;

    /// The reason `xml` exists, as a test that fails if someone puts the CSV
    /// back.
    ///
    /// It looks for a *Rust string literal* naming the CSV format rather than for
    /// the words "FO CSV" anywhere in the file. The first version of this test
    /// searched for the prose, and its own comment contained the phrase -- which
    /// is the second time in this project a test has caught itself.
    #[test]
    fn no_source_file_asks_schtasks_for_csv_output() {
        // `concat!` cannot take the loop variable, so the files that could
        // plausibly grow a CSV path are named outright. Adding a module to this
        // directory means adding a line here, and that is the point: the list has
        // to be maintained for the test to mean anything.
        for source in [
            include_str!("xml.rs"),
            include_str!("cron.rs"),
            include_str!("schtasks.rs"),
            include_str!("elevate.rs"),
        ] {
            assert!(
                !source.contains("\"CSV\"") && !source.contains("\"FO CSV\""),
                "a schtasks invocation asking for CSV has come back; see the header of xml.rs"
            );
        }
    }

    /// Our folder is ours and nothing else is, which is the only reason the
    /// page is readable: 219 tasks on a normal desktop, two of them ours.
    #[test]
    fn the_task_folder_is_ours_and_not_the_reference_s() {
        assert_eq!(TASK_FOLDER, r"\WSA-Dashboard");
        assert!(
            !TASK_FOLDER.starts_with(r"\WSLDashboard"),
            "the reference's folder would read the reference's tasks"
        );
    }
}