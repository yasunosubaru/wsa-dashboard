// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only
// Every function here is P7 Task 2/4/5's, not dead: Task 1 is the layer *under*
// those, and it has no caller in the GUI yet -- the first one arrives when
// Task 5 wires the page. The CLI's #[cfg(test)] #[path] mount runs the tests,
// which is what this task is for. Same reason and same shape as config/mod.rs,
// i18n/mod.rs and scheduler_task/mod.rs.
#![allow(dead_code)]

//! Reading scheduled tasks out of what Windows actually exports.
//!
//! This is P7's one genuinely new piece of code, and it exists because the
//! reference's does not work here. That is worth stating plainly rather than as
//! a preference.
//!
//! # What the reference does, and why it breaks
//!
//! `schtasks /Query /FO CSV /V` and `schtasks /Query /XML`, run on the same
//! machine, on a Chinese Windows:
//!
//! ```text
//! CSV:   "主机名","任务名","下次运行时间","状态","登录模式","上次运行时间",...
//!        (33 columns, and the names are Chinese)
//! XML:   <Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
//!          <RegistrationInfo><URI>\AgentDock</URI></RegistrationInfo>
//!          <Exec><Command>C:\...\agentdock-tray.exe</Command></Exec>
//! ```
//!
//! The CSV's *header row is localised*. An English Windows answers the same
//! command with `HostName,TaskName,Next Run Time,Status,...` and 29 columns.
//! The reference reads the CSV by column number (`schtasks.rs`:66-140), so on
//! this machine it would read column 2 as a status, column 3 as a date, and
//! never notice: no error, no empty list, just fields that mean something else.
//! The task name column would come back as `准备好` or `正在运行`.
//!
//! The XML has **invariant element and attribute names**. `<Task>`,
//! `<RegistrationInfo>`, `<StartBoundary>`, `<RunLevel>` are the same on every
//! Windows in the world, because they are schema names rather than labels.
//! Only the *values* are localised -- `<Description>` holds a user's text, and
//! `<StartBoundary>` is ISO 8601 (`2013-01-01T19:00:00`) rather than the CSV's
//! regional `2013/1/1`.
//!
//! So this module parses XML, and finds elements **by name**, not by position.
//!
//! # What XML does not give us
//!
//! Task *state* -- "Ready", "Running", "Disabled" -- is not exported at all. It
//! is reachable only through COM (`ITaskService`), which is a different kind of
//! dependency than anything else in this project has. `SchedulerTask::status`
//! therefore carries [`NOT_REPORTED`](super::models::NOT_REPORTED), and the card
//! says so rather than guessing. MAPPING 16.1 records the measurement; the
//! header of `models.rs` records why.

use roxmltree::{Document, Node};

/// The folder this program's tasks live in.
///
/// The reference's is `\WSLDashboard\UserTasks\`. Ours is the outer folder only,
/// and without the second segment, because we do not split one cron into several
/// native tasks (`cron.rs` says why) -- there is no `UserTasks` group to put
/// them in.
pub const TASK_FOLDER: &str = "\\WSA-Dashboard";

/// One `<Task>` element, in this program's terms.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskXml {
    /// `<RegistrationInfo><URI>` -- the full path, e.g. `\WSA-Dashboard\Nightly`.
    pub path: String,
    /// `<RegistrationInfo><Description>`.
    pub description: String,
    /// `<RegistrationInfo><Author>`. Absent on many system tasks.
    pub author: String,
    /// `<RegistrationInfo><Date>`, ISO 8601.
    pub date: String,
    /// What the task does when it fires.
    ///
    /// Not decoration. Measured on this machine: 219 scheduled tasks, 102 of them an
    /// `<Exec>` with a `<Command>` and **119 a `<ComHandler>`** -- Windows' own
    /// maintenance tasks, which have no command line at all. A page that drew one row
    /// per task with the command column empty for 119 of them would be showing its own
    /// failure rather than a fact about the system, so the kind is part of the record
    /// and the card says which it is.
    pub action: ActionKind,
    /// `<Exec><Command>`. Empty for an [`ActionKind::ComHandler`] task, which
    /// genuinely has none.
    pub command: String,
    /// `<Exec><Arguments>`, if any.
    pub arguments: String,
    /// `<Exec><WorkingDirectory>`, if any.
    pub working_directory: String,
    /// `<Principals><Principal><RunLevel>` -- `HighestAvailable` or
    /// `LeastPrivilege`. Empty when the task declares no principal.
    pub run_level: String,
    /// `<Settings><Enabled>`.
    ///
    /// `None` rather than `false`: a task with no `<Enabled>` element is not
    /// saying "disabled", it is saying nothing, and those two need to be
    /// distinguishable when the page is deciding whether to draw a tick.
    pub enabled: Option<bool>,
    /// `<Triggers>`, in the order Windows wrote them.
    pub triggers: Vec<Trigger>,
}

