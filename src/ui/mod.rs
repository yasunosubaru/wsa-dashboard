// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The UI layer.
//!
//! `home_model` and `refresh` are pure and are compiled into **both** bins:
//! the GUI draws from them, and `wsadash` runs their tests (via `#[path]`, so
//! the CLI does not get dragged into the Slint UI). `data` and `handlers` are
//! GUI-only and stay out of the CLI's module tree.

pub mod data;
pub mod home_model;
pub mod refresh;
