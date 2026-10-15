// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The settings page's own work: read `settings.toml` into the window, write it
//! back when a Save button is pressed, re-resolve the language when one is
//! chosen, and give the four folder buttons something to open.
//!
//! The counterpart of the reference's `src/ui/handlers/settings.rs` (413 lines)
//! together with the `SettingsStrings` block it reads in `handlers/data.rs:98`.
//!
//! # The four directory buttons
//!
//! | the reference                       | here                                       |
//! |-------------------------------------|--------------------------------------------|
//! | pick the distribution install dir  | open the folder WSA keeps its data in      |
//! | pick the backup/export dir          | open the folder this program owns          |
//! | pick the logs dir                   | open the folder the logs are written to    |
//! | open `~/.wslconfig` in an editor    | open this program's settings directory    |
//!
//! The reference's three pickers opened a `rfd` folder chooser. Ours has
//! nothing to choose -- the first two are fixed by what the subsystem and this
//! program actually use -- so a chooser would be a dialog whose only answer is
//! the answer. What is left is still a real action with a visible result, which
//! is the property that matters: a button that opens a folder is doing
//! something, and a button that saves a string nothing reads is not.
//!
//! # The two rows that changed meaning
//!
//! `sparse_vhd` and `mount_auto_probe` are WSL concepts. Rather than ship two
//! checkboxes that persist a value with no consequence, each became the
//! nearest real thing this program can do -- `disk_model` reports allocated
//! rather than logical size, and the storage page refreshes when the window
//! opens. The label and the description under each say which, so the switch is
//! described by what it does. `config::UserSettings` says the same thing next
//! to the fields, and MAPPING 15 records it.
//!
//! No `#[cfg(test)]` module, for the reason `disk.rs` gives at length: every
//! function here names `AppWindow`, which only the GUI binary compiles. The
//! decisions that are worth testing live in `config`, which does.

use std::sync::Arc;

use slint::{ComponentHandle, Weak};
use tracing::{debug, info, warn};

use crate::app::constants;
use crate::app::state::AppState;
use crate::config::{self, Config};
use crate::i18n;
use crate::ui::disk_model;
use crate::{AppI18n, AppWindow, SettingsStrings, Theme};

pub fn setup(app: &AppWindow, app_handle: Weak<AppWindow>, app_state: Arc<AppState>) {
    // The picker's list. `runner::language_options` already computes it for
    // `AppI18n.language-options`, so this asks for the same one rather than
    // making a second list that could be a different length.
    app.set_settings_language_options(crate::app::runner::language_options());
    reload(app_handle.clone());

    // ---- save -------------------------------------------------------------
    // Three buttons, three sets of fields, one file. Split by tab rather than
    // one Save for the page because that is the reference's shape, and because
    // it is what lets each toast name what was written -- a reader who presses
    // Save on Advanced and is told the interface was saved has learned that the
    // toast is not about their click.
    app.on_save_general_settings({
        let handle = app_handle.clone();
        move || save(handle.clone(), Section::General)
    });
    app.on_save_advanced_settings({
        let handle = app_handle.clone();
        move || save(handle.clone(), Section::Advanced)
    });
    app.on_save_interface_settings({
        let handle = app_handle.clone();
        move || save(handle.clone(), Section::Interface)
    });

    // ---- the four folder rows ---------------------------------------------
    app.on_select_distro_folder({
        let handle = app_handle.clone();
        move || open_folder(handle.clone(), Folder::WsaData)
    });
    app.on_select_logs_folder({
        let handle = app_handle.clone();
        move || open_folder(handle.clone(), Folder::Logs)
    });
    app.on_select_backup_folder({
        let handle = app_handle.clone();
        move || open_folder(handle.clone(), Folder::AppData)
    });
    app.on_open_wsl_settings({
        let handle = app_handle.clone();
        move || open_folder(handle.clone(), Folder::AppData)
    });

    // ---- stop the subsystem -----------------------------------------------
    // The reference's button here asks for confirmation and then stops WSL.
    // Ours stops the subsystem without a second dialog, and the reason is that
    // the confirmation already exists: the home page's Stop button is where a
    // reader confirms they mean to stop WSA. A dialog here would be asking the
    // same question twice on two different pages. The callback name is the
    // reference's (Q12); MAPPING 15 says what it does.
    app.on_stop_wsl({
        let handle = app_handle.clone();
        let state = app_state.clone();
        move || {
            // `WsaDashboard` holds a process handle and a pipe, neither of which
            // are cheap or useful to duplicate -- so the `Arc` comes along
            // instead of the dashboard. Cloned here rather than moved, because
            // this closure runs more than once.
            let handle = handle.clone();
            let state = state.clone();
            let _ = slint::spawn_local(async move {
                match state.wsa_dashboard.stop_subsystem().await {
                    Ok(()) => toast(&handle, i18n::t("dialog.stop_wsl_success")),
                    Err(error) => {
                        warn!(%error, "could not stop the subsystem");
                        toast(&handle, error.to_string());
                    }
                }
            });
        }
    });

    // ---- language ---------------------------------------------------------
    // The picker hands over a code rather than an index, so there is no lookup
    // table here at all -- see `language_select_dialog.slint`.
    //
    // Loading and saving are one gesture: a language chosen here is a choice
    // about the next launch too, and the reference persists it for the same
    // reason (`settings.rs:71`). The Save button on the General tab still works
    // for everything else on that tab.
    app.on_language_selected({
        let handle = app_handle.clone();
        move |code| {
            let code = code.to_string();
            apply_language(&handle, &code);
            save(handle.clone(), Section::General);
            debug!(language = %code, "language changed");
        }
    });
}

