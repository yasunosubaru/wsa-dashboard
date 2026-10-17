// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only
//
// Root module for app-level concerns.
//
// P0 only carries constants (build.rs and the About page read them). The rest
// of the reference's src/app/* -- launcher, lifecycle (single instance),
// runner, tray, cli, scheduler, window -- lands in P2+ as it is ported.
// See docs/MAPPING.md section 2.1 for the identifier substitutions.

pub mod constants;
pub mod immediate;
pub mod launcher;
pub mod runner;
pub mod state;
pub mod tasks;
pub mod window;

/// One instance, and waking it (`single_instance`), the Run entry
/// (`autostart`), and the startup log line (`startup_info`). All three are the
/// reference's, and all three are app-level rather than page-level.
pub mod autostart;
pub mod single_instance;
pub mod startup_info;

/// Scheduled tasks (P7). The pure layer only: what a task is, what a cron
/// means, and what Windows exports. The commands that create and delete tasks
/// arrive in P7 Task 2.
pub mod scheduler_task;

/// The tray (P7): the icon itself, and the menu it shows as data. Split for the
/// reason `ui/disk_model.rs` is split from `ui/handlers/disk.rs` -- `tray.rs`
/// names `AppWindow` and only the GUI binary compiles it, while `tray_model.rs`
/// is a decision a test can reach.
pub mod tray;
pub mod tray_model;
