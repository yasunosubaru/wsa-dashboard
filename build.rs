// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// Build script.
//
// Structure mirrors the reference (docs/reference/build.rs) so later phases can
// re-add its blocks one for one:
//
//   block                              this file    reference
//   ---------------------------------  -----------  -------------------------
//   slint compilation                  yes          build.rs:13
//   .slint tree change detection       yes          build.rs:17 (see C3 below)
//   APP_EXPIRE_TIME from Cargo.toml    yes          build.rs:19-42
//   icon + icon.rc + embed-resource    P8           build.rs:45-139
//   verify_translations (i18n)         P5           build.rs:145-223
//
// When the icon/rc block lands, add the reference's line 4 verbatim:
//     #[path = "src/app/constants.rs"]
//     mod constants;
// It reads APP_NAME / APP_ID / COMPANY_NAME / LEGAL_COPYRIGHT / PROJECT_REPOSITORY
// from src/app/constants.rs, which is why that file must exist from P0 on.

use std::fs;
use std::path::Path;

use toml::Value;

fn main() {
    slint_build::compile("src/ui/app.slint").expect("Slint compilation failed");

    println!("cargo:rerun-if-changed=Cargo.toml");

    // C3. The reference watches only app.slint, which is enough once a project
    // has stopped growing: every .slint below it is reached by import, and
    // slint-build recompiles them then anyway. This project is still adding a
    // dozen of them, and an unwatched edit leaves cargo insisting nothing had
    // changed while the window keeps showing the old markup. Watching the tree
    // is the difference between "I changed the file and the build is stale" and
    // a build that is merely slow to look.
    watch_tree(Path::new("src/ui"));

    emit_expire_time();
}

/// Emit a `rerun-if-changed` for every `.slint` file at or under `dir`.
///
/// Directories are walked rather than listed, because `views/` and
/// `components/` are both new here and both will keep growing.
fn watch_tree(dir: &Path) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        // A missing directory is not worth failing the build over: the compile
        // step above already reported it, and with a better message.
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            watch_tree(&path);
        } else if path.extension().is_some_and(|ext| ext == "slint") {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

/// Resolve `[package.metadata] expire` into APP_EXPIRE_TIME.
///
/// Copied from reference build.rs:19-42. `expire = 0` means "never expires",
/// but the consumers compare against a timestamp, so 0 is expanded to
/// now + 365 days rather than left as 0.
fn emit_expire_time() {
    let cargo_toml_path = Path::new("Cargo.toml");
    let cargo_toml_content =
        fs::read_to_string(cargo_toml_path).expect("Failed to read Cargo.toml");
    let cargo_toml: Value = toml::from_str(&cargo_toml_content).expect("Failed to parse Cargo.toml");

    let mut expire_time = cargo_toml
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("expire"))
        .and_then(|e| e.as_integer())
        .unwrap_or(0);

    // If expire is 0, set it to 1 year from now.
    if expire_time == 0 && cargo_toml_content.contains("expire = 0") {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as i64;

        expire_time = now + (365 * 24 * 60 * 60 * 1000);
    }

    println!("cargo:rustc-env=APP_EXPIRE_TIME={}", expire_time);
}
