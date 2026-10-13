// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The nine operations the rest of the app calls.
//!
//! Everything that talks to adb goes through [`WsaDashboard::run`], which is
//! the [`WsaCommandExecutor`] single exit point (spec 4). Everything that
//! does *not* talk to adb -- activating a `wsa://` URI, killing `WsaClient` --
//! is implemented here instead, because it has no adb argv and so has no
//! business entering the executor (docs/MAPPING.md 4.1.1).
//!
//! Three decisions worth knowing before reading the code:
//!
//!   * `launch_app` / `start_subsystem` use `open::that` (ShellExecuteW) on a
//!     `wsa://` URI. Spec 1.2 measured `WsaClient.exe /launch` exiting 0 with
//!     zero effect under seven mechanisms; the URI is the only thing that
//!     works, and a test pins the string so it cannot be "tidied" back.
//!   * `stop_subsystem` kills `WsaClient.exe`. Spec 1.3 measured `/terminate`
//!     as a no-op and the kill as a clean `vmmemWSA` shutdown.
//!   * `probe` reconnects when -- and only when -- the endpoint has no entry.
//!     Spec 1.4: after a stop the adb server drops the entry entirely rather
//!     than leaving it offline, and nothing else in the system would ever
//!     bring it back.
//!
//! TDD: 97 of 99 passed on the first run. The two that failed are worth
//! keeping in the history -- the fake's `connect` wrote a doubled serial
//! (`127.0.0.1:` + `%~2`, which already carries it) so the reconnected
//! endpoint could never be recognised, and the "adb missing" test deleted
//! the fake `.cmd`, which models a *broken* adb rather than a *missing* one:
//! `cmd.exe` still runs, so the spawn succeeds and the error arrives as
//! `Failed { code: 1 }`, not `NotFound`. Only a non-existent `.exe` path
//! produces `NotFound`, which is the real remediation fork (install
//! platform-tools vs retry).

// The three operations that start and stop things are consumed by the CLI in
// Task 13 and by the UI from P2, and are deliberately *not* unit tested: the
// only way to exercise them is to really cold-start or really kill the
// reader's own subsystem (each one is annotated at its definition). Until a
// consumer lands they therefore read as unused, along with the helper and the
// timeout they alone pull in.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use crate::adb::decoder::{
    is_system_package, listing_is_unparseable, parse_package_info, parse_package_list,
    parse_process_list, subsystem_status,
};
use crate::wsa::command::WsaCommand;
use crate::wsa::executor::{run_program, CommandOutput, ExecError};
use crate::wsa::models::{AppInfo, SubsystemStatus, SubsystemState};

use super::op_guard::{self, OpBusy, WsaOpGuard};
use super::WsaDashboard;

/// Long enough for `taskkill` / `tasklist`, both of which answer in
/// milliseconds. Separate from the adb tiers: these are not adb commands and
/// must not inherit a 1800s budget they could never need.
const HELPER_TIMEOUT: Duration = Duration::from_secs(10);

/// How long `stop_subsystem` waits for `vmmemWSA` to leave before it reports
/// a failure (spec 1.3: the subsystem goes down cleanly, so 30s is generous).
const STOP_WAIT: Duration = Duration::from_secs(30);

/// Gap between `vmmemWSA` checks. Deliberately not a constant loop: the
/// check spawns `tasklist`, and polling faster than the process can change
/// only burns CPU.
const CHECK_INTERVAL: Duration = Duration::from_millis(400);

/// Why an operation did not succeed.
///
/// Not `ExecError`: three of the nine operations never run a command (the two
/// `wsa://` activations), and two more run a Windows helper rather than adb.
/// Collapsing them would make the caller guess at what went wrong from a
/// string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpsError {
    /// adb itself failed: missing, timed out, or exited non-zero. Carries
    /// `ExecError::reason()` for the user-facing text.
    Adb(ExecError),
    /// ShellExecute refused to activate a `wsa://` URI. The URI and the OS
    /// error are both kept -- "could not open wsa://com.x" tells the user
    /// what was attempted, which `PermissionDenied` alone does not.
    Activate(String),
    /// A Windows helper (`taskkill`, `tasklist`) is missing or hung.
    Helper(String),
    /// adb exited 0 but did not report the operation as done. This is how an
    /// `adb install` that printed `Failure [...]` on stdout while returning 0
    /// surfaces (see `ExecError` for where that behaviour was measured).
    Rejected(String),
    /// The command ran and produced output we could not read -- typically
    /// `dumpsys package <name>` for a package that is not installed.
    NoData(String),
    /// Another operation holds the slot (spec 2.2: the next request is
    /// refused rather than queued or silently overwritten).
    ///
    /// Carries the operation that got there first so the UI can map it to
    /// `toast.distro_busy` and name what is in the way.
    Busy(OpBusy),
}

impl From<ExecError> for OpsError {
    fn from(err: ExecError) -> Self {
        Self::Adb(err)
    }
}

impl From<OpBusy> for OpsError {
    fn from(busy: OpBusy) -> Self {
        Self::Busy(busy)
    }
}

impl std::fmt::Display for OpsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Adb(err) => write!(f, "{}", err.reason()),
            Self::Activate(detail) | Self::Helper(detail) | Self::Rejected(detail) => {
                write!(f, "{detail}")
            }
            Self::NoData(detail) => write!(f, "{detail}"),
            Self::Busy(busy) => write!(f, "{busy}"),
        }
    }
}

impl std::error::Error for OpsError {}

// ---- pure decisions ---------------------------------------------------------
//
// The pieces below are pulled out of the async operations for one reason: they
// are where a wrong answer is silent. An operation that fails loudly is
// noticed; one that returns the wrong *state* just renders the wrong button.

/// What `adb devices` says about one host endpoint (spec 1.4 requires all
/// four to be tolerated; only `Device` counts as ready).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdbEntry {
    Device,
    Offline,
    Unauthorized,
    Absent,
}

/// Locate `127.0.0.1:<port>` in `adb devices` output.
///
/// Serial match rather than "any line": `adb devices` lists every host the
/// server knows about, and taking the first state found would report a
/// phone plugged in over USB as a ready subsystem.
fn find_entry(devices_stdout: &str, port: u16) -> AdbEntry {
    let expected = format!("127.0.0.1:{port}");

    for line in devices_stdout.lines().map(str::trim) {
        // The daemon prefixes its own output (`* daemon started at
        // tcp:5037`), and neither that nor the header is a device row.
        if line.is_empty() || line.starts_with('*') || line.starts_with("List of devices") {
            continue;
        }
        let mut fields = line.split_whitespace();
        let (Some(serial), Some(state)) = (fields.next(), fields.next()) else {
            continue;
        };
        if serial != expected {
            continue;
        }
        return match state {
            "device" => AdbEntry::Device,
            "unauthorized" => AdbEntry::Unauthorized,
            // `offline`, `authorizing` and anything a future platform-tools
            // invents are all "an entry exists but there is no session".
            _ => AdbEntry::Offline,
        };
    }
    AdbEntry::Absent
}

