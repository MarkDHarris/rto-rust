use crate::calc::workday::{Workday, create_workday_map};
use crate::data::{BadgeEntryData, HolidayData, TimePeriod, VacationData};
use anyhow::Result;
use chrono::{Local, NaiveDate};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct QuarterStats {
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub days_badged_in: i32,
    pub flex_days: i32,
    pub days_thus_far: i32,
    pub days_left: i32,
    pub total_calendar_days: i32,
    pub available_workdays: i32,
    pub total_days: i32,
    pub days_required: i32,
    pub days_still_needed: i32,
    pub days_off: i32,
    pub holidays: i32,
    pub vacation_days: i32,
    pub current_average: f64,
    pub required_future_average: f64,
    pub compliance_status: String,
    pub days_ahead_of_pace: i32,
    pub remaining_missable_days: i32,
    pub projected_completion_date: Option<NaiveDate>,
    pub workday_stats: HashMap<String, Workday>,
}

/// Computes full statistics for a time period.
/// `goal_pct` is the required office percentage (e.g. 50 means 50%).
pub fn calculate_quarter_stats(
    period: &TimePeriod,
    badge: &BadgeEntryData,
    holiday: &HolidayData,
    vacation: &VacationData,
    goal_pct: i32,
    today: Option<NaiveDate>,
) -> Result<QuarterStats> {
    let today = today.unwrap_or_else(|| Local::now().date_naive());

    let start = period.start_date.unwrap();
    let end = period.end_date.unwrap();

    let badge_map = badge.get_badge_map(start, end);
    let vacation_map = vacation.get_vacation_map();
    let holiday_map = holiday.get_holiday_map();
    let mut workday_map = create_workday_map(start, end);

    let mut keys: Vec<String> = workday_map.keys().cloned().collect();
    keys.sort();

    let total_calendar_days = (end - start).num_days() as i32 + 1;

    let mut days_badged_in = 0i32;
    let mut flex_days = 0i32;
    let mut days_thus_far = 0i32;
    let mut available_workdays = 0i32;
    let mut total_days = 0i32;
    let mut holidays = 0i32;
    let mut vacation_days = 0i32;

    for key in &keys {
        let day = workday_map.get_mut(key).unwrap();

        if holiday_map.contains_key(key.as_str()) {
            day.is_holiday = true;
            holidays += 1;
            available_workdays += 1;
            continue;
        }

        available_workdays += 1;

        if vacation_map.contains_key(key.as_str()) {
            day.is_vacation = true;
            vacation_days += 1;
            continue;
        }

        total_days += 1;

        if let Some(badge_entry) = badge_map.get(key.as_str())
            && badge_entry.is_badged_in
        {
            day.is_badged_in = true;
            days_badged_in += 1;
            if badge_entry.is_flex_credit {
                day.is_flex_credit = true;
                flex_days += 1;
            }
        }

        if day.date > today {
            continue;
        }

        if day.date == today {
            continue;
        }

        days_thus_far += 1;
    }

    let days_left = total_days - days_thus_far;
    let days_required = ((total_days as f64) * (goal_pct as f64) / 100.0).ceil() as i32;

    let mut days_still_needed = days_required - days_badged_in;
    if days_still_needed < 0 {
        days_still_needed = 0;
    }

    let days_off = days_thus_far - days_badged_in;

    let days_ahead_of_pace = if days_thus_far > 0 && total_days > 0 {
        let expected =
            ((days_thus_far as f64) * (days_required as f64) / (total_days as f64)).round() as i32;
        days_badged_in - expected
    } else {
        0
    };

    let remaining_missable = days_left - days_still_needed;

    let current_average = if days_thus_far > 0 {
        days_badged_in as f64 / days_thus_far as f64
    } else {
        0.0
    };

    let required_future_average = if days_left > 0 {
        days_still_needed as f64 / days_left as f64
    } else {
        0.0
    };

    let compliance_status = determine_compliance_status(
        days_badged_in,
        days_required,
        days_ahead_of_pace,
        days_still_needed,
        days_left,
    );

    let projected_completion_date =
        if days_badged_in > 0 && days_thus_far > 0 && days_still_needed > 0 {
            let rate = days_badged_in as f64 / days_thus_far as f64;
            if rate > 0.0 {
                let estimated_days = (days_still_needed as f64 / rate).ceil() as i64;
                today.checked_add_days(chrono::Days::new(estimated_days as u64))
            } else {
                None
            }
        } else {
            None
        };

    Ok(QuarterStats {
        name: period.name.clone(),
        start_date: start,
        end_date: end,
        days_badged_in,
        flex_days,
        days_thus_far,
        days_left,
        total_calendar_days,
        available_workdays,
        total_days,
        days_required,
        days_still_needed,
        days_off,
        holidays,
        vacation_days,
        current_average,
        required_future_average,
        compliance_status,
        days_ahead_of_pace,
        remaining_missable_days: remaining_missable,
        projected_completion_date,
        workday_stats: workday_map,
    })
}

