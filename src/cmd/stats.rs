use crate::calc::{QuarterStats, calculate_quarter_stats};
use crate::data::{
    AppSettings, BadgeEntryData, HolidayData, Persistable, TimePeriodData, VacationData,
};
use anyhow::{Result, bail};

pub fn run(period_key: Option<&str>) -> Result<()> {
    let settings = AppSettings::load()?;
    let td = TimePeriodData::load()?;
    let badge_data = BadgeEntryData::load()?;
    let holiday_data = HolidayData::load()?;
    let vacation_data = VacationData::load()?;

    let key = match period_key {
        Some(k) => k.to_string(),
        None => {
            let tp = td.get_current_period();
            match tp {
                Some(tp) => tp.key.clone(),
                None => bail!("cannot determine current period — try specifying a period key"),
            }
        }
    };

    let period = match td.get_period_by_key(&key) {
        Some(p) => p,
        None => bail!(
            "Period key '{}' not found — run 'rto init' to create data files",
            key
        ),
    };

    let stats = calculate_quarter_stats(
        period,
        &badge_data,
        &holiday_data,
        &vacation_data,
        settings.goal,
        None,
    )?;

    write_stats(&stats, &settings, &mut std::io::stdout())
}

pub(crate) fn write_stats<W: std::io::Write>(
    stats: &QuarterStats,
    settings: &AppSettings,
    out: &mut W,
) -> Result<()> {
    writeln!(
        out,
        "Period: {}  ({} – {})",
        stats.name,
        stats.start_date.format("%b %-d, %Y"),
        stats.end_date.format("%b %-d, %Y"),
    )?;

    writeln!(out)?;
    writeln!(out, "  Status:               {}", stats.compliance_status)?;
    writeln!(
        out,
        "  Days ahead of pace:   {:+}",
        stats.days_ahead_of_pace
    )?;
    if stats.remaining_missable_days >= 0 {
        writeln!(
            out,
            "  Skippable days left:  {}",
            stats.remaining_missable_days
        )?;
    }

    writeln!(out)?;
    writeln!(
        out,
        "  Required badge-ins:   {} of {} total days ({}%)",
        stats.days_required, stats.total_days, settings.goal
    )?;
    writeln!(out, "  Badged in:            {}", stats.days_badged_in)?;
    writeln!(out, "  Still needed:         {}", stats.days_still_needed)?;

    let office_days = stats.days_badged_in - stats.flex_days;
    writeln!(out)?;
    writeln!(
        out,
        "  Badge-ins:            {}  ({} office, {} flex)",
        stats.days_badged_in, office_days, stats.flex_days
    )?;

    writeln!(out)?;
    writeln!(out, "  Days worked so far:   {}", stats.days_thus_far)?;
    writeln!(out, "  Days remaining:       {}", stats.days_left)?;
    if stats.days_thus_far > 0 {
        writeln!(
            out,
            "  Current average:      {:.1}%",
            stats.current_average * 100.0
        )?;
    }
    if stats.days_left > 0 && stats.days_still_needed > 0 {
        writeln!(
            out,
            "  Rate needed:          {:.1}%",
            stats.required_future_average * 100.0
        )?;
    }

    if let Some(proj) = stats.projected_completion_date {
        writeln!(out)?;
        writeln!(out, "  Projected completion: {}", proj.format("%b %-d, %Y"))?;
    }

    writeln!(out)?;
    writeln!(out, "  Holidays:             {}", stats.holidays)?;
    writeln!(out, "  Vacation days:        {}", stats.vacation_days)?;
    writeln!(out, "  Days off (remote):    {}", stats.days_off)?;
    writeln!(out, "  Available workdays:   {}", stats.available_workdays)?;

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
            name: "Q1".to_string(),
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
            current_average: 0.60,
            required_future_average: 0.40,
            compliance_status: compliance_status.to_string(),
            days_ahead_of_pace,
            remaining_missable_days: 5,
            projected_completion_date,
            workday_stats: HashMap::new(),
        }
    }

    fn default_settings() -> AppSettings {
        AppSettings::default()
    }

    #[test]
    fn test_write_stats_achieved() {
        let stats = make_stats("Achieved", 5, 10, 0, None);
        let mut buf = Vec::new();
        write_stats(&stats, &default_settings(), &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Achieved"));
        assert!(output.contains("Q1"));
    }

    #[test]
    fn test_write_stats_pace_ahead() {
        let stats = make_stats("On Track", 3, 10, 2, None);
        let mut buf = Vec::new();
        write_stats(&stats, &default_settings(), &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("+3"));
    }

    #[test]
    fn test_write_stats_pace_behind() {
        let stats = make_stats("At Risk", -4, 10, 6, None);
        let mut buf = Vec::new();
        write_stats(&stats, &default_settings(), &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("-4"));
    }

    #[test]
    fn test_write_stats_includes_projected_completion() {
        let stats = make_stats("On Track", 2, 10, 3, Some(d(2025, 3, 15)));
        let mut buf = Vec::new();
        write_stats(&stats, &default_settings(), &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Projected completion"));
        assert!(output.contains("Mar 15, 2025"));
    }

    #[test]
    fn test_write_stats_no_projected_when_none() {
        let stats = make_stats("On Track", 2, 10, 3, None);
        let mut buf = Vec::new();
        write_stats(&stats, &default_settings(), &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.contains("Projected completion"));
    }

    #[test]
    fn test_write_stats_shows_goal_percentage() {
        let stats = make_stats("On Track", 0, 10, 2, None);
        let settings = AppSettings {
            goal: 60,
            ..AppSettings::default()
        };
        let mut buf = Vec::new();
        write_stats(&stats, &settings, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("60%"));
    }

    #[test]
    fn test_write_stats_badge_breakdown() {
        let stats = make_stats("On Track", 0, 10, 2, None);
        let mut buf = Vec::new();
        write_stats(&stats, &default_settings(), &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("25 office"));
        assert!(output.contains("5 flex"));
    }
}
