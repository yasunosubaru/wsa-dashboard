// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The device page's own work: what adb's targets are, attaching or
//! detaching, and what a target says about itself.
//!
//! The counterpart of the reference's `src/ui/handlers/usb.rs` (564 lines).
//! Most of that file is usbip -- enumerating through `usbipd`, binding and
//! unbinding, attaching to WSL, a scheduled task for auto-attach -- and none of
//! it has an adb counterpart. What is here is the same page in three moves:
//! read the list, change a connection, read a property table.

use std::rc::Rc;
use std::sync::Arc;

use slint::{ModelRc, VecModel, Weak};
use tracing::{info, warn};

use crate::AppWindow;
use crate::app::state::AppState;
use crate::wsa::executor::ExecError;
use crate::wsa::dashboard::ops::{AdbTarget, OpsError, is_session_state};

/// The properties the table shows, and in what order.
///
/// Measured rather than remembered: `adb shell getprop` against the live
/// subsystem on 2026-10-13 answered 385 lines, and every key below came back
/// except `ro.serialno` -- WSA has no serial to report, which is exactly why
/// [`SHOWN_PROPS`] is filtered by "is it in the reply" and not assumed.
/// Seven lines is the whole table; a viewer over all 385 would be a wall of
/// build properties nobody scrolls past, and the reference's own detail panel
/// shows eight fields for the same reason.
const SHOWN_PROPS: &[&str] = &[
    "ro.product.manufacturer",
    "ro.product.model",
    "ro.build.version.release",
    "ro.build.version.sdk",
    "ro.hardware",
    "ro.build.display.id",
    "ro.build.fingerprint",
];

/// Register the device page's callbacks (reference `handlers/usb.rs`).
pub fn setup(app: &AppWindow, app_handle: Weak<AppWindow>, app_state: Arc<AppState>) {
    let ah = app_handle.clone();
    let st = app_state.clone();
    app.on_attach_clicked(move |target| {
        run_target_action(ah.clone(), st.clone(), Action::Attach, target.as_str());
    });

    let ah = app_handle.clone();
    let st = app_state.clone();
    app.on_detach_clicked(move |target| {
        run_target_action(ah.clone(), st.clone(), Action::Detach, target.as_str());
    });

    let ah = app_handle.clone();
    let st = app_state.clone();
    app.on_info_clicked(move |target| {
        // Logged the moment the click lands, because "the chevron was pressed"
        // and "the chevron was missed" are otherwise indistinguishable: the
        // card stays collapsed either way, and a silent miss looks exactly like
        // a table that failed to load.
        tracing::debug!(serial = %target.serial, "info clicked");
        let ah = ah.clone();
        let st = st.clone();
        let serial = target.serial.to_string();
        let _ = slint::spawn_local(async move { load_props(ah, st, serial).await });
    });
}

/// What the row's one button does.
#[derive(Clone, Copy, Debug)]
enum Action {
    Attach,
    Detach,
}

/// One attach/detach, then the list again.
///
/// The refresh is not cosmetic: `adb connect` adds a row the list did not
/// have, and `adb disconnect` removes one the list did. Showing the old list
/// until the next visit would be a page that disagrees with itself.
fn run_target_action(
    handle: Weak<AppWindow>,
    state: Arc<AppState>,
    action: Action,
    serial: &str,
) {
    let serial = serial.to_string();
    let _ = slint::spawn_local(async move {
        let result = match action {
            Action::Attach => state.wsa_dashboard.connect_target(&serial).await,
            Action::Detach => state.wsa_dashboard.disconnect_target(&serial).await,
        };
        match result {
            Ok(()) => {
                info!(%serial, target = ?action, "target updated");
                // The properties described a connection that no longer exists,
                // or one just opened. Neither is worth keeping on screen.
                if let Some(app) = handle.upgrade() {
                    app.set_usb_device_info_data(ModelRc::from(Rc::new(VecModel::from(vec![]))));
                    app.set_usb_info_target("".into());
                }
            }
            Err(error) => {
                warn!(%serial, ?error, "target action failed");
                toast(&handle, error.to_string());
            }
        }
        load_targets(handle, state).await;
    });
}

/// Read the list and put it on the window.
///
/// Called when the page is opened (`handlers/common::on_select_tab`) and after
/// an attach/detach. Not on a timer: the reference refreshes this page when it
/// is switched to and when something changes it, and there is no third reason.
pub fn refresh(handle: Weak<AppWindow>, state: Arc<AppState>) {
    let _ = slint::spawn_local(load_targets(handle, state));
}

