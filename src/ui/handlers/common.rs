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
        // reference does the same from here -- `refresh_usb`, `refresh_tasks`,
        // `refresh_mounted_disks`, three fetches -- and it is why this handler
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
        // Tab 4 is the scheduler page, and what it shows is the task store.
        // `schtasks` is the only reader of it, so this is where the list is
        // asked for: the page is P7's and the answer changes without this
        // program's involvement (the Task Scheduler, a logon trigger, another
        // user), so a list read once at startup would be a page that lies.
        if tab == 4 {
            crate::ui::handlers::scheduler::refresh(ah.clone(), app_state.clone());
        }
        // Tab 7 is the settings page, and what it shows is the settings file.
        // There is nothing to fetch from a subsystem -- but a file the reader
        // edited by hand between two visits should be what they see when they
        // come back, which is what the reference does from here too.
        if tab == 7 {
            crate::ui::handlers::settings::reload(ah.clone());
        }
        // Tab 8 is the about page, and it has nothing to read -- but its strings
        // are resolved on this side of the i18n bridge, so a language chosen on
        // the settings page does not reach them unless they are resolved again.
        // Re-filling on arrival is what makes that true without the settings page
        // having to know a second page exists.
        if tab == 8 {
            crate::ui::handlers::about::reload(ah.clone());
        }
    });

    // The reference persists this through `ConfigManager`, and each of its
    // per-tab entries there refreshes the page being switched to (the reference
    // calls `refresh_usb`, `refresh_tasks`, `refresh_mounted_disks` and three
    // fetches from this one handler). Ours now calls three of those four, each
    // from the block for its own tab above.
    //
    // The write itself is P8's, and it is a read-modify-write on the file rather
    // than a save of a settings struct this handler holds, for the reason
    // `settings.rs` gives at length: the window is not the file, and a handler
    // that saved its own copy of the settings would write back every other
    // value as it was when the window opened. One field, one write.
    let ah = app_handle.clone();
    app.on_save_sidebar_state(move || {
        let Some(app) = ah.upgrade() else { return };
        let collapsed = app.get_sidebar_collapsed();
        if let Err(error) = crate::config::update_user_settings(|user| {
            user.sidebar_collapsed = collapsed;
        }) {
            // A collapsed side bar that did not survive a restart is not worth
            // a toast or a failed click. The log is where a reader goes when
            // they notice, and `settings.rs`'s own failures do the same.
            tracing::warn!(%error, "could not persist the side bar's collapsed state");
        }
    });
}