/// `sys.boot_completed` from the raw single-key `getprop` line.
///
/// Single-key reads print the bare value (`1`, `0`, or nothing at all), not
/// the `[key]: [value]` shape a full `getprop` emits -- that one belongs to
/// `parse_props`. Both shapes are handled in their own place because feeding
/// this function a full dump would quietly decide `1` from the wrong line.
fn boot_completed(stdout: &str) -> bool {
    stdout.trim() == "1"
}

/// Did an install / uninstall actually happen? Needs both signals.
///
/// Exit code alone trusts a tool that has been seen printing `Failure [...]`
/// while returning 0; the `Success` line alone would blame adb for a format
/// change. Requiring both fails closed, and closed is the right direction:
/// a false "failed" gets retried, a false "succeeded" loses an install.
fn op_succeeded(out: &CommandOutput) -> bool {
    out.exit_code == 0 && out.stdout.lines().any(|line| line.trim() == "Success")
}

/// Turn an install/uninstall result into an `Ok` or an explained `Err`.
fn require_success(out: CommandOutput) -> Result<(), OpsError> {
    if op_succeeded(&out) {
        return Ok(());
    }
    let detail = if !out.stdout.trim().is_empty() {
        out.stdout.trim().to_string()
    } else if !out.stderr.trim().is_empty() {
        out.stderr.trim().to_string()
    } else {
        format!("adb exited {} without reporting success", out.exit_code)
    };
    Err(OpsError::Rejected(detail))
}

/// The URI that launches one app (spec 1.2).
fn launch_uri(package: &str) -> String {
    format!("wsa://{package}")
}

/// The URI that cold-starts the subsystem.
///
/// A system package rather than a third-party one: while the subsystem is
/// booting its package manager may not have registered user apps yet, and
/// Settings is always there (spec 1.2 measured a 12s cold start this way).
const START_TRIGGER_PACKAGE: &str = "com.android.settings";

fn start_uri() -> String {
    format!("wsa://{START_TRIGGER_PACKAGE}")
}

/// Build an `AppInfo` from a listing entry plus whatever `dumpsys` said.
///
/// The `None` arm is the rule the home page depends on: a package that
/// cannot be read is still a package the user installed, so it stays in the
/// grid with an unknown version. Dropping it would remove rows at exactly the
/// moment adb is misbehaving -- which is the one moment spec 5 says the page
/// must not go blank.
fn assemble(package: String, dumped: Option<AppInfo>) -> AppInfo {
    match dumped {
        Some(info) => info,
        None => AppInfo {
            is_system: is_system_package(&package),
            package,
            version_name: String::new(),
            version_code: 0,
            apk_size_bytes: 0,
        },
    }
}

// ---- Windows helpers --------------------------------------------------------

/// Spawn `taskkill` / `tasklist` through the same spawn, capture, timeout and
/// kill path adb uses, and hand back whatever they printed.
///
/// A non-zero exit is *not* an error here: `taskkill /IM WsaClient.exe`
/// reports "not found" when the image is already gone, and "already stopped"
/// is a successful outcome for the caller. The code is returned in the
/// `CommandOutput` for the cases where it does matter.
async fn helper(program: &str, argv: &[String]) -> Result<CommandOutput, OpsError> {
    match run_program(Path::new(program), argv, HELPER_TIMEOUT).await {
        Ok(out) => Ok(out),
        Err(ExecError::Failed {
            code,
            stdout,
            stderr,
        }) => Ok(CommandOutput {
            stdout,
            stderr,
            exit_code: code,
        }),
        // TimedOut / NotFound are different in kind: the helper is hung or
        // absent, and neither is answered by pretending it said something.
        Err(other) => Err(OpsError::Helper(format!("{program}: {}", other.reason()))),
    }
}

/// Poll `check` until it reports true, or `budget` runs out.
///
/// Kept as its own function because the timeout branch is the part that
/// matters and the part no other test can reach: running it inside
/// `stop_subsystem` would need a real subsystem to stop, and waiting the full
/// 30s would make the suite unusable. The interval shrinks with the budget so
/// the test exercises the same code path in milliseconds.
async fn wait_until<F, Fut>(mut check: F, budget: Duration) -> Result<(), OpsError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let interval = CHECK_INTERVAL.min(budget / 4).max(Duration::from_millis(1));
    let deadline = tokio::time::Instant::now() + budget;

    loop {
        if check().await {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(OpsError::Helper(format!(
                "the subsystem did not stop within {}s",
                budget.as_secs()
            )));
        }
        tokio::time::sleep(interval).await;
    }
}

/// What "adb has no entry for our port" means.
///
/// It is two situations, and they want opposite things from the page:
///
///  * the subsystem is not running, so the page offers to start it, and
///  * the subsystem **is** running and Android has not opened the endpoint
///    yet, so the page should wait and offering to start it is wrong.
///
/// Measured 2026-10-12 on this machine with WSA genuinely running:
/// `WsaClient.exe` and `vmmemWSA` both alive, nothing listening on 58526, and
/// `adb connect` refused with "the target machine actively refused". The page
/// said "WSA is not running" and showed a Start WSA button for a subsystem
/// that was mid-boot. Spec 1.3 measured the same thing from the other side --
/// after a stop the entry disappears entirely and nothing reconnects it -- which
/// is why the reconnect above exists, and why the absence alone cannot be read
/// as "down".
///
/// Getting it wrong the other way is the safe way: a subsystem that has just
/// died can still have a `vmmemWSA` for a moment, and one poll of `Booting` is
/// a spinner, whereas one poll of a wrong `Stopped` tells the user to start
/// something they have already started.
fn state_for_absent_endpoint(vm_worker_alive: bool) -> SubsystemState {
    if vm_worker_alive {
        SubsystemState::Booting
    } else {
        SubsystemState::Stopped
    }
}

