// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The UI layer.
//!
//! `home_model` is pure and is compiled into **both** bins: the GUI draws
//! from it, and `wsadash` runs its tests (via `#[path = "ui/home_model.rs"]`,
//! so the CLI does not get dragged into the Slint UI). `data` and `handlers`
//! are GUI-only and stay out of the CLI's module tree.

pub mod home_model;
