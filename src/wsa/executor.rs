// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The single exit point for every adb call (spec 2.2).
//!
//! Everything that touches adb goes through here so that four things happen
//! in exactly one place: the three timeout tiers, read/write concurrency
//! lanes, the heavy-operation lock, and cancellation by killing the child.
//!
//! TDD history: first cut returned `Ok` for every exit code and used
//! powershell.exe as the test double; the tests below are what caught both.

#![allow(dead_code)]

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;

use super::command::WsaCommand;

/// Timeout tiers, spec 2.2. `install`/`uninstall` wait on the package
/// manager rather than on adb, so they get their own tier.
const READ_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(45);
const HEAVY_TIMEOUT: Duration = Duration::from_secs(1800);

/// How long to wait for a free slot before reporting the executor as busy.
/// Kept separate from the command timeout on purpose: a saturated executor
/// and a hung command are different problems and must not be conflated at
/// the layer that has to explain one or the other to the user.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(20);

/// Spec 2.2: reads and writes get their own lanes. The adb server is a
/// process-wide singleton, but a probe must never queue behind a pile of
/// installs, and a listing must not be starved by one long write.
const READ_CONCURRENCY: usize = 16;
const WRITE_CONCURRENCY: usize = 4;

/// What a successful command produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    /// Always 0 in an `Ok` -- a non-zero exit is `ExecError::Failed`.
    pub exit_code: i32,
}

/// Why a command did not produce an `Ok`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecError {
    /// The command outran its tier, or the executor stayed saturated for
    /// longer than `ACQUIRE_TIMEOUT`. Both mean "not finished in time".
    TimedOut,
    /// The adb executable itself could not be started: it is not there.
    /// Separate from `Failed` because the remedy is installing adb, not
    /// retrying (Review Focus #1).
    NotFound,
    /// The program ran and reported a non-zero exit.
    ///
    /// Both streams are kept because adb does not put its diagnosis where you
    /// would expect: `adb install` explains a failure as `Failure [...]` on
    /// **stdout**, while only the spawn-level problems land on stderr. A
    /// variant carrying `stderr` alone would report an empty reason for the
    /// most common failure the app will ever show.
    Failed {
        code: i32,
        stdout: String,
        stderr: String,
    },
}

impl ExecError {
    /// Best available explanation of the failure, for a toast or a log line.
    pub fn reason(&self) -> String {
        match self {
            Self::TimedOut => "the command took too long and was cancelled".into(),
            Self::NotFound => "adb was not found; install Android platform-tools and set the path"
                .into(),
            Self::Failed {
                code,
                stdout,
                stderr,
            } => {
                // adb's own text first -- it names the real cause.
                let detail = if !stdout.trim().is_empty() {
                    stdout.trim()
                } else {
                    stderr.trim()
                };
                if detail.is_empty() {
                    format!("exit code {code}")
                } else {
                    format!("{detail} (exit code {code})")
                }
            }
        }
    }
}

/// The single exit point for adb (spec 2.2).
///
/// Construct one and keep it: it owns the concurrency state, so a fresh
/// executor per call would silently disable every guarantee below.
#[derive(Debug)]
pub struct WsaCommandExecutor {
    adb_path: PathBuf,
    heavy_lock: Arc<Mutex<()>>,
    read_sem: Arc<Semaphore>,
    write_sem: Arc<Semaphore>,
}

impl WsaCommandExecutor {
    pub fn new(adb_path: impl Into<PathBuf>) -> Self {
        Self {
            adb_path: adb_path.into(),
            heavy_lock: Arc::new(Mutex::new(())),
            read_sem: Arc::new(Semaphore::new(READ_CONCURRENCY)),
            write_sem: Arc::new(Semaphore::new(WRITE_CONCURRENCY)),
        }
    }

    /// The adb executable this executor invokes, for diagnostics.
    pub fn adb_path(&self) -> &Path {
        &self.adb_path
    }