/// Is the VM worker process still there?
///
/// A helper that answers nothing counts as "still there": concluding "gone"
/// from an empty reply would report a stop that never happened, which is the
/// failure the user cannot recover from. Reporting a timeout they can.
async fn vm_worker_present() -> bool {
    // `vmmemWSA*`, not `vmmemWSA.exe` -- measured with `tasklist /FI` on this
    // machine: the image is reported *without* an extension, and a filter
    // that appends `.exe` matches nothing at all. Worse, `tasklist` exits 0
    // either way, so the exit code cannot rescue a wrong filter; only the
    // output says which happened. The wildcard covers both spellings.
    let argv = [
        "/FI".to_string(),
        "IMAGENAME eq vmmemWSA*".to_string(),
        "/NH".to_string(),
    ];
    match helper("tasklist", &argv).await {
        // `tasklist /FI` with no match still prints an INFO line, so an
        // empty reply means the command itself did not work.
        Ok(out) if out.stdout.trim().is_empty() => true,
        Ok(out) => out.stdout.contains("vmmemWSA"),
        Err(_) => true,
    }
}

// ---- the operations ---------------------------------------------------------

impl WsaDashboard {
    /// Run one adb command through the executor.
    async fn run(&self, cmd: WsaCommand) -> Result<CommandOutput, OpsError> {
        self.executor.run(&cmd).await.map_err(OpsError::Adb)
    }

    /// Read-only readiness check, self-healing (spec 1.4 / 5).
    ///
    /// Two probes decide everything: is `127.0.0.1:<port>` a `device`, and
    /// does `getprop sys.boot_completed` read back `1`. Nothing else counts --
    /// spec 1.4 measured `mCurrentFocus` as `null` long after boot finished.
    pub async fn probe(&self) -> Result<SubsystemStatus, OpsError> {
        self.probe_with(vm_worker_present).await
    }

    /// `probe` with the "is the hypervisor still there" question supplied.
    ///
    /// It is a parameter because the stand-in tests cannot answer it: it is a
    /// `tasklist` against the *reader's* machine, so a test that called it
    /// would be asserting about whatever Windows happens to be running when the
    /// suite runs. Here the answer is a fact the test owns.
    pub async fn probe_with<F, Fut>(&self, vm_worker_alive: F) -> Result<SubsystemStatus, OpsError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let port = self.adb_port();
        let devices = self.run(WsaCommand::ListAdbDevices).await?;

        let mut entry = find_entry(&devices.stdout, port);
        if entry == AdbEntry::Absent {
            // Best effort: a refused connect exits non-zero on some
            // platform-tools builds, and a failed reconnect is exactly the
            // state this method was going to report anyway.
            let _ = self.run(WsaCommand::ConnectAdb { port }).await;
            let again = self.run(WsaCommand::ListAdbDevices).await?;
            entry = find_entry(&again.stdout, port);
        }

        match entry {
            // Reaching `subsystem_status` keeps "absent is Stopped, and never
            // NotInstalled" in the one place that owns it and tests it
            // (Review Focus #2) -- except when the subsystem is demonstrably
            // up, which is `state_for_absent_endpoint`'s whole reason to exist.
            AdbEntry::Absent => {
                let state = state_for_absent_endpoint(vm_worker_alive().await);
                Ok(SubsystemStatus {
                    state,
                    adb_port: port,
                    boot_completed: false,
                })
            }
            AdbEntry::Offline => Ok(self.status(SubsystemState::Offline, false)),
            AdbEntry::Unauthorized => Ok(self.status(SubsystemState::Unauthorized, false)),
            AdbEntry::Device => {
                let boot = match self.run(WsaCommand::ProbeReady).await {
                    Ok(out) => boot_completed(&out.stdout),
                    // The endpoint can vanish between the two calls. Stopped
                    // is what the next tick confirms either way; failing the
                    // whole health check over a transient would blank the
                    // home page, which spec 5 forbids outright.
                    Err(_) => return Ok(subsystem_status(&HashMap::new(), false, port)),
                };
                let mut props = HashMap::new();
                props.insert(
                    String::from("sys.boot_completed"),
                    String::from(if boot { "1" } else { "0" }),
                );
                Ok(subsystem_status(&props, true, port))
            }
        }
    }

    /// Installed packages, with `dumpsys` detail attached.
    ///
    /// `include_system = false` is what the home page asks for: the `-3` flag
    /// trims what adb sends and the name heuristic trims what adb mislabels,
    /// so both are applied.
    pub async fn list_apps(&self, include_system: bool) -> Result<Vec<AppInfo>, OpsError> {
        let listing = self.run(WsaCommand::ListPackages { include_system }).await?;
        // Review Focus #3: a reply that parses to nothing and a reply we did
        // not understand are the same to the caller, so say so while we still
        // have the raw bytes. `tracing` rather than `eprintln` because the
        // GUI consumes this same method from P2 and there is only one place
        // that can see the reply.
        if listing_is_unparseable(&listing.stdout) {
            tracing::warn!(
                bytes = listing.stdout.len(),
                sample = %listing.stdout.trim().chars().take(160).collect::<String>(),
                "adb replied to `pm list packages` with no package: line; \
                 reporting an empty list rather than a failure"
            );
        }
        let names = parse_package_list(&listing.stdout, !include_system);

        let mut apps = Vec::with_capacity(names.len());
        for package in names {
            let dumped = self
                .run(WsaCommand::PackageInfo {
                    package: package.clone(),
                })
                .await
                .ok()
                .and_then(|out| parse_package_info(&out.stdout));
            apps.push(assemble(package, dumped));
        }
        Ok(apps)
    }

    /// Which packages are currently running, for the home card's start/stop
    /// toggle.
    ///
    /// The reference reads a `Running`/`Stopped` column straight out of
    /// `wsl -l -v`; Android publishes no such listing, so `ps -A` is the
    /// closest equivalent and was verified against a real device
    /// (2026-10-11).
    ///
    /// **Deliberately not fallible.** A subsystem that is booting,
    /// unauthorized or offline answers nothing useful to `ps -A`, and a
    /// missing or wedged adb answers nothing at all. Spec 5 says a degraded
    /// connection must not blank the home page, so "could not read" has to
    /// become "nothing is running" -- and returning `Result` would leave the
    /// caller free to turn it into a blank page. The failure is logged while
    /// the reply is still in hand.
    pub async fn running_apps(&self) -> HashSet<String> {
        match self.run(WsaCommand::ListProcesses).await {
            Ok(reply) => parse_process_list(&reply.stdout),
            Err(error) => {
                tracing::warn!(
                    %error,
                    "`ps -A` was not readable; reporting no running apps so the home page still renders"
                );
                HashSet::new()
            }
        }
    }

    /// One package with its version, for the detail dialog.
    pub async fn app_info(&self, package: &str) -> Result<AppInfo, OpsError> {
        let out = self
            .run(WsaCommand::PackageInfo {
                package: package.to_owned(),
            })
            .await?;
        parse_package_info(&out.stdout).ok_or_else(|| {
            OpsError::NoData(format!(
                "`dumpsys package {package}` reported no version -- \
                 the package is not installed, or the platform refused the read"
            ))
        })
    }

    /// Start one Android app (spec 1.2).
    pub async fn launch_app(&self, package: &str) -> Result<(), OpsError> {
        let _guard = WsaOpGuard::create(self, op_guard::LAUNCHING)?;
        activate(&launch_uri(package))
    }

    /// Stop one Android app the way Android itself does.
    ///
    /// **No guard, deliberately.** Spec 2.2 authorises exactly five operation
    /// names and `stop_app` is not one of them; reusing `STOPPING` would make
    /// the busy toast say a subsystem operation was running when it was an
    /// app, and inventing a sixth name would need an i18n key the spec did not
    /// ask for. It needs no exclusion either: `am force-stop` is a single
    /// fast, idempotent write against a different package than any install is
    /// touching, so there is no state for it to corrupt and nothing for it to
    /// wait on.
    pub async fn stop_app(&self, package: &str) -> Result<(), OpsError> {
        self.run(WsaCommand::AmForceStop {
            package: package.to_owned(),
        })
        .await?;
        Ok(())
    }

    /// Install an apk (spec 1.4: `adb install -r` round-trip verified).
    ///
    /// Mutual exclusion between two installs comes from the executor's heavy
    /// lock, which `run` takes for the whole round trip because `InstallApk`
    /// is `is_heavy()`. There is no second lock here to take.
    pub async fn install_apk(&self, path: &Path) -> Result<(), OpsError> {
        let _guard = WsaOpGuard::create(self, op_guard::INSTALLING)?;
        let out = self
            .run(WsaCommand::InstallApk {
                path: path.to_string_lossy().into_owned(),
            })
            .await?;
        require_success(out)
    }

    /// Remove a package.
    pub async fn uninstall_apk(&self, package: &str) -> Result<(), OpsError> {
        let _guard = WsaOpGuard::create(self, op_guard::UNINSTALLING)?;
        let out = self
            .run(WsaCommand::UninstallApk {
                package: package.to_owned(),
            })
            .await?;
        require_success(out)
    }

    /// Cold-start the subsystem (spec 1.2: 12s measured).
    ///
    /// Deliberately returns as soon as the URI is handed to the shell. The
    /// subsystem needs 12-17s and the UI shows that as a busy state driven by
    /// `probe` ticks, not as an unresponsive button. The `adb connect` that
    /// spec 1.4 requires after a start happens inside `probe` itself, once
    /// the endpoint exists to connect to.
    pub async fn start_subsystem(&self) -> Result<(), OpsError> {
        let _guard = WsaOpGuard::create(self, op_guard::STARTING)?;
        activate(&start_uri())
    }

    /// Stop the subsystem (spec 1.3).
    ///
    /// Not unit tested on purpose: the only way to exercise it is to really
    /// kill `WsaClient.exe`, which would shut down the reader's own running
    /// subsystem. `wait_until` -- the part that decides success -- is tested
    /// separately, and `probe` against the live subsystem is the `#[ignore]`
    /// test at the bottom of this file.
    pub async fn stop_subsystem(&self) -> Result<(), OpsError> {
        let _guard = WsaOpGuard::create(self, op_guard::STOPPING)?;
        // `taskkill`'s own exit code is deliberately not consulted: it exits
        // non-zero when the image is already gone, which is the state we
        // want. The poll below is what actually decides.
        helper(
            "taskkill",
            &["/F", "/IM", "WsaClient.exe"]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        )
        .await?;
        wait_until(vm_worker_present, STOP_WAIT).await
    }

    fn status(&self, state: SubsystemState, boot_completed: bool) -> SubsystemStatus {
        SubsystemStatus {
            state,
            adb_port: self.adb_port(),
            boot_completed,
        }
    }
}

