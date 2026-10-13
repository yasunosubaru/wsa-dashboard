// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! What happens when a side-bar entry is clicked, and when it is collapsed.
//!
//! The counterpart of the reference's `src/ui/handlers/common.rs`. Two of its
//! four registrations are ours; the other two -- `open_url` and
//! `close_task_status` -- went into `home.rs` during P2, when this file did not
//! exist yet, and stay there: moving working code to match a directory listing
//! is churn with no behaviour behind it, and `home.rs` says plainly which
//! reference file it is closest to. MAPPING section 8 records the split.

use std::sync::Arc;

use slint::Weak;

use crate::AppWindow;
use crate::app::state::AppState;
use crate::ui::tabs::has_a_view;

// The list lives in `ui/tabs.rs` and not beside this handler: `handlers`
// belongs to the GUI binary and is reached through `mod ui;`, which the CLI's
// test run never compiles. Written here, its tests would have run zero times
// while looking, in a diff, exactly like tests that do. `ui/tabs.rs` says the
// same thing at greater length, including which phase adds which index.

pub fn setup(app: &AppWindow, app_handle: Weak<AppWindow>, _app_state: Arc<AppState>) {
    let ah = app_handle.clone();
    app.on_select_tab(move |tab| {
        if !has_a_view(tab) {
            // Not an error: the entry is visible and its page is not written
            // yet, and the user has just been told nothing about why. `debug`
            // rather than `warn` because this is expected behaviour for as long
            // as P3 stands, and a log file full of warnings about the planned
            // state teaches nobody anything.
            tracing::debug!(tab, "tab has no page yet; staying where we are");
            return;
        }
        if let Some(app) = ah.upgrade() {
            app.set_selected_tab(tab);
        }
    });

    // The reference persists this through `ConfigManager`, and each of its
    // per-tab entries there refreshes the page being switched to (the reference
    // calls `refresh_usb`, `refresh_tasks`, `refresh_mounted_disks` and three
    // fetches from this one handler). Ours has neither yet: `ConfigManager` is
    // P6 and every one of those pages is P4 or later.
    //
    // So the property moves -- `sidebar_collapsed` is `in-out`, and the side bar
    // binds to it directly -- and the write that would make it survive a restart
    // arrives with P8's configuration. Declared and connected now so the side
    // bar's `toggle_collapsed` has somewhere to land, and so nothing has to
    // change here when it starts doing something.
    let ah = app_handle.clone();
    app.on_save_sidebar_state(move || {
        if let Some(app) = ah.upgrade() {
            let collapsed = app.get_sidebar_collapsed();
            tracing::debug!(collapsed, "sidebar toggled; persisted in P8");
        }
    });
}