    /// The heavy-operation lock, for callers that want the same non-blocking
    /// `try_lock` pre-check the reference's UI handlers do before starting
    /// something heavy.
    ///
    /// Exposed rather than duplicated on the dashboard: `run` already holds
    /// this lock for the whole round trip of every `is_heavy()` command, so a
    /// second lock elsewhere would be a second truth about one invariant, and
    /// the two could disagree at the worst moment.
    pub fn heavy_lock(&self) -> &Mutex<()> {
        &self.heavy_lock
    }

    /// Run one command through the gate.
    ///
    /// Order matters: the heavy lock is taken before the semaphore, so a
    /// heavy op holds mutual exclusion for its whole run rather than only for
    /// the stretch after a slot happened to free up.
    pub async fn run(&self, cmd: &WsaCommand) -> Result<CommandOutput, ExecError> {
        let argv = cmd.argv();
        let limit = timeout_for(cmd);

        // Two installs contend for the package manager, not just for adb.
        let _heavy = if cmd.is_heavy() {
            Some(self.heavy_lock.lock().await)
        } else {
            None
        };

        let sem = if cmd.is_write() {
            &self.write_sem
        } else {
            &self.read_sem
        };
        let _slot = match timeout(ACQUIRE_TIMEOUT, sem.acquire()).await {
            Ok(Ok(permit)) => permit,
            // The semaphore is never closed, so `Err` cannot happen today; if
            // a later change closes it, report contention rather than panic
            // inside a background probe.
            Ok(Err(_)) => return Err(ExecError::TimedOut),
            Err(_) => return Err(ExecError::TimedOut),
        };

        run_program(&self.adb_path, &argv, limit).await
    }
}

/// Read / write / heavy tier for a command (spec 2.2).
fn timeout_for(cmd: &WsaCommand) -> Duration {
    if cmd.is_heavy() {
        HEAVY_TIMEOUT
    } else if cmd.is_write() {
        WRITE_TIMEOUT
    } else {
        READ_TIMEOUT
    }
}

/// Spawn `program argv...`, capture both streams, enforce `limit`.
///
/// Kept as a free function rather than folded into `run` because it is the
/// part that has to be right -- and the part that cannot be exercised through
/// `run` without a live adb (Q8): the executor decides the program and the
/// argv, but not what the program does.
///
/// `pub(crate)` rather than private so the ops layer can stop the subsystem
/// (`taskkill`) and check it (`tasklist`) through the same spawn, capture,
/// timeout and kill-on-drop path instead of growing a second one that would
/// never be tested. Those helpers go through `run`'s sibling, not through
/// `run` itself: they are not adb commands and must not take a semaphore slot
/// reserved for adb traffic.
pub(crate) async fn run_program(
    program: &Path,
    argv: &[String],
    limit: Duration,
) -> Result<CommandOutput, ExecError> {
    let mut command = Command::new(program);
    command
        .args(argv)
        // `null` on stdin matters: without it, `adb shell` with no arguments
        // waits on the terminal forever and only ever dies to the timeout.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Spec 2.2's cancellation mechanism, in one line: dropping this
        // future -- which a timeout does -- kills the child instead of
        // leaking an orphaned install holding the package manager.
        .kill_on_drop(true);

    let child = command.spawn().map_err(map_spawn_error)?;

    match timeout(limit, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let code = output.status.code().unwrap_or(-1);
            if code == 0 {
                Ok(CommandOutput {
                    stdout,
                    stderr,
                    exit_code: code,
                })
            } else {
                Err(ExecError::Failed {
                    code,
                    stdout,
                    stderr,
                })
            }
        }
        // A failure part-way through still deserves a message; it is not the
        // same as "the program is not there".
        Ok(Err(err)) => Err(ExecError::Failed {
            code: -1,
            stdout: String::new(),
            stderr: err.to_string(),
        }),
        Err(_) => Err(ExecError::TimedOut),
    }
}

