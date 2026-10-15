// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! What `settings.toml` holds, and what it holds by default.
//!
//! The counterpart of the reference's `src/config/models.rs` (358 lines), and
//! the same three sections under the same names with the same serde renames, so
//! a diff against it lines up and a reader can tell which of our fields are the
//! reference's by looking for the ones that are not.
//!
//! # What is not here, and why
//!
//! [`UserSettings`] drops five of the reference's fields. Three of them name
//! directories that only mean something when you can install things: the
//! instance directory, the backup directory and the temp directory. WSA mounts
//! nothing and installs nothing, so there is no directory for a reader to
//! choose and no default worth guessing. The fourth, `mount_auto_probe`, is
//! "auto-start a distribution so the mount table can be read" -- the disk page
//! does not probe, by design (spec section 11 line 348 asks it to work with
//! the subsystem stopped). The fifth, `temp_location`, goes with the first
//! three.
//!
//! Two more are kept with nothing reading them yet, because the key is the
//! reference's and the thing it will drive arrives later:
//!
//!   `show_drag`      the reference's instance list has a drag-sort handle.
//!                    Ours is a fixed list of nine entries, so it has no
//!                    handle -- the field defaults to false rather than
//!                    carrying a true that would promise a UI that is not there.
//!   `auto_shutdown`  "close the distribution when the program exits". WSA's
//!                    equivalent is a real question (stop the subsystem, or
//!                    leave it running?) and P7 answers it; until then the
//!                    switch is stored and inert, which is what the setting
//!                    page says about it.
//!
//! MAPPING 15 lists these by name.

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// The reference's default log verbosity: 4 of 5, one below trace.
pub fn default_log_level() -> u8 {
    4
}

/// A week. The reference's picker offers 7 / 15 / 30 and this keeps those three
/// as the values it recognises, while [`clamp_log_days`] accepts any positive
/// number so a hand-edited file is not rejected for using 14.
pub fn default_log_days() -> u8 {
    7
}

/// A week between update checks. The reference's picker offers daily / weekly /
/// every two weeks / monthly and encodes them 1 / 7 / 14 / 30; the default is
/// weekly.
pub fn default_check_update() -> u8 {
    7
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserSettings {
    /// When the file was last written, as a human-readable stamp. The reference
    /// keeps it for the same reason: a settings file a reader wants to inspect
    /// should be able to say when it last said something.
    #[serde(rename = "modify-time", default)]
    pub modify_time: String,
    /// When the update check last ran (P8).
    #[serde(rename = "check-time", default)]
    pub check_time: String,
    /// Days between update checks (P8).
    #[serde(rename = "check-update", default = "default_check_update")]
    pub check_update: u8,
    #[serde(rename = "logs-location", default = "default_logs_location")]
    pub logs_location: String,
    #[serde(rename = "ui-language", default = "default_ui_language")]
    pub ui_language: String,
    #[serde(rename = "auto-shutdown", default)]
    pub auto_shutdown: bool,
    #[serde(rename = "system-color", default)]
    pub system_color: bool,
    #[serde(rename = "dark-mode", default)]
    pub dark_mode: bool,
    #[serde(rename = "sidebar-collapsed", default)]
    pub sidebar_collapsed: bool,
    #[serde(rename = "log-level", default = "default_log_level")]
    pub log_level: u8,
    #[serde(rename = "log-days", default = "default_log_days")]
    pub log_days: u8,
    #[serde(rename = "colorful-icons", default)]
    pub colorful_icons: bool,
    /// The reference's `mail` shows a system-messages icon; ours keeps the
    /// field because the title bar already carries the slot, and P8's mail
    /// reader is what would fill it.
    #[serde(rename = "mail", default = "default_true")]
    pub mail: bool,
    #[serde(rename = "hide-pin", default)]
    pub hide_pin: bool,
    /// Stored and inert: ours has no drag-sort handle. See the header.
    #[serde(rename = "show-drag", default)]
    pub show_drag: bool,
    /// The reference's "enable sparse VHD by default", which it applies to each
    /// newly created WSL image.
    ///
    /// Not dropped with the other WSL-only fields, because P6 found it a real
    /// job: the storage page already reads a VHDX footer, and a footer carries
    /// both sizes -- the logical one and the one actually allocated. Preferring
    /// the allocated one is what a person asking this question wants to see, and
    /// it is a switch that changes what the page says. Key name and default are
    /// the reference's; MAPPING 15 records the change of meaning.
    #[serde(rename = "sparse-vhd", default)]
    pub sparse_vhd: bool,
    /// The reference's "auto-start a distribution so the mount table can be
    /// read". Ours cannot need that -- the storage page reads images off disk
    /// and spec section 11 line 348 holds it to working with the subsystem
    /// stopped -- so it became "refresh storage when the window opens", which
    /// is the same switch pointed at the thing this program does have to wait
    /// for.
    #[serde(rename = "mount-auto-probe", default)]
    pub mount_auto_probe: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            modify_time: String::new(),
            check_time: String::new(),
            check_update: default_check_update(),
            logs_location: default_logs_location(),
            ui_language: default_ui_language(),
            auto_shutdown: false,
            system_color: false,
            dark_mode: false,
            sidebar_collapsed: false,
            log_level: default_log_level(),
            log_days: default_log_days(),
            colorful_icons: false,
            mail: true,
            hide_pin: false,
            show_drag: false,
            sparse_vhd: false,
            mount_auto_probe: false,
        }
    }
}

