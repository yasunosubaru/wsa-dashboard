// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// Every function here is P7 Task 5's, not dead: the CLI's `#[cfg(test)] #[path]`
// mount runs the tests, which is what this task is for. Same reason and same
// shape as `config/mod.rs`, `i18n/mod.rs` and `scheduler_task/mod.rs`.
#![allow(dead_code)]

//! Creating, deleting, toggling and running scheduled tasks.
//!
//! The counterpart of the reference's `schtasks.rs:189-611`. The 400 lines
//! *above* that -- the CSV query -- are `xml.rs`, and the cron conversion is
//! `cron.rs`; see `mod.rs` for the split.
//!
//! # What the reference does, and what survives
//!
//! The reference reads tasks by CSV column number and writes them back with
//! `schtasks /Create`. The read half is gone (see `xml.rs`). The write half is
//! carried over almost whole, because Windows' task store is the same store:
//! `/Create`, `/Delete`, `/Change /Enable`, `/Run` and the elevated path through
//! a single `cmd.exe` batch.
//!
//! Two adaptations, both forced by what the task *is*:
//!
//!   * **The task name is `\WSA-Dashboard\<id>`, not `\WSLDashboard\UserTasks\<id>`.**
//!     [`super::TASK_FOLDER`] has no second segment because this program does
//!     not split one cron into several native tasks, so there is no `UserTasks`
//!     group to put them in. [`super::task_path_for_id`] is what builds the name.
//!   * **The cron is still carried in the task's `<Description>` field** as
//!     `cron:<expr>`, exactly as the reference's. That is a hack and the honest
//!     one: `schtasks` will not store the expression it was given, only the
//!     native trigger it built from it, so the only way the dialog can show the
//!     reader what they typed is to keep a copy somewhere the task's own XML
//!     round-trips. [`cron_from_description`] reads it back.
//!
//! One function of the reference's is deliberately *not* carried over: it asks
//! `schtasks /Query /TN "\WSLDashboard\UserTasks" /FO CSV /V` first and falls
//! back to a full query when the folder does not exist. Measured here, the XML
//! form of that folder query fails with exit code 1 and `ERROR: The system
//! cannot find the file specified.` on a machine where the folder has never been
//! created -- which is every machine before its first task. So the "fast path"
//! costs a second full query exactly when the page is emptiest, and buys speed
//! only when it already has tasks. [`list_tasks`] queries once and filters by
//! [`super::TASK_FOLDER`], which is a path *we* control.
//!
//! # What is testable, and is
//!
//! Everything that builds a command line, decodes Windows' output, or turns a
//! [`TaskXml`] into a row. The five functions that spawn `schtasks` have no unit
//! test and are the only five that touch the machine -- see `xml.rs`'s
//! `against_the_real_schtasks_on_this_machine` and MAPPING 16.9 for how the
//! read path was exercised, and `the_real_query_output_is_what_the_decoder_was_
//! built_for` below for the measurement that `decode_output` rests on.

use std::os::windows::process::CommandExt;
use std::process::Command;
use tracing::{debug, info, warn};

use super::cron::parse_cron;
use super::elevate::{
    CREATE_NO_WINDOW, run_invisible_elevated_command, run_invisible_elevated_commands,
};
use super::models::{CreateTaskParams, NOT_REPORTED, SchedulerTask, SchtasksParams};
use super::xml::{
    TASK_FOLDER, TaskXml, our_tasks, parse_tasks_xml, runs_as_admin, task_id_from_path,
    task_path_for_id,
};

/// Where the cron expression lives inside a task's `<Description>`.
pub const CRON_PREFIX: &str = "cron:";

/// `schtasks /Query /XML` output bytes, as text.
///
/// # What this is for, and what it is not
///
/// Measured on this machine, with `cmd /c`'s redirection so that nothing between
/// `schtasks` and the file transcodes: **about 362 000 bytes, no BOM, no NUL byte
/// anywhere, no leading `<?xml ?>` declaration, and the first sixteen bytes are
/// `0D 0A 3C 54 61 73 6B 73 3E 0D 0A 0D 0A 0D 0A 3C`** -- that is `\r\n<Tasks>`
/// and a newline. On this machine the output is single-byte, which is why
/// `xml.rs`'s live test gets away with `from_utf8_lossy`.
///
/// The byte count is given as a magnitude and not as a constant: two reads a
/// minute apart gave 362 090 (through the pipe this test uses) and 362 097
/// (through `cmd /c`'s redirect), and the count moves whenever a task is added or
/// removed. The first sixteen bytes were identical in both. A number that
/// changes is not something the decoder can be asserted against; the *shape* is.
///
/// So the UTF-16 branches below are a **guard, not a path this machine takes**.
/// They are here because `schtasks` is documented to write UTF-16LE when its
/// stdout is not a console on some configurations, and because the reference's
/// `from_utf8_lossy` is what would silently turn that into a document full of
/// NULs and 0 tasks. A wrong answer that says "no tasks" is the failure this
/// whole directory exists to stop repeating; the cost of the guard is one
/// `strip_prefix` on the common path.
///
/// An earlier version of this comment stated the opposite -- that this machine's
/// output starts with `3C 00 3F 00 78 00 6D 00 6C 00`. That was written from
/// expectation rather than from measurement, and the live test below is what
/// caught it.
pub fn decode_output(bytes: &[u8]) -> String {
    const MAX_PROBE: usize = 512;

    // A BOM is authoritative and costs nothing.
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(rest, false);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).to_string();
    }

    // No BOM. Count the NULs on each side of the byte pairs in a probe window,
    // which separates UTF-16 from a single-byte encoding and also says which
    // variant of UTF-16. A UTF-16 document has a NUL in roughly every other
    // byte; a single-byte one has none at all.
    let probe = &bytes[..bytes.len().min(MAX_PROBE)];
    let even_nuls = probe.iter().step_by(2).filter(|b| **b == 0).count();
    let odd_nulls = probe.iter().skip(1).step_by(2).filter(|b| **b == 0).count();

    if odd_nulls > probe.len() / 64 {
        return decode_utf16(bytes, true);
    }
    if even_nuls > probe.len() / 64 {
        return decode_utf16(bytes, false);
    }

    String::from_utf8_lossy(bytes).to_string()
}