async fn load_targets(handle: Weak<AppWindow>, state: Arc<AppState>) {
    if let Some(app) = handle.upgrade() {
        app.set_usb_loading(true);
    }
    let outcome = state.wsa_dashboard.adb_targets().await;
    let Some(app) = handle.upgrade() else {
        return;
    };
    match outcome {
        Ok(targets) => {
            let rows = to_rows(targets, state.wsa_dashboard.adb_port());
            app.set_usb_error("".into());
            app.set_usb_targets(ModelRc::from(Rc::new(VecModel::from(rows))));
        }
        Err(error) => {
            // The view has one string it treats specially, and `ExecError::
            // NotFound` is the one it means: adb is not on the path, and no
            // amount of retrying will put it there. Everything else is shown
            // as itself, because a message about why there is no list is more
            // use than an empty one.
            warn!(?error, "could not list adb targets");
            let message = match error {
                OpsError::Adb(ExecError::NotFound) => "not_found".to_string(),
                other => other.to_string(),
            };
            app.set_usb_error(message.into());
            app.set_usb_targets(ModelRc::from(Rc::new(VecModel::from(Vec::new()))));
        }
    }
    app.set_usb_loading(false);
}

async fn load_props(handle: Weak<AppWindow>, state: Arc<AppState>, serial: String) {
    // `WsaCommand::GetProps` is bare `adb shell getprop`, as spec section 2.3
    // lists it -- no `-s`, because the command surface has one shape. With a
    // single connected target that is the same command; with two, adb refuses
    // and says so on stderr, which arrives here as an error rather than as
    // another target's properties under this one's name. Showing the failure
    // is the right answer either way.
    match state.wsa_dashboard.device_props().await {
        Ok(props) => {
            if let Some(app) = handle.upgrade() {
                let rows = to_props(props);
                tracing::debug!(rows = rows.len(), serial = %serial, "device properties read");
                app.set_usb_device_info_data(ModelRc::from(Rc::new(VecModel::from(rows))));
                app.set_usb_info_target(serial.into());
            }
        }
        Err(error) => {
            warn!(%serial, ?error, "could not read device properties");
            toast(&handle, error.to_string());
        }
    }
}

/// adb's words on the window.
fn to_rows(targets: Vec<AdbTarget>, subsystem_port: u16) -> Vec<crate::DeviceTarget> {
    let subsystem = format!("127.0.0.1:{subsystem_port}");
    targets
        .into_iter()
        .map(|target| crate::DeviceTarget {
            is_connected: is_session_state(&target.state),
            is_subsystem: target.serial == subsystem,
            serial: target.serial.into(),
            state: target.state.into(),
        })
        .collect()
}

/// `getprop` as the table's rows, in [`SHOWN_PROPS`] order and only for the
/// keys the device actually answered.
///
/// Both halves matter. The order is ours, not `HashMap`'s -- a property table
/// whose rows move between openings is a table nobody learns to read. And a
/// key the device does not have is skipped rather than shown empty, which is
/// how `ro.serialno` disappears on WSA (measured: 385 properties, none of them
/// that one) while still appearing for a real phone plugged in over USB.
fn to_props(props: std::collections::HashMap<String, String>) -> Vec<crate::DeviceProp> {
    SHOWN_PROPS
        .iter()
        .filter_map(|key| {
            props.get(*key).map(|value| crate::DeviceProp {
                key: (*key).into(),
                value: value.clone().into(),
            })
        })
        .collect()
}

fn toast(handle: &Weak<AppWindow>, text: String) {
    if let Some(app) = handle.upgrade() {
        app.set_task_status_text(text.into());
        app.set_task_status_visible(true);
    }
}

#[cfg(test)]
mod tests {
    // Nothing, and the reason is the usual one: every function here names
    // `AppWindow`, which only the GUI binary compiles, so a test written in
    // this module would sit behind `cargo test --release --bin wsadash` and
    // never run -- a fact the project has now written down twice.
    //
    // What is worth pinning is the *selection* in `to_props`, and it is a
    // pure decision about a list of names. `SHOWN_PROPS` is checked by eye
    // against a real `getprop` run recorded above; the part that cannot be
    // eyeballed -- "is a missing key skipped rather than shown blank" -- is
    // exactly the kind of thing that should be a test, so it belongs in a
    // module the CLI can reach if it ever needs to change.
    fn nothing_here_is_testable_without_a_window() {}
}
