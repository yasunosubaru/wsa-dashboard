// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The scheduler page's view model: the two decisions that are worth a test, in
//! a file that a test can reach.
//!
//! The counterpart of the reference's `ui/handlers/scheduler.rs:13-21` and
//! `:341-377`. Both of those live in the handler because the handler is where
//! they are called; they are here because `ui/handlers/` is GUI-only and
//! `wsadash.rs` mounts this file with `#[cfg(test)] #[path = ...]`, the same
//! move `disk_model` and `network_model` made. A `fn` that names `AppWindow`
//! cannot be tested without one.
//!
//! The two:
//!
//! * [`short_filename`], which is the card's second line.
//! * [`SchedulerField`] and [`validate_new_task`], which say *which* of the
//!   dialog's three error lines a failure belongs to. The reference decides that
//!   by calling one of three near-identical setters from inside a chain of
//!   `return`s, and there is no way to ask it a question.
//!
//! The dialog's third check -- whether the schedule is a usable cron expression
//! -- is not here. It belongs to the dialect that defines it, and
//! `app::scheduler_task::cron::error_key` is where the messages and their
//! sentences are kept together.
//!
//! Nothing in this file names another module of this program, absolute path or
//! relative. It cannot: the CLI mounts it at the crate root and `ui/mod.rs`
//! mounts it one level down, so a path that resolves in one is off by a level in
//! the other. `crate::` would not save it either -- `scheduler_task` is
//! `crate::app::scheduler_task` in one binary and `crate::scheduler_task` in the
//! other -- which is why the cron half of the reference's validation lives in
//! `cron.rs` instead of beside the code that calls it.

/// A shortened command for the card's second line.
///
/// The reference's `short_filename` (`:13-21`): the last path component, cut to
/// 32 characters with `...` when it is longer.
///
/// Ours differs in exactly one place, and it is a panic rather than a
/// preference. The reference writes `&name[..29]`, which is a *byte* slice: a
/// command whose last component is 29 bytes in but not on a character boundary
/// -- any non-ASCII name, and this program ships fifty languages -- panics
/// inside `String`'s index. Taking characters instead gives the identical answer
/// for every ASCII name, which is every name the reference was written against,
/// and a name instead of a crash for the rest. The 32 and the three dots are the
/// reference's.
///
/// The length test is in characters too, for the same reason and one more: it
/// decides whether anything is cut, so a byte-count test over a name of 20
/// three-byte characters would append `...` to a name that was never shortened.
pub fn short_filename(command: &str) -> String {
    let trimmed = command.trim();
    let name = trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed);
    if name.chars().count() > 32 {
        let head: String = name.chars().take(29).collect();
        format!("{head}...")
    } else {
        name.to_string()
    }
}

/// Which of the dialog's three error lines a validation failure belongs to.
///
/// The reference expresses this by calling `set_name_error`, `set_command_error`
/// or `set_cron_error` and then `return`ing, which is why its `on_save` is 66
/// lines of near-identical five-line blocks. Naming the field makes the chain
/// data instead of control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerField {
    Name,
    Command,
    Cron,
}

