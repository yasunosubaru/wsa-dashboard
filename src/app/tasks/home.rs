// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The home page's poll.
//!
//! The counterpart of the reference's `tasks::spawn_wsl_monitor`, which every
//! five seconds re-reads the distro list. Two differences, both of them the
//! reference doing work this project does not need:
//!
//!   * It calls `refresh_distros_ui` directly on the window's handle; this goes
//!     through `data::refresh_home`, which owns the debounce, the single-flight
//!     guard and the dedup that decides whether anything is worth re-rendering.
//!   * It has no way to stop. A window in the tray keeps spending `wsl -l -v`
//!     round trips, so `is_visible` is a gate on the *question* (see
//!     `refresh::should_refresh`) rather than on this loop: the task runs
//!     forever and is told to skip work it cannot justify.

use std::sync::Arc;

use slint::Weak;

use crate::app::state::AppState;
use crate::app::runner::AppWindow;

/// Drive [`crate::ui::data::refresh_home`] on a fixed tick until the process
/// ends.
///
/// On the Slint event loop rather than on tokio: the first poll must happen
/// *now*. A window that takes five seconds to say anything looks broken, and
/// `should_refresh` is what stops that immediate poll from turning into one
/// per tick when the previous one has not settled.
pub fn spawn_home_monitor(weak: Weak<AppWindow>, state: Arc<AppState>) {
    let _ = slint::spawn_local(async move {
        loop {
            crate::ui::data::refresh_home(weak.clone(), state.clone()).await;
            tokio::time::sleep(crate::ui::data::poll_interval()).await;
        }
    });
}
