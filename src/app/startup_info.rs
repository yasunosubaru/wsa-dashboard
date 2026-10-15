// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// `log_startup_system_info` is called by `runner.rs` in the GUI build. Under the
// CLI's `#[cfg(test)] #[path]` mount of this module only the tests are compiled
// against it -- the two pure parsers they cover -- so the spawn is unreachable by
// construction from there, not dead. Same reason and same shape as
// `scheduler_task/models.rs` and `config/mod.rs`.
#![allow(dead_code)]

//! One log line at startup, and no other effect.
//!
//! The counterpart of the reference's `src/app/startup_info.rs`, which runs
//! `wsl --version` and pulls a subsystem version and a Windows build number out
//! of its output. That same parse is still here and is still the interesting part,
//! because `wsl --version` prints its labels in the display language and the
//! reference's answer to that is the right one: look for a *token shaped like a
//! version* rather than for the word "version".
//!
//! # What we log instead
//!
//! We have no `wsl.exe` and no reason to run one. The three facts available at
//! startup are this build's version, the Windows build, and whether WSA is
//! installed -- and the third is the one that decides why every later line in the
//! log says what it says. So it gets a line.
//!
//! [`parse_version_token`] and [`parse_windows_build`] are the two pure helpers,
//! and they are what the tests cover. Everything else here is a process spawn.

use tracing::info;

/// The first token in `line` that looks like `1.2` or `1.2.3.4`.
///
/// Locale-proof by construction: it does not know any label. `WSL 版本: 2.4.8.0`
/// and `WSL version: 2.4.8.0` yield the same answer, because what is being looked
/// for is a shape rather than a word.
///
/// Returns `None` when nothing in the line has that shape -- which is how a line
/// of prose says "no version here" without a language table.
pub fn parse_version_token(line: &str) -> Option<String> {
    line.split(|c: char| !c.is_ascii_digit() && c != '.')
        .find(|token| {
            !token.is_empty()
                && token.contains('.')
                && token.chars().all(|c| c.is_ascii_digit() || c == '.')
                && !token.starts_with('.')
                && !token.ends_with('.')
        })
        .map(|version| version.to_string())
}

/// The Windows build, from `ver`'s output.
///
/// # This is not the reference's parser, and the reference's would fail here
///
/// The reference splits on the literal `"Version "` in `wsl --version`'s output.
/// That string is localised. Measured on this machine: `cmd /c ver` answers
/// `Microsoft Windows [版本 10.0.26300.9457]`, so a `split_once("Version ")`
/// finds nothing and the line says `N/A` on every Chinese, Japanese, Korean,
/// German and Russian Windows there is -- silently, because `N/A` is also what a
/// genuinely answerable question looks like when the command is missing.
///
/// What is *not* localised is the bracketed number: Windows formats it itself and
/// prints the same digits everywhere. So this finds the last bracketed run of
/// text and takes the version-shaped token out of it, which is
/// [`parse_version_token`]'s job and the reason that function exists.
pub fn parse_windows_build(output: &str) -> String {
    output
        .lines()
        .filter_map(|line| {
            // The last `[` that has a `]` after it: `ver` prints one bracketed
            // run, and a machine with a second one in the output is not a machine
            // running `ver`.
            let open = line.rfind('[')?;
            let close = line[open..].find(']')?;
            let inside = &line[open + 1..open + close];
            // The token, not the whole bracketed run: the label is still in there
            // in the reader's language and the digits are what we came for.
            parse_version_token(inside)
        })
        .next_back()
        .unwrap_or_else(not_reported)
}

/// When Windows would not say.
fn not_reported() -> String {
    NOT_REPORTED.to_string()
}

/// The value [`not_reported`] hands back, exported so a test can assert against
/// it rather than against a second copy of the string.
pub const NOT_REPORTED: &str = "N/A";

