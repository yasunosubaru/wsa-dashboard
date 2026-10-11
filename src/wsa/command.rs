// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The adb command surface: one enum, one place that decides argv, timeout
//! tier and whether a command mutates state.
//!
//! Only commands that actually run through `adb` live here. Spec 2.3 lists
//! `LaunchApp`, `StartSubsystem`, `StopSubsystem`, `OpenSettings` and
//! `VhdxInfo` in the same table, but those activate a protocol, kill a
//! process, shell out to explorer and read a local vhdx respectively -- none
//! of them has an adb argv, and none of them reaches the executor (spec 1.2 /
//! 1.3; Task 11 wires them directly).
//!
//! TDD step 1 (RED): `WsaCommand` is not implemented yet.

#![allow(dead_code)]

/// One adb command, as seen by the executor.
///
/// `argv()` is everything after the adb executable; the executor supplies the
/// executable path. Classification (`is_write` / `is_heavy`) exists because
/// spec 2.2 times out in three tiers -- read 10s, write 45s, heavy 1800s --
/// and the tier has to be a property of the command rather than a decision
/// each call site re-derives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsaCommand {
    /// `adb connect 127.0.0.1:<port>`
    ConnectAdb { port: u16 },
    /// `adb devices`
    ListAdbDevices,
    /// `adb shell getprop sys.boot_completed` -- the single health truth
    /// (spec 1.4; `mCurrentFocus` is not usable).
    ProbeReady,
    /// `adb shell getprop`, for the device page.
    GetProps,
    /// `adb shell pm list packages [-3]`
    ListPackages { include_system: bool },
    /// `adb shell ps -A` -- per-app running state.
    ///
    /// The reference gets this fact for free from the `Running`/`Stopped`
    /// column of `wsl -l -v`; Android has no equivalent listing, and without
    /// it the home card's start/stop toggle could only ever say "start".
    /// Verified against a real device 2026-10-11.
    ListProcesses,
    /// `adb shell dumpsys package <package>`
    PackageInfo { package: String },
    /// `adb shell am force-stop <package>` -- how an app is actually stopped.
    AmForceStop { package: String },
    /// `adb install -r <path>`
    InstallApk { path: String },
    /// `adb uninstall <package>`
    UninstallApk { package: String },
    /// `adb forward tcp:<local> tcp:<remote>`; `local` is the host port.
    ForwardAdd { local: u16, remote: u16 },
    /// `adb forward --remove tcp:<local>`
    ForwardRemove { local: u16 },
    /// `adb reverse tcp:<device> tcp:<host>`; `device` is the Android port.
    ReverseAdd { device: u16, host: u16 },
    /// `adb reverse --remove tcp:<device>`
    ReverseRemove { device: u16 },
    /// `adb shell <args...>`: the escape hatch for anything not worth its own
    /// variant.
    Shell { args: Vec<String> },
}

