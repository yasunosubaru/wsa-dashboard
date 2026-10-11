// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// GUI entry point (`wsadashboard`).
//
// P0: open the skeleton window. P2 replaces `run()` with the reference's
// launcher -> lifecycle -> runner sequence (docs/MAPPING.md, spec section 2.1).

// Builds src/ui/app.slint and exposes `AppWindow`.
slint::include_modules!();

mod adb;
mod app;
mod i18n;
mod ui;
mod wsa;

// The reference hangs `AppState` and the generated types off the crate root
// (`src/main.rs:22`), and every module in the tree refers to them as
// `crate::AppState` / `crate::AppWindow` / `crate::Distro`. Keeping that shape
// is what lets a file be diffed against its counterpart line by line.
pub use app::state::AppState;

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    ui.run()
}
