// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The network page's view model: adb's two listings in, plain rows out.
//!
//! The counterpart of the reference's rules list handling, and the same shape
//! as `disk_model` -- deliberately free of the `PortProxyRuleUI` struct from
//! `theme.slint`, because that type exists only in the GUI binary and naming it
//! here would make the module unmountable and its tests would never compile.
//! `ui/handlers/network.rs` does the conversion, exactly as it does for the
//! storage page.
//!
//! The one decision this file exists to hold is **what a rule's `id` is**.
//!
//! The page passes `rule.id` straight back into `delete_rule`, so the id has to
//! carry everything a delete needs -- which direction, and which port -- because
//! the callback's signature carries nothing else (it is the reference's, and
//! the reference's id was a netsh key holding the same two facts). Encoding it
//! as `forward:<port>` / `reverse:<port>` keeps the field opaque to Slint and
//! makes it parseable here, in a module whose tests actually run.

use crate::wsa::dashboard::ops::PortMapping;

/// Which of adb's two verbs a rule came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// `adb forward tcp:<local> tcp:<remote>` -- host listens, device is dialled.
    Forward,
    /// `adb reverse tcp:<device> tcp:<host>` -- device listens, host is dialled.
    Reverse,
}

impl Direction {
    /// The leading token of a rule id, and the token `delete_network_rule`
    /// looks for.
    pub fn token(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Reverse => "reverse",
        }
    }
}

/// One row for the cards. Fields are the `PortProxyRuleUI` ones, minus the two
/// that are `string` in Slint and `&'static str` here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleRow {
    /// `"<token>:<port>"` -- see the module docs.
    pub id: String,
    /// The i18n key for the direction line, not the sentence: the card takes a
    /// key through `AppI18n`, and a pre-translated string here would mean this
    /// module had to know which language the window is showing.
    pub direction_key: &'static str,
    pub listen_address: String,
    pub listen_port: u16,
    pub target_port: u16,
    pub enable_firewall: bool,
    pub is_active: bool,
}

/// Turn adb's two listings into rows, forwards first.
///
/// Order is the listings', not sorted: a reader who comes back to this page
/// sees the same rows in the same order as last time, and a list that reshuffles
/// itself between refreshes is a list where the delete you aimed at moved.
pub fn rows(forward: &[PortMapping], reverse: &[PortMapping]) -> Vec<RuleRow> {
    forward
        .iter()
        .map(|mapping| build(Direction::Forward, mapping))
        .chain(reverse.iter().map(|mapping| build(Direction::Reverse, mapping)))
        .collect()
}

fn build(direction: Direction, mapping: &PortMapping) -> RuleRow {
    RuleRow {
        id: format!("{}:{}", direction.token(), mapping.local),
        direction_key: match direction {
            Direction::Forward => "network.dir_forward",
            Direction::Reverse => "network.dir_reverse",
        },
        listen_address: listen_address(&mapping.serial).to_owned(),
        listen_port: mapping.local,
        target_port: mapping.remote,
        // Never `false` by choice: there is no staged state to be out of, so
        // every row is applied. Whether a *firewall* rule exists for it is a
        // separate question and is `false` only because nothing has asked yet
        // -- see `handlers/network.rs`.
        enable_firewall: false,
        is_active: true,
    }
}

/// Split a rule id back into the direction and the port a delete needs.
///
/// Returns `None` rather than a default for every input it does not
/// recognise, and `None` specifically rather than `(Forward, 0)`: `adb forward
/// --remove tcp:0` is a command that addresses nothing, and the id came from
/// the window, so it may be stale.
pub fn parse_id(id: &str) -> Option<(Direction, u16)> {
    let (token, port) = id.split_once(':')?;
    let direction = match token {
        "forward" => Direction::Forward,
        "reverse" => Direction::Reverse,
        _ => return None,
    };
    // `port.parse::<u16>()` alone would accept `"0"`, and 0 is not a rule.
    let port: u16 = port.parse().ok()?;
    if port == 0 {
        return None;
    }
    Some((direction, port))
}

