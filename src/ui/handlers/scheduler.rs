// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! The scheduler page's own work: list the tasks, and change them.
//!
//! The counterpart of the reference's `src/ui/handlers/scheduler.rs` (504
//! lines), and it is a close copy of its shape: the same ten callbacks, the same
//! four `busy_task_id` brackets, the same toast on success and on failure, the
//! same "an edit is a delete and a create" hand-off to
//! [`schtasks::create_task`], the same default schedule of `* * * * *` for a new
//! task, and the same file-picker filter.
//!
//! # Four things this file does differently, each for a reason
//!
//! **The blocking call does not run on the UI thread, and it runs on a plain
//! thread.** The reference reaches `tokio::spawn` here and `spawn_blocking` for
//! its file picker (`:472`); neither is needed by anything this page does. What
//! it does is run `schtasks`, a blocking process that reads about 362 KB of task
//! XML before it answers on this machine, and then get the answer back to the UI
//! thread. The second half is `slint::invoke_from_event_loop`, which is Slint's
//! and not tokio's, and the first half is what [`std::thread::spawn`] is for.
//!
//! A tokio task would work, and it was the first draft -- but `tokio::spawn`
//! panics outside a runtime context, so the runtime handle has to be taken where
//! it is certain it exists and carried into every callback. This page is also
//! refreshed by `handlers/common.rs`'s tab switch, which is a second entry point
//! that would need the same handle threaded to it. A thread has no such
//! requirement, and one thread per button press is not a resource this page can
//! exhaust: the reader is the rate limit.
//!
//! **The dialog's validation is one function, not a chain of `return`s.** The
//! reference's `on_save` is 66 lines of five-line blocks, each calling one of
//! three near-identical setters (`:343-399`); ours asks
//! [`crate::ui::scheduler_model::validate_new_task`] which field is wrong and
//! runs the four lines once. The two agree on the rules and on the order they
//! are checked in, which is what matters and what the tests in that file pin.
//!
//! **The error sentences are looked up, not concatenated.** The reference writes
//! `format!("{}: {}", i18n::t(key), e)`, and every one of its keys ends in a
//! `{0}` placeholder (`scheduler.error_create_failed` is literally "Failed to
//! create task: {0}"), so its toast reads "Failed to create task: {0}: the
//! reason". Ours passes the reason as the argument: `i18n::tr(key, &[e])`. The
//! two `Toggle failed:` / `Run failed:` strings the reference hardcodes
//! (`:278`, `:319`) are keys here, for the reason this project has an i18n
//! table at all.
//!
//! **A failed query says so.** The view has two empty states, "querying" and
//! "no tasks", and a query that *failed* looks exactly like the second of them.
//! The reference fills `scheduler_error` on failure and never renders it
//! (`app.slint`'s note on that property says the same); ours fills it too, and
//! also toasts, so the two are different screens. Clearing it on success is the
//! other half of that: an unrendered property that only ever accumulates is a
//! trap for whoever renders it next.
//!
//! # The help button
//!
//! The reference fills the dialog's two `?` links from `api1.wslui.com`, the
//! service spec Q6 forbids this build from calling, so both are left at the
//! empty default `app.slint` gives them and `?` goes to
//! [`super::about::open`], which says there is no address rather than opening
//! nothing. That is the same treatment the about page's links get, and the same
//! sentence: the fact is about this build, not about either page.
//!
//! # No `#[cfg(test)]` module
//!
//! Because every function here names `AppWindow`, which only the GUI binary
//! compiles: tests written in this file would sit behind
//! `cargo test --release --bin wsadash` and never run. What can be tested is
//! [`crate::ui::scheduler_model`], and `cron.rs`'s `error_key` is the third
//! piece -- it lives beside the parser that produces the messages it maps,
//! because this file's two mount points in the CLI are at different depths.

use std::sync::Arc;

use slint::{ModelRc, VecModel, Weak};
use tracing::{debug, error, info};

