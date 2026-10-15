// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The window's own callbacks: close, minimise, maximise, drag, pin.
//!
//! Ported from the reference's `src/ui/handlers/window.rs` (75 lines), plus the
//! three window-state operations the tray needs -- [`park_in_tray`],
//! [`hide_to_tray`] and [`show_and_center`]. Those three are the reference's
//! `app/window.rs::show_and_center` split apart: the reference has one 120-line
//! function with a `silent` flag and a background reveal thread, and what it is
//! doing is three separate things that are each called from a different place.
//! Splitting them also keeps `app/window.rs` free of the Slint types, which is
//! what lets the CLI mount it and run its tests. MAPPING §16 records it.
//!
//! # What the reference's close paths do, and ours
//!
//! Both close paths ask `tray_close_to_tray`. On, the window is hidden and left
//! running -- which is the whole point of a tray, and why the property exists. Off,
//! the program quits. The reference keeps the window up (`KeepWindowShown`) in the
//! hidden case rather than hiding it through Slint, because the tray is what brings
//! it back and Slint's own hide would fight the Win32 style changes; that shape is
//! kept.
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
use tracing::info;

use crate::AppWindow;

/// Register the window callbacks (reference `handlers/window.rs`).
pub fn setup(app: &AppWindow, app_handle: slint::Weak<AppWindow>) {
    // Alt+F4, and the title bar's close button, both land here first.
    let ah = app_handle.clone();
    app.window().on_close_requested(move || {
        if let Some(app) = ah.upgrade() {
            if app.get_tray_close_to_tray() {
                info!("close requested; hiding to the tray");
                hide_to_tray(&app);
                // Kept shown, not hidden: the tray owns whether the window is up,
                // and Slint hiding it would undo the taskbar style in
                // `set_skip_taskbar` on the next restore.
                return slint::CloseRequestResponse::KeepWindowShown;
            }
            info!("close requested; quitting");
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
        if let Some(app) = ah.upgrade() {
            if app.get_tray_close_to_tray() {
                info!("title bar close clicked; hiding to the tray");
                hide_to_tray(&app);
                return;
            }
            info!("title bar close clicked; quitting");
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
            info!(pinned, "window pin toggled");
        }
    });
}

/// Park the window in the tray before it is ever shown.
///
/// Called before `app.run()`, which is what would otherwise put the window up: a
/// start-minimised launch that paints a frame first is not start-minimised. The
/// three parts each matter -- `is_window_visible` is what the tray reads to decide
/// which way its toggle goes, `set_minimized` keeps Slint's own show from
/// un-minimising it, and the position keeps it off screen while the taskbar style
/// is being changed.
pub fn park_in_tray(app: &AppWindow) {
    app.set_is_window_visible(false);
    app.window().set_minimized(true);
    app.window()
        .set_position(slint::LogicalPosition::new(-32000.0, -32000.0));
    crate::app::window::set_skip_taskbar(true);
}

/// Hide a window that is on screen.
pub fn hide_to_tray(app: &AppWindow) {
    app.set_is_window_visible(false);
    crate::app::window::set_skip_taskbar(true);
}

/// Bring the window back: visible, on screen, and on top.
pub fn show_and_center(app: &AppWindow) {
    info!("showing the window");
    app.set_is_window_visible(true);
    app.window().set_minimized(false);
    let _ = app.show();
    // Restores the taskbar style, clears the layered alpha, re-centres a window
    // that was parked at -32000, and activates it -- all four steps live in
    // `app::window` because none of them needs the Slint handle.
    crate::app::window::set_skip_taskbar(false);
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

