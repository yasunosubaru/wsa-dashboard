// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

#![allow(dead_code)]

// Three of this module's functions are called from `ui::handlers::settings`,
// and none of the rest are called from anywhere yet. The CLI mounts this module
// with `#[cfg(test)] #[path]` so the tests at the bottom can run at all, and from
// *there* those functions have no caller either -- which is a property of the
// mount rather than of the code. The same reason and the same shape as
// `i18n/mod.rs`, which does this for the same test runner.

//! Where the settings live, and how they get there and back.
//!
//! The counterpart of the reference's `src/config/mod.rs` (646 lines). Most of
//! that file is WSL: reading `~/.wslconfig`, enumerating registered instances,
//! migrating `distros.toml`. None of it exists here, because WSA has no
//! instances and no `.wslconfig`. What remains is the part that is about the
//! *program*: three TOML files under one directory, a read that never fails, and
//! a write that says so when it cannot.
//!
//! # Why the read never fails
//!
//! A settings file that cannot be parsed is a file a person edited, a file a
//! newer build wrote with keys this build does not know, or a file that was cut
//! short when the machine lost power. In all three cases the right answer is the
//! same: open with the defaults and say so in the log. Refusing to start would
//! turn a cosmetic problem into a broken program, and the reference's own
//! `load` already returns defaults rather than an error.
//!
//! # Why the paths are computed here
//!
//! `logs_dir` lives in this module rather than in `app::launcher` because this
//! module has to be mountable into the CLI, where `app::` does not exist -- and
//! the other way round the settings page would name one directory while the
//! logger wrote to another. `launcher::logs_dir()` calls this one.

use std::path::{Path, PathBuf};

use tracing::{warn, info};

pub mod migration;
pub mod models;

// Re-exported so a caller says `config::Settings` rather than reaching two
// modules down. The `allow` is for the CLI's `#[path]` mount, where `mod
// config` is private and therefore nothing outside it can reach the re-exports
// -- so the import is unused there by construction, not by accident.
#[allow(unused_imports)]
pub use migration::SETTINGS_VERSION;
#[allow(unused_imports)]
pub use models::{ApplicationConfig, Config, SidebarConfig, TraySettings, UserSettings};

/// `~/.wsadashboard` -- the one directory holding the logs and the settings.
pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".wsadashboard")
}

/// `~/.wsadashboard/logs`.
///
/// The single source of truth for this path: [`app::launcher::logs_dir`] calls
/// it, and the settings default is this path as a string.
pub fn logs_dir() -> PathBuf {
    config_dir().join("logs")
}

/// `~/.wsadashboard/settings.toml`.
///
/// The file name is the reference's. Nothing about this program's settings
/// needs a different name, and keeping it means a user who has read the
/// reference's documentation has read ours.
pub fn config_path() -> PathBuf {
    config_dir().join("settings.toml")
}

/// Read the settings, or the defaults.
pub fn load() -> Config {
    load_from(&config_path())
}

/// Read from an explicit path.
///
/// Takes the path rather than reaching for [`config_path`] itself so that the
/// interesting cases -- a file that does not exist, a file that is not valid
/// TOML, a file from a newer build -- can be tested without touching the real
/// one.
///
/// Every one of those cases ends the same way: a `Config` with defaults for
/// whatever was missing, brought forward by [`migration::migrate_config`]. The
/// migration runs on **every** read, including the ones that read nothing --
/// which is what makes "the file on disk is a version behind" and "the file on
/// disk is not there yet" the same amount of work for every caller.
pub fn load_from(path: &Path) -> Config {
    let mut config = read_config(path);
    migration::migrate_config(&mut config);
    config
}

/// The read itself, with no migration. Split out so that [`load_from`] has one
/// exit rather than four.
fn read_config(path: &Path) -> Config {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Not a warning: a first run has no settings, and that is the
            // expected state rather than something that went wrong.
            return Config::default();
        }
        Err(error) => {
            warn!(path = %path.display(), %error, "could not read the settings");
            return Config::default();
        }
    };
    match toml::from_str::<Config>(&raw) {
        Ok(config) => config,
        Err(error) => {
            // A TOML syntax error usually names the line, and that is worth more
            // than a generic "bad settings file": the reader can go and look.
            warn!(path = %path.display(), %error, "settings did not parse; using defaults");
            Config::default()
        }
    }
}

