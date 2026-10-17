// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Everything between "we have a state" and "the window is on screen".
//!
//! The counterpart of the reference's `src/app/runner.rs`. What is here is the
//! reference's steps 1–3 and 6, in the reference's order: the i18n bridge, the
//! `AppInfo` values, the handler registration, the monitor task, `app.run`.
//! Steps 4 (complex CLI), 5 (single instance, log rotation, start-on-login) and
//! the tray are P7/P8; step 7 (the helper bootstrap) belongs to `api1.wslui.com`
//! and is deliberately absent -- spec Q6 forbids calling it.
//!
//! `slint::include_modules!()` lives here rather than in `main.rs` (plan C1) so
//! that `main.rs` stays a sequencing file you can read top to bottom, and so
//! that the generated types are declared beside the code that configures them.

slint::include_modules!();

use std::rc::Rc;
use std::sync::Arc;

use slint::{ComponentHandle, Model, ModelRc, VecModel};
use tracing::{error, info};

use crate::app::constants::{APP_NAME, PROJECT_REPOSITORY};
use crate::app::state::AppState;
use crate::i18n;
use crate::{app, ui};

/// The languages this build can render.
///
/// Built rather than written into `theme.slint`, for the reason the reference
/// records at this point -- a literal long enough to matter has caused a Slint
/// compiler stack overflow -- and because `i18n::available_languages()` is the
/// list of files in the binary. Fifty packs, one source.
pub(crate) fn language_options() -> ModelRc<slint::SharedString> {
    let options: Vec<slint::SharedString> = i18n::available_languages()
        .into_iter()
        .map(slint::SharedString::from)
        .collect();
    ModelRc::from(Rc::new(VecModel::from(options)))
}

