// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! English text, and the two functions the UI calls to get at it.
//!
//! The signatures are the reference's (`src/i18n/mod.rs` lines 25 and the `t`
//! / `tr` it exposes), fixed in P1 so that `.slint` files could carry
//! `AppI18n.t("main.loading", ...)` from the first day and be diffed line by
//! line against the reference at review. Nothing has to change at a call site.
//!
//! # What changed in P6, and why it is not the reference's version
//!
//! The reference loads a pack over the English one, unconditionally. That is
//! right for it and wrong for us, because **our English is not the reference's
//! English**. Q12 keeps the key names and changes the sentences wherever WSA has
//! no counterpart to what the reference labels -- `operation.deleting` says
//! "Uninstalling ..." rather than "Deleting ...", `sidebar.mount` says "Storage"
//! rather than "Mount", `network.proxy_error_port` names 65535 rather than
//! 65534 because that is what the code checks.
//!
//! The other 49 packs were translated *from the reference's sentences*. Letting
//! them overwrite ours does not produce a translated dashboard; it produces a
//! dashboard that describes a different program, in a language, politely. So
//! [`OURS`] names the keys whose wording is ours and [`lookup`] refuses to
//! overlay them. The list is not a judgement call written by hand: a test
//! compares [`ENGLISH`] against the reference's `en.toml` and fails if the two
//! sets differ in either direction.
//!
//! # Where the strings come from
//!
//! [`ENGLISH`] is the authority and is what `assets/i18n/en.toml` is generated
//! from; a test asserts they agree. The other 49 files are the reference's own
//! packs, copied verbatim.

// Compiled into the CLI only so `cargo test --release --bin wsadash` can run
// these tests; the GUI is the only caller.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use rust_embed::RustEmbed;
use toml::Value;
use tracing::warn;

#[derive(RustEmbed)]
#[folder = "assets/i18n/"]
struct Asset;

/// The flattened current-language table: `section.key` -> text.
static TRANSLATIONS: Lazy<Mutex<HashMap<String, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

static CURRENT_LANG: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new("en".to_string()));

/// Resolve one key. A missing key is reported and then shown verbatim: a
/// blank string would hide the problem, and a panic would take the window
/// with it.
pub fn t(key: &str) -> String {
    match lookup(key) {
        Some(value) => value,
        None => {
            warn!(key, "no translation; showing the key itself");
            key.to_string()
        }
    }
}

/// Resolve one key and fill its `{0}`, `{1}`, ... placeholders.
///
/// More arguments than placeholders are ignored and fewer are not an error:
/// a translation that outgrew its caller must not crash the window over it.
pub fn tr(key: &str, args: &[String]) -> String {
    let mut text = t(key);
    for (index, value) in args.iter().enumerate() {
        text = text.replace(&format!("{{{index}}}"), value);
    }
    text
}

/// Right-to-left, for the reference's duplicated LTR/RTL markup.
///
/// The reference lists five; this is that list, and it is the only reason those
/// branches are not dead code.
pub fn is_rtl(lang: &str) -> bool {
    let lower = lang.to_lowercase();
    matches!(lower.as_str(), "ar" | "he" | "fa" | "ur" | "sd")
        || lower.starts_with("ar-")
        || lower.starts_with("he-")
        || lower.starts_with("fa-")
        || lower.starts_with("ur-")
        || lower.starts_with("sd-")
}

/// The language in force right now, as a normalised pack name.
pub fn current_lang() -> String {
    CURRENT_LANG.lock().expect("i18n lock").clone()
}

/// Every language this build can render, in the order the picker shows them.
///
/// Sorted by pack name rather than by whatever order the filesystem enumerates,
/// because this list *is* the drop-down's order and a picker that reshuffles
/// itself between runs is a picker whose position means nothing.
pub fn available_languages() -> Vec<String> {
    let mut names: Vec<String> = Asset::iter()
        .map(|file| {
            let name = file.to_string();
            name.trim_end_matches(".toml").to_string()
        })
        .collect();
    names.sort();
    names
}

/// Load `lang_code`'s pack over English, and make it current.
///
/// English first and the pack over the top, the reference's order, with one
/// exception: a key in [`OURS`] is not overlaid. The pack is still loaded and
/// still occupies memory -- it simply does not win for those keys.
pub fn load_resources(lang_code: &str) {
    let normalized = normalize_language_code(lang_code);
    let mut map = TRANSLATIONS.lock().expect("i18n lock");
    map.clear();
    load_file_to_map("en", &mut map);
    if normalized != "en" {
        load_file_to_map(&normalized, &mut map);
    }
    let count = map.len();
    drop(map);
    *CURRENT_LANG.lock().expect("i18n lock") = normalized;
    tracing::debug!(keys = count, "i18n resources loaded");
}

/// Map a locale onto a pack we actually ship, or `en`.
///
/// The reference answers this with a hundred-and-sixty-line chain of
/// `if lower == "..."` tests, one per pack, which is the same decision written
/// out longhand: *which shipped name does this locale mean?* The shipped list
/// is already in the binary, so the chain is replaced by a matcher over it plus
/// the handful of aliases that are not a prefix relationship at all --
/// `fil` <- `tl`, `no` <- `nb`/`nn`, and the two Chinese packs, which Windows
/// reports as `zh-CN` and `zh-TW` but which are also reachable as `zh`,
/// `zh-Hans`, `zh-Hant`, `zh-HK` and `zh-MO`.
///
/// A test pins the behaviour over all fifty shipped names, so "it agrees with
/// the reference" is asserted rather than hoped for. MAPPING 15 records the
/// substitution.
pub fn normalize_language_code(lang: &str) -> String {
    let lower = lang.to_lowercase().replace('_', "-");
    let names = available_languages();

    // Exact, ignoring case: "ZH-cn" and "zh-CN" are the same request.
    if let Some(hit) = names.iter().find(|name| name.to_lowercase() == lower) {
        return hit.clone();
    }

    // The Chinese packs are reachable by script name, and the two overlap:
    // `zh` alone means simplified, which is what a bare "中文" request means
    // everywhere the reference supports it.
    let traditional = ["zh-tw", "zh-hk", "zh-mo"];
    if traditional.contains(&lower.as_str()) || lower.starts_with("zh-hant") {
        if names.iter().any(|name| name == "zh-TW") {
            return "zh-TW".to_string();
        }
    }
    if lower == "zh"
        || lower == "zh-cn"
        || lower == "zh-sg"
        || lower.starts_with("zh-hans")
    {
        if names.iter().any(|name| name == "zh-CN") {
            return "zh-CN".to_string();
        }
    }

    // "pt-BR" is not a pack, but "pt" is, and a Brazilian reader is better
    // served by the pack we have than by English.
    if let Some(hit) = names
        .iter()
        .find(|name| lower.starts_with(&format!("{}-", name.to_lowercase())))
    {
        return hit.clone();
    }

    // Norwegian and Filipino: different codes, one language.
    for alias in ["nb", "nn"] {
        if lower == alias || lower.starts_with(&format!("{alias}-")) {
            if names.iter().any(|name| name == "no") {
                return "no".to_string();
            }
        }
    }
    if lower == "tl" || lower.starts_with("tl-") {
        if names.iter().any(|name| name == "fil") {
            return "fil".to_string();
        }
    }

    "en".to_string()
}