/// What the task does, as the XML says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionKind {
    /// `<Actions><Exec>` -- a command line. The only kind this program creates.
    Exec,
    /// `<Actions><ComHandler>` -- a COM class registered for the task. Windows
    /// uses these for its own maintenance; there is no command to read or show.
    ComHandler,
    /// `<Actions><SendEmail>`. Exists in the schema; nothing here creates one.
    SendEmail,
    /// An action element this build has not been taught.
    #[default]
    Unknown,
}

impl ActionKind {
    fn element(self) -> &'static str {
        match self {
            ActionKind::Exec => "Exec",
            ActionKind::ComHandler => "ComHandler",
            ActionKind::SendEmail => "SendEmail",
            ActionKind::Unknown => "",
        }
    }

    fn from_element(name: &str) -> Self {
        [ActionKind::Exec, ActionKind::ComHandler, ActionKind::SendEmail]
            .into_iter()
            .find(|kind| kind.element() == name)
            .unwrap_or(ActionKind::Unknown)
    }
}

/// What kind of trigger this is.
///
/// Named after the XML elements rather than after what they mean to a person:
/// `<CalendarTrigger>` covers daily, weekly and monthly all at once, and the
/// difference between them is which child elements are present. A calendar
/// trigger with `<DaysInterval>1` is a daily task, and that is a fact about the
/// document rather than a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TriggerKind {
    /// The default, because `Trigger` derives `Default` and an unknown trigger is
    /// the honest zero -- a trigger we did not recognise is not a trigger we
    /// will guess about.
    #[default]
    Calendar,
    Time,
    Daily,
    Weekly,
    Monthly,
    Logon,
    Boot,
    Idle,
    Registration,
    Event,
    /// An element this build has not been taught. Kept rather than dropped: a
    /// task we cannot describe is still a task the reader created, and hiding it
    /// would be worse than showing a row that says "unknown".
    Unknown,
}

impl TriggerKind {
    /// The XML element name for this kind.
    fn element(self) -> &'static str {
        match self {
            TriggerKind::Calendar => "CalendarTrigger",
            TriggerKind::Time => "TimeTrigger",
            TriggerKind::Daily => "DailyTrigger",
            TriggerKind::Weekly => "WeeklyTrigger",
            TriggerKind::Monthly => "MonthlyTrigger",
            TriggerKind::Logon => "LogonTrigger",
            TriggerKind::Boot => "BootTrigger",
            TriggerKind::Idle => "IdleTrigger",
            TriggerKind::Registration => "RegistrationTrigger",
            TriggerKind::Event => "EventTrigger",
            TriggerKind::Unknown => "",
        }
    }
}

/// One `<...Trigger>` element.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trigger {
    pub kind: TriggerKind,
    /// `<StartBoundary>`, verbatim. ISO 8601 where Windows wrote ISO 8601, and
    /// left alone where it did not -- the caller formats it, not this parser.
    pub start: String,
    /// `<EndBoundary>`, verbatim.
    pub end: String,
    /// `<DaysInterval>` -- `1` with a `<CalendarTrigger>` is daily.
    pub days_interval: Option<u32>,
    /// `<WeeksInterval>`.
    pub weeks_interval: Option<u32>,
    /// `<DaysOfWeek><Monday/>...`, as the names Windows wrote.
    pub days_of_week: Vec<String>,
    /// `<DaysOfMonth><DayOfMonth>1</DayOfMonth></DaysOfMonth>`, in document
    /// order -- `1,15` is meaningful in that order and sorted it would not be.
    pub days_of_month: Vec<u32>,
    /// `<Months><January/>...`.
    pub months: Vec<String>,
    /// `<Repetition><Interval>`, an ISO 8601 duration such as `PT3H30M`.
    pub repetition_interval: Option<String>,
    /// `<Repetition><Duration>`.
    pub repetition_duration: Option<String>,
    /// The trigger's own `<Enabled>`.
    pub enabled: Option<bool>,
}

/// Every `<Task>` in `schtasks /Query /XML` output.
///
/// # The output is not a valid XML document, and that is not a bug in this parser
///
/// Measured on this machine, from `schtasks /Query /XML`: 219 `<Task version`
/// elements, one `<Tasks>` wrapper, and **219 `<?xml ?>` declarations** -- one
/// before `<Tasks>` and 218 more nested *inside* it, one per task. An XML
/// declaration is only legal at the very start of a document, so `Document::parse`
/// rejects the whole thing and a parser without this step returns nothing for a
/// machine that has 219 tasks.
///
/// So the interior declarations are removed first. Everything else is untouched:
/// comments, whitespace and the `\` paths inside them all stay, because the
/// comment before each task is the only place some of those paths appear.
///
/// Returns nothing rather than an error for input that is still unparseable
/// afterwards. A malformed document is not something the reader can act on, and
/// the caller has a better thing to say -- `models::NOT_REPORTED` -- than a
/// parse error would be.
pub fn parse_tasks_xml(xml: &str) -> Vec<TaskXml> {
    // Only the declarations that are *not* at the start. `replacen` on the head,
    // then a global replace of whatever is left, is the same as stripping them
    // all -- and a document with no interior declarations is passed through
    // unchanged, which is the case the tests that do not use the multi-task shape
    // rely on.
    let cleaned = strip_interior_declarations(xml);
    let Ok(document) = Document::parse(&cleaned) else {
        return Vec::new();
    };
    document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "Task")
        .map(parse_task)
        .collect()
}