/// Hand a `wsa://` URI to the shell (ShellExecuteW via the `open` crate).
///
/// A free function rather than a method because it touches no dashboard
/// state -- which is also why no test may call it: `wsa://` really does
/// cold-start the subsystem, so a unit test invoking it would shut down and
/// restart the reader's machine on every run. The URI is what gets tested.
fn activate(uri: &str) -> Result<(), OpsError> {
    open::that(uri).map_err(|err| OpsError::Activate(format!("could not open {uri}: {err}")))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;
    use crate::wsa::app::wsa_path::WsaPaths;
    use crate::wsa::dashboard::dashboard_for;

    const PORT: u16 = 58526;

    /// Captured from the local device 2026-10-11 via `adb shell ps -A`, with
    /// the numeric columns shortened. A colon subprocess is included because
    /// the decoder has to keep it.
    const PS: &str = "\
USER           PID  PPID     VSZ    RSS WCHAN            ADDR S NAME
u0_a2         7021   247 16053040 240564 0                  0 S com.microsoft.launcher
u0_a2         7022   247 16053040 240564 0                  0 S com.topjohnwu.magisk
u0_a2         7023   247 16053040 240564 0                  0 S com.topjohnwu.magisk:daemon
";

    // -- the fake adb --------------------------------------------------------
    //
    // Q8: no test may need a running WSA. A `.cmd` stands in for the adb
    // executable -- the executor chooses the program and the argv but not
    // what the program does -- and reads its replies from files in its own
    // folder, so every test decides what adb "says".
    //
    // Verified before this was written: `std::process::Command` does execute
    // a `.cmd` on Windows (it goes through `cmd.exe /c`), and the
    // tab-separated `adb devices` row survives the round trip intact. The
    // script is pure ASCII and takes its state directory from `%~dp0` rather
    // than a baked path, so a temp folder with spaces or non-ASCII characters
    // cannot corrupt it.
    const BATCH: &str = r#"@echo off
setlocal
set "S=%~dp0"

if "%~1"=="devices" goto devices
if "%~1"=="connect" goto connect
if "%~1"=="install" goto op
if "%~1"=="uninstall" goto op
if "%~1"=="shell" goto shell
echo fake-adb: no rule for %* 1>&2
exit /b 1

:devices
type "%S%devices.txt"
exit /b 0

:connect
if exist "%S%connect.allow" (
  echo connected to %~2
  >>"%S%devices.txt" echo %~2@TAB@device
  exit /b 0
)
echo failed to connect to %~2: Connection refused 1>&2
exit /b 1

:op
if exist "%S%op.delay" ping -n 2 127.0.0.1 >nul
type "%S%op.stdout"
type "%S%op.stderr" 1>&2
set "CODE=1"
if exist "%S%op.exit" set /p CODE=<"%S%op.exit"
exit /b %CODE%

:shell
if "%~2"=="getprop" goto getprop
if "%~2"=="ps" goto ps
if "%~2"=="pm" goto pm
if "%~2"=="dumpsys" goto dumpsys
if "%~2"=="am" goto am
echo fake-adb: no rule for shell %* 1>&2
exit /b 1

:ps
if exist "%S%ps.fail" (
  echo fake-adb: ps is unavailable 1>&2
  exit /b 1
)
type "%S%processes.txt"
exit /b 0

:getprop
if "%~3"=="sys.boot_completed" goto boot
type "%S%props.txt"
exit /b 0

:boot
type "%S%boot.txt"
exit /b 0

:pm
type "%S%packages.txt"
exit /b 0

:dumpsys
if exist "%S%dumpsys.%~4.txt" (
  type "%S%dumpsys.%~4.txt"
  exit /b 0
)
type "%S%dumpsys.txt"
exit /b 0

:am
exit /b 0
"#;

    struct FakeAdb {
        state: TempDir,
    }

    fn paths_fixture() -> WsaPaths {
        WsaPaths {
            install_location: PathBuf::from(r"C:\WSA"),
            manifest_path: PathBuf::from(r"C:\WSA\AppxManifest.xml"),
            client_exe: PathBuf::from(r"C:\WSA\WsaClient\WsaClient.exe"),
            settings_aumid:
                "MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe!SettingsApp"
                    .into(),
            adb_port: PORT,
        }
    }

    impl FakeAdb {
        fn new() -> Self {
            let state = tempfile::tempdir().expect("temp dir");
            let fake = Self { state };
            std::fs::write(
                fake.path().join("fake-adb.cmd"),
                BATCH.replace("@TAB@", "\t"),
            )
            .expect("write fake adb");
            // Every file the script may read starts out present, so a
            // missing file is never the unexplained reason a test behaved
            // differently than intended.
            fake.write("devices.txt", "");
            fake.write("boot.txt", "");
            fake.write("props.txt", "");
            fake.write("packages.txt", "");
            fake.write("processes.txt", "");
            fake.write("dumpsys.txt", "");
            fake.write("op.stdout", "");
            fake.write("op.stderr", "");
            fake.write("op.exit", "0");
            fake
        }

        fn path(&self) -> PathBuf {
            self.state.path().to_path_buf()
        }

        fn write(&self, name: &str, content: &str) {
            std::fs::write(self.path().join(name), content).expect("write state file");
        }

        /// Let the next `adb connect` succeed and register the endpoint, the
        /// way a reachable relay would. Absent by default: that is what an
        /// unreachable port looks like.
        fn allow_connect(&self) {
            self.write("connect.allow", "");
        }

        fn dashboard(&self) -> WsaDashboard {
            dashboard_for(self.path().join("fake-adb.cmd"), paths_fixture())
        }
    }

    /// `adb devices` as the platform prints it: a header, then
    /// `serial<TAB>state` per endpoint.
    fn devices(entries: &[(&str, &str)]) -> String {
        let mut out = String::from("List of devices attached\n");
        for (serial, state) in entries {
            out.push_str(serial);
            out.push('\t');
            out.push_str(state);
            out.push('\n');
        }
        out
    }

    fn out(stdout: &str, stderr: &str, exit_code: i32) -> CommandOutput {
        CommandOutput {
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            exit_code,
        }
    }

    /// A trimmed but structurally real `dumpsys package` block: the header,
    /// an indented body, and the three fields the decoder reads.
    fn dumpsys(package: &str, version_name: &str, version_code: i32, flags: &str) -> String {
        format!(
            "Activity Resolver Table:\n\
             Packages:\n\
             \x20 Package [{package}] (1a2b3c4):\n\
             \x20   userId=10234\n\
             \x20   pkg=Package{{1a2b3c4 {package}}}\n\
             \x20   versionName={version_name}\n\
             \x20   versionCode={version_code} minSdk=23 targetSdk=34\n\
             \x20   pkgFlags=[ {flags} ]\n\
             Dexopt state:\n"
        )
    }

    // -- pure decisions ------------------------------------------------------

    #[test]
    fn find_entry_recognises_each_adb_state() {
        let cases = [
            ("device", AdbEntry::Device),
            ("offline", AdbEntry::Offline),
            ("unauthorized", AdbEntry::Unauthorized),
        ];
        for (reported, expected) in cases {
            assert_eq!(
                find_entry(&devices(&[("127.0.0.1:58526", reported)]), PORT),
                expected,
                "reported {reported}"
            );
        }
        assert_eq!(find_entry(&devices(&[]), PORT), AdbEntry::Absent);
    }

    /// The daemon prefixes its own chatter and other hosts share the list;
    /// neither may be mistaken for our endpoint.
    #[test]
    fn find_entry_matches_the_endpoint_not_the_first_row() {
        let out = "\
* daemon not running; starting now at tcp:5037\n\
* daemon started successfully\n\
List of devices attached\n\
192.168.1.5:5555\tdevice\n\
127.0.0.1:5555\toffline\n";

        assert_eq!(find_entry(out, PORT), AdbEntry::Absent);
        assert_eq!(find_entry(out, 5555), AdbEntry::Offline);
    }

    /// A different port on the same host is a different endpoint.
    #[test]
    fn find_entry_does_not_match_a_neighbouring_port() {
        assert_eq!(
            find_entry(&devices(&[("127.0.0.1:58525", "device")]), PORT),
            AdbEntry::Absent
        );
    }

    #[test]
    fn boot_is_read_from_the_bare_getprop_line() {
        assert!(boot_completed("1\n"));
        assert!(boot_completed("  1\r\n"));
        assert!(!boot_completed("0"));
        assert!(!boot_completed(""));
        // The bracketed shape is a full `getprop` dump, which never comes
        // back from the single-key probe. Accepting it here would be reading
        // the wrong parser's output.
        assert!(!boot_completed("[sys.boot_completed]: [1]"));
    }

    #[test]
    fn success_needs_both_a_zero_exit_and_the_success_line() {
        assert!(op_succeeded(&out(
            "Performing Streamed Install\nSuccess\n",
            "",
            0
        )));
        // adb has been seen reporting failure on stdout while exiting 0.
        assert!(!op_succeeded(&out(
            "Failure [REINSTALL_FAILED_VERSION_DOWNGRADE]\n",
            "",
            0
        )));
        assert!(!op_succeeded(&out("Success\n", "", 1)));
        assert!(!op_succeeded(&out("", "", 0)));
    }

    #[test]
    fn rejected_operations_carry_adb_text() {
        let err = require_success(out("Failure [DELETE_FAILED_INTERNAL_ERROR]\n", "", 0))
            .expect_err("no Success line means no success");
        assert!(
            err.to_string().contains("DELETE_FAILED_INTERNAL_ERROR"),
            "{err}"
        );

        // Nothing to quote: say what happened rather than showing an empty
        // message the UI would render as a bare red box.
        let err = require_success(out("", "", 0)).expect_err("empty output is not success");
        assert!(err.to_string().contains("without reporting success"), "{err}");

        assert!(require_success(out("Success\n", "", 0)).is_ok());
    }

    /// Spec 1.2: `wsa://<package>` is the only mechanism that launched an
    /// app. Pinning the string is what stops a later tidy-up from swapping it
    /// back to `WsaClient.exe /launch`, which exited 0 and did nothing under
    /// seven attempts.
    #[test]
    fn launch_uri_is_the_wsa_scheme_and_nothing_else() {
        assert_eq!(
            launch_uri("com.topjohnwu.magisk"),
            "wsa://com.topjohnwu.magisk"
        );
        assert!(!launch_uri("com.x").contains("WsaClient"));
    }

    #[test]
    fn start_uri_uses_a_package_that_exists_while_booting() {
        assert_eq!(start_uri(), "wsa://com.android.settings");
    }

    #[test]
    fn a_package_that_cannot_be_read_is_still_listed() {
        let unknown = assemble("com.example.app".into(), None);
        assert_eq!(unknown.package, "com.example.app");
        assert_eq!(unknown.version_code, 0);
        assert_eq!(unknown.version_name, "");
        assert!(!unknown.is_system);

        // Even a placeholder has to know it is a system package, or the
        // "hide system apps" filter would let them through on the fallback
        // path exactly when dumpsys was failing.
        assert!(assemble("com.android.settings".into(), None).is_system);
    }

    #[tokio::test]
    async fn wait_until_returns_once_the_check_passes() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);

        let result = wait_until(
            move || {
                let counter = Arc::clone(&counter);
                async move { counter.fetch_add(1, Ordering::SeqCst) >= 2 }
            },
            Duration::from_secs(5),
        )
        .await;

        assert!(result.is_ok());
        assert!(calls.load(Ordering::SeqCst) >= 3, "it must have polled");
    }

    #[tokio::test]
    async fn wait_until_reports_a_subsystem_that_will_not_stop() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);

        let result = wait_until(
            move || {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    false
                }
            },
            Duration::from_millis(120),
        )
        .await;

        let err = result.expect_err("a check that never passes must give up");
        assert!(err.to_string().contains("did not stop"), "{err}");
        assert!(calls.load(Ordering::SeqCst) >= 1);
    }

    // -- probe, against the fake -------------------------------------------

    #[tokio::test]
    async fn probe_is_ready_only_when_the_endpoint_is_a_device_and_boot_completed() {
        let adb = FakeAdb::new();
        adb.write(
            "devices.txt",
            &devices(&[("127.0.0.1:58526", "device")]),
        );
        adb.write("boot.txt", "1\n");

        let status = adb.dashboard().probe().await.expect("probe");

        assert_eq!(status.state, SubsystemState::Ready);
        assert_eq!(status.adb_port, PORT);
        assert!(status.boot_completed);
        assert!(status.is_ready());
    }

    #[tokio::test]
    async fn probe_is_booting_while_the_boot_flag_is_not_one() {
        let adb = FakeAdb::new();
        adb.write(
            "devices.txt",
            &devices(&[("127.0.0.1:58526", "device")]),
        );
        adb.write("boot.txt", "0\n");

        let status = adb.dashboard().probe().await.expect("probe");

        assert_eq!(status.state, SubsystemState::Booting);
        assert!(!status.boot_completed);
        assert!(!status.is_ready());
    }

    /// A refused reconnect must not fail the health check: `adb connect` to a
    /// closed port exits non-zero, and the answer was going to be `Stopped`
    /// either way.
    #[tokio::test]
    async fn probe_reports_stopped_when_the_endpoint_is_absent() {
        let adb = FakeAdb::new();
        adb.write("devices.txt", &devices(&[]));

        let status = adb
            .dashboard()
            .probe_with(|| async { false })
            .await
            .expect("probe");

        assert_eq!(status.state, SubsystemState::Stopped);
        assert!(!status.is_ready());
    }

    /// Measured 2026-10-12 on this machine, with WSA genuinely running:
    /// `WsaClient.exe` and `vmmemWSA` both alive, nothing listening on 58526,
    /// `adb connect` refused outright. The page answered "WSA is not running"
    /// with a Start WSA button -- for a subsystem that was already starting,
    /// because Android had not opened its adb endpoint yet.
    ///
    /// "Absent entry" is therefore not one state but two, and only the
    /// hypervisor's process tells them apart.
    #[tokio::test]
    async fn an_absent_endpoint_is_booting_when_the_vm_worker_is_alive() {
        let adb = FakeAdb::new();
        adb.write("devices.txt", &devices(&[]));

        let status = adb
            .dashboard()
            .probe_with(|| async { true })
            .await
            .expect("probe");

        assert_eq!(status.state, SubsystemState::Booting);
        assert!(!status.boot_completed);
        assert!(!status.is_ready());
    }

    /// Spec 1.4: after a stop the adb server drops the entry entirely, and
    /// nothing else would ever bring it back. A probe that only reads would
    /// report `Stopped` forever even with WSA up, so it reconnects -- once,
    /// and only when the entry is missing.
    #[tokio::test]
    async fn probe_reconnects_an_endpoint_that_has_no_entry() {
        let adb = FakeAdb::new();
        adb.write("devices.txt", &devices(&[]));
        adb.allow_connect();
        adb.write("boot.txt", "1\n");

        let status = adb.dashboard().probe().await.expect("probe");

        assert_eq!(status.state, SubsystemState::Ready);
    }

    /// Spec 1.4 wants all four states tolerated, and spec 5 puts both of
    /// these in the T2 tier rather than the "not running" one.
    #[tokio::test]
    async fn probe_tolerates_unauthorized_and_offline_endpoints() {
        let adb = FakeAdb::new();
        adb.write(
            "devices.txt",
            &devices(&[("127.0.0.1:58526", "unauthorized")]),
        );
        let status = adb.dashboard().probe().await.expect("probe");
        assert_eq!(status.state, SubsystemState::Unauthorized);

        adb.write(
            "devices.txt",
            &devices(&[("127.0.0.1:58526", "offline")]),
        );
        let status = adb.dashboard().probe().await.expect("probe");
        assert_eq!(status.state, SubsystemState::Offline);
        assert!(!status.is_ready());
    }

    // -- listing and detail --------------------------------------------------

    #[tokio::test]
    async fn list_apps_hides_system_packages_by_default() {
        let adb = FakeAdb::new();
        adb.write(
            "packages.txt",
            "package:com.topjohnwu.magisk\npackage:com.android.settings\n",
        );
        adb.write(
            "dumpsys.com.topjohnwu.magisk.txt",
            &dumpsys("com.topjohnwu.magisk", "30.6", 30600, "ALLOW_BACKUP"),
        );

        let apps = adb.dashboard().list_apps(false).await.expect("list");

        assert_eq!(apps.len(), 1, "the system package must be filtered out");
        assert_eq!(apps[0].package, "com.topjohnwu.magisk");
        assert_eq!(apps[0].version_name, "30.6");
        assert_eq!(apps[0].version_code, 30600);
        assert!(!apps[0].is_system);
    }

    #[tokio::test]
    async fn list_apps_keeps_system_packages_when_asked() {
        let adb = FakeAdb::new();
        adb.write(
            "packages.txt",
            "package:com.topjohnwu.magisk\npackage:com.android.settings\n",
        );
        adb.write(
            "dumpsys.com.topjohnwu.magisk.txt",
            &dumpsys("com.topjohnwu.magisk", "30.6", 30600, "ALLOW_BACKUP"),
        );
        adb.write(
            "dumpsys.com.android.settings.txt",
            &dumpsys("com.android.settings", "14", 34, "SYSTEM"),
        );

        let apps = adb.dashboard().list_apps(true).await.expect("list");

        assert_eq!(apps.len(), 2);
        assert!(apps[1].is_system, "dumpsys flags must survive the merge");
    }

    /// The home page must not lose rows when `dumpsys` misbehaves (spec 5's
    /// "never blank" requirement, applied per package).
    #[tokio::test]
    async fn list_apps_keeps_a_package_it_cannot_read() {
        let adb = FakeAdb::new();
        adb.write(
            "packages.txt",
            "package:com.topjohnwu.magisk\npackage:com.example.broken\n",
        );
        // dumpsys.txt stays empty: `dumpsys.com.example.broken.txt` and the
        // default both yield nothing parseable.
        adb.write(
            "dumpsys.com.topjohnwu.magisk.txt",
            &dumpsys("com.topjohnwu.magisk", "30.6", 30600, "ALLOW_BACKUP"),
        );

        let apps = adb.dashboard().list_apps(false).await.expect("list");

        assert_eq!(apps.len(), 2, "an unreadable package must not vanish");
        assert_eq!(apps[1].package, "com.example.broken");
        assert_eq!(apps[1].version_code, 0);
    }

    #[tokio::test]
    async fn running_apps_reports_what_ps_says() {
        let fake = FakeAdb::new();
        fake.write("processes.txt", PS);
        let running = fake.dashboard().running_apps().await;
        assert!(running.contains("com.microsoft.launcher"));
        assert!(running.contains("com.topjohnwu.magisk"));
        assert!(running.contains("com.topjohnwu.magisk:daemon"));
    }

    /// The degradation anchor for spec 5. A subsystem that is booting,
    /// unauthorized or offline answers nothing useful to `ps -A`, and a
    /// missing or wedged adb answers nothing at all. Either way the answer
    /// must be "nothing is running" -- not an error -- so the home page can
    /// still draw its cards.
    #[tokio::test]
    async fn running_apps_degrades_to_nothing_when_ps_fails() {
        let fake = FakeAdb::new();
        fake.write("ps.fail", "");
        assert!(fake.dashboard().running_apps().await.is_empty());
    }

    #[tokio::test]
    async fn list_apps_reports_a_missing_adb_rather_than_an_empty_list() {
        // A path that does not exist, with an `.exe` extension: that is what
        // `adb` missing from PATH actually looks like to `CreateProcess`.
        // Deleting the fake's `.cmd` would not model it -- `cmd.exe` still
        // runs, so the spawn succeeds and the failure arrives as
        // `Failed { code: 1 }` instead of `NotFound`, which is a different
        // remedy (retry vs install platform-tools).
        let dash = dashboard_for(r"C:\no\such\platform-tools\adb.exe", paths_fixture());

        let err = dash.list_apps(false).await.expect_err("no adb to run");

        assert!(matches!(err, OpsError::Adb(ExecError::NotFound)), "{err:?}");
    }

    #[tokio::test]
    async fn app_info_without_a_version_is_an_error_not_a_fabricated_one() {
        let adb = FakeAdb::new();
        adb.write("dumpsys.txt", "");

        let err = adb
            .dashboard()
            .app_info("com.example.absent")
            .await
            .expect_err("nothing parseable must not become version 0.0");

        assert!(matches!(err, OpsError::NoData(_)), "{err:?}");
        assert!(err.to_string().contains("com.example.absent"), "{err}");
    }

    #[tokio::test]
    async fn app_info_reads_the_version_dumpsys_reports() {
        let adb = FakeAdb::new();
        adb.write(
            "dumpsys.com.example.app.txt",
            &dumpsys("com.example.app", "2.1.0", 20100, "ALLOW_BACKUP"),
        );

        let info = adb
            .dashboard()
            .app_info("com.example.app")
            .await
            .expect("info");

        assert_eq!(info.package, "com.example.app");
        assert_eq!(info.version_name, "2.1.0");
        assert_eq!(info.version_code, 20100);
    }

    // -- mutating operations -------------------------------------------------

    #[tokio::test]
    async fn stop_app_runs_force_stop() {
        let adb = FakeAdb::new();

        adb.dashboard()
            .stop_app("com.example.app")
            .await
            .expect("force-stop exits 0 with no output");
    }

    #[tokio::test]
    async fn install_is_success_only_when_adb_says_success() {
        let adb = FakeAdb::new();
        adb.write("op.stdout", "Performing Streamed Install\nSuccess\n");

        adb.dashboard()
            .install_apk(Path::new(r"C:\apks\demo app.apk"))
            .await
            .expect("adb reported Success");
    }

    #[tokio::test]
    async fn install_rejects_a_zero_exit_that_never_reported_success() {
        let adb = FakeAdb::new();
        adb.write("op.stdout", "");

        let err = adb
            .dashboard()
            .install_apk(Path::new(r"C:\apks\demo app.apk"))
            .await
            .expect_err("exit 0 without Success is still not a success");

        assert!(err.to_string().contains("without reporting success"), "{err}");
    }

    /// The exact failure shape Task 10 established: adb puts its diagnosis on
    /// stdout, so this is the text the user has to see.
    #[tokio::test]
    async fn install_failure_carries_adb_text_from_stdout() {
        let adb = FakeAdb::new();
        adb.write(
            "op.stdout",
            "Failure [REINSTALL_FAILED_VERSION_DOWNGRADE]\n",
        );
        adb.write("op.exit", "1");

        let err = adb
            .dashboard()
            .install_apk(Path::new(r"C:\apks\demo app.apk"))
            .await
            .expect_err("non-zero exit is an error");

        assert!(
            matches!(&err, OpsError::Adb(ExecError::Failed { code: 1, .. })),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .contains("REINSTALL_FAILED_VERSION_DOWNGRADE"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn uninstall_failure_carries_adb_text() {
        let adb = FakeAdb::new();
        adb.write(
            "op.stdout",
            "Failure [DELETE_FAILED_INTERNAL_ERROR]\n",
        );
        adb.write("op.exit", "1");

        let err = adb
            .dashboard()
            .uninstall_apk("com.example.app")
            .await
            .expect_err("non-zero exit is an error");

        assert!(err.to_string().contains("DELETE_FAILED_INTERNAL_ERROR"), "{err}");
    }

    #[tokio::test]
    async fn uninstall_is_success_when_adb_prints_success() {
        let adb = FakeAdb::new();
        adb.write("op.stdout", "Success\n");

        adb.dashboard()
            .uninstall_apk("com.example.app")
            .await
            .expect("adb reported Success");
    }

    // -- the guard, observed from inside a running operation -----------------

    /// Task 12's real claim: the five mutating operations actually *hold* the
    /// slot while they run. Proving it needs to look inside a live call, so
    /// the fake adb gains an optional `op.delay` file that makes an install
    /// answer ~1s late -- long enough to observe the guard, short enough that
    /// this test costs 1s rather than the 10s read timeout.
    ///
    /// The intruder is `uninstall_apk` rather than `launch_app` on purpose:
    /// if the guard were broken, `launch_app` would hand a real `wsa://`
    /// URI to ShellExecute and cold-start the reader's own subsystem. An
    /// uninstall only reaches the fake adb, so a failure here is a failed
    /// assertion rather than an interrupted session.
    #[tokio::test]
    async fn a_running_operation_is_visible_and_refuses_the_next_one() {
        let adb = FakeAdb::new();
        adb.write("op.stdout", "Performing Streamed Install\nSuccess\n");
        adb.write("op.delay", "");
        let dash = Arc::new(adb.dashboard());

        let installing = tokio::spawn({
            let dash = Arc::clone(&dash);
            async move { dash.install_apk(Path::new(r"C:\apks\demo app.apk")).await }
        });

        // Wait for the guard rather than sleeping a guessed amount: the
        // fake's delay is an implementation detail, and a fixed sleep would
        // turn into a flaky test the moment the machine got slower.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while dash.get_active_op() != Some(op_guard::INSTALLING) {
            assert!(
                tokio::time::Instant::now() < deadline,
                "install never registered its guard"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(dash.get_active_op(), Some(op_guard::INSTALLING));

        let err = dash
            .uninstall_apk("com.example.app")
            .await
            .expect_err("a second operation must be refused, not queued");
        assert!(matches!(err, OpsError::Busy(_)), "{err:?}");
        assert!(err.to_string().contains("Installing"), "{err}");
        assert_eq!(
            dash.get_active_op(),
            Some(op_guard::INSTALLING),
            "the refusal must not clobber the operation in flight"
        );

        installing
            .await
            .expect("join")
            .expect("the install itself succeeds");
        assert_eq!(dash.get_active_op(), None, "released on completion");
    }

    // -- live subsystem ------------------------------------------------------

    /// Step 5's placeholder until the CLI exists (Task 13): the one test that
    /// does need the real thing, and so is excluded from the default run (Q8).
    #[tokio::test]
    #[ignore = "needs a live WSA; run with `cargo test --release --bin wsadash -- --ignored`"]
    async fn probe_against_the_live_subsystem() {
        let paths = match crate::wsa::app::wsa_path::resolve() {
            Ok(paths) => paths,
            Err(err) => {
                eprintln!("skipping: WSA is not installed ({err:?})");
                return;
            }
        };
        let dash = dashboard_for("adb", paths);

        match dash.probe().await {
            Err(OpsError::Adb(ExecError::NotFound)) => {
                eprintln!("skipping: adb is not on PATH");
            }
            other => {
                let status = other.expect("probe must answer even when WSA is stopped");
                eprintln!(
                    "live probe: state={:?} port={} boot_completed={}",
                    status.state, status.adb_port, status.boot_completed
                );
            }
        }
    }

    /// `ps -A` against the live subsystem: prove the decoder's shape matches
    /// what Android actually prints, by launching a real app and watching it
    /// appear.
    ///
    /// Verified by hand first (2026-10-11): the command is supported, prints
    /// 75 lines including the header, and a launched app shows up as the last
    /// column.
    #[tokio::test]
    #[ignore = "needs a live WSA; run with `cargo test --release --bin wsadash -- --ignored`"]
    async fn running_apps_sees_a_real_running_app() {
        let paths = match crate::wsa::app::wsa_path::resolve() {
            Ok(paths) => paths,
            Err(err) => {
                eprintln!("skipping: WSA is not installed ({err:?})");
                return;
            }
        };
        let dash = dashboard_for(crate::adb::path::adb_path(), paths);

        match dash.probe().await {
            Err(OpsError::Adb(ExecError::NotFound)) => {
                eprintln!("skipping: adb is not on PATH");
                return;
            }
            Ok(status) if !status.is_ready() => {
                eprintln!("skipping: WSA is {:?}", status.state);
                return;
            }
            _ => {}
        }

        let apps = dash.list_apps(false).await.expect("list apps");
        let Some(target) = apps.first() else {
            eprintln!("skipping: no third-party app installed");
            return;
        };
        eprintln!("launching {} to watch it appear in `ps -A`", target.package);

        dash.launch_app(&target.package).await.expect("launch");
        let mut seen = false;
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if dash.running_apps().await.contains(&target.package) {
                seen = true;
                break;
            }
        }
        let _ = dash.stop_app(&target.package).await;

        assert!(
            seen,
            "`ps -A` never listed {} as running -- the last column is not the process name",
            target.package
        );
    }
}
