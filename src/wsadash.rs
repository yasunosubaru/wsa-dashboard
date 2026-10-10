// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// CLI entry point (`wsadash`).
//
// P0 placeholder so `cargo build --release` produces both binaries named in
// Cargo.toml. Task 13 (spec Q19) implements the subcommands:
//   probe | list | info | launch | stop-app | install | uninstall | start | stop

// The WSA domain layer hangs off this bin for P1 (models, adb, decoders,
// command table, executor). It is deliberately NOT a lib target: a lib would
// force `crate::app::...` to become `wsadashboard::app::...` everywhere and
// break line-by-line diffing against the reference. The GUI bin adds the same
// `mod wsa;` line when it starts consuming these types in P2.
mod adb;
mod wsa;

fn main() {
    eprintln!("wsadash: not implemented yet (P0 skeleton; CLI lands in Task 13)");
    std::process::exit(2);
}