/// Which third of the page a Save button belongs to.
#[derive(Clone, Copy, Debug)]
enum Section {
    General,
    Advanced,
    Interface,
}

/// Which directory a row names.
#[derive(Clone, Copy, Debug)]
enum Folder {
    /// `LocalCache` of the WSA package -- where the subsystem keeps its data.
    WsaData,
    /// `~/.wsadashboard/logs`.
    Logs,
    /// `~/.wsadashboard`.
    AppData,
}

impl Folder {
    /// The directory, or `None` when it is not there.
    ///
    /// `None` is a state rather than a failure: `LocalCache` does not exist
    /// until WSA has run, which is exactly the machine state spec section 11
    /// line 348 is about. The row says so instead of opening Explorer at a path
    /// that fails to appear.
    fn path(self) -> Option<std::path::PathBuf> {
        match self {
            Folder::WsaData => crate::wsa::app::storage::cache_dir(),
            Folder::Logs => {
                let path = config::logs_dir();
                path.is_dir().then_some(path)
            }
            Folder::AppData => {
                let path = config::config_dir();
                path.is_dir().then_some(path)
            }
        }
    }
}

/// Read the settings file into the window, and resolve the strings it shows.
pub fn reload(handle: Weak<AppWindow>) {
    let Some(app) = handle.upgrade() else { return };
    let config = config::load();
    apply(&app, &config);
}

/// Copy a [`Config`] onto the window.
///
/// One function, three callers: at startup, when the settings tab is opened, and
/// after a save. Splitting it would be three places to keep in step, and the
/// failure is silent -- the page would show a value the file disagrees with and
/// nothing would say so.
fn apply(app: &AppWindow, config: &Config) {
    let user = &config.user;
    app.set_ui_language(user.ui_language.clone().into());
    app.set_logs_location(user.logs_location.clone().into());
    app.set_log_level(user.log_level as i32);
    app.set_log_days(user.log_days as i32);
    app.set_check_update_interval(config.user.check_update as i32);
    app.set_auto_shutdown(user.auto_shutdown);
    app.set_system_color(user.system_color);
    app.set_colorful_icons(user.colorful_icons);
    app.set_mail_icon_always(user.mail);
    app.set_hide_pin_icon(user.hide_pin);
    app.set_show_drag(user.show_drag);
    app.set_sparse_vhd(user.sparse_vhd);
    app.set_mount_auto_probe(user.mount_auto_probe);

    let tray = &config.tray;
    app.set_tray_autostart(tray.autostart);
    app.set_tray_start_minimized(tray.start_minimized);
    app.set_tray_close_to_tray(tray.close_to_tray);

    let sidebar = &config.sidebar;
    app.set_sidebar_toggle(sidebar.toggle);
    // The eight switches, each one gated on the page existing.
    //
    // `SidebarConfig::default()` says all eight are on, because the reference
    // ships all nine pages. Taking that at face value is how Donate and Task
    // Scheduler appeared in the side bar during P6 with nothing behind them --
    // an entry that leads to an empty content area, which is the one outcome
    // `tabs::has_a_view` exists to prevent.
    //
    // So the file says what the reader wants and `has_a_view` says what this
    // build has, and the entry is shown when both agree. It also means the file
    // needs no edit when a phase lands: the `true` that was already in it starts
    // taking effect the moment the page appears.
    app.set_sidebar_add(offered(sidebar.add, 1));
    app.set_sidebar_usb(offered(sidebar.usb, 2));
    app.set_sidebar_network(offered(sidebar.network, 3));
    app.set_sidebar_scheduler(offered(sidebar.scheduler, 4));
    app.set_sidebar_mount(offered(sidebar.mount, 5));
    app.set_sidebar_donate(offered(sidebar.donate, 6));
    app.set_sidebar_about(offered(sidebar.about, 8));

    // The two rows whose directory is a fact rather than a choice. Both come
    // from the same place the storage page reads them from, so the settings page
    // and the storage page cannot name two different directories.
    app.set_distro_location(
        crate::wsa::app::storage::cache_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
            .into(),
    );
    app.set_backup_location(config::config_dir().to_string_lossy().into_owned().into());

    // Index into the picker's list rather than a language code, because that is
    // what the view holds. `unwrap_or(0)` is English, which is the one entry
    // that is always there.
    let index = i18n::available_languages()
        .iter()
        .position(|name| *name == user.ui_language)
        .unwrap_or(0) as i32;
    app.set_selected_language_index(index);

    app.set_settings_strings(settings_strings().into());
}

