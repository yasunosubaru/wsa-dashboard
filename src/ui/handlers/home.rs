// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! What the home page's buttons actually do.
//!
//! The counterpart of the reference's `src/ui/handlers/distro.rs`. Five of
//! the reference's callbacks are declared on `AppWindow` for P2 --
//! `start_distro`, `stop_distro`, `open_url`, `reorder_distros` and
//! `close_task_status`.
//!
//! Of the expanded action strip, the three buttons spec Q22 keeps for WSA
//! (`information`, `delete`, `settings`) are wired as of P5a, which is also the
//! phase that brought `modal_manager.slint`. This note used to say they arrive
//! in P3; P3 delivered the shell and wired none of them, and the gap sat open
//! for two phases with a comment claiming otherwise. See
//! `components/distro_card.slint` for the same fact.

use std::rc::Rc;
use std::sync::Arc;

use slint::{Model, ModelRc, SharedString, VecModel, Weak};
use tracing::{info, warn};

use crate::AppWindow;
use crate::app::state::AppState;
use crate::i18n;
use crate::wsa::dashboard::WsaDashboard;
use crate::wsa::dashboard::ops::OpsError;

pub fn setup(app: &AppWindow, handle: Weak<AppWindow>, state: Arc<AppState>) {
    app.on_start_distro(run(handle.clone(), state.clone(), Action::Launch, Busy::Yes));
    app.on_stop_distro(run(handle.clone(), state.clone(), Action::Stop, Busy::No));
    app.on_open_url(open_url(handle.clone()));
    app.on_reorder_distros(reorder(handle.clone()));
    app.on_close_task_status(clear(handle.clone()));

    // The three the expanded action strip keeps (spec Q22), wired in P5a.
    setup_card_actions(app, &handle, &state);

    // The side bar's home entry fires this alongside `tab_selected(0)`.
    //
    // Clicking the entry you are already on is the reference's "refresh now"
    // gesture -- its own registration is in `handlers/distro/manage.rs` and
    // gates on `should_refresh_wsl("manual trigger", ...)`. It returns at once
    // like every other handler here; the refresh runs on the event loop, not on
    // this thread.
    let ah = handle.clone();
    let st = state.clone();
    app.on_home_clicked(move || {
        let _ = slint::spawn_local(crate::ui::data::refresh_home_with(
            crate::ui::refresh::MANUAL_TRIGGER,
            ah.clone(),
            st.clone(),
        ));
    });
}

/// What the button asked for.
///
/// `Copy` because the closure that owns it runs more than once, and a value
/// that has to be cloned into every invocation is a value whose clone can be
/// forgotten.
#[derive(Clone, Copy)]
enum Action {
    Launch,
    Stop,
}

/// Whether the button shows a "working on it" toast *before* asking the
/// subsystem anything.
///
/// Launching gets one because activating a `wsa://` URI is fire-and-forget:
/// `open::that` returns the moment the URI is handed to the shell, while WSA
/// takes twelve to seventeen seconds to come up. Without the toast the user
/// presses a button and is told nothing, which is the state Review Focus #3
/// exists to prevent.
///
/// Stopping does not. `am force-stop` is a real round trip that reports its
/// own failure, so a pre-toast would be a label over a wait rather than an
/// answer to one -- and `operation.stopping` names the *subsystem*
/// operation, which this is not.
enum Busy {
    Yes,
    No,
}

/// Register one action-callback pair.
///
/// The closure is not the work: it returns at once. Every handler here does
/// the same thing, because the window's thread must never be the thread that
/// waits for adb.
fn run(
    handle: Weak<AppWindow>,
    state: Arc<AppState>,
    action: Action,
    busy: Busy,
) -> impl Fn(SharedString) + 'static {
    move |name| {
        // A blank row carries no package. Rows like that are placeholders the
        // tier asked for (`home_model::placeholder_count`), and asking
        // `launch_app("")` would be a command sent in order to watch it fail.
        if name.is_empty() {
            return;
        }
        let package = name.to_string();
        let handle = handle.clone();
        let state = state.clone();
        let show_busy = matches!(busy, Busy::Yes);

        let _ = slint::spawn_local(async move {
            if show_busy {
                toast(&handle, i18n::t("operation.launching"));
            }
            // `launch_app` and `stop_app` each take their own `WsaOpGuard`
            // inside, which is what claims and releases the single operation
            // slot. Wrapping the call in a second guard would refuse the very
            // operation the inner one just authorised.
            let outcome = act(&state.wsa_dashboard, &package, &action).await;
            match outcome {
                Ok(()) => {
                    // Cleared here rather than left to the next poll: the poll
                    // is deliberately gated while an operation holds the slot
                    // (`should_refresh`), so waiting for it would leave the
                    // toast up for as long as the operation ran.
                    clear_toast(&handle);
                }
                Err(error) => toast(&handle, describe(&package, error)),
            }
        });
    }
}

