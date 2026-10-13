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
pub mod launcher;
pub mod runner;
pub mod state;
pub mod tasks;
pub mod window;
