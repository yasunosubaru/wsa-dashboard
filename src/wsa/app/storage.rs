// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! WSA's storage, read off the local disk.
//!
//! Spec 2 lists `VhdxInfo` as "read `LocalState\*.vhdx` metadata (no adb)" for
//! the disk page, and spec 11 line 348 makes that page the acceptance test for
//! "the disk page still works with the subsystem closed". Both halves of that
//! sentence are why this module is deliberately adb-free: the page has to have
//! an answer on a machine where the subsystem is not running, which is exactly
//! when the `LocalCache` directory may be missing.
//!
//! # Two facts that cost a wrong guess each
//!
//! **The directory is `LocalCache`, not `LocalState`.** Spec 2 says `LocalState`;
//! on this machine the images are in
//! `%LOCALAPPDATA%\Packages\<PFN>\LocalCache`. MAPPING 12 records the
//! correction, alongside spec 111's `LocalPortMin` one.
//!
//! **The file names carry a version suffix.** `userdata.2.vhdx`, not
//! `userdata.vhdx`. Matching the full name would empty this page the first time
//! WSA renumbers its images, which is why [`images_in`] matches a prefix and
//! the exact `.vhdx` extension and nothing in between.

/// The package family WSA's per-user data lives under. Fixed rather than
/// derived from [`crate::wsa::app::wsa_path`]: this module is the one place
/// that must work when WSA is *not* installed, and `WsaPaths` is precisely
/// what fails to resolve in that state.
pub const PACKAGE_FAMILY: &str = "MicrosoftCorporationII.WindowsSubsystemForAndroid_8wekyb3d8bbwe";

/// The directory under the package's per-user folder that holds the images.
/// Spec 2 says `LocalState`; this is where they actually are.
pub const CACHE_DIR_NAME: &str = "LocalCache";

/// Which of WSA's two images a file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StorageRole {
    /// The user's Android filesystem. The big one; the one anybody asks about.
    UserData,
    /// Package and user bookkeeping. Small, and its size means nothing.
    Metadata,
}

/// One image on disk.
///
/// Every field is read from the filesystem. There is deliberately no field
/// that could only come from adb: the page renders identically whether or not a
/// device is attached, and anything device-derived arrives separately as the
/// Android-side usage line.
#[derive(Debug, Clone, PartialEq)]
pub struct StorageImage {
    pub file_name: String,
    pub path: std::path::PathBuf,
    pub size_bytes: u64,
    /// `%Y-%m-%d %H:%M` local time, the format the reference's
    /// `MountedDiskInfo.mount_time` uses; empty when the platform refuses to
    /// say.
    pub modified: String,
    pub role: StorageRole,
}

/// `<%LOCALAPPDATA%>\Packages\<PACKAGE_FAMILY>\LocalCache`.
///
/// `None` when the environment variable is absent, which is the same answer as
/// "WSA has never run on this account". Does not touch the filesystem, so it
/// cannot fail in a way the caller has to handle differently from `Some`.
pub fn cache_dir() -> Option<std::path::PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    let mut path = std::path::PathBuf::from(local);
    path.push("Packages");
    path.push(PACKAGE_FAMILY);
    path.push(CACHE_DIR_NAME);
    Some(path)
}

/// Every WSA image under `dir`, newest role first.
///
/// A missing `dir` is an empty list, not an error: that is the ordinary state
/// of a machine where WSA is stopped, uninstalled, or has never run, and it is
/// the state this page exists to be useful in.
///
/// An entry that is not a readable regular file is skipped rather than
/// failing the whole call, because the one time this runs is while WSA may be
/// writing.
pub fn images_in(dir: &std::path::Path) -> std::io::Result<Vec<StorageImage>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };

    let mut images = Vec::new();
    for entry in entries.flatten() {
        let Some(role) = role_of(&entry.file_name()) else {
            continue;
        };
        // `metadata` on a *directory* succeeds; its length is what does not.
        // Requiring a regular file covers the sharing-violation case and a
        // stray directory that happens to be named like an image, with one
        // check.
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        images.push(StorageImage {
            file_name: entry.file_name().to_string_lossy().into_owned(),
            path: entry.path(),
            size_bytes: meta.len(),
            modified: format_modified(meta.modified().ok()),
            role,
        });
    }

    // `StorageRole`'s declaration order *is* the display order: UserData sorts
    // before Metadata, so the image anyone is looking for is on top without a
    // custom comparator.
    images.sort_by(|a, b| a.role.cmp(&b.role).then_with(|| a.file_name.cmp(&b.file_name)));
    Ok(images)
}