/// Open the window and run the event loop until it closes.
pub async fn run_app(state: AppState) {
    // -- the single-instance handshake, before any window (P7) ---------------
    // Before `AppWindow::new`, not after: creating the window is the step that
    // puts a caption on screen, and a second launch that has already drawn one
    // flash is a launch the reader saw.
    let _single_instance = crate::app::single_instance::SingleInstance::new(
        &crate::app::single_instance::mutex_name(),
    );
    if !_single_instance.is_single() {
        if crate::app::single_instance::try_activate_existing_instance() {
            info!("another instance is running and has been woken; this one exits");
        } else {
            // The mutex was owned but nobody answered the pipe and no window was
            // found: the other process is starting, or it is dying. Returning
            // rather than proceeding is the safe half of that choice -- a second
            // window appearing over a first that is mid-startup is the confusing
            // version -- and the reader's second double-click gets the answer.
            info!("the mutex is owned but nothing answered; exiting");
        }
        return;
    }

    let app = match AppWindow::new() {
        Ok(app) => app,
        Err(error) => {
            error!(?error, "failed to create the window");
            return;
        }
    };

    // P2 still has the system frame, so the reference's
    // `WSA_DASHBOARD_WINDOW_UI` would be a caption a user reads. See
    // `app.slint` for why this has to be a property: Slint 1.17 has no
    // `Window::set_title`, and the `.slint` file keeps the constant as its
    // default for the P7 single-instance scan to find.
    app.set_window_title(APP_NAME.into());

    // -- the settings, before the first paint (P6) ----------------------------
    // Read here rather than inside `handlers::settings::setup` because three of
    // these have to be true before anything is drawn: the window opens in the
    // saved language, not in English with the English swapped a moment later;
    // `Theme.dark_mode` and the side-bar switches are already right rather than
    // flipping on the reader's first click.
    //
    // `reload` is the same function the settings page calls when it is opened,
    // so the page and the file cannot disagree about what the settings are.
    ui::handlers::settings::reload(app.as_weak());

    // -- the i18n bridge (reference runner.rs:78-97) ---------------------------
    app.global::<AppI18n>().on_t(|key, args| {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        i18n::tr(&key, &args).into()
    });
    app.global::<AppI18n>()
        .set_locale_code(i18n::current_lang().into());
    // Whatever the file asked for, loaded before the version bump below, so the
    // first `t(...)` a `.slint` file evaluates already has the right table behind
    // it. `load_resources` normalises, so a pack that is not shipped lands on
    // English rather than on an empty table.
    i18n::load_resources(&crate::config::load().user.ui_language);
    app.global::<AppI18n>()
        .set_locale_code(i18n::current_lang().into());
    app.global::<AppI18n>().set_is_rtl(i18n::is_rtl(&i18n::current_lang()));
    // Not a timestamp or a build number: a value bumped when the table changes,
    // so that every `AppI18n.t(...)` expression re-evaluates. Setting it is what
    // makes the first paint translated rather than the second.
    app.global::<AppI18n>().set_version(1);
    app.global::<AppI18n>()
        .set_language_options(language_options().into());
    app.global::<AppI18n>().on_get_language_index(|lang| {
        i18n::available_languages()
            .iter()
            .position(|candidate| candidate == &lang.to_string())
            .map(|index| index as i32)
            .unwrap_or(0)
    });

    // -- AppInfo (reference runner.rs:99-103) ---------------------------------
    app.global::<AppInfo>()
        .set_version(env!("CARGO_PKG_VERSION").into());
    // `arch_suffix`, `issues_url`, `donate_url` and the rest are set when the
    // page that renders them exists (P9's About). Setting them now would put
    // `PROJECT_REPOSITORY + "/issues"` -- `"/issues"` -- behind a link nobody
    // has built yet, and a wrong link is worse than an unset one.
    app.global::<AppInfo>()
        .set_project_repository(PROJECT_REPOSITORY.into());

    let state = Arc::new(state);

    ui::handlers::setup(&app, app.as_weak(), state.clone()).await;

    // -- the Run entry, and the startup line (P7) -----------------------------
    // Both after the handlers, because the tray switches the Run entry follows are
    // read by `handlers::settings` when it sets up, and both before `app.run`, so a
    // failure is logged before the window's first paint rather than after.
    //
    // The switches are read again here rather than reused from the settings page,
    // and the reason is that the Run entry has to be written whether or not the
    // reader ever opens that page.
    //
    // The startup line is spawned rather than awaited: it runs `cmd /c ver`, and a
    // reader should not see the window later because a process they did not launch
    // was slow to start.
    let tray = crate::config::load().tray;
    let _ = crate::app::autostart::apply_tray_autostart(tray.autostart, tray.start_minimized).await;
    let _ = tokio::spawn(crate::app::startup_info::log_startup_system_info());

    // Per-two-second UI counters (CPU investigation). Inside the process, so the
    // answer cannot be moved by what else the machine is doing -- which is what
    // made the external sampling useless: the same binary read 86.6% and 0.0%
    // twenty minutes apart.
    tokio::spawn(async move {
        let mut last = [0u64; 11];
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let now = crate::ui::data::counters::snapshot();
            let d: Vec<u64> = now
                .iter()
                .zip(last.iter())
                .map(|(now, then)| now.saturating_sub(*then))
                .collect();
            tracing::info!(
                "ui/2s refresh={} probe={}({}ms) list={}({}ms) running={}({}ms) build={}({}ms) apply={}({}ms)",
                d[0], d[1], d[6], d[2], d[7], d[3], d[8], d[4], d[9], d[5], d[10]
            );
            last = now;
        }
    });

    // -- the tray, and the window's first appearance (P7) ---------------------
    // `initially_visible` is the state the window will actually be in, so that a
    // later rebuild (a language change) cannot make a hidden window visible.
    if let Err(error) = crate::app::tray::SystemTray::initialize(app.as_weak(), !tray.start_minimized)
    {
        // Logged rather than fatal: a machine whose notification area refuses a
        // new icon is unusual, and the window works without one.
        error!(%error, "could not create the tray icon");
    }
    app.on_reinit_tray({
        let weak = app.as_weak();
        move || {
            let visible = weak
                .upgrade()
                .map(|app| app.get_is_window_visible())
                .unwrap_or(false);
            if let Err(error) = crate::app::tray::SystemTray::initialize(weak.clone(), visible) {
                error!(%error, "could not rebuild the tray icon");
            }
        }
    });

    // Start-minimised parks the window before `app.run`, which is what would
    // otherwise put it on screen. The ordinary path does nothing here and lets
    // Slint show it, so that a normal launch does not go through the tray's
    // hide-then-restore round trip and flash on the way up.
    if tray.start_minimized {
        ui::handlers::window::park_in_tray(&app);
    }

    app::tasks::spawn_home_monitor(app.as_weak(), state);

    if let Err(error) = app.run() {
        error!(?error, "the event loop ended with an error");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window's title is what the single-instance check will match on in
    /// P7, and it is what a user sees on the taskbar today. A placeholder
    /// leaking into the caption is the kind of thing that ships unnoticed, and
    /// the binding that prevents it is in `.slint`, where a unit test cannot
    /// render it -- so the two halves are checked separately: the constant
    /// here, and the property that holds it below.
    #[test]
    fn the_visible_title_is_the_application_name() {
        assert_eq!(APP_NAME, "WSA Dashboard");
    }

    /// The `.slint` default is the string P7's single-instance scan looks for
    /// (reference `app/single_instance.rs:28` lists both spellings). Reading
    /// the file rather than constructing a window keeps this test off the
    /// display server -- the same trade the i18n key-scan makes.
    #[test]
    fn the_handshake_title_still_defaults_to_the_reference_constant() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/app.slint");
        let text = std::fs::read_to_string(&root)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", root.display()));
        assert!(
            text.contains("window_title: \"WSA_DASHBOARD_WINDOW_UI\";"),
            "the default is what P7 scans for; changing it breaks the \
             single-instance handshake silently"
        );
    }

    /// One language in the list, and it is the one `current_lang` reports --
    /// a picker whose only option is not the active language would report an
    /// index the table has no entry for.
    #[test]
    fn the_language_list_matches_what_is_actually_loaded() {
        let options = language_options();
        assert!(
            options.row_count() > 0,
            "an empty list is a picker that cannot be used"
        );
        let current = i18n::current_lang();
        assert!(
            options.iter().any(|option| option.to_string() == current),
            "the active language {current:?} is not among the picker's options"
        );
    }

    /// Same claim, through the index callback the picker reads: every option
    /// must resolve to a position that exists.
    #[test]
    fn every_option_resolves_to_a_usable_index() {
        let options = language_options();
        for (index, option) in options.iter().enumerate() {
            let option = option.to_string();
            let resolved = i18n::available_languages()
                .iter()
                .position(|candidate| *candidate == option);
            assert_eq!(resolved, Some(index));
        }
    }
}