/// The text to show for one key, in the current language.
fn lookup(key: &str) -> Option<String> {
    let english = ENGLISH
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, value)| (*value).to_string());

    // English is authoritative: a pack is consulted only for a key that is
    // translated *and* whose wording is the reference's.
    if is_ours(key) {
        return english;
    }
    TRANSLATIONS
        .lock()
        .expect("i18n lock")
        .get(key)
        .cloned()
        .or(english)
}

/// Whether this key's wording is ours rather than the reference's.
///
/// The answer is "yes" for the keys [`OURS`] lists and for keys the reference
/// never had -- those cannot be overlaid by anything anyway -- and "no" for
/// every other key. Unknown keys report "no" rather than "yes" so that a typo
/// in a call site shows the key itself through the missing-key path, which
/// warns, instead of silently resolving.
fn is_ours(key: &str) -> bool {
    OURS.contains(&key) || !ENGLISH.iter().any(|(candidate, _)| *candidate == key)
}

fn load_file_to_map(lang: &str, map: &mut HashMap<String, String>) {
    let filename = format!("{lang}.toml");
    let Some(content) = Asset::get(&filename) else {
        warn!(lang, "no language pack with that name; falling back to English");
        return;
    };
    let content = String::from_utf8_lossy(content.data.as_ref());
    // A pack that does not parse is skipped rather than fatal: one broken file
    // must not take the window down, and English underneath is a working page.
    // `toml::from_str::<Table>`, not `content.parse::<Value>()`: in toml 1.x the
    // latter parses a *value* and rejects a document outright ("unexpected
    // content, expected nothing"), which is how four tests came back failing
    // with the loader never having loaded anything.
    match toml::from_str::<toml::Table>(content.as_ref()) {
        Ok(table) => flatten("", &Value::Table(table), map),
        Err(error) => warn!(lang, %error, "language pack did not parse"),
    }
}

/// `[network]` + `no_rules` -> `network.no_rules`, recursively.
fn flatten(prefix: &str, value: &Value, map: &mut HashMap<String, String>) {
    match value {
        Value::Table(table) => {
            for (key, nested) in table {
                let full = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(&full, nested, map);
            }
        }
        Value::String(text) => {
            // An empty string in a pack means "not translated", and writing it
            // over the English would blank a label rather than leave it.
            if !text.is_empty() {
                map.insert(prefix.to_string(), text.clone());
            }
        }
        _ => {}
    }
}