/// `userdata…` / `metadata…` with exactly the `.vhdx` extension.
///
/// A prefix match on purpose: the real names are `userdata.2.vhdx` and the
/// number moves. It is *not* a substring match, so `userdata.vhdx.bak` and
/// `myuserdata.vhdx` are both rejected -- a page that reports "how much space
/// WSA has left" cannot afford to count a backup copy.
fn role_of(name: &std::ffi::OsStr) -> Option<StorageRole> {
    let lower = name.to_str()?.to_ascii_lowercase();
    if !lower.ends_with(".vhdx") {
        return None;
    }
    if lower.starts_with("userdata") {
        Some(StorageRole::UserData)
    } else if lower.starts_with("metadata") {
        Some(StorageRole::Metadata)
    } else {
        None
    }
}

fn format_modified(modified: Option<std::time::SystemTime>) -> String {
    let Some(seconds) = modified
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|since| i64::try_from(since.as_secs()).ok())
    else {
        return String::new();
    };
    chrono::DateTime::from_timestamp(seconds, 0)
        .map(|utc| utc.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{StorageRole, images_in};

    /// The state this page has to survive most often: WSA stopped. Not an
    /// error, not a panic, and emphatically not a red page -- an empty list and
    /// whatever the view layer wants to say about it.
    #[test]
    fn a_missing_cache_directory_is_empty_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("LocalCache");
        assert!(images_in(&gone).unwrap().is_empty());
    }

    /// One unreadable entry must not take the other one down with it. A file
    /// being written by WSA at the moment we look is the ordinary way this
    /// happens, and the page's whole purpose is to work while WSA is around.
    #[test]
    fn one_unreadable_image_does_not_hide_the_others() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("userdata.2.vhdx"), b"x").unwrap();
        // A directory named like an image: `metadata` on it succeeds, but its
        // length does not, which is the closest a fixture gets to a sharing
        // violation without needing a real one.
        std::fs::create_dir(dir.path().join("metadata.2.vhdx")).unwrap();

        let images = images_in(dir.path()).unwrap();
        assert_eq!(images.len(), 1, "{images:?}");
        assert_eq!(images[0].role, StorageRole::UserData);
    }

    /// The glob is a prefix match and an extension match, not a substring
    /// match. `userdata.vhdx.bak` is a backup someone made; showing it as
    /// WSA's live data image would be a lie on a page whose whole content is
    /// "how much space is left".
    #[test]
    fn no_matching_glob_is_an_empty_list_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("readme.txt"), b"x").unwrap();
        std::fs::write(dir.path().join("userdata.vhdx.bak"), b"x").unwrap();
        assert!(images_in(dir.path()).unwrap().is_empty());
    }

    /// The version number is not `2` and is not `1`. WSA renumbers these; the
    /// suffix is a version marker, not part of the name.
    #[test]
    fn the_version_suffix_is_not_hardcoded() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("userdata.7.vhdx"), vec![0u8; 10]).unwrap();
        std::fs::write(dir.path().join("metadata.7.vhdx"), vec![0u8; 4]).unwrap();

        let images = images_in(dir.path()).unwrap();
        assert_eq!(images.len(), 2, "{images:?}");
        assert_eq!(images[0].size_bytes, 10);
        assert_eq!(images[1].size_bytes, 4);
    }

    /// `userdata` first, always. The page is read top to bottom and the data
    /// image is the one anyone is looking for; making them sort alphabetically
    /// would put `metadata` first half the time and hide the answer under a
    /// 72 MB curiosity.
    #[test]
    fn user_data_sorts_before_metadata() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["metadata.3.vhdx", "userdata.3.vhdx", "userdata.1.vhdx"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }

        let roles: Vec<_> = images_in(dir.path()).unwrap().iter().map(|i| i.role).collect();
        assert_eq!(
            roles,
            vec![StorageRole::UserData, StorageRole::UserData, StorageRole::Metadata],
        );
    }
}
