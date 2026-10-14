// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! What the home page's buttons actually do.
//!
//! The counterpart of the reference's `src/ui/handlers/distro.rs`. Five of
//! the reference's callbacks are declared on `AppWindow` for P2 --
//! `start_distro`, `stop_distro`, `open_url`, `reorder_distros` and
//! `close_task_status`.
//!
//! Of the expanded action strip, only the three buttons spec Q22 keeps for WSA
//! render (`information`, `delete`, `settings`), and **none of the three is
//! wired**: `MainView` is instantiated binding only the five above, and all
//! three kept buttons want a modal, which arrives with `modal_manager.slint` in
//! P5. This note used to say they arrive in P3; P3 delivered the shell and
//! wired none of them. See `components/distro_card.slint` for the same fact.

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
