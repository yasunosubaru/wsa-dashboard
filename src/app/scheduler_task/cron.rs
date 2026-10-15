// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only
// Every function here is P7 Task 2/4/5's, not dead: Task 1 is the layer *under*
// those, and it has no caller in the GUI yet -- the first one arrives when
// Task 5 wires the page. The CLI's #[cfg(test)] #[path] mount runs the tests,
// which is what this task is for. Same reason and same shape as config/mod.rs,
// i18n/mod.rs and scheduler_task/mod.rs.
#![allow(dead_code)]

//! Cron expression to `schtasks /Create` parameters.
//!
//! The counterpart of the reference's `src/app/scheduler_task/schtasks.rs`
//! lines 612-771: `parse_field`, `parse_cron`, `convert_to_schtasks`,
//! `month_name`. Carried over as written, including the ten tests at the bottom,
//! because this is the one part of that 758-line file with no Windows in it at
//! all -- it is arithmetic over five strings, it is where a wrong answer produces
//! a task that fires at the wrong hour, and it is exactly the kind of thing that
//! deserves to be copied rather than rewritten.
//!
//! # One thing is not copied: the split
//!
//! The reference splits one cron expression into **several** native tasks when
//! `schtasks /Create` cannot express it as a single one -- `MyTask_1`,
//! `MyTask_2`, ... -- and `build_tasks` reassembles them by stripping the numeric
//! suffix. That is `is_simple: false`, and it is the reason `RawSchtasksRecord`
//! has a `task_id` field separate from `task_name`.
//!
//! We do not do it. `parse_field` rejects comma-separated values, so a
//! `0 9 1,15 * *` never reaches `convert_to_schtasks` at all -- the expression is
//! refused with a sentence saying what is wrong. That is a better answer than
//! quietly creating two tasks that each fire on one of the two days, and it is
//! also the reason one cron is one native task here, which is in turn why a task
//! has no numeric suffix to strip.
//!
//! `CronParsed::is_simple` is kept for the reference's sake and is always
//! `true`; `models.rs` says what it would mean if it were not.

// `super::` rather than `crate::app::scheduler_task::`: the CLI mounts this
// whole module at the crate root with `#[cfg(test)] #[path]`, where `crate::app`
// does not exist. `super::` is correct in both places, which is the only reason
// this works without a feature flag.
use super::models::{CronFields, CronParsed, SchtasksParams};

/// The three-letter month names `schtasks /M` wants.
fn month_name(m: i32) -> String {
    match m {
        1 => "JAN".into(),
        2 => "FEB".into(),
        3 => "MAR".into(),
        4 => "APR".into(),
        5 => "MAY".into(),
        6 => "JUN".into(),
        7 => "JUL".into(),
        8 => "AUG".into(),
        9 => "SEP".into(),
        10 => "OCT".into(),
        11 => "NOV".into(),
        12 => "DEC".into(),
        _ => "JAN".into(),
    }
}

/// One cron field: `*`, `*/N`, or a single number.
///
/// No comma-separated lists and no ranges. `*/N` is not a cron step either -- in
/// cron `*/N` means "every N from 0" and here it means exactly the same thing,
/// which is why the step is a plain `step_by`.
fn parse_field(field: &str, min: i32, max: i32) -> Result<Vec<i32>, String> {
    if field.contains(',') {
        return Err(format!(
            "Comma-separated values are not supported: '{field}'. Use a single value, *, or */N"
        ));
    }

    if field == "*" {
        return Ok((min..=max).collect());
    }

    if let Some(step_str) = field.strip_prefix("*/") {
        let step: i32 = step_str
            .parse()
            .map_err(|_| format!("Invalid step value: '{field}'"))?;
        if step <= 0 {
            return Err(format!("Step must be positive: '{field}'"));
        }
        return Ok((min..=max).step_by(step as usize).collect());
    }

    let val: i32 = field
        .parse()
        .map_err(|_| format!("Invalid value: '{field}'. Use a single number, *, or */N"))?;
    if val < min || val > max {
        return Err(format!("Value {val} out of range [{min}, {max}]"));
    }
    Ok(vec![val])
}

