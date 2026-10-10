// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Dynamic resolution of the adb port WSA listens on.
//!
//! The port is declared by WSA's own `AppxManifest.xml` as a firewall rule:
//!
//! ```xml
//! <desktop2:Rule Direction="in" IPProtocol="TCP"
//!                LocalPortMin="58526" LocalPortMax="58526" Profile="private" />
//! ```
//!
//! The attribute is `LocalPortMin` / `LocalPortMax` -- there is no `LocalPort`
//! attribute. Verified against the installed package on 2026-10-10
//! (AppxManifest.xml line 211).
//!
//! String scanning is deliberate: the manifest is a WSA-produced artifact with
//! a stable shape, and pulling in an XML dependency to read one number is not
//! worth it.
//!
//! TDD step 1 (RED): `parse_local_port` is not implemented yet.

#![allow(dead_code)]

/// Port WSA's adb listener is known to use when the manifest cannot be read.
///
/// This is only ever applied by the *caller*: `parse_local_port` returns
/// `None` on failure so that the fallback decision stays testable.
pub const FALLBACK_PORT: u16 = 58526;

/// Read the adb listener port out of an `AppxManifest.xml`.
///
/// Returns `None` when the attribute is absent, unparseable, or when the
/// manifest declares a *range* (`LocalPortMin` != `LocalPortMax`) -- a range
/// is not one port, and guessing the low end would silently connect somewhere
/// else. WSA has always declared a single port.
pub fn parse_local_port(manifest_xml: &str) -> Option<u16> {
    let min = parse_port_attr(manifest_xml, "LocalPortMin")?;
    match parse_port_attr(manifest_xml, "LocalPortMax") {
        Some(max) if max != min => None,
        _ => Some(min),
    }
}

/// Pull `<name>="<value>"` and parse the value as `u16`.
fn parse_port_attr(xml: &str, name: &str) -> Option<u16> {
    let needle = format!("{name}=\"");
    let start = xml.find(&needle)? + needle.len();
    let rest = &xml[start..];
    let end = rest.find('"')?;
    rest[..end].trim().parse::<u16>().ok()
}

#[cfg(test)]
mod tests {
    use super::{parse_local_port, FALLBACK_PORT};

    /// The real manifest, trimmed. Note the attribute is `LocalPortMin`,
    /// not `LocalPort`.
    const MANIFEST: &str = include_str!("../../tests/fixtures/AppxManifest.xml");

    #[test]
    fn reads_the_real_port_from_the_manifest() {
        assert_eq!(parse_local_port(MANIFEST), Some(58526));
    }

    /// The parser must not assume 58526; it reads whatever is declared.
    /// Both bounds move together in a real manifest, so the fixture rewrite
    /// changes both -- changing only `LocalPortMin` would fabricate a range.
    #[test]
    fn reads_a_non_default_port() {
        let moved = MANIFEST
            .replace("LocalPortMin=\"58526\"", "LocalPortMin=\"60000\"")
            .replace("LocalPortMax=\"58526\"", "LocalPortMax=\"60000\"");
        assert_eq!(parse_local_port(&moved), Some(60000));
    }

    /// Returns None rather than the fallback: the caller decides whether to
    /// fall back, which is what makes the fallback itself testable.
    #[test]
    fn missing_port_attribute_is_none_not_fallback() {
        let stripped = MANIFEST
            .replace("LocalPortMin=\"58526\" ", "")
            .replace("LocalPortMax=\"58526\" ", "");
        assert_eq!(parse_local_port(&stripped), None);
    }

    /// Guards the fallback constant itself so a silent change is caught.
    #[test]
    fn fallback_port_is_58526() {
        assert_eq!(FALLBACK_PORT, 58526);
    }

    /// A declared range is not a single port; refuse rather than guess.
    #[test]
    fn port_range_is_rejected() {
        let ranged = MANIFEST.replace("LocalPortMax=\"58526\"", "LocalPortMax=\"58530\"");
        assert_eq!(parse_local_port(&ranged), None);
    }

    /// A non-numeric value must not panic.
    #[test]
    fn non_numeric_port_is_none() {
        let junk = MANIFEST.replace("LocalPortMin=\"58526\"", "LocalPortMin=\"abc\"");
        assert_eq!(parse_local_port(&junk), None);
    }
}