impl WsaCommand {
    /// Arguments following the adb executable.
    ///
    /// Nothing is re-quoted or joined here: `Command` passes each `String` as
    /// one argument, so an apk path containing spaces comes out the other end
    /// intact.
    pub fn argv(&self) -> Vec<String> {
        match self {
            Self::ConnectAdb { port } => vec!["connect".into(), format!("127.0.0.1:{port}")],
            Self::ListAdbDevices => vec!["devices".into()],
            Self::ProbeReady => vec!["shell".into(), "getprop".into(), "sys.boot_completed".into()],
            Self::GetProps => vec!["shell".into(), "getprop".into()],
            Self::ListPackages { include_system } => {
                let mut argv = vec!["shell".into(), "pm".into(), "list".into(), "packages".into()];
                // `-3` is third-party only; the home page must not be handed
                // a wall of platform packages.
                if !include_system {
                    argv.push("-3".into());
                }
                argv
            }
            Self::PackageInfo { package } => {
                vec!["shell".into(), "dumpsys".into(), "package".into(), package.clone()]
            }
            Self::ListProcesses => vec!["shell".into(), "ps".into(), "-A".into()],
            Self::AmForceStop { package } => {
                vec!["shell".into(), "am".into(), "force-stop".into(), package.clone()]
            }
            Self::InstallApk { path } => vec!["install".into(), "-r".into(), path.clone()],
            Self::UninstallApk { package } => vec!["uninstall".into(), package.clone()],
            Self::ForwardAdd { local, remote } => {
                vec!["forward".into(), format!("tcp:{local}"), format!("tcp:{remote}")]
            }
            Self::ForwardRemove { local } => {
                vec!["forward".into(), "--remove".into(), format!("tcp:{local}")]
            }
            Self::ReverseAdd { device, host } => {
                vec!["reverse".into(), format!("tcp:{device}"), format!("tcp:{host}")]
            }
            Self::ReverseRemove { device } => {
                vec!["reverse".into(), "--remove".into(), format!("tcp:{device}")]
            }
            Self::Shell { args } => {
                let mut argv = Vec::with_capacity(args.len() + 1);
                argv.push("shell".into());
                argv.extend(args.iter().cloned());
                argv
            }
        }
    }

    /// Whether the command mutates state, and so gets the 45s tier.
    ///
    /// `Shell` is a write unless it is explicitly a listing. Guessing "read"
    /// for an unknown shell would hand it the 10s tier and time out mid-run,
    /// so the safe default is the slow one.
    pub fn is_write(&self) -> bool {
        match self {
            Self::ListAdbDevices
            | Self::ProbeReady
            | Self::GetProps
            | Self::ListProcesses
            | Self::PackageInfo { .. }
            | Self::ListPackages { .. } => false,
            Self::Shell { args } => !args.iter().any(|arg| arg == "--list"),
            _ => true,
        }
    }

    /// Whether the command is long-running enough for the 1800s tier and the
    /// heavy-operation lock. Installing and uninstalling are the only ones:
    /// both wait on the package manager, not on adb.
    pub fn is_heavy(&self) -> bool {
        matches!(self, Self::InstallApk { .. } | Self::UninstallApk { .. })
    }

    /// One-line human label for logs and the operation toast.
    ///
    /// Always a single line by construction -- a toast that has been split in
    /// two by an argument containing a newline looks broken.
    pub fn display(&self) -> String {
        match self {
            Self::ConnectAdb { port } => format!("connect 127.0.0.1:{port}"),
            Self::ListAdbDevices => "list adb devices".into(),
            Self::ProbeReady => "probe readiness".into(),
            Self::GetProps => "read device properties".into(),
            Self::ListPackages { include_system } => {
                if *include_system {
                    "list all packages".into()
                } else {
                    "list third-party packages".into()
                }
            }
            Self::PackageInfo { package } => format!("package info {package}"),
            Self::ListProcesses => "list running processes".into(),
            Self::AmForceStop { package } => format!("force-stop {package}"),
            Self::InstallApk { path } => format!("install {}", squeeze(path)),
            Self::UninstallApk { package } => format!("uninstall {package}"),
            Self::ForwardAdd { local, remote } => format!("forward tcp:{local} to tcp:{remote}"),
            Self::ForwardRemove { local } => format!("remove forward tcp:{local}"),
            Self::ReverseAdd { device, host } => format!("reverse tcp:{device} to tcp:{host}"),
            Self::ReverseRemove { device } => format!("remove reverse tcp:{device}"),
            Self::Shell { args } => format!("shell {}", squeeze(&args.join(" "))),
        }
    }
}