/// Write the settings, creating the directory if it is not there yet.
pub fn save(config: &Config) -> Result<(), String> {
    save_to(&config_path(), config)
}

/// Write to an explicit path.
pub fn save_to(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    let body = toml::to_string_pretty(config)
        .map_err(|error| format!("could not format the settings: {error}"))?;
    std::fs::write(path, body)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    info!(path = %path.display(), "settings written");
    Ok(())
}

/// Change one field of the settings and write the file back.
///
/// The read-modify-write the settings page does inline (`settings.rs:368`) for a
/// caller that has exactly one thing to say. The reason it reads first is the
/// reason it is a function here rather than a `config.user.sidebar_collapsed =
/// x; save(&config)`: the side bar's handler has no `Config` of its own and must
/// not invent one, or the next Save on the settings page would write back every
/// other value as it was when *this* handler happened to run.
///
/// `modify-time` is stamped for the same reason the page stamps it: a reader who
/// opens the file should be able to see when it last said something, and a write
/// that leaves the stamp alone makes the file lie about its own age.
///
/// The whole struct is written, including the schema version `load` just
/// migrated -- which is how a file that was a version behind stops being one.
pub fn update_user_settings(change: impl FnOnce(&mut UserSettings)) -> Result<(), String> {
    let mut config = load();
    change(&mut config.user);
    config.user.modify_time = stamp_now();
    save(&config)
}

/// Days of logs to keep, as the reference's picker offers them, plus anything
/// in between.
///
/// Out of range is clamped rather than refused: this is a retention count, and
/// a reader who typed 400 does not want the settings page to fail -- they want
/// 255, which is what a `u8` can hold and the longest a rotation can express.
pub fn clamp_log_days(days: i64) -> u8 {
    days.clamp(1, u8::MAX as i64) as u8
}

/// `error` / `warn` / `info` / `debug` / `trace` to 1..=5.
///
/// `None` for anything else, rather than a silent fallback: this is a value the
/// reader typed, and quietly turning `verbose` into `info` is how a setting
/// stops meaning what it says.
pub fn parse_log_level(text: &str) -> Option<u8> {
    match text.trim().to_ascii_lowercase().as_str() {
        "error" => Some(1),
        "warn" | "warning" => Some(2),
        "info" => Some(3),
        "debug" => Some(4),
        "trace" => Some(5),
        _ => None,
    }
}

/// What level number means, for the settings page's current selection.
pub fn log_level_name(level: u8) -> &'static str {
    match level {
        1 => "error",
        2 => "warn",
        3 => "info",
        5 => "trace",
        // 4 and anything above it: `debug` is the reference's default, and an
        // out-of-range number from a hand-edited file should not render as a
        // blank in the picker.
        _ => "debug",
    }
}

/// Log verbosity into the 1..=5 the reference uses.
///
/// The `settings/mod.slint` index arithmetic assumes it: the picker writes
/// `index + 1`, so a 0 would turn the "Error" row into a selection nothing
/// matches and a 6 would name a level no row holds. Clamped rather than
/// refused, for the same reason [`clamp_log_days`] clamps: this is a number in a
/// file, not an invitation to argue about it.
pub fn clamp_log_level(level: i64) -> u8 {
    level.clamp(1, 5) as u8
}

/// Days between update checks, accepting the reference's four choices and any
/// positive day count.
///
/// Zero and negatives become a week rather than becoming "check every time the
/// program starts", which is what a 0 interval would otherwise mean to a
/// reader who left the field empty.
pub fn normalize_interval(text: &str) -> u8 {
    match text.trim().parse::<i64>() {
        Ok(days) if days > 0 => clamp_log_days(days),
        _ => models::default_check_update(),
    }
}

