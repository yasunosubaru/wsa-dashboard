// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The first thing the GUI does.
//!
//! The counterpart of the reference's `src/app/launcher.rs`. The reference's
//! `bootstrap` returns an `AppContext` carrying a config manager, the loaded
//! settings, the logging system and (step 3.1) a generated device id. None of
//! that exists here yet -- config lands in P5 and the device id is part of the
//! telemetry the spec does not ask for -- so this returns what the window
//! actually needs on day one: a dashboard that can be asked a question, and a
//! logger that outlives it.
//!
//! The shape and the ordering are kept, because the ordering is the point:
//! **logging first**, so that everything after it can say what went wrong,
//! then **locate WSA**, so that "not installed" is a fact established before
//! any command is attempted rather than a timeout that reads like a hang.

use std::path::PathBuf;

use tracing::{info, warn};
use tracing_appender::non_blocking;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

use crate::adb::path::adb_path;
use crate::app::state::AppState;
use crate::wsa::app::wsa_path::{self, WsaPaths};
use crate::wsa::dashboard::dashboard_for;

/// `~/.wsadashboard/logs`.
///
/// Under the home directory rather than `%LOCALAPPDATA%` because it is the one
/// location a user can be told about in a sentence and find without a shell
/// expansion -- and because a log directory beside the user is what the spec
/// asks for (Q12 keeps file locations to the reference's names; this is a
/// path, not a name, and the reference's `~/.wsl-dashboard` has no reason to
/// be carried to a product that is not about WSL).
pub fn logs_dir() -> PathBuf {
    // The arithmetic lives in `config` so that there is one answer to "where do
    // the logs go": the settings page's default is the same call. Two copies of
    // this expression is how a settings page ends up naming a directory the
    // logger never writes to.
    crate::config::logs_dir()
}

/// Rolling daily file at `<logs_dir>/wsadashboard.log`, level from
/// `RUST_LOG` and `info` by default.
///
/// `try_init` rather than `init`: the second would panic, and this is called
/// from a function a test may reasonably call twice. A failed install leaves
/// the default (no subscriber) instead of taking the process down -- losing
/// logs is bad, losing the app is worse.
fn init_logging() -> WorkerGuard {
    let dir = logs_dir();
    let _ = std::fs::create_dir_all(&dir);

    let (writer, guard) = non_blocking(tracing_appender::rolling::daily(
        &dir,
        "wsadashboard.log",
    ));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(writer)
        .with_ansi(false)
        .try_init();
    guard
}

/// Build the shared state: logging, then where WSA lives, then the facade.
///
/// **No failure path.** A machine with no WSA is the state `DistroNotFoundView`
/// exists to describe (spec section 5 / T3), and refusing to open the window on
/// one would trade a whole view for an error dialog. `resolve` failing is
/// therefore recorded and turned into [`WsaPaths::absent`], whose fields are
/// deliberately unreachable: `probe` reports `NotInstalled` from the Appx
/// registry before any adb command could consult them.
pub async fn bootstrap() -> AppState {
    let logging = init_logging();

    let paths = match wsa_path::resolve() {
        Ok(paths) => paths,
        Err(error) => {
            warn!(
                ?error,
                "WSA not located; the window will open on the not-installed state"
            );
            WsaPaths::absent()
        }
    };

    info!(
        adb_port = paths.adb_port,
        absent = paths.install_location.as_os_str().is_empty(),
        "starting"
    );

    AppState::new(dashboard_for(adb_path(), paths), logging)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The log directory is the one piece of this module that is decided
    /// before anything can fail, so it is worth pinning: a regression to a
    /// relative path would leave the logs beside whatever directory the exe
    /// happened to be launched from, which is a place nobody looks.
    #[test]
    fn logs_live_under_a_dot_directory_in_home() {
        let dir = logs_dir();
        assert!(dir.is_absolute(), "an absolute path is required, got {dir:?}");
        assert_eq!(
            dir.components().next_back().map(|c| c.as_os_str().to_string_lossy().to_string()),
            Some("logs".to_string())
        );
        assert!(
            dir.to_string_lossy().contains(".wsadashboard"),
            "the directory must be recognisable as this app's, got {dir:?}"
        );
    }

    /// The absent paths must not look like a real install: a non-empty
    /// `install_location` is what `probe` would read to decide WSA is present.
    #[test]
    fn absent_paths_cannot_be_mistaken_for_an_install() {
        let paths = WsaPaths::absent();
        assert!(paths.install_location.as_os_str().is_empty());
        assert!(paths.manifest_path.as_os_str().is_empty());
        assert!(paths.client_exe.as_os_str().is_empty());
        assert!(paths.settings_aumid.is_empty());
        // ...but the port stays a real one, so anything reading it early gets
        // 58526 rather than 0.
        assert_eq!(paths.adb_port, crate::adb::port::FALLBACK_PORT);
    }
}