/// Decode UTF-16 bytes into a `String`.
///
/// `> probe.len() / 64` in [`decode_output`] is deliberately a low bar: a
/// document with one NUL in 512 bytes is not UTF-16, and a UTF-16 document has
/// hundreds.
fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| {
            if little_endian {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Hex MD5. The only thing it hashes is a task id, and the only reason it
/// exists is that the launcher file's name has to be stable across an edit and
/// has to be a legal Windows file name -- a task id can contain characters a
/// path cannot. Asserted against known digests rather than against itself.
pub fn md5_hex(bytes: &[u8]) -> String {
    format!("{:x}", md5::compute(bytes))
}

// -- the cron, kept in the task's own Description ------------------------------

/// The cron expression out of a `<Description>` written by [`description_for_cron`].
///
/// `""` when the description is anything else, which includes the description of
/// a task somebody else created -- the reader will see an empty expression
/// rather than a prefix match of whatever prose happened to contain `cron:`.
pub fn cron_from_description(description: &str) -> String {
    description
        .trim()
        .strip_prefix(CRON_PREFIX)
        .map(|cron| cron.trim().to_string())
        .unwrap_or_default()
}

/// What `<Description>` should say for this cron expression.
pub fn description_for_cron(cron_expr: &str) -> String {
    format!("{CRON_PREFIX}{}", cron_expr.trim())
}

// -- the command line, and the launcher that hides its window -------------------

/// This program's own executable, for the launcher's cold-start line.
///
/// `current_exe` rather than a constant: the launcher has to run *this* build,
/// and a path assembled from `APP_ID` would be a guess about where the reader
/// put it. The one failure mode it has -- a path that moved after the task was
/// created -- is a task that starts the subsystem it cannot find; the command
/// still runs, which is the same best-effort rule the launcher applies
/// everywhere else.
pub fn our_exe_path() -> Result<String, String> {
    std::env::current_exe()
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|e| format!("could not locate this program's own exe: {e}"))
}

/// The launcher directory, for the scripts that have no console of their own.
///
/// The reference's is `%USERPROFILE%\.wsldashboard\scripts\launcher`. Ours is
/// the same path with this program's directory name, which is where
/// `settings.toml` and `logs/` already live.
pub fn vbs_launcher_dir() -> Result<std::path::PathBuf, String> {
    let userprofile = std::env::var("USERPROFILE").map_err(|_| "USERPROFILE not found".to_string())?;
    let dir = std::path::Path::new(&userprofile)
        .join(".wsadashboard")
        .join("scripts")
        .join("launcher");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create launcher directory: {e}"))?;
    Ok(dir)
}

/// The launcher file name for a task id. Hashed, so it is stable across an edit
/// and is a legal file name whatever the reader typed as a display name.
pub fn vbs_launcher_file_name(task_id: &str) -> String {
    format!("launcher-{}.vbs", md5_hex(task_id.as_bytes()))
}

/// The prefix our own comment lines in a launcher carry.
const LAUNCHER_MARK: &str = "' wsadashboard:";

/// The body of a launcher that brings the subsystem up and then runs
/// `command_line` with no window at all.
///
/// **Three things in one file, and every one of them is load-bearing.**
///
/// 1. **The markers.** The first two lines are VBS comments carrying the command
///    and the arguments *as the reader typed them*. They are here because `/TR`
///    holds a `wscript.exe "<launcher>"` line that does not show either, and
///    because the interpreter form below (`powershell.exe -File "..."`) is not
///    what the reader wrote. This file is the task's own record of its command,
///    the same way `<Description>` is its record of the schedule -- and for the
///    same reason: Windows' task store has nowhere else to put it. A reader who
///    opens the file sees their own command line, and this is the only place the
///    arguments survive at all.
///
/// 2. **The cold start.** The first run is this program in its `start` mode,
///    which is already the cold start every other page uses: it probes, returns
///    at once when the subsystem is up, and otherwise starts it and waits
///    (measured at 17s on this machine, spec §13.6). Without it, a task that
///    runs an `adb` command is a *silent* failure on a machine that has just
///    booted: the subsystem is not up, `adb` says so on a stderr nobody reads,
///    and Task Scheduler reports the task as having run. That is spec §11's
///    tenth acceptance item.
///
///    Its exit code is deliberately ignored. A task whose command does not need
///    the subsystem -- a backup script, anything else a reader wants on a
///    schedule -- must still run when the subsystem cannot be started, and a
///    launcher that stopped there would turn a working task into a missing one.
///
/// 3. **The command, hidden and waited for.** `wscript.exe` is a GUI-subsystem
///    process, so it never allocates a console, and `WScript.Shell.Run` with
///    window style `0` runs the command with no window and waits for it
///    (`True`). All three parts matter: without the third the launcher exits
///    immediately and the task's "last result" is the launcher's, not the
///    command's.
///
/// **The last `shell.Run` is the task's own command; the first is ours.** That
/// is the contract [`extract_script_path_from_vbs`] relies on for a launcher
/// written before the markers existed, and any further run added here must go
/// *before* the command's.
pub fn vbs_launcher_body(
    our_exe: &str,
    command: &str,
    arguments: &str,
    command_line: &str,
) -> String {
    format!(
        "{LAUNCHER_MARK}cmd={}\n\
         {LAUNCHER_MARK}args={}\n\
              ' Written by WSA Dashboard. The command above is the one the reader\n\
         ' typed; the shell.Run below it brings the subsystem up first (a no-op\n\
         ' when it is already running).\n\
         Set shell = CreateObject(\"WScript.Shell\")\n\
         shell.Run \"\"\"{}\"\" /scheduler\", 0, True\n\
         shell.Run \"{}\", 0, True\n",
        command.replace('\r', "").replace('\n', " "),
        arguments.replace('\r', "").replace('\n', " "),
        our_exe.replace('"', "\"\""),
        command_line.replace('"', "\"\"")
    )
}

/// One marked value out of a launcher body, or `None` when it is not there.
///
/// Pure, so that the shape a launcher is written in and the shape it is read
/// back in are held together by a test rather than by an afternoon.
///
/// A newline in the value is impossible: [`vbs_launcher_body`] replaces any with
/// a space, because a marker is one line and a command line a reader pasted out
/// of somewhere could otherwise end it early and turn the rest of itself into
/// what looks like another marker.
pub fn launcher_marker(body: &str, key: &str) -> Option<String> {
    let prefix = format!("{LAUNCHER_MARK}{key}=");
    body.lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .map(|value| value.to_string())
}

/// The command and arguments out of a launcher body, when it carries both.
///
/// `None` for a launcher written before the markers existed -- the reference's,
/// or one an older build of this program wrote -- which is the answer that sends
/// the caller to [`extract_script_path_from_vbs`] and [`unwrap_command`].
pub fn launcher_command_from_body(body: &str) -> Option<(String, String)> {
    let command = launcher_marker(body, "cmd")?;
    let arguments = launcher_marker(body, "args")?;
    Some((command, arguments))
}

/// The command and arguments recorded in the launcher a `/TR` value points at.
pub fn launcher_command(tr: &str) -> Option<(String, String)> {
    let body = std::fs::read_to_string(launcher_path_in(tr)?).ok()?;
    launcher_command_from_body(&body)
}

/// Write the launcher for `task_id`, and answer with its path.
pub fn ensure_vbs_launcher(
    task_id: &str,
    our_exe: &str,
    command: &str,
    arguments: &str,
    command_line: &str,
) -> Result<String, String> {
    let dir = vbs_launcher_dir()?;
    let path = dir.join(vbs_launcher_file_name(task_id));
    std::fs::write(
        &path,
        vbs_launcher_body(our_exe, command, arguments, command_line),
    )
    .map_err(|e| format!("Failed to write VBS launcher: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// Remove the launcher for `task_id`, if there is one. Deleting a task is what
/// calls this: without it the directory grows one orphan per deleted task.
pub fn cleanup_vbs_launcher(task_id: &str) {
    if let Ok(dir) = vbs_launcher_dir() {
        let path = dir.join(vbs_launcher_file_name(task_id));
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// The launcher path inside a `/TR` value, when it is one of ours.
///
/// Shared by [`launcher_command`] and [`unwrap_command`] so that "is this a
/// wrapped command, and where is its file" is answered once.
///
/// Two spellings (`wscript.exe` and `wscript`) and any case: Windows accepts all
/// four, and which one a task store entry carries is the writer's business.
/// `unwrap_command` had to retry with both sides lowered before this existed,
/// which is exactly the kind of duplication this function removes.
pub fn launcher_path_in(tr: &str) -> Option<String> {
    /// The program name and then whitespace, or it is not a wrapped command:
    /// `wscript.exeX` is some other file.
    fn strip_program<'a>(trimmed: &'a str, program: &str) -> Option<&'a str> {
        let head = trimmed.get(..program.len())?;
        if !head.eq_ignore_ascii_case(program) {
            return None;
        }
        let rest = &trimmed[program.len()..];
        rest.starts_with(char::is_whitespace).then(|| rest.trim_start())
    }

    let trimmed = tr.trim();
    let rest = ["wscript.exe", "wscript"]
        .iter()
        .find_map(|program| strip_program(trimmed, program))?;

    Some(match rest.strip_prefix('"') {
        Some(quoted) => match quoted.find('"') {
            Some(end) => quoted[..end].to_string(),
            None => quoted.to_string(),
        },
        None => rest.split_whitespace().next().unwrap_or(rest).to_string(),
    })
}

/// The script path inside a launcher written by [`vbs_launcher_body`].
///
/// The **legacy** path: a launcher that carries no markers, which means one the
/// reference wrote or one this program wrote before the cold start arrived. It
/// reads the last `shell.Run` and then the `""`-quoted run that the escaping
/// produces, so it works for any script type. `None` for a file this program did
/// not write, which is the answer that keeps a hand-written `.vbs` in that
/// directory from being read as one of ours.
///
/// `rfind` rather than `find` because of the cold-start line: the first run in a
/// two-run launcher is this program starting the subsystem, and reading it would
/// hand the caller the path of our own exe. A launcher with a single run answers
/// the same under both.
///
/// `None` is also the answer for a *direct program*, whose command line carries
/// no `""` pair for this to find -- which is exactly the case the markers above
/// exist to cover.
pub fn extract_script_path_from_vbs(vbs_path: &str) -> Option<String> {
    let content = std::fs::read_to_string(vbs_path).ok()?;
    let after_prefix = &content[content.rfind("shell.Run \"")? + "shell.Run \"".len()..];
    let after_first = &after_prefix[after_prefix.find("\"\"")? + 2..];
    let end = after_first.find("\"\"")?;
    Some(after_first[..end].to_string())
}

/// The interpreter line a script type needs, or `None` for a program that can be
/// run directly.
///
/// Pure, and the reason it is separate from [`tr_value`]: this is the decision
/// -- `.ps1` runs under PowerShell, `.py` under Python, `.bat`/`.cmd` under
/// `cmd.exe /c`, everything else as itself -- and the decision is what a test
/// can pin. Writing the launcher file is I/O and is not.
///
/// A command with no extension is run directly and not guessed at: an unknown
/// type launched through an interpreter this program picked is worse than an
/// unknown type that fails with Windows' own message.
pub fn wrapper_command_line(command: &str, arguments: &str) -> Option<String> {
    let lower = command.to_lowercase();
    let program = if lower.ends_with(".ps1") {
        // `-ExecutionPolicy Bypass` because the default policy on a managed
        // machine refuses an unsigned local script, and the reader who typed the
        // path has already decided to run it.
        format!("powershell.exe -ExecutionPolicy Bypass -File \"{command}\"")
    } else if lower.ends_with(".py") {
        format!("python.exe \"{command}\"")
    } else if lower.ends_with(".bat") || lower.ends_with(".cmd") {
        format!("cmd.exe /c \"{command}\"")
    } else {
        return None;
    };
    Some(if arguments.is_empty() {
        program
    } else {
        format!("{program} {arguments}")
    })
}

/// What `schtasks /TR` has to say for this command.
///
/// **Every task is wrapped**, not only the script types. The launcher is where
/// the cold start lives ([`vbs_launcher_body`]), so a task that runs an `adb`
/// command needs it whether the command is a script or a program.
///
/// The cost is that `/TR` no longer *shows* what the task runs, and that the
/// task's own command cannot be read back out of it for a direct program (there
/// is no quoted path for [`extract_script_path_from_vbs`] to find). Both are
/// paid for in `<Description>`, which carries `cmd:` and `args:` lines -- the
/// same place the cron already lives, and for the same reason: Windows'
/// task store cannot hold what we need to remember, so the task remembers it for
/// itself.
pub fn tr_value(
    command: &str,
    arguments: &str,
    task_id: &str,
    our_exe: &str,
) -> Result<String, String> {
    let inner = match wrapper_command_line(command, arguments) {
        None => command_line(command, arguments),
        Some(inner) => inner,
    };
    let launcher = ensure_vbs_launcher(task_id, our_exe, command, arguments, &inner)?;
    Ok(format!("wscript.exe \"{launcher}\""))
}

/// A command and its arguments as one line, with no wrapper.
fn command_line(command: &str, arguments: &str) -> String {
    if arguments.is_empty() {
        command.to_string()
    } else {
        format!("{command} {arguments}")
    }
}

/// Whether a `/TR` value is one of our launchers.
fn is_wrapped_command(tr: &str) -> bool {
    let lower = tr.trim().to_lowercase();
    lower.starts_with("wscript.exe ") || lower.starts_with("wscript ")
}

/// The original command, recovered from a `/TR` value a previous run wrote.
///
/// `wscript.exe` gets its launcher read; the three interpreters are unwrapped by
/// prefix; a direct program is returned as itself. The reference's function,
/// carried over, with the quotes-and-tokens rules it had.
///
/// What is **not** recovered is the arguments of a *legacy* wrapped command:
/// they were baked into the launcher as `interpreter "script" args`, and pulling
/// them back out would mean parsing a line this program wrote rather than a line
/// Windows wrote. That is the wart the reference has, and MAPPING 16.7 records
/// it. Launchers written since the cold start arrived carry markers instead and
/// are read exactly -- command *and* arguments -- by [`launcher_command`].
pub fn unwrap_command(cmd: &str) -> String {
    let trimmed = cmd.trim();

    if let Some((command, _)) = launcher_command(trimmed) {
        return command;
    }

    let prefixes = [
        "wscript.exe ",
        "powershell.exe -WindowStyle Hidden -ExecutionPolicy Bypass -File ",
        "powershell.exe -ExecutionPolicy Bypass -File ",
        "python.exe ",
        "cmd.exe /c ",
    ];

    for prefix in prefixes {
        let Some(rest) = trimmed.strip_prefix(prefix) else {
            // The comparison is on an ASCII prefix, but the reader may have a
            // path that differs in case; retry once with both lowered rather
            // than duplicate five more arms.
            let (a, b) = (trimmed.to_lowercase(), prefix.to_lowercase());
            if a.starts_with(&b) {
                return unwrap_command(&format!("{prefix}{}", &trimmed[prefix.len()..]));
            }
            continue;
        };
        let rest = rest.trim();

        if prefix == "wscript.exe " {
            // No markers, so this is a launcher written before they existed: the
            // reference's, or an older build's. `launcher_path_in` rather than
            // the inline pair below, so that "which file does this /TR point at"
            // is answered in one place.
            let launcher = launcher_path_in(trimmed).unwrap_or_else(|| rest.to_string());
            return extract_script_path_from_vbs(&launcher).unwrap_or(launcher);
        }

        if let Some(quoted) = rest.strip_prefix('"') {
            return match quoted.find('"') {
                Some(end) => quoted[..end].to_string(),
                // No closing quote: the whole rest is the path. Windows accepted
                // it, so the reader's task runs, and truncating it here would
                // make the dialog edit a path that is not the one on disk.
                None => quoted.to_string(),
            };
        }
        return rest.split_whitespace().next().unwrap_or(rest).to_string();
    }

    trimmed.to_string()
}

// -- the /Create argument list -------------------------------------------------

/// Escape the `/TR` value for `cmd.exe`, and leave every other argument alone.
///
/// The elevated path runs `schtasks` as a command *line* rather than as an
/// argument vector, and the command it usually carries is a path with spaces in
/// it -- `C:\Program Files\...`. Without this, `cmd.exe` splits the path at the
/// first space and `schtasks` is asked to run `C:\Program`, which fails with a
/// message about a file nobody mentioned.
///
/// `""` for a literal `"` is `cmd.exe`'s own rule inside a quoted string, not
/// Rust's and not `schtasks`'s.
pub fn escape_tr_args(args: &[String]) -> Vec<String> {
    let mut out = args.to_vec();
    for index in 0..out.len() {
        if out[index] == "/TR" {
            if let Some(value) = out.get(index + 1).cloned() {
                if value.contains(' ') || value.contains('"') {
                    out[index + 1] = format!("\"{}\"", value.replace('"', "\"\""));
                }
            }
            break;
        }
    }
    out
}

/// The `schtasks /Create` argument vector.
///
/// Pure, so the argument *order* is pinned: `schtasks` does not care, but a
/// reader comparing this against the dialog's fields does, and the optional
/// flags are emitted in the reference's order.
pub fn build_create_args(
    task_name: &str,
    tr: &str,
    schtasks: &SchtasksParams,
    admin_mode: bool,
) -> Vec<String> {
    let mut args = vec![
        "/Create".to_string(),
        "/TN".to_string(),
        task_name.to_string(),
        "/TR".to_string(),
        tr.to_string(),
        "/F".to_string(),
        "/SC".to_string(),
        schtasks.sc.clone(),
    ];

    for (flag, value) in [
        ("/MO", &schtasks.mo),
        ("/ST", &schtasks.st),
        ("/D", &schtasks.d),
        ("/M", &schtasks.m),
    ] {
        if let Some(value) = value {
            args.push(flag.to_string());
            args.push(value.clone());
        }
    }

    if admin_mode {
        args.push("/RL".to_string());
        args.push("HIGHEST".to_string());
    }

    args
}

/// The PowerShell that writes the cron into the task's description.
///
/// `Get-ScheduledTask` / `Set-ScheduledTask` rather than `schtasks /Change`,
/// which has no switch for a description. Single quotes inside the PowerShell
/// string literal are doubled, which is PowerShell's own escape; without it a
/// display name containing an apostrophe ends the literal and the rest of the
/// name is executed as PowerShell.
///
/// **This string contains no `"`, and that is a requirement rather than a
/// coincidence.** [`build_description_command`] wraps it in one pair of double
/// quotes for `cmd.exe`, and cmd's parser is what decides whether the `|` in
/// `$t | Set-ScheduledTask` is a pipe or part of the quoted text. One `"` inside
/// the script would end that quotation early and hand cmd a pipe. Measured, not
/// reasoned: the whole shape -- `(schtasks … & powershell -NoProfile -Command
/// "…") > "log" 2>&1`, on one line -- was run and wrote `cron:0 3 * * *` into a
/// task's `<Description>`, which then read back out of `/Query /XML`.
pub fn description_script(task_path: &str, cron_expr: &str) -> String {
    let (folder, name) = match task_path.rfind('\\') {
        Some(pos) => (&task_path[..=pos], &task_path[pos + 1..]),
        None => ("", task_path),
    };
    format!(
        "$t = Get-ScheduledTask -TaskPath '{}' -TaskName '{}'; if($t){{ $t.Description = '{}'; $t | Set-ScheduledTask | Out-Null }}",
        folder.replace('\'', "''"),
        name.replace('\'', "''"),
        description_for_cron(cron_expr).replace('\'', "''"),
    )
}

/// [`description_script`] with an interpreter around it, as one `cmd.exe` line.
///
/// The line, not a `Command`: the elevated path joins several of these with `&`
/// into a batch behind one UAC prompt (`elevate::build_elevated_shell`), and
/// there is no argument vector at that end to put a script into.
///
/// No **additional** outer quotes, deliberately. Wrapping the whole thing again
/// would nest the pair this already has, and the probe above is the standing
/// evidence that one pair is what cmd needs.
pub fn build_description_command(task_path: &str, cron_expr: &str) -> String {
    format!(
        "powershell -NoProfile -Command \"{}\"",
        description_script(task_path, cron_expr)
    )
}

// -- TaskXml to the row the page draws ----------------------------------------

/// One parsed `<Task>` as the struct the handlers pass to the page.
///
/// # The four fields Windows will not give us
///
/// `status`, `last_run` and `next_run` are `NOT_REPORTED`. The reference filled
/// them from `schtasks /Query /FO CSV /V` columns 3, 5 and 2 -- and those are
/// exactly the columns whose *headers and values* are localised, which is what
/// `xml.rs` replaced. The XML carries no task state at all: not the last result,
/// not the last run time, and the `<StartBoundary>` of a trigger is the schedule
/// the reader configured, not the moment Windows will next fire it. So all three
/// say `N/A` rather than a date this program computed.
///
/// `trigger_summary` is empty, and not `NOT_REPORTED`, because the difference is
/// real: `N/A` means the system would not say, and this is a field the reference
/// declared and then never filled -- it is `String::new()` at the one site that
/// constructs the struct. Nothing renders it either. Filling it here would be
/// adding a feature the reference does not have.
pub fn to_scheduler_task(task: &TaskXml) -> SchedulerTask {
    let task_id = task_id_from_path(&task.path);
    // **Windows splits `/TR` into a program and its arguments**, at the first
    // space unless the program is quoted -- so `wscript.exe "<launcher>"` is
    // stored as `<Command>wscript.exe</Command>` with the launcher path in
    // `<Arguments>`. The line this program wrote therefore has to be put back
    // together before it can be read, and reading only `<Command>` was a defect
    // P7 shipped: every script task came back as the command `wscript.exe`.
    // The live round trip found it the moment every task started being wrapped;
    // the unit test could not, because it built the unsplit line by hand.
    let tr = command_line(&task.command, &task.arguments);
    let wrapped = is_wrapped_command(&tr);
    // The task's own record of what it runs, when it has one. A launcher written
    // since the cold start arrived carries the reader's command *and* arguments
    // as marker lines, and this is what reads them.
    let recorded = launcher_command(&tr);
    let (command, arguments) = match recorded {
        Some((command, arguments)) => (command, arguments),
        // A legacy launcher -- no markers -- keeps the reference's wart: the
        // command is recoverable and the arguments are not; see `unwrap_command`.
        None if wrapped => (unwrap_command(&tr), String::new()),
        // Not wrapped at all, so `<Command>` and `<Arguments>` are already the
        // reader's two fields. `unwrap_command` still runs, because a *command*
        // can carry its own interpreter prefix (`python.exe "..."`) from the
        // reference's era.
        None => (unwrap_command(&task.command), task.arguments.clone()),
    };
    SchedulerTask {
        display_name: task_id.clone(),
        task_id,
        cron_expr: cron_from_description(&task.description),
        command,
        arguments,
        // **Absent means enabled.** Measured, and the measurement is the whole
        // reason this reads the way it does: `schtasks /Create` writes a task
        // with no `<Enabled>` element at all and `schtasks /Query /XML` answers
        // the same for it, while `schtasks /Change /DISABLE` *adds*
        // `<Enabled>false</Enabled>` and `/ENABLE` takes it away again. Windows's
        // own default for a task it just created is therefore `true`, and reading
        // it as `false` -- which this did until a live round trip caught it --
        // shows every task on the page as switched off and makes the card's
        // switch dead: it would read `!false`, send `/ENABLE` to a task that is
        // already enabled, and change nothing.
        //
        // `Option` is still the field's type, because `<Enabled>true</Enabled>`
        // written by the Task Scheduler GUI is a real thing this has to read too.
        // The three states are: absent (on), explicit true (on), explicit false
        // (off).
        enabled: task.enabled.unwrap_or(true),
        status: NOT_REPORTED.to_string(),
        last_run: NOT_REPORTED.to_string(),
        next_run: NOT_REPORTED.to_string(),
        trigger_summary: String::new(),
        admin_mode: runs_as_admin(task),
        author: if task.author.trim().is_empty() {
            NOT_REPORTED.to_string()
        } else {
            task.author.clone()
        },
    }
}

// -- the five that touch the machine ------------------------------------------

/// Read this program's tasks. Empty is empty, and a failed query is an error.
///
/// The distinction is the whole reason this returns a `Result`: `schtasks` writes
/// its failures to stdout as often as to stderr, and an `ERROR:` line parses to
/// zero tasks -- so a page that only looked at the vector would show "you have no
/// tasks" for a machine where the command did not run.
pub fn list_tasks() -> Result<Vec<SchedulerTask>, String> {
    let output = Command::new("schtasks")
        .args(["/Query", "/XML"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to execute schtasks: {e}"))?;

    let stdout = decode_output(&output.stdout);
    if !output.status.success() {
        let stderr = decode_output(&output.stderr);
        let message = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(format!("schtasks query failed: {message}"));
    }

    let tasks = parse_tasks_xml(&stdout);
    if tasks.is_empty() && stdout.contains("ERROR") {
        return Err(format!("schtasks query failed: {}", stdout.trim()));
    }

    debug!(
        "schtasks /Query /XML: {} bytes, {} tasks, {} in {}",
        stdout.len(),
        tasks.len(),
        our_tasks(&tasks).len(),
        TASK_FOLDER
    );
    Ok(our_tasks(&tasks).into_iter().map(to_scheduler_task).collect())
}

/// Create (or replace) a task, and record its cron in the task's own description.
///
/// `requires_elevation` and `old_task_id` are the handler layer's answers, as in
/// the reference: whether the task store needs an administrator is a machine
/// question, and an edit is a delete and a create behind one prompt.
///
/// **The launcher is written before `schtasks` runs**, because `/TR` has to name
/// it: a task that fails to be created leaves one orphan file in the launcher
/// directory, which is the smaller of the two problems and is what
/// `cleanup_vbs_launcher` is for.
pub fn create_task(
    params: &CreateTaskParams,
    requires_elevation: bool,
    old_task_id: Option<&str>,
) -> Result<(), String> {
    let parsed = parse_cron(&params.cron_expr)?;
    let task_name = task_path_for_id(&params.display_name);
    let our_exe = our_exe_path()?;
    let tr = tr_value(
        &params.command,
        &params.arguments,
        &params.display_name,
        &our_exe,
    )?;
    let args = build_create_args(&task_name, &tr, &parsed.schtasks, params.admin_mode);

    if requires_elevation {
        let mut commands = Vec::new();
        if let Some(old) = old_task_id {
            commands.push(format!(
                "schtasks /Delete /TN \"{}\" /F",
                task_path_for_id(old)
            ));
        }
        commands.push(format!("schtasks {}", escape_tr_args(&args).join(" ")));
        commands.push(build_description_command(&task_name, &params.cron_expr));
        return run_invisible_elevated_commands(commands);
    }

    let output = Command::new("schtasks")
        .args(&args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to execute schtasks: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "schtasks create failed: {}",
            decode_output(&output.stderr).trim()
        ));
    }

    set_task_description(&task_name, &params.cron_expr);
    Ok(())
}

/// Delete a task, and its launcher.
pub fn delete_task(task_id: &str, needs_elevation: bool) -> Result<(), String> {
    let task_name = task_path_for_id(task_id);

    if needs_elevation {
        run_invisible_elevated_command(&format!("schtasks /Delete /TN \"{task_name}\" /F"))?;
    } else {
        let output = Command::new("schtasks")
            .args(["/Delete", "/TN", &task_name, "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Failed to execute schtasks: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "schtasks delete failed: {}",
                decode_output(&output.stderr).trim()
            ));
        }
    }

    // After the task is really gone. Doing it first leaves a task whose command
    // is a file that no longer exists if the delete then fails.
    cleanup_vbs_launcher(task_id);
    Ok(())
}

/// Turn a task on or off.
pub fn toggle_task(task_id: &str, enable: bool, needs_elevation: bool) -> Result<(), String> {
    let task_name = task_path_for_id(task_id);
    let action = if enable { "/ENABLE" } else { "/DISABLE" };

    if needs_elevation {
        return run_invisible_elevated_command(&format!(
            "schtasks /Change /TN \"{task_name}\" {action}"
        ));
    }

    let output = Command::new("schtasks")
        .args(["/Change", "/TN", &task_name, action])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to execute schtasks: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "schtasks change failed: {}",
            decode_output(&output.stderr).trim()
        ));
    }
    Ok(())
}

