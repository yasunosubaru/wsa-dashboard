// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The update check: GitHub's own release feed, and nothing else.
//!
//! Three functions and a struct. The two decisions that can be *wrong without
//! failing* are pure and tested here ([`parse_release`], [`version_is_newer`]);
//! the third opens a socket and is deliberately thin, because everything a test
//! could say about it on this machine is about this machine's network.
//!
//! GitHub's API needs no key for a public repository, but it does require a
//! `User-Agent` and answers a missing one with `403`. That is the kind of thing
//! this file has to get right rather than interesting: the failure is reported
//! as a failure, which is the one thing the caller can act on.

use std::time::Duration;

/// How long the check may take before it is called a failure.
///
/// Long enough for a slow connection to answer, short enough that the reader is
/// not left looking at a toast that says "checking" for a minute. The check is
/// also on a thread, so this is the point at which the toast is replaced by the
/// reason it did not finish, not the point at which the window unfreezes.
const TIMEOUT: Duration = Duration::from_secs(15);

/// One published release, as much of it as this program shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// `tag_name` -- what GitHub calls the release, e.g. `v0.1.1`.
    pub tag: String,
    /// `html_url` -- where a reader can go to get it.
    pub url: String,
}

/// Read GitHub's release JSON.
///
/// The two fields are the ones the page needs; the other twenty in GitHub's
/// answer are ignored, and deliberately: a struct that named them all would be
/// a struct that breaks when GitHub adds a field this program does not use.
///
/// **A missing `tag_name` is an error rather than an empty release.** That is
/// the whole point of this function existing separately from the request:
/// GitHub answers a 404 for a private or renamed repository with
/// `{"message":"Not Found"}`, which is a perfectly valid JSON object with no
/// `tag_name` in it. Returned as an empty-tag `Release`, it would compare as
/// not-newer and the page would say "you are up to date" -- which is what it
/// says when it is right.
pub fn parse_release(json: &str) -> Result<Release, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|error| format!("not JSON: {error}"))?;

    if let Some(message) = value.get("message").and_then(|m| m.as_str()) {
        // GitHub's own sentence, whatever it is. Prefixed with who said it so
        // that a reader cannot mistake it for ours.
        return Err(format!("GitHub says: {message}"));
    }

    let tag = value
        .get("tag_name")
        .and_then(|tag| tag.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if tag.is_empty() {
        return Err("the release has no tag".to_string());
    }

    Ok(Release {
        tag,
        url: value
            .get("html_url")
            .and_then(|url| url.as_str())
            .unwrap_or_default()
            .trim()
            .to_string(),
    })
}

/// Is `candidate` a release later than `current`?
///
/// A dotted number comparison, one component at a time, and **not** a string
/// comparison: `"0.10.0" < "0.9.0"` as text and `>` as versions, and a build
/// that got that wrong would offer a downgrade to every reader on 0.10.
///
/// The three things this accepts beyond plain numbers, each because the strings
/// it is handed really do look like that:
///
///   * A leading `v` on either side (`v0.1.1`), which is how this repository
///     tags and how `APP_VERSION` is not written.
///   * Different component counts (`1.0` against `1.0.0`) -- equal, because
///     neither has anything to say the other has not.
///   * A component that is not a number stops the comparison there and the rest
///     is not looked at: `0.2.0-beta` is later than `0.1.9` and not later than
///     `0.2.0`, which is the answer a reader expects and the only one that does
///     not require this program to implement semver's pre-release rules.
///
/// An unparseable `candidate` is never newer, so a malformed tag from GitHub
/// cannot make the page announce a release that does not exist.
pub fn version_is_newer(current: &str, candidate: &str) -> bool {
    fn components(version: &str) -> Vec<u32> {
        version
            .trim()
            .trim_start_matches(['v', 'V'])
            .split('.')
            .map_while(|part| part.trim().parse::<u32>().ok())
            .collect()
    }

    let current = components(current);
    let candidate = components(candidate);
    if candidate.is_empty() {
        return false;
    }

    // Compare component by component; a missing component is zero, so `1.0` and
    // `1.0.0` are the same version.
    let length = current.len().max(candidate.len());
    for index in 0..length {
        let left = current.get(index).copied().unwrap_or(0);
        let right = candidate.get(index).copied().unwrap_or(0);
        if right != left {
            return right > left;
        }
    }
    false
}

