// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! RAII registration of the subsystem's current operation.
//!
//! Corresponds to `src/wsl/dashboard/operation_guard.rs` in the reference,
//! whose `DistroOpGuard` registers on `create` and unregisters on `Drop`.
//! Spec 2.2 renames its operation set and adds the rule that matters:
//! **an already-registered slot rejects the next request**, surfacing as
//! `toast.distro_busy` (docs/MAPPING.md 3 / Q12).
//!
//! Three deliberate differences from the reference, all consequences of there
//! being one subsystem here rather than a map of distros:
//!
//!   * The reference keys `active_ops` by distro name and inserts
//!     unconditionally, so the first guard to drop clears whatever label the
//!     second one set -- last-out-wins, with no way to tell. A single
//!     `Option<&'static str>` has no ordering to get wrong, and `create`
//!     refuses rather than overwrites.
//!   * The reference's `unregister_operation` is `async`, so its `Drop` has
//!     to spawn a task and logs an error when no runtime is available --
//!     meaning the slot can be left registered forever with nothing to notice.
//!     Registration here is a `std::sync::Mutex`, so `Drop` is synchronous,
//!     infallible, and needs no runtime. The slot therefore cannot leak, which
//!     is the one failure mode that would brick every later operation.
//!   * The reference registers whatever `String` it is handed. The five names
//!     are constants here because the UI matches on them to pick the toast.
//!
//! TDD step 1 (RED): nothing below exists yet.

use super::WsaDashboard;

/// Operation names (spec 2.2), replacing the reference's
/// `Starting/Stopping/Restarting/Deleting/Exporting/Importing/Moving`.
///
/// Constants rather than literals at the call site: the UI matches on these
/// to choose `toast.distro_busy`, so a typo would otherwise only ever show up
/// as an untranslated string at runtime.
pub const STARTING: &str = "Starting";
pub const STOPPING: &str = "Stopping";
pub const LAUNCHING: &str = "Launching";
pub const INSTALLING: &str = "Installing";
pub const UNINSTALLING: &str = "Uninstalling";

/// Every name [`WsaOpGuard::create`] accepts.
pub const OPS: &[&str] = &[STARTING, STOPPING, LAUNCHING, INSTALLING, UNINSTALLING];

/// A second operation was requested while one is already running.
///
/// Carries the operation that got there first so the UI can say *which* one
/// is in the way rather than a bare "busy".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpBusy {
    pub current: &'static str,
}

impl std::fmt::Display for OpBusy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "another operation is already running: {}", self.current)
    }
}

impl std::error::Error for OpBusy {}

/// Holds the single operation slot for as long as it is in scope.
///
/// Every mutating operation starts with one and never releases it by hand:
/// `?`, an early `return`, a panic, or falling off the end all run `Drop`.
#[derive(Debug)]
pub struct WsaOpGuard<'a> {
    dashboard: &'a WsaDashboard,
    /// Which operation this guard registered. `Drop` only clears a slot that
    /// still holds it -- `create` makes a second guard impossible today, but
    /// an unconditional clear would become a real bug the moment that is
    /// relaxed (per-package operations, say).
    op: &'static str,
}

impl<'a> WsaOpGuard<'a> {
    /// Take the operation slot, or report who already holds it.
    ///
    /// Synchronous, unlike the reference's `async create`: there is no
    /// `await` between deciding and storing, so the check and the write are
    /// one atomic step under the mutex. An `async` version would invite a
    /// second task to slip in between them.
    pub fn create(dashboard: &'a WsaDashboard, op: &'static str) -> Result<Self, OpBusy> {
        let mut active = dashboard.active_op_lock();
        match *active {
            Some(current) => Err(OpBusy { current }),
            None => {
                *active = Some(op);
                Ok(Self { dashboard, op })
            }
        }
    }
}

impl Drop for WsaOpGuard<'_> {
    fn drop(&mut self) {
        let mut active = self.dashboard.active_op_lock();
        if *active == Some(self.op) {
            *active = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wsa::app::wsa_path::WsaPaths;
    use crate::wsa::dashboard::dashboard_for;
    use std::path::PathBuf;

    fn dashboard() -> WsaDashboard {
        dashboard_for(
            r"C:\no\such\platform-tools\adb.exe",
            WsaPaths {
                install_location: PathBuf::from(r"C:\WSA"),
                manifest_path: PathBuf::from(r"C:\WSA\AppxManifest.xml"),
                client_exe: PathBuf::from(r"C:\WSA\WsaClient\WsaClient.exe"),
                settings_aumid:
                    "MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe!SettingsApp"
                        .into(),
                adb_port: 58526,
            },
        )
    }

    // The dashboard built here is never asked to run anything: the guard only
    // touches the slot, so the adb path above does not exist and never needs
    // to.

    #[test]
    fn create_registers_the_operation_it_was_given() {
        let dash = dashboard();
        assert_eq!(dash.get_active_op(), None);

        let guard = WsaOpGuard::create(&dash, LAUNCHING).expect("free slot");

        assert_eq!(dash.get_active_op(), Some(LAUNCHING));
        drop(guard);
    }

    #[test]
    fn drop_clears_the_slot() {
        let dash = dashboard();
        let guard = WsaOpGuard::create(&dash, STARTING).expect("free slot");
        assert_eq!(dash.get_active_op(), Some(STARTING));

        drop(guard);

        assert_eq!(dash.get_active_op(), None);
    }

    /// Spec 2.2: already registered means the next request is refused, and
    /// the first one keeps its label. The reference overwrites instead, which
    /// would let the first guard's `Drop` erase the second's label.
    #[test]
    fn a_second_operation_is_refused_without_overwriting_the_first() {
        let dash = dashboard();
        let guard = WsaOpGuard::create(&dash, INSTALLING).expect("free slot");

        let err = WsaOpGuard::create(&dash, LAUNCHING).expect_err("slot is taken");

        assert_eq!(err, OpBusy {
            current: INSTALLING
        });
        assert_eq!(dash.get_active_op(), Some(INSTALLING), "must not be clobbered");

        // And the refuser did not take the slot either: once the real holder
        // leaves, the slot is free, not left stuck on LAUNCHING.
        drop(guard);
        assert_eq!(dash.get_active_op(), None);
    }

    /// The failure mode that would matter most: a guard that never releases
    /// freezes every later operation behind an error nothing can clear.
    #[test]
    fn the_slot_is_released_even_when_the_operation_bails_out_early() {
        fn run(dash: &WsaDashboard) -> Result<(), OpBusy> {
            let _guard = WsaOpGuard::create(dash, UNINSTALLING)?;
            // Stands in for the `?` an operation hits when adb fails.
            return Err(OpBusy {
                current: UNINSTALLING,
            });
        }

        let dash = dashboard();
        let err = run(&dash).expect_err("the stand-in operation fails");
        assert_eq!(err.current, UNINSTALLING);
        assert_eq!(dash.get_active_op(), None, "Drop ran on the way out");
    }

    /// The UI picks `toast.distro_busy` from this set (spec 2.2 / Q12), so
    /// the names in the spec are the names in the code, byte for byte.
    #[test]
    fn the_operation_set_is_exactly_the_one_specified() {
        assert_eq!(OPS, &["Starting", "Stopping", "Launching", "Installing", "Uninstalling"]);
    }

    #[test]
    fn op_busy_explains_which_operation_is_in_the_way() {
        let err = OpBusy { current: INSTALLING };
        assert!(err.to_string().contains("Installing"), "{err}");
    }
}
