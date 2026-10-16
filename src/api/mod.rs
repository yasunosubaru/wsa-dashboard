// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

#![allow(dead_code)]

// The `allow` is for the CLI's `#[path]` mount, where `mod api` is private and
// nothing outside it can call these -- the same reason and the same shape as
// `config/mod.rs`. What the mount buys is the two pure functions' tests.

//! The one network call this program makes.
//!
//! The counterpart of the reference's `src/api/` is a *reduction* rather than a
//! port. The reference's tree is a client for `https://api1.wslui.com`: a
//! request builder with per-request metadata headers, a response envelope, a
//! one-hour disk cache, and ten endpoint wrappers over it (about links, the
//! download page, the group chat, cron help, rootfs help, ...). **This build
//! never calls that host** -- spec Q6 and MAPPING section 5 -- so all of it is
//! gone, and what is left is the one thing that was not on that server: the
//! update check against GitHub releases, which the reference does through
//! `api::github` rather than through its own client.
//!
//! # Why the pure half is separate from the half that opens a socket
//!
//! [`releases::parse_release`] and [`releases::version_is_newer`] are the two
//! decisions with a wrong answer that is not an error. A JSON shape read
//! wrongly gives an empty tag, and an empty tag compares as not-newer -- so the
//! page says "you are up to date" for every release that exists, which is
//! indistinguishable from the truth. A version comparison read wrongly says a
//! number is smaller than itself. Both are testable without a network, and they
//! are tested; the function that reads the socket is the thin part.

pub mod releases;