use crate::app::scheduler_task::models::CreateTaskParams;
use crate::app::scheduler_task::{cron, schtasks};
use crate::app::state::AppState;
use crate::i18n;
use crate::ui::scheduler_model::{short_filename, validate_new_task, SchedulerField};
use crate::{AppWindow, SchedulerTaskUI};

/// One task in the list as the card wants it.
///
/// The reference's `to_ui_task` (`:24-41`), including the part that is easy to
/// miss: `command` and `command_short` are the *unwrapped* command -- the one
/// the reader typed -- while the task store holds a command line with this
/// program's launcher wrapped around it (`schtasks::unwrap_command`).
fn to_ui_task(task: &crate::app::scheduler_task::SchedulerTask) -> SchedulerTaskUI {
    let unwrapped_command = schtasks::unwrap_command(&task.command);
    SchedulerTaskUI {
        task_id: task.task_id.clone().into(),
        display_name: task.display_name.clone().into(),
        command: unwrapped_command.clone().into(),
        command_short: short_filename(&unwrapped_command).into(),
        arguments: task.arguments.clone().into(),
        cron_expr: task.cron_expr.clone().into(),
        enabled: task.enabled,
        status: task.status.clone().into(),
        last_run: task.last_run.clone().into(),
        next_run: task.next_run.clone().into(),
        trigger_summary: task.trigger_summary.clone().into(),
        admin_mode: task.admin_mode,
    }
}

/// Read the tasks and put them on the page.
///
/// Called when the page is opened (`handlers/common.rs`, the tab switch) and
/// again after every action that can change the list.
///
/// The loading state is set *synchronously*, before the work is handed off
/// (`:46-50`), because "querying" is a sentence about the moment the reader
/// pressed the button and not about the moment the answer arrives. The two empty
/// states the view has are "querying" and "no tasks", and this is what keeps
/// them apart.
pub fn refresh(handle: Weak<AppWindow>, _app_state: Arc<AppState>) {
    refresh_tasks(handle);
}

fn refresh_tasks(handle: Weak<AppWindow>) {
    if let Some(app) = handle.upgrade() {
        app.set_scheduler_loading(true);
        app.set_scheduler_tasks(ModelRc::new(VecModel::from(Vec::<SchedulerTaskUI>::new())));
    }

    std::thread::spawn(move || {
        let result = schtasks::list_tasks();
        let moved = handle.clone();
        let _ = slint::invoke_from_event_loop(move || {
            let Some(app) = moved.upgrade() else { return };
            app.set_scheduler_loading(false);
            match result {
                Ok(tasks) => {
                    // Cleared, where the reference leaves the last failure in it
                    // forever: the property is unrendered today and a stale one
                    // is a trap for whoever renders it.
                    app.set_scheduler_error("".into());
                    let rows: Vec<SchedulerTaskUI> = tasks.iter().map(to_ui_task).collect();
                    app.set_scheduler_tasks(ModelRc::new(VecModel::from(rows)));
                }
                Err(message) => {
                    error!(%message, "the task store could not be read");
                    let sentence = i18n::tr("scheduler.error_query_failed", &[message]);
                    app.set_scheduler_error(sentence.clone().into());
                    app.set_scheduler_tasks(ModelRc::new(VecModel::from(
                        Vec::<SchedulerTaskUI>::new(),
                    )));
                    toast(&moved, sentence);
                }
            }
        });
    });
}

