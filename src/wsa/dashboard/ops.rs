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
    parse_process_list, parse_props, subsystem_status,
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
    /// The request needed a registered WSA package and there is not one -- so
    /// there was nothing to act on, before any command ran.
    ///
    /// Its own variant rather than a `Rejected` string because the two mean
    /// opposite things to a user: `Rejected` is "the operation ran and did not
    /// do what you asked", this is "there is nothing here to ask". Folding it
    /// into `Activate` would be worse still, since `Activate` means Windows
    /// refused a URI we did try to open.
    NotInstalled(String),

    /// The request was malformed, so no command was run at all.
    ///
    /// Its own variant for the same reason `NotInstalled` has one: `Rejected`
    /// documents itself as "the operation ran and did not do what you asked",
    /// and this is the opposite -- nothing was attempted. Folding it in would
    /// make the two indistinguishable to a caller that wants to tell the user
    /// "that is not a proxy" rather than "adb refused".
    Invalid(String),
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
            Self::NotInstalled(detail) => write!(f, "{detail}"),
            Self::Invalid(detail) => write!(f, "{detail}"),
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

/// One row of `adb devices`: what a target is called, and how it answers.
///
/// `state` is a `String` and not an enum. Adb invents states -- `authorizing`,
/// `no permissions`, `recovery` -- and the device page has to show the machine's
/// own words; collapsing them here would lose the one thing the page exists to
/// tell you. The three states the *home page* cares about are decided in
/// [`find_entry`], where that decision already lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbTarget {
    pub serial: String,
    pub state: String,
}

/// Every target in `adb devices` output, in order.
///
/// The filter over the daemon's chatter and the header is the same one
/// [`find_entry`] has always used, and deliberately shared: two filters over
/// the same text is how the home page and the device page end up describing
/// the same machine differently -- `find_entry_agrees_with_parse_targets_about_every_state`
/// exists to keep that from being a thing that happens quietly.
///
/// A malformed row is skipped rather than guessed at. `adb devices` prints the
/// header, an empty line, and the daemon's `*`-prefixed progress; a line with
/// fewer than two fields is none of those and not a device either.
pub fn parse_targets(devices_stdout: &str) -> Vec<AdbTarget> {
    let mut out = Vec::new();
    for line in devices_stdout.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('*') || line.starts_with("List of devices") {
            continue;
        }
        let mut fields = line.split_whitespace();
        let (Some(serial), Some(state)) = (fields.next(), fields.next()) else {
            continue;
        };
        out.push(AdbTarget {
            serial: serial.to_string(),
            state: state.to_string(),
        });
    }
    out
}

/// Does this state mean there is a session with the target?
///
/// One line, and worth being one line: the home page decides "our endpoint is
/// ready" through [`find_entry`] and the device page decides "this row can be
/// disconnected" through [`Self::adb_targets`], and two copies of a string
/// comparison spread over two files is how a future adb state ends up counted
/// as connected on one page and not the other. `authorizing` and anything else
/// invented after this was written are not sessions -- `device` is the only
/// state in which adb will accept a command against the target.
pub fn is_session_state(state: &str) -> bool {
    state == "device"
}

/// Locate `127.0.0.1:<port>` in `adb devices` output.
///
/// Serial match rather than "any line": `adb devices` lists every host the
/// server knows about, and taking the first state found would report a
/// phone plugged in over USB as a ready subsystem.
fn find_entry(devices_stdout: &str, port: u16) -> AdbEntry {
    let expected = format!("127.0.0.1:{port}");

    let Some(target) = parse_targets(devices_stdout)
        .into_iter()
        .find(|target| target.serial == expected)
    else {
        return AdbEntry::Absent;
    };

    if is_session_state(&target.state) {
        return AdbEntry::Device;
    }
    match target.state.as_str() {
        "unauthorized" => AdbEntry::Unauthorized,
        // `offline`, `authorizing` and anything a future platform-tools
        // invents are all "an entry exists but there is no session".
        _ => AdbEntry::Offline,
    }
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

/// Turn a result into an `Ok` or an explained `Err`.
///
/// **Not** [`require_success`], and the difference is the whole point of this
/// function. `adb install` and `adb uninstall` print a literal `Success` line, and
/// a zero exit without it is how "printed `Failure [...]` while returning 0"
/// surfaces -- so for those two, exit code alone would be a lie.
/// `adb forward` / `adb reverse` are the opposite: they succeed **silently**, and
/// exit 0 with no output is success. Asking them for a `Success` line makes every
/// successful call look like `adb exited 0 without reporting success`, which is
/// exactly what the first version of the forward round-trip test reported.
///
/// So: exit code decides, and whatever adb *did* print becomes the error text
/// when the exit code is non-zero.
fn silent_success(out: CommandOutput) -> Result<(), OpsError> {
    if out.exit_code == 0 {
        return Ok(());
    }
    let detail = if !out.stderr.trim().is_empty() {
        out.stderr.trim().to_string()
    } else if !out.stdout.trim().is_empty() {
        out.stdout.trim().to_string()
    } else {
        format!("adb exited {}", out.exit_code)
    };
    Err(OpsError::Rejected(detail))
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

/// The URI that opens WSA's own Settings app (spec 2 line 223).
///
/// `shell:AppsFolder\<aumid>`, handed to `ShellExecute` as a single URI -- not
/// an `explorer` process with arguments, because `explorer.exe shell:...`
/// happens to work and is not the mechanism: what the shell namespace resolves
/// is the package registration.
///
/// `None` for a blank AUMID, and the reason is worth spelling out. Bare
/// `shell:AppsFolder\` opens as an ordinary *folder*: a user who clicked
/// Settings on a machine where WSA is not installed would get a blank Explorer
/// window and no message at all, which reads as a broken WSA rather than as
/// this program having no package to open. So it refuses, and the op turns that
/// into words.
fn settings_uri(aumid: &str) -> Option<String> {
    let aumid = aumid.trim();
    if aumid.is_empty() {
        None
    } else {
        Some(format!("shell:AppsFolder\\{aumid}"))
    }
}

/// What Android wants written to `http_proxy` to turn the proxy **off**.
///
/// `:0`, not an empty string. Measured on this machine: with nothing configured
/// every one of the three globals answers the literal `null`, and writing `:0`
/// is what leaves all three answering `:0` afterwards. An empty value is not
/// what the platform uses for "off", so writing one is writing a value nothing
/// recognises. The constant exists so the op and the test cannot drift into
/// different "off" values.
pub const CLEARED_PROXY: &str = ":0";

/// Globals that have never been written read back as this literal, not as an
/// empty line (measured).
pub const UNSET_PROXY_GLOBAL: &str = "null";

/// `<host>:<port>` from two `settings` reads, or `None` for "not set".
///
/// Absence has three spellings on this platform and all three mean "off":
///
/// - `null` -- the global has never been written (measured).
/// - `:0` -- the value this module writes to turn the proxy off, read back
///   through (measured).
/// - empty -- what a shell that strips the reply gives (measured on the fake,
///   and the safe assumption about any host that trims differently).
///
/// A host with no port is `None` too, and deliberately: Android accepts
/// `settings put global http_proxy 127.0.0.1` and then ignores it, so treating
/// it as a value would write a setting that looks applied and is not.
pub fn http_proxy_value(host: &str, port: &str) -> Option<String> {
    let host = host.trim();
    let port = port.trim();
    if is_unset_proxy_global(host) || is_unset_proxy_global(port) {
        return None;
    }
    Some(format!("{host}:{port}"))
}

/// One global's reply meaning "no proxy configured".
fn is_unset_proxy_global(value: &str) -> bool {
    value.is_empty() || value == UNSET_PROXY_GLOBAL || value == CLEARED_PROXY
}

/// One port mapping, either direction.
///
/// `serial` is opaque and is *not* parsed: `adb forward --list` puts
/// `127.0.0.1:58526` there and `adb reverse --list` puts `host-16`, so a type
/// that tried to read a port out of it would have to be wrong half the time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortMapping {
    pub serial: String,
    pub local: u16,
    pub remote: u16,
}

/// `adb forward --list` / `adb reverse --list`, both of the shape
/// `<serial> tcp:<local> tcp:<remote>`.
///
/// One routine for both, and the two public names are aliases rather than
/// wrappers over different logic -- they are the same format, and a parser that
/// treated `reverse` as a special case would be a parser with two chances to be
/// wrong about the same grammar.
pub fn parse_forward_list(stdout: &str) -> Vec<PortMapping> {
    parse_port_mappings(stdout)
}

pub fn parse_reverse_list(stdout: &str) -> Vec<PortMapping> {
    parse_port_mappings(stdout)
}

/// A line is kept only when all three columns are present and both ports
/// parse. A line that does not parse is dropped rather than aborting the read:
/// one malformed row must not empty the page.
fn parse_port_mappings(stdout: &str) -> Vec<PortMapping> {
    let mut rules = Vec::new();
    for line in stdout.lines() {
        let mut columns = line.split_whitespace();
        let (Some(serial), Some(local), Some(remote)) =
            (columns.next(), columns.next(), columns.next())
        else {
            continue;
        };
        let (Some(local), Some(remote)) = (tcp_port(local), tcp_port(remote)) else {
            continue;
        };
        rules.push(PortMapping {
            serial: serial.to_owned(),
            local,
            remote,
        });
    }
    rules
}

/// `tcp:18081` -> `18081`. Anything else, including a bare number, is `None`:
/// the prefix is what distinguishes a mapping from some other kind of row, and
/// accepting a bare number would let a future adb version's output through as a
/// rule with a different meaning.
fn tcp_port(column: &str) -> Option<u16> {
    column.strip_prefix("tcp:")?.parse().ok()
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
            // state this method was going to report anyway. The prefix is
            // assembled here rather than in `argv` because *this* caller is
            // the one that means "our own endpoint"; the device page passes
            // whatever target it was handed.
            let _ = self
                .run(WsaCommand::ConnectAdb {
                    target: format!("127.0.0.1:{port}"),
                })
                .await;
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

    /// Every target adb knows about, with the state it is answering in.
    ///
    /// **Reads nothing else and reconnects nothing.** Spec 1.4 made
    /// `probe()` reconnect our own endpoint when its row disappears, because
    /// a stopped WSA takes its row off the list and nothing would ever bring
    /// it back -- but this method is on the *device* page, which exists so a
    /// user can see a target and disconnect it. Reconnecting everything the
    /// list knows about would undo what the user just clicked, on the page
    /// where they clicked it. `adb_targets_reads_the_list_and_reconnects_nothing`
    /// grants the fake permission to reconnect and then shows it unused.
    ///
    /// The parsing is `parse_targets`, the same function `find_entry` is built
    /// out of, so the home page and the device page cannot disagree about what
    /// a row means (P4 Review Focus 2).
    pub async fn adb_targets(&self) -> Result<Vec<AdbTarget>, OpsError> {
        let listing = self.run(WsaCommand::ListAdbDevices).await?;
        Ok(parse_targets(&listing.stdout))
    }

    /// Every property the target reports, as `getprop` printed them.
    ///
    /// Bare `adb shell getprop`, exactly as spec section 2.3 lists
    /// `DeviceProps` -- no `-s` prefix, because the command surface has one
    /// shape and every variant carries only its own arguments. With a single
    /// connected target that is the same command; with two, adb refuses and
    /// says why on stderr, which surfaces here as an error rather than as one
    /// target's properties under another's name.
    ///
    /// The whole dump is returned and *filtered by the caller*, not filtered
    /// here: which of these keys are worth a row is a presentation decision
    /// and belongs with the page that shows them (`SHOWN_PROPS` in
    /// `handlers/device.rs`, each key measured against a real `getprop`).
    pub async fn device_props(&self) -> Result<HashMap<String, String>, OpsError> {
        let out = self.run(WsaCommand::GetProps).await?;
        Ok(parse_props(&out.stdout))
    }

    /// Drop a connection: `adb disconnect <target>`.
    ///
    /// Symmetrical with [`Self::connect_target`] -- both hand adb the target
    /// exactly as it appeared in the list, which is the only thing that keeps
    /// "the row the device page is showing" and "the argument adb receives"
    /// the same string.
    pub async fn disconnect_target(&self, target: &str) -> Result<(), OpsError> {
        self.run(WsaCommand::DisconnectAdb {
            serial: target.to_string(),
        })
        .await?;
        Ok(())
    }

    /// Make a connection: `adb connect <target>`.
    ///
    /// Not the same as [`Self::probe`]'s reconnect: that one exists because a
    /// stopped subsystem has its row removed and no one else would put it
    /// back (spec 1.4). This one is a user asking for it.
    pub async fn connect_target(&self, target: &str) -> Result<(), OpsError> {
        self.run(WsaCommand::ConnectAdb {
            target: target.to_string(),
        })
        .await?;
        Ok(())
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

    /// `adb forward tcp:<local> tcp:<remote>` (spec 2 line 224).
    ///
    /// `local` is the **host** port a reader connects to, `remote` the port on
    /// the device.
    ///
    /// **No `WsaOpGuard`, deliberately**, on the same reasoning `stop_app`
    /// gives: spec 2.2 authorises exactly five operation names and a sixth would
    /// need an i18n key the spec never asked for. There is also nothing to
    /// exclude here. The slot exists so two operations cannot corrupt the same
    /// *device* state; a forwarding rule lives in the adb **server's** table,
    /// which adb serialises itself. Two forwards to different ports coexist, and
    /// two forwards to the same port are a replace -- which is what a reader who
    /// double-clicked "Add" meant anyway.
    ///
    /// **No probe first.** This page has to work with the subsystem stopped --
    /// the same reason the disk page does (spec 11 line 348) -- and adb keeps a
    /// forwarding rule across a subsystem stop. Refusing because the device is
    /// down would make the page depend on something it does not need.
    pub async fn forward_add(&self, local: u16, remote: u16) -> Result<(), OpsError> {
        let out = self
            .run(WsaCommand::ForwardAdd { local, remote })
            .await?;
        silent_success(out)
    }

    pub async fn forward_remove(&self, local: u16) -> Result<(), OpsError> {
        let out = self.run(WsaCommand::ForwardRemove { local }).await?;
        silent_success(out)
    }

    pub async fn reverse_add(&self, device: u16, host: u16) -> Result<(), OpsError> {
        let out = self
            .run(WsaCommand::ReverseAdd { device, host })
            .await?;
        silent_success(out)
    }

    pub async fn reverse_remove(&self, device: u16) -> Result<(), OpsError> {
        let out = self.run(WsaCommand::ReverseRemove { device }).await?;
        silent_success(out)
    }

    /// What adb currently has forwarded. Never a `NoData`: an empty listing is
    /// the ordinary state of a machine with no rules, and reporting it as a
    /// failure would put an error on a page that is working correctly.
    pub async fn forward_list(&self) -> Result<Vec<PortMapping>, OpsError> {
        let out = self.run(WsaCommand::ForwardList).await?;
        Ok(parse_forward_list(&out.stdout))
    }

    pub async fn reverse_list(&self) -> Result<Vec<PortMapping>, OpsError> {
        let out = self.run(WsaCommand::ReverseList).await?;
        Ok(parse_reverse_list(&out.stdout))
    }

    /// Point Android at an HTTP proxy (spec 3 line 239).
    ///
    /// Writes **three** globals, through [`WsaCommand::Shell`] rather than a new
    /// variant: they are ordinary `adb shell settings` subcommands, and a variant
    /// per `settings put` would put the whole settings namespace in the command
    /// table.
    ///
    /// **Why three, measured on this machine:** the platform keeps them
    /// *independently* and derives nothing between them. Writing
    /// `http_proxy 10.0.2.2:8081` leaves `http_proxy_host` and `http_proxy_port`
    /// answering `:0`; writing `http_proxy_host` + `http_proxy_port` leaves
    /// `http_proxy` answering `:0`. So whichever one is written alone leaves the
    /// other two saying "off" -- a state where the Settings app and the system's
    /// own proxy tracker disagree about whether there is a proxy.
    ///
    /// **This corrects spec 1 line 116**, which recorded that
    /// `global_http_proxy_host` / `_port` read back consistently after a
    /// combined write. They do not, on this build. See MAPPING 14.
    ///
    /// **Refuses a host with no port** rather than writing it, because Android
    /// accepts that and then ignores it.
    pub async fn set_http_proxy(&self, host: &str, port: &str) -> Result<(), OpsError> {
        let Some(value) = http_proxy_value(host, port) else {
            return Err(OpsError::Invalid(format!(
                "a proxy needs both a host and a port (got host={host:?}, port={port:?})"
            )));
        };
        self.put_global("http_proxy", &value).await?;
        self.put_global("http_proxy_host", host.trim()).await?;
        self.put_global("http_proxy_port", port.trim()).await
    }

    /// Turn the proxy off, by putting the platform's own off value into all three
    /// globals. See [`set_http_proxy`] for why all three, and [`CLEARED_PROXY`]
    /// for why `:0` rather than an empty value.
    pub async fn clear_http_proxy(&self) -> Result<(), OpsError> {
        for name in ["http_proxy", "http_proxy_host", "http_proxy_port"] {
            self.put_global(name, CLEARED_PROXY).await?;
        }
        Ok(())
    }

    /// The proxy Android is currently configured with, or `""`.
    ///
    /// Reads the **split pair**, which is what the page's two fields edit, and
    /// not the combined string. Reading the combined one would put the
    /// splitting rule here and nowhere else, and would disagree with what
    /// [`set_http_proxy`] last wrote whenever an older tool left the two out of
    /// step -- which the platform permits, as measured.
    pub async fn get_http_proxy(&self) -> Result<String, OpsError> {
        let host = self.read_global("http_proxy_host").await?;
        let port = self.read_global("http_proxy_port").await?;
        Ok(http_proxy_value(&host, &port).unwrap_or_default())
    }

    /// `adb shell settings put global <name> <value>`.
    async fn put_global(&self, name: &str, value: &str) -> Result<(), OpsError> {
        let out = self
            .run(WsaCommand::Shell {
                args: vec![
                    "settings".into(),
                    "put".into(),
                    "global".into(),
                    name.into(),
                    value.into(),
                ],
            })
            .await?;
        silent_success(out)
    }

    /// `adb shell settings get global <name>`, trimmed. Never an error for an
    /// absent global: Android answers `null` and exits 0, and this is called on
    /// a page that has to render with nothing configured.
    async fn read_global(&self, name: &str) -> Result<String, OpsError> {
        let out = self
            .run(WsaCommand::Shell {
                args: vec!["settings".into(), "get".into(), "global".into(), name.into()],
            })
            .await?;
        Ok(out.stdout.trim().to_owned())
    }

    /// Open WSA's own Settings app (spec 2 line 223).
    ///
    ///
    /// **No guard.** It activates a Windows shell namespace, not an Android
    /// operation: nothing here touches the subsystem's state, so there is no
    /// slot for it to hold and nothing for it to exclude. Spec 2.2's five
    /// guarded names are all operations against the subsystem, and borrowing
    /// `LAUNCHING` for this would make the busy toast claim something was
    /// running that was not.
    ///
    /// **No `WsaCommand` variant either**, for the reason `command.rs`'s own
    /// module comment gives: this shells out to the shell namespace rather than
    /// to adb, so it has no argv and never reaches the executor.
    pub async fn open_settings(&self) -> Result<(), OpsError> {
        let uri = settings_uri(&self.paths.settings_aumid).ok_or_else(|| {
            OpsError::NotInstalled(
                "WSA is not installed, so it has no settings app to open (no Appx package \
                 registered for this user)."
                    .to_owned(),
            )
        })?;
        activate(&uri)
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
if "%~1"=="disconnect" goto disconnect
if "%~1"=="install" goto op
if "%~1"=="uninstall" goto op
if "%~1"=="forward" goto mapping
if "%~1"=="reverse" goto mapping
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

:disconnect
rem Written rather than acted on: a fake has no connection to drop, and the
rem thing under test is that the serial reached adb byte for byte.
echo %~2>"%S%disconnected.txt"
exit /b 0

:op
if exist "%S%op.delay" ping -n 2 127.0.0.1 >nul
type "%S%op.stdout"
type "%S%op.stderr" 1>&2
set "CODE=1"
if exist "%S%op.exit" set /p CODE=<"%S%op.exit"
call :apply %~1 %~2
exit /b %CODE%

rem -- the loop itself -------------------------------------------------------
rem Accepting both commands was not enough: :op answered install and uninstall
rem from the same fixture, so `list_apps` could never tell the difference and
rem the round trip had nothing to observe. A failed operation changes nothing
rem (hence the code check), and `install -r` on a package already present keeps
rem exactly one row rather than adding a second -- what a real `pm list
rem packages` does.
rem
rem Written as jumps rather than as an `if ( ... )` block for one reason: a
rem `set` inside parentheses is *not* visible to the `%VAR%`s in that same
rem block, which are expanded when the whole block is parsed. The first version
rem read an empty NEWPKG, matched "any line containing package:" against it, and
rem silently added nothing -- a fake that accepts the command, reports success,
rem and changes no state, which is worse than one that reports failure.
:apply
if not "%CODE%"=="0" goto :eof
if "%~1"=="uninstall" goto :uninstall
if not "%~1"=="install" goto :eof
if not exist "%S%install.package" goto :eof
set "NEWPKG="
set /p NEWPKG=<"%S%install.package"
if "%NEWPKG%"=="" goto :eof
findstr /l /c:"package:%NEWPKG%" "%S%packages.txt" >nul 2>&1
if errorlevel 1 echo package:%NEWPKG%>>"%S%packages.txt"
goto :eof

:uninstall
findstr /v /l /c:"package:%~2" "%S%packages.txt" >"%S%packages.tmp" 2>nul
move /y "%S%packages.tmp" "%S%packages.txt" >nul
goto :eof

rem -- adb port mappings (P5b) ----------------------------------------------
rem `forward` and `reverse` share this because adb's two verbs share a grammar:
rem `add tcp:<a> tcp:<b>`, `--remove tcp:<a>`, `--list`. One implementation for
rem both is also the honest one -- a fake that modelled them separately would be
rem testing two different fakes.
rem
rem The serial column is written as adb writes it: `forward` names the device by
rem `host:port`, `reverse` by the literal `host-N`. The parser carries it
rem opaquely, so the fake does not have to reproduce which is which -- only the
rem shape, which is what the parser actually reads.
rem
rem Every step is a jump rather than an `if ( ... )` block, for the same reason
rem `:apply` is: a `set` inside parentheses is not visible to the `%VAR%`s in
rem that same block, which are expanded when the whole block is parsed.
:mapping
set "VERB=%~1"
set "TBL=%S%%VERB%.txt"
if "%~2"=="--list" goto :maplist
if "%~2"=="--remove" goto :mapdel
goto :mapadd

:maplist
type "%TBL%"
exit /b 0

rem `--remove` is `%~2` and the port is `%~3`; a plain `add` has the port in
rem `%~2` and the remote in `%~3`. Matching on the wrong slot removes nothing and
rem says nothing, which is what the first version did.
:mapdel
findstr /v /l /c:"%~3 " "%TBL%" >"%TBL%.tmp" 2>nul
move /y "%TBL%.tmp" "%TBL%" >nul
exit /b 0

rem Adding over an existing local port replaces it, which is what adb does and
rem what `re_forwarding_the_same_local_port_replaces_rather_than_duplicates`
rem pins. `%~2` is `tcp:<local>` here, so the match is on that whole token.
:mapadd
findstr /v /l /c:"%~2 " "%TBL%" >"%TBL%.tmp" 2>nul
>>"%TBL%.tmp" echo %~1 %~2 %~3
move /y "%TBL%.tmp" "%TBL%" >nul
exit /b 0

:shell
if "%~2"=="getprop" goto getprop
if "%~2"=="ps" goto ps
if "%~2"=="pm" goto pm
if "%~2"=="dumpsys" goto dumpsys
if "%~2"=="am" goto am
if "%~2"=="settings" goto settings
echo fake-adb: no rule for shell %* 1>&2
exit /b 1

rem `settings get|put global <name> [value]`. Two files, host and port, because
rem that is how Android really keeps it and a fake that stored one combined
rem string would let a bug that dropped the split pass every test here.
:settings
if "%~3"=="get" goto :settings_get
if "%~3"=="put" goto :settings_put
echo fake-adb: no rule for shell settings %* 1>&2
exit /b 1

rem Absent file answers the literal `null`, which is what Android answers for a
rem global that has never been written (measured). An earlier version also
rem answered the combined `http_proxy` by reading both files into variables
rem inside a parenthesised block -- which prints `::` forever, because a `set`
rem in a block is not visible to the `%VAR%`s in that same block.
:settings_get
if "%~5"=="http_proxy_host" goto :get_host
if "%~5"=="http_proxy_port" goto :get_port
if "%~5"=="http_proxy" goto :get_combined
echo null
exit /b 0

:get_host
if exist "%S%proxy.host" (type "%S%proxy.host") else (echo null)
exit /b 0

:get_port
if exist "%S%proxy.port" (type "%S%proxy.port") else (echo null)
exit /b 0

:get_combined
if exist "%S%proxy.combined" (type "%S%proxy.combined") else (echo null)
exit /b 0

rem All three globals are stored independently and derive nothing from each
rem other, because that is how the platform behaves (measured): writing
rem `http_proxy` alone leaves host and port at `:0`. A fake that kept them in
rem step would let a regression that dropped one of the three writes pass here.
:settings_put
if "%~5"=="http_proxy" goto :put_proxy
if "%~5"=="http_proxy_host" goto :put_host
if "%~5"=="http_proxy_port" goto :put_port
echo fake-adb: no rule for shell settings put %* 1>&2
exit /b 1

:put_proxy
>"%S%proxy.host" echo %~6
>"%S%proxy.combined" echo %~6
exit /b 0

:put_host
>"%S%proxy.host" echo %~6
exit /b 0

:put_port
>"%S%proxy.port" echo %~6
exit /b 0

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
                // CRLF, and normalised rather than appended.
                //
                // `goto <label>` finds labels in an LF-only batch file, which is
                // why nothing complained for six months -- but `call <label>`
                // does not: it reports "系统找不到指定的批处理标签" and carries on,
                // so the first `call` added here produced a fake that accepted
                // the command, reported success, and changed no state. The error
                // lands in the wrong stream to notice, and the round-trip test
                // was the only thing that could see it. Normalising both ways
                // keeps this true no matter how the file was checked out.
                BATCH.replace("@TAB@", "\t").replace("\r\n", "\n").replace('\n', "\r\n"),
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
            // Which package an `install` produces. The fake cannot read an APK
            // manifest, and neither can this list -- the name has to come from
            // somewhere, and an empty file means "this fake was not told", which
            // `:apply` treats as nothing to add rather than as a blank name.
            fake.write("install.package", "");
            // The adb forwarding table, per verb. Empty because a machine with
            // no rules is the ordinary state, and a fake that started with a
            // rule would make `an_empty_listing_is_an_empty_list` untestable
            // without first removing something.
            fake.write("forward.txt", "");
            fake.write("reverse.txt", "");
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

    /// The device page needs *every* target, not just ours.
    ///
    /// Adb's own chatter is dropped by the same rule `find_entry` uses, because
    /// two filters over the same text is how "offline" and "authorizing" start
    /// being read differently in two places (P4 Review Focus 2).
    #[test]
    fn parse_targets_lists_every_target_and_ignores_the_daemons_chatter() {
        let out = "\
* daemon not running; starting now at tcp:5037\n\
* daemon started successfully\n\
List of devices attached\n\
127.0.0.1:58526\tdevice product:sdk model:Pixel\n\
192.168.1.5:5555\toffline\n\
R5CT30XXXXX\tunauthorized\n\
\n";

        let targets = parse_targets(out);
        assert_eq!(
            targets
                .iter()
                .map(|t| (t.serial.as_str(), t.state.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("127.0.0.1:58526", "device"),
                ("192.168.1.5:5555", "offline"),
                ("R5CT30XXXXX", "unauthorized"),
            ]
        );
    }

    /// One parser, two callers.
    ///
    /// `find_entry` answers the home page ("is *our* endpoint ready") and
    /// `parse_targets` answers the device page ("what is out there"), and if
    /// they disagreed about what a line means the two pages would describe the
    /// same machine differently. So `find_entry` is expressed in terms of
    /// `parse_targets` and this test pins that: every state adb can report
    /// classifies the same way whether you arrived at it through either door.
    #[test]
    fn find_entry_agrees_with_parse_targets_about_every_state() {
        for state in ["device", "offline", "unauthorized", "authorizing", "no permissions"] {
            let out = devices(&[("127.0.0.1:58526", state)]);
            let from_list = parse_targets(&out)
                .into_iter()
                .find(|t| t.serial == format!("127.0.0.1:{}", PORT))
                .map(|t| t.state);
            let direct = find_entry(&out, PORT);

            match direct {
                AdbEntry::Absent => assert_eq!(from_list, None, "absent but the list had it"),
                AdbEntry::Device => assert_eq!(from_list.as_deref(), Some("device")),
                AdbEntry::Unauthorized => assert_eq!(from_list.as_deref(), Some("unauthorized")),
                AdbEntry::Offline => assert!(
                    from_list.is_some_and(|s| s != "device" && s != "unauthorized"),
                    "{state} should be 'offline' to find_entry and an unmatched state to the list"
                ),
            }
        }
    }

    #[test]
    fn parse_targets_of_empty_output_is_empty() {
        assert!(parse_targets("List of devices attached\n\n").is_empty());
        assert!(parse_targets("").is_empty());
    }

    /// The device page is for *looking at* targets, and one of the two things
    /// that must not happen behind the user's back is a reconnect.
    ///
    /// `allow_connect()` is deliberately given before the read: permission to
    /// reconnect is granted and then shown not to be used. Without it the test
    /// could pass while `adb_targets` still called `adb connect`, because the
    /// fake would have refused the call for a completely different reason.
    #[tokio::test]
    async fn adb_targets_reads_the_list_and_reconnects_nothing() {
        let adb = FakeAdb::new();
        adb.write(
            "devices.txt",
            &devices(&[
                ("127.0.0.1:58526", "device"),
                ("192.168.1.5:5555", "offline"),
            ]),
        );
        adb.allow_connect();
        let before = std::fs::read_to_string(adb.path().join("devices.txt")).expect("devices");

        let targets = adb.dashboard().adb_targets().await.expect("targets");

        let after = std::fs::read_to_string(adb.path().join("devices.txt")).expect("devices");
        assert_eq!(before, after, "reading the target list must not add a row to it");
        assert_eq!(targets.len(), 2, "both targets, in order");
        assert_eq!(targets[1].serial, "192.168.1.5:5555");
        assert_eq!(targets[1].state, "offline");
    }

    /// The other half of that: reconnecting is something the *user* asks for,
    /// and it has to reach adb verbatim. The fake appends exactly what it was
    /// handed, so a `127.0.0.1:` prefix added where it was not asked for shows
    /// up here as a row with the wrong serial.
    #[tokio::test]
    async fn connect_target_hands_the_whole_target_to_adb() {
        let adb = FakeAdb::new();
        adb.write("devices.txt", &devices(&[]));
        adb.allow_connect();

        adb.dashboard()
            .connect_target("192.168.1.5:5555")
            .await
            .expect("connect");

        let after = std::fs::read_to_string(adb.path().join("devices.txt")).expect("devices");
        assert!(after.contains("192.168.1.5:5555\tdevice"), "got: {after}");
        assert!(
            !after.contains("127.0.0.1:192.168.1.5"),
            "the loopback prefix from the home page's endpoint belongs to its caller, not here"
        );
    }

    #[tokio::test]
    async fn disconnect_target_hands_the_serial_to_adb() {
        let adb = FakeAdb::new();
        adb.write(
            "devices.txt",
            &devices(&[("192.168.1.5:5555", "offline"), ("R5CT30XXXXX", "device")]),
        );

        adb.dashboard()
            .disconnect_target("192.168.1.5:5555")
            .await
            .expect("disconnect");

        let asked = std::fs::read_to_string(adb.path().join("disconnected.txt")).expect("record");
        assert_eq!(asked.trim(), "192.168.1.5:5555");
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

    /// Spec 2 line 223: `explorer shell:AppsFolder\…!SettingsApp`. The
    /// single-URI form, because that is what `ShellExecute` wants -- there is
    /// no `explorer` process to spawn and hand arguments to.
    #[test]
    fn settings_uri_is_the_apps_folder_registration_not_a_wsa_scheme() {
        assert_eq!(
            settings_uri("MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe!SettingsApp")
                .unwrap(),
            "shell:AppsFolder\\MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe!SettingsApp"
        );
        assert!(!settings_uri("X!SettingsApp").unwrap().contains("wsa://"));
    }

    /// An empty AUMID would otherwise produce `shell:AppsFolder\`, which
    /// Explorer opens as a *folder* -- a blank-looking window with no error,
    /// which reads as a broken WSA rather than as "we never found WSA".
    #[test]
    fn settings_uri_refuses_an_empty_aumid() {
        assert!(settings_uri("").is_none());
        assert!(settings_uri("   ").is_none());
        assert!(settings_uri("X!SettingsApp").is_some());
    }

    /// WSA is not installed, so there is no Settings app to open. The op has to
    /// say so rather than hand Explorer a registration that does not exist --
    /// and it has to say so *without* launching anything, which is why this is
    /// worth a test at all: `open::that` on a real machine opens a window, and a
    /// test that does that is worse than no test.
    #[tokio::test]
    async fn opening_settings_without_a_resolved_package_refuses() {
        let paths = WsaPaths {
            settings_aumid: String::new(),
            ..paths_fixture()
        };
        let dash = dashboard_for("adb", paths);
        let err = dash
            .open_settings()
            .await
            .expect_err("no aumid means nothing to open");
        assert!(err.to_string().contains("settings"), "{err}");
    }

    // --- adb port mappings (P5b) --------------------------------------------
    //
    // The stand-in adb gains a real `forward`/`reverse` branch backed by a list
    // file, so the six operations can be exercised as a loop -- add, list,
    // remove, list -- the way the VHDX round trip was. Without it every test
    // here would assert "adb was called" and prove nothing about whether the
    // two halves agree.

    /// `adb forward --list` prints `<serial> tcp:<local> tcp:<remote>`; a rule
    /// that does not parse is dropped rather than aborting the read, because one
    /// malformed line must not empty the page.
    #[test]
    fn forward_list_parses_the_measured_shape_and_drops_junk() {
        let rules = parse_forward_list(
            "127.0.0.1:58526 tcp:18081 tcp:18081\n\
             garbage line with no ports\n\
             \n\
             host-16 tcp:18080 tcp:9999\n",
        );
        assert_eq!(rules.len(), 2, "{rules:?}");
        assert_eq!(rules[0].serial, "127.0.0.1:58526");
        assert_eq!(rules[0].local, 18081);
        assert_eq!(rules[0].remote, 18081);
        assert_eq!(rules[1].local, 18080);
        assert_eq!(rules[1].remote, 9999);
    }

    /// `reverse` puts a *host name* in the column where `forward` puts
    /// `serial:port`. A parser that expected a serial with a colon would drop
    /// every reverse rule and the page would show an empty list while rules
    /// exist -- the failure this pins is not hypothetical, it is the reason the
    /// serial column is carried as an opaque string rather than parsed.
    #[test]
    fn reverse_list_accepts_a_host_name_in_the_serial_column() {
        let rules = parse_reverse_list("host-16 tcp:18080 tcp:18080\n");
        assert_eq!(rules.len(), 1, "{rules:?}");
        assert_eq!(rules[0].serial, "host-16");
        assert_eq!(rules[0].local, 18080);
        assert_eq!(rules[0].remote, 18080);
    }

    /// An empty listing is the normal state, not a failure.
    #[test]
    fn an_empty_listing_is_an_empty_list() {
        assert!(parse_forward_list("").is_empty());
        assert!(parse_forward_list("\n\n").is_empty());
    }

    /// A port that is not a number must not become 0: a rule on port 0 is not a
    /// rule, and rendering one is how a page offers to delete something that
    /// does not exist.
    #[test]
    fn a_port_that_is_not_a_number_is_dropped_not_zeroed() {
        let rules = parse_forward_list("serial tcp:abc tcp:18081\nserial tcp:80 tcp:18081\n");
        assert_eq!(rules.len(), 1, "{rules:?}");
        assert_eq!(rules[0].local, 80);
    }

    // --- HTTP proxy (P5b) ----------------------------------------------------
    //
    // Spec 1 line 116 records that `settings put global http_proxy <host:port>`
    // is writable on this subsystem and that `global_http_proxy_host` /
    // `_port` read back consistently. Spec 3 line 239 makes this the network
    // page's second tab, *replacing* the reference's approach -- the reference
    // exports an `HTTP_PROXY` environment variable into a WSL shell, which has
    // no meaning for an Android subsystem.

    /// Measured on this machine with no proxy configured: all three reads come
    /// back the literal `null`, not an empty line. Reading `null` as a host
    /// would render the page as "proxy: null" instead of "not set", so the
    /// literal has to be recognised.
    ///
    /// `:0` is the third spelling of "off" -- the value this module writes when
    /// clearing, read straight back through. The first version of this function
    /// knew only `null` and empty, so a proxy the dashboard had just turned off
    /// read back as the host `:0` on port `:0`.
    #[test]
    fn all_three_spellings_of_off_are_recognised() {
        assert_eq!(http_proxy_value("null", "null"), None);
        assert_eq!(http_proxy_value("", ""), None);
        assert_eq!(http_proxy_value(":0", ":0"), None, "the off value read back");
        assert_eq!(http_proxy_value("127.0.0.1", "7890").as_deref(), Some("127.0.0.1:7890"));
    }

    /// A host with no port is not a usable proxy either. Android accepts
    /// `settings put global http_proxy 127.0.0.1` and then ignores it, so
    /// writing it would be a setting that silently does nothing.
    #[test]
    fn a_proxy_without_a_port_is_refused_rather_than_written() {
        assert_eq!(http_proxy_value("127.0.0.1", ""), None);
        assert_eq!(http_proxy_value("127.0.0.1", "null"), None);
    }

    /// Turning the proxy off is `settings put global http_proxy :0` on Android,
    /// not an empty value -- an empty value leaves the previous proxy in place,
    /// which is the opposite of what the reader asked for.
    #[test]
    fn clearing_the_proxy_writes_androids_off_value() {
        assert_eq!(CLEARED_PROXY, ":0");
        assert!(http_proxy_value("", "").is_none());
    }

    /// Setting a proxy must write **all three** globals.
    ///
    /// This is the one measurement that changed the design. On this build the
    /// platform stores them independently and derives nothing between them:
    /// writing `http_proxy 10.0.2.2:8081` leaves `http_proxy_host` and
    /// `http_proxy_port` answering `:0`, and the reverse holds too. An
    /// implementation that writes only the combined string leaves the Settings
    /// app showing "no proxy" next to a system that has one; one that writes
    /// only the split pair leaves the system's own proxy tracker with `:0`.
    /// Either half of that disagreement is invisible to a read of a single
    /// global, which is exactly why this asserts the file, not the value.
    #[tokio::test]
    async fn setting_a_proxy_writes_the_combined_and_the_split_globals() {
        let adb = FakeAdb::new();
        adb.dashboard()
            .set_http_proxy("10.0.2.2", "8081")
            .await
            .expect("set");

        let state = adb.path();
        for (file, expected) in [
            ("proxy.combined", "10.0.2.2:8081"),
            ("proxy.host", "10.0.2.2"),
            ("proxy.port", "8081"),
        ] {
            let actual = std::fs::read_to_string(state.join(file))
                .unwrap_or_else(|e| panic!("{file} was never written: {e}"));
            assert_eq!(actual.trim(), expected, "{file}");
        }
    }

    /// And clearing must put the off value into all three, for the same reason:
    /// a global left holding the old host is a proxy the reader cannot switch
    /// off from this page.
    #[tokio::test]
    async fn clearing_writes_the_off_value_into_all_three_globals() {
        let adb = FakeAdb::new();
        let dash = adb.dashboard();
        dash.set_http_proxy("10.0.2.2", "8081").await.expect("set");
        dash.clear_http_proxy().await.expect("clear");

        for file in ["proxy.combined", "proxy.host", "proxy.port"] {
            let actual = std::fs::read_to_string(adb.path().join(file))
                .unwrap_or_else(|e| panic!("{file} vanished instead of being cleared: {e}"));
            assert_eq!(actual.trim(), CLEARED_PROXY, "{file}");
        }
    }

    /// The loop, on the stand-in: set, read back, clear, read back empty. The
    /// stand-in gains a `settings` branch for this, because `settings` is a
    /// `shell` subcommand and the existing fake only modelled five of them.
    #[tokio::test]
    async fn the_proxy_survives_a_set_read_clear_loop() {
        let adb = FakeAdb::new();
        let dash = adb.dashboard();

        dash.set_http_proxy("127.0.0.1", "7890").await.expect("set");
        assert_eq!(
            dash.get_http_proxy().await.expect("read back"),
            "127.0.0.1:7890"
        );

        dash.clear_http_proxy().await.expect("clear");
        assert_eq!(
            dash.get_http_proxy().await.expect("read back after clear"),
            "",
            "cleared but still reports a proxy"
        );
    }

    /// A host with no port must never reach `settings put`, because Android
    /// accepts it and then ignores it -- a setting that looks applied and is
    /// not.
    ///
    /// Also the assertion that the stand-in keeps the two globals **separate**.
    /// The set/read/clear loop above would still pass if the fake stored one
    /// combined string, so it cannot tell "the code asks for both globals" from
    /// "the code asks for one". Writing the host file directly is the only way
    /// to reach `get_http_proxy` in that state, and it answers "no port, so no
    /// proxy" rather than half a proxy.
    #[tokio::test]
    async fn a_host_with_no_port_reads_back_as_no_proxy_not_as_half_of_one() {
        let adb = FakeAdb::new();
        adb.write("proxy.host", "10.0.2.2\n");
        assert_eq!(
            adb.dashboard().get_http_proxy().await.expect("read back"),
            "",
            "a host with no port is not a proxy"
        );

        adb.write("proxy.port", "8080\n");
        assert_eq!(
            adb.dashboard().get_http_proxy().await.expect("read back"),
            "10.0.2.2:8080",
            "the port is kept in its own global, not folded into the host"
        );
    }

    /// Reading a proxy that was never set is not a failure: `null` is what
    /// Android answers, and a page that reports an error for "no proxy
    /// configured" is a page that errors on every fresh install.
    #[tokio::test]
    async fn reading_an_unset_proxy_is_empty_and_not_an_error() {
        let adb = FakeAdb::new();
        assert_eq!(
            adb.dashboard().get_http_proxy().await.expect("unset is readable"),
            ""
        );
    }

    /// A host with no port must never reach `settings put`, because Android
    /// accepts it and then ignores it -- a setting that looks applied and is
    /// not.
    #[tokio::test]
    async fn setting_a_proxy_without_a_port_is_refused() {
        let adb = FakeAdb::new();
        let err = adb
            .dashboard()
            .set_http_proxy("127.0.0.1", "")
            .await
            .expect_err("half a proxy is not a proxy");
        assert!(err.to_string().contains("port"), "{err}");
    }

    /// The whole loop on the stand-in: forward a port, see it listed, remove it,
    /// see it gone. This is the assertion that the two halves agree.
    #[tokio::test]
    async fn a_forward_mapping_survives_the_add_list_remove_loop() {
        let adb = FakeAdb::new();
        let dash = adb.dashboard();

        dash.forward_add(18081, 18081).await.expect("forward_add");
        let listed = dash.forward_list().await.expect("forward_list");
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert_eq!(listed[0].local, 18081);

        dash.forward_remove(18081).await.expect("forward_remove");
        assert!(
            dash.forward_list().await.expect("forward_list again").is_empty(),
            "removed but still listed"
        );
    }

    /// Same loop for `reverse`: it is a different adb verb, and a parser that
    /// had only ever seen `forward --list` would look fine until it did not.
    #[tokio::test]
    async fn a_reverse_mapping_survives_the_add_list_remove_loop() {
        let adb = FakeAdb::new();
        let dash = adb.dashboard();

        dash.reverse_add(18080, 18080).await.expect("reverse_add");
        let listed = dash.reverse_list().await.expect("reverse_list");
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert_eq!(listed[0].local, 18080);

        dash.reverse_remove(18080).await.expect("reverse_remove");
        assert!(
            dash.reverse_list().await.expect("reverse_list again").is_empty(),
            "removed but still listed"
        );
    }

    /// `adb forward` over an existing local port is a *replace*: after it the
    /// list shows one rule with the new remote, not two rules and not the old
    /// remote. The stand-in models this the way adb does.
    #[tokio::test]
    async fn re_forwarding_the_same_local_port_replaces_rather_than_duplicates() {
        let adb = FakeAdb::new();
        let dash = adb.dashboard();

        dash.forward_add(18081, 1111).await.expect("first");
        dash.forward_add(18081, 2222).await.expect("second");

        let listed = dash.forward_list().await.expect("list");
        assert_eq!(listed.len(), 1, "duplicated: {listed:?}");
        assert_eq!(listed[0].remote, 2222);
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

    /// The whole loop, on the stand-in: a package that is not there becomes
    /// there, and then it is not.
    ///
    /// This is the half of spec section 10's P4 acceptance that can be run
    /// without touching a real device. The other half is deliberately not
    /// attempted against one: `adb uninstall` clears the package's data, the
    /// APKs on this machine were counted (zero in three trees), and removing an
    /// installed application in order to photograph it disappearing is a change
    /// to the reader's machine rather than to this program. MAPPING section
    /// 11.8 records that, and the three ways it could have been done instead.
    ///
    /// The fake has to *model* the loop, not merely accept the two commands --
    /// which is the point. `:op` answered install and uninstall from the same
    /// fixture before this, so `list_apps` could never show a difference and
    /// the loop had nothing to observe. The package name for an install comes
    /// from a fixture file rather than from the fake parsing an APK manifest,
    /// because that is what the truth is on a real device too: adb learns the
    /// package from the file, and here the test hands it over.
    #[tokio::test]
    async fn install_then_uninstall_round_trip_through_the_stand_in() {
        let adb = FakeAdb::new();
        adb.write(
            "packages.txt",
            "package:com.topjohnwu.magisk\npackage:com.android.settings\n",
        );
        // What the APK under test declares. The fake cannot read a manifest, so
        // the test says which package the file is.
        adb.write("install.package", "com.example.fresh");
        adb.write("op.stdout", "Performing Streamed Install\nSuccess\n");
        adb.write("op.exit", "0");

        let dashboard = adb.dashboard();

        let before = dashboard.list_apps(false).await.expect("list before");
        assert_eq!(before.len(), 1, "one third-party package: {before:?}");

        dashboard
            .install_apk(Path::new(r"C:\pkgs\com.example.fresh.apk"))
            .await
            .expect("install");

        let middle = dashboard.list_apps(false).await.expect("list after install");
        assert_eq!(middle.len(), 2, "the new package should be there: {middle:?}");
        assert!(
            middle.iter().any(|app| app.package == "com.example.fresh"),
            "installed and then not found: {middle:?}"
        );

        dashboard
            .uninstall_apk("com.example.fresh")
            .await
            .expect("uninstall");

        let after = dashboard.list_apps(false).await.expect("list after uninstall");
        assert_eq!(after.len(), 1, "back to where it started: {after:?}");
        assert!(
            !after.iter().any(|app| app.package == "com.example.fresh"),
            "uninstalled and still there: {after:?}"
        );
        assert!(after.iter().any(|app| app.package == "com.topjohnwu.magisk"));
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
