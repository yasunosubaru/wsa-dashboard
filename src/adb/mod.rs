// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! adb transport: endpoint resolution, output decoders, command building.
//!
//! Mirrors the reference's `src/network/` + `src/usb/` split at the file
//! level; see docs/MAPPING.md.

pub mod port;
