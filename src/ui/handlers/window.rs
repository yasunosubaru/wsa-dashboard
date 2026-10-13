// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The window's own callbacks: close, minimise, maximise, drag, pin.
//!
//! Ported from the reference's `src/ui/handlers/window.rs` (75 lines). Three
//! branches of the original are absent and the reason is one line each:
//!
//!   * `tray_close_to_tray` -- there is no tray until P7, so both close paths
//!     quit. The *shape* of the reference's branch is kept in the comments
//!     below rather than a condition that is always false; a dead `if` would
//!     read as a decision that was never made.
//!   * `set_skip_taskbar` -- the tray hides the window. P7.
//!   * `is_window_visible` -- nothing hides it yet. The property it feeds lives
//!     on the tray side too, so it arrives with it.
//!
//! `on_window_drag_delta` is worth reading twice, because the obvious
//! alternative is wrong:
//!
//! ```ignore
//! // NOT this, even though it is the idiomatic answer:
//! app.window().start_drag();
//! ```
//!
//! `start_drag` hands the drag to the operating system, which then swallows
//! every subsequent mouse event until the button comes up -- and the title bar's
//! `moved` would stop firing. The reference instead takes the reported delta and
//! moves the window by it, which works because `TitleBar` reports the distance
//! from the *press point*, and the window follows the cursor, so the reported
//! distance stays bounded instead of accumulating. The reference's own shape,
//! and the one P2's plan got wrong in its review notes; MAPPING section 8
//! records the correction.

use slint::ComponentHandle;

use crate::AppWindow;

/// Register the window callbacks (reference `handlers/window.rs`).
pub fn setup(app: &AppWindow, app_handle: slint::Weak<AppWindow>) {
    // Alt+F4 is the only way to close this window: P3 has no caption to click.
    // The reference keeps the window up for the tray to have a window to re-show
    // (`tray_close_to_tray`); with no tray, hiding would mean the only way back
    // is killing the process.
    let ah = app_handle.clone();
    app.window().on_close_requested(move || {
        if ah.upgrade().is_some() {
            tracing::info!("close requested (Alt+F4), quitting");
            let _ = slint::quit_event_loop();
        }
        slint::CloseRequestResponse::HideWindow
    });

    let ah = app_handle.clone();
    app.on_window_minimize(move || {
        if let Some(app) = ah.upgrade() {
            app.window().set_minimized(true);
        }
    });

    // Declared because the reference declares it, and wired because the
    // reference wires it. Neither reference nor this file has a caller: the
    // reference's `TitleBar` draws no maximize button, so the only way to reach
    // this would be a keyboard binding neither program defines. Kept so the
    // button can be added without touching the handler layer.
    let ah = app_handle.clone();
    app.on_window_maximize(move || {
        if let Some(app) = ah.upgrade() {
            let is_max = app.get_is_maximized();
            app.set_is_maximized(!is_max);
            app.window().set_maximized(!is_max);
        }
    });

    let ah = app_handle.clone();
    app.on_window_close(move || {
        if ah.upgrade().is_some() {
            // The reference's other branch here hides to the tray.
            tracing::info!("title bar close clicked, quitting");
            let _ = slint::quit_event_loop();
        }
    });

    let ah = app_handle.clone();
    app.on_window_drag_delta(move |dx, dy| {
        if let Some(app) = ah.upgrade() {
            let pos = app.window().position();
            app.window().set_position(slint::PhysicalPosition::new(
                pos.x + dx as i32,
                pos.y + dy as i32,
            ));
        }
    });

    let ah = app_handle.clone();
    app.on_window_pin_toggle(move || {
        if let Some(app) = ah.upgrade() {
            let pinned = !app.get_is_pinned();
            app.set_is_pinned(pinned);
            crate::app::window::set_always_on_top(pinned);
            tracing::info!(pinned, "window pin toggled");
        }
    });
}

#[cfg(test)]
mod tests {
    /// None, and not for want of them. Every handler here mutates a live
    /// `AppWindow`: minimizing, toggling the level, moving by a delta. The
    /// component types live in the GUI binary, which is not what `cargo test`
    /// runs, so the honest split is the one this project already uses -- the
    /// decisions that can be pure live in a module `wsadash` can reach, and the
    /// files that can only talk to a window say so.
    ///
    /// What is checkable instead, and was checked for P3: the window can be
    /// dragged by its title bar, minimized, re-shown from the taskbar, and
    /// pinned above a second window. That is a desktop test, and it is recorded
    /// in docs/MAPPING.md section 8 rather than pretended at here.
    fn nothing_here_is_testable_without_a_desktop() {}
}