/// Load a language and re-resolve everything the window shows.
fn apply_language(handle: &Weak<AppWindow>, code: &str) {
    let previous = i18n::current_lang();
    i18n::load_resources(code);
    let loaded = i18n::current_lang();
    let Some(app) = handle.upgrade() else { return };

    // `version` is the lever, and it is the reference's lever (settings.rs:52):
    // every `AppI18n.t(...)` in every `.slint` file passes it as an argument, so
    // bumping it re-evaluates them all. Without it the page's own labels would
    // keep the language they were painted in while everything around them
    // changed.
    app.global::<AppI18n>().set_is_rtl(i18n::is_rtl(&loaded));
    app.global::<AppI18n>().set_locale_code(loaded.clone().into());
    app.global::<AppI18n>().set_version(app.global::<AppI18n>().get_version() + 1);

    // A font that can draw the new language's script. Skipped when the language
    // did not change, because assigning a font family re-lays every element in
    // the window.
    if previous != loaded {
        let system_lang = config::load().system.system_language;
        app.global::<Theme>()
            .set_default_font(constants::get_font_for(&loaded, &system_lang).into());

        // The tray's tooltip and menu are built on the Rust side out of translated
        // strings, so the `version` bump above cannot reach them -- the icon has to
        // be rebuilt. This is the other half of the same problem the version lever
        // solves for the `.slint` strings, and the reference does it from the same
        // place (`settings.rs:61-68`).
        app.invoke_reinit_tray();
    }

    // The `*Strings` structs are resolved on this side of the bridge, so they
    // need their own pass: `apply` re-resolves them, and it needs the language
    // that was just chosen rather than the one in the file, because the save
    // that follows is what puts it there.
    let mut config = config::load();
    config.user.ui_language = code.to_string();
    apply(&app, &config);
}

/// Whether a side-bar entry is offered: the file wants it *and* this build has
/// the page behind it.
///
/// The two questions are separate and both real. `SidebarConfig` records what a
/// reader asked for, and survives a phase landing; `tabs::has_a_view` records
/// what exists, and does not. Answering only the first is how P6 briefly showed
/// a Donate entry with nothing behind it.
fn offered(wanted: bool, tab: i32) -> bool {
    wanted && crate::ui::tabs::has_a_view(tab)
}

/// The twenty-eight strings the page shows.
///
/// A struct rather than `AppI18n.t` at each site, and for the reference's reason
/// (`settings/mod.slint` and `data.rs:98`): a `.slint` file with a few hundred
/// `t()` calls is what makes the Slint compiler run out of stack.
fn settings_strings() -> SettingsStrings {
    SettingsStrings {
        language: str_of("settings.language"),
        auto_update: str_of("settings.update_interval"),
        log_level: str_of("settings.log_level"),
        log_retention: str_of("settings.log_retention_days"),
        log_path: str_of("settings.log_path"),
        select_folder: str_of("settings.select_folder"),
        install_dir: str_of("settings.distro_dir"),
        backup_dir: str_of("settings.backup_dir"),
        auto_close: str_of("settings.auto_shutdown_msg"),
        system_color: str_of("settings.system_color"),
        auto_start: str_of("settings.tray_autostart"),
        minimize_tray: str_of("settings.tray_start_minimized"),
        close_to_tray: str_of("settings.tray_close_to_tray"),
        save: str_of("settings.save"),
        wsl_settings: str_of("settings.wsl_settings"),
        sidebar_features: str_of("settings.sidebar_features"),
        tab_general: str_of("settings.tab_general"),
        tab_advanced: str_of("settings.tab_advanced"),
        tab_interface: str_of("settings.tab_interface"),
        sparse_vhd: str_of("settings.sparse_vhd"),
        sparse_vhd_desc: str_of("settings.sparse_vhd_desc"),
        mount_auto_probe: str_of("settings.mount_auto_probe"),
        mount_auto_probe_desc: str_of("settings.mount_auto_probe_desc"),
        colorful_icons: str_of("settings.colorful_icons"),
        mail_icon_always: str_of("settings.mail_icon_always"),
        hide_pin_icon: str_of("settings.hide_pin_icon"),
        show_drag_icon: str_of("settings.show_drag_icon"),
        stop_wsl: str_of("settings.stop_wsl"),
    }
}

