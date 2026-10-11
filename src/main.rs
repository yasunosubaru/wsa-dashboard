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

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    ui.run()
}
