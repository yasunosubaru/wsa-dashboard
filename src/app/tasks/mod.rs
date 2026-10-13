// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Long-running work driven by the event loop.
//!
//! The counterpart of the reference's `src/app/tasks/mod.rs`, which spawns a
//! WSL monitor, a USB monitor, a state listener and a wakeup listener. One
//! task exists here for P2, because one page exists.

pub mod home;

pub use home::spawn_home_monitor;