fn str_of(key: &str) -> slint::SharedString {
    i18n::t(key).into()
}

/// Write the window's settings into the file.
///
/// Every section writes the whole file -- the three are one TOML document, and a
/// partial write would be a second file format -- but each reads and reports on
/// its own fields.
fn save(handle: Weak<AppWindow>, section: Section) {
    let Some(app) = handle.upgrade() else { return };

    let mut config = config::load();
    config.user.modify_time = config::stamp_now();

    match section {
        Section::General => {
            config.user.ui_language = app.get_ui_language().to_string();
            config.user.log_level = config::clamp_log_level(app.get_log_level() as i64);
            config.user.log_days = config::clamp_log_days(app.get_log_days() as i64);
            config.user.check_update =
                config::normalize_interval(&app.get_check_update_interval().to_string());
            config.user.auto_shutdown = app.get_auto_shutdown();
            // `logs_location` is deliberately not written back. The row shows the
            // directory the logger uses, which is `config::logs_dir()`; accepting
            // an edit would let the page claim a directory the logger ignores.
        }
        Section::Advanced => {
            config.user.sparse_vhd = app.get_sparse_vhd();
            config.user.mount_auto_probe = app.get_mount_auto_probe();
        }
        Section::Interface => {
            config.user.system_color = app.get_system_color();
            config.user.colorful_icons = app.get_colorful_icons();
            config.user.mail = app.get_mail_icon_always();
            config.user.hide_pin = app.get_hide_pin_icon();
            config.user.show_drag = app.get_show_drag();
            config.tray.autostart = app.get_tray_autostart();
            config.tray.start_minimized = app.get_tray_start_minimized();
            config.tray.close_to_tray = app.get_tray_close_to_tray();
            config.sidebar.toggle = app.get_sidebar_toggle();
            config.sidebar.add = app.get_sidebar_add();
            config.sidebar.usb = app.get_sidebar_usb();
            config.sidebar.network = app.get_sidebar_network();
            config.sidebar.scheduler = app.get_sidebar_scheduler();
            config.sidebar.mount = app.get_sidebar_mount();
            config.sidebar.donate = app.get_sidebar_donate();
            config.sidebar.about = app.get_sidebar_about();
        }
    }

    match config::save(&config) {
        Ok(()) => {
            info!(?section, "settings saved");
            // The Run entry is a consequence of the file rather than a second
            // copy of it, so it is applied from the value that was just written.
            // Without this the two tray switches on this tab would only take
            // effect at the next launch, which is the shape of a control that
            // does nothing: the reader turns "start with Windows" on, sees no
            // change, and has no way to tell it worked.
            //
            // The tray's two other switches need nothing here: `close_to_tray` is
            // read from the window property at the moment a close is requested,
            // and `start_minimized` is read from the file at the next launch --
            // which is the only moment it could mean anything.
            if matches!(section, Section::General) {
                match crate::app::autostart::set_dashboard_autostart(
                    config.tray.autostart,
                    config.tray.start_minimized,
                ) {
                    Ok(changed) => debug!(changed, "Run entry reconciled with the file"),
                    // `warn` and not a toast: the settings file did save, and the
                    // reader can set the Run entry by hand. The reference logs here
                    // too (`settings.rs:27`).
                    Err(error) => warn!(%error, "the Run entry could not be written"),
                }
            }
            toast(&handle, i18n::t("settings.saved_success"));
        }
        Err(error) => {
            // Toasted rather than only logged: this is the one save failure a
            // reader can do something about -- a read-only home directory, a file
            // open in an editor -- and they are standing right there.
            toast(&handle, i18n::tr("settings.saved_failed", &[error.clone()]));
            warn!(%error, "settings could not be written");
        }
    }
}

/// Open one of the page's folders.
fn open_folder(handle: Weak<AppWindow>, folder: Folder) {
    let path = match folder.path() {
        Some(path) => path,
        None => {
            toast(&handle, i18n::t("main.distro_not_found"));
            return;
        }
    };
    let arg = disk_model::explorer_arg_for_dir(&path);
    debug!(folder = ?folder, arg = %arg, "opening in explorer");
    // A failure here is a missing Explorer, not a wrong path, and the reader can
    // do nothing about it -- the same call the storage page makes, and the same
    // reason it logs rather than toasts.
    if let Err(error) = open::that(&arg) {
        warn!(%error, "could not open explorer");
    }
}

fn toast(handle: &Weak<AppWindow>, text: String) {
    if let Some(app) = handle.upgrade() {
        app.set_task_status_text(text.into());
        app.set_task_status_visible(true);
    }
}
