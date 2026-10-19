// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The tray icon.
//!
//! The counterpart of the reference's `src/app/tray.rs` (100 lines). The shape is
//! the same -- a menu, a tooltip, a static that owns the icon, and one polling
//! timer that drains both event queues -- with three differences worth naming.
//!
//! # The menu is data, not two call sites
//!
//! The reference writes each id twice: once in `MenuItem::with_id("show", ...)`
//! and once in `match event.id.as_ref() { "show" => ... }`. A rename in either
//! place gives a menu item that renders correctly and does nothing when clicked.
//! Here the rows come from [`tray_model::entries`] and the match arms use the same
//! constants, so there is one spelling of each id in the program.
//!
//! # The icon is embedded, not a Win32 resource
//!
//! The reference's first choice is `Icon::from_resource(1, None)`, which works
//! because its `build.rs` embeds an icon into the executable. Ours does not --
//! that is P8's block, and `build.rs` says so -- so a resource lookup here would
//! fail on every machine until then. `assets/logo/mini.png` is in the repository
//! already, so it is compiled in and decoded at startup instead. When P8 lands the
//! resource, this can switch to the reference's order without changing anything
//! else.
//!
//! # The window operations are not here
//!
//! The reference's tray calls `crate::app::window::{set_skip_taskbar,
//! show_and_center}`. Ours calls into `ui::handlers::window` for the two that need
//! an `AppWindow`, because `app/window.rs` is deliberately free of the Slint types
//! (see its header). `set_skip_taskbar` itself stays in `app/window.rs`, since it
//! needs nothing but the handle.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use once_cell::sync::Lazy;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};
use tracing::{error, info};

use crate::app::constants::APP_NAME;
use crate::app::tray_model::{self, EXIT_ID, SHOW_ID};
use crate::i18n;
use crate::AppWindow;

/// `tray-icon`'s handle is not `Send`/`Sync` on Windows -- it holds `Rc`/`RefCell`
/// internally -- but it is only ever touched from the UI thread, which is the
/// thread that owns the Slint event loop and the timer below. The reference wraps
/// it the same way and for the same reason.
struct TrayIconHandle(#[allow(dead_code)] TrayIcon);

unsafe impl Send for TrayIconHandle {}
unsafe impl Sync for TrayIconHandle {}

/// The one live tray icon.
///
/// Replacing it drops the previous one, and dropping a `TrayIcon` is what removes
/// it from the notification area. That is how a language change gets a rebuilt
/// tooltip without leaving the old icon behind next to the new one.
static TRAY_ICON: Lazy<Mutex<Option<TrayIconHandle>>> = Lazy::new(|| Mutex::new(None));

/// Whether the polling timer exists.
///
/// `slint::Timer` cannot be restarted once started, so the timer is created once
/// and the re-init path only replaces the icon. A second timer would drain the
/// same two queues and each event would be handled twice -- a double-click that
/// hides and then shows again, which reads as nothing happening.
static POLLING: AtomicBool = AtomicBool::new(false);

pub struct SystemTray;

impl SystemTray {
    /// Build the icon, put it in the tray, and start polling its events.
    ///
    /// `initially_visible` is the window's state at the moment of the call, not a
    /// preference: a re-init after a language change must not make a hidden window
    /// visible again.
    pub fn initialize(
        app_weak: slint::Weak<AppWindow>,
        initially_visible: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("initialising the tray icon");

        let icon = load_icon()?;

        let menu = Menu::new();
        let entries = tray_model::entries();
        for (index, entry) in entries.iter().enumerate() {
            // A separator before the last row: the reference's layout is show,
            // separator, exit, and the quit row is the one that should not be hit
            // by aiming for the row above it.
            if index + 1 == entries.len() {
                menu.append(&PredefinedMenuItem::separator())?;
            }
            let label = i18n::t(entry.label_key);
            menu.append(&MenuItem::with_id(entry.id, label, entry.enabled, None))?;
        }

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            // Left click does not open the menu, the reference's choice: a reader
            // who left-clicks an icon expects the window, and the double-click
            // handler below is what gives them that.
            .with_menu_on_left_click(false)
            .with_tooltip(tray_model::tooltip(APP_NAME, env!("CARGO_PKG_VERSION")))
            .with_icon(icon)
            .build()?;

        {
            let mut slot = TRAY_ICON
                .lock()
                .map_err(|_| "the tray icon slot is poisoned")?;
            *slot = Some(TrayIconHandle(tray));
        }

        if let Some(app) = app_weak.upgrade() {
            app.set_is_window_visible(initially_visible);
        }

        start_polling(app_weak);
        Ok(())
    }
}

/// Decode the embedded PNG into the RGBA buffer `tray-icon` wants.
///
/// `include_bytes!` rather than a path: the executable is shipped on its own (spec
/// Q3), so a file next to it is a file that will not be there.
fn load_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    const PNG: &[u8] = include_bytes!("../../assets/logo/mini.png");
    let decoded = image::load_from_memory(PNG)?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height).map_err(|error| format!("{error}").into())
}

