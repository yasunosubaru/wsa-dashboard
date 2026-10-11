// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Polling, debouncing, and the push to the window.
//!
//! The counterpart of the reference's `src/ui/data.rs`, which is a 976-line
//! file covering eight pages. Only the home page's part is here for P2; the
//! rest arrives with the pages that own it.
//!
//! Deliberately thin, and tested indirectly. Everything with a decision in it
//! -- [`crate::ui::refresh::should_refresh`], [`crate::ui::refresh::repush_needed`],
//! `HomeViewModel::placeholder_count`, `snapshot_changed` -- lives in the two
//! pure modules beside it, which compile into the CLI bin and so are covered by
//! `cargo test --release --bin wsadash`. What is left here cannot be: it names
//! `AppWindow` and `Distro`, and a test target that could reach those would
//! have to open a window to run. So it does the least it can and defers.

#![allow(dead_code)]

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slint::{ModelRc, VecModel, Weak};

use crate::i18n;
use crate::ui::home_model::{build, AppRow, HomeInputs, HomeSnapshot, HomeViewModel};
use crate::ui::refresh::{repush_needed, should_refresh, PERIODIC_TRIGGER};
use crate::wsa::models::{SubsystemState, SubsystemStatus};
use crate::{AppWindow, AppState, Distro};

/// How long the pump between two polls is, once [`should_refresh`] has allowed
/// one through. Slower than [`crate::ui::refresh::DEBOUNCE`], because this is
/// the only caller on a timer and `pm list packages` is not free.
const TICK: Duration = Duration::from_secs(5);

/// The interval Task 8's task drives the pump on.
///
/// Not a literal there, so that changing it here changes it everywhere -- the
/// number is the only thing standing between "the page is live" and "the page
/// is a second behind", and there is no reason for two copies of it.
pub fn poll_interval() -> Duration {
    TICK
}

/// When the last refresh was allowed through.
static LAST_REFRESH: Mutex<Option<Instant>> = Mutex::new(None);

/// The last list the window was given: the snapshot its rows came from (none,
/// when the page was showing placeholders) and the query it was filtered with.
static LAST_PUSHED: Mutex<Option<(Option<HomeSnapshot>, String)>> = Mutex::new(None);

/// The best snapshot seen while the subsystem was still answerable. It only
/// ever grows, which is the whole point: a tier that drops keeps its rows.
static LAST_GOOD: Mutex<Option<HomeSnapshot>> = Mutex::new(None);

/// One refresh at a time. A second caller arriving mid-flight is not a second
/// opinion, it is the same work started twice.
static IS_REFRESHING: AtomicBool = AtomicBool::new(false);

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // A poisoned lock means a thread panicked while holding it. What is in
    // here is a cache and a clock: dropping both would silently restart the
    // debounce and lose every row, where poisoning costs one bad read.
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The pump the timer drives.
///
/// Safe to call far more often than it does anything: [`should_refresh`] is the
/// gate, and it refuses most calls.
pub async fn refresh_home(weak: Weak<AppWindow>, state: Arc<AppState>) {
    if IS_REFRESHING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    // Puts the flag back whatever happens below. A refresh that leaves it set
    // would stop every future one, which is a worse failure than any it can
    // cause here.
    struct RefreshGuard;
    impl Drop for RefreshGuard {
        fn drop(&mut self) {
            IS_REFRESHING.store(false, Ordering::SeqCst);
        }
    }
    let _guard = RefreshGuard;

    // One place decides whether an operation is running: the dashboard's own
    // single slot. Reading it here rather than keeping a second flag is what
    // keeps the two from disagreeing.
    let busy = state.wsa_dashboard.get_active_op().is_some();
    let visible = state.is_visible.load(Ordering::SeqCst);
    let elapsed = lock(&LAST_REFRESH).map(|at| at.elapsed());
    if !should_refresh(PERIODIC_TRIGGER, visible, busy, elapsed) {
        return;
    }
    *lock(&LAST_REFRESH) = Some(Instant::now());

    let status = probe(&state).await;
    let fresh = if status.is_ready() {
        match state.wsa_dashboard.list_apps(false).await {
            Ok(apps) => {
                let running = state.wsa_dashboard.running_apps().await;
                Some(HomeSnapshot { apps, running })
            }
            Err(error) => {
                // Ready means the subsystem answered a moment ago, so this is
                // not "there are no apps": it is a read that failed. The page
                // falls back to whatever it last saw, which is why that is
                // kept.
                tracing::warn!(?error, "list_apps failed");
                None
            }
        }
    } else {
        None
    };

    if let Some(snapshot) = fresh.as_ref() {
        *lock(&LAST_GOOD) = Some(snapshot.clone());
    }
    let cached = lock(&LAST_GOOD).clone();
    // What the list is being built from, so the push can tell whether it would
    // be saying anything new.
    let source = fresh.as_ref().or(cached.as_ref()).cloned();

    let _ = slint::invoke_from_event_loop(move || {
        let Some(app) = weak.upgrade() else { return };
        // Reading the query has to happen here: the LineEdit owns the string
        // and lives on the window's thread.
        let query = app.get_search_query().to_string();
        let model = build(&HomeInputs {
            status: &status,
            fresh: fresh.as_ref(),
            last_good: cached.as_ref(),
            active_op: state.wsa_dashboard.get_active_op(),
            query: &query,
        });
        apply(&app, &model, source.as_ref(), &query);
    });
}