/// Parse a five-field cron expression into `schtasks` parameters.
///
/// `minute hour day-of-month month day-of-week`, and the five have the ranges
/// cron gives them -- which is **not** what `schtasks /Create` offers, and the
/// conversion below is where that difference gets resolved.
pub fn parse_cron(expr: &str) -> Result<CronParsed, String> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return Err("Cron expression must have 5 fields: minute hour day month weekday".into());
    }

    let minutes = parse_field(parts[0], 0, 59)?;
    let hours = parse_field(parts[1], 0, 23)?;
    let days_of_month = parse_field(parts[2], 1, 31)?;
    let months = parse_field(parts[3], 1, 12)?;
    let days_of_week = parse_field(parts[4], 0, 7)?;

    // 7 is Sunday in cron and does not exist in `schtasks /D`; 0 is.
    let mut days_of_week: Vec<i32> = days_of_week.iter().map(|&d| if d == 7 { 0 } else { d }).collect();
    days_of_week.sort();
    days_of_week.dedup();

    let schtasks = convert_to_schtasks(&minutes, &hours, &days_of_month, &months, &days_of_week)?;

    Ok(CronParsed {
        fields: CronFields {
            minutes,
            hours,
            days_of_month,
            months,
            days_of_week,
        },
        is_simple: true,
        schtasks,
    })
}

/// Cron fields to `schtasks /Create` parameters.
///
/// Seven patterns, tried in order, each one a shape `schtasks` has a `/SC` value
/// for. The "all" tests are deliberately generous -- `days_of_month.len() >= 28`
/// accepts February's 28 days as "every day" -- because the alternative is a
/// daily task becoming a monthly one, and a task that fires far less often than
/// asked for is worse than one that fires a little too often.
fn convert_to_schtasks(
    minutes: &[i32],
    hours: &[i32],
    days_of_month: &[i32],
    months: &[i32],
    days_of_week: &[i32],
) -> Result<SchtasksParams, String> {
    let all_minutes = minutes.len() == 60;
    let all_hours = hours.len() == 24;
    let all_days = days_of_month.len() >= 28;
    let all_months = months.len() == 12;
    let all_weekdays = days_of_week.len() == 7 || days_of_week.is_empty();

    // Pattern 0: every minute (* * * * *)
    if all_minutes && all_hours && all_days && all_months && all_weekdays {
        return Ok(SchtasksParams {
            sc: "MINUTE".into(),
            mo: Some("1".into()),
            st: None,
            d: None,
            m: None,
        });
    }

    // Pattern 1: every N minutes (*/N * * * *) -- minutes must have multiple values
    if !all_minutes && all_hours && all_days && all_months && all_weekdays && minutes.len() > 1 {
        let step = minutes[1] - minutes[0];
        return Ok(SchtasksParams {
            sc: "MINUTE".into(),
            mo: Some(step.to_string()),
            st: None,
            d: None,
            m: None,
        });
    }

    // Pattern 2: every hour (M * * * *)
    if !all_minutes && all_hours && all_days && all_months && all_weekdays {
        return Ok(SchtasksParams {
            sc: "HOURLY".into(),
            mo: Some("1".into()),
            st: Some(format!("{:02}:{:02}", 0, minutes[0])),
            d: None,
            m: None,
        });
    }

    // Pattern 3: daily (M H * * *)
    if !all_minutes && !all_hours && all_days && all_months && all_weekdays {
        return Ok(SchtasksParams {
            sc: "DAILY".into(),
            mo: Some("1".into()),
            st: Some(format!("{:02}:{:02}", hours[0], minutes[0])),
            d: None,
            m: None,
        });
    }

    // Pattern 4: weekly (M H * * dow)
    if !all_minutes && !all_hours && all_days && all_months && !all_weekdays {
        let dow_str = days_of_week
            .iter()
            .map(|d| match d {
                0 => "SUN",
                1 => "MON",
                2 => "TUE",
                3 => "WED",
                4 => "THU",
                5 => "FRI",
                6 => "SAT",
                _ => "SUN",
            })
            .collect::<Vec<_>>()
            .join(",");
        return Ok(SchtasksParams {
            sc: "WEEKLY".into(),
            mo: None,
            st: Some(format!("{:02}:{:02}", hours[0], minutes[0])),
            d: Some(dow_str),
            m: None,
        });
    }

    // Pattern 5: monthly (M H d * *)
    if !all_minutes && !all_hours && !all_days && all_months && all_weekdays {
        return Ok(SchtasksParams {
            sc: "MONTHLY".into(),
            mo: None,
            st: Some(format!("{:02}:{:02}", hours[0], minutes[0])),
            d: Some(days_of_month[0].to_string()),
            m: None,
        });
    }

    // Pattern 6: specific months (M H d mon *)
    if !all_minutes && !all_hours && !all_days && !all_months {
        return Ok(SchtasksParams {
            sc: "MONTHLY".into(),
            mo: None,
            st: Some(format!("{:02}:{:02}", hours[0], minutes[0])),
            d: Some(days_of_month[0].to_string()),
            m: Some(month_name(months[0])),
        });
    }

    Err(format!(
        "Unsupported cron pattern: {} {} {} {} {}",
        minutes.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(","),
        hours.iter().map(|h| h.to_string()).collect::<Vec<_>>().join(","),
        days_of_month.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(","),
        months.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(","),
        days_of_week.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(","),
    ))
}

