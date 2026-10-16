// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// Every function here is P7 Task 5's, not dead: the CLI's `#[cfg(test)] #[path]`
// mount runs the tests, which is what this task is for. Same reason and same
// shape as `config/mod.rs`, `i18n/mod.rs` and `scheduler_task/mod.rs`.
#![allow(dead_code)]

//! Running a command with a UAC prompt, showing nothing while it runs.
//!
//! The counterpart of the reference's `src/utils/system.rs:61-169`, which the
//! scheduler page needs for four of its five actions: `schtasks /Create`,
//! `/Delete`, `/Change` and `/Run` all write to the task store, and on a machine
//! where the reader is not an administrator that write needs elevation.
//!
//! # What is here, and what is not
//!
//! The reference's module holds a dozen unrelated helpers (legacy VBS cleanup,
//! sparse files, attaching to a parent console). Only the elevation pair is
//! carried over, because that is the only part the scheduler uses, and a module
//! that exists to hold three functions this program does not call is a module
//! a reader has to check.
//!
//! The Win32 half is a thin shell around `ShellExecuteExW` with the `runas`
//! verb -- the reference's own lines, because that is simply how Windows does
//! it: `CreateProcess` has no "ask for elevation" flag, and the only way is to
//! ask the shell.
//!
//! # What is testable, and is
//!
//! Everything that builds the command line. What leaves this process is two
//! strings and a verb, and getting either of them wrong produces a UAC prompt
//! that says yes to the wrong thing -- so [`build_elevated_shell`],
//! [`escape_tr_args`] (in `schtasks.rs`) and [`elevated_log_name`] are pure and
//! have tests. The `ShellExecuteExW` call itself has no test and needs none:
//! it is six lines of the `windows` crate's API pointed at constants.

use tracing::{debug, error, info};

/// The verb that asks for elevation. Not a magic string: it is the documented
/// `ShellExecuteExW` verb, and it is what a manifest would ask for.
pub const RUNAS_VERB: &str = "runas";

/// `SW_HIDE`. The elevated console must not flash: a window that appears and
/// disappears is exactly what the invisible in the name is for.
pub const HIDE_WINDOW: i32 = 0;

/// `CREATE_NO_WINDOW`, for every `Command` this project spawns that the reader
/// must not see.
///
/// Here rather than in each caller because this is the module that owns the
/// Windows-facing details, and because a console window flashing once per page
/// refresh is the kind of defect that only shows up on the machine it was
/// written on. `startup_info.rs` keeps its own copy: it is in `app/` and this
/// one is in `app/scheduler_task/`, and a constant shared across those two
/// subtrees would have to be cited from one of them in the other's direction.
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

/// The directory an elevated `cmd.exe` starts in.
///
/// The reference's is `C:\Windows\System32` hardcoded. Ours asks for the system
/// directory rather than assuming the drive letter and the folder both hold,
/// which is the same request on every machine that matters.
pub fn working_directory() -> String {
    std::env::var("SystemRoot")
        .map(|root| format!(r"{}\System32", root.trim_end_matches('\\')))
        .unwrap_or_else(|_| r"C:\Windows\System32".to_string())
}

/// Build the `cmd.exe` line the elevated shell runs.
///
/// ```text
/// (schtasks /Delete /TN "\WSA-Dashboard\X" /F & schtasks /Create ...) > "C:\...\wsa_elevated_123.log" 2>&1
/// ```
///
/// Three decisions, each of them a line the reference has already paid for:
///
///   * The commands are joined with `&`, so one UAC prompt covers a create and
///     the delete-then-recreate an edit turns into.
///   * The whole thing is wrapped in parentheses and redirected to a log file,
///     because an elevated process the reader cannot see must leave *some*
///     evidence when it fails. Without this, a refused elevation and a failed
///     `schtasks` look identical from the outside.
///   * **There is no extra outer quoting**, and the reference's comment says
///     why: an additional `" "` pair around a redirect target that is already
///     quoted nests, and `cmd.exe`'s parser then misreads device paths such as
///     `\\.\PHYSICALDRIVE1`.
///
/// Returns `None` for an empty command list rather than the `log` of a shell
/// that runs nothing.
pub fn build_elevated_shell(commands: &[String], log_path: &str) -> Option<String> {
    if commands.is_empty() {
        return None;
    }
    Some(format!(
        "({}) > \"{}\" 2>&1",
        commands.join(" & "),
        log_path
    ))
}