/// An `Err` here has no better answer than "we could not ask".
///
/// Every state the probe can *reach* is reported by itself, including "absent",
/// which the probe folds into `Stopped`. So an `Err` is not one of those: it
/// means adb could not be run at all -- not on PATH, or `WSADASH_ADB` pointing
/// at nothing -- and `Offline` is how the page renders that. Not "not
/// installed", not "not running": unknown this moment.
async fn probe(state: &AppState) -> SubsystemStatus {
    match state.wsa_dashboard.probe().await {
        Ok(status) => status,
        Err(error) => {
            tracing::warn!(?error, "probe failed");
            SubsystemStatus {
                state: SubsystemState::Offline,
                adb_port: state.wsa_dashboard.adb_port(),
                boot_completed: false,
            }
        }
    }
}

fn apply(app: &AppWindow, model: &HomeViewModel, source: Option<&HomeSnapshot>, query: &str) {
    // The scalars below are cheap, and some of them move with state the list
    // cannot see -- an operation starting, the subsystem dropping a tier -- so
    // they are set every time. The list is not. `repush_needed` decides that.
    app.set_home_loading(model.degraded == crate::ui::home_model::Degraded::LoadingOverlay);
    app.set_home_not_found_text(text_of(model.headline_key()).into());
    app.set_home_guide_url(model.action_url().into());
    app.set_home_guide_link_text(text_of(model.action_key()).into());
    app.set_home_greyed(model.greyed);
    app.set_home_buttons_disabled(model.buttons_disabled);
    app.set_task_status_text(text_of(model.toast_key).into());
    app.set_task_status_visible(!model.toast_key.is_empty());

    // Scoped so the guard is gone before the write below: `std::sync::Mutex`
    // is not reentrant, and a second `lock` on this same static while the first
    // is still held would hang the event loop this runs on.
    let changed = {
        let pushed = lock(&LAST_PUSHED);
        let previous = pushed
            .as_ref()
            .and_then(|(source, query)| source.as_ref().map(|source| (source, query.as_str())));
        repush_needed(previous, source, query)
    };

    if changed {
        let rows = to_slint_rows(model);
        app.set_distros(ModelRc::from(Rc::new(VecModel::from(rows))));
        *lock(&LAST_PUSHED) = Some((source.cloned(), query.to_string()));
    }
}

/// An empty key is the model saying "nothing to say"; passing it to `tr` would
/// look up the empty string and warn about a key that was never meant to exist.
fn text_of(key: &str) -> String {
    if key.is_empty() {
        String::new()
    } else {
        i18n::tr(key, &[])
    }
}

/// The model's rows as the window's `Distro` values, plus blank cards when the
/// model asked for them.
fn to_slint_rows(model: &HomeViewModel) -> Vec<Distro> {
    let mut rows: Vec<Distro> = model.rows.iter().map(row).collect();
    for _ in 0..model.placeholder_count() {
        rows.push(blank());
    }
    rows
}

fn row(from: &AppRow) -> Distro {
    Distro {
        name: from.display_name.clone().into(),
        // The card's three-way lookup keys off these two words, so they are
        // the reference's spellings and not ours.
        status: if from.running { "Running" } else { "Stopped" }.into(),
        version: from.version.clone().into(),
        // The reference is default-distro aware; WSA has one subsystem and no
        // notion of a default among them.
        is_default: false,
        // No icon source: the platform exposes no application label, and so no
        // drawable -- the same reason `display_name` is regularised out of the
        // package name (spec 1.7 / risk R3).
        icon: Default::default(),
        has_icon: false,
        initial: from.initial.clone().into(),
        // The tooltip carries what the label could not: the package itself.
        distro_display_name: from.package.clone().into(),
        // Self-start is P7, and WSA does not boot on login by default.
        boot_run: false,
        notes: String::new().into(),
        initial_color_index: from.initial_color_index,
    }
}

/// A blank card.
///
/// Its empty `status` is deliberate. The card looks up "Running" and "Stopped"
/// and falls through to a third key for anything else; an empty string is
/// therefore a value that *means* "unknown", and that key reads "Unavailable"
/// in this project -- the honest word for an app whose state cannot be known
/// right now.
fn blank() -> Distro {
    Distro {
        name: String::new().into(),
        status: String::new().into(),
        version: String::new().into(),
        is_default: false,
        icon: Default::default(),
        has_icon: false,
        initial: String::new().into(),
        distro_display_name: String::new().into(),
        boot_run: false,
        notes: String::new().into(),
        initial_color_index: 0,
    }
}
