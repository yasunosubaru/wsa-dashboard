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
use tracing::error;

use crate::app::constants::{APP_NAME, PROJECT_REPOSITORY};
use crate::app::state::AppState;
use crate::i18n;
use crate::{app, ui};

/// The languages shipped so far.
///
/// One entry, deliberately: `i18n` has a single table until P6 builds the
/// `en.toml` set out, and offering a language picker with one language in it is
/// a control that cannot be used. The array is built here rather than written
/// into `theme.slint` for the reason the reference records at this point --
/// a literal long enough to matter has caused a Slint compiler stack overflow.
fn language_options() -> ModelRc<slint::SharedString> {
    let options: Vec<slint::SharedString> = i18n::available_languages()
        .into_iter()
        .map(slint::SharedString::from)
        .collect();
    ModelRc::from(Rc::new(VecModel::from(options)))
}

/// Open the window and run the event loop until it closes.
pub async fn run_app(state: AppState) {
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

    // -- the i18n bridge (reference runner.rs:78-97) ---------------------------
    app.global::<AppI18n>().on_t(|key, args| {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        i18n::tr(&key, &args).into()
    });
    app.global::<AppI18n>()
        .set_locale_code(i18n::current_lang().into());
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