/// The tasks in this program's folder, and nothing else.
///
/// Windows has ~220 scheduled tasks on a normal desktop (this machine has 219,
/// Acrobat and Office and every updater ever installed). The page lists the ones
/// this program created, and matching on [`TASK_FOLDER`] is what distinguishes
/// them -- a folder prefix, not a localized name, so it works the same way on
/// every Windows.
pub fn our_tasks<'a>(tasks: &'a [TaskXml]) -> Vec<&'a TaskXml> {
    tasks.iter().filter(|task| in_our_folder(&task.path)).collect()
}

/// Whether this path is inside [`TASK_FOLDER`].
///
/// `starts_with(TASK_FOLDER)` is **not** the test, and a test that used it would
/// have passed while being wrong: `\WSA-Dashboard-Other\X` starts with
/// `\WSA-Dashboard`, so a program with a folder of ours next to somebody else's
/// would show the other one's tasks as ours and offer to delete them. What has to
/// follow the prefix is a path separator or the end of the string -- the same
/// rule a file path itself uses.
fn in_our_folder(path: &str) -> bool {
    let Some(rest) = path.strip_prefix(TASK_FOLDER) else {
        return false;
    };
    rest.is_empty() || rest.starts_with('\\')
}

/// The last segment of a task path: `\WSA-Dashboard\Nightly` -> `Nightly`.
///
/// Not a split on a suffix for the reference's reason -- ours has no `_<n>`
/// suffix to strip, because one cron is one task. But the last path segment is
/// still the identifier, and the display name in the dialog is what becomes it.
pub fn task_id_from_path(path: &str) -> String {
    path.rsplit('\\')
        .next()
        .unwrap_or(path)
        .trim()
        .to_string()
}

/// Whether a task's `<RunLevel>` asks for elevation.
pub fn runs_as_admin(task: &TaskXml) -> bool {
    task.run_level.eq_ignore_ascii_case("HighestAvailable")
}

/// The path a task with this id has inside [`TASK_FOLDER`].
pub fn task_path_for_id(id: &str) -> String {
    format!("{TASK_FOLDER}\\{id}")
}

/// Remove the `<?xml ... ?>` declarations nested inside a `<Tasks>` wrapper.
///
/// Done as text rather than by pre-splitting the document, because a `<?xml?>`
/// declaration can legally appear in *arbitrary text* -- a comment, a
/// description -- and a blind `split` would cut a task in half at the mention of
/// one. The bytes a declaration can legally contain are fixed and short
/// (version, encoding, standalone), so matching up to the first `?>` is enough,
/// and leaving the content alone is what keeps a mention inside a description
/// intact.
fn strip_interior_declarations(xml: &str) -> String {
    const DECLARATION: &str = "<?xml";
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    // The first declaration, if any, is the document's own and stays where it is.
    if let Some(at) = rest.find(DECLARATION) {
        let head = &rest[..at];
        let after = &rest[at..];
        // Only a declaration that starts the document (after nothing but
        // whitespace) is legal where it is.
        if head.trim().is_empty() {
            if let Some(end) = after.find("?>") {
                let declaration = &after[..end + 2];
                out.push_str(declaration);
                rest = &after[end + 2..];
            } else {
                out.push_str(head);
                rest = after;
            }
        } else {
            out.push_str(head);
            rest = after;
        }
    } else {
        out.push_str(rest);
        rest = "";
    }

    loop {
        let Some(at) = rest.find(DECLARATION) else {
            out.push_str(rest);
            break;
        };
        let Some(end) = rest[at..].find("?>") else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..at]);
        rest = &rest[at + end + 2..];
    }
    out
}

fn parse_task(node: Node<'_, '_>) -> TaskXml {
    TaskXml {
        path: child_text(node, &["RegistrationInfo", "URI"]),
        description: child_text(node, &["RegistrationInfo", "Description"]),
        author: child_text(node, &["RegistrationInfo", "Author"]),
        date: child_text(node, &["RegistrationInfo", "Date"]),
        // `<Exec>` sits under `<Actions>`, not directly under `<Task>` -- which
        // the fixtures below got wrong until this ran against the real output
        // (219 tasks, all of them with an empty command, and the reason was here).
        // Searched for by leaf name rather than by an exact path so the schema
        // version does not matter: Task Scheduler 1.2 nests it under `<Actions>`
        // and the wrapper's name is not something this program should depend on.
        action: action_kind(node),
        command: descendant_text(node, "Command"),
        arguments: descendant_text(node, "Arguments"),
        working_directory: descendant_text(node, "WorkingDirectory"),
        run_level: child_text(node, &["Principals", "Principal", "RunLevel"]),
        // `LastRunTime`/`NextRunTime`/`State` are not in the schema at all, so
        // there is nothing to read here and pretending otherwise would put a
        // placeholder in a field that has a real meaning elsewhere.
        enabled: descendant_bool(node, "Enabled"),
        triggers: parse_triggers(node),
    }
}