async fn act(
    dashboard: &WsaDashboard,
    package: &str,
    action: &Action,
) -> Result<(), OpsError> {
    match action {
        Action::Launch => dashboard.launch_app(package).await,
        Action::Stop => dashboard.stop_app(package).await,
    }
}

/// The three buttons the expanded action strip keeps, and what each does.
///
/// `settings` is the simplest of the three and was the one that waited longest:
/// it activates a Windows shell namespace and needs no dialog at all, so it
/// could have been wired in P3 with `launch_app` and was not.
fn setup_card_actions(app: &AppWindow, handle: &Weak<AppWindow>, state: &Arc<AppState>) {
    // --- information: read the package, fill the dialog, show it -------------
    let ah = handle.clone();
    let st = state.clone();
    app.on_information_clicked(move |name| {
        if name.is_empty() {
            return;
        }
        let package = name.to_string();
        let ah = ah.clone();
        let st = st.clone();
        let _ = slint::spawn_local(async move {
            match st.wsa_dashboard.app_info(&package).await {
                Ok(info) => {
                    if let Some(app) = ah.upgrade() {
                        app.set_information(crate::Information {
                            package: info.package.clone().into(),
                            // Android leaves `versionName` empty for plenty of
                            // installed packages, and `AppInfo`'s own test
                            // pins that we do not invent one. So the dialog says
                            // the value was not reported -- a fact about the
                            // package, which is a different claim from "0".
                            version_name: if info.version_name.is_empty() {
                                i18n::t("dialog.unknown_version")
                            } else {
                                info.version_name.clone()
                            }
                            .into(),
                            version_code: info.version_code.to_string().into(),
                            apk_size: crate::ui::disk_model::size_text(info.apk_size_bytes).into(),
                            is_system: info.is_system,
                        });
                        app.set_show_information(true);
                    }
                }
                Err(error) => toast(&ah, describe(&package, error)),
            }
        });
    });

    // --- settings: open WSA's own Settings app -------------------------------
    // The card passes a package because every card callback takes one; there is
    // no per-package settings app, so the argument is ignored rather than
    // turned into something it is not.
    let ah = handle.clone();
    let st = state.clone();
    app.on_settings_clicked(move |_package| {
        let ah = ah.clone();
        let st = st.clone();
        let _ = slint::spawn_local(async move {
            if let Err(error) = st.wsa_dashboard.open_settings().await {
                toast(&ah, error.to_string());
            }
        });
    });

    // --- delete: ask first --------------------------------------------------
    // `adb uninstall` has no `-k` (spec 2.3, MAPPING 11.8), so it clears the
    // package's data. The confirmation is the only thing standing between a
    // mis-click and that, so it is not a toast and not a menu -- it is a dialog
    // whose only button is the red one, exactly as the reference's is.
    let ah = handle.clone();
    app.on_delete_clicked(move |name| {
        if name.is_empty() {
            return;
        }
        if let Some(app) = ah.upgrade() {
            app.set_package_to_delete(name.clone());
            app.set_show_delete_confirmation(true);
        }
    });

    // Cancel touches one boolean. There is nothing else to unwind: nothing was
    // started, and the toast belongs to whoever raised it.
    let ah = handle.clone();
    app.on_cancel_delete(move || {
        if let Some(app) = ah.upgrade() {
            app.set_show_delete_confirmation(false);
        }
    });

    let ah = handle.clone();
    app.on_close_information(move || {
        if let Some(app) = ah.upgrade() {
            app.set_show_information(false);
        }
    });

    // --- confirm: actually uninstall ----------------------------------------
    let ah = handle.clone();
    let st = state.clone();
    app.on_confirm_delete(move || {
        let Some(app) = ah.upgrade() else { return };
        let package = app.get_package_to_delete().to_string();
        if package.is_empty() {
            app.set_show_delete_confirmation(false);
            return;
        }
        // Closed before the round trip, not after. `adb uninstall` on a 2 GB
        // package can take seconds, and a confirmation that stays up while the
        // user waits looks like it did not register.
        app.set_show_delete_confirmation(false);
        let ah = ah.clone();
        let st = st.clone();
        let _ = slint::spawn_local(async move {
            toast(&ah, i18n::t("operation.deleting"));
            match st.wsa_dashboard.uninstall_apk(&package).await {
                Ok(()) => {
                    clear_toast(&ah);
                    // The row is gone; the poll would get to it in five seconds
                    // but is gated while an operation holds the slot, so it
                    // would not get to it until the operation finished anyway.
                    let _ = slint::spawn_local(crate::ui::data::refresh_home_with(
                        crate::ui::refresh::MANUAL_TRIGGER,
                        ah.clone(),
                        st.clone(),
                    ));
                }
                Err(error) => toast(&ah, describe(&package, error)),
            }
        });
    });
}