/// The host part of a serial, for the card's `listen_address`.
///
/// `adb forward --list` reports `127.0.0.1:58526` (the adb endpoint) and
/// `adb reverse --list` reports `host-16` (a device name with no port in it).
/// The card renders `listen_address + ":" + listen_port`, so a serial kept whole
/// would read `127.0.0.1:58526:18081`.
///
/// Split on the **first** colon and keep the left side; when there is no colon
/// the whole thing is the host and it is returned unchanged. `:18081` returns
/// `18081`, not an empty address -- an empty one renders as a bare colon in
/// front of the port.
pub fn listen_address(serial: &str) -> &str {
    match serial.split_once(':') {
        Some((host, rest)) if host.is_empty() => rest,
        Some((host, _)) => host,
        None => serial,
    }
}

/// The placeholder text the proxy form shows when a field is empty.
///
/// The reference's own values from `src/network/models.rs` line 51-61, kept
/// verbatim: an `example.com` host and an `10808` port are correct for an
/// Android proxy too, so there is nothing for Q12's "change the value where the
/// domain differs" rule to change. What *did* change there is the key
/// `network.proxy_error_port`, whose "1 and 65534" the code now contradicts
/// deliberately (MAPPING 14).
pub fn default_no_proxy() -> String {
    "localhost,127.0.0.1,.example.com".to_string()
}

pub fn default_host() -> String {
    "e.g. 192.168.1.10".to_string()
}

