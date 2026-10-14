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

pub fn setup(app: &AppWindow, app_handle: Weak<AppWindow>, app_state: Arc<AppState>) {
    let ah = app_handle.clone();
    app.on_select_tab(move |tab| {
        // Logged unconditionally and at debug, because "the click did not
        // arrive" and "the click arrived and was refused" are different
        // problems that look identical from the outside -- the page simply
        // does not change, and this is the only place that can tell them
        // apart.
        tracing::debug!(tab, allowed = has_a_view(tab), "tab selected");
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

        // Each page that has work to do asks for it when it is opened. The
        // reference does the same from here -- efresh_usb, efresh_tasks,
        // efresh_mounted_disks, three fetches -- and it is why this handler
        // has the state: switching to a page is the moment its data is
        // known-to-be-wanted, and a page that only refreshed on a timer would
        // be blank for as long as the first visit takes.
        if tab == 2 {
            crate::ui::handlers::device::refresh(ah.clone(), app_state.clone());
        }
        // Tab 3 is the network page, and it reads two things: adb's forwarding
        // tables and the proxy globals. Neither needs a live subsystem -- adb
        // keeps a forwarding table whether or not the device is up, and
        // `settings get` answers `null` rather than erroring when the value was
        // never written -- so the page is useful with the subsystem stopped,
        // which is the same criterion the storage page is held to. What it does
        // need is to be asked when it is opened: a list read once at startup
        // would show whatever was forwarded *then*, and a mapping added from a
        // terminal since would be missing from it.
        if tab == 3 {
            crate::ui::handlers::network::refresh(ah.clone(), app_state.clone());
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
