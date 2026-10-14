// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The UI layer.
//!
//! `home_model`, `refresh` and `tabs` are pure and are compiled into **both**
//! bins: the GUI draws from them, and `wsadash` runs their tests (via
//! `#[path]`, so the CLI does not get dragged into the Slint UI). `data` and
//! `handlers` are GUI-only and stay out of the CLI's module tree -- which is
//! why `tabs` exists at all: the decision it holds was written inside
//! `handlers/common.rs` first, where its three tests would have compiled
//! nowhere.

pub mod data;
pub mod disk_model;
pub mod handlers;
pub mod home_model;
pub mod refresh;
pub mod tabs;