/// `~/.wsadashboard/logs` -- the directory the program has been writing since
/// P2, asked of the one function that computes it rather than re-derived here.
///
/// `super::logs_dir` and not `crate::app::launcher::logs_dir` for two reasons:
/// the launcher calls *this* module's function, so asking it back would be a
/// loop; and this module has to be mountable into the CLI, where `crate::app`
/// does not exist. Two copies of the same path expression is how a settings
/// page ends up naming a directory the logger never writes to.
pub fn default_logs_location() -> String {
    super::logs_dir().to_string_lossy().into_owned()
}

/// English, because that is the one language this build can always render: a
/// pack that fails to load falls back to it, and a preference that points at a
/// pack that is not there would not.
pub fn default_ui_language() -> String {
    "en".to_string()
}

/// The tray switches (P7). All three are the reference's and all three are
/// stored from P6 so that the settings page can show real values for what it
/// will start honouring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraySettings {
    #[serde(default)]
    pub autostart: bool,
    #[serde(rename = "start-minimized", default)]
    pub start_minimized: bool,
    #[serde(rename = "close-to-tray", default = "default_true")]
    pub close_to_tray: bool,
}

impl Default for TraySettings {
    fn default() -> Self {
        Self {
            autostart: false,
            start_minimized: false,
            close_to_tray: true,
        }
    }
}

/// The eight switches behind the side bar's nine entries, the reference's
/// `SidebarConfig` field for field -- including that `home` is absent, because
/// the home entry is not a switch in either project.
///
/// `mount` keeps the reference's `alias = "mount_disk"`: a file written by an
/// older build of this program, or by the reference, spells it the other way
/// and should still be honoured rather than silently defaulting to true.
/// What Windows itself says, remembered.
///
/// The reference keeps these two beside the user's settings rather than asking
/// the OS each time, and for the same reason it keeps `system-language` at all:
/// `get_font_for` needs to know whether DirectWrite will render CJK on this
/// machine before it decides which font to ask Skia for, and that answer should
/// not change underneath a running window because a user flipped a Windows
/// setting mid-session.
///
/// Field names are the reference's; `timezone` has no reader here and is kept
/// only because the section is the reference's and a file with half of a section
/// in it is worse than one with all of it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemConfig {
    #[serde(rename = "system-language", default = "default_system_language")]
    pub system_language: String,
    #[serde(rename = "timezone", default)]
    pub timezone: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            system_language: default_system_language(),
            timezone: String::new(),
        }
    }
}

/// The locale Windows reports, as a locale name (`zh-CN`, `en-US`).
///
/// `GetUserDefaultLocaleName` rather than `LANG`: this program only runs on
/// Windows, and a Unix environment variable would answer on a host that is not
/// the one being asked about. Empty when the call fails, which is a real answer
/// and the one `get_font_for` treats as "ask Windows to fall back".
pub fn default_system_language() -> String {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;

    let mut buffer = [0_u16; 85];
    let length = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    if length <= 1 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..(length as usize).saturating_sub(1)])
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SidebarConfig {
    #[serde(default = "default_true")]
    pub toggle: bool,
    #[serde(default = "default_true")]
    pub add: bool,
    #[serde(default = "default_true")]
    pub usb: bool,
    #[serde(default = "default_true")]
    pub network: bool,
    #[serde(default = "default_true")]
    pub scheduler: bool,
    #[serde(default = "default_true", alias = "mount_disk")]
    pub mount: bool,
    #[serde(default = "default_true")]
    pub about: bool,
    #[serde(default = "default_true")]
    pub donate: bool,
}

impl Default for SidebarConfig {
    fn default() -> Self {
        Self {
            toggle: true,
            add: true,
            usb: true,
            network: true,
            scheduler: true,
            mount: true,
            about: true,
            donate: true,
        }
    }
}

/// The whole file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Config {
    /// Read once from the OS and written back. See [`SystemConfig`].
    #[serde(default)]
    pub system: SystemConfig,
    #[serde(default)]
    pub user: UserSettings,
    #[serde(default)]
    pub tray: TraySettings,
    #[serde(default)]
    pub sidebar: SidebarConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults are the reference's defaults. A number here that differs
    /// means a reader gets different behaviour from the same file, and it is
    /// the kind of difference nobody notices until a bug report says "it logs
    /// too much".
    #[test]
    fn defaults_are_the_reference_defaults() {
        let user = UserSettings::default();
        assert_eq!(user.log_level, 4);
        assert_eq!(user.log_days, 7);
        assert_eq!(user.check_update, 7);
        assert!(user.mail, "the reference shows the messages icon by default");
        assert!(!user.dark_mode);
        assert!(!user.sidebar_collapsed);

        assert!(TraySettings::default().close_to_tray);
        assert!(!TraySettings::default().autostart);

        // Every entry on by default, all eight of them.
        let sidebar = SidebarConfig::default();
        assert_eq!(sidebar, SidebarConfig {
            toggle: true, add: true, usb: true, network: true,
            scheduler: true, mount: true, about: true, donate: true,
        });
    }

    /// `show_drag` defaults to false even though the reference's is true: the
    /// reference has a drag-sort handle and ours has nothing to drag, so a
    /// stored `true` would describe a control that does not exist.
    #[test]
    fn show_drag_is_off_because_there_is_no_handle() {
        assert!(!UserSettings::default().show_drag);
    }

    /// The language default has to be a pack that is actually embedded, or the
    /// first launch silently falls back and the settings page shows a
    /// selection that does not match what is on screen.
    #[test]
    fn the_default_language_is_one_we_ship() {
        assert!(
            crate::i18n::available_languages()
                .contains(&UserSettings::default().ui_language),
            "the default language is not among the shipped packs"
        );
    }
}