fn map_spawn_error(err: io::Error) -> ExecError {
    if err.kind() == io::ErrorKind::NotFound {
        ExecError::NotFound
    } else {
        ExecError::Failed {
            code: -1,
            stdout: String::new(),
            stderr: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use super::{
        run_program, timeout_for, ExecError, WsaCommandExecutor, READ_TIMEOUT, WRITE_CONCURRENCY,
        WRITE_TIMEOUT,
    };
    use crate::wsa::command::WsaCommand;

    // -- the fake adb ----------------------------------------------------
    //
    // These tests must not need a running WSA (Q8). Two doubles instead, both
    // picked by measurement rather than assumption:
    //
    //   * `fake_adb` (findstr) -- where the executor builds the argv. It must
    //     fail on anything, with a real exit code and a real stderr.
    //   * `cmd` -- where the test builds the argv, for the exit-code, timeout
    //     and kill tests.
    //
    // What both buy is the half of the executor that matters here -- spawn,
    // capture, timeout, kill, lock, semaphore -- observable without adb. The
    // doubles are never used to assert what adb would print.
    //
    // Rejected along the way: powershell.exe (~2s per start, profile-dependent
    // exit codes) and cmd.exe as the executor's double (with stdin at null its
    // prompt EOFs and it exits 0, so a failing command looked successful).

    /// Stands in for adb in the tests that go through `WsaCommandExecutor::run`.
    ///
    /// The executor builds the argv, so the double has to cope with whatever
    /// `WsaCommand` emits -- `shell getprop`, `uninstall com.x`,
    /// `install -r a.apk`. `findstr` takes an arbitrary pattern and file list,
    /// fails all of them with a non-zero exit and a non-empty stderr, and
    /// starts in single-digit milliseconds.
    ///
    /// Measured, not assumed: `cmd.exe` was the first choice and silently
    /// returned 0, because with stdin at null its interactive prompt EOFs and
    /// exits clean. That made three tests pass or fail for reasons that had
    /// nothing to do with what they were asserting.
    fn fake_adb() -> PathBuf {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        PathBuf::from(root).join(r"System32\findstr.exe")
    }

    /// cmd.exe, used only where the test controls the argv itself (calling
    /// `run_program` directly). It gives a dependable exit code, both streams
    /// on demand, and -- for the kill test -- a loop with no child process
    /// that could outlive it.
    fn cmd() -> PathBuf {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        PathBuf::from(root).join(r"System32\cmd.exe")
    }

    /// `cmd /c` script that writes to both streams and exits 7.
    fn failing_argv() -> Vec<String> {
        vec![
            "/c".into(),
            "echo out & echo err 1>&2 & exit /b 7".into(),
        ]
    }

    /// A loop with no child process of its own, so killing it really kills
    /// everything: `ping`-based sleeps would leave an orphan behind.
    fn busy_argv() -> Vec<String> {
        vec!["/c".into(), "for /l %i in (1,0,2) do @echo x>nul".into()]
    }

    // -- timeout tiers (spec 2.2: read 10s / write 45s / heavy 1800s) -----

    #[tokio::test]
    async fn tier_follows_the_command_class() {
        let heavy = timeout_for(&WsaCommand::InstallApk {
            path: "a.apk".into(),
        });
        let write = timeout_for(&WsaCommand::AmForceStop {
            package: "com.x".into(),
        });
        let read = timeout_for(&WsaCommand::GetProps);

        assert_eq!(heavy, Duration::from_secs(1800));
        assert_eq!(write, WRITE_TIMEOUT);
        assert_eq!(read, READ_TIMEOUT);
        assert!(heavy > write && write > read);
    }

    // -- outcome decoding ------------------------------------------------

    #[tokio::test]
    async fn stdout_is_captured_on_success() {
        let out = run_program(
            &cmd(),
            &["/c".into(), "echo hello".into()],
            Duration::from_secs(30),
        )
        .await
        .expect("echo cannot fail");

        assert_eq!(out.exit_code, 0);
        assert!(out.stdout.contains("hello"), "{:?}", out.stdout);
    }

    /// Both streams survive, and the exit code comes with them. adb puts its
    /// diagnosis on stdout, so a variant that kept only stderr would report
    /// an empty reason for the most common failure.
    #[tokio::test]
    async fn a_failing_command_reports_code_and_both_streams() {
        let err = run_program(&cmd(), &failing_argv(), Duration::from_secs(30))
            .await
            .expect_err("exit 7 must not read as success");

        match &err {
            ExecError::Failed {
                code,
                stdout,
                stderr,
            } => {
                assert_eq!(*code, 7);
                assert!(stdout.contains("out"), "{stdout:?}");
                assert!(stderr.contains("err"), "{stderr:?}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }

        // The user-facing sentence must name the real cause, not just a code.
        assert!(err.reason().contains("out"), "{}", err.reason());
        assert!(err.reason().contains('7'), "{}", err.reason());
    }

    /// Review Focus #1: a missing adb must be a readable diagnostic, not a
    /// panic and not a generic failure.
    #[tokio::test]
    async fn a_missing_program_is_not_found() {
        let err = run_program(
            Path::new(r"C:\definitely\not\here\adb.exe"),
            &["devices".into()],
            Duration::from_secs(30),
        )
        .await
        .expect_err("no such exe");

        assert_eq!(err, ExecError::NotFound);
        assert!(err.reason().contains("adb"), "{}", err.reason());
    }

    // -- timeout and cancellation ----------------------------------------

    #[tokio::test]
    async fn a_command_past_its_limit_is_a_timeout() {
        let started = std::time::Instant::now();
        let err = run_program(&cmd(), &busy_argv(), Duration::from_secs(1))
            .await
            .expect_err("infinite loop, 1s limit");

        assert_eq!(err, ExecError::TimedOut);
        // It must have given up near the limit, not instantly and not after
        // the loop's own business.
        assert!(started.elapsed() < Duration::from_secs(10), "{started:?}");
    }

    /// Spec 2.2: cancellation is `kill_on_drop(true)`. Without it every
    /// timed-out install leaves an orphaned process holding the package
    /// manager -- precisely the failure that is hardest to notice.
    #[tokio::test]
    async fn a_timed_out_child_is_killed_not_leaked() {
        let marker = std::env::temp_dir().join(format!(
            "wsadash-executor-kill-{}.log",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&marker);

        // cmd's `>>` cannot quote a path, so a temp dir containing a space
        // would break the redirect. Rather than fail mysteriously elsewhere,
        // say so and step aside.
        if marker.display().to_string().contains(' ') {
            eprintln!("skipped: temp path contains a space: {}", marker.display());
            return;
        }

        let argv = vec![
            "/c".into(),
            format!("for /l %i in (1,0,2) do @echo x>>{}", marker.display()),
        ];
        let err = run_program(&cmd(), &argv, Duration::from_secs(2))
            .await
            .expect_err("loop must time out");
        assert_eq!(err, ExecError::TimedOut);

        // Let the kill land, then watch for a while.
        tokio::time::sleep(Duration::from_millis(400)).await;
        let before = std::fs::metadata(&marker).map(|m| m.len()).unwrap_or(0);
        tokio::time::sleep(Duration::from_millis(900)).await;
        let after = std::fs::metadata(&marker).map(|m| m.len()).unwrap_or(0);

        let _ = std::fs::remove_file(&marker);
        assert_eq!(before, after, "the timed-out child is still writing");
    }

    // -- concurrency lanes -----------------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn writes_queue_on_the_write_semaphore() {
        let ex = Arc::new(WsaCommandExecutor::new(fake_adb()));

        // Hold every write slot, then start a write: it must wait.
        let mut held = Vec::new();
        for _ in 0..WRITE_CONCURRENCY {
            held.push(ex.write_sem.acquire().await.expect("open"));
        }

        let task = tokio::spawn({
            let ex = ex.clone();
            async move {
                ex.run(&WsaCommand::AmForceStop {
                    package: "com.x".into(),
                })
                .await
            }
        });

        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!task.is_finished(), "the write should still be waiting");

        drop(held);
        // The outcome is irrelevant here -- what is under test is that the
        // write ran to completion once a slot freed up.
        let _ = tokio::time::timeout(Duration::from_secs(30), task)
            .await
            .expect("write should run once a slot frees")
            .expect("join");
    }

    /// The lanes are independent on purpose: the adb server is a single
    /// process, but a probe must never be stuck behind a pile of installs,
    /// and a listing must not be starved by one long write.
    #[tokio::test(flavor = "multi_thread")]
    async fn reads_do_not_queue_behind_writes() {
        let ex = Arc::new(WsaCommandExecutor::new(fake_adb()));

        let mut held = Vec::new();
        for _ in 0..WRITE_CONCURRENCY {
            held.push(ex.write_sem.acquire().await.expect("open"));
        }

        let result = tokio::time::timeout(Duration::from_secs(30), ex.run(&WsaCommand::GetProps))
            .await
            .expect("a saturated write lane must not block a read");
        // The fake adb does not speak getprop; what matters is that the read
        // came back at all rather than queueing behind the writes.
        assert!(result.is_err(), "{result:?}");

        drop(held);
    }

    /// A read must not wait behind an install either -- that is what the
    /// heavy lock is for, and leaking it is the silent failure.
    #[tokio::test(flavor = "multi_thread")]
    async fn reads_never_touch_the_heavy_lock() {
        let ex = WsaCommandExecutor::new(fake_adb());
        let held = ex.heavy_lock.lock().await;

        let result = tokio::time::timeout(Duration::from_secs(30), ex.run(&WsaCommand::GetProps))
            .await
            .expect("a read must not wait behind an install");
        assert!(result.is_err(), "{result:?}");

        drop(held);
    }

    // -- the heavy-operation lock ----------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn heavy_commands_wait_for_the_lock() {
        let ex = Arc::new(WsaCommandExecutor::new(fake_adb()));

        let held = ex.heavy_lock.lock().await;
        let task = tokio::spawn({
            let ex = ex.clone();
            async move {
                ex.run(&WsaCommand::InstallApk {
                    path: "a.apk".into(),
                })
                .await
            }
        });

        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !task.is_finished(),
            "a second heavy op must not start while one is running"
        );

        drop(held);
        // Again, completion is the assertion; the fake adb cannot succeed.
        let _ = tokio::time::timeout(Duration::from_secs(30), task)
            .await
            .expect("heavy op should run once the lock frees")
            .expect("join");
    }

    /// The realistic bug is not "no mutual exclusion", it is "the guard was
    /// never released" -- which deadlocks the next install, silently.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_heavy_lock_is_released_after_each_run() {
        let ex = WsaCommandExecutor::new(fake_adb());

        for _ in 0..2 {
            let _ = ex
                .run(&WsaCommand::InstallApk {
                    path: "a.apk".into(),
                })
                .await;
        }

        let guard = tokio::time::timeout(Duration::from_secs(2), ex.heavy_lock.lock())
            .await
            .expect("the lock must be free after a heavy run");
        drop(guard);
    }

    // -- end to end through WsaCommandExecutor::run -----------------------

    /// Proves the whole path -- classify, lock, acquire, spawn, capture --
    /// without needing adb. What is asserted is that the call came back with
    /// the program's real failure rather than being lost to a timeout or a
    /// panic, and that the reason is non-empty.
    #[tokio::test(flavor = "multi_thread")]
    async fn run_wires_through_to_the_program() {
        let ex = WsaCommandExecutor::new(fake_adb());

        let err = ex
            .run(&WsaCommand::UninstallApk {
                package: "com.x".into(),
            })
            .await
            .expect_err("cmd.exe cannot uninstall com.x");

        assert!(
            matches!(&err, ExecError::Failed { stderr, .. } if !stderr.is_empty()),
            "{err:?}"
        );
    }
}
