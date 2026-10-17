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
/// **A `slint::Timer`, not `tokio::time::sleep`.** The first version of this
/// function was a `spawn_local` loop that awaited `tokio::time::sleep(5s)`, and it
/// never slept: this future runs on Slint's *local* executor, where a tokio timer
/// has no driver to fire it, so the sleep returned at once and the loop became a
/// spin. Measured on the released build: **8.7 seconds of CPU in 10**, one thread
/// `Running`, and a window that answered — a page that is technically alive and
/// unusable in practice. `should_refresh` swallowed the work, so nothing in the
/// log said so.
///
/// The timer belongs to the executor that runs this code, which is the whole
/// reason it is the right primitive here. `forget` rather than a local: a
/// `slint::Timer` stops when it is dropped, and this one is meant to live as long
/// as the process. One timer, once.
pub fn spawn_home_monitor(weak: Weak<AppWindow>, state: Arc<AppState>) {
    // The first poll happens *now*: a window that takes five seconds to say
    // anything looks broken.
    let _ = slint::spawn_local({
        let weak = weak.clone();
        let state = state.clone();
        async move { crate::ui::data::refresh_home(weak, state).await }
    });

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        crate::ui::data::poll_interval(),
        move || {
            let _ = slint::spawn_local(crate::ui::data::refresh_home(
                weak.clone(),
                state.clone(),
            ));
        },
    );
    std::mem::forget(timer);
}
