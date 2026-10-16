// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The about page's own work, which is very little -- and saying so is most of
//! this file.
//!
//! The counterpart of the reference's `src/ui/handlers/about.rs` (124 lines).
//! Four-fifths of that is a fetch, and the fetch is the part we do not do: it
//! calls `api1.wslui.com` for the project's documents URL, the discussions URL
//! and a group image picked by matching the reader's language against a list of
//! regions. Spec Q6 forbids calling that service, so none of it exists here.
//!
//! # What is left, and why each piece is honest
//!
//! | the reference                | here |
//! |------------------------------|------|
//! | `group_clicked` pops the group image | does nothing, exactly as the reference's own handler does when the image is not ready (`about.rs:15-22`) |
//! | `check_update_clicked` checks GitHub | says the check is not available in this build; P8's update check replaces the sentence |
//! | `open_url` opens whatever the server said | opens a configured address, or says there is none |
//!
//! The alternative in each row was a control that looks live and does nothing.
//! The reference ships live controls, so the shapes are the same; what differs is
//! that ours say why.
//!
//! # No `#[cfg(test)]` module
//!
//! For the reason `disk.rs` and `network.rs` give at length: every function here
//! names `AppWindow`, which only the GUI binary compiles, so tests written in
//! this file would sit behind `cargo test --release --bin wsadash` and never run.
//! The decisions worth testing -- which year the copyright says, whether a URL
//! is one we are willing to hand to the shell -- are [`copyright_years`] and
//! [`openable_url`], and they are `pub(crate)` so a caller on either side can
//! ask.

use std::sync::Arc;

use slint::Weak;
use tracing::{debug, info};

use crate::api;
use crate::app::constants;
use crate::app::state::AppState;
use crate::config;
use crate::i18n;
use crate::{AboutStrings, AppWindow};

/// What this build calls itself, for the update check's comparison.
///
/// The same string `app.rs` puts in the title bar and `startup_info.rs` logs,
/// read from the one place that cannot disagree with the package: Cargo's own
/// manifest version. A constant here would be a second copy of it.
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn setup(_app: &AppWindow, app_handle: Weak<AppWindow>, app_state: Arc<AppState>) {
    let _ = app_state;

    // Resolved once rather than on every visit. The strings are the same every
    // time, and the reference re-fills this struct from `data.rs` on each
    // refresh; nothing here depends on a refresh.
    let Some(window) = app_handle.upgrade() else { return };
    window.set_about_strings(about_strings().into());
}

pub fn reload(handle: Weak<AppWindow>) {
    let Some(app) = handle.upgrade() else { return };
    app.set_about_strings(about_strings().into());
}

/// The twenty fields, resolved now.
///
/// `github` is empty because the reference's is empty (`data.rs:144` says the UI
/// does not use it), and this one does not either.
fn about_strings() -> AboutStrings {
    AboutStrings {
        title: str_of("about.title"),
        description: str_of("about.description"),
        version: str_of("about.version"),
        updates: str_of("about.updates"),
        check_update: str_of("about.check_update"),
        website: str_of("about.website"),
        repository: str_of("about.repository"),
        group: str_of("about.group"),
        issues: str_of("about.issues"),
        announcements: str_of("about.announcements"),
        talk: str_of("about.talk"),
        documents: str_of("about.documents"),
        star: str_of("about.star"),
        love: str_of("about.love"),
        github: "".into(),
        license: str_of("about.license"),
        copyright: i18n::tr("about.copyright", &[copyright_years()]).into(),
        privacy_policy: str_of("about.privacy_policy"),
        terms_of_service: str_of("about.terms_of_service"),
    }
}

fn str_of(key: &str) -> slint::SharedString {
    i18n::t(key).into()
}

/// The year, or `2026-<year>` once the calendar has moved past 2026.
///
/// The reference's rule (`data.rs:147-152`) and ours: a copyright line that
/// silently rewrites itself every January is a small lie about when the work was
/// done, so the first year is pinned and later ones are appended.
pub(crate) fn copyright_years() -> String {
    // `Datelike` in scope for `year()`. The trait method is private until the
    // trait is imported, and the error names the method rather than the trait --
    // which reads like a chrono bug rather than a missing `use`.
    use chrono::Datelike;
    let now = chrono::Local::now().date_naive();
    if now.year() <= 2026 {
        "2026".to_string()
    } else {
        format!("2026-{}", now.year())
    }
}