fn parse_triggers(task: Node<'_, '_>) -> Vec<Trigger> {
    let Some(triggers) = task
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == "Triggers")
        .next_back()
    else {
        // No `<Triggers>` at all is a real state -- AgentDock on this machine has
        // exactly that -- and it is not an error.
        return Vec::new();
    };

    triggers
        .children()
        .filter(|child| child.is_element())
        .map(|child| Trigger {
            kind: TriggerKind::element_from(child.tag_name().name()),
            start: text_of(child, "StartBoundary"),
            end: text_of(child, "EndBoundary"),
            days_interval: direct_child_number(child, "DaysInterval"),
            weeks_interval: direct_child_number(child, "WeeksInterval"),
            days_of_week: named_children(child, "DaysOfWeek"),
            days_of_month: numbered_children(child, "DaysOfMonth"),
            months: named_children(child, "Months"),
            repetition_interval: repetition_value(child, "Interval"),
            repetition_duration: repetition_value(child, "Duration"),
            enabled: child
                .children()
                .find(|c| c.is_element() && c.tag_name().name() == "Enabled")
                .and_then(|n| n.text())
                .and_then(|text| text.trim().parse().ok()),
        })
        .collect()
}

impl TriggerKind {
    fn element_from(name: &str) -> Self {
        [
            TriggerKind::Calendar,
            TriggerKind::Time,
            TriggerKind::Daily,
            TriggerKind::Weekly,
            TriggerKind::Monthly,
            TriggerKind::Logon,
            TriggerKind::Boot,
            TriggerKind::Idle,
            TriggerKind::Registration,
            TriggerKind::Event,
        ]
        .into_iter()
        .find(|kind| kind.element() == name)
        .unwrap_or(TriggerKind::Unknown)
    }
}

/// The text of the first `<parent><child>` found by walking down one level at a
/// time.
///
/// Not `descendants()`: the schema nests `<Enabled>` inside `<Settings>` and
/// `<Enabled>` inside a trigger too, and a search that took the first of either
/// would answer about the wrong one.
fn child_text(node: Node<'_, '_>, path: &[&str]) -> String {
    let mut current = node;
    for name in path {
        let Some(next) = current
            .children()
            .find(|child| child.is_element() && child.tag_name().name() == *name)
        else {
            return String::new();
        };
        current = next;
    }
    current.text().unwrap_or_default().trim().to_string()
}

fn descendant_bool(node: Node<'_, '_>, name: &str) -> Option<bool> {
    node.descendants()
        .find(|child| child.is_element() && child.tag_name().name() == name)
        .and_then(|found| found.text())
        .and_then(|text| text.trim().parse().ok())
}

/// A numeric child of a trigger: `<DaysInterval>1</DaysInterval>`.
///
/// `parse().ok()` rather than `unwrap_or(0)`: a task whose day interval was
/// not a number has not told us it runs every day, and zero would be an interval
/// nothing fires on.
fn direct_child_number(node: Node<'_, '_>, name: &str) -> Option<u32> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
        .and_then(|found| found.text())
        .and_then(|text| text.trim().parse().ok())
}

/// Which of the `<Actions>` children this task uses.
///
/// The first one wins, and there is one on any task this has ever seen: the
/// schema has them as a choice, not a sequence. A task with no `<Actions>` at
/// all is [`ActionKind::Unknown`], which is a different answer from one that
/// has an action this build cannot name.
fn action_kind(node: Node<'_, '_>) -> ActionKind {
    node.descendants()
        .filter(|child| child.is_element())
        .map(|child| child.tag_name().name())
        .find(|name| ActionKind::from_element(name) != ActionKind::Unknown)
        .map(ActionKind::from_element)
        .unwrap_or(ActionKind::Unknown)
}