/// The `latest` release of `repository`, where the repository is `owner/name`.
///
/// Takes the repository rather than reading `constants::PROJECT_REPOSITORY`
/// itself, and that is a mount requirement rather than a preference: this file
/// is compiled into both binaries, where the constant lives at
/// `crate::app::constants::PROJECT_REPOSITORY` in one and `crate::constants::`
/// in the other. The caller supplies it, and the caller is GUI-only.
///
/// Blocking. The caller is a thread of its own; see `handlers/about.rs`.
pub fn latest_for(repository: &str) -> Result<Release, String> {
    let repository = repository.trim();
    if repository.is_empty() {
        return Err("no repository is configured".to_string());
    }

    let url = format!("https://api.github.com/repos/{repository}/releases/latest");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .into();

    let mut response = agent
        .get(&url)
        .header("User-Agent", "wsa-dashboard")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| match error {
            ureq::Error::StatusCode(code) => format!("GitHub answered {code}"),
            other => other.to_string(),
        })?;

    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| format!("could not read the answer: {error}"))?;

    parse_release(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- reading GitHub's answer ---------------------------------------------

    #[test]
    fn a_release_is_its_tag_and_its_page() {
        let release = parse_release(
            r#"{"tag_name": "v0.1.1", "html_url": "https://github.com/a/b/releases/tag/v0.1.1",
                "name": "0.1.1", "body": "notes", "draft": false}"#,
        )
        .expect("a well formed release");
        assert_eq!(release.tag, "v0.1.1");
        assert_eq!(release.url, "https://github.com/a/b/releases/tag/v0.1.1");
    }

    /// GitHub's 404 body. Valid JSON, a `message`, no `tag_name` -- and read as
    /// a release it would be an empty tag, which compares as not-newer, which
    /// is what the page says when the build *is* current.
    #[test]
    fn the_not_found_body_is_an_error_and_not_an_empty_release() {
        let error = parse_release(r#"{"message":"Not Found","documentation_url":"..."}"#)
            .expect_err("GitHub's 404 body is not a release");
        assert!(error.contains("Not Found"), "got {error}");
    }

    #[test]
    fn a_release_without_a_tag_is_an_error() {
        for body in [
            r#"{"html_url":"https://example.invalid"}"#,
            r#"{"tag_name":""}"#,
            r#"{"tag_name":"   "}"#,
        ] {
            assert!(parse_release(body).is_err(), "{body}");
        }
    }

    #[test]
    fn a_body_that_is_not_json_says_so_rather_than_panicking() {
        // What a captive portal or a proxy answers with.
        let error = parse_release("<html>Sign in to the network</html>").unwrap_err();
        assert!(error.starts_with("not JSON"), "got {error}");
    }

    #[test]
    fn a_release_with_no_html_url_still_has_a_tag() {
        let release = parse_release(r#"{"tag_name":"v2.0.0"}"#).expect("a tag is enough");
        assert_eq!(release.tag, "v2.0.0");
        assert_eq!(release.url, "");
    }

    // -- comparing versions --------------------------------------------------

    #[test]
    fn a_later_number_is_a_later_version() {
        assert!(version_is_newer("0.1.0", "0.1.1"));
        assert!(version_is_newer("0.1.0", "0.2.0"));
        assert!(version_is_newer("0.1.0", "1.0.0"));
        assert!(!version_is_newer("0.1.1", "0.1.0"));
        assert!(!version_is_newer("0.1.0", "0.1.0"));
    }

    /// The one a string comparison gets backwards: `"0.10.0" < "0.9.0"` as
    /// text, and `0.10.0` is the later release.
    #[test]
    fn ten_is_later_than_nine_and_not_the_other_way_round() {
        assert!(version_is_newer("0.9.0", "0.10.0"));
        assert!(!version_is_newer("0.10.0", "0.9.0"));
    }

    #[test]
    fn a_leading_v_is_not_part_of_the_version() {
        assert!(version_is_newer("0.1.0", "v0.1.1"));
        assert!(version_is_newer("v0.1.0", "0.1.1"));
        assert!(!version_is_newer("v0.1.1", "v0.1.1"));
    }

    #[test]
    fn a_missing_component_counts_as_zero() {
        assert!(!version_is_newer("1.0.0", "1.0"));
        assert!(!version_is_newer("1.0", "1.0.0"));
        assert!(version_is_newer("1.0", "1.0.1"));
        assert!(version_is_newer("1", "1.1"));
    }

    /// A pre-release tag: later than the release before it, not later than the
    /// release it is a pre-release of. The component that is not a number stops
    /// the comparison, which gives both answers without implementing semver.
    #[test]
    fn a_tag_that_is_not_all_numbers_stops_at_the_last_number() {
        assert!(version_is_newer("0.1.9", "0.2.0-beta"));
        assert!(!version_is_newer("0.2.0", "0.2.0-beta"));
        assert!(!version_is_newer("0.2.0", "0.2.0+build7"));
    }

    /// The direction that matters for safety: a tag this program cannot read is
    /// never announced as a new release.
    #[test]
    fn an_unreadable_tag_is_never_newer() {
        for candidate in ["", "   ", "nightly", "latest", "v", "release-2"] {
            assert!(
                !version_is_newer("0.1.0", candidate),
                "{candidate:?} was read as a newer release"
            );
        }
    }

    /// And the other direction: an unreadable *current* version makes everything
    /// readable newer, which is the harmless way round for a build whose version
    /// string got mangled.
    #[test]
    fn an_unreadable_current_version_makes_a_readable_release_newer() {
        assert!(version_is_newer("", "0.1.0"));
        assert!(version_is_newer("dev", "0.1.0"));
        assert!(!version_is_newer("dev", "also-dev"));
    }

    // -- the request itself --------------------------------------------------

    /// The empty repository never reaches the network. Asserted because the
    /// alternative -- an `agent.get("https://api.github.com/repos//releases/
    /// latest")` -- answers `404 Not Found` after a round trip, and a build with
    /// no repository configured must not depend on the network to say so.
    #[test]
    fn a_blank_repository_is_refused_before_any_socket_is_opened() {
        let error = latest_for("").expect_err("no repository");
        assert!(error.contains("no repository"), "got {error}");
        let error = latest_for("   ").expect_err("no repository");
        assert!(error.contains("no repository"), "got {error}");
    }

    /// The request half, against the real API.
    ///
    /// The pure tests above pin what this program does with an answer; this is
    /// the one that says an answer arrives at all. Three things about the
    /// request itself cannot be checked without a socket, and all three are
    /// silent failures when wrong:
    ///
    ///   * **The `User-Agent`.** GitHub answers a request without one with `403`,
    ///     and a `403` from a proxy is indistinguishable from a `403` from
    ///     GitHub. If this header ever comes out, this test fails first.
    ///   * **The URL's shape.** `repos/<owner>/<name>/releases/latest` is
    ///     `releases` and not `releases/latest`... in the other order? The 404
    ///     for a wrong path is the same 404 as a wrong repository.
    ///   * **The JSON.** `parse_release` is tested against strings written here;
    ///     this is the only place a *live* body goes through it.
    ///
    /// `rust-lang/rust` rather than this project's own repository, which has no
    /// remote (Q2) and would make this test pass or fail on whether one was
    /// added. It is a repository with releases that will not be deleted.
    ///
    /// `#[ignore]` for Q8's reason twice over: it needs the network, and its
    /// answer changes when someone else publishes.
    #[test]
    #[ignore = "needs api.github.com; Q8 keeps machine-dependent work out of the suite"]
    fn the_real_github_api_answers_a_latest_release() {
        let release = match latest_for("rust-lang/rust") {
            Ok(release) => release,
            // A machine with no route to GitHub is not a defect in this file,
            // and the difference is reported rather than swallowed: the same
            // courtesy `probe_against_the_live_subsystem` extends when WSA is
            // not installed (MAPPING 11.8).
            Err(error) => {
                eprintln!("skipping: api.github.com did not answer ({error})");
                return;
            }
        };
        println!("latest rust-lang/rust: tag={:?} url={:?}", release.tag, release.url);

        assert!(!release.tag.is_empty(), "the tag came back empty");
        assert!(
            release.url.starts_with("https://github.com/rust-lang/rust/releases/tag/"),
            "the html_url is not a release page: {:?}",
            release.url
        );
        // And the comparison this program exists to make, on a real version
        // string: an ancient build is behind it, and it is not behind itself.
        assert!(version_is_newer("1.0.0", &release.tag));
        assert!(!version_is_newer(&release.tag, &release.tag));
    }
}
