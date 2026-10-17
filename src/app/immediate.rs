// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

#![allow(dead_code)]

// The `allow` is for the CLI's `#[path]` mount, where nothing can call `run` --
// the same reason and the same shape as `config/mod.rs`. What the mount buys is
// `requested`'s tests, which are the ones that matter: this function answering
// "yes" is a process that never draws a window.

//! `/scheduler`: bring the subsystem up, then exit. No window.
//!
//! This is the first line of every scheduled task's launcher
//! ([`crate::app::scheduler_task::schtasks::vbs_launcher_body`]), and it exists
//! because without it a task that runs an `adb` command is a *silent* failure on
//! a machine that has just booted: the subsystem is not up, `adb` says so on a
//! stderr nobody reads, and Task Scheduler reports the task as having run.
//!
//! Spec Q25 lists `/scheduler` as one of this binary's switches, and the
//! reference's own scheduled task runs `wsldashboard.exe /scheduler` for exactly
//! this. It is handled **before** `launcher::bootstrap` and returns before the
//! window, the tray, the logs and the settings: those are the reader's business,
//! and a task that opens a window is a task that looks hung. (Without this
//! branch the launcher's first line would do precisely that -- start a second
//! copy of the window, which the single-instance guard turns into "focus the one
//! that is already there".)
//!
//! # Why this is not `wsadash start`
//!
//! The CLI has the same verb, and a launcher could call *it* -- but only if
//! `wsadash.exe` were installed beside `wsadashboard.exe`, and spec Q3's
//! deliverable is one exe. A launcher that depends on a second file existing is
//! a launcher that silently stops starting the subsystem the day somebody copies
//! one file instead of two.
//!
//! The wait loop below is therefore a twin of `wsadash.rs::wait_until_ready`,
//! and they are deliberately not one function: that one prints each state change
//! for a person watching a terminal, and this one is silent inside a scheduled
//! task where the only reader is a log file.

use std::time::{Duration, Instant};

use tracing::{info, warn};

use crate::adb::path::adb_path;
use crate::wsa::app::wsa_path;
use crate::wsa::dashboard::dashboard_for;
use crate::wsa::models::SubsystemState;

/// How long the cold start may take before this gives up and lets the task's
/// command run anyway.
///
/// Ninety generous seconds plus room: spec §1.2 measured 12–17s cold on this
/// machine, and a machine that is still booting Windows itself is slower. The
/// caller -- the launcher -- ignores the code either way, so this deadline is a
/// bound on how long a task's *own* command is delayed, not a pass/fail.
const DEADLINE: Duration = Duration::from_secs(120);
const TICK: Duration = Duration::from_millis(500);

/// Exit codes, spec §2.1's, so a reader who runs this by hand gets the same
/// numbers `wsadash probe` gives. The launcher ignores them.
const EXIT_OK: i32 = 0;
const EXIT_NOT_INSTALLED: i32 = 4;
const EXIT_FAILED: i32 = 6;

/// Is this run one of the immediate modes rather than the window?
///
/// Pure, so the argument handling is testable without starting anything -- which
/// matters more here than usual: this function deciding "yes" is a process that
/// never draws a window, and a false positive is an app that appears to do
/// nothing.
pub fn requested(args: impl Iterator<Item = String>) -> bool {
    args.into_iter().any(|arg| arg == "/scheduler" || arg == "--scheduler")
}

/// Start the subsystem if it is not already up, wait for it, and answer with an
/// exit code.
pub async fn run() -> i32 {
    let paths = match wsa_path::resolve() {
        Ok(paths) => paths,
        Err(error) => {
            // Not installed, or an install this cannot read. Either way there is
            // nothing to start, and saying where that is decided belongs here
            // rather than in the launcher's silent exit code.
            warn!(?error, "/scheduler: WSA could not be located; nothing to start");
            return EXIT_NOT_INSTALLED;
        }
    };
    let dash = dashboard_for(adb_path(), paths);

    // The no-op case, and the common one: every task in the store runs this line
    // first, and a machine that is already up should not pay for the cold start
    // path.
    if let Ok(status) = dash.probe().await {
        if status.is_ready() {
            info!("/scheduler: the subsystem is already up");
            return EXIT_OK;
        }
    }

    if let Err(error) = dash.start_subsystem().await {
        warn!(%error, "/scheduler: the subsystem refused to start");
        return EXIT_FAILED;
    }

    let started = Instant::now();
    loop {
        match dash.probe().await {
            Ok(status) => {
                if status.is_ready() {
                    info!(
                        "/scheduler: ready after {}s",
                        started.elapsed().as_secs()
                    );
                    return EXIT_OK;
                }
                // The package cannot have vanished mid-boot; if it did, no
                // amount of waiting changes the answer.
                if matches!(status.state, SubsystemState::NotInstalled) {
                    return EXIT_NOT_INSTALLED;
                }
            }
            // A transient probe failure mid-boot is not the answer: the endpoint
            // genuinely appears late.
            Err(_) => {}
        }

        if started.elapsed() >= DEADLINE {
            warn!(
                "/scheduler: gave up after {}s; the task's command runs anyway",
                DEADLINE.as_secs()
            );
            return EXIT_FAILED;
        }
        tokio::time::sleep(TICK).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> + use<> {
        list.iter().map(|arg| arg.to_string()).collect::<Vec<_>>().into_iter()
    }

    #[test]
    fn the_switch_is_recognised_in_both_spellings() {
        assert!(requested(args(&["/scheduler"])));
        assert!(requested(args(&["--scheduler"])));
        assert!(requested(args(&["/scheduler", "/other"])));
    }

    /// A false positive is an app that never draws a window -- it would look
    /// like a program that does nothing at all, which is the worst possible
    /// outcome for the one code path that returns before Slint starts.
    #[test]
    fn nothing_else_is_recognised() {
        for list in [
            vec![],
            vec!["scheduler"],
            vec!["/schedulers"],
            vec!["-scheduler"],
            vec!["/SCHEDULER"],
            vec!["--version"],
            vec!["C:\\a\\scheduler"],
        ] {
            assert!(!requested(args(&list)), "{list:?} was taken for the switch");
        }
    }

    /// The launcher writes its own path and then this switch, so the exact
    /// spelling is a contract between two files rather than a preference.
    ///
    /// The other half of the contract lives in `schtasks.rs`, next to
    /// `vbs_launcher_body`, and it asserts the same literal for the same reason:
    /// two files own the two halves of one string, and a test in each of them
    /// pinning the literal is what makes a rename on one side fail loudly
    /// instead of producing a launcher that silently does nothing.
    #[test]
    fn the_switch_is_the_one_the_launcher_writes() {
        const WRITTEN_BY_THE_LAUNCHER: &str = "/scheduler";
        assert!(
            requested(args(&[WRITTEN_BY_THE_LAUNCHER])),
            "the launcher writes {WRITTEN_BY_THE_LAUNCHER} and this does not accept it"
        );
    }
}