/// Run one task-store change off the UI thread and report how it went.
///
/// The three callbacks that change one card differ only in which function they
/// call and which two sentences they show; the reference writes the rest --
/// bracket `busy_task_id`, hand the work off, clear the bracket, toast, refresh
/// on success -- out three times (`:209-325`). It is written once here.
///
/// The save path is not one of the three. It has no card to bracket, it has a
/// dialog to close, and it has a `submitting` flag of its own to clear on
/// failure, so it keeps its own four lines rather than being fitted into this.
fn change_task(
    handle: &Weak<AppWindow>,
    busy_task_id: String,
    work: impl FnOnce() -> Result<(), String> + Send + 'static,
    ok_key: &'static str,
    err_key: &'static str,
) {
    if let Some(app) = handle.upgrade() {
        app.set_scheduler_busy_task_id(busy_task_id.as_str().into());
    }

    let moved = handle.clone();
    std::thread::spawn(move || {
        let result = work();
        let _ = slint::invoke_from_event_loop(move || {
            let Some(app) = moved.upgrade() else { return };
            // The bracket is cleared before the outcome is looked at, so a
            // failure does not leave a card spinning.
            app.set_scheduler_busy_task_id("".into());
            match result {
                Ok(()) => {
                    toast(&moved, i18n::t(ok_key));
                    refresh_tasks(moved.clone());
                }
                Err(message) => {
                    error!(key = err_key, %message, "a task-store change failed");
                    toast(&moved, i18n::tr(err_key, &[message]));
                }
            }
        });
    });
}

fn toast(handle: &Weak<AppWindow>, text: String) {
    if let Some(app) = handle.upgrade() {
        app.set_task_status_text(text.into());
        app.set_task_status_visible(true);
    }
}

fn set_field_error(handle: &Weak<AppWindow>, field: SchedulerField, message: &str) {
    let Some(app) = handle.upgrade() else { return };
    let message: slint::SharedString = message.into();
    match field {
        SchedulerField::Name => app.set_scheduler_dialog_name_error(message),
        SchedulerField::Command => app.set_scheduler_dialog_command_error(message),
        SchedulerField::Cron => app.set_scheduler_dialog_cron_error(message),
    }
}

fn clear_all_errors(handle: &Weak<AppWindow>) {
    let Some(app) = handle.upgrade() else { return };
    app.set_scheduler_dialog_name_error("".into());
    app.set_scheduler_dialog_command_error("".into());
    app.set_scheduler_dialog_cron_error("".into());
}