/// The address to hand the shell, or `None` when there is not one.
///
/// Two questions, not one. The reference only ever hands `open` a URL its own
/// server just returned, so it never had to check; ours can be handed an empty
/// string by a `.slint` binding that has no value behind it, and `open::that("")`
/// is not an error -- it is a shell command with no arguments. And the scheme has
/// to be one we will pass on: `file:` is how the disk page opens Explorer, so it
/// belongs here, and anything else is a page this build has no reason to open.
pub(crate) fn openable_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();
    for scheme in ["https://", "http://", "mailto:", "file:"] {
        if lower.starts_with(scheme) {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Register the page's three callbacks.
pub fn setup_callbacks(app: &AppWindow, app_handle: Weak<AppWindow>) {
    // The group image. The reference pops a dialog when the picture is ready and
    // deliberately does *not* respond when it is not (`about.rs:15-22`); ours is
    // never ready, because the picture is the thing spec Q6 forbids fetching, so
    // this is the reference's second branch and nothing more.
    app.on_group_clicked({
        // Cloned for the same reason as everywhere else in this project: the
        // closure runs more than once, so it cannot consume the handle.
        let handle = app_handle.clone();
        move || {
        let Some(window) = handle.upgrade() else { return };
        if window.get_about_group_pic_ready() {
            window.set_show_about_group_popup(true);
        } else {
            debug!("about: no group image in this build; the reference does the same");
        }
        }
    });

    app.on_close_group_popup({
        let handle = app_handle.clone();
        move || {
            if let Some(window) = handle.upgrade() {
                window.set_show_about_group_popup(false);
            }
        }
    });

    // The update check (P8). A real one now: GitHub's own `releases/latest` for
    // `PROJECT_REPOSITORY`, on a thread of its own, with the answer delivered to
    // the event loop.
    //
    // **Not a `slint::spawn_local`**, which is what every other handler in this
    // tree uses: `ureq` is a blocking client and the request is allowed to take
    // fifteen seconds, so running it on the UI thread's executor would freeze
    // the window for exactly as long as the network is slow. The shape is
    // `handlers/scheduler.rs`'s, for the same reason -- `std::thread::spawn` +
    // `slint::invoke_from_event_loop` -- and the result is a toast either way,
    // including the failure, because a button that does nothing and a check that
    // found nothing look identical otherwise.
    app.on_check_update_clicked({
        let handle = app_handle.clone();
        move || {
            let handle = handle.clone();
            info!("about: update check requested");

            // Q2: no remote yet, so there is nothing to ask. Said without a
            // thread and without a socket, and it is a different sentence from
            // the one a failed request gets -- "this build has no address for
            // that" and "GitHub did not answer" are not the same problem.
            if constants::PROJECT_REPOSITORY.trim().is_empty() {
                toast(&handle, i18n::t("about.update_check_unavailable"));
                return;
            }

            // Synchronously, before the thread: the reader pressed a button and
            // gets an answer to the press immediately.
            toast(&handle, i18n::t("about.check_update"));

            std::thread::spawn(move || {
                let result = api::releases::latest_for(constants::PROJECT_REPOSITORY);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(app) = handle.upgrade() else { return };
                    let message = match result {
                        Ok(release) => {
                            // Recorded so the file can say when it last looked,
                            // which is what `check-time` is for and was, until
                            // now, a field nothing wrote.
                            let _ = config::update_user_settings(|user| {
                                user.check_time = config::stamp_now();
                            });
                            if api::releases::version_is_newer(VERSION, &release.tag) {
                                i18n::tr("about.new_version", &[release.tag.clone()])
                            } else {
                                i18n::t("about.up_to_date")
                            }
                        }
                        Err(error) => {
                            tracing::warn!(%error, "the update check failed");
                            i18n::tr("about.update_check_failed", &[error])
                        }
                    };
                    app.set_task_status_text(message.into());
                    app.set_task_status_visible(true);
                });
            });
        }
    });
}

/// Open one of the page's links.
///
/// The reference's `open_url` is wired to `handlers/common.rs` and shared with
/// the home page's cards; ours is the same shared callback, and this is where
/// the empty case is caught.
pub fn open(handle: &Weak<AppWindow>, raw: String) {
    match openable_url(&raw) {
        Some(url) => {
            debug!(url = %url, "about: opening a link");
            if let Err(error) = open::that(&url) {
                tracing::warn!(%error, "could not open the link");
            }
        }
        None => {
            // The row is here because the reference's page has it; the address is
            // not here because spec Q6 forbids the service that would have
            // supplied it. Saying which is the difference between a build that is
            // incomplete and one that is lying.
            toast(handle, i18n::t("about.link_not_configured"));
        }
    }
}

fn toast(handle: &Weak<AppWindow>, text: String) {
    if let Some(app) = handle.upgrade() {
        app.set_task_status_text(text.into());
        app.set_task_status_visible(true);
    }
}