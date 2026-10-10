// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Locating the installed WSA package.
//!
//! Unlike WSL -- which is part of Windows and needs no discovery -- WSA here
//! is a sideloaded Appx package whose install directory can be anywhere, so
//! every path the app uses has to be resolved at runtime.

pub mod wsa_path;