/// The temp file the elevated output lands in.
///
/// Named so that two concurrent requests cannot collide and so that a file left
/// behind by a killed process is recognisable as ours rather than as somebody
/// else's junk.
pub fn elevated_log_name(nanos: u128) -> String {
    format!("wsa_elevated_{}.log", nanos)
}

/// Run several commands behind one UAC prompt, hidden.
///
/// Blocks until the elevated process exits, which is the whole reason this is
/// `fn` and not a spawned task: the caller's next step reads what the command
/// wrote, and reading it while the command is still running reads nothing.
pub fn run_invisible_elevated_commands(commands: Vec<String>) -> Result<(), String> {
    if commands.is_empty() {
        return Ok(());
    }

    let log_path = std::env::temp_dir()
        .join(elevated_log_name(stamp()))
        .to_string_lossy()
        .to_string();
    let Some(shell) = build_elevated_shell(&commands, &log_path) else {
        return Ok(());
    };

    info!(
        "Requesting invisible elevated execution for {} commands via cmd.exe",
        commands.len()
    );
    debug!("Full elevated command: cmd /c {}", shell);

    let result = run_command_with_elevation("cmd.exe", vec!["/c".to_string(), shell]);

    // The log exists even when the elevation succeeded, because `>` creates the
    // file whether or not it is written to. Read it before removing it, and only
    // pay attention when something went wrong -- a successful `schtasks` prints
    // nothing.
    let output = std::fs::read_to_string(&log_path).unwrap_or_default();
    let _ = std::fs::remove_file(&log_path);

    if let Err(ref e) = result {
        if !output.trim().is_empty() {
            error!("Elevated command output:\n{}", output.trim());
        }
        return Err(e.clone());
    }
    Ok(())
}

/// One command, one prompt. The plural form's single-element case, kept because
/// the four `schtasks` shells each call it.
pub fn run_invisible_elevated_command(command: &str) -> Result<(), String> {
    run_invisible_elevated_commands(vec![command.to_string()])
}

fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