/// The stamp written into `modify-time`, so a reader who opens the file can see
/// when it last said something.
pub fn stamp_now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What [`load_from`] answers for a file that says nothing: the defaults,
    /// with the schema stamp the migration puts on top of them.
    ///
    /// Spelled as a function rather than as `Config::default()` because the two
    /// are no longer the same, and the difference is the point: a read brings a
    /// file -- or the absence of one -- up to this build's schema, and only the
    /// version field moves. A test that wrote `Config::default()` here would be
    /// asserting that loading does *not* migrate, which is the opposite of what
    /// this module now promises.
    fn migrated_default() -> Config {
        let mut config = Config::default();
        crate::config::migration::migrate_config(&mut config);
        config
    }

    #[test]
    fn a_missing_file_is_the_default_settings() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("nothing.toml");
        assert_eq!(load_from(&path), migrated_default());
    }

    /// The important one. A settings file that does not parse must not stop the
    /// program, and must not leave the page showing half of what was read
    /// alongside half of what was guessed.
    #[test]
    fn a_file_that_does_not_parse_falls_back_to_defaults() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.toml");
        std::fs::write(&path, "this is not = = toml [[[").expect("write");
        assert_eq!(load_from(&path), migrated_default());
    }

    /// And the stamp is the only thing a read adds: a file whose fields are all
    /// defaults comes back equal to the defaults except for the version.
    #[test]
    fn reading_a_file_that_says_nothing_changes_only_the_schema_stamp() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.toml");
        std::fs::write(&path, "[user]\n").expect("write");

        let mut expected = Config::default();
        expected.application.setting_version = crate::config::SETTINGS_VERSION;
        assert_eq!(load_from(&path), expected);
    }

    /// A file from a newer build, or one a person edited halfway. Everything it
    /// says is kept; everything it does not say is the default.
    #[test]
    fn a_partial_file_keeps_what_it_says_and_defaults_the_rest() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.toml");
        std::fs::write(&path, "[user]\nlog-days = 30\n").expect("write");

        let config = load_from(&path);
        assert_eq!(config.user.log_days, 30, "what the file said");
        assert_eq!(config.user.log_level, 4, "what it did not");
        assert!(config.sidebar.network, "a section it never mentioned");
    }

    /// A key this build does not know is not an error. That is the whole reason
    /// a downgrade should still start.
    #[test]
    fn a_key_we_do_not_know_is_ignored() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.toml");
        std::fs::write(
            &path,
            "[user]\nlog-days = 30\nsomething-from-the-future = 7\n",
        )
        .expect("write");
        assert_eq!(load_from(&path).user.log_days, 30);
    }

    #[test]
    fn save_then_load_gives_back_the_same_settings() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.toml");

        let mut config = Config::default();
        config.user.ui_language = "zh-CN".into();
        config.user.dark_mode = true;
        config.user.log_days = 30;
        config.sidebar.scheduler = false;
        config.tray.close_to_tray = false;

        save_to(&path, &config).expect("save");
        // The stamp is the one difference a round trip is allowed to add: a
        // program that saves what it read writes the migrated version, not the
        // one it found.
        config.application.setting_version = crate::config::SETTINGS_VERSION;
        assert_eq!(load_from(&path), config);
    }

    /// The one live test in this module: **the settings file that is actually on
    /// this machine**.
    ///
    /// Every other test here writes its own file, which is the right way to pin
    /// a parser and the wrong way to answer "will the file a reader already has
    /// still load". This one reads the real `~/.wsadashboard/settings.toml`,
    /// whatever it happens to contain, and asserts the two things the migration
    /// promises: the read succeeds, and the schema ends up at this build's --
    /// which for every file that exists today means the chain fired, because
    /// none of them has a `setting-version` in it.
    ///
    /// **Reads only.** A migration that wrote would make this test a test that
    /// edits the reader's settings, which is not a thing a test may do.
    ///
    /// `#[ignore]` because Q8 keeps machine-dependent work out of the suite, and
    /// because on a machine that has never run this program there is no file
    /// here to be interesting about.
    #[test]
    #[ignore = "reads the real settings.toml; Q8 keeps machine-dependent work out of the suite"]
    fn the_real_settings_file_loads_and_migrates() {
        let path = config_path();
        let raw = std::fs::read_to_string(&path).unwrap_or_default();
        println!("{} ({} bytes)", path.display(), raw.len());
        println!("mentions setting-version: {}", raw.contains("setting-version"));

        let config = load_from(&path);
        println!(
            "schema v{} -> v{}, sparse-vhd={}, mount-auto-probe={}, ui-language={}",
            if raw.contains("setting-version") { "?" } else { "0" },
            config.application.setting_version,
            config.user.sparse_vhd,
            config.user.mount_auto_probe,
            config.user.ui_language,
        );

        assert_eq!(
            config.application.setting_version,
            SETTINGS_VERSION,
            "the file on this machine did not come forward to this build's schema"
        );
        // A file that exists must keep the language it says, whatever the
        // migration did with the two keys whose meaning changed.
        assert!(
            !config.user.ui_language.is_empty(),
            "the reader's language did not survive the migration"
        );
    }

    /// The directory does not exist on a first run, and refusing to write is
    /// how a setting silently fails to save the first time the reader uses it.
    #[test]
    fn save_creates_the_directory_it_needs() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("a").join("b").join("settings.toml");
        save_to(&path, &Config::default()).expect("save should create its parents");
        assert!(path.exists());
    }

    /// A write that cannot happen is reported, not swallowed: the settings page
    /// has to be able to say "that did not save".
    #[test]
    fn a_write_that_cannot_happen_is_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        // A file where a directory has to be: `create_dir_all` cannot win.
        let blocker = dir.path().join("blocked");
        std::fs::write(&blocker, "in the way").expect("write");
        let path = blocker.join("settings.toml");

        let error = save_to(&path, &Config::default()).expect_err("should not succeed");
        assert!(!error.is_empty(), "the reason has to reach the reader");
    }

    // -- the two validators the settings page uses --------------------------

    #[test]
    fn log_days_are_clamped_rather_than_refused() {
        assert_eq!(clamp_log_days(0), 1, "zero days would keep nothing");
        assert_eq!(clamp_log_days(-5), 1);
        assert_eq!(clamp_log_days(7), 7);
        assert_eq!(clamp_log_days(30), 30);
        assert_eq!(clamp_log_days(4000), 255, "a u8 is the ceiling, not an error");
    }

    /// The picker writes `index + 1`, so 0 and 6 are not "levels", they are
    /// indices that land outside the table. Clamping is what keeps a hand-edited
    /// file from rendering a blank where the selection should be.
    #[test]
    fn log_levels_are_clamped_into_the_pickers_range() {
        assert_eq!(clamp_log_level(0), 1);
        assert_eq!(clamp_log_level(-3), 1);
        assert_eq!(clamp_log_level(1), 1);
        assert_eq!(clamp_log_level(5), 5);
        assert_eq!(clamp_log_level(6), 5);
        assert_eq!(clamp_log_level(999), 5);
    }

    #[test]
    fn log_levels_round_trip_through_their_names() {
        for (name, level) in [
            ("error", 1),
            ("warn", 2),
            ("info", 3),
            ("debug", 4),
            ("trace", 5),
        ] {
            assert_eq!(parse_log_level(name), Some(level), "{name}");
            assert_eq!(log_level_name(level), name, "{level}");
        }
        assert_eq!(parse_log_level("  DEBUG "), Some(4), "case and space");
        assert_eq!(parse_log_level("warning"), Some(2), "the longer spelling");
        assert_eq!(parse_log_level("verbose"), None, "not a level we have");
        assert_eq!(parse_log_level(""), None);
    }

    /// A number the file cannot hold still has to render as something the
    /// picker can show, or the page shows a blank where the selection should be.
    #[test]
    fn an_out_of_range_level_still_names_something() {
        assert_eq!(log_level_name(0), "debug");
        assert_eq!(log_level_name(99), "debug");
    }

    #[test]
    fn an_empty_interval_becomes_the_default_not_a_check_per_launch() {
        assert_eq!(normalize_interval(""), models::default_check_update());
        assert_eq!(normalize_interval("0"), models::default_check_update());
        assert_eq!(normalize_interval("-1"), models::default_check_update());
        assert_eq!(normalize_interval("nonsense"), models::default_check_update());
        assert_eq!(normalize_interval(" 14 "), 14);
    }

    /// The paths are what the spec and the docs say they are. A reader who is
    /// told "it is in `~/.wsadashboard`" and finds it elsewhere has been lied to.
    #[test]
    fn the_paths_are_where_the_documentation_says() {
        let dir = config_dir();
        assert!(dir.ends_with(".wsadashboard"), "{}", dir.display());
        assert_eq!(config_path(), dir.join("settings.toml"));
        assert_eq!(logs_dir(), dir.join("logs"));
        assert_eq!(
            UserSettings::default().logs_location,
            logs_dir().to_string_lossy(),
            "the settings default and the logger must name one directory"
        );
    }
}