/// `parse_cron` for the caller that only wants to know whether an expression is
/// usable, without building the parameters.
///
/// The settings page's dialog validates on every keystroke, and a cron typed into
/// five text boxes is wrong most of the way through being typed. A separate
/// "is this any good" question means the page can grey out Save on `0 9 1,` and
/// say why, rather than waiting for a button press to find out.
pub fn validate_cron(expr: &str) -> Result<(), String> {
    parse_cron(expr).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    // `super::` from inside `mod tests` is `cron` itself, which already has
    // `SchtasksParams` in scope from the import above -- rather than `models`,
    // which is `super::super`.
    use super::SchtasksParams;

    fn parse_and_convert(cron: &str) -> SchtasksParams {
        parse_cron(cron).unwrap().schtasks
    }

    // The reference's ten tests, verbatim (`schtasks.rs`:781-864). They are here
    // because this function is here: a copy of code with no copy of its tests is
    // a copy whose next edit is a copy whose first test is wrong.

    #[test]
    fn test_every_minute() {
        let result = parse_and_convert("* * * * *");
        assert_eq!(result.sc, "MINUTE");
        assert_eq!(result.mo.as_deref(), Some("1"));
    }

    #[test]
    fn test_every_5_minutes() {
        let result = parse_and_convert("*/5 * * * *");
        assert_eq!(result.sc, "MINUTE");
        assert_eq!(result.mo.as_deref(), Some("5"));
    }

    #[test]
    fn test_every_hour_at_minute() {
        let result = parse_and_convert("30 * * * *");
        assert_eq!(result.sc, "HOURLY");
        assert_eq!(result.st.as_deref(), Some("00:30"));
    }

    #[test]
    fn test_daily() {
        let result = parse_and_convert("5 12 * * *");
        assert_eq!(result.sc, "DAILY");
        assert_eq!(result.st.as_deref(), Some("12:05"));
    }

    #[test]
    fn test_weekly_monday() {
        let result = parse_and_convert("0 9 * * 1");
        assert_eq!(result.sc, "WEEKLY");
        assert_eq!(result.d.as_deref(), Some("MON"));
    }

    #[test]
    fn test_monthly() {
        let result = parse_and_convert("0 0 1 * *");
        assert_eq!(result.sc, "MONTHLY");
        assert_eq!(result.d.as_deref(), Some("1"));
    }

    #[test]
    fn test_specific_month() {
        let result = parse_and_convert("5 12 1 6 *");
        assert_eq!(result.sc, "MONTHLY");
        assert_eq!(result.d.as_deref(), Some("1"));
        assert_eq!(result.m.as_deref(), Some("JUN"));
    }

    #[test]
    fn test_all_month_names() {
        let names: Vec<String> = (1..=12).map(month_name).collect();
        assert_eq!(
            names,
            vec!["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"]
        );
    }

    #[test]
    fn test_parse_field_rejects_comma() {
        assert!(parse_field("1,5", 0, 59).is_err());
    }

    #[test]
    fn test_parse_field_rejects_range() {
        assert!(parse_field("1-5", 0, 59).is_err());
    }

    #[test]
    fn test_parse_field_out_of_range() {
        assert!(parse_field("60", 0, 59).is_err());
    }

    #[test]
    fn test_parse_cron_too_few_fields() {
        assert!(parse_cron("5 12 *").is_err());
    }

    #[test]
    fn test_weekday_7_maps_to_0() {
        let result = parse_and_convert("0 9 * * 7");
        assert_eq!(result.d.as_deref(), Some("SUN"));
    }

    // Ours. The six cases the reference has no test for, in rough order of how
    // much damage getting them wrong would do.

    /// A half-typed expression is refused with a reason, and the reason names
    /// the field that is wrong rather than saying "invalid".
    #[test]
    fn the_error_says_which_field_is_wrong() {
        let error = parse_cron("5 12 * *").unwrap_err();
        assert!(error.contains("5 fields"), "got: {error}");

        let error = parse_cron("0 25 * * *").unwrap_err();
        assert!(error.contains("out of range"), "got: {error}");

        let error = parse_cron("*/0 * * * *").unwrap_err();
        assert!(error.contains("Step must be positive"), "got: {error}");
    }

    /// A comma-separated day is the one the reference would have *silently
    /// split into two tasks*. Refusing it is the whole point of not copying the
    /// split, so it gets a test that says so.
    #[test]
    fn a_comma_separated_day_is_refused_rather_than_split_into_two_tasks() {
        let error = parse_cron("0 9 1,15 * *").unwrap_err();
        assert!(
            error.contains("Comma-separated"),
            "the error has to name the reason, not just fail: {error}"
        );
    }

    /// `*/5` starts at the field's minimum and steps. `*/7` in minutes is
    /// 0,7,14,...56 -- nine values, and the step the reference reads as
    /// `minutes[1] - minutes[0]`.
    #[test]
    fn a_step_is_the_difference_between_the_first_two_values() {
        let result = parse_and_convert("*/7 * * * *");
        assert_eq!(result.mo.as_deref(), Some("7"));

        // A step larger than the field itself yields exactly one value, which is
        // an hourly task at that minute rather than a "every N minutes" one.
        let result = parse_and_convert("*/90 * * * *");
        assert_eq!(result.sc, "HOURLY");
        assert_eq!(result.st.as_deref(), Some("00:00"));
    }

    /// What the weekday field actually does with a step, and what it cannot do.
    ///
    /// This test was written expecting `*/4` to mean Monday-and-Friday, which is
    /// what a person reading "every fourth day" would guess. It does not: the
    /// field runs 0..=7, **0 is Sunday**, so the step lands on 0, 4 and 7 -- and 7
    /// is folded into 0 by the `parse_cron` mapping, leaving Sunday and
    /// Thursday.
    ///
    /// The second half is the part worth keeping. "Monday and Friday" is not
    /// expressible at all in this parser: `1,5` is refused as a comma list, and
    /// `1-5/2` is refused because ranges are not supported. So a task that should
    /// run on two named weekdays cannot be created, and the page has to say so
    /// rather than quietly creating a task on the wrong two days.
    #[test]
    fn a_weekday_step_starts_at_sunday_and_two_named_days_are_not_expressible() {
        let result = parse_and_convert("0 9 * * */4");
        assert_eq!(result.sc, "WEEKLY");
        assert_eq!(
            result.d.as_deref(),
            Some("SUN,THU"),
            "the field is 0..=7 and 0 is Sunday, not Monday"
        );

        for expr in ["0 9 * * 1,5", "0 9 * * 1-5/2", "0 9 * * MON,FRI"] {
            assert!(
                parse_cron(expr).is_err(),
                "{expr} should be refused rather than guessed at"
            );
        }
    }

    /// February has 28 days and `all_days` is `>= 28` on purpose. A daily task
    /// must not become a monthly one because its day-of-month field was spelled
    /// as an enumeration of all 31 days.
    #[test]
    fn an_explicit_every_day_of_month_is_still_a_daily_task() {
        let result = parse_and_convert("0 9 */1 * *");
        assert_eq!(result.sc, "DAILY");
        assert_eq!(result.st.as_deref(), Some("09:00"));
    }

    /// `validate_cron` is the page's per-keystroke question, and it must answer
    /// exactly what `parse_cron` would -- otherwise Save is enabled for an
    /// expression the dialog will then refuse.
    #[test]
    fn validation_agrees_with_parsing() {
        for expr in ["* * * * *", "0 9 * * 1", "5 12 1 6 *", "0 0 1 * *"] {
            assert!(parse_cron(expr).is_ok(), "{expr}");
            assert!(validate_cron(expr).is_ok(), "{expr}");
        }
        for expr in ["", "0 9 * *", "0 25 * * *", "0 9 1,15 * *", "nonsense"] {
            assert!(parse_cron(expr).is_err(), "{expr}");
            assert!(validate_cron(expr).is_err(), "{expr}");
        }
    }
}