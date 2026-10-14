// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The network page's own work: read adb's two forwarding tables, delete a
//! rule from one of them, and set or clear Android's global HTTP proxy.
//!
//! The counterpart of the reference's `src/ui/handlers/network/`, which is four
//! files (`mod`, `rules`, `proxy`, `service`, `utils`) totalling most of a
//! thousand lines. Ours is one file, and the reason is not tidiness: almost all
//! of that code exists to drive `netsh interface portproxy` and a scheduled
//! task that re-applies it.
//!
//! What that means for the mapping, one branch at a time:
//!
//!   `rules.rs`     `adb forward --remove` / `adb reverse --remove` instead of
//!                  `netsh ... portproxy delete`, and the read is one `--list`
//!                  per direction rather than parsing `netsh show all`. The
//!                  staging logic -- apply a rule, cancel a rule, apply all,
//!                  cancel all -- has no counterpart and is not wired up, see
//!                  `handlers/mod.rs` and MAPPING 14.
//!   `proxy.rs`     `settings put` / `settings delete` on Android's globals
//!                  instead of exporting `HTTP_PROXY` into a WSL shell, and the
//!                  three reads are AOSP's own three keys.
//!   `service.rs`   the reference's Windows service that carries netsh work
//!                  needing elevation. Nothing here needs elevation, so there is
//!                  no service to talk to and no `initialize` to wire.
//!   `utils.rs`     one piece survives: the port and address validation, which
//!                  lives in `network_model` and in `http_proxy_value` rather
//!                  than here, because it is pure and the tests must run.
//!
//! No `#[cfg(test)]` module, for the reason `disk.rs` gives at length: every
//! function here names `AppWindow`, which only the GUI binary compiles, so
//! anything written in this file would sit behind `cargo test --release --bin
//! wsadash` and never run. The decisions live in `network_model`, which does.

use std::rc::Rc;
use std::sync::Arc;

use slint::{ModelRc, VecModel, Weak};
use tracing::warn;

use crate::AppWindow;
use crate::app::state::AppState;
use crate::i18n;
use crate::ui::network_model;

/// Register the network page's callbacks (reference `handlers/network/*`).
pub fn setup(app: &AppWindow, app_handle: Weak<AppWindow>, app_state: Arc<AppState>) {
    // The reference reads these three from `network::models` in two places
    // (setup, and after every save). They are constants, so reading them once
    // here is the same result with one fewer round trip through Rust.
    app.set_network_proxy_default_host(network_model::default_host().into());
    app.set_network_proxy_default_port(network_model::default_port().into());
    app.set_network_proxy_default_no_proxy(network_model::default_no_proxy().into());

    let ah = app_handle.clone();
    let st = app_state.clone();
    app.on_delete_network_rule(move |id| {
        // The id is the *only* argument this callback gets, so everything a
        // delete needs is encoded in it -- `network_model::parse_id` is where
        // that is decided and tested.
        let Some((direction, port)) = network_model::parse_id(id.as_str()) else {
            // Not reachable from a row this module built, so it is a stale row
            // or a corrupted window rather than a bad rule. Told rather than
            // dropped: a delete that silently does nothing reads as the rule
            // being undeletable.
            warn!(id = %id, "rule id does not address anything");
            toast(
                &ah,
                i18n::tr(
                    "network.rules_cancel_all_failed",
                    &[format!("rule id {id:?} does not name a rule")],
                ),
            );
            return;
        };

        let handle = ah.clone();
        let state = st.clone();
        let _ = slint::spawn_local(async move {
            let dash = &state.wsa_dashboard;
            let outcome = match direction {
                network_model::Direction::Forward => dash.forward_remove(port).await,
                network_model::Direction::Reverse => dash.reverse_remove(port).await,
            };
            match outcome {
                Ok(()) => refresh(handle.clone(), state),
                Err(error) => toast(
                    &handle,
                    i18n::tr("network.rules_cancel_all_failed", &[error.to_string()]),
                ),
            }
        });
    });

    let ah = app_handle.clone();
    let st = app_state.clone();
    app.on_save_proxy_settings(move |enabled, host, port, no_proxy, auth, username, password| {
        let handle = ah.clone();
        let state = st.clone();
        let host = host.to_string();
        let port = port.to_string();
        let no_proxy = no_proxy.to_string();
        let _ = slint::spawn_local(async move {
            // The two authentication arguments are read into locals and then
            // dropped, deliberately and visibly, rather than ignored by not
            // binding them: AOSP has nowhere to put a proxy credential
            // (`ProxyTracker.setGlobalProxy` writes host, port, exclusion list
            // and PAC url, and `Settings.Global` has no username or password
            // entry), so writing a key the system never reads would make this
            // look configured when it is not. The reference targets a WSL
            // shell, which does have `http_proxy` env vars. MAPPING 14.
            let _ = (auth, username, password);

            let dash = &state.wsa_dashboard;
            let outcome = if enabled {
                dash.set_http_proxy(&host, &port, &no_proxy).await
            } else {
                dash.clear_http_proxy().await
            };
            match outcome {
                Ok(()) => {
                    toast(&handle, i18n::t("network.proxy_save_success"));
                    refresh(handle.clone(), state);
                }
                Err(error) => toast(
                    &handle,
                    i18n::tr("network.proxy_error_invalid", &[error.to_string()]),
                ),
            }
        });
    });

    // Read once at startup for the same reason the storage page does: the
    // first thing a reader sees after switching to the page should be the list,
    // not an empty frame followed by a spinner.
    refresh(app_handle.clone(), app_state);
}

