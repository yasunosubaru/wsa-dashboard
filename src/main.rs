#![windows_subsystem = "windows"]
// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// GUI entry point (`wsadashboard`).
//
// Deliberately a sequencing file, in the reference's order
// (reference `src/main.rs:25-89`):
//
//   1. `launcher::bootstrap`  -- logs, then locate WSA, then the state
//   2. `runner::run_app`      -- the window, the handlers, the event loop
//
// Not yet implemented, and named so that the absence is visible rather than
// silent (P7 unless noted):
//
//   * step 3, the immediate CLI (`wsadashboard --version`), needs P1's exit
//     codes to be reachable from this binary too -- P7
//   * step 5, the single-instance handshake, the log-rotation sweep and the
//     start-on-login repair -- P7 / P8
//   * the system tray -- P8
//
// `slint::include_modules!()` lives in `app::runner` (plan C1) so that this
// file stays readable and the generated types sit beside the code that
// configures them. The `pub use` below restores the reference's shape, where
// every module in the tree spells them `crate::AppWindow` / `crate::Distro`.

mod adb;
mod app;
mod i18n;
mod ui;
mod wsa;

pub use app::runner::*;
pub use app::state::AppState;

#[tokio::main]
async fn main() {
    let state = app::launcher::bootstrap().await;
    app::runner::run_app(state).await;
}