/// Register the page's ten callbacks.
pub fn setup(app: &AppWindow, app_handle: Weak<AppWindow>, _app_state: Arc<AppState>) {
    // Refresh -- opening the page, and after every action below.
    app.on_refresh_scheduler_tasks({
        let handle = app_handle.clone();
        move || refresh_tasks(handle.clone())
    });

    // Add -- the dialog in add mode, with an empty name and an every-minute
    // schedule, which is the reference's default (`:120`).
    app.on_add_scheduler_task({
        let handle = app_handle.clone();
        move || {
            let Some(app) = handle.upgrade() else { return };
            app.set_scheduler_dialog_is_edit(false);
            app.set_scheduler_dialog_task_id("".into());
            app.set_scheduler_dialog_task_name("".into());
            app.set_scheduler_dialog_command("".into());
            app.set_scheduler_dialog_arguments("".into());
            app.set_scheduler_dialog_cron_expr("* * * * *".into());
            app.set_scheduler_dialog_admin_mode(false);
            app.set_scheduler_dialog_submitting(false);
            clear_all_errors(&handle);
            app.set_show_scheduler_dialog(true);
        }
    });

    // Edit -- read the store again and fill the dialog from the task itself.
    //
    // The reference does the same (`:152-198`) rather than filling it from the
    // row the reader clicked, and the reason survives the move: the row's
    // command is the wrapped one, so unwrapping against a fresh read is the only
    // way the dialog shows what the store actually holds.
    app.on_edit_scheduler_task({
        let handle = app_handle.clone();
        move |task_id| {
            let wanted = task_id.to_string();
            let moved = handle.clone();
            std::thread::spawn(move || {
                let result = schtasks::list_tasks();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(app) = moved.upgrade() else { return };
                    let tasks = match result {
                        Ok(tasks) => tasks,
                        Err(message) => {
                            error!(%message, "the task store could not be read for an edit");
                            toast(&moved, i18n::tr("scheduler.error_query_failed", &[message]));
                            return;
                        }
                    };
                    let Some(task) = tasks.iter().find(|task| task.task_id == wanted) else {
                        // The reference logs and leaves the dialog shut, which is
                        // what happens when the page is stale because the task was
                        // deleted in the Task Scheduler by hand. The list is read
                        // again so the stale row leaves the screen.
                        debug!(%wanted, "no such task to edit");
                        refresh_tasks(moved.clone());
                        return;
                    };
                    app.set_scheduler_dialog_is_edit(true);
                    app.set_scheduler_dialog_task_id(task.task_id.clone().into());
                    app.set_scheduler_dialog_task_name(task.display_name.clone().into());
                    app.set_scheduler_dialog_command(
                        schtasks::unwrap_command(&task.command).into(),
                    );
                    app.set_scheduler_dialog_arguments(task.arguments.clone().into());
                    app.set_scheduler_dialog_cron_expr(task.cron_expr.clone().into());
                    app.set_scheduler_dialog_admin_mode(task.admin_mode);
                    app.set_scheduler_dialog_submitting(false);
                    clear_all_errors(&moved);
                    app.set_show_scheduler_dialog(true);
                });
            });
        }
    });

    // Delete -- and the reference's second branch, which is worth keeping: a task
    // that is in the store but not in the listing has been deleted by hand, and
    // the launcher this program wrote for it is still on disk. That is a cleanup
    // rather than a failure.
    app.on_delete_scheduler_task({
        let handle = app_handle.clone();
        move |task_id| {
            let target = task_id.to_string();
            info!(%target, "deleting a scheduled task");
            change_task(
                &handle,
                target.clone(),
                move || match schtasks::list_tasks() {
                    Ok(tasks) => {
                        if tasks.iter().any(|task| task.task_id == target) {
                            schtasks::delete_task(&target, true)
                        } else {
                            schtasks::cleanup_vbs_launcher(&target);
                            Ok(())
                        }
                    }
                    Err(message) => Err(message),
                },
                "scheduler.task_deleted",
                "scheduler.error_delete_failed",
            );
        }
    });

    // Enable / disable -- the state is read from the store rather than from the
    // row, so a task changed in the Task Scheduler by hand flips the right way.
    app.on_toggle_scheduler_task({
        let handle = app_handle.clone();
        move |task_id| {
            let target = task_id.to_string();
            change_task(
                &handle,
                target.clone(),
                move || match schtasks::list_tasks() {
                    Ok(tasks) => match tasks.iter().find(|task| task.task_id == target) {
                        Some(task) => schtasks::toggle_task(&target, !task.enabled, true),
                        None => Err("no such task".to_string()),
                    },
                    Err(message) => Err(message),
                },
                "scheduler.task_toggled",
                "scheduler.error_toggle_failed",
            );
        }
    });

    // Run now.
    app.on_run_scheduler_task({
        let handle = app_handle.clone();
        move |task_id| {
            let target = task_id.to_string();
            info!(%target, "running a scheduled task now");
            change_task(
                &handle,
                target.clone(),
                move || match schtasks::list_tasks() {
                    Ok(tasks) => {
                        if tasks.iter().any(|task| task.task_id == target) {
                            schtasks::run_task(&target, true)
                        } else {
                            Err("no such task".to_string())
                        }
                    }
                    Err(message) => Err(message),
                },
                "scheduler.task_started",
                "scheduler.error_run_failed",
            );
        }
    });

    // Save -- create, or replace the task being edited.
    app.on_save_scheduler_task({
        let handle = app_handle.clone();
        move |name, command, arguments, cron_expr| {
            let Some(app) = handle.upgrade() else { return };
            let params = CreateTaskParams {
                display_name: name.to_string(),
                command: command.to_string(),
                arguments: arguments.to_string(),
                cron_expr: cron_expr.to_string(),
                admin_mode: app.get_scheduler_dialog_admin_mode(),
            };
            let is_edit = app.get_scheduler_dialog_is_edit();
            let old_id = app.get_scheduler_dialog_task_id().to_string();

            clear_all_errors(&handle);

            // One question, one answer: the first field that is wrong is the one
            // the reader is told about.
            if let Some((field, key)) =
                validate_new_task(&params.display_name, &params.command, &params.cron_expr)
            {
                set_field_error(&handle, field, &i18n::t(key));
                app.set_scheduler_dialog_submitting(false);
                return;
            }

            if let Err(message) = cron::parse_cron(&params.cron_expr) {
                // A message this build wrote after the mapping was: shown as it
                // is rather than swallowed, and in English, which is the wart the
                // reference's own fallback branch has (`:391-393`).
                let shown = match cron::error_key(&message) {
                    Some(key) => i18n::t(key),
                    None => message,
                };
                set_field_error(&handle, SchedulerField::Cron, &shown);
                app.set_scheduler_dialog_submitting(false);
                return;
            }

            let moved = handle.clone();
            std::thread::spawn(move || {
                // An edit is a delete and a create behind one prompt, which is
                // the reference's hand-off and `create_task`'s third argument.
                let old_task = if is_edit && !old_id.is_empty() {
                    Some(old_id.as_str())
                } else {
                    None
                };
                let result = schtasks::create_task(&params, true, old_task);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(app) = moved.upgrade() else { return };
                    match result {
                        Ok(()) => {
                            app.set_show_scheduler_dialog(false);
                            let key = if is_edit {
                                "scheduler.task_updated"
                            } else {
                                "scheduler.task_created"
                            };
                            toast(&moved, i18n::t(key));
                            refresh_tasks(moved.clone());
                        }
                        Err(message) => {
                            error!(%message, "the task could not be written");
                            app.set_scheduler_dialog_submitting(false);
                            toast(
                                &moved,
                                i18n::tr("scheduler.error_create_failed", &[message]),
                            );
                        }
                    }
                });
            });
        }
    });

    // Close -- and the dialog's state is reset here rather than on the next
    // open, because a reader who closes a failing save and reopens the dialog
    // must not meet yesterday's red line.
    app.on_close_scheduler_dialog({
        let handle = app_handle.clone();
        move || {
            let Some(app) = handle.upgrade() else { return };
            app.set_scheduler_dialog_submitting(false);
            app.set_show_scheduler_dialog(false);
            clear_all_errors(&handle);
        }
    });

    // Browse -- the reference's filter and its title, both (`:474-475`).
    app.on_browse_scheduler_command({
        let handle = app_handle.clone();
        move || {
            let moved = handle.clone();
            std::thread::spawn(move || {
                let picked = rfd::FileDialog::new()
                    .add_filter("Scripts", &["ps1", "bat", "cmd", "exe", "py"])
                    .set_title(i18n::t("scheduler.dialog_select_command"))
                    .pick_file();
                if let Some(path) = picked {
                    let text = path.to_string_lossy().to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = moved.upgrade() {
                            app.set_scheduler_dialog_command(text.into());
                        }
                    });
                }
            });
        }
    });

    // The two `?` buttons. Always empty in this build; see the module docs.
    app.on_open_scheduler_help({
        let handle = app_handle.clone();
        move |url| {
            let url = url.to_string();
            if url.is_empty() {
                debug!("scheduler: the help links have no address in this build");
            }
            super::about::open(&handle, url);
        }
    });

    // The dialog's four change callbacks. The reference registers them empty
    // (`:499-503`) and so does this: the dialog's own `if` clauses are what
    // disable Save, and nothing here has an opinion between keystrokes.
    app.on_scheduler_name_changed(|_| {});
    app.on_scheduler_command_changed(|_| {});
    app.on_scheduler_arguments_changed(|_| {});
    app.on_scheduler_cron_changed(|_| {});
}