pub static ENGLISH: &[(&str, &str)] = &[
    ("common.app_name", "WSA Dashboard"),
    // -- sidebar: the nine entries (P3) -------------------------------------
    // Key names are the reference's; the three whose page means something else
    // here carry our words, which is Q12's whole rule:
    //   add      -> spec 3 maps the reference's "Add an instance" (a WSL distro)
    //               onto "install an APK"
    //   usb      -> mapped onto a device page; it lists adb targets, not WSL USB
    //   mount    -> the reference mounts physical disks into WSL; ours is
    //               storage management (spec 3), so "Storage"
    // `home` and `settings` are the two P3 actually shows; the rest are here so
    // the phases that turn their `show_*` on find them already written.
    ("sidebar.home", "Home"),
    ("sidebar.add", "Install APK"),
    ("sidebar.usb", "Devices"),
    ("sidebar.network", "Network"),
    ("sidebar.scheduler", "Task scheduler"),
    ("sidebar.mount", "Storage"),
    ("sidebar.settings", "Settings"),
    ("sidebar.about", "About"),
    ("sidebar.donate", "Donate"),
    // -- usb: the device page (P4) ------------------------------------------
    // Key names are the reference's own `[usb]` block (Q12: identifiers stay),
    // and the view they label was renamed in spec section 3 because what it
    // shows changed from usbip targets to adb targets. The values are ours:
    //   empty   the reference counts USB devices on the host; we list adb's own targets
    //   attach  the reference attaches a USB device to WSL; we open a session
    //           to a target ("Attach to WSL" -> "Connect" is the same verb in
    //           a different domain, and renaming the key would only lose the
    //           line that ties ours to theirs)
    //   detach  and its opposite
    // `usb.bind`/`unbind` and the four usbip settings keys are deliberately not
    // carried over: there is nothing to bind. See MAPPING section 11.
    ("usb.empty", "No adb targets connected"),
    ("usb.loading", "Loading devices..."),
    ("usb.not_found_error", "adb was not found. Install Android SDK platform-tools, then restart."),
    ("usb.attach", "Connect"),
    ("usb.detach", "Disconnect"),
    // Ours: the reference has no subsystem row to distinguish, because every
    // row of its list is a USB device and none of them is the OS's own.
    ("usb.subsystem", "Windows Subsystem for Android"),
    ("usb.device_info", "Device properties"),
    ("usb.detail", "Properties"),
    // -- add: the install-APK page (P4) -------------------------------------
    // The reference's `[add]` block is a *dotted* section, so these are named
    // `add.<key>` rather than `add.<key>` nested under a table -- same thing
    // here. Keys are its; values are ours where its name an object we do not
    // have (see the four rows below and MAPPING section 11).
    ("add.select_file", "Select File"),
    // Ours: the reference's path field is an install *directory* for a WSL
    // instance, and ours is a file to install from. Nothing to reuse.
    ("add.apk_file", "APK file"),
    ("add.no_file", "No file selected"),
    ("add.create", "Install APK"),
    ("add.installing", "Installing..."),
    ("add.note", "The package is installed on the connected target, which can take a moment."),
    // Ours: the reference's page reports progress by streaming into
    // `terminal_output`, and `adb install` streams nothing, so these are the
    // two things it can say when it comes back.
    ("add.installed", "Installed"),
    ("add.file_missing", "That file does not exist."),
    // -- scheduler: the task scheduler page (P7) -----------------------------
    // Key names **and values** are the reference's `[scheduler]` block verbatim
    // (Q12), which is why none of them is in [`OURS`]: the page is a 1:1 port of
    // its structure and copy. The reference's `en.toml` carries the block
    // between `[mail]` and `[mount_disk]`, and it is inserted in that position
    // here too.
    //
    // Three of these are *known* to describe a Windows-shaped task rather than
    // an `adb shell` one and are left untouched on purpose for now:
    //   dialog_command       ends with "(Supports select executables)", which is
    //                        about browsing for a Windows binary. The dialog
    //                        still offers Browse, so the sentence is not yet a
    //                        lie about a control that is missing -- it is a lie
    //                        about what the control picks.
    //   cron_error_*         the reference's cron dialect. Ours accepts a subset
    //                        (`schtasks` cannot express `1,5` or `1-5/2`; see
    //                        MAPPING 16.1), so these sentences will need a
    //                        companion that names what *is* accepted.
    //   error_name_invalid   the reference's 60-character Windows task-name
    //                        rule. Whether we keep that bound is a decision the
    //                        handler makes, not this table.
    // Rewording any of them moves it into [`OURS`] and regenerates `en.toml`,
    // and that is Task 6's work (`错误文案全部要重新对齐`, MAPPING 16.8), done
    // with the rest of the page's semantics rather than split across commits.
    ("scheduler.add_btn", "Add Task"),
    ("scheduler.no_tasks", "No tasks found"),
    ("scheduler.loading", "Querying scheduled tasks..."),
    ("scheduler.edit_btn", "Edit"),
    ("scheduler.delete_btn", "Delete"),
    ("scheduler.enable_btn", "Enable"),
    ("scheduler.disable_btn", "Disable"),
    ("scheduler.run_now_btn", "Run Now"),
    ("scheduler.dialog_add_title", "Add Task"),
    ("scheduler.dialog_edit_title", "Edit Task"),
    ("scheduler.dialog_task_name", "Task Name"),
    ("scheduler.dialog_command", "Command (Supports select executables)"),
    ("scheduler.dialog_arguments", "Arguments"),
    ("scheduler.dialog_browse", "Browse"),
    ("scheduler.dialog_cron_expr", "Schedule (Cron Expression)"),
    ("scheduler.dialog_admin_mode", "Run with administrator privileges"),
    ("scheduler.cron_error_required", "Schedule rule is required"),
    ("scheduler.cron_error_unsupported", "Unsupported Cron expression"),
    ("scheduler.cron_error_fields", "Cron expression must have 5 fields (minute hour day month weekday)"),
    ("scheduler.cron_error_comma", "Comma-separated values are not supported, use single value, *, or */N"),
    ("scheduler.cron_error_step", "Step must be a positive integer"),
    // The reference's sentence here is "Cron field value is out of bounds", and
    // it is used for two different failures: a number outside the field's range
    // and a field that is not a number at all (`abc`, which `parse_field`
    // refuses with "Invalid value: 'abc'. Use a single number, *, or */N"). The
    // mapping is the reference's -- `cron::error_key` has one branch for both --
    // so the sentence has to say what is true of both.
    ("scheduler.cron_error_range", "Cron field must be a single number, *, or */N, within that field's range"),
    ("scheduler.error_name_required", "Task name is required"),
    ("scheduler.error_command_required", "Command is required"),
    // The reference's is "Name must be 60 characters or less and can only
    // contain A-Z, 0-9, -, _, .", and the `A-Z` is a trap: this program accepts
    // lower case too (`is_ascii_alphanumeric`, which is the reference's check as
    // well). A reader who believes the sentence deletes a working name or
    // capitalises for nothing.
    ("scheduler.error_name_invalid", "Name must be 60 characters or less and can only contain A-Z, a-z, 0-9, -, _, ."),
    ("scheduler.error_create_failed", "Failed to create task: {0}"),
    ("scheduler.error_delete_failed", "Failed to delete task: {0}"),
    ("scheduler.error_query_failed", "Failed to query tasks: {0}"),
    ("scheduler.task_created", "Task created successfully"),
    ("scheduler.task_deleted", "Task deleted successfully"),
    ("scheduler.task_updated", "Task updated successfully"),
    ("scheduler.task_toggled", "Task status changed"),
    ("scheduler.task_started", "Task started"),
    // The two failures the reference hardcodes in English (`handlers/
    // scheduler.rs`:278, :319, `format!("Toggle failed: {}", e)`). Ours are keys
    // for the reason the rest of this table exists: a sentence about a failed
    // action is not more English than a sentence about a successful one.
    ("scheduler.error_toggle_failed", "Failed to change the task: {0}"),
    ("scheduler.error_run_failed", "Failed to run the task: {0}"),
    // The file picker's own window title, which the reference hardcodes in
    // English next to its filter (`handlers/scheduler.rs`:475).
    ("scheduler.dialog_select_command", "Select Command"),
    // -- mount_disk: the storage page (P5a) ---------------------------------
    // Key names are the reference's `[mount_disk]` block (Q12). Values are ours
    // throughout, because the page means something else: spec section 3 line 241
    // turns "mount a physical disk or VHD into WSL" into "WSA storage
    // management", and WSA mounts nothing.
    //
    //   refresh    the reference's and ours: the same verb.
    //   mount_btn  "Mount Disk" -> "Open Folder". The reference's opens a dialog
    //              to pick a disk; there is nothing here to pick, so it opens
    //              the directory the images live in.
    //   mount      the card's tooltip, same substitution as `mount_btn`.
    //   no_mounted the empty state. Ours says *where* it looked, because on a
    //              machine where WSA has never run the empty list is the normal
    //              answer and a bare "no mounted disks" would read as a fault.
    //              No `{0}` placeholder: `tr` fills `{0}`, `{1}`, ... only, and
    //              the real path is appended by Rust instead -- it knows where
    //              it looked and this string does not.
    //
    // `mount_disk.unmount` and `mount_disk.delete` are deliberately absent: the
    // card does not render the two icons that would ask for them, and a key
    // nothing asks for is a line to keep honest. See MAPPING section 12.
    ("mount_disk.refresh", "Refresh"),
    ("mount_disk.mount_btn", "Open Folder"),
    ("mount_disk.mount", "Open Folder"),
    (
        "mount_disk.no_mounted",
        "No WSA storage images in:",
    ),
    // Ours: the case where there is no folder to name at all, which the
    // sibling key cannot describe -- it ends in a colon and expects a path
    // after it. The two are worded apart because they are different facts: a
    // folder that is there and holds nothing means WSA was reset or moved
    // package family, and a folder that could not be located means this
    // account has never had one.
    (
        "mount_disk.no_mounted_no_dir",
        "No WSA storage images, and LOCALAPPDATA is not set, so there is no WSA data folder to look in.",
    ),
    // -- dialog: the shared shells (P5a) -------------------------------------
    // Key names are the reference's `[dialog]` block (Q12). The delete pair is
    // ours in wording only: the reference deletes a WSL *distribution* and this
    // uninstalls an Android *package*, and it is worth saying so in the one
    // dialog whose confirm button destroys something.
    ("dialog.confirm", "Confirm"),
    ("dialog.delete_title", "Uninstall {0}"),
    (
        "dialog.delete_msg",
        "Uninstall {0}? Its app data is deleted too, and this cannot be undone.",
    ),
    // Ours: the reference's title is the distro's display name, which we cannot
    // get for an Android app (spec line 122 -- `pm list` and `dumpsys` both
    // refuse), so the package name is all there is to show.
    ("dialog.info_title", "{0}"),
    // Ours, all six. The reference's information dialog reports a WSL
    // distribution -- wsl_version, install_location, vhdx_path, vhdx_size,
    // actual_used, ip, is_sparse -- none of which means anything for a package.
    // `package_kind_system` / `package_kind_user` replace its single `status`
    // row, which said whether a *distro* was running; the question that matters
    // about a package is whether Android shipped it, since that decides whether
    // "uninstall" is even offered anywhere.
    ("dialog.package_version", "Version"),
    // Ours. Android leaves `versionName` empty for plenty of installed packages,
    // and `AppInfo`'s own test (`app_info_without_a_version_is_an_error_not_a_
    // fabricated_one`) pins that we do not invent one. So the dialog says the
    // value was not reported -- which is a fact about the package, and is not
    // the same claim as "version 0".
    ("dialog.unknown_version", "not reported"),
    ("dialog.version_code", "Version code"),
    ("dialog.package_size", "APK size"),
    ("dialog.package_kind", "Source"),
    ("dialog.package_kind_system", "Android system app"),
    ("dialog.package_kind_user", "Installed by user"),
    // -- main: the home page ---------------------------------------------
    ("main.loading", "Loading..."),
    // The reference names a WSL distribution; we list Android packages, so
    // the sentence is ours. Q28.
    ("main.distro_not_found", "No apps to show. Install an app first."),
    ("main.guide_link", "View Installation Guide"),
    // The three-tier degradation (spec section 5). None of these has a
    // reference counterpart: it reads Running/Stopped off `wsl -l -v`.
    ("main.subsystem_booting", "WSA is starting..."),
    (
        "main.subsystem_unauthorized",
        "Waiting for authorization on the device",
    ),
    ("main.subsystem_offline", "Connection lost"),
    ("main.subsystem_stopped", "WSA is not running"),
    ("main.subsystem_not_installed", "WSA is not installed"),
    ("main.start_subsystem", "Start WSA"),
    ("main.stale_data", "Data may be out of date"),
    ("main.no_apps", "No apps to show"),
    ("main.search_placeholder", "Search apps"),
    // -- distro: one card -------------------------------------------------
    ("distro.start", "Start"),
    ("distro.stop", "Stop"),
    ("distro.restart", "Restart"),
    ("distro.compress", "Compress"),
    ("distro.terminal", "Terminal"),
    ("distro.vscode", "VS Code"),
    ("distro.explorer", "Explorer"),
    ("distro.expand", "Expand"),
    ("distro.collapse", "Collapse"),
    ("distro.information", "Information"),
    ("distro.export", "Export"),
    ("distro.clone", "Clone"),
    ("distro.move", "Move"),
    ("distro.delete", "Delete"),
    ("distro.settings", "Settings"),
    ("distro.configs", "Configs"),
    ("distro.running", "Running"),
    ("distro.stopped", "Stopped"),
    // The reference's DistroInfo asks for this as its THIRD state -- anything
    // that is neither Running nor Stopped -- but never defines it, so the
    // reference renders the literal string "distro.installing" on screen. The
    // key name is kept (Q12) and the value is ours: for us the third state is
    // an app whose state is unknown because the subsystem is unreachable, and
    // "Installing" would be a claim the data cannot support.
    ("distro.installing", "Unavailable"),
    // -- operation: the single busy slot -----------------------------------
    ("operation.starting", "Starting ..."),
    ("operation.stopping", "Stopping ..."),
    ("operation.restarting", "Restarting ..."),
    // Ours in wording only: the reference's `operation.deleting` fires while a
    // WSL *distribution* is being removed from disk. Q12 keeps the key so the
    // line back to theirs survives; the sentence is the honest one for what this
    // program does, which is `adb uninstall`.
    //
    // Edited in place rather than added as a second row: `lookup` is a `find`,
    // so the first match wins and a duplicate would silently keep the old text.
    ("operation.deleting", "Uninstalling ..."),
    ("operation.fetching_info", "Fetching information..."),
    // The reference never launches or installs from its UI, so it has no key
    // for either; both follow its naming and its trailing spaces.
    ("operation.launching", "Launching ..."),
    ("operation.installing", "Installing ..."),
    // -- toast -------------------------------------------------------------
    // The reference's sentence names a distro; ours names an app and drops
    // the lecture, because the home page says it every few seconds while
    // anything is busy.
    ("toast.distro_busy", "{0} is busy ({1})"),
    // -- network: the port-forwarding / proxy page (P5b) --------------------
    // Key names are the reference's own `[network]` block (Q12: identifiers,
    // keys and config names keep the original). Values are its wherever the
    // words are about what is being *looked at*, and ours wherever the words
    // are about the *mechanism*, because the mechanism changed:
    //
    //   forward    the reference forwards to a WSL distro's IP through
    //              `netsh interface portproxy`; ours is `adb forward` /
    //              `adb reverse`, which has no distro and no address (spec 3
    //              line 239). The nouns in the labels are still ports, so the
    //              English is unchanged and only the data behind it differs.
    //   firewall   kept verbatim -- spec 3 line 239 keeps the reference's netsh
    //              firewall half even though the forwarding half is replaced.
    //   staging    `apply_all_rules_btn` / `cancel_all_rules_btn` /
    //              `rule_apply_tooltip` / `rule_cancel_tooltip` describe the
    //              reference's "edit now, apply later" model. adb has no
    //              staging: a mapping is in the table or it is not, and it is
    //              in the table the moment you add it. The keys stay and the
    //              cards no longer render them (same trade as P5a's
    //              `MountDiskCard`; MAPPING 14).
    //   initialize the reference's button installs the elevated helper netsh
    //              needs. adb needs no helper, so the button is gated off; P7's
    //              task scheduler is what gives it a real job here.
    ("network.tab_port_forwarding", "Port Forwarding"),
    ("network.tab_http_proxy", "HTTP Proxy"),
    ("network.add_rule_btn", "Add Rule"),
    ("network.apply_all_rules_btn", "Apply all rules"),
    ("network.cancel_all_rules_btn", "Cancel all rules"),
    ("network.initialize_btn", "Auto-Start Forwarding"),
    ("network.no_rules", "No port forwarding rules configured"),
    // The reference's wording says *it* has a mode. Ours does not -- `adb
    // forward` works whatever the subsystem is doing -- but the key, the branch
    // that renders it, and `{0}` all survive, because the reference's own
    // non-NAT branch is compiled and simply unreachable on a NAT system.
    ("network.nat_mode_required", "Port forwarding is only available in NAT mode. Your current mode is: {0}"),
    ("network.rule_fw_on", "FW: On"),
    ("network.rule_fw_off", "FW: Off"),
    ("network.rule_apply_tooltip", "Apply Rule"),
    ("network.rule_cancel_tooltip", "Cancel Rule"),
    // The proxy tab. The reference is configuring a proxy for a WSL shell;
    // ours is configuring Android's global HTTP proxy, so three of these
    // values drop the reference's WSL context:
    ("network.proxy_no_proxy", "No proxy"),
    ("network.proxy_config", "Proxy configuration"),
    ("network.proxy_host", "Host name:"),
    ("network.proxy_port", "Port number:"),
    ("network.proxy_no_proxy_label", "No proxy for:"),
    ("network.proxy_auth", "Proxy authentication"),
    ("network.proxy_user", "Login:"),
    ("network.proxy_pass", "Password:"),
    ("network.proxy_check_btn", "Check Connection"),
    // The reference's says "Reopen the terminal", which is a WSL shell. Android
    // reads the setting itself; nothing of ours has a terminal to reopen.
    ("network.proxy_save_success", "Proxy saved."),
    // "1 and 65534" is the reference's number, and its netsh range. Android's
    // is 1..=0xFFFF (AOSP `ProxySelector.validate`), which is what
    // `http_proxy_value` enforces -- so the sentence has to match the code
    // rather than the key's original English. MAPPING 14 records the
    // off-by-one.
    ("network.proxy_error_port", "Port must be a number between 1 and 65535"),
    ("network.proxy_error_invalid", "Invalid proxy config: {0}"),
    ("network.proxy_error_localhost", "Localhost or 127.0.0.1 is not allowed"),
    ("network.proxy_test_success", "Connection successful: HTTP {0}"),
    ("network.proxy_test_failed", "Connection failed: {0}"),
    ("network.rules_apply_success", "Port {0} successfully mapped to {1}"),
    ("network.rules_apply_all_failed", "Apply all failed: {0}"),
    ("network.rules_cancel_all_failed", "Cancel all failed: {0}"),
    // Ours, and necessarily so: the card's title line is `rule.distro_name`,
    // which in the reference holds a *distro's* name -- there is nothing to
    // name here, because a rule belongs to no instance. What distinguishes a
    // forward from a reverse is what the reader cannot get from the address
    // alone (both sides are ports), so the slot carries the direction. `usb`
    // got its own two extra keys the same way in P4.
    //
    // Deliberately phrased as a direction rather than as a port: the line under
    // it already prints `address:port -> port`, so repeating the numbers there
    // would buy no information.
    ("network.dir_forward", "Host -> WSA"),
    ("network.dir_reverse", "WSA -> Host"),
    // -- the add-rule dialog (P5b) ------------------------------------------
    // The reference's keys. Three values are ours, because the dialog's three
    // nouns are the ones whose referent does not exist here:
    //
    //   target_distro      the reference picks a WSL instance to forward to.
    //                      `adb forward` takes no address at all, so the field
    //                      names the one thing there is: "Target".
    //   listen_ip          the reference's is the WSL host's address; ours is
    //                      the address adb binds, which the reader cannot
    //                      change -- so it is filled in and the combo shows one
    //                      entry rather than pretending to be a choice.
    //   placeholders       kept: an example port is an example port.
    ("network.target_distro", "Target"),
    ("network.add_rule_listen_ip", "Listen IP"),
    ("network.add_rule_listen_port", "Listen Port"),
    ("network.add_rule_port_placeholder", "e.g. 8080"),
    ("network.add_rule_target_port", "Target Port"),
    ("network.add_rule_target_port_placeholder", "e.g. 80"),
    ("network.add_rule_enable_fw", "Add Firewall Rule"),
    ("dialog.add", "Add"),
    ("dialog.add_network_rule_title", "Add Port Forwarding Rule"),
    // -- the proxy help panel (P5b) -----------------------------------------
    // The reference's two sentences describe a WSL shell with an environment
    // variable. Ours describe what Android actually does with the setting.
    ("dialog.http_proxy_helper", "HTTP Proxy Usage"),
    ("dialog.http_proxy_helper_desc1", "1. When an app inside Android reaches the internet through a proxy you run."),
    ("dialog.http_proxy_helper_desc2", "2. Android reads it as a system-wide hint: apps that honour the system proxy are affected, apps that do not are not."),
    // -- the settings page (P6) ---------------------------------------------
    // Key names are the reference's whole `[settings]` block, plus the
    // `[settings.languages]`-shaped sub-tables the picker reads
    // (`log_levels`, `log_retention_options`, `update_options`). Values are
    // its unless the row is about something WSA does not have, in which case the
    // value is ours and the row says so:
    //
    //   distro_dir   the reference picks where WSL distributions are installed.
    //                Ours names the directory WSA keeps its data in -- a real
    //                path, read from the package registration, and the button
    //                beside it opens that path. Calling it an install
    //                directory would be a claim nothing implements.
    //   backup_dir   the reference exports and backs up instances here. Ours
    //                names the directory this program keeps everything it owns
    //                in, which is the same honest trade at one level up.
    //   stop_wsl / wsl_settings / wsl2_required / update_wsl
    //                and the two sparse-VHD rows describe WSL. Their key names
    //                stay -- the view renders them -- and MAPPING 15 says the
    //                values are ours.
    //   saved_success / saved_failed are ours because the reference's says
    //                nothing about a file; ours does.
    ("sidebar.toggle", "Toggle"),
    ("settings.tab_general", "General"),
    ("settings.tab_advanced", "Advanced"),
    ("settings.tab_interface", "Interface"),
    ("settings.language", "Software Language"),
    ("settings.auto_update", "Automatic update check"),
    ("settings.update_interval", "Automatic update check"),
    ("settings.log_level", "Log level"),
    ("settings.log_retention_days", "Log retention days"),
    ("settings.log_path", "Log path"),
    ("settings.distro_dir", "WSA data folder"),
    ("settings.backup_dir", "App data folder"),
    // Not "Select Folder". The reference's three pickers chose a directory with
    // `rfd`; ours have nothing to choose -- the first two rows show where the
    // subsystem and this program actually keep their data -- so the button opens
    // the folder it sits next to. A button labelled "Select" that cannot select
    // is worse than one labelled "Open" that does.
    ("settings.select_folder", "Open Folder"),
    ("settings.select_log_dir", "Select the folder the logs are written to"),
    ("settings.select_distro_dir", "Open the folder WSA keeps its data in"),
    ("settings.select_backup_dir", "Open the folder this program keeps its data in"),
    ("settings.auto_shutdown_msg", "Stop WSA when the program exits."),
    ("settings.system_color", "Use system color mode"),
    ("settings.tray_autostart", "Automatically start on system startup."),
    ("settings.tray_start_minimized", "Start minimised to the system tray."),
    ("settings.tray_close_to_tray", "Close the window to the tray instead of exiting."),
    ("settings.save", "Save"),
    ("settings.saved_success", "Settings saved."),
    ("settings.saved_failed", "Could not save the settings: {0}"),
    ("settings.sidebar_features", "Sidebar features"),
    ("settings.sparse_vhd", "Report allocated size"),
    ("settings.sparse_vhd_desc", "On the storage page, show how much of each image is actually allocated rather than the size it claims to be. Both numbers are in the image footer, so this changes which of them you read, not how much disk you get back."),
    ("settings.mount_auto_probe", "Refresh storage when the window opens"),
    ("settings.mount_auto_probe_desc", "The storage page reads VHD images off disk, so it works with the subsystem stopped and there is nothing to probe for. This asks for it to re-read on launch instead of on your first visit."),
    ("settings.colorful_icons", "Enable colorful icons mode"),
    ("settings.mail_icon_always", "Always show the messages icon"),
    ("settings.hide_pin_icon", "Hide always-on-top button"),
    // The reference's instance list has a drag-sort handle. Ours is a fixed list
    // of nine entries with an order that means something, so there is nothing to
    // drag and the switch is off by default. Kept because the key is the
    // reference's and because turning it on should not silently cost a list order.
    ("settings.show_drag_icon", "Show list drag-sort handle"),
    // The reference opens `~/.wslconfig` in an editor. There is no such file for
    // this program and no editor to open it with; ours opens the folder its own
    // settings live in. Keeping the reference's sentence would name a file that
    // does not exist on the machine the reader is looking at.
    ("settings.wsl_settings", "Show settings file"),
    ("settings.wsl2_required", "This feature needs WSL 2.3.0 or newer."),
    ("settings.update_wsl", "Visit the WSL download page."),
    // Ours stops the subsystem. The key is the reference's (Q12).
    ("settings.stop_wsl", "Stop WSA"),
    ("dialog.stop_wsl_success", "The subsystem has been stopped."),
    // -- the about page (P6, Task 5) ----------------------------------------
    // Key names are the reference's whole `[about]` block. Four values are ours,
    // and the reason is the same in each case: the reference's sentence is about
    // WSL or about a project that is not this one.
    //
    //   description     the reference describes WSL instance management. Ours
    //                   describes what this window does, which is a different
    //                   list of things.
    //   copyright       "WSL Dashboard Contributors" -> ours. Same licence, our
    //                   name; leaving theirs in would credit a project that is
    //                   not shipping this binary.
    //   star / love     the reference asks for a GitHub star and calls itself a
    //                   labour of love. Ours keeps `love` (it is a true and
    //                   cheerful thing to say about free software) and points
    //                   `star` at the issue tracker instead, because that is the
    //                   address that exists.
    //   group / talk / documents / announcements
    //                   the reference fetches all four from api1.wslui.com
    //                   (`handlers/about.rs:38`), which spec Q6 forbids calling.
    //                   The rows stay and the wording stays; the handler says
    //                   the address is not configured instead of opening nothing.
    ("about.title", "Software Introduction"),
    ("about.description", "A desktop dashboard for Windows Subsystem for Android: start and stop the subsystem, see what it is using, install packages onto it, and forward ports to and from it. Built with Rust and Slint."),
    ("about.version", "Version"),
    ("about.updates", "(Check for updates)"),
    ("about.check_update", "Checking for updates..."),
    ("about.website", "Project website"),
    ("about.repository", "Project repository"),
    ("about.group", "Join the official communication group"),
    ("about.issues", "Issues (Submit bugs, request new features)"),
    ("about.announcements", "Project announcements"),
    ("about.talk", "Project discussions"),
    ("about.documents", "Project documents"),
    ("about.star", "If something here was useful, or something was not, an issue is the fastest way to say so. Reports of a wrong number on a page are as welcome as requests for a new one."),
    ("about.love", "Labor of love"),
    ("about.license", "GPL-3.0 license"),
    ("about.copyright", "Copyright © {0} WSA Dashboard Contributors\nFree & open source · Beware of unofficial paid versions"),
    ("about.privacy_policy", "Privacy Policy"),
    ("about.terms_of_service", "Terms of Service"),
    ("about.new_version", "A new version is available: v{0}"),
    // The two sentences the reference does not have, because its check answered
    // in a dialog with the release notes rather than in one line. Ours says one
    // of three things and stops.
    ("about.up_to_date", "This build is the latest release."),
    ("about.update_check_failed", "Could not check for updates: {0}"),
    // Ours, and they exist because the rows they answer for have no address in
    // this build. `about.group`, `about.talk`, `about.documents` and
    // `about.announcements` all came from a service spec Q6 forbids, so the
    // click on those rows gets one of these two sentences instead of opening
    // nothing.
    ("about.link_not_configured", "No address is configured for this build."),
    ("about.update_check_unavailable", "No project repository is configured for this build."),
    ("settings.log_levels.error", "Error level"),
    ("settings.log_levels.warn", "Warning level"),
    ("settings.log_levels.info", "Information level"),
    ("settings.log_levels.debug", "Debug level"),
    ("settings.log_levels.trace", "Trace level"),
    ("settings.log_retention_options.seven_days", "seven days"),
    ("settings.log_retention_options.half_month", "a half month"),
    ("settings.log_retention_options.one_month", "one month"),
    ("settings.update_options.daily", "Every day"),
    ("settings.update_options.weekly", "Every week"),
    ("settings.update_options.biweekly", "Every two weeks"),
    ("settings.update_options.monthly", "Every month"),
    ("dialog.save", "Save"),
    // The tray menu. Two items, and the wording is the reference's verbatim --
    // which is why neither is in [`OURS`]: the reference's tray also shows the
    // window and exits, so there is no sentence of ours for a pack to overwrite.
    ("tray.show_window", "Show Window"),
    ("tray.exit", "Exit"),
];

