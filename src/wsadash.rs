// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! CLI entry point (`wsadash`), spec Q19.
//!
//! The reference's verb set has no WSA equivalent -- it is `wsl.exe`'s -- so
//! this file is the one place where nothing is ported from anywhere. What it
//! does take from the reference is its posture: **the exit code is the
//! interface**. `probe` reports the subsystem's state as a number so a script
//! can branch on it, and every other verb reuses those same numbers rather
//! than inventing a second scheme.
//!
//! Deliberately hand-written dispatch. The reference uses no argument parser,
//! and a dependency that exists only to read nine subcommands would be the
//! CLI's sole reason to rebuild.

// The WSA domain layer hangs off this bin for P1 (models, adb, decoders,
// command table, executor). It is deliberately NOT a lib target: a lib would
// force `crate::app::...` to become `wsadashboard::app::...` everywhere and
// break line-by-line diffing against the reference. The GUI bin adds the same
// `mod wsa;` line when it starts consuming these types in P2.
mod adb;
mod wsa;

// The home view model is pure and belongs to `src/ui/`, but the CLI must not
// pull in `data.rs` / `handlers/`, which need the Slint `AppWindow`. One
// `#[path]` gives the CLI the file it tests without the rest of the module
// tree; the GUI bin reaches it through `mod ui;` instead.
#[path = "ui/home_model.rs"]
mod home_model;

#[path = "ui/refresh.rs"]
mod refresh;

// Same reason as `home_model`: `tabs` is a decision with tests, and it was
// written inside `handlers/common.rs` first -- where it would have compiled
// into nothing, because `handlers` is GUI-only. See `src/ui/tabs.rs`.
//
// `cfg(test)` because the CLI has no other use for it: the GUI reaches the same
// file through `mod ui;`, and a non-test `wsadash` build would otherwise
// compile four items it never calls and warn about all four.
#[cfg(test)]
#[path = "ui/tabs.rs"]
mod tabs;

#[path = "i18n/mod.rs"]
mod i18n;

// The storage page's view model, for the same reason: plain Rust in, plain Rust
// out, and its tests have to run in the CLI. It deliberately does not name the
// `StorageImageUI` struct from `theme.slint` -- that one exists only in the GUI
// binary, so naming it here would make the module unmountable and its tests
// would never compile. The conversion happens in `ui/handlers/disk.rs`, exactly
// as `data::row` turns a `home_model::AppRow` into a `Distro`.
//
// `cfg(test)` for the same reason as `tabs`: the CLI's own verbs call
// `storage`, not `disk_model`, so a non-test build would compile the two
// `explorer_arg_for_*` helpers and warn that nothing calls them.
#[cfg(test)]
#[path = "ui/disk_model.rs"]
mod disk_model;

// The network page's view model, for the third time the same reason: it turns
// adb's two `forward` / `reverse` listings into rows, and the decision worth
// testing -- what a rule's `id` must carry for `delete_network_rule` to be able
// to address a rule at all -- is pure. `handlers/network.rs` is GUI-only, so a
// module that named `AppWindow` or `PortProxyRuleUI` would put these eleven
// tests in the one place they can never run.
//
// `cfg(test)`, and for `disk_model`'s reason rather than a different one: the
// CLI's `wsadash` verbs call `storage`, never this, so a non-test build would
// compile three functions nothing calls and warn about each.
#[cfg(test)]
#[path = "ui/network_model.rs"]
mod network_model;

// The scheduler page's view model, for the fourth time the same reason. Two
// functions: the card's shortened command, and the dialog's name/command/
// schedule checks as one function that answers *which* field is wrong rather
// than setting one of three error properties. Both live in the reference's
// `ui/handlers/scheduler.rs`, which is GUI-only.
//
// This file is the one `_model` with no `use crate::...` line at all, and that
// is forced rather than tidy: the CLI mounts it at the crate root and
// `ui/mod.rs` mounts it one level down, so a path is off by a level in one of
// them, and `scheduler_task` -- which the cron half of the validation needed --
// is `crate::scheduler_task` here and `crate::app::scheduler_task` in the GUI.
// The cron half therefore lives in `cron.rs`, where both binaries reach it.
#[cfg(test)]
#[path = "ui/scheduler_model.rs"]
mod scheduler_model;