/// The first thing wrong with what the reader typed, if anything.
///
/// Returns the field whose error line should show and the i18n key of the
/// sentence to put there. `None` means the dialog's name and command are
/// acceptable; the schedule is then the caller's to check, since that is
/// [`error_key`]'s half and it needs the cron dialect rather than a string.
///
/// Order and rules are the reference's (`:343-377`), including the quirk that
/// the 60-character bound is checked before the character set: a 61-byte name
/// made of letters outside `A-Z` gets the "invalid" sentence either way, so the
/// order is unobservable and is kept as found.
///
/// [`error_key`]: crate::app::scheduler_task::cron::error_key
pub fn validate_new_task(
    name: &str,
    command: &str,
    cron_expr: &str,
) -> Option<(SchedulerField, &'static str)> {
    if name.is_empty() {
        return Some((SchedulerField::Name, "scheduler.error_name_required"));
    }
    if name.len() > 60
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Some((SchedulerField::Name, "scheduler.error_name_invalid"));
    }
    if command.is_empty() {
        return Some((SchedulerField::Command, "scheduler.error_command_required"));
    }
    if cron_expr.is_empty() {
        return Some((SchedulerField::Cron, "scheduler.cron_error_required"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- short_filename ------------------------------------------------------

    /// The two examples the reference wrote in its comment above the function
    /// (`:11-12`), kept because they are the shapes the page actually sees: a
    /// batch file somebody scheduled, and `cmd.exe` behind a full path.
    #[test]
    fn the_last_component_is_what_the_card_shows() {
        assert_eq!(
            short_filename(r"D:\program\Docker\get-ip.bat"),
            "get-ip.bat"
        );
        assert_eq!(
            short_filename(r"C:\Windows\System32\cmd.exe"),
            "cmd.exe"
        );
    }

    #[test]
    fn either_separator_works_and_surrounding_space_is_not_part_of_the_name() {
        assert_eq!(short_filename("/usr/bin/python3"), "python3");
        // Windows accepts `/` and so does this: a command copied out of a
        // browser address bar arrives with forward slashes.
        assert_eq!(short_filename("C:/tools/adb.exe"), "adb.exe");
        assert_eq!(short_filename("  adb.exe  "), "adb.exe");
    }

    #[test]
    fn a_command_with_no_separator_is_already_its_own_name() {
        assert_eq!(short_filename("adb"), "adb");
        assert_eq!(short_filename(""), "");
    }

    /// 32 is the whole name and 33 is not: the reference cuts at 29 and appends
    /// three dots, so the two lengths meet exactly at 32.
    #[test]
    fn the_cut_lands_where_the_reference_puts_it() {
        let exactly = "a".repeat(32);
        assert_eq!(short_filename(&exactly), exactly);

        let one_more = "a".repeat(33);
        assert_eq!(short_filename(&one_more), format!("{}...", "a".repeat(29)));
    }

    /// The reference panics here. `&name[..29]` splits a UTF-8 string at byte
    /// 29, and byte 29 of this name is inside its second character.
    #[test]
    fn a_name_that_is_not_ascii_is_cut_between_characters_not_inside_one() {
        let name = format!("{}.ps1", "备".repeat(40));
        let short = short_filename(&name);
        assert_eq!(short, format!("{}...", "备".repeat(29)));
        assert_eq!(
            short.chars().count(),
            32,
            "29 characters and three dots, not 29 bytes and three dots"
        );
    }

    /// And the other half of counting characters: a short name of wide
    /// characters is 60 bytes and 20 characters, and it is not shortened --
    /// appending `...` to a name that was never cut is a lie about the command.
    #[test]
    fn a_short_name_of_wide_characters_is_not_dotted() {
        let name = "备".repeat(20);
        assert!(name.len() > 32, "this test is pointless if it is not narrow");
        assert_eq!(short_filename(&name), name);
    }

    // -- validate_new_task ---------------------------------------------------

    #[test]
    fn a_filled_in_dialog_has_nothing_wrong_with_it() {
        assert_eq!(validate_new_task("nightly", "adb.exe", "0 3 * * *"), None);
    }

    #[test]
    fn an_empty_field_says_which_one_it_is() {
        assert_eq!(
            validate_new_task("", "adb.exe", "0 3 * * *"),
            Some((SchedulerField::Name, "scheduler.error_name_required"))
        );
        assert_eq!(
            validate_new_task("nightly", "", "0 3 * * *"),
            Some((SchedulerField::Command, "scheduler.error_command_required"))
        );
        assert_eq!(
            validate_new_task("nightly", "adb.exe", ""),
            Some((SchedulerField::Cron, "scheduler.cron_error_required"))
        );
    }

    /// Only the first failure is reported. A dialog with three empty fields is
    /// one question, not three, and the reference's chain of `return`s says the
    /// same thing -- the name is the one the reader fills in first.
    #[test]
    fn only_the_first_failure_is_reported() {
        assert_eq!(
            validate_new_task("", "", ""),
            Some((SchedulerField::Name, "scheduler.error_name_required"))
        );
    }

    /// `schtasks /TN` takes the name into a path, so the character set is not a
    /// style rule: a backslash in a name is a folder, and a colon is a drive.
    #[test]
    fn a_name_the_task_store_could_not_hold_is_refused() {
        for bad in [
            "nightly task", // a space
            r"WSA-Dashboard\sub", // a folder separator
            "a:b",
            "夜里", // not ASCII
            "nightly!",
        ] {
            assert_eq!(
                validate_new_task(bad, "adb.exe", "0 3 * * *"),
                Some((SchedulerField::Name, "scheduler.error_name_invalid")),
                "{bad:?} should not be an acceptable task name"
            );
        }
    }

    #[test]
    fn the_dots_and_dashes_and_underscores_of_a_real_name_survive() {
        assert_eq!(
            validate_new_task("WSA-Dashboard_nightly.v2", "adb.exe", "* * * * *"),
            None
        );
    }

    #[test]
    fn sixty_characters_is_a_name_and_sixty_one_is_not() {
        let sixty = "a".repeat(60);
        assert_eq!(validate_new_task(&sixty, "adb.exe", "* * * * *"), None);
        let sixty_one = "a".repeat(61);
        assert_eq!(
            validate_new_task(&sixty_one, "adb.exe", "* * * * *"),
            Some((SchedulerField::Name, "scheduler.error_name_invalid"))
        );
    }
}