/// Keys whose wording is **ours**, so a translation must not overwrite it.
///
/// Not a judgement call written by hand: `ours_list_is_exactly_the_keys_whose
/// wording_changed` compares [`ENGLISH`] against the reference's own
/// `en.toml` and fails if this list and that difference differ in either
/// direction. Add a key here when a sentence changes and the test will hold you
/// to it; leave it out by accident and the test says which key.
///
/// `tools/derive-ours-keys.py` regenerates this list from the two files.
pub static OURS: &[&str] = &[
    "about.copyright",
    "about.description",
    "about.star",
    "add.create",
    "add.note",
    "common.app_name",
    "dialog.delete_msg",
    "dialog.delete_title",
    "dialog.http_proxy_helper_desc1",
    "dialog.http_proxy_helper_desc2",
    "dialog.info_title",
    "dialog.stop_wsl_success",
    "main.distro_not_found",
    "main.loading",
    "mount_disk.mount",
    "mount_disk.mount_btn",
    "mount_disk.no_mounted",
    "network.proxy_error_invalid",
    "network.proxy_error_port",
    "network.proxy_save_success",
    "network.target_distro",
    "operation.deleting",
    "scheduler.cron_error_range",
    "scheduler.error_name_invalid",
    "settings.auto_shutdown_msg",
    "settings.backup_dir",
    "settings.distro_dir",
    "settings.log_retention_options.half_month",
    "settings.mail_icon_always",
    "settings.mount_auto_probe",
    "settings.mount_auto_probe_desc",
    "settings.saved_failed",
    "settings.saved_success",
    "settings.select_backup_dir",
    "settings.select_distro_dir",
    "settings.select_folder",
    "settings.select_log_dir",
    "settings.show_drag_icon",
    "settings.sparse_vhd",
    "settings.sparse_vhd_desc",
    "settings.stop_wsl",
    "settings.tray_close_to_tray",
    "settings.tray_start_minimized",
    "settings.update_wsl",
    "settings.wsl2_required",
    "settings.wsl_settings",
    "sidebar.add",
    "sidebar.mount",
    "sidebar.usb",
    "toast.distro_busy",
    "usb.attach",
    "usb.detach",
    "usb.device_info",
    "usb.empty",
    "usb.loading",
    "usb.not_found_error",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The loader mutates two process-wide values, so every test that calls it
    /// runs under this. Tests in one binary run on many threads; without it a
    /// `load_resources("ja")` in one test is a `t()` answering in Japanese in
    /// another, and the failure looks like a translation bug.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn serialized() -> std::sync::MutexGuard<'static, ()> {
        // A poisoned lock means an earlier test panicked while holding it; the
        // state it guards is still just two maps, and cascading that into every
        // other test would hide the original failure.
        SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn english_value(key: &str) -> String {
        ENGLISH
            .iter()
            .find(|(candidate, _)| *candidate == key)
            .map(|(_, value)| (*value).to_string())
            .unwrap_or_default()
    }

    // -- the shipped packs ---------------------------------------------------

    /// `assets/i18n/en.toml` is generated from [`ENGLISH`]; if the table is
    /// edited and the file is not regenerated, the file is what the program
    /// actually shows and the table is what a reader believes. One test, both
    /// directions.
    #[test]
    fn the_generated_english_pack_matches_the_table() {
        let raw = Asset::get("en.toml").expect("assets/i18n/en.toml is embedded");
        let text = String::from_utf8_lossy(raw.data.as_ref());
        let parsed = toml::from_str::<toml::Table>(text.as_ref()).expect("en.toml parses");
        let mut from_file = HashMap::new();
        flatten("", &Value::Table(parsed), &mut from_file);

        let from_table: HashMap<String, String> = ENGLISH
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();

        assert_eq!(
            from_file.len(),
            from_table.len(),
            "en.toml has {} keys, the table has {}",
            from_file.len(),
            from_table.len()
        );
        for (key, value) in &from_table {
            assert_eq!(
                from_file.get(key),
                Some(value),
                "en.toml and ENGLISH disagree about {key}"
            );
        }
    }

    /// Every language this build claims to offer has a file behind it.
    #[test]
    fn every_listed_language_has_a_pack() {
        let listed = available_languages();
        assert!(
            listed.len() >= 45,
            "expected the reference's 50 packs, found {}",
            listed.len()
        );
        for name in &listed {
            assert!(
                Asset::get(&format!("{name}.toml")).is_some(),
                "{name} is listed but has no pack"
            );
        }
    }

    /// The picker shows this list in this order, so a filesystem enumeration
    /// order would move entries under the reader's finger between runs.
    #[test]
    fn the_language_list_is_sorted() {
        let listed = available_languages();
        let mut sorted = listed.clone();
        sorted.sort();
        assert_eq!(listed, sorted);
        assert!(listed.contains(&"en".to_string()), "English is not offered");
        assert!(
            listed.contains(&"zh-CN".to_string()) && listed.contains(&"zh-TW".to_string()),
            "both Chinese packs should be shipped and separately selectable"
        );
    }

    // -- the OURS list, checked against the reference's own English ---------

    /// The loader's one judgement, and the one place a mistake silently
    /// changes what a user reads. A key that belongs here and is missing is a
    /// translated sentence about a different program; a key that does not
    /// belong and is listed is a label that stays English for no reason.
    #[test]
    fn ours_list_is_exactly_the_keys_whose_wording_changed() {
        let upstream = toml::from_str::<toml::Table>(
            include_str!("../../docs/reference/assets/i18n/en.toml"),
        )
        .expect("the reference's en.toml parses");
        let mut theirs = HashMap::new();
        flatten("", &Value::Table(upstream), &mut theirs);

        let mut should_be_ours: Vec<&str> = Vec::new();
        for (key, value) in ENGLISH {
            match theirs.get(*key) {
                // No upstream key at all: ours by construction. Not listed,
                // because no pack can overwrite it and listing it would only
                // mean the test below had to know that.
                None => {}
                Some(theirs) if theirs == *value => {}
                Some(_) => should_be_ours.push(key),
            }
        }
        should_be_ours.sort_unstable();

        let mut listed: Vec<&str> = OURS.to_vec();
        listed.sort_unstable();

        assert_eq!(
            should_be_ours, listed,
            "OURS is out of step with the wording; run tools/derive-ours-keys.py"
        );
    }

    // -- normalisation -------------------------------------------------------

    /// Windows hands out regional tags and underscores; a locale we do not
    /// ship must still land on something readable.
    #[test]
    fn normalizes_the_locales_a_user_would_actually_have() {
        assert_eq!(normalize_language_code("en"), "en");
        assert_eq!(normalize_language_code("EN-US"), "en");
        assert_eq!(normalize_language_code("zh_CN"), "zh-CN");
        assert_eq!(normalize_language_code("zh-TW"), "zh-TW");
        assert_eq!(normalize_language_code("zh-Hant-HK"), "zh-TW");
        assert_eq!(normalize_language_code("pt-BR"), "pt");
        assert_eq!(normalize_language_code("nb-NO"), "no");
        assert_eq!(normalize_language_code("tl-PH"), "fil");
        assert_eq!(normalize_language_code(""), "en");
        assert_eq!(
            normalize_language_code("kl-GL"),
            "en",
            "a language we do not ship must fall back to English, not to itself"
        );
    }

    /// Every shipped name normalises to itself. If a pack ever cannot be
    /// selected -- which is the whole job of the picker -- this is what says so.
    #[test]
    fn every_shipped_pack_normalizes_to_itself() {
        let _guard = serialized();
        for name in available_languages() {
            assert_eq!(
                normalize_language_code(&name),
                name,
                "{name} is shipped but cannot be selected"
            );
        }
    }

    #[test]
    fn rtl_languages_are_the_reference_five() {
        for lang in ["ar", "ar-EG", "he", "he-IL", "fa", "fa-IR", "ur", "sd"] {
            assert!(is_rtl(lang), "{lang} should be RTL");
        }
        for lang in ["en", "zh-CN", "zh-TW", "ja", "de"] {
            assert!(!is_rtl(lang), "{lang} should not be RTL");
        }
    }

    // -- lookup --------------------------------------------------------------

    /// The loader's job, end to end: a key the reference and we agree on is
    /// translated; a key whose wording is ours is not.
    #[test]
    fn a_translated_key_shows_the_translation_and_ours_does_not() {
        let _guard = serialized();
        load_resources("en");
        let shared = "sidebar.home";
        let english = english_value(shared);
        assert!(english == "Home", "fixture changed: {english:?}");

        load_resources("ja");
        assert_eq!(current_lang(), "ja");
        assert_eq!(available_languages().contains(&"ja".to_string()), true);
        assert_ne!(
            t(shared),
            english,
            "a key both projects word the same way should be translated"
        );

        // Ours. The Japanese pack says something here; it describes deleting a
        // WSL distribution, and our page uninstalls an APK.
        assert!(
            Asset::get("ja.toml")
                .is_some_and(|raw| String::from_utf8_lossy(raw.data.as_ref()).contains("deleting")),
            "fixture changed: ja.toml no longer mentions deleting"
        );
        assert_eq!(
            t("operation.deleting"),
            english_value("operation.deleting"),
            "our own wording must survive a language change"
        );
    }

    /// A key no pack has -- ours only, 27 of them -- has to keep working in
    /// every language, and in a language we do not ship.
    #[test]
    fn a_key_no_pack_has_falls_back_to_our_english_everywhere() {
        let _guard = serialized();
        for lang in ["en", "ja", "zh-CN", "kl-GL"] {
            load_resources(lang);
            assert_eq!(
                t("network.dir_forward"),
                english_value("network.dir_forward"),
                "ours-only key under {lang}"
            );
        }
    }

    /// A missing key shows itself, which is how a typo gets noticed.
    #[test]
    fn an_unknown_key_shows_itself() {
        let _guard = serialized();
        load_resources("ja");
        assert_eq!(t("no.such.key"), "no.such.key");
    }

    /// Placeholders still fill, and over-supplying them is not an error.
    #[test]
    fn placeholders_still_fill() {
        let _guard = serialized();
        load_resources("en");
        assert_eq!(
            tr("toast.distro_busy", &["Foo".into(), "Starting ...".into()]),
            "Foo is busy (Starting ...)"
        );
        assert_eq!(
            tr("toast.distro_busy", &["Foo".into()]),
            "Foo is busy ({1})",
            "fewer arguments must not panic"
        );
    }

    /// An empty value in a pack means "not translated". Writing it through
    /// would blank a label that English fills correctly, which reads as a bug
    /// in a language the user never chose deliberately.
    #[test]
    fn an_empty_pack_value_does_not_blank_a_label() {
        let parsed = toml::from_str::<toml::Table>(
            "[demo]\nfilled = \"translated\"\nblank = \"\"\n",
        )
        .expect("fixture parses");
        let mut map = HashMap::new();
        flatten("", &Value::Table(parsed), &mut map);
        assert_eq!(map.get("demo.filled"), Some(&"translated".to_string()));
        assert!(!map.contains_key("demo.blank"), "an empty string was stored");
    }

    /// More arguments than placeholders is not an error: a translation that
    /// outgrew its caller must not crash the window over it.
    #[test]
    fn too_many_arguments_is_not_an_error() {
        let _guard = serialized();
        load_resources("en");
        assert_eq!(
            tr("toast.distro_busy", &["Foo".into(), "Starting".into(), "extra".into()]),
            "Foo is busy (Starting)"
        );
    }

    /// A duplicate key resolves to the **first** match, because the lookup is a
    /// `find`. A second row for the same key therefore does nothing at all, and
    /// the table reads as though the new text were live. It bit twice already --
    /// `operation.deleting` had to be edited in place because adding a row for
    /// it would have left the old sentence showing -- and with
    /// `assets/i18n/en.toml` generated from this table it would now also make
    /// the generated file fail to parse.
    #[test]
    fn every_key_is_listed_once() {
        let mut seen = std::collections::HashSet::new();
        for (key, _) in ENGLISH {
            assert!(seen.insert(*key), "{key} appears more than once in ENGLISH");
        }
    }

    /// An empty or whitespace-only value renders as a blank label, and a blank
    /// label is a bug the reader cannot act on.
    #[test]
    fn no_key_is_empty_or_whitespace() {
        for (key, value) in ENGLISH {
            assert!(
                !value.trim().is_empty(),
                "{key} has no text; delete the row rather than ship an empty label"
            );
        }
    }

    /// `[sidebar]` plus `sidebar.home` would make a key that no pack has, so
    /// every call site would render the key itself. The key *name* is the
    /// reference's, which means it already carries its section.
    #[test]
    fn no_key_repeats_its_section() {
        for (key, _) in ENGLISH {
            let Some((section, rest)) = key.split_once('.') else {
                continue;
            };
            assert!(
                !rest.starts_with(&format!("{section}.")),
                "{key} looks like the section is written twice"
            );
        }
    }
    /// Every key a `.slint` file asks for has to be in the table. Kept from P1:
    /// it is the test that notices a new dialog was written with a key nobody
    /// defined.
    #[test]
    fn every_key_a_slint_file_asks_for_is_in_the_table() {
        let mut checked = 0usize;
        for entry in slint_file_paths("src/ui") {
            let text = std::fs::read_to_string(&entry).expect("slint file reads");
            for key in asked_for_keys(&text) {
                assert!(
                    ENGLISH.iter().any(|(candidate, _)| *candidate == key),
                    "{} asks for a key the table does not have: {key}",
                    entry.display()
                );
                checked += 1;
            }
        }
        assert!(checked > 100, "only {checked} keys checked; the scan found nothing");
    }

    fn slint_file_paths(root: &str) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![std::path::PathBuf::from(root)];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|ext| ext == "slint") {
                    out.push(path);
                }
            }
        }
        out.sort();
        out
    }

    /// Every `AppI18n.t("...")` literal in a `.slint` file.
    fn asked_for_keys(text: &str) -> Vec<String> {
        let mut keys = Vec::new();
        let mut rest = text;
        while let Some(at) = rest.find("AppI18n.t(\"") {
            let after = &rest[at + "AppI18n.t(\"".len()..];
            let Some(end) = after.find('"') else { break };
            keys.push(after[..end].to_string());
            rest = &after[end..];
        }
        keys
    }
}