pub fn default_port() -> String {
    "e.g. 10808".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(serial: &str, local: u16, remote: u16) -> PortMapping {
        PortMapping {
            serial: serial.to_owned(),
            local,
            remote,
        }
    }

    // -- the id -------------------------------------------------------------
    //
    // `delete_network_rule` gets this string back from the card and nothing
    // else, so a direction or a port that does not survive the round trip turns
    // a delete into a command that does not address any rule. `adb forward
    // --remove tcp:0` is not an error, it is a no-op -- which is how a broken id
    // would present: the toast says it worked.

    #[test]
    fn an_id_carries_the_direction_and_the_port_a_delete_needs() {
        assert_eq!(
            parse_id("forward:18081"),
            Some((Direction::Forward, 18081))
        );
        assert_eq!(parse_id("reverse:18080"), Some((Direction::Reverse, 18080)));
    }

    /// Anything else is `None`, never a `(Forward, 0)`. A zero that came from a
    /// failed parse would be handed to `adb forward --remove tcp:0`, which
    /// answers "cannot remove a non-existent forward" or says nothing at all --
    /// and the id arrived from the window, so it is whatever a stale row put
    /// there.
    #[test]
    fn a_malformed_id_is_rejected_rather_than_guessed() {
        assert_eq!(parse_id(""), None);
        assert_eq!(parse_id("forward"), None);
        assert_eq!(parse_id("forward:"), None);
        assert_eq!(parse_id("forward:abc"), None);
        assert_eq!(parse_id("forward:0"), None, "port 0 is not a rule");
        assert_eq!(parse_id("forward:70000"), None, "above u16");
        assert_eq!(parse_id("18081-18081"), None, "the netsh key, not ours");
        assert_eq!(parse_id("inet:18081"), None, "a direction we do not have");
    }

    #[test]
    fn the_direction_token_round_trips_through_the_id() {
        for direction in [Direction::Forward, Direction::Reverse] {
            let id = format!("{}:{}", direction.token(), 18081);
            assert_eq!(parse_id(&id), Some((direction, 18081)), "{id}");
        }
    }

    // -- the serial ---------------------------------------------------------

    /// `adb forward --list` puts `127.0.0.1:58526` there and `adb reverse
    /// --list` puts `host-16`. The card renders `listen_address:listen_port`, so
    /// a serial kept whole would read `127.0.0.1:58526:18081`.
    #[test]
    fn the_serial_keeps_its_host_part_only() {
        assert_eq!(listen_address("127.0.0.1:58526"), "127.0.0.1");
        assert_eq!(listen_address("host-16"), "host-16");
        assert_eq!(listen_address("localhost"), "localhost");
        assert_eq!(listen_address(""), "");
    }

    /// A serial with a port in it but no host before it still yields something
    /// rather than an empty address -- an empty address renders as a leading
    /// colon in front of the port.
    #[test]
    fn a_bare_port_serial_keeps_the_port_rather_than_becoming_empty() {
        assert_eq!(listen_address(":18081"), "18081");
    }

    // -- the rows -----------------------------------------------------------

    #[test]
    fn both_listings_become_rows_with_their_own_direction() {
        let rows = rows(
            &[mapping("127.0.0.1:58526", 18081, 8081)],
            &[mapping("host-16", 18080, 18080)],
        );
        assert_eq!(rows.len(), 2, "{rows:?}");

        assert_eq!(rows[0].id, "forward:18081");
        assert_eq!(rows[0].direction_key, "network.dir_forward");
        assert_eq!(rows[0].listen_address, "127.0.0.1");
        assert_eq!(rows[0].listen_port, 18081);
        assert_eq!(rows[0].target_port, 8081);

        assert_eq!(rows[1].id, "reverse:18080");
        assert_eq!(rows[1].direction_key, "network.dir_reverse");
        assert_eq!(rows[1].listen_address, "host-16");
        assert_eq!(rows[1].listen_port, 18080);
        assert_eq!(rows[1].target_port, 18080);
    }

    /// No staging means no inactive row, and `is_active` is what the card's
    /// green dot reads. A false here would render a grey dot against a rule that
    /// *is* in adb's table -- the state the reference only shows for a rule it
    /// has not applied yet.
    #[test]
    fn a_rule_read_out_of_adb_is_always_active() {
        for row in rows(
            &[mapping("127.0.0.1:58526", 1, 2)],
            &[mapping("host-16", 3, 4)],
        ) {
            assert!(row.is_active, "{row:?}");
        }
    }

    /// Two rules on the same port but opposite directions must not collide: the
    /// id is what a delete addresses, and `forward:18081` and `reverse:18081`
    /// are different rules.
    #[test]
    fn opposite_directions_on_one_port_get_different_ids() {
        let rows = rows(
            &[mapping("127.0.0.1:58526", 18081, 8081)],
            &[mapping("host-16", 18081, 8081)],
        );
        let ids: Vec<_> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, ["forward:18081", "reverse:18081"], "{ids:?}");
    }

    /// An empty listing is an empty page, not an error -- the same rule the
    /// parsers already hold.
    #[test]
    fn nothing_forwarded_is_no_rows() {
        assert!(rows(&[], &[]).is_empty());
    }

    /// The whole point of encoding: every row's id parses back to the direction
    /// and port it was built from.
    #[test]
    fn every_row_id_parses_back_to_its_own_rule() {
        let built = rows(
            &[mapping("127.0.0.1:58526", 18081, 8081), mapping("127.0.0.1:58526", 18082, 8082)],
            &[mapping("host-16", 18080, 18080)],
        );
        for row in built {
            let (direction, port) = parse_id(&row.id).expect("row id must parse");
            let source = match direction {
                Direction::Forward => 18081..=18082,
                Direction::Reverse => 18080..=18080,
            };
            assert!(source.contains(&port), "{} outside its own listing", row.id);
            assert_eq!(direction.token().len() + 1 + port.to_string().len(), row.id.len());
        }
    }

    /// The three placeholders, and why they are identical to the reference's:
    /// they are example *addresses*, and an example address does not become
    /// wrong on another OS. The point of pinning them is the second half --
    /// `default_no_proxy` is also what a page falls back to when Android
    /// reports no exclusion list, so a change here silently changes what an
    /// unconfigured page shows.
    #[test]
    fn the_placeholders_are_the_reference_ones() {
        assert_eq!(default_host(), "e.g. 192.168.1.10");
        assert_eq!(default_port(), "e.g. 10808");
        assert_eq!(default_no_proxy(), "localhost,127.0.0.1,.example.com");
        // None of them is a plausible value: a placeholder that could be
        // mistaken for a configured proxy is one that gets saved.
        assert!(default_host().starts_with("e.g."));
        assert!(default_port().starts_with("e.g."));
    }
}
