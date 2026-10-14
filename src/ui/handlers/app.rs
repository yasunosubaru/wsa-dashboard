// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The install page: choose an APK, install it, say what happened.
//!
//! The counterpart of the reference's `src/ui/handlers/distro/install.rs` plus
//! its `install_logic.rs` (314 + 713 lines). That pair downloads a distro over
//! a mirror, verifies a checksum, expands a rootfs and writes progress into
//! `terminal_output` line by line, because the work takes minutes and streams.
//!
//! `adb install -r` is one synchronous round trip with a 45s write budget
//! (spec section 2.2). There is nothing to stream, so there is nothing to
//! build: pick the file, await the command, report the two outcomes. The one
//! thing worth copying is *when* to ask -- the reference opens the dialog from
//! `on_select_install_file` and installs from a separate callback, so a
//! cancelled dialog is a no-op rather than a half-done install.

use std::path::Path;
use std::sync::Arc;

use slint::Weak;
use tracing::{info, warn};

use crate::AppWindow;
use crate::app::state::AppState;
use crate::i18n;

pub fn setup(app: &AppWindow, app_handle: Weak<AppWindow>, app_state: Arc<AppState>) {
    let ah = app_handle.clone();
    app.on_select_install_file(move |_source_idx| {
        // One filter, unlike the reference's three: that page chooses *how* to
        // create an instance (rootfs, vhdx, a store distro) and the argument is
        // the choice, while this one always installs a package. The argument is
        // kept because `select_install_file(int)` is the reference's signature
        // (Q12) and an ignored parameter is cheaper than a divergent callback.
        let picked = rfd::FileDialog::new()
            .set_title(&i18n::t("add.select_file"))
            .add_filter("Android package", &["apk"])
            .pick_file();

        if let Some(path) = picked {
            if let Some(app) = ah.upgrade() {
                let shown = path.display().to_string();
                // The path is the field's whole job, so it is checked here
                // rather than after it has been handed to adb: a dialog can
                // return a file that a subsequent save or move removed, and
                // adb's own answer for that is a message about a path the user
                // did not type and cannot see on screen any more.
                if path.is_file() {
                    app.set_path_error("".into());
                } else {
                    app.set_path_error(i18n::t("add.file_missing").into());
                }
                app.set_install_file_path(shown.into());
            }
        }
    });

    let ah = app_handle.clone();
    let st = app_state;
    app.on_install_clicked(move |path| {
        let path = path.to_string();
        if path.is_empty() {
            return;
        }
        let handle = ah.clone();
        let state = st.clone();

        let _ = slint::spawn_local(async move {
            let target = Path::new(&path);
            if !target.is_file() {
                // Caught before the round trip for the reason above; the field
                // keeps saying why rather than the page quietly doing nothing.
                if let Some(app) = handle.upgrade() {
                    app.set_path_error(i18n::t("add.file_missing").into());
                }
                return;
            }

            if let Some(app) = handle.upgrade() {
                app.set_path_error("".into());
                app.set_is_installing(true);
                app.set_install_success(false);
                app.set_terminal_output("".into());
                // Pre-flight toast, for the same reason launch has one (see
                // `handlers/home.rs`): `Busy`/`Rejected` aside, a 45s wait with
                // no feedback reads as a hang.
                toast(&handle, i18n::t("add.installing"));
            }

            let outcome = state.wsa_dashboard.install_apk(target).await;

            if let Some(app) = handle.upgrade() {
                app.set_is_installing(false);
            }
            match outcome {
                Ok(()) => {
                    info!(path, "apk installed");
                    if let Some(app) = handle.upgrade() {
                        app.set_install_success(true);
                        app.set_terminal_output(i18n::t("add.installed").into());
                    }
                    clear_toast(&handle);
                    // A new package is a new row on the home page; the poll
                    // would get to it in five seconds, and a page that has just
                    // been told "Installed" should not need it.
                    crate::ui::data::refresh_home_with(
                        crate::ui::refresh::MANUAL_TRIGGER,
                        handle.clone(),
                        state.clone(),
                    )
                    .await;
                }
                Err(error) => {
                    warn!(path, ?error, "apk install failed");
                    if let Some(app) = handle.upgrade() {
                        app.set_install_success(false);
                        app.set_terminal_output(error.to_string().into());
                    }
                    toast(&handle, error.to_string());
                }
            }
        });
    });
}

fn toast(handle: &Weak<AppWindow>, text: String) {
    if let Some(app) = handle.upgrade() {
        app.set_task_status_text(text.into());
        app.set_task_status_visible(true);
    }
}

fn clear_toast(handle: &Weak<AppWindow>) {
    if let Some(app) = handle.upgrade() {
        app.set_task_status_visible(false);
    }
}

// No tests here, for the reason `handlers/home.rs` records: this module names
// `AppWindow`, which only the GUI bin compiles, so anything written here would
// sit behind `cargo test --release --bin wsadash` and never run. The decision
// worth testing -- "does an absent file refuse before the round trip" -- is a
// `Path::is_file` check inside a closure that also needs a window, so it is
// checked by the screen rather than pretended at in a test.
#[cfg(test)]
mod tests {
    fn nothing_here_is_testable_without_a_window() {}
}
