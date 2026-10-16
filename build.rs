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
//   icon + icon.rc + embed-resource    yes (P8)     build.rs:45-139
//   verify_translations (i18n)         P5           build.rs:145-223
//
// The icon block landed in P8 and brought the reference's line 4 with it:
//     #[path = "src/app/constants.rs"]
//     mod constants;
// It reads APP_NAME / APP_ID / COMPANY_NAME / LEGAL_COPYRIGHT / PROJECT_REPOSITORY
// from src/app/constants.rs, which is why that file has existed since P0.

use std::fs;
use std::path::Path;

use toml::Value;

#[path = "src/app/constants.rs"]
mod constants;

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

    // The icon and the version resource (P8). The reference's block
    // (`build.rs`:45-139) carried across whole, including the six sizes and
    // `Lanczos3`: Windows picks the entry that matches the DPI, so an exe with
    // only a 256px icon and no 16px one is blurry in the taskbar and sharp in
    // the Alt-Tab list.
    //
    // **The artwork is still the reference's.** Spec Q6 says to reuse its ICO
    // during development and replace it before packaging; there is no new icon
    // to replace it with, so `assets/logo/logo.png` is a copy of the
    // reference's 256x256 and MAPPING 18.5 records that as a debt rather than
    // pretending otherwise. It is licence-clean (GPL-3.0 upstream, GPL-3.0
    // here) and it is *ours* to change the moment there is something to change
    // it to.
    //
    // Written into `assets/logo/` rather than to `OUT_DIR` because the
    // reference writes it there and because the `.ico` and the `.rc` are
    // artefacts a reader can look at. They are build outputs, so
    // `assets/logo/logo.ico` and `assets/logo/icon.rc` are gitignored.
    #[cfg(windows)]
    {
        use image::ImageReader;

        let png_path = Path::new("assets/logo/logo.png");
        let ico_path = Path::new("assets/logo/logo.ico");

        if png_path.exists() {
            println!("cargo:rerun-if-changed={}", png_path.display());

            let img = ImageReader::open(png_path)
                .expect("Failed to open PNG file")
                .decode()
                .expect("Failed to decode PNG");

            use image::imageops::FilterType;
            let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
            let sizes = [16, 32, 48, 64, 128, 256];

            for &size in &sizes {
                let resized = img.resize_exact(size, size, FilterType::Lanczos3);
                let rgba = resized.to_rgba8();
                let icon_image = ico::IconImage::from_rgba_data(size, size, rgba.into_raw());
                let entry =
                    ico::IconDirEntry::encode(&icon_image).expect("Failed to encode icon image");
                icon_dir.add_entry(entry);
            }

            let file = std::fs::File::create(ico_path).expect("Failed to create ICO file");
            icon_dir.write(file).expect("Failed to write ICO file");

            let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.1".to_string());
            let mut version_parts = version
                .split('.')
                .map(|p| p.parse::<u16>().unwrap_or(0))
                .collect::<Vec<_>>();
            while version_parts.len() < 3 {
                version_parts.push(0);
            }
            let (major, minor, patch) = (version_parts[0], version_parts[1], version_parts[2]);

            let icon_rc_path = Path::new("assets/logo/icon.rc");
            // The reference's sentence is "{} - Management Tool for WSL". This
            // is the same sentence about the thing this program manages.
            let file_description =
                format!("{} - Management Tool for Windows Subsystem for Android", constants::APP_NAME);
            let original_filename = format!("{}.exe", constants::APP_ID);

            std::fs::write(
                icon_rc_path,
                format!(
                    r#"#include <windows.h>

1 ICON "logo.ico"

VS_VERSION_INFO VERSIONINFO
 FILEVERSION {major},{minor},{patch},0
 PRODUCTVERSION {major},{minor},{patch},0
 FILEFLAGSMASK 0x3fL
#ifdef _DEBUG
 FILEFLAGS 0x1L
#else
 FILEFLAGS 0x0L
#endif
 FILEOS 0x40004L
 FILETYPE 0x1L
 FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "{company_name}"
            VALUE "FileDescription", "{file_description}"
            VALUE "FileVersion", "{major}.{minor}.{patch}.0"
            VALUE "InternalName", "{app_id}"
            VALUE "LegalCopyright", "{copyright}"
            VALUE "LegalTrademarks", "{project_repository}"
            VALUE "OriginalFilename", "{original_filename}"
            VALUE "ProductName", "{app_name}"
            VALUE "ProductVersion", "{major}.{minor}.{patch}.0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#,
                    company_name = constants::COMPANY_NAME,
                    file_description = file_description,
                    app_id = constants::APP_ID,
                    copyright = constants::LEGAL_COPYRIGHT,
                    project_repository = constants::PROJECT_REPOSITORY,
                    original_filename = original_filename,
                    app_name = constants::APP_NAME,
                    major = major,
                    minor = minor,
                    patch = patch
                ),
            )
            .expect("Failed to write icon.rc");

            let _ = embed_resource::compile(icon_rc_path, std::iter::empty::<&std::ffi::OsStr>());
        }
    }
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