/// Log the startup line.
///
/// `async` and spawned from the launcher, so a slow `ver` never holds up the
/// window. The reference's is `async` for the same reason and with the same
/// consequence: if it is ever made to block, the window appears late on a machine
/// whose `cmd.exe` is slow to start.
pub async fn log_startup_system_info() {
    use tokio::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let app_version = env!("CARGO_PKG_VERSION");
    let windows_version = {
        let mut cmd = Command::new("cmd.exe");
        cmd.arg("/c")
            .arg("ver")
            .creation_flags(CREATE_NO_WINDOW)
            .kill_on_drop(true);
        match cmd.output().await {
            Ok(output) if output.status.success() => {
                parse_windows_build(&String::from_utf8_lossy(&output.stdout))
            }
            _ => NOT_REPORTED.to_string(),
        }
    };
    // Whether WSA is registered. Not a version -- the package registration does
    // not always carry one in a place `wsa_path` reads -- because the fact that
    // decides it, the one that explains every "no such device" line later in the
    // log, is presence.
    let wsa = crate::wsa::app::wsa_path::resolve();
    let wsa_state = match &wsa {
        Ok(paths) => format!("{}", paths.install_location.display()),
        Err(_) => "not installed".to_string(),
    };
    info!(
        "[STARTUP] app v{app_version} | Windows {windows_version} | WSA at {wsa_state}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reason this parser is written the way it is: the same output, two
    /// languages, one answer.
    #[test]
    fn a_version_token_is_found_from_a_label_in_any_language() {
        for line in [
            "WSL version: 2.4.8.0",
            "WSL 版本: 2.4.8.0",
            "버전 2.4.8.0",
            "  2.4.8.0  ",
            "wsl 2.4.8.0",
        ] {
            assert_eq!(
                parse_version_token(line).as_deref(),
                Some("2.4.8.0"),
                "line was {line:?}"
            );
        }
    }

    /// A dotted number followed by a suffix. The first assertion is the reference's
    /// own line from `wsl --version`; the second is the same shape with a suffix
    /// attached, which is what a kernel version looks like.
    ///
    /// The second one was written expecting `5.15.167.4-1`, and it returns
    /// `5.15.167.4`. That is the better answer and the test now says so: the
    /// tokenizer splits on every non-digit that is not a dot, so the `-` ends the
    /// token, and what is left is the version rather than the version plus a suffix
    /// this program has no use for. An `.or(Some(...))` on the assertion was the
    /// tell that the expectation had not been decided -- it accepted either.
    #[test]
    fn the_only_dotted_number_in_a_line_is_the_answer() {
        assert_eq!(
            parse_version_token("WSLG version: 2.0.0").as_deref(),
            Some("2.0.0")
        );
        assert_eq!(
            parse_version_token("Kernel version: 5.15.167.4-1").as_deref(),
            Some("5.15.167.4"),
            "the suffix after the version is not part of the version"
        );
        assert_eq!(
            parse_version_token("1.2.3.4.5").as_deref(),
            Some("1.2.3.4.5"),
            "five dotted segments is still one version"
        );
    }

    #[test]
    fn a_line_with_no_version_yields_nothing_rather_than_a_guess() {
        for line in ["", "no numbers here", "2026", "just a dot.", ".x.", ".."] {
            assert_eq!(
                parse_version_token(line),
                None,
                "line was {line:?}"
            );
        }
    }

    /// `ver`'s real output, in English and in Chinese, is the same bracketed
    /// number -- so this is a case where the *value* is stable even though no
    /// label is.
    #[test]
    fn the_windows_build_comes_from_the_bracketed_number_not_the_label() {
        assert_eq!(
            parse_windows_build("\r\nMicrosoft Windows [Version 10.0.26100.2605]\r\n\r\n"),
            "10.0.26100.2605"
        );
        assert_eq!(
            parse_windows_build("Microsoft Windows [版本 10.0.26100.2605]"),
            "10.0.26100.2605"
        );
        assert_eq!(parse_windows_build("garbage"), NOT_REPORTED);
        assert_eq!(parse_windows_build(""), NOT_REPORTED);
    }

    /// The parser's own claim, checked against the machine it runs on. `ver`'s
    /// label is localised and the number is not; this is the one place a fixture
    /// cannot prove it, because the fixture is written in whatever language the
    /// author thought of.
    ///
    /// `#[ignore]` because Q8 keeps machine-dependent work out of the suite, and
    /// `-- --ignored --nocapture` is how it runs.
    #[test]
    #[ignore = "spawns cmd /c ver; Q8 keeps machine-dependent work out of the suite"]
    fn the_real_ver_output_on_this_machine() {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let output = std::process::Command::new("cmd.exe")
            .args(["/c", "ver"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .expect("cmd is on PATH");
        let text = String::from_utf8_lossy(&output.stdout);
        println!("ver says: {}", text.trim());
        let build = parse_windows_build(&text);
        println!("parsed build: {build}");
        assert_ne!(build, NOT_REPORTED, "the real ver output did not parse");
        assert!(
            parse_version_token(&build).is_some(),
            "{build} does not look like a build number"
        );
    }
}