/// Collapse whitespace runs and trim, so a label can never span lines.
fn squeeze(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::WsaCommand;

    // -- classification: this is what picks the timeout tier --------------

    #[test]
    fn reads_are_not_writes() {
        for cmd in [
            WsaCommand::ListAdbDevices,
            WsaCommand::ProbeReady,
            WsaCommand::GetProps,
            WsaCommand::ListProcesses,
            WsaCommand::PackageInfo {
                package: "com.x".into(),
            },
            WsaCommand::ListPackages {
                include_system: false,
            },
        ] {
            assert!(!cmd.is_write(), "{cmd:?} should be a read");
            assert!(!cmd.is_heavy(), "{cmd:?} should not be heavy");
        }
    }

    #[test]
    fn mutations_are_writes() {
        for cmd in [
            WsaCommand::ConnectAdb { port: 58526 },
            WsaCommand::AmForceStop {
                package: "com.x".into(),
            },
            WsaCommand::ForwardAdd {
                local: 18081,
                remote: 18081,
            },
            WsaCommand::ReverseRemove { device: 18080 },
            WsaCommand::Shell {
                args: vec!["settings".into(), "put".into()],
            },
        ] {
            assert!(cmd.is_write(), "{cmd:?} should be a write");
        }
    }

    /// Spec 2.2 puts install/uninstall in the 1800s tier, not the 45s one.
    #[test]
    fn only_install_and_uninstall_are_heavy() {
        assert!(WsaCommand::InstallApk {
            path: "a.apk".into()
        }
        .is_heavy());
        assert!(WsaCommand::UninstallApk {
            package: "com.x".into()
        }
        .is_heavy());

        assert!(!WsaCommand::AmForceStop {
            package: "com.x".into()
        }
        .is_heavy());
    }

    /// An arbitrary `shell` is treated as a write unless it is explicitly a
    /// listing: guessing "read" for an unknown command would hand it the 10s
    /// tier and time out mid-run.
    #[test]
    fn shell_is_a_write_unless_it_lists() {
        assert!(WsaCommand::Shell {
            args: vec!["rm".into(), "-rf".into(), "/sdcard/x".into()]
        }
        .is_write());
        assert!(!WsaCommand::Shell {
            args: vec!["forward".into(), "--list".into()]
        }
        .is_write());
        assert!(!WsaCommand::Shell {
            args: vec!["reverse".into(), "--list".into()]
        }
        .is_write());
    }

    // -- argv: executor prepends the adb exe itself -----------------------

    #[test]
    fn argv_of_connect() {
        assert_eq!(
            WsaCommand::ConnectAdb { port: 58526 }.argv(),
            vec!["connect", "127.0.0.1:58526"]
        );
    }

    #[test]
    fn argv_of_probe_is_the_single_health_truth() {
        assert_eq!(
            WsaCommand::ProbeReady.argv(),
            vec!["shell", "getprop", "sys.boot_completed"]
        );
        assert_eq!(WsaCommand::GetProps.argv(), vec!["shell", "getprop"]);
        assert_eq!(WsaCommand::ListAdbDevices.argv(), vec!["devices"]);
    }

    /// `-3` means third-party only. The flag is on the variant, not decided
    /// by the caller, so the home page cannot accidentally list system apps.
    #[test]
    fn argv_of_package_listing() {
        assert_eq!(
            WsaCommand::ListPackages {
                include_system: true
            }
            .argv(),
            vec!["shell", "pm", "list", "packages"]
        );
        assert_eq!(
            WsaCommand::ListPackages {
                include_system: false
            }
            .argv(),
            vec!["shell", "pm", "list", "packages", "-3"]
        );
    }

    #[test]
    fn argv_of_package_ops() {
        assert_eq!(
            WsaCommand::PackageInfo {
                package: "com.topjohnwu.magisk".into()
            }
            .argv(),
            vec!["shell", "dumpsys", "package", "com.topjohnwu.magisk"]
        );
        assert_eq!(
            WsaCommand::AmForceStop {
                package: "com.x".into()
            }
            .argv(),
            vec!["shell", "am", "force-stop", "com.x"]
        );
        assert_eq!(
            WsaCommand::InstallApk {
                path: r"C:\a b\c.apk".into()
            }
            .argv(),
            vec!["install", "-r", r"C:\a b\c.apk"]
        );
        assert_eq!(
            WsaCommand::UninstallApk {
                package: "com.x".into()
            }
            .argv(),
            vec!["uninstall", "com.x"]
        );
    }

    #[test]
    fn argv_of_port_rules() {
        assert_eq!(
            WsaCommand::ForwardAdd {
                local: 18081,
                remote: 18081
            }
            .argv(),
            vec!["forward", "tcp:18081", "tcp:18081"]
        );
        assert_eq!(
            WsaCommand::ForwardRemove { local: 18081 }.argv(),
            vec!["forward", "--remove", "tcp:18081"]
        );
        assert_eq!(
            WsaCommand::ReverseAdd {
                device: 18080,
                host: 18080
            }
            .argv(),
            vec!["reverse", "tcp:18080", "tcp:18080"]
        );
        assert_eq!(
            WsaCommand::ReverseRemove { device: 18080 }.argv(),
            vec!["reverse", "--remove", "tcp:18080"]
        );
    }

    /// `shell` must forward every argument verbatim -- the executor never
    /// re-quotes, so an apk path with spaces survives intact. Note `shell`
    /// itself is prepended here: `argv()` is what follows the adb exe.
    #[test]
    fn argv_of_shell_passes_args_through() {
        assert_eq!(
            WsaCommand::Shell {
                args: vec!["ls".into(), "-l".into(), "/data".into()]
            }
            .argv(),
            vec!["shell", "ls", "-l", "/data"]
        );
        assert_eq!(WsaCommand::Shell { args: vec![] }.argv(), vec!["shell"]);
    }

    // -- per-app process listing: the home card's Running/Stopped ---------

    /// `ps -A` was verified on the local device (2026-10-11): 75 lines
    /// including the header, and a running app shows up as the last column.
    /// The reference reads `Running`/`Stopped` out of `wsl -l -v` for free;
    /// we have to ask Android for the same fact.
    #[test]
    fn argv_of_process_listing() {
        assert_eq!(
            WsaCommand::ListProcesses.argv(),
            vec!["shell", "ps", "-A"]
        );
    }

    /// A listing is a read, so it gets the 10 s tier rather than 45 s; the
    /// home page polls it and must not sit for 45 s when adb is wedged.
    #[test]
    fn process_listing_is_a_read_not_a_write() {
        assert!(!WsaCommand::ListProcesses.is_write());
        assert!(!WsaCommand::ListProcesses.is_heavy());
    }

    // -- display: goes into logs and the operation toast ------------------

    #[test]
    fn display_names_every_variant() {
        for cmd in [
            WsaCommand::ConnectAdb { port: 58526 },
            WsaCommand::ListAdbDevices,
            WsaCommand::ProbeReady,
            WsaCommand::GetProps,
            WsaCommand::ListPackages {
                include_system: false,
            },
            WsaCommand::PackageInfo {
                package: "com.x".into(),
            },
            WsaCommand::ListProcesses,
            WsaCommand::AmForceStop {
                package: "com.x".into(),
            },
            WsaCommand::InstallApk {
                path: "a.apk".into(),
            },
            WsaCommand::UninstallApk {
                package: "com.x".into(),
            },
            WsaCommand::ForwardAdd {
                local: 1,
                remote: 2,
            },
            WsaCommand::ForwardRemove { local: 1 },
            WsaCommand::ReverseAdd {
                device: 1,
                host: 2,
            },
            WsaCommand::ReverseRemove { device: 1 },
            WsaCommand::Shell {
                args: vec!["id".into()],
            },
        ] {
            let shown = cmd.display();
            assert!(!shown.is_empty(), "{cmd:?} has an empty display");
            assert!(!shown.contains('\n'), "{shown:?} spans lines");
        }
    }
}