// The settings store, for the same reason as the two above: the decisions worth
// testing here are "a file that does not parse falls back to defaults" and "a
// retention count is clamped rather than refused", and neither can be reached
// from the GUI binary's test run.
//
// Not `cfg(test)`-only in the way `disk_model` is: `config` is already part of
// the GUI's module tree, so the CLI mount here is the *only* copy it sees, and
// the tests below are the only ones that will ever run for it.
#[cfg(test)]
#[path = "config/mod.rs"]
mod config;

// The update check. Mounted here for its two pure decisions -- reading GitHub's
// release JSON and comparing two version strings -- both of which can be wrong
// in a way that is not an error: an empty tag compares as not-newer and the page
// says "you are up to date", which is exactly what it says when it is right.
#[cfg(test)]
#[path = "api/mod.rs"]
mod api;

// The scheduled-task layer, for the same reason as the two above and `config`:
// "does `*/5 * * * *` become a MINUTE task with a 5-minute interval" is a
// question about arithmetic over five strings, and it has to be answerable by a
// test rather than by creating a task on the reader's machine and looking at it.
//
// Mounted at the top level rather than inside a `mod app { .. }` because
// `#[path]` inside an inline module resolves against the *inline* module's
// directory, which does not exist -- so the path came out as
// `src/app/app/scheduler_task/mod.rs`. The nested `pub mod cron;` declarations
// inside the mounted file do resolve correctly, because for a `#[path]` module
// they are relative to the file's own directory.
#[cfg(test)]
#[path = "app/scheduler_task/mod.rs"]
mod scheduler_task;

// App-level modules with decisions worth pinning, mounted here for the same
// reason as `scheduler_task`: `cargo test --bin wsadash` is the only test run
// this project has, and tests written in `app::startup_info` or `app::autostart`
// sit in the GUI binary where nothing runs them. That is not a hypothetical: the
// first version of `parse_windows_build` split on the English label `"Version "`,
// `ver` on this machine answers in Chinese, and its test would have said `N/A`
// was correct -- while never once being executed.
//
// The files below reach their one shared constant through
// `super::constants::APP_ID` rather than `crate::app::constants::APP_ID`, which
// is why `constants` is mounted beside them here -- that path resolves in the
// GUI (`super` = `app`) and here (`super` = the crate root, where this file puts
// `constants`). The other spelling works in one and not the other, and the error
// names the module that cannot be found rather than the mount that is missing.
#[cfg(test)]
#[path = "app/constants.rs"]
mod constants;

#[cfg(test)]
#[path = "app/startup_info.rs"]
mod startup_info;

// `/scheduler`, this binary's own cold-start switch (P8). Mounted for one
// function: `requested`, which decides whether a run draws a window at all. A
// false positive there is an app that appears to do nothing, so it is the last
// decision in this program that should be untested.
#[cfg(test)]
#[path = "app/immediate.rs"]
mod immediate;

#[cfg(test)]
#[path = "app/autostart.rs"]
mod autostart;

// The tray's menu, for the same reason: the ids in it are matched by `tray.rs`
// and its label keys are looked up in the English table, and both are couplings a
// test can hold. `tray.rs` itself names `AppWindow` and `tray-icon`, so it stays
// in the GUI binary -- the same split as `disk_model` and `handlers/disk.rs`.
#[cfg(test)]
#[path = "app/tray_model.rs"]
mod tray_model;

// `window` and `single_instance`, mounted together because they have to be: the
// one function in `single_instance` that is not a name is
// `try_activate_existing_instance`, and it reaches its window through
// `super::window`. Mounting only the second would leave that path unresolved.
//
// `window.rs` is the one GUI file with no Slint in it -- Win32 window handles
// from top to bottom -- which is what makes it mountable here at all. Everything
// in it is `#[cfg(target_os = "windows")]`, so this is a Windows-only mount, and
// the `#![allow(dead_code)]` at the top of each file is what keeps the shell
// quiet: the tests call the names, not the handles.
//
// The test worth the pair is one design difference with a regression attached:
// the reference finds the running instance's window by trying three *titles*, and
// ours finds it by process id. The title list is asserted empty, which is the only
// thing standing between this and the reference's silent "double-click does
// nothing" failure coming back.
#[cfg(test)]
#[path = "app/window.rs"]
mod window;

#[cfg(test)]
#[path = "app/single_instance.rs"]
mod single_instance;

use adb::path::adb_path;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use wsa::app::storage;
use wsa::app::wsa_path::{self, WsaPathError};
use wsa::dashboard::ops::OpsError;
use wsa::dashboard::{dashboard_for, WsaDashboard};
use wsa::executor::ExecError;
use wsa::models::SubsystemState;

// ---- exit codes --------------------------------------------------------------
//
// A caller must be able to branch on one number no matter which verb it ran,
// so these are constants rather than the incidental values of whatever `main`
// happened to return.

/// The subsystem is ready, or the verb did what it was asked.
const EXIT_OK: u8 = 0;
/// The arguments did not describe an operation.
const EXIT_USAGE: u8 = 1;
/// adb is answering but boot has not finished (spec 5, tier 2).
const EXIT_BOOTING: u8 = 2;
/// No usable endpoint (spec 5, tier 3).
const EXIT_STOPPED: u8 = 3;
/// No WSA package registered, or its manifest is unreadable.
const EXIT_NOT_INSTALLED: u8 = 4;
/// adb itself is missing. Review Focus #1: a remedy, not a crash.
const EXIT_NO_ADB: u8 = 5;
/// The verb ran and failed.
const EXIT_FAILED: u8 = 6;
/// The endpoint needs the key prompt accepted inside WSA.
const EXIT_UNAUTHORIZED: u8 = 7;
/// An endpoint entry exists but no session with it.
const EXIT_OFFLINE: u8 = 8;

fn code(value: u8) -> ExitCode {
    ExitCode::from(value)
}

fn state_code(state: SubsystemState) -> ExitCode {
    code(match state {
        SubsystemState::Ready => EXIT_OK,
        SubsystemState::Booting => EXIT_BOOTING,
        SubsystemState::Stopped => EXIT_STOPPED,
        SubsystemState::NotInstalled => EXIT_NOT_INSTALLED,
        SubsystemState::Unauthorized => EXIT_UNAUTHORIZED,
        SubsystemState::Offline => EXIT_OFFLINE,
    })
}

/// What the caller should do about a state that is not `Ready`.
///
/// Printed on both streams: on stdout as `hint=` for `probe`, where it is
/// part of the machine-readable answer, and on stderr by `require_device`,
/// where it is the whole point of the message.
fn state_hint(state: SubsystemState) -> &'static str {
    match state {
        SubsystemState::NotInstalled => "WSA is not installed",
        SubsystemState::Stopped => "run `wsadash start`",
        SubsystemState::Booting => "still booting; wait a moment",
        SubsystemState::Ready => "",
        SubsystemState::Unauthorized => {
            "the endpoint is unauthorized; accept the adb prompt inside WSA"
        }
        SubsystemState::Offline => "the endpoint is offline; run `wsadash start`",
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    // Warnings go to stderr so `wsadash list | cut -f1` stays a clean pipe.
    // `RUST_LOG` still overrides, because the operator who is chasing a
    // decoder bug should not have to rebuild to raise the level.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(verb) = args.first().map(String::as_str) else {
        // No verb: report the state, which is what a bare invocation is
        // always trying to find out. Read-only, so it is safe to default to.
        return probe().await;
    };
    let rest = &args[1..];

    match (verb, rest) {
        ("probe", []) => probe().await,
        ("list", flags) => list(flags).await,
        ("info", [package]) => info(package).await,
        ("launch", [package]) => launch(package).await,
        ("stop-app", [package]) => stop_app(package).await,
        ("install", [apk]) => install(apk).await,
        ("uninstall", [package]) => uninstall(package).await,
        ("start", []) => start().await,
        ("stop", []) => stop().await,
        ("vhdx", []) => vhdx().await,
        ("-h" | "--help" | "help", _) => {
            usage(&mut std::io::stdout());
            code(EXIT_OK)
        }
        _ => {
            eprintln!("wsadash: unexpected arguments for `{verb}`");
            usage(&mut std::io::stderr());
            code(EXIT_USAGE)
        }
    }
}

fn usage(to: &mut dyn Write) {
    let _ = writeln!(
        to,
        "\
wsadash - drive Windows Subsystem for Android

usage: wsadash <command> [arguments]

commands:
  probe                 report subsystem state (bare `wsadash` does this)
  list [--all]          installed packages; --all includes system ones
  info <package>        one package in detail
  launch <package>      start one Android app
  stop-app <package>    force-stop one Android app
  install <apk>         install an apk
  uninstall <package>   remove a package
  start                 cold-start the subsystem and wait for it
  stop                  shut the subsystem down
  vhdx                  WSA's storage images, read off the disk; needs no device

environment:
  WSADASH_ADB           path to adb (default: `adb` on PATH)

exit codes:
  0  ready / success      3  stopped              6  operation failed
  1  usage                4  not installed        7  unauthorized
  2  booting              5  adb missing          8  offline"
    );
}

// ---- setup -------------------------------------------------------------------

/// Locate WSA and build the facade, or explain why that is impossible.
///
/// Resolving first matters: a missing WSA is a state the caller must be told
/// about before any adb command is attempted, and attempting one first would
/// turn "not installed" into a timeout that reads like a hung machine.
async fn connect() -> Result<WsaDashboard, ExitCode> {
    let paths = match wsa_path::resolve() {
        Ok(paths) => paths,
        Err(WsaPathError::NotInstalled) => {
            eprintln!("wsadash: WSA is not installed (no Appx package registered for this user).");
            return Err(code(EXIT_NOT_INSTALLED));
        }
        Err(WsaPathError::MalformedManifest) => {
            eprintln!(
                "wsadash: WSA's AppxManifest.xml declares no package identity; \
                 the install looks damaged. Reinstall it from the Microsoft Store."
            );
            return Err(code(EXIT_NOT_INSTALLED));
        }
    };
    Ok(dashboard_for(adb_path(), paths))
}

/// Turn an operation failure into the message and the number.
///
/// `ExecError::NotFound` is matched rather than described because the review
/// asked for the remedy by name, and because a missing adb must never reach
/// the panic path: it is a first-run condition, not a bug.
fn ops_failure(error: &OpsError) -> ExitCode {
    match error {
        OpsError::Adb(ExecError::NotFound) => {
            eprintln!(
                "wsadash: adb was not found. Install Android platform-tools with:\n  \
                 winget install Microsoft.PlatformTools\n\
                 or set WSADASH_ADB to an existing adb.exe."
            );
            code(EXIT_NO_ADB)
        }
        other => {
            eprintln!("wsadash: {other}");
            code(EXIT_FAILED)
        }
    }
}

/// Gate for every verb that needs a live endpoint.
///
/// `probe` first, rather than duplicating its reconnect logic here: the CLI
/// and the future UI should decide "can I talk to it?" the same way, in the
/// one place that is tested for it.
///
/// A non-`Ready` answer is not an error, so it is reported with exactly the
/// number `probe` would have used. `wsadash list` on a stopped subsystem then
/// says *stopped* instead of letting adb's own failure become an opaque 6.
async fn require_device(dash: &WsaDashboard) -> Result<(), ExitCode> {
    let status = dash.probe().await.map_err(|err| ops_failure(&err))?;
    if matches!(
        status.state,
        SubsystemState::Ready | SubsystemState::Booting
    ) {
        return Ok(());
    }
    eprintln!(
        "wsadash: the subsystem is {:?} -- {}",
        status.state,
        state_hint(status.state)
    );
    Err(state_code(status.state))
}