fn determine_compliance_status(
    days_badged_in: i32,
    days_required: i32,
    days_ahead_of_pace: i32,
    days_still_needed: i32,
    days_left: i32,
) -> String {
    if days_badged_in >= days_required {
        return "Achieved".to_string();
    }
    if days_ahead_of_pace == 0 && days_badged_in == 0 {
        return "On Track".to_string();
    }
    if days_still_needed > days_left {
        return "Impossible".to_string();
    }
    if days_ahead_of_pace < 0 {
        return "At Risk".to_string();
    }
    "On Track".to_string()
}

/// Computes aggregate statistics across multiple time periods (for year stats).
pub fn calculate_year_stats(
    periods: &[&TimePeriod],
    badge: &BadgeEntryData,
    holiday: &HolidayData,
    vacation: &VacationData,
    goal_pct: i32,
    today: Option<NaiveDate>,
) -> Result<Option<QuarterStats>> {
    if periods.is_empty() {
        return Ok(None);
    }

    let mut start = periods[0].start_date.unwrap();
    let mut end = periods[0].end_date.unwrap();
    for tp in periods {
        let s = tp.start_date.unwrap();
        let e = tp.end_date.unwrap();
        if s < start {
            start = s;
        }
        if e > end {
            end = e;
        }
    }

    let synthetic = TimePeriod {
        key: "Year".to_string(),
        name: "Year".to_string(),
        start_date_raw: start.format("%Y-%m-%d").to_string(),
        end_date_raw: end.format("%Y-%m-%d").to_string(),
        start_date: Some(start),
        end_date: Some(end),
    };

    let mut stats = calculate_quarter_stats(&synthetic, badge, holiday, vacation, goal_pct, today)?;
    stats.name = "Year".to_string();
    Ok(Some(stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::badge_entry::BadgeEntry;
    use crate::data::holiday::Holiday;
    use crate::data::vacation::Vacation;
    use crate::data::{BadgeEntryData, HolidayData, VacationData};

    fn make_period(start: &str, end: &str) -> TimePeriod {
        let mut tp = TimePeriod {
            key: "TEST_Q".to_string(),
            name: "Test".to_string(),
            start_date_raw: start.to_string(),
            end_date_raw: end.to_string(),
            start_date: None,
            end_date: None,
        };
        tp.parse_dates().unwrap();
        tp
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn test_on_track_status() {
        let q = make_period("2025-01-06", "2025-01-17");
        let mut badge = BadgeEntryData::default();
        badge.add(BadgeEntry::new(date(2025, 1, 6), "Office", false));
        badge.add(BadgeEntry::new(date(2025, 1, 7), "Office", false));
        badge.add(BadgeEntry::new(date(2025, 1, 8), "Office", false));
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = date(2025, 1, 13);
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        assert_eq!(stats.days_badged_in, 3);
        assert!(stats.compliance_status == "On Track" || stats.compliance_status == "At Risk");
    }

    #[test]
    fn test_achieved_status() {
        let q = make_period("2025-01-06", "2025-01-09");
        let mut badge = BadgeEntryData::default();
        for d in 6..=9 {
            badge.add(BadgeEntry::new(date(2025, 1, d), "Office", false));
        }
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = date(2025, 1, 20);
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        assert_eq!(stats.compliance_status, "Achieved");
    }

    #[test]
    fn test_impossible_status() {
        let q = make_period("2025-01-06", "2025-01-09");
        let badge = BadgeEntryData::default();
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = date(2025, 1, 20);
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        assert_eq!(stats.compliance_status, "Impossible");
    }

    #[test]
    fn test_days_ahead_of_pace_positive() {
        let q = make_period("2025-01-06", "2025-01-17");
        let mut badge = BadgeEntryData::default();
        for d in 6..=9 {
            badge.add(BadgeEntry::new(date(2025, 1, d), "Office", false));
        }
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = date(2025, 1, 10);
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        assert!(stats.days_ahead_of_pace > 0);
    }

    #[test]
    fn test_remaining_missable_days() {
        let q = make_period("2025-01-06", "2025-01-17");
        let mut badge = BadgeEntryData::default();
        for d in [6, 7, 8, 9, 10] {
            badge.add(BadgeEntry::new(date(2025, 1, d), "Office", false));
        }
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = date(2025, 1, 13);
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        assert!(stats.remaining_missable_days >= 0);
    }

    #[test]
    fn test_empty_quarter_no_divide_by_zero() {
        let q = make_period("2025-01-06", "2025-01-06");
        let badge = BadgeEntryData::default();
        let mut holiday = HolidayData::default();
        holiday.add(Holiday::new("Test", "2025-01-06"));
        let vacation = VacationData::default();
        let today = date(2025, 1, 7);
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        assert_eq!(stats.total_days, 0);
        assert_eq!(stats.current_average, 0.0);
    }

    #[test]
    fn test_flex_credit_counted_separately() {
        let q = make_period("2025-01-06", "2025-01-09");
        let mut badge = BadgeEntryData::default();
        badge.add(BadgeEntry::new(date(2025, 1, 6), "Office", false));
        badge.add(BadgeEntry::new(date(2025, 1, 7), "Flex Credit", true));
        badge.add(BadgeEntry::new(date(2025, 1, 8), "Office", false));
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = date(2025, 1, 20);
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        assert_eq!(stats.days_badged_in, 3);
        assert_eq!(stats.flex_days, 1);
        assert_eq!(stats.compliance_status, "Achieved");
    }

    #[test]
    fn test_working_days_and_available_workdays() {
        // Mon 01/06 = Holiday, Wed 01/08 = Vacation, Tue+Thu+Fri = work days
        // available_workdays = all weekdays (including holidays) = 5
        // total_days = weekdays minus holidays minus vacations = 3
        let q = make_period("2025-01-06", "2025-01-10");
        let badge = BadgeEntryData::default();
        let mut holiday = HolidayData::default();
        holiday.add(Holiday::new("Holiday", "2025-01-06"));
        let mut vacation = VacationData::default();
        vacation.add(Vacation::new("Trip", "2025-01-08", "2025-01-08", true));
        let today = date(2025, 1, 20);
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        assert_eq!(stats.total_calendar_days, 5);
        assert_eq!(stats.available_workdays, 5);
        assert_eq!(stats.total_days, 3);
    }

    #[test]
    fn test_total_calendar_days_includes_weekends() {
        let q = make_period("2025-01-06", "2025-01-12");
        let badge = BadgeEntryData::default();
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = date(2025, 1, 20);
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        assert_eq!(stats.total_calendar_days, 7);
        assert_eq!(stats.available_workdays, 5);
        assert_eq!(stats.total_days, 5);
    }

    #[test]
    fn test_configurable_goal_percentage() {
        let q = make_period("2025-01-06", "2025-01-17"); // 10 workdays
        let badge = BadgeEntryData::default();
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = date(2025, 1, 20);
        let stats_50 =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        let stats_60 =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, 60, Some(today)).unwrap();
        assert_eq!(stats_50.days_required, 5);
        assert_eq!(stats_60.days_required, 6);
    }

    #[test]
    fn test_year_stats_computation() {
        let q1 = make_period("2025-01-06", "2025-03-31");
        let q2 = make_period("2025-04-01", "2025-06-30");
        let badge = BadgeEntryData::default();
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = date(2025, 1, 7);
        let periods: Vec<&TimePeriod> = vec![&q1, &q2];
        let year_stats =
            calculate_year_stats(&periods, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        assert!(year_stats.is_some());
        let ys = year_stats.unwrap();
        assert_eq!(ys.name, "Year");
        assert!(ys.total_days > 0);
    }

    #[test]
    fn test_year_stats_empty_periods() {
        let badge = BadgeEntryData::default();
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = date(2025, 1, 1);
        let periods: Vec<&TimePeriod> = vec![];
        let result =
            calculate_year_stats(&periods, &badge, &holiday, &vacation, 50, Some(today)).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_compliance_status_on_track_no_badges() {
        let status = determine_compliance_status(0, 5, 0, 5, 10);
        assert_eq!(status, "On Track");
    }
}