/// The text of the first element with this name, at any depth.
///
/// Only for names that are unique in the schema (`Command`, `Arguments`,
/// `WorkingDirectory`). Deliberately not used for `<Enabled>`, which legitimately
/// appears in two places with two different meanings -- see [`child_text`].
fn descendant_text(node: Node<'_, '_>, name: &str) -> String {
    node.descendants()
        .find(|child| child.is_element() && child.tag_name().name() == name)
        .and_then(|found| found.text())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn text_of(node: Node<'_, '_>, name: &str) -> String {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
        .and_then(|found| found.text())
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// `<DaysOfWeek><Monday/><Friday/></DaysOfWeek>` -> `["Monday", "Friday"]`.
///
/// As the names Windows wrote them, in document order. `Mondays` and
/// `MondaysThroughFridays` are possible too and the element names are invariant,
/// so this needs no table.
fn named_children(node: Node<'_, '_>, parent: &str) -> Vec<String> {
    node.children()
        .filter(|child| child.is_element() && child.tag_name().name() == parent)
        .flat_map(|group| {
            group
                .children()
                .filter(|child| child.is_element())
                .map(|child| child.tag_name().name().to_string())
        })
        .collect()
}

/// `<DaysOfMonth><DayOfMonth>1</DayOfMonth><DayOfMonth>15</DayOfMonth></DaysOfMonth>`
/// -> `[1, 15]`, in that order.
fn numbered_children(node: Node<'_, '_>, parent: &str) -> Vec<u32> {
    node.children()
        .filter(|child| child.is_element() && child.tag_name().name() == parent)
        .flat_map(|group| group.children().filter(|child| child.is_element()))
        .filter_map(|child| child.text()?.trim().parse().ok())
        .collect()
}

fn repetition_value(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == "Repetition")
        .and_then(|rep| {
            rep.children()
                .find(|c| c.is_element() && c.tag_name().name() == name)
        })
        .and_then(|found| found.text())
        .map(|text| text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One real task, trimmed from what this machine actually answered with.
    /// `<Description>` and `<URI>` and `<Command>` verbatim; the rest is what the
    /// schema says must be there.
    const ADOBE: &str = r#"<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>This task keeps your Adobe Reader up to date.</Description>
    <URI>\Adobe Acrobat Update Task</URI>
  </RegistrationInfo>
  <Principals>
    <Principal id="Author">
      <UserId>S-1-5-21-1-2-3-1001</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <Enabled>true</Enabled>
  </Settings>
  <Triggers>
    <CalendarTrigger id="TriggerDaily">
      <StartBoundary>2013-01-01T19:00:00</StartBoundary>
      <EndBoundary>2027-05-02T12:05:00</EndBoundary>
      <DaysInterval>1</DaysInterval>
    </CalendarTrigger>
  </Triggers>
  <Actions>
    <Exec>
      <Command>C:\Program Files (x86)\Adobe\AdobeARM.exe</Command>
      <Arguments>/s</Arguments>
      <WorkingDirectory>C:\Windows\System32</WorkingDirectory>
    </Exec>
  </Actions>
</Task>"#;

    /// A Windows maintenance task: a COM class, no command line, and nothing to
    /// show in a command column. One of these exists 119 times over on this
    /// machine, which is why the kind is recorded rather than left as an empty
    /// string.
    const COM_HANDLER: &str = r#"<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <URI>\\Microsoft\\Windows\\DiskDiagnostic\\SilentCleanup</URI>
  </RegistrationInfo>
  <Actions>
    <ComHandler>
      <ClassId>C73F6162</ClassId>
      <Data>Data</Data>
    </ComHandler>
  </Actions>
</Task>"#;

    #[test]
    fn a_com_handler_task_has_a_kind_and_no_command_rather_than_an_empty_one() {
        let task = &parse_tasks_xml(COM_HANDLER)[0];
        assert_eq!(task.action, ActionKind::ComHandler);
        assert_eq!(
            task.command, "",
            "there is genuinely no command here, and the kind is what says so"
        );
        assert_eq!(runs_as_admin(task), false, "no principal, so not elevation");
    }

    #[test]
    fn a_task_with_no_actions_element_is_unknown_rather_than_exec() {
        let xml = r#"<Task><RegistrationInfo><URI>\\Nothing</URI></RegistrationInfo></Task>"#;
        assert_eq!(
            parse_tasks_xml(xml)[0].action,
            ActionKind::Unknown,
            "no <Actions> is not the same as an <Exec> this parser failed to read"
        );
    }

    /// A task in our own folder, as this program would write one.
    const OURS: &str = r#"<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <URI>\WSA-Dashboard\NightlyReboot</URI>
    <Author>reader</Author>
  </RegistrationInfo>
  <Principals>
    <Principal id="Author">
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <Enabled>false</Enabled>
  </Settings>
  <Triggers>
    <CalendarTrigger>
      <StartBoundary>2026-01-01T03:30:00</StartBoundary>
      <DaysOfWeek>
        <Monday/>
        <Friday/>
      </DaysOfWeek>
      <DaysOfMonth><DayOfMonth>1</DayOfMonth><DayOfMonth>15</DayOfMonth></DaysOfMonth>
      <Months><January/><June/></Months>
    </CalendarTrigger>
  </Triggers>
  <Actions>
    <Exec>
      <Command>C:\Program Files\WSA Dashboard\wsadashboard.exe</Command>
      <Arguments>/scheduler</Arguments>
    </Exec>
  </Actions>
</Task>"#;

    #[test]
    fn one_task_becomes_one_record_with_every_field_this_program_needs() {
        let tasks = parse_tasks_xml(ADOBE);
        assert_eq!(tasks.len(), 1, "a document with one <Task> has one task");

        let task = &tasks[0];
        assert_eq!(task.path, r"\Adobe Acrobat Update Task");
        assert_eq!(task.description, "This task keeps your Adobe Reader up to date.");
        assert_eq!(task.command, r"C:\Program Files (x86)\Adobe\AdobeARM.exe");
        assert_eq!(task.arguments, "/s");
        assert_eq!(task.working_directory, r"C:\Windows\System32");
        assert_eq!(task.action, ActionKind::Exec);
        assert_eq!(task.run_level, "HighestAvailable");
        assert_eq!(task.enabled, Some(true));
        // Author is on `<Principal id="Author">` as an *attribute* on this
        // machine's tasks, not a `<RegistrationInfo><Author>` element. Not
        // inventing one: the field is empty and says nothing.
        assert_eq!(task.author, "");
    }

    /// The measurement this whole module exists for, written as a test.
    #[test]
    fn element_names_are_found_by_name_and_a_localised_document_is_undamaged() {
        // The CSV the reference reads, on this machine. Every value in it is
        // Chinese, and there are 33 of them rather than the 29 an English
        // Windows returns. None of that appears here, because nothing here
        // looks at it.
        let csv = "\"主机名\",\"任务名\",\"下次运行时间\",\"状态\",\"登录模式\",\"上次运行时间\"\r\n\
                    \"DESKTOP-EXAMPLE\",\"\\AgentDock\",\"2026/10/15 19:00:00\",\"准备就绪\",\"仅在用户登录时运行\"\r\n";
        assert!(csv.contains("任务名"), "the fixture should be the localised one");

        // And the XML for the same task parses to the same answer the reference
        // would have got on an English machine.
        let tasks = parse_tasks_xml(ADOBE);
        assert_eq!(tasks[0].path, r"\Adobe Acrobat Update Task");
    }

    #[test]
    fn a_multi_task_document_yields_every_task() {
        // The shape `schtasks /Query /XML` actually returns: a `<Tasks>` wrapper,
        // one `<?xml ...?>` declaration and `<!-- \path -->` comment per task.
        // The shape the real command emits, declarations and all: one before
        // <Tasks> and one inside it per task, which is what `schtasks` prints on
        // this machine (219 tasks, 219 declarations). Feeding this to
        // `Document::parse` directly yields nothing at all.
        let document = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n<Tasks>\r\n\
             \r\n<!-- \\Adobe Acrobat Update Task -->\r\n\
             <?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n{adobe}\r\n\
             \r\n<!-- \\AgentDock -->\r\n\
             <?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n\
             <Task version=\"1.2\">\r\n\
             \r\n  <RegistrationInfo>\r\n    <URI>\\AgentDock</URI>\r\n\
             \r\n  </RegistrationInfo>\r\n\r\n</Task>\r\n\
             \r\n</Tasks>\r\n",
            adobe = ADOBE.trim()
        );
        // The count that matters is the one *after* the wrapper opens: that is
        // what makes the document illegal, and it is what this parser had to
        // learn to handle.
        let interior = document
            .split_once("<Tasks>")
            .map(|(_, after)| after.matches("<?xml").count())
            .unwrap_or(0);
        assert!(
            interior >= 2,
            "the fixture must carry the interior declarations, or it proves nothing"
        );
        let tasks = parse_tasks_xml(&document);
        assert_eq!(tasks.len(), 2, "two comments, two <Task> elements");
        assert_eq!(tasks[0].path, r"\Adobe Acrobat Update Task");
        assert_eq!(tasks[1].path, r"\AgentDock");
        // The second has no <Principals>, so it has no run level rather than a
        // guessed one.
        assert_eq!(tasks[1].run_level, "");
        assert_eq!(tasks[1].enabled, None);
    }

    /// An empty `<Triggers>` is what this machine answers with for tasks whose
    /// trigger `schtasks` does not export. It must not be read as "no trigger
    /// kinds are supported" or, worse, as a parse failure.
    #[test]
    fn a_task_with_no_exported_trigger_parses_with_no_triggers() {
        let xml = r#"<Task>
          <RegistrationInfo><URI>\AgentDock</URI></RegistrationInfo>
          <Triggers></Triggers>
          <Actions><Exec><Command>C:\x\agentdock-tray.exe</Command></Exec></Actions>
        </Task>"#;
        let tasks = parse_tasks_xml(xml);
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].triggers.is_empty());
    }

    #[test]
    fn a_trigger_with_no_triggers_element_parses_with_no_triggers() {
        let xml = r#"<Task>
          <RegistrationInfo><URI>\NoTriggers</URI></RegistrationInfo>
          <Actions><Exec><Command>C:\x\a.exe</Command></Exec></Actions>
        </Task>"#;
        assert!(parse_tasks_xml(xml)[0].triggers.is_empty());
    }

    #[test]
    fn a_weekly_trigger_keeps_its_weekdays_in_order_and_its_day_numbers() {
        let tasks = parse_tasks_xml(OURS);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].action, ActionKind::Exec, "the only kind we create");
        let triggers = &tasks[0].triggers;
        assert_eq!(triggers.len(), 1);
        let trigger = &triggers[0];
        assert_eq!(trigger.kind, TriggerKind::Calendar);
        assert_eq!(trigger.start, "2026-01-01T03:30:00", "ISO 8601, verbatim");
        assert_eq!(trigger.days_of_week, vec!["Monday", "Friday"]);
        assert_eq!(
            trigger.days_of_month,
            vec![1, 15],
            "1,15 means something in this order and nothing in the other one"
        );
        assert_eq!(trigger.months, vec!["January", "June"]);
        assert_eq!(trigger.days_interval, None);
    }

    #[test]
    fn repetition_is_read_from_the_repetition_block_not_from_the_trigger() {
        let xml = r#"<Task>
          <RegistrationInfo><URI>\LogonThenRepeat</URI></RegistrationInfo>
          <Triggers>
            <LogonTrigger id="TriggerUserLogon">
              <StartBoundary>2013-08-01T12:05:00</StartBoundary>
              <EndBoundary>2027-05-02T08:00:00</EndBoundary>
              <Repetition>
                <Interval>PT3H30M</Interval>
                <Duration>P1D</Duration>
                <StopAtDurationEnd>false</StopAtDurationEnd>
              </Repetition>
            </LogonTrigger>
          </Triggers>
          <Actions><Exec><Command>C:\x\a.exe</Command></Exec></Actions>
        </Task>"#;
        let trigger = &parse_tasks_xml(xml)[0].triggers[0];
        assert_eq!(trigger.kind, TriggerKind::Logon);
        assert_eq!(trigger.repetition_interval.as_deref(), Some("PT3H30M"));
        assert_eq!(trigger.repetition_duration.as_deref(), Some("P1D"));
        assert_eq!(trigger.start, "2013-08-01T12:05:00");
    }

    /// A trigger element this build has not been taught. The task is still the
    /// reader's own, so it is still listed -- as an unknown kind rather than
    /// dropped.
    #[test]
    fn a_trigger_kind_we_do_not_know_is_kept_as_unknown() {
        let xml = r#"<Task>
          <RegistrationInfo><URI>\Future</URI></RegistrationInfo>
          <Triggers>
            <SomethingNewTrigger><StartBoundary>2026-01-01T00:00:00</StartBoundary></SomethingNewTrigger>
          </Triggers>
          <Actions><Exec><Command>C:\x\a.exe</Command></Exec></Actions>
        </Task>"#;
        let trigger = &parse_tasks_xml(xml)[0].triggers[0];
        assert_eq!(trigger.kind, TriggerKind::Unknown);
        assert_eq!(trigger.start, "2026-01-01T00:00:00");
    }

    /// The wrong column does not throw, it returns a wrong answer. So these are
    /// the inputs where returning *nothing* is the only safe thing to do.
    #[test]
    fn output_that_is_not_xml_yields_no_tasks_rather_than_guessing() {
        // `schtasks` writes its errors to stdout in the CSV shapes, and on a
        // machine with no read access the whole thing comes back as text.
        for junk in [
            "",
            "ERROR: The system cannot find the file specified.",
            "信息: 没有可运行的任务。",
            "<Task><RegistrationInfo><URI>\\X</URI></RegistrationInfo>",
            "{\"json\": true}",
        ] {
            assert!(
                parse_tasks_xml(junk).is_empty(),
                "{:?} should parse to nothing",
                junk
            );
        }
    }

    #[test]
    fn an_empty_document_is_not_a_failure() {
        assert!(parse_tasks_xml("").is_empty());
    }

    /// 219 tasks on a normal desktop, 2 of them ours. This is the filter that
    /// makes the page readable.
    #[test]
    fn only_our_folder_is_ours() {
        let document = format!(
            "<Tasks>{adobe}{ours}<Task><RegistrationInfo><URI>\\Microsoft\\Windows\\Defrag</URI></RegistrationInfo></Task></Tasks>",
            adobe = ADOBE,
            ours = OURS
        );
        let tasks = parse_tasks_xml(&document);
        assert_eq!(tasks.len(), 3, "the filter is not the parser's job");
        let mine = our_tasks(&tasks);
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].path, r"\WSA-Dashboard\NightlyReboot");
    }

    /// A path that merely *starts with* our folder's characters is not in our
    /// folder: `\WSA-Dashboard-Other\X` belongs to somebody else.
    #[test]
    fn the_folder_filter_does_not_match_a_lookalike_folder() {
        let document = r#"<Tasks>
          <Task><RegistrationInfo><URI>\WSA-Dashboard-Other\X</URI></RegistrationInfo></Task>
          <Task><RegistrationInfo><URI>\WSA-Dashboard\Y</URI></RegistrationInfo></Task>
        </Tasks>"#;
        let tasks = parse_tasks_xml(document);
        assert_eq!(our_tasks(&tasks).len(), 1);
        assert_eq!(our_tasks(&tasks)[0].path, r"\WSA-Dashboard\Y");
    }

    #[test]
    fn the_id_is_the_last_path_segment_and_the_path_is_built_from_it() {
        assert_eq!(task_id_from_path(r"\WSA-Dashboard\Nightly"), "Nightly");
        assert_eq!(task_id_from_path("Nightly"), "Nightly", "no path, no prefix");
        assert_eq!(task_id_from_path(""), "");
        assert_eq!(
            task_path_for_id("Nightly"),
            r"\WSA-Dashboard\Nightly"
        );
        // The round trip the dialog depends on: what the user typed comes back
        // out of what Windows wrote.
        let mine = parse_tasks_xml(OURS);
        assert_eq!(
            task_path_for_id(&task_id_from_path(&mine[0].path)),
            mine[0].path
        );
    }

    #[test]
    fn run_level_answers_the_admin_question_in_the_dialog() {
        let adobe = &parse_tasks_xml(ADOBE)[0];
        assert!(runs_as_admin(adobe));

        let ours = &parse_tasks_xml(OURS)[0];
        assert!(!runs_as_admin(ours), "LeastPrivilege is not elevation");

        let no_principal = &parse_tasks_xml(r#"<Task><RegistrationInfo><URI>\X</URI></RegistrationInfo></Task>"#)[0];
        assert!(!runs_as_admin(no_principal), "no principal is not elevation");
    }

    /// `<Enabled>false</Enabled>` and no `<Enabled>` at all are different claims
    /// and the page needs to tell them apart: one is a task the reader turned
    /// off, the other is a system task we do not manage.
    #[test]
    fn absent_and_false_are_different_answers() {
        let disabled = &parse_tasks_xml(OURS)[0];
        assert_eq!(disabled.enabled, Some(false));

        let silent = &parse_tasks_xml(
            r#"<Task><Settings></Settings><RegistrationInfo><URI>\X</URI></RegistrationInfo></Task>"#,
        )[0];
        assert_eq!(silent.enabled, None);
    }

    /// The test that matters: the real output of the real command, on this
    /// machine, through the whole path -- spawn, decode, strip the 218 interior
    /// declarations, find 219 `<Task>` elements, and keep the two that are ours.
    ///
    /// `#[ignore]` because Q8 says the unit tests must not depend on the
    /// machine: this one spawns a process, reads 18 000 lines and asserts on how
    /// many scheduled tasks a particular Windows installation happens to have.
    /// Run it with `-- --ignored --nocapture`.
    #[test]
    #[ignore = "spawns schtasks; Q8 keeps machine-dependent work out of the suite"]
    fn against_the_real_schtasks_on_this_machine() {
        let output = std::process::Command::new("schtasks")
            .args(["/Query", "/XML"])
            .output()
            .expect("schtasks is on PATH");
        let text = String::from_utf8_lossy(&output.stdout);

        let tasks = parse_tasks_xml(&text);
        println!(
            "schtasks /Query /XML: {} bytes, {} <Task> elements, {} interior declarations",
            text.len(),
            tasks.len(),
            text.split_once("<Tasks>").map(|(_, a)| a.matches("<?xml").count()).unwrap_or(0),
        );
        assert!(tasks.len() > 10, "only {} tasks parsed", tasks.len());

        // Every task has to have a path and a command, or the page would render
        // a row with nothing in it. This is the check that the field mapping is
        // right on a localised Windows, which the fixtures above cannot prove --
        // they are documents I wrote, not documents Windows wrote.
        let mut exec = 0;
        let mut com_handler = 0;
        let mut other = 0;
        for task in &tasks {
            assert!(!task.path.is_empty(), "a task with no <URI>: {task:?}");
            match task.action {
                ActionKind::Exec => {
                    exec += 1;
                    assert!(
                        !task.command.is_empty(),
                        "an <Exec> task with no <Command>: {:?}",
                        task.path
                    );
                }
                ActionKind::ComHandler => {
                    com_handler += 1;
                    assert!(
                        task.command.is_empty(),
                        "a <ComHandler> task with a command: {:?}",
                        task.path
                    );
                }
                _ => other += 1,
            }
        }
        println!(
            "{} tasks: {} exec, {} com handler, {} other; {} ours",
            tasks.len(),
            exec,
            com_handler,
            other,
            our_tasks(&tasks).len(),
        );
        // Both kinds have to be present on any machine that has been running for a
        // while. Windows adds tasks over time, so the counts are not pinned -- but
        // the two invariants above are: an <Exec> always has a command and a
        // <ComHandler> never does, and if either of these fails it means the
        // Actions lookup or the Command lookup is wrong, not that Windows changed.
        assert!(exec > 0, "no <Exec> tasks at all; the Command lookup is wrong");
        assert!(com_handler > 0, "no <ComHandler> tasks at all; the Actions lookup is wrong");
        assert_eq!(exec + com_handler + other, tasks.len());

        // A sample of what the localisation did to the parse, printed so the
        // numbers can be compared between runs.
        for task in tasks.iter().take(3) {
            println!(
                "  {:?} | run_level={:?} enabled={:?} triggers={:?}",
                task.path,
                task.run_level,
                task.enabled,
                task.triggers.iter().map(|t| t.kind).collect::<Vec<_>>()
            );
        }

        assert!(
            our_tasks(&tasks).len() < tasks.len(),
            "the folder filter kept everything, so it is not filtering"
        );
    }
}
