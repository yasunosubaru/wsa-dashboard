// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The facade the CLI and the GUI both drive.
//!
//! Mirrors `src/wsl/dashboard/mod.rs` in the reference, which carries a
//! `distros` cache, a `Notify` and a refresh interval because `wsl -l -v` is
//! a snapshot command that tells you nothing until you ask again. Here the
//! reads go through [`WsaCommandExecutor`], which already owns the
//! concurrency state (single exit point, spec 4), so those fields would only
//! duplicate it. What is kept is the shape: one struct that owns an executor
//! plus the paths, with the operations living in a sibling `ops` module the
//! way the reference keeps them under `src/wsl/ops/`.

// `executor()` and `paths()` have no caller until the CLI (Task 13) and the
// device page (P2) reach for them; the ops methods in the sibling module
// access the fields directly.
#![allow(dead_code)]

pub mod op_guard;
pub mod ops;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use crate::wsa::app::wsa_path::WsaPaths;
use crate::wsa::executor::WsaCommandExecutor;

/// Everything needed to operate the subsystem.
#[derive(Debug)]
pub struct WsaDashboard {
    /// The single exit point for adb. Constructed once and kept: a fresh
    /// executor per call would silently drop every concurrency guarantee it
    /// carries.
    executor: WsaCommandExecutor,
    /// Where WSA lives on this machine, resolved from the Appx manifest.
    ///
    /// Resolved by the caller rather than here, because `resolve()` runs
    /// PowerShell and can fail with `WsaPathError::NotInstalled`. That is a
    /// state the UI must render *before* it has a dashboard, so it cannot be
    /// hidden inside this constructor.
    paths: WsaPaths,
    /// Which mutating operation is running, if any (spec 2.2).
    ///
    /// A `std::sync::Mutex` and not `tokio::sync::Mutex` on purpose:
    /// [`op_guard::WsaOpGuard`] is synchronous, and `Drop` cannot await, so an
    /// async mutex here would make unregistering the one thing that can fail
    /// at exactly the moment failure cannot be reported. Nothing holds this
    /// across an `await` -- only a `Copy` value is moved in and out -- so the
    /// sync kind costs nothing.
    active_op: Mutex<Option<&'static str>>,
}

impl WsaDashboard {
    pub fn new(executor: WsaCommandExecutor, paths: WsaPaths) -> Self {
        Self {
            executor,
            paths,
            active_op: Mutex::new(None),
        }
    }

    /// The operation registered by the guard holding the slot right now, if
    /// any. This is what the UI polls to decide whether to show the busy
    /// state, and what the CLI reports instead of guessing.
    pub fn get_active_op(&self) -> Option<&'static str> {
        *self.active_op_lock()
    }

    /// Lock the operation slot.
    ///
    /// `pub(crate)` rather than private because [`op_guard::WsaOpGuard`] is
    /// the only thing that should ever hold it, and it lives in a sibling
    /// module.
    pub(crate) fn active_op_lock(&self) -> MutexGuard<'_, Option<&'static str>> {
        // Nothing in `create` or `drop` can panic while the lock is held --
        // both only move a `Copy` value in and out. Recover anyway: a
        // permanently poisoned mutex would refuse every future operation with
        // an error no user action could clear, which is a strictly worse
        // outcome than losing a slot that was already lost.
        self.active_op.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// The heavy-operation lock (spec 2.2).
    ///
    /// Delegates to the executor's lock rather than introducing a second one.
    /// The reference keeps `heavy_op_lock` on the dashboard *and* relies on
    /// its UI handlers to remember a `try_lock()` pre-check before every heavy
    /// action; a second lock here would instead be a second truth about the
    /// same invariant, and the two can drift. There is one heavy lock, it is
    /// held for the whole `install`/`uninstall` round trip, and this accessor
    /// exists so the UI can do the same non-blocking pre-check the reference's
    /// handlers do.
    pub fn heavy_op_lock(&self) -> &tokio::sync::Mutex<()> {
        self.executor.heavy_lock()
    }

    /// The adb gate, for callers that need a command this facade has no
    /// operation for (the network page's port rules, the device page's
    /// `getprop` dump).
    pub fn executor(&self) -> &WsaCommandExecutor {
        &self.executor
    }

    /// Resolved WSA locations: install dir, manifest, client exe, settings
    /// AUMID and the adb port from the manifest firewall rule.
    pub fn paths(&self) -> &WsaPaths {
        &self.paths
    }

    /// Adb port the manifest declares, so call sites do not reach through
    /// `paths()` for the one number almost every operation needs.
    pub fn adb_port(&self) -> u16 {
        self.paths.adb_port
    }
}

/// Convenience for the CLI and the tests: a dashboard pointed at whatever
/// executable the caller says is adb, with hand-written paths.
///
/// The reference has no equivalent -- it hardcodes `wsl.exe` -- but adb is
/// not guaranteed to be on PATH (Review Focus #1), so the executable is an
/// input rather than a constant.
pub fn dashboard_for(adb_path: impl Into<PathBuf>, paths: WsaPaths) -> WsaDashboard {
    WsaDashboard::new(WsaCommandExecutor::new(adb_path), paths)
}
