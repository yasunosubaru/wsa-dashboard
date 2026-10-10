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
    println!("cargo:rerun-if-changed=src/ui/app.slint");

    emit_expire_time();
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