/// Ask the shell to run `program` with the `runas` verb, hidden, and wait.
///
/// The reference's implementation, unchanged except for the working directory
/// and the error text. Two details that matter:
///
///   * `SEE_MASK_NOCLOSEPROCESS` is what makes `sei.hProcess` valid, which is
///     the only way to wait. Without it there is no handle to wait on and the
///     caller would return before the command ran.
///   * A non-zero exit code is reported with the code in both decimal and hex,
///     because the two failure modes this has to distinguish (`4294967295`,
///     "the target is not ready or is in use", and an ordinary `1`) are
///     meaningless as a decimal number.
fn run_command_with_elevation(program_name: &str, args: Vec<String>) -> Result<(), String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
    use windows::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    use windows::core::{HSTRING, PCWSTR};

    let args_str = args.join(" ");
    let program = HSTRING::from(program_name);
    let parameters = HSTRING::from(&args_str);
    let verb = HSTRING::from(RUNAS_VERB);
    let directory = HSTRING::from(working_directory());

    debug!("Executing elevated command: {} {}", program_name, args_str);

    let mut sei = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(program.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        lpDirectory: PCWSTR(directory.as_ptr()),
        nShow: SW_HIDE.0 as i32,
        ..Default::default()
    };

    unsafe {
        match ShellExecuteExW(&mut sei) {
            Ok(()) => {
                // Wait for the elevated process to finish. `SW_HIDE` and the
                // handle are the two reasons the caller can trust the result.
                if !sei.hProcess.is_invalid() {
                    WaitForSingleObject(sei.hProcess, INFINITE);

                    let mut exit_code: u32 = 0;
                    if GetExitCodeProcess(sei.hProcess, &mut exit_code).is_ok() {
                        let _ = CloseHandle(sei.hProcess);
                        if exit_code != 0 {
                            let msg = format!("Command exited with code {} (0x{:X})", exit_code, exit_code);
                            error!("Elevated command failed: {}", msg);
                            return Err(msg);
                        }
                    }
                }
                Ok(())
            }
            Err(e) => {
                let msg = format!("UAC elevation failed or was denied: {}", e);
                error!("{}", msg);
                Err(msg)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_commands_means_no_shell_at_all() {
        // The empty case returning `Ok(())` in `run_invisible_elevated_commands`
        // would be a UAC prompt that approved nothing. `None` here is what makes
        // that impossible to write by accident.
        assert_eq!(build_elevated_shell(&[], r"C:\log"), None);
    }

    #[test]
    fn one_command_is_parenthesised_and_redirected() {
        let shell = build_elevated_shell(
            &[r#"schtasks /Delete /TN "\WSA-Dashboard\X" /F"#.to_string()],
            r"C:\Temp\wsa_elevated_1.log",
        )
        .expect("one command still builds a shell");

        assert_eq!(
            shell,
            r#"(schtasks /Delete /TN "\WSA-Dashboard\X" /F) > "C:\Temp\wsa_elevated_1.log" 2>&1"#
        );
        // Both halves of the redirect, and only one of each: `2>&1` after `>` is
        // the shape the reference measured as working with device paths.
        assert_eq!(shell.matches(" > \"").count(), 1);
        assert!(shell.ends_with(" 2>&1"), "stderr must land in the same file");
        // No outer quoting. The reference's comment is explicit that adding a
        // pair breaks `\\.\PHYSICALDRIVE1`; the parentheses are the grouping.
        assert!(
            !shell.starts_with("\"") && !shell.ends_with("\""),
            "an outer quote pair nests with the already-quoted log path"
        );
    }

    #[test]
    fn several_commands_share_one_prompt() {
        // This is the edit path: delete the old task, then create the new one,
        // behind a single UAC consent the reader only has to answer once.
        let shell = build_elevated_shell(
            &[
                r#"schtasks /Delete /TN "\WSA-Dashboard\X" /F"#.to_string(),
                r#"schtasks /Create /TN "\WSA-Dashboard\X" /TR "cmd.exe""#.to_string(),
                r#"powershell -NoProfile -Command "Set-ScheduledTask""#.to_string(),
            ],
            "log.txt",
        )
        .expect("three commands build a shell");

        assert_eq!(shell.matches(" & ").count(), 2);
        assert!(shell.starts_with('('), "all three are one parenthesised group");
        assert!(shell.contains(" /Delete "));
        assert!(shell.contains(" /Create "));
        assert!(shell.contains("powershell"));
        // One prompt for three commands: the parentheses are what group them
        // into a single `cmd.exe /c`.
        assert_eq!(shell.matches(") >").count(), 1);
    }

    #[test]
    fn the_log_name_carries_a_nanos_stamp_so_two_runs_cannot_share_one() {
        let first = elevated_log_name(1);
        let second = elevated_log_name(2);
        assert_eq!(first, "wsa_elevated_1.log");
        assert_eq!(second, "wsa_elevated_2.log");
        assert_ne!(first, second);
        // The name is what a reader of `%TEMP%` would use to recognise a log
        // this program left behind, so it says whose it is.
        assert!(first.starts_with("wsa_"), "the reference's name said wsldashboard");
    }

    #[test]
    fn the_working_directory_is_a_system_directory_and_not_a_hardcoded_drive() {
        let dir = working_directory();
        assert!(
            dir.ends_with("System32"),
            "cmd.exe has to start in System32 for schtasks to resolve: {dir}"
        );
        assert!(
            !dir.ends_with('\\'),
            "a trailing backslash makes System32\\System32 when something joins a name on"
        );
        assert!(!dir.contains("\""));
    }

    /// The reference's constant, asserted so that a change to either is a change
    /// a reader has to notice.
    #[test]
    fn the_verb_and_the_window_style_are_the_documented_pair() {
        assert_eq!(RUNAS_VERB, "runas");
        assert_eq!(HIDE_WINDOW, 0);
    }
}
