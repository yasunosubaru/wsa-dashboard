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

pub mod ops;

use std::path::PathBuf;

use crate::wsa::app::wsa_path::WsaPaths;
use crate::wsa::executor::WsaCommandExecutor;

/// Everything needed to operate the subsystem.
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
}

impl WsaDashboard {
    pub fn new(executor: WsaCommandExecutor, paths: WsaPaths) -> Self {
        Self { executor, paths }
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