fn describe(package: &str, error: OpsError) -> String {
    match error {
        // The one failure with a second name to look up: what got there first
        // is what the user needs to be told, and the template is the
        // reference's own. The whole sentence lives in `home_model`, because
        // this file cannot be reached by the CLI's test runner.
        OpsError::Busy(busy) => crate::ui::home_model::busy_text(package, busy.current),
        other => other.to_string(),
    }
}

/// `open::that` is fire-and-forget by nature, so this reports nothing about
/// whether the URI worked -- only about whether the shell accepted it.
fn open_url(handle: Weak<AppWindow>) -> impl Fn(SharedString) + 'static {
    move |url| {
        // `wsa://` is how WSA's own surfaces are reached, and how
        // `DistroNotFoundView`'s link slot starts the subsystem in P2. Saying
        // so before handing it over is the difference between a click that
        // appears to do nothing and one that says it is starting. The label
        // clears on the next poll, which is gated while WSA is coming up, so
        // it stays until the subsystem answers.
        if url.starts_with("wsa://") {
            toast(&handle, i18n::t("operation.starting"));
        }
        // An address this build has no value for, or a scheme we will not hand
        // to the shell, gets the about page's explanation rather than
        // `open::that("")` -- which is not an error, it is a shell command with
        // no arguments, and the click would look like it did nothing. Most of the
        // rows that can produce one are on the about page, but the test lives here
        // because this is the one `open_url` the window has.
        if crate::ui::handlers::about::openable_url(url.as_str()).is_none() {
            crate::ui::handlers::about::open(&handle, url.to_string());
            return;
        }
        match open::that(url.as_str()) {
            Ok(_) => info!(%url, "opened"),
            Err(error) => warn!(%url, ?error, "could not open"),
        }
    }
}

/// Move one row.
///
/// There is no config to write the order to yet (P5), so it lives only in the
/// window: it survives polls because `repush_needed` will not rebuild a model
/// that would be identical, and is lost the moment the underlying snapshot
/// actually changes. That is a limitation, not a design, and it is why P5's
/// settings land alongside the page's other writes.
fn reorder(handle: Weak<AppWindow>) -> impl Fn(i32, i32) + 'static {
    move |from, to| {
        let Some(app) = handle.upgrade() else { return };
        let current = app.get_distros();
        let count = current.row_count() as i32;
        if from < 0 || to < 0 || from >= count || to >= count {
            return;
        }
        let mut rows: Vec<crate::Distro> = current.iter().collect();
        let row = rows.remove(from as usize);
        rows.insert(to as usize, row);
        app.set_distros(ModelRc::from(Rc::new(VecModel::from(rows))));
    }
}

fn clear(handle: Weak<AppWindow>) -> impl Fn() + 'static {
    move || clear_toast(&handle)
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

// No tests here, by the same rule `data.rs` records: this module names
// `AppWindow`, which only the GUI bin compiles, so anything written here
// would sit behind `cargo test --release --bin wsadash` and never run. Every
// sentence it assembles is in `home_model::busy_text`, which both trees
// compile and which the CLI's tests cover.