// ---- commands ----------------------------------------------------------------

async fn probe() -> ExitCode {
    let dash = match connect().await {
        Ok(dash) => dash,
        Err(exit) => return exit,
    };
    match dash.probe().await {
        Ok(status) => {
            // `key=value` throughout: one line per fact, greppable, and no
            // column widths to break when a port or a package name is long.
            println!("state={:?}", status.state);
            println!("adb_port={}", status.adb_port);
            println!("boot_completed={}", status.boot_completed);
            println!("hint={}", state_hint(status.state));
            state_code(status.state)
        }
        Err(error) => ops_failure(&error),
    }
}

async fn list(flags: &[String]) -> ExitCode {
    let mut include_system = false;
    for flag in flags {
        match flag.as_str() {
            "--all" | "-a" => include_system = true,
            other => {
                eprintln!("wsadash: unknown option for `list`: {other}");
                return code(EXIT_USAGE);
            }
        }
    }

    let dash = match connect().await {
        Ok(dash) => dash,
        Err(exit) => return exit,
    };
    if let Err(exit) = require_device(&dash).await {
        return exit;
    }
    let apps = match dash.list_apps(include_system).await {
        Ok(apps) => apps,
        Err(error) => return ops_failure(&error),
    };

    // No header: the output is meant to be piped. A reply adb could not be
    // understood is already on stderr from `list_apps`, so an empty stdout
    // here never silently claims "no apps installed".
    for app in &apps {
        println!("{}\t{}\t{}", app.package, app.version_name, app.display_name());
    }
    code(EXIT_OK)
}

async fn info(package: &str) -> ExitCode {
    let dash = match connect().await {
        Ok(dash) => dash,
        Err(exit) => return exit,
    };
    if let Err(exit) = require_device(&dash).await {
        return exit;
    }
    match dash.app_info(package).await {
        Ok(app) => {
            println!("package={}", app.package);
            println!("display_name={}", app.display_name());
            println!("version_name={}", app.version_name);
            println!("version_code={}", app.version_code);
            println!("apk_size_bytes={}", app.apk_size_bytes);
            println!("is_system={}", app.is_system);
            code(EXIT_OK)
        }
        Err(error) => ops_failure(&error),
    }
}

async fn launch(package: &str) -> ExitCode {
    let dash = match connect().await {
        Ok(dash) => dash,
        Err(exit) => return exit,
    };
    if let Err(exit) = require_device(&dash).await {
        return exit;
    }
    match dash.launch_app(package).await {
        Ok(()) => {
            // ShellExecute handing back "yes" is not the app being on screen
            // yet; the subsystem needs a moment to bring it forward.
            println!("requested launch of {package}");
            code(EXIT_OK)
        }
        Err(error) => ops_failure(&error),
    }
}

async fn stop_app(package: &str) -> ExitCode {
    let dash = match connect().await {
        Ok(dash) => dash,
        Err(exit) => return exit,
    };
    if let Err(exit) = require_device(&dash).await {
        return exit;
    }
    match dash.stop_app(package).await {
        Ok(()) => {
            println!("stopped {package}");
            code(EXIT_OK)
        }
        Err(error) => ops_failure(&error),
    }
}

async fn install(apk: &str) -> ExitCode {
    let path = Path::new(apk);
    if !path.is_file() {
        eprintln!("wsadash: no such apk: {apk}");
        return code(EXIT_USAGE);
    }
    let dash = match connect().await {
        Ok(dash) => dash,
        Err(exit) => return exit,
    };
    if let Err(exit) = require_device(&dash).await {
        return exit;
    }
    match dash.install_apk(path).await {
        Ok(()) => {
            println!("installed {}", path.display());
            code(EXIT_OK)
        }
        Err(error) => ops_failure(&error),
    }
}

