use crate::calc::{calculate_quarter_stats, QuarterStats};
use crate::data::{BadgeEntryData, HolidayData, Persistable, QuarterData, VacationData};
use anyhow::{bail, Result};

pub fn run(quarter_key: &str) -> Result<()> {
    let quarter_data = QuarterData::load_and_parse()?;
    let badge_data = BadgeEntryData::load()?;
    let holiday_data = HolidayData::load()?;
    let vacation_data = VacationData::load()?;

    let quarter = match quarter_data.get_quarter_by_key(quarter_key) {
        Some(q) => q,
        None => bail!("Quarter key '{}' not found in configuration.", quarter_key),
    };

    let stats = calculate_quarter_stats(quarter, &badge_data, &holiday_data, &vacation_data, None)?;

    write_stats(&stats, &mut std::io::stdout())
}

pub(crate) fn write_stats<W: std::io::Write>(stats: &QuarterStats, out: &mut W) -> Result<()> {
    let required_pct = if stats.total_days > 0 {
        100.0 * stats.days_required as f64 / stats.total_days as f64
    } else {
        0.0
    };

    let rate_needed_pct = if stats.days_left > 0 {
        format!("{:.2}%", stats.required_future_average)
    } else if stats.days_still_needed > 0 {
        "Infinite".to_string()
    } else {
        "N/A".to_string()
    };

    let pace_str = if stats.days_ahead_of_pace > 0 {
        format!("{} days ahead", stats.days_ahead_of_pace)
    } else if stats.days_ahead_of_pace < 0 {
        format!("{} days behind", -stats.days_ahead_of_pace)
    } else {
        "On pace".to_string()
    };

    let office_days = stats.days_badged_in - stats.flex_days;

    writeln!(out, "Quarter Stats for {}", stats.quarter)?;
    writeln!(
        out,
        "Range: [{} - {}]",
        stats.start_date.format("%Y-%m-%d"),
        stats.end_date.format("%Y-%m-%d")
    )?;
    writeln!(out, "---")?;
    writeln!(out, "{:<26} {}", "Status:", stats.compliance_status)?;
    writeln!(out, "{:<26} {}", "Days Ahead of Pace:", pace_str)?;
    writeln!(out, "{:<26} {}", "Skippable Days Left:", stats.remaining_missable_days)?;
    writeln!(out, "---")?;
    writeln!(
        out,
        "{:<26} ({} / {})  = {:.2}%",
        "Goal (50% Required):", stats.days_required, stats.total_days, required_pct
    )?;
    writeln!(out, "{:<26} {}", "Badged In:", stats.days_badged_in)?;
    writeln!(out, "{:<26} {}", "Still Needed:", stats.days_still_needed)?;
    writeln!(
        out,
        "{:<26} ({} / {})  = {:.2}%",
        "Rate So Far:", stats.days_badged_in, stats.days_thus_far, stats.current_average
    )?;
    writeln!(out, "---")?;
    writeln!(
        out,
        "{:<26} ({} / {})  = {}",
        "Rate Needed (Remaining):", stats.days_still_needed, stats.days_left, rate_needed_pct
    )?;
    if let Some(proj) = stats.projected_completion_date {
        writeln!(
            out,
            "{:<26} {}",
            "Projected Completion:",
            proj.format("%Y-%m-%d")
        )?;
    }
    writeln!(out, "---")?;
    writeln!(out, "{:<26} {}", "Holidays:", stats.holidays)?;
    writeln!(out, "{:<26} {}", "Vacation Days:", stats.vacation_days)?;
    writeln!(out, "{:<26} {}", "Total Days Off:", stats.days_off)?;
    writeln!(out, "---")?;
    writeln!(out, "{:<26} {}", "Office Days:", office_days)?;
    writeln!(out, "{:<26} {}", "Flex Credits:", stats.flex_days)?;
    writeln!(out, "{:<26} {}", "Total Badged In:", stats.days_badged_in)?;
    writeln!(out, "---")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::QuarterStats;
    use chrono::NaiveDate;
    use std::collections::HashMap;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn make_stats(
        compliance_status: &str,
        days_ahead_of_pace: i32,
        days_left: i32,
        days_still_needed: i32,
        projected_completion_date: Option<NaiveDate>,
    ) -> QuarterStats {
        QuarterStats {
            quarter: "Q1_2025".to_string(),
            start_date: d(2025, 1, 1),
            end_date: d(2025, 3, 31),
            days_badged_in: 30,
            flex_days: 5,
            days_thus_far: 50,
            days_left,
            total_calendar_days: 90,
            available_workdays: 62,
            total_days: 60,
            days_required: 30,
            days_still_needed,
            days_off: 3,
            holidays: 1,
            vacation_days: 2,
            current_average: 60.0,
            required_future_average: 40.0,
            compliance_status: compliance_status.to_string(),
            days_ahead_of_pace,
            remaining_missable_days: 5,
            projected_completion_date,
            workday_stats: HashMap::new(),
        }
    }

    #[test]
    fn test_run_nonexistent_quarter_returns_error() {
        // Regardless of what DATA_DIR points to, a quarter key that
        // cannot possibly exist should always return an error.
        let result = run("NONEXISTENT_QUARTER_XYZ");
        assert!(result.is_err(), "expected error for unknown quarter key");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("NONEXISTENT_QUARTER_XYZ"),
            "error message should include the bad key"
        );
    }

    #[test]
    fn test_write_stats_achieved_quarter() {
        let stats = make_stats("Achieved", 5, 10, 0, None);
        let mut buf = Vec::new();
        write_stats(&stats, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Achieved"));
        assert!(output.contains("Q1_2025"));
    }

    #[test]
    fn test_write_stats_rate_needed_infinite() {
        // days_left == 0, days_still_needed > 0 → "Infinite"
        let stats = make_stats("Impossible", -3, 0, 5, None);
        let mut buf = Vec::new();
        write_stats(&stats, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Infinite"));
    }

    #[test]
    fn test_write_stats_rate_needed_na() {
        // days_still_needed == 0 → "N/A"
        let stats = make_stats("Achieved", 5, 0, 0, None);
        let mut buf = Vec::new();
        write_stats(&stats, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("N/A"));
    }

    #[test]
    fn test_write_stats_pace_ahead() {
        let stats = make_stats("On Track", 3, 10, 2, None);
        let mut buf = Vec::new();
        write_stats(&stats, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("days ahead"));
    }

    #[test]
    fn test_write_stats_pace_behind() {
        let stats = make_stats("At Risk", -4, 10, 6, None);
        let mut buf = Vec::new();
        write_stats(&stats, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("days behind"));
    }

    #[test]
    fn test_write_stats_on_pace() {
        let stats = make_stats("On Track", 0, 10, 2, None);
        let mut buf = Vec::new();
        write_stats(&stats, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("On pace"));
    }

    #[test]
    fn test_write_stats_includes_projected_completion() {
        let stats = make_stats("On Track", 2, 10, 3, Some(d(2025, 3, 15)));
        let mut buf = Vec::new();
        write_stats(&stats, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Projected Completion"));
        assert!(output.contains("2025-03-15"));
    }

    #[test]
    fn test_write_stats_no_projected_completion_when_none() {
        let stats = make_stats("On Track", 2, 10, 3, None);
        let mut buf = Vec::new();
        write_stats(&stats, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.contains("Projected Completion"));
    }
}