/// Start a task now, without waiting for its trigger.
pub fn run_task(task_id: &str, needs_elevation: bool) -> Result<(), String> {
    let task_name = task_path_for_id(task_id);

    if needs_elevation {
        return run_invisible_elevated_command(&format!("schtasks /Run /TN \"{task_name}\""));
    }

    let output = Command::new("schtasks")
        .args(["/Run", "/TN", &task_name])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to execute schtasks: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "schtasks run failed: {}",
            decode_output(&output.stderr).trim()
        ));
    }
    Ok(())
}

/// Write the cron into the task's description, on the unelevated path.
///
/// A failure is a `warn` and nothing more: the task has already been created by
/// the time this runs, so returning an error here would tell the reader their
/// task was not created when it was. What they lose is the expression in the
/// edit dialog, and that is what the warning says. Both streams are logged,
/// because PowerShell writes its own failures to stdout as often as to stderr.
///
/// **No `cmd.exe` in the middle.** It used to be `cmd /c <build_description_-
/// command(..)>`, and the cron was silently lost on every task this path
/// created: `Command` quotes its arguments the way `CommandLineToArgvW` expects,
/// the command line already carried a pair of quotes of its own around the
/// PowerShell script, and `cmd.exe` -- which does not read `\"` as an escape --
/// was handed a mangled line. `powershell` never ran, the exit code was reported
/// as a `warn` in a log nobody reads, and the only symptom was an edit dialog
/// that opened with an empty schedule. `powershell` takes the script as one
/// argument instead, which is what an argument vector is for. The live round
/// trip below is what found it; nothing short of a real `schtasks` could have.
fn set_task_description(task_name: &str, cron_expr: &str) {
    let script = description_script(task_name, cron_expr);
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script.as_str()])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            info!("recorded the cron for '{task_name}'");
        }
        Ok(out) => warn!(
            "could not record the cron for '{task_name}': {} {}",
            decode_output(&out.stdout).trim(),
            decode_output(&out.stderr).trim()
        ),
        Err(e) => warn!("could not run powershell to record the cron for '{task_name}': {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::xml::ActionKind;
    use super::*;

    // -- decoding ------------------------------------------------------------

    #[test]
    fn a_utf8_document_with_a_bom_loses_only_the_bom() {
        let bytes = [&[0xEF, 0xBB, 0xBF][..], b"<Tasks/>"].concat();
        assert_eq!(decode_output(&bytes), "<Tasks/>");
    }

    #[test]
    fn utf16_is_decoded_in_both_endiannesses_and_in_both_bom_states() {
        let text = "<Tasks/>";

        let mut le_bom = vec![0xFF, 0xFE];
        let mut be_bom = vec![0xFE, 0xFF];
        let mut le_bare = Vec::new();
        for unit in text.encode_utf16() {
            le_bom.extend_from_slice(&unit.to_le_bytes());
            be_bom.extend_from_slice(&unit.to_be_bytes());
            le_bare.extend_from_slice(&unit.to_le_bytes());
        }

        assert_eq!(decode_output(&le_bom), "<Tasks/>");
        assert_eq!(decode_output(&be_bom), "<Tasks/>");
        // The no-BOM case is the one that matters: it is what the reference's
        // `from_utf8_lossy` turns into a NUL between every character, and it is
        // the reason the probe exists.
        assert_eq!(decode_output(&le_bare), "<Tasks/>");
    }

    /// The measurement `decode_output` rests on, as a test. If `schtasks` ever
    /// starts answering in UTF-16 on a future Windows, this fails first and the
    /// comment above stops being true.
    ///
    /// `#[ignore]` because Q8 keeps machine-dependent work out of the suite.
    #[test]
    #[ignore = "spawns schtasks; Q8 keeps machine-dependent work out of the suite"]
    fn the_real_query_output_is_what_the_decoder_was_built_for() {
        let output = std::process::Command::new("schtasks")
            .args(["/Query", "/XML"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .expect("schtasks is on PATH");
        let raw = &output.stdout;
        println!("bytes: {}", raw.len());
        println!(
            "first 16: {}",
            raw[..16.min(raw.len())]
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" ")
        );

        let text = decode_output(raw);
        println!("first 40 chars: {:?}", &text[..40.min(text.len())]);
        let tasks = parse_tasks_xml(&text);
        println!("{} tasks decoded, {} ours", tasks.len(), our_tasks(&tasks).len());

        assert!(
            tasks.len() > 10,
            "decoded {} tasks; the decoder and the document have diverged",
            tasks.len()
        );
        assert!(
            !text.contains('\u{0}'),
            "the decoded text still has NULs in it, so the encoding was misread"
        );
    }

    // -- the id and its launcher ---------------------------------------------

    /// The reference's `md5` crate call, checked against the digest everyone
    /// knows, so that a dependency bump that changed the output would fail here
    /// rather than by silently renaming every launcher on disk.
    #[test]
    fn md5_is_the_md5_everyone_else_computes() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        // Lowercase, and exactly 32 characters: the launcher name is what a
        // reader would compare by hand.
        assert_eq!(md5_hex(b"abc").len(), 32);
        assert_eq!(md5_hex(b"abc"), md5_hex(b"abc").to_lowercase());
    }

    #[test]
    fn the_launcher_name_is_stable_for_an_id_and_different_for_another() {
        assert_eq!(vbs_launcher_file_name("Nightly"), vbs_launcher_file_name("Nightly"));
        assert_ne!(vbs_launcher_file_name("Nightly"), vbs_launcher_file_name("nightly"));
        assert!(vbs_launcher_file_name("Nightly").starts_with("launcher-"));
        assert!(vbs_launcher_file_name("Nightly").ends_with(".vbs"));
        // A display name can contain anything; a file name cannot. That is the
        // whole reason the id is hashed rather than used.
        let awkward = vbs_launcher_file_name(r#"a:b|c?d*e"f<g>h\i/j"#);
        assert!(!awkward.contains(':') && !awkward.contains('\\') && !awkward.contains('"'));
    }

    #[test]
    fn the_launcher_runs_hidden_and_waits() {
        let body = vbs_launcher_body(
            r"C:\Program Files\WSA Dashboard\wsadashboard.exe",
            r#"C:\a b\task.bat"#,
            "--go",
            r#"cmd.exe /c "C:\a b\task.bat" --go"#,
        );
        // Window style 0 and the third argument `True`: without the second the
        // task's result is the launcher's exit code, not the command's.
        assert!(body.ends_with(", 0, True\n"), "got {body:?}");
        assert!(body.contains("CreateObject(\"WScript.Shell\")"));
        // Every `"` in the command is doubled, which is VBS's own escape.
        assert!(
            body.contains(r#""cmd.exe /c ""C:\a b\task.bat"" --go""#),
            "got {body:?}"
        );
    }

    /// The cold start, and its order: this program first, the reader's command
    /// second. Asserted on the *sequence* rather than on two `contains` checks,
    /// because two runs in the wrong order is a launcher that runs `adb` against
    /// a subsystem it has not started yet -- which is the failure this whole
    /// mechanism exists to prevent, and one that reads as working.
    #[test]
    fn the_launcher_starts_the_subsystem_before_it_runs_the_command() {
        let our_exe = r"C:\Program Files\WSA Dashboard\wsadashboard.exe";
        let body = vbs_launcher_body(our_exe, "adb.exe", "shell ls", "adb.exe shell ls");

        let cold_start = body
            .find(r#"shell.Run """#)
            .expect("the cold start line is missing");
        let command = body
            .rfind("shell.Run \"")
            .expect("the command line is missing");
        assert!(
            cold_start < command,
            "the command runs before the subsystem is started:\n{body}"
        );
        // The exe's own quotes doubled, and `/scheduler` after them: this is
        // this program's own cold start (`app::immediate`), which is spec Q25's
        // switch and the same thing the home page's Launch button gets from the
        // domain layer. Not the CLI's `wsadash start`: that would make every
        // task depend on a second exe being installed beside this one.
        assert!(
            body[cold_start..].starts_with(&format!(
                "shell.Run \"\"\"{}\"\" /scheduler\", 0, True\n",
                our_exe
            )),
            "the cold start line is not this program's `/scheduler`:\n{body}"
        );
        // The literal, asserted here as well as in `app::immediate` where it is
        // accepted: two files own the two halves of one string, and this is the
        // half that writes it.
        const SWITCH: &str = "/scheduler";
        assert!(
            body.contains(&format!("\" {SWITCH}\"")),
            "the launcher no longer calls {SWITCH}:\n{body}"
        );
    }

    /// The reader's own words survive, exactly, even though `/TR` cannot show
    /// them and the interpreter form below is not what they typed.
    #[test]
    fn the_launcher_records_the_command_and_the_arguments_as_they_were_typed() {
        for (command, arguments) in [
            ("C:\\a.exe", "/x"),
            ("C:\\Program Files\\WSA\\adb.exe", "shell am start -n com.x/.Y"),
            ("C:\\s.ps1", "--flag \"quoted value\""),
            ("C:\\a.exe", ""),
        ] {
            let inner = wrapper_command_line(command, arguments)
                .unwrap_or_else(|| format!("{command} {arguments}").trim().to_string());
            let body = vbs_launcher_body("C:\\w.exe", command, arguments, &inner);
            assert_eq!(
                launcher_command_from_body(&body),
                Some((command.to_string(), arguments.to_string())),
                "{body}"
            );
        }
    }

    /// A command line a reader pasted out of somewhere can contain a newline, and
    /// a marker is one line. Left alone, the tail of such a command would read as
    /// a marker of its own and the dialog would pre-fill something that is not
    /// what runs.
    #[test]
    fn a_newline_in_a_command_cannot_end_its_marker_line() {
        let body = vbs_launcher_body("C:\\w.exe", "C:\\a.exe", "/x\ncmd=evil", "C:\\a.exe /x");
        assert_eq!(
            launcher_command_from_body(&body),
            Some(("C:\\a.exe".to_string(), "/x cmd=evil".to_string())),
            "{body}"
        );
    }

    /// A launcher that carries no markers -- the reference's, or one an older
    /// build of this program wrote -- is `None`, which is what sends the caller
    /// to `extract_script_path_from_vbs`.
    #[test]
    fn a_launcher_without_markers_has_no_recorded_command() {
        let legacy = format!(
            "Set shell = CreateObject(\"WScript.Shell\")\nshell.Run \"{}\", 0, True\n",
            r#"cmd.exe /c "C:\a b\task.bat""#.replace('"', "\"\"")
        );
        assert_eq!(launcher_command_from_body(&legacy), None);
        // And the legacy reader still answers for it.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("legacy.vbs");
        std::fs::write(&path, &legacy).expect("write");
        assert_eq!(
            extract_script_path_from_vbs(&path.to_string_lossy()),
            Some(r"C:\a b\task.bat".to_string())
        );
    }

    /// `launcher_path_in` is what both readers use to find the file, so the
    /// three shapes a `/TR` value comes in all have to resolve.
    #[test]
    fn a_wrapped_tr_resolves_to_its_launcher_whatever_the_quoting() {
        for tr in [
            r#"wscript.exe "C:\l\launcher-a.vbs""#,
            r#"WScript.exe "C:\l\launcher-a.vbs""#,
            r#"wscript.exe C:\l\launcher-a.vbs"#,
        ] {
            assert_eq!(
                launcher_path_in(tr),
                Some(r"C:\l\launcher-a.vbs".to_string()),
                "{tr}"
            );
        }
        for not_ours in [r"C:\a.exe", "", "powershell.exe -File C:\\a.ps1"] {
            assert_eq!(launcher_path_in(not_ours), None, "{not_ours}");
        }
    }

    // -- the cron in the description -----------------------------------------

    #[test]
    fn the_cron_round_trips_through_the_description() {
        for cron in ["0 9 * * *", "*/5 * * * *", "30 8 1,15 * *"] {
            assert_eq!(cron_from_description(&description_for_cron(cron)), cron);
        }
    }

    #[test]
    fn a_description_that_is_not_ours_yields_no_expression() {
        for description in [
            "",
            "WSA Dashboard nightly task",
            // The prefix has to be at the start and has to be the whole prefix:
            // prose that mentions cron is not a task this program wrote, and
            // reading half a sentence as a schedule is how a task gets edited
            // into a different schedule than it had.
            "see cron:0 9 * * * for details",
            "cronx:0 9 * * *",
        ] {
            assert_eq!(
                cron_from_description(description),
                "",
                "description was {description:?}"
            );
        }
    }

    #[test]
    fn surrounding_whitespace_around_a_recorded_cron_is_not_part_of_it() {
        assert_eq!(cron_from_description("  cron:  0 9 * * *  "), "0 9 * * *");
    }

    // -- the interpreter decision --------------------------------------------

    #[test]
    fn a_script_type_gets_its_interpreter_and_a_program_gets_none() {
        assert_eq!(
            wrapper_command_line(r"C:\a\s.ps1", "").as_deref(),
            Some(r#"powershell.exe -ExecutionPolicy Bypass -File "C:\a\s.ps1""#)
        );
        assert_eq!(
            wrapper_command_line(r"C:\a\s.py", "").as_deref(),
            Some(r#"python.exe "C:\a\s.py""#)
        );
        assert_eq!(
            wrapper_command_line(r"C:\a\s.bat", "").as_deref(),
            Some(r#"cmd.exe /c "C:\a\s.bat""#)
        );
        assert_eq!(
            wrapper_command_line(r"C:\a\s.cmd", "").as_deref(),
            Some(r#"cmd.exe /c "C:\a\s.cmd""#)
        );
        // The ones that are run as themselves. `adb` is in here, and so is a
        // path with no extension at all -- an unknown type launched through an
        // interpreter this program chose is worse than Windows' own error.
        assert_eq!(wrapper_command_line(r"C:\a\adb.exe", ""), None);
        assert_eq!(wrapper_command_line(r"C:\a\nothing", ""), None);
        assert_eq!(wrapper_command_line("", ""), None);
    }

    #[test]
    fn the_extension_is_matched_without_regard_to_case() {
        assert!(wrapper_command_line(r"C:\a\S.PS1", "").is_some());
        assert!(wrapper_command_line(r"C:\a\S.Bat", "").is_some());
        assert!(wrapper_command_line(r"C:\a\s.PY", "").is_some());
    }

    #[test]
    fn arguments_are_appended_to_a_wrapped_line_and_not_to_a_bare_one() {
        assert_eq!(
            wrapper_command_line(r"C:\a\s.py", "--verbose").as_deref(),
            Some(r#"python.exe "C:\a\s.py" --verbose"#)
        );
        // A `""` argument is the same as no argument: one space of difference
        // would otherwise show up as a trailing space in the dialog's field.
        assert_eq!(
            wrapper_command_line(r"C:\a\s.py", "").as_deref(),
            Some(r#"python.exe "C:\a\s.py""#)
        );
    }

    #[test]
    fn a_script_name_that_merely_ends_in_the_letters_is_not_a_script() {
        // `.ps1x` is not a PowerShell script, and a task whose command is a file
        // called that has to be run as itself.
        assert_eq!(wrapper_command_line(r"C:\a\s.ps1x", ""), None);
        assert_eq!(wrapper_command_line(r"C:\a\ps1", ""), None);
    }

    // -- unwrapping what Windows hands back ----------------------------------

    #[test]
    fn the_three_interpreter_wrappers_unwrap_to_the_script_path() {
        assert_eq!(
            unwrap_command(r#"powershell.exe -ExecutionPolicy Bypass -File "C:\a\s.ps1""#),
            r"C:\a\s.ps1"
        );
        assert_eq!(unwrap_command(r#"python.exe "C:\a\s.py""#), r"C:\a\s.py");
        assert_eq!(unwrap_command(r#"cmd.exe /c "C:\a\s.bat""#), r"C:\a\s.bat");
    }

    #[test]
    fn a_program_that_was_never_wrapped_comes_back_as_written() {
        for cmd in [
            r"C:\tools\adb.exe",
            r#"C:\Program Files\WSA\adb.exe"#,
            "",
            r"C:\a\s.ps1",
        ] {
            assert_eq!(unwrap_command(cmd), cmd, "command was {cmd:?}");
        }
    }

    /// The case the whole VBS launcher exists for, and the one that cannot be
    /// tested without a file: a `/TR` of `wscript.exe "<path>"` unwraps to the
    /// script the launcher runs, not to the launcher.
    ///
    /// This is a **legacy** launcher -- no markers -- which is what the reference
    /// writes and what this program wrote before the cold start arrived. The
    /// marked shape is covered by
    /// `the_launcher_records_the_command_and_the_arguments_as_they_were_typed`.
    #[test]
    fn a_launcher_unwraps_to_the_script_it_runs() {
        let dir = std::env::temp_dir().join(format!("wsa-launcher-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let launcher = dir.join("launcher-abc.vbs");

        let inner = wrapper_command_line(r"C:\a\s.ps1", "--go").expect("a ps1 is wrapped");
        let legacy = format!(
            "Set shell = CreateObject(\"WScript.Shell\")\nshell.Run \"{}\", 0, True\n",
            inner.replace('"', "\"\"")
        );
        std::fs::write(&launcher, &legacy).expect("write launcher");

        let tr = format!(r#"wscript.exe "{}""#, launcher.to_string_lossy());
        assert!(is_wrapped_command(&tr));
        assert_eq!(unwrap_command(&tr), r"C:\a\s.ps1");

        // And a launcher file this program did not write falls back to the path
        // rather than to a guess.
        let foreign = dir.join("launcher-other.vbs");
        std::fs::write(&foreign, "WScript.Echo \"hi\"\n").expect("write foreign");
        assert_eq!(
            unwrap_command(&format!(r#"wscript.exe "{}""#, foreign.to_string_lossy())),
            foreign.to_string_lossy()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_a_wscript_line_counts_as_wrapped() {
        assert!(is_wrapped_command(r#"wscript.exe "C:\l.vbs""#));
        assert!(is_wrapped_command("  WScript.exe  x.vbs"));
        assert!(is_wrapped_command("wscript x.vbs"));
        assert!(!is_wrapped_command(r#"C:\tools\adb.exe"#));
        assert!(!is_wrapped_command(""));
        // A path that merely contains the word is not the wrapper.
        assert!(!is_wrapped_command(r#"C:\wscript.exe\adb.exe"#));
    }

    // -- the command line for the elevated batch ------------------------------

    #[test]
    fn a_tr_value_with_spaces_is_quoted_for_cmd_and_the_others_are_left_alone() {
        let args = vec![
            "/Create".to_string(),
            "/TN".to_string(),
            r"\WSA-Dashboard\Nightly".to_string(),
            "/TR".to_string(),
            r#"C:\Program Files\app.exe"#.to_string(),
            "/RL".to_string(),
            "HIGHEST".to_string(),
        ];
        let escaped = escape_tr_args(&args);

        assert_eq!(escaped[4], r#""C:\Program Files\app.exe""#);
        // Nothing else is touched: a `\WSA-Dashboard\Nightly` that picked up
        // quotes would be a task name with quotes in it.
        assert_eq!(escaped[2], r"\WSA-Dashboard\Nightly");
        assert_eq!(escaped[6], "HIGHEST");
        assert_eq!(escaped[0], "/Create");
    }

    #[test]
    fn a_tr_value_with_a_quote_doubles_it_because_that_is_cmds_rule() {
        let args = vec![
            "/TR".to_string(),
            r#"cmd.exe /c "x""#.to_string(),
        ];
        assert_eq!(escape_tr_args(&args)[1], r#""cmd.exe /c ""x""""#);
    }

    #[test]
    fn a_tr_value_without_spaces_or_quotes_is_not_quoted() {
        let args = vec!["/TR".to_string(), r"C:\app.exe".to_string()];
        assert_eq!(escape_tr_args(&args)[1], r"C:\app.exe");
    }

    #[test]
    fn escaping_a_list_with_no_tr_changes_nothing() {
        let args = vec!["/Delete".to_string(), "/TN".to_string(), r"\X".to_string()];
        assert_eq!(escape_tr_args(&args), args);
    }

    // -- the /Create argument list -------------------------------------------

    fn simple(sc: &str) -> SchtasksParams {
        SchtasksParams {
            sc: sc.to_string(),
            mo: None,
            st: None,
            d: None,
            m: None,
        }
    }

    #[test]
    fn the_minimal_create_is_the_five_flags_and_the_schedule() {
        let args = build_create_args(r"\WSA-Dashboard\N", r"C:\a.exe", &simple("MINUTE"), false);
        assert_eq!(
            args,
            vec![
                "/Create",
                "/TN",
                r"\WSA-Dashboard\N",
                "/TR",
                r"C:\a.exe",
                "/F",
                "/SC",
                "MINUTE",
            ]
        );
    }

    #[test]
    fn every_optional_flag_appears_only_when_it_has_a_value_in_the_reference_order() {
        let full = SchtasksParams {
            sc: "MONTHLY".to_string(),
            mo: Some("2".to_string()),
            st: Some("09:30".to_string()),
            d: Some("15".to_string()),
            m: Some("JAN".to_string()),
        };
        let args = build_create_args(r"\WSA-Dashboard\N", r"C:\a.exe", &full, false);
        let tail: Vec<&str> = args[8..].iter().map(String::as_str).collect();
        assert_eq!(
            tail,
            vec!["/MO", "2", "/ST", "09:30", "/D", "15", "/M", "JAN"]
        );

        // A `Some("")` is not the same as a `None`: the flag is emitted with an
        // empty value, which is what `cron.rs` means when it produces one, and
        // hiding it here would silently change the schedule.
        let empty_st = SchtasksParams {
            st: Some(String::new()),
            ..simple("DAILY")
        };
        assert!(build_create_args("t", "c", &empty_st, false).contains(&"/ST".to_string()));
    }

    #[test]
    fn admin_mode_adds_the_run_level_and_is_the_last_pair() {
        let args = build_create_args(r"\WSA-Dashboard\N", r"C:\a.exe", &simple("DAILY"), true);
        assert_eq!(&args[args.len() - 2..], &["/RL", "HIGHEST"]);
        // Exactly once, and after every schedule flag -- the argument order is
        // not what `schtasks` reads, but it is what a reader comparing this
        // against the dialog reads.
        assert_eq!(args.iter().filter(|a| *a == "/RL").count(), 1);
    }

    #[test]
    fn the_task_name_carries_the_folder_and_the_display_name_carries_nothing() {
        let args = build_create_args(
            &task_path_for_id("Nightly backup"),
            r"C:\a.exe",
            &simple("DAILY"),
            false,
        );
        assert_eq!(args[2], r"\WSA-Dashboard\Nightly backup");
    }

    // -- the description writer ----------------------------------------------

    #[test]
    fn the_description_command_names_the_folder_and_the_task_separately() {
        let command = build_description_command(r"\WSA-Dashboard\Nightly", "0 9 * * *");
        // PowerShell wants the folder with its trailing backslash and the name
        // without it. Passing the whole path as `-TaskName` finds nothing and
        // fails silently, which is how the cron goes missing.
        assert!(command.contains("-TaskPath '\\WSA-Dashboard\\'"), "{command}");
        assert!(command.contains("-TaskName 'Nightly'"), "{command}");
        assert!(command.contains("'cron:0 9 * * *'"), "{command}");
    }

    #[test]
    fn an_apostrophe_in_a_display_name_is_doubled_and_does_not_end_the_literal() {
        let command = build_description_command(r"\WSA-Dashboard\Bob's job", "0 9 * * *");
        assert!(command.contains("-TaskName 'Bob''s job'"), "{command}");
        assert!(!command.contains("'Bob's job'"), "the literal was not escaped");
    }

    #[test]
    fn the_description_command_is_quoted_as_one_cmd_argument() {
        let command = build_description_command(r"\WSA-Dashboard\N", "0 9 * * *");
        assert!(command.starts_with("powershell -NoProfile -Command \""));
        assert!(command.ends_with('"'));

        // Exactly one pair of double quotes, and none inside it. This is the
        // property cmd's parser was measured against: with the script quoted
        // once, the `|` in `$t | Set-ScheduledTask` is text and the line runs;
        // with the script quoting itself, the `|` becomes a pipe and the
        // description is never written. See `description_script`.
        assert_eq!(command.matches('"').count(), 2, "{command}");
        let inner = &command["powershell -NoProfile -Command \"".len()..command.len() - 1];
        assert!(!inner.contains('"'), "an unescaped quote survived: {inner}");
        // And the interpreter is not wrapped in a second pair, which is the
        // nesting `elevate::build_elevated_shell` records the same way.
        assert!(!command.starts_with('"'), "{command}");
    }

    #[test]
    fn the_script_and_the_command_line_say_the_same_thing() {
        // `set_task_description` runs the script, the elevated batch runs the
        // command line around it. One PowerShell, two callers, so a test that
        // the second is the first with an interpreter bolted on.
        let script = description_script(r"\WSA-Dashboard\N", "0 9 * * *");
        assert_eq!(
            build_description_command(r"\WSA-Dashboard\N", "0 9 * * *"),
            format!("powershell -NoProfile -Command \"{script}\"")
        );
        // The caret and the ampersand are cmd's, and cmd is what runs this on
        // the elevated path: a script containing one would be split in two.
        for cmd_metacharacter in ['^', '&', '<', '>'] {
            assert!(
                !script.contains(cmd_metacharacter),
                "{cmd_metacharacter} in {script}"
            );
        }
    }

    // -- TaskXml to the row --------------------------------------------------

    fn task_with(path: &str, description: &str, command: &str) -> TaskXml {
        TaskXml {
            path: path.to_string(),
            description: description.to_string(),
            command: command.to_string(),
            action: ActionKind::Exec,
            enabled: Some(true),
            ..Default::default()
        }
    }

    #[test]
    fn a_task_becomes_the_row_the_card_draws() {
        let mut task = task_with(
            r"\WSA-Dashboard\Nightly",
            "cron:0 9 * * *",
            r"C:\Program Files\WSA\adb.exe",
        );
        task.arguments = "shell ls".to_string();
        task.author = r"DESKTOP\reader".to_string();
        task.run_level = "HighestAvailable".to_string();

        let row = to_scheduler_task(&task);
        assert_eq!(row.task_id, "Nightly");
        // The display name is the id: this program has one name for a task, and
        // `task_id_from_path` is the only thing that decides it.
        assert_eq!(row.display_name, "Nightly");
        assert_eq!(row.cron_expr, "0 9 * * *");
        assert_eq!(row.command, r"C:\Program Files\WSA\adb.exe");
        assert_eq!(row.arguments, "shell ls");
        assert_eq!(row.author, r"DESKTOP\reader");
        assert!(row.enabled);
        assert!(row.admin_mode);
        // The three Windows will not report, and the one the reference declared
        // and never filled. See `to_scheduler_task`.
        assert_eq!(row.status, NOT_REPORTED);
        assert_eq!(row.last_run, NOT_REPORTED);
        assert_eq!(row.next_run, NOT_REPORTED);
        assert_eq!(row.trigger_summary, "");
    }

    /// The shape Windows actually stores, and the shape the first version of this
    /// read got wrong: `/TR` is split at the first space, so a wrapped task
    /// arrives as `<Command>wscript.exe</Command>` with the launcher path in
    /// `<Arguments>`. A test that builds `wscript.exe "<path>"` as one string
    /// cannot see that -- and one did not, which is how a script task came back
    /// as the command `wscript.exe` for as long as only direct programs had ever
    /// been created on a real machine.
    #[test]
    fn a_wrapped_task_is_read_from_the_two_halves_windows_stores_it_in() {
        let dir = std::env::temp_dir().join(format!("wsa-split-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let launcher = dir.join("launcher-split.vbs");

        let inner = wrapper_command_line(r"C:\a\s.ps1", "--go").expect("a ps1 is wrapped");
        std::fs::write(
            &launcher,
            vbs_launcher_body("C:\\w.exe", r"C:\a\s.ps1", "--go", &inner),
        )
        .expect("write launcher");

        let mut task = task_with(r"\WSA-Dashboard\Split", "cron:0 9 * * *", "wscript.exe");
        task.arguments = format!("\"{}\"", launcher.to_string_lossy());

        let row = to_scheduler_task(&task);
        assert_eq!(row.command, r"C:\a\s.ps1");
        assert_eq!(
            row.arguments, "--go",
            "the launcher's markers are the only place these survive"
        );
        assert_eq!(row.cron_expr, "0 9 * * *");

        // And the same row without the launcher on disk. The file *is* the
        // record, so there is nothing left to read: the row falls back to
        // `unwrap_command`'s documented answer, which is the launcher's own path
        // -- a true statement about what Windows will try to run, and visibly
        // not the reader's command.
        let _ = std::fs::remove_file(&launcher);
        let gone = to_scheduler_task(&task);
        assert_eq!(gone.command, launcher.to_string_lossy());
        assert_eq!(gone.arguments, "");
    }

    #[test]
    fn a_wrapped_command_keeps_its_script_and_drops_its_wrapper_arguments() {
        let mut task = task_with(
            r"\WSA-Dashboard\S",
            "cron:0 9 * * *",
            r#"python.exe "C:\a\s.py" --go"#,
        );
        // Windows records the whole interpreter line as the command and has no
        // separate arguments for it.
        task.arguments = String::new();

        let row = to_scheduler_task(&task);
        assert_eq!(row.command, r"C:\a\s.py");
        assert_eq!(
            row.arguments, "",
            "the launcher's arguments are not the dialog's arguments"
        );
    }

    /// The three states of `<Enabled>`, and which way the absent one goes.
    ///
    /// This test asserted the opposite until a live round trip against the real
    /// store went red on it: it read "no element" as "off", and *no element* is
    /// what `schtasks /Create` writes for a task it has just created and enabled.
    /// The measurement is in the fixture rather than in a comment, because the
    /// comment was wrong in exactly the same way.
    #[test]
    fn a_task_with_no_enabled_element_is_on_and_an_explicit_false_is_off() {
        let mut silent = task_with(r"\WSA-Dashboard\N", "", r"C:\a.exe");
        silent.enabled = None;
        assert!(
            to_scheduler_task(&silent).enabled,
            "an absent <Enabled> is what schtasks writes for an enabled task"
        );

        let mut on = task_with(r"\WSA-Dashboard\N", "", r"C:\a.exe");
        on.enabled = Some(true);
        assert!(to_scheduler_task(&on).enabled);

        let mut off = task_with(r"\WSA-Dashboard\N", "", r"C:\a.exe");
        off.enabled = Some(false);
        assert!(!to_scheduler_task(&off).enabled);
    }

    #[test]
    fn a_task_with_no_author_says_so_rather_than_being_blank() {
        let mut task = task_with(r"\WSA-Dashboard\N", "", r"C:\a.exe");
        task.author = "   ".to_string();
        assert_eq!(to_scheduler_task(&task).author, NOT_REPORTED);
    }

    #[test]
    fn a_com_handler_task_becomes_a_row_with_no_command() {
        let task = TaskXml {
            path: r"\Microsoft\Windows\X".to_string(),
            action: ActionKind::ComHandler,
            ..Default::default()
        };
        let row = to_scheduler_task(&task);
        assert_eq!(row.command, "");
        assert_eq!(row.arguments, "");
        assert_eq!(row.cron_expr, "");
        // Not a claim about ComHandler tasks: `Default` leaves `enabled` as
        // `None`, and `None` is on. This one is here so that the four fields'
        // emptiness is asserted together rather than three of them.
        assert!(row.enabled);
    }

    // -- the real task store -------------------------------------------------

    /// One whole round trip against the real `schtasks`: create a task, read it
    /// back, switch it off and on, run it, delete it, and see it gone.
    ///
    /// Every other test in this file stops at the strings this program hands
    /// `schtasks`, and that is not the same claim. `build_create_args` can
    /// produce a perfect argument list for a switch Windows spells differently,
    /// and `cron_from_description` can round-trip a description this program
    /// wrote and never one `Set-ScheduledTask` accepted. The two claims that need
    /// a real machine are **the cron in the description** -- this program's own
    /// design, and the only place the schedule is stored -- and **the read-back
    /// through `xml.rs`**.
    ///
    /// `#[ignore]` because Q8 keeps machine-dependent work out of the suite, and
    /// because this one *writes*: it creates a task in the real store and takes
    /// it away again. The id is fixed and deleted before it is used, so a run
    /// that dies half-way leaves one task that the next run clears rather than a
    /// folder that grows.
    ///
    /// `false` at every `needs_elevation`, and that narrows what this proves.
    /// The elevated path is one `ShellExecuteExW` away and cannot be exercised
    /// without a UAC prompt, which no test may ask for -- and the two writes it
    /// owns cannot be reached without it either, both measured:
    ///
    ///   * `schtasks /Create … /RL HIGHEST` answers `ERROR: Access is denied.`
    ///     unelevated, so a task with the admin box ticked cannot be created from
    ///     here and this test's `admin_mode` stays `false`. That the *left* side
    ///     of that works is what `build_create_args`'s own tests pin.
    ///   * `Set-ScheduledTask` does **not** need elevation, which is why the cron
    ///     round trip below is real rather than theoretical -- and it is what the
    ///     description write on the unelevated path is measured against, since
    ///     the elevated path runs the same PowerShell through `cmd.exe` instead.
    ///
    /// What that leaves unproven is the prompt, the `cmd.exe` batch around these
    /// five calls, and `admin_mode`. What it proves is the other five things: the
    /// `/Create` argument list, the cron in the description, the read-back through
    /// `xml.rs`, `/Change /ENABLE|/DISABLE`, `/Run`, `/Delete`, and
    /// `<Enabled>`'s absent-means-on -- that last one being a bug this test found
    /// on its first run, when the page would have drawn every enabled task as
    /// switched off.
    /// The whole point of the page, end to end, and the only test that proves
    /// the chain rather than its pieces: **create a task, press Run Now, and
    /// watch the command do something.**
    ///
    /// What it exercises, in order: the `/Create` argument list, the VBS launcher
    /// this program writes (including the cold-start line, which really runs --
    /// `wsadash start` on a machine where the subsystem is wedged costs the
    /// launcher its timeout and then the command runs anyway, which is the
    /// best-effort rule under test), `schtasks /Run`, and the command itself.
    ///
    /// The command is `cmd.exe /c md "<dir>"` because it needs no redirection
    /// (`WScript.Shell.Run` does not go through a shell, so a `>` in the command
    /// line would be an argument rather than a redirect) and because a directory
    /// appearing is not something a wrong exit code can fake.
    ///
    /// `#[ignore]` because Q8 keeps machine-dependent work out of the suite, and
    /// because this one creates a real task and a real directory -- both removed
    /// at the end, and both removed at the start as well, so a run that dies
    /// half-way leaves one task and one directory that the next run clears.
    #[test]
    #[ignore = "writes to the real task store; Q8 keeps machine-dependent work out of the suite"]
    fn pressing_run_now_makes_the_command_run() {
        const ID: &str = "wsa-dashboard-runproof";
        let proof = std::env::temp_dir().join(format!("wsa-task-proof-{}", std::process::id()));

        let _ = delete_task(ID, false);
        let _ = std::fs::remove_dir_all(&proof);

        let params = CreateTaskParams {
            display_name: ID.to_string(),
            command: "cmd.exe".to_string(),
            arguments: format!(r#"/c md "{}""#, proof.display()),
            cron_expr: "0 3 * * *".to_string(),
            admin_mode: false,
        };
        create_task(&params, false, None).expect("the store accepted the task");

        // What the store holds: a launcher, not the reader's command. That is
        // the wrap, and asserting it here is what keeps someone from "fixing"
        // `/TR` back to the bare command line and quietly dropping the cold
        // start with it.
        let raw = std::process::Command::new("schtasks")
            .args([
                "/Query",
                "/TN",
                &task_path_for_id(ID),
                "/XML",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .expect("schtasks is on PATH");
        let raw = decode_output(&raw.stdout);
        assert!(
            raw.contains("wscript.exe"),
            "the task's /TR is not a launcher, so nothing starts the subsystem:\n{raw}"
        );

        // The reader's own words come back out of the launcher, exactly.
        let created = list_tasks()
            .expect("the store reads back")
            .into_iter()
            .find(|task| task.task_id == ID)
            .expect("the task is in the store");
        assert_eq!(created.command, "cmd.exe");
        assert_eq!(created.arguments, params.arguments);

        run_task(ID, false).expect("Run Now");

        // The launcher may first wait out the subsystem's cold start, and on a
        // machine where WSA is wedged that wait is the whole timeout. Polling
        // rather than sleeping once, so a fast machine is not punished for a
        // slow one.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
        let mut appeared = false;
        while std::time::Instant::now() < deadline {
            if proof.is_dir() {
                appeared = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        // Clean up before asserting, so a failure does not leave the proof on
        // disk for the next run to trip over.
        let _ = std::fs::remove_dir_all(&proof);
        delete_task(ID, false).expect("delete");

        assert!(
            appeared,
            "the task ran but {} never appeared: the command did not execute",
            proof.display()
        );
    }

    /// The real task store takes a task and gives it back, with the cron and the
    /// command both coming out of the task's own record of itself.
    #[test]
    #[ignore = "writes to the real task store; Q8 keeps machine-dependent work out of the suite"]
    fn the_real_task_store_takes_a_task_and_gives_it_back() {
        const ID: &str = "wsa-dashboard-selftest";

        fn find(id: &str) -> SchedulerTask {
            list_tasks()
                .expect("the store reads back")
                .into_iter()
                .find(|task| task.task_id == id)
                .unwrap_or_else(|| panic!("{id} is not in the store"))
        }

        // Idempotent: whatever an earlier run left behind goes first, and a
        // leftover is not an error.
        let _ = delete_task(ID, false);

        let params = CreateTaskParams {
            display_name: ID.to_string(),
            command: "cmd.exe".to_string(),
            // `run_task` below is a real execution, so this one exits at once.
            arguments: "/c exit 0".to_string(),
            cron_expr: "0 3 * * *".to_string(),
            admin_mode: false,
        };

        create_task(&params, false, None).expect("the store accepted the task");

        let created = find(ID);
        println!("created: {created:?}");
        assert_eq!(
            created.cron_expr, "0 3 * * *",
            "the cron went into the description and did not come back out"
        );
        assert!(created.enabled, "a task Windows just created is enabled");
        assert_eq!(created.command, "cmd.exe");
        assert_eq!(created.arguments, "/c exit 0");

        toggle_task(ID, false, false).expect("disable");
        assert!(!find(ID).enabled, "/Change /DISABLE did not take");

        toggle_task(ID, true, false).expect("enable");
        assert!(find(ID).enabled, "/Change /ENABLE did not take");

        run_task(ID, false).expect("/Run");

        delete_task(ID, false).expect("/Delete");
        assert!(
            !list_tasks()
                .expect("the store still reads")
                .iter()
                .any(|task| task.task_id == ID),
            "the task survived its own deletion"
        );
    }
}