async fn uninstall(package: &str) -> ExitCode {
    let dash = match connect().await {
        Ok(dash) => dash,
        Err(exit) => return exit,
    };
    if let Err(exit) = require_device(&dash).await {
        return exit;
    }
    match dash.uninstall_apk(package).await {
        Ok(()) => {
            println!("uninstalled {package}");
            code(EXIT_OK)
        }
        Err(error) => ops_failure(&error),
    }
}

async fn start() -> ExitCode {
    let dash = match connect().await {
        Ok(dash) => dash,
        Err(exit) => return exit,
    };
    if let Err(error) = dash.start_subsystem().await {
        return ops_failure(&error);
    }
    println!("start requested; waiting for the subsystem");
    wait_until_ready(&dash).await
}

/// Poll until `Ready`, printing each state change as it happens.
///
/// Spec 1.2 measured 12-17s cold, so this is the verb that would otherwise
/// leave a caller staring at an unexplained pause. A transient probe failure
/// mid-boot is not the answer -- the endpoint genuinely appears late -- so it
/// is swallowed until the deadline rather than returned as a verdict.
async fn wait_until_ready(dash: &WsaDashboard) -> ExitCode {
    const DEADLINE: Duration = Duration::from_secs(90);
    const TICK: Duration = Duration::from_millis(500);

    let started = std::time::Instant::now();
    let mut last: Option<SubsystemState> = None;
    loop {
        match dash.probe().await {
            Ok(status) => {
                if last != Some(status.state) {
                    println!("state={:?}", status.state);
                    last = Some(status.state);
                }
                if status.is_ready() {
                    return code(EXIT_OK);
                }
                // The package cannot have vanished mid-boot; if it did, no
                // amount of waiting will change the answer.
                if matches!(status.state, SubsystemState::NotInstalled) {
                    return state_code(status.state);
                }
            }
            Err(error) if matches!(error, OpsError::Adb(ExecError::NotFound)) => {
                // Not installed tooling. Polling would be a 90s way to say so.
                return ops_failure(&error);
            }
            Err(_) => {}
        }

        if started.elapsed() >= DEADLINE {
            eprintln!(
                "wsadash: gave up after {}s waiting for the subsystem; last state {:?}",
                DEADLINE.as_secs(),
                last
                    .map(|state| format!("{state:?}"))
                    .unwrap_or_else(|| "unknown".to_string())
            );
            return code(EXIT_FAILED);
        }
        tokio::time::sleep(TICK).await;
    }
}

async fn stop() -> ExitCode {
    let dash = match connect().await {
        Ok(dash) => dash,
        Err(exit) => return exit,
    };
    match dash.stop_subsystem().await {
        Ok(()) => {
            println!("subsystem stopped");
            code(EXIT_OK)
        }
        Err(error) => ops_failure(&error),
    }
}

/// WSA's storage images, tab-separated, `userdata` first.
///
/// The one command here that does **not** call [`connect`], and that is the
/// point of it rather than an omission: the disk page has to answer on a
/// machine where the subsystem is stopped (spec 11 line 348), and `connect()`
/// resolves the Appx package, which is exactly what fails when WSA is not
/// running. Reading `LocalCache` needs no package and no device.
///
/// Output columns: role, size in bytes, modified, file name, full path.
async fn vhdx() -> ExitCode {
    let Some(dir) = storage::cache_dir() else {
        eprintln!(
            "wsadash: LOCALAPPDATA is not set, so there is no WSA data folder to look in."
        );
        return code(EXIT_NOT_INSTALLED);
    };
    let images = match storage::images_in(&dir) {
        Ok(images) => images,
        Err(err) => {
            eprintln!("wsadash: cannot read {}: {err}", dir.display());
            return code(EXIT_FAILED);
        }
    };
    for image in &images {
        let role = match image.role {
            storage::StorageRole::UserData => "userdata",
            storage::StorageRole::Metadata => "metadata",
        };
        println!(
            "{}\t{}\t{}\t{}\t{}",
            role,
            image.size_bytes,
            image.modified,
            image.file_name,
            image.path.display()
        );
    }
    code(EXIT_OK)
}