/// Re-read both forwarding tables and the proxy, then hand them to the window.
///
/// Called from `handlers/common.rs` when tab 3 is opened, and from every write
/// above -- the reference does the same from its `refresh_usb` / `refresh_tasks`
/// / `refresh_mounted_disks` triple, on the same reasoning: a page that only
/// refreshed on a timer is blank for as long as the first visit takes, and a
/// page that does not refresh after a write shows what you just changed.
///
/// **Four reads, and any one of them failing does not empty the page.** `forward
/// --list` and `reverse --list` need no live subsystem -- adb keeps a
/// forwarding table whether or not the device is connected -- so the rules half
/// works with WSA stopped, which is the same acceptance criterion the storage
/// page is held to (spec section 11 line 348). The proxy half is the same: the
/// globals are read through `settings`, and an unreachable device reports an
/// error rather than an empty list.
pub fn refresh(handle: Weak<AppWindow>, state: Arc<AppState>) {
    let _ = slint::spawn_local(load(handle, state));
}

async fn load(handle: Weak<AppWindow>, state: Arc<AppState>) {
    let dash = &state.wsa_dashboard;
    let (forward, reverse, proxy, exclusion) = tokio::join!(
        dash.forward_list(),
        dash.reverse_list(),
        dash.get_http_proxy(),
        dash.get_proxy_exclusion_list(),
    );

    let Some(app) = handle.upgrade() else { return };

    // Each half on its own terms: a proxy read that failed leaves the last
    // known proxy in place rather than blanking a form the reader is typing in.
    match proxy {
        Ok(value) => {
            let enabled = !value.is_empty();
            app.set_network_proxy_is_enabled(enabled);
            if enabled {
                // `http_proxy_value` joined them with the platform's own
                // separator, so splitting on the last colon is the only place
                // that has to know it -- and a host containing a colon (an IPv6
                // address) keeps the rest, which is why it is `rsplit_once`.
                match value.rsplit_once(':') {
                    Some((host, port)) => {
                        app.set_network_proxy_host(host.into());
                        app.set_network_proxy_port(port.into());
                    }
                    None => warn!(value = %value, "proxy value has no port"),
                }
            }
            match exclusion {
                Ok(list) => {
                    app.set_network_proxy_no_proxy(
                        if list.is_empty() {
                            network_model::default_no_proxy()
                        } else {
                            list
                        }
                        .into(),
                    );
                }
                Err(error) => warn!(?error, "could not read the exclusion list"),
            }
        }
        Err(error) => warn!(?error, "could not read the proxy"),
    }

    // The rules half: both listings, and a listing that failed is reported as
    // an empty one *plus* a log line -- the page's own empty state says "no port
    // forwarding rules configured", which is a true statement about a machine
    // with no rules and a false one about a machine adb could not answer for.
    let (forward, reverse) = match (forward, reverse) {
        (Ok(forward), Ok(reverse)) => (forward, reverse),
        (Err(error), _) | (_, Err(error)) => {
            warn!(?error, "could not list port mappings");
            ([].to_vec(), [].to_vec())
        }
    };
    let rows: Vec<crate::PortProxyRuleUI> = network_model::rows(&forward, &reverse)
        .into_iter()
        .map(|row| crate::PortProxyRuleUI {
            id: row.id.into(),
            // The direction sentence, translated here rather than in the view:
            // `NetworkRuleCard` takes a plain string for this slot (it is the
            // reference's `distro_name`), so the key has to be resolved before
            // it reaches Slint.
            distro_name: i18n::t(row.direction_key).into(),
            listen_address: row.listen_address.into(),
            listen_port: row.listen_port.to_string().into(),
            target_port: row.target_port.to_string().into(),
            enable_firewall: row.enable_firewall,
            is_active: row.is_active,
        })
        .collect();
    app.set_network_rules(ModelRc::from(Rc::new(VecModel::from(rows))));
}

fn toast(handle: &Weak<AppWindow>, text: String) {
    if let Some(app) = handle.upgrade() {
        app.set_task_status_text(text.into());
        app.set_task_status_visible(true);
    }
}