/// Drain the tray's two event queues on the UI thread.
///
/// A timer rather than a thread, because both queues deliver to the thread that
/// owns the icon and the window operations have to run on the Slint thread. 100 ms
/// is the reference's interval: fast enough that a click feels immediate, slow
/// enough that an idle tray costs nothing measurable.
fn start_polling(app_weak: slint::Weak<AppWindow>) {
    // The bisection's last periodic wake (MAPPING §20.12): with the switch the
    // icon still shows, but double-click and the menu go dead for that run --
    // what an experiment is for, and nothing else reads the flag.
    if std::env::var("WSADASH_EXP_NO_TRAY").as_deref() == Ok("1") {
        return;
    }

    if POLLING.swap(true, Ordering::SeqCst) {
        return;
    }

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                // A left click or a double-click brings the window back. Both,
                // because Windows delivers `Click` twice before `DoubleClick`:
                // a toggle on double-click would hide a window the reader just
                // asked to see. Hiding stays on the window's own close button --
                // a tray icon that hides a window is how the tray gets called
                // broken, which is exactly what happened when only the
                // double-click was wired and a single click did nothing.
                let show = match &event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } => true,
                    TrayIconEvent::DoubleClick { .. } => true,
                    _ => false,
                };
                if show {
                    show_window(&app_weak);
                }
            }

            while let Ok(event) = MenuEvent::receiver().try_recv() {
                match event.id.as_ref() {
                    SHOW_ID => {
                        if let Some(app) = app_weak.upgrade() {
                            info!("tray menu: show the window");
                            crate::ui::handlers::window::show_and_center(&app);
                        }
                    }
                    EXIT_ID => {
                        info!("tray menu: exit");
                        let _ = slint::quit_event_loop();
                    }
                    other => {
                        // Cannot happen while `entries` is the only source of
                        // ids, which is the point of routing both through it --
                        // but a silent drop would be the same bug the reference
                        // has, so it is logged.
                        error!(id = other, "tray menu: an id nothing handles");
                    }
                }
            }
        },
    );

    // The timer has to outlive this call: a `slint::Timer` stops when it is
    // dropped. The reference leaks it the same way, and there is exactly one for
    // the life of the process.
    std::mem::forget(timer);
}

/// A click on the icon: bring the window back.
///
/// Not a toggle. The reader clicked the icon to get the window; hiding it is
/// the window's own close button's job. The old handler toggled on
/// double-click only, so the first gesture most readers try -- a single click
/// -- did nothing at all.
fn show_window(app_weak: &slint::Weak<AppWindow>) {
    let Some(app) = app_weak.upgrade() else { return };
    info!("tray click: showing the window");
    crate::ui::handlers::window::show_and_center(&app);
}
