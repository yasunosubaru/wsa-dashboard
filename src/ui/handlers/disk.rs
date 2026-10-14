// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The storage page's own work: read WSA's images off the disk, and open one.
//!
//! The counterpart of the reference's `src/ui/handlers/mount_disk.rs` (25,083
//! bytes), and the shortest handler in the tree, on purpose. Almost all of that
//! file is `wsl --mount`: enumerate physical disks through PowerShell, pick a
//! VHD, mount it, remember it in `mount.toml`, unmount it later. None of it
//! exists on the WSA side, because WSA mounts nothing -- spec section 3 line
//! 241 redefines the page as "WSA storage management", listing the images WSA
//! already owns.
//!
//! So there is no device to connect to and no adb to reach for. That is not a
//! simplification; it is the acceptance criterion. Spec section 11 line 348
//! asks that the disk page still work with the subsystem closed, and the only
//! way to satisfy that is for the page to never have needed it.

use std::rc::Rc;
use std::sync::Arc;

use slint::{ModelRc, VecModel, Weak};
use tracing::{debug, warn};

use crate::AppWindow;
use crate::app::state::AppState;
use crate::ui::disk_model;
use crate::wsa::app::storage;

/// Register the storage page's callbacks (reference `handlers/mount_disk.rs`).
pub fn setup(app: &AppWindow, app_handle: Weak<AppWindow>, _app_state: Arc<AppState>) {
    // Read once at startup: the folder does not move, and the bottom bar's
    // button needs it even when the list is empty -- which is exactly the
    // machine this page is for.
    let folder = storage::cache_dir();
    if let Some(app) = app_handle.upgrade() {
        app.set_disk_folder_to_open(
            folder
                .as_ref()
                .map(|dir| dir.to_string_lossy().into_owned())
                .unwrap_or_default()
                .into(),
        );
    }

    let ah = app_handle.clone();
    let for_refresh = folder.clone();
    app.on_disk_refresh_list(move || {
        let Some(app) = ah.upgrade() else { return };
        apply(&app, for_refresh.as_deref());
    });

    let for_open = folder.clone();
    app.on_disk_open_folder(move |path| {
        // Both callers arrive here -- a card's folder icon and the bottom bar's
        // primary button -- and they mean different things by "open". The
        // bottom bar's argument is the folder itself; a card's is the image
        // inside it, which Explorer should *select*. `path.is_dir()` cannot
        // tell them apart on a machine where WSA has never run, because then
        // the folder does not exist either, so the caller says which it meant.
        let is_folder = for_open
            .as_ref()
            .is_some_and(|dir| dir.to_string_lossy() == path.as_str());
        let arg = if is_folder {
            disk_model::explorer_arg_for_dir(std::path::Path::new(path.as_str()))
        } else {
            disk_model::explorer_arg_for_file(std::path::Path::new(path.as_str()))
        };
        debug!(arg = %arg, "opening in explorer");
        // A failure here is a missing Explorer, not a wrong path, and the user
        // can do nothing about it -- so it is logged rather than toasted. The
        // window that would show the toast may not be frontmost anyway, which
        // is the situation this click came from.
        if let Err(error) = open::that(&arg) {
            warn!(%error, "could not open explorer");
        }
    });

    if let Some(app) = app_handle.upgrade() {
        apply(&app, folder.as_deref());
    }
}

fn apply(app: &AppWindow, folder: Option<&std::path::Path>) {
    let page = disk_model::model(folder);
    let rows: Vec<crate::StorageImageUI> = page
        .rows
        .iter()
        .map(|row| crate::StorageImageUI {
            file_name: row.file_name.clone().into(),
            path: row.path.clone().into(),
            size_text: row.size_text.clone().into(),
            modified_text: row.modified_text.clone().into(),
            role_text: row.role_text.into(),
            // The Android-side usage line. Filled in P5b, where `adb shell df`
            // arrives with the network page; until then the card shows nothing
            // rather than a zero that would read as "your data disk is empty".
            usage_text: "".into(),
            has_usage: false,
        })
        .collect();
    app.set_disk_images(ModelRc::from(Rc::new(VecModel::from(rows))));
    app.set_disk_empty_note(page.note.into());
}

#[cfg(test)]
mod tests {
    // Nothing, and the reason is the usual one: every function here names
    // `AppWindow`, which only the GUI binary compiles, so anything written in
    // this module would sit behind `cargo test --release --bin wsadash` and
    // never run -- a test that looks, in a diff, exactly like one that does.
    //
    // The decisions that were worth testing did not stay here. `disk_model`
    // (sizes, roles, the empty state's two different notes, and which Explorer
    // argument each caller wants) is mounted into the CLI, and `storage` reads
    // the filesystem without a window. What is left in this file is the
    // translation between them, and a translation that cannot be exercised
    // without a window is better as eight lines of struct literal than as a
    // test that never runs.
}
