use crate::calc::workday::{create_workday_map, Workday};
use crate::data::{BadgeEntryData, HolidayData, QuarterConfig, VacationData};
use anyhow::Result;
use chrono::{Local, NaiveDate};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct QuarterStats {
    pub quarter: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub days_badged_in: i32,
    pub flex_days: i32,
    pub days_thus_far: i32,
    pub days_left: i32,
    /// All calendar days in the quarter range (including weekends).
    pub total_calendar_days: i32,
    /// Weekdays that are not holidays (vacation days are included).
    pub available_workdays: i32,
    /// Weekdays that are not holidays and not vacation days.
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

pub fn calculate_quarter_stats(
    quarter: &QuarterConfig,
    badge: &BadgeEntryData,
    holiday: &HolidayData,
    vacation: &VacationData,
    today: Option<NaiveDate>,
) -> Result<QuarterStats> {
    let today = today.unwrap_or_else(|| Local::now().date_naive());

    let start = quarter.start_date.unwrap();
    let end = quarter.end_date.unwrap();

    let badge_map = badge.get_badge_map(start, end);
    let vacation_map = vacation.get_vacation_map();
    let holiday_map = holiday.get_holiday_map();
    let mut workday_map = create_workday_map(start, end);

    let mut keys: Vec<String> = workday_map.keys().cloned().collect();
    keys.sort();

    // All calendar days from start to end inclusive (includes weekends).
    let total_calendar_days = (end - start).num_days() as i32 + 1;

    let mut days_badged_in = 0i32;
    let mut flex_days = 0i32;
    let mut days_thus_far = 0i32;
    let mut days_left = 0i32;
    let mut days_in_quarter = 0i32;
    let mut available_workdays = 0i32;
    let mut days_off = 0i32;
    let mut holidays = 0i32;
    let mut vacation_days = 0i32;

    for key in &keys {
        let day = workday_map.get_mut(key).unwrap();

        if let Some(badge_entry) = badge_map.get(key.as_str()) {
            day.is_badged_in = badge_entry.is_badged_in;
            day.is_flex_credit = badge_entry.is_flex_credit;
        }
        if vacation_map.contains_key(key.as_str()) {
            day.is_vacation = true;
        }
        if holiday_map.contains_key(key.as_str()) {
            day.is_holiday = true;
        }

        // Every entry in workday_map is a weekday; available_workdays excludes holidays only.
        if !day.is_holiday {
            available_workdays += 1;
        }

        if !day.is_holiday && !day.is_vacation {
            day.is_workday = true;
            days_in_quarter += 1;

            if day.is_badged_in {
                days_badged_in += 1;
                if day.is_flex_credit {
                    flex_days += 1;
                }
            }

            if day.date < today || (day.date == today && day.is_badged_in) {
                days_thus_far += 1;
            } else {
                days_left += 1;
            }
        } else {
            day.is_workday = false;
            days_off += 1;
            if day.is_holiday {
                holidays += 1;
            }
            if day.is_vacation {
                vacation_days += 1;
            }
        }
    }

    let current_average = if days_thus_far > 0 {
        100.0 * days_badged_in as f64 / days_thus_far as f64
    } else {
        0.0
    };

    let required_badge_ins = (days_in_quarter + 1) / 2;
    let mut days_still_needed = required_badge_ins - days_badged_in;

    let required_future_average = if days_still_needed > 0 {
        if days_left > 0 {
            100.0 * days_still_needed as f64 / days_left as f64
        } else if end >= today {
            100.0
        } else {
            0.0
        }
    } else {
        0.0
    };

    let target_pace_percentage = if days_in_quarter > 0 {
        required_badge_ins as f64 / days_in_quarter as f64
    } else {
        0.5
    };

    let expected_badge_ins_so_far = (days_thus_far as f64 * target_pace_percentage) as i32;
    let days_ahead_of_pace = days_badged_in - expected_badge_ins_so_far;

    let total_missable_days = days_in_quarter - required_badge_ins;
    let days_missed_so_far = days_thus_far - days_badged_in;
    let mut remaining_missable_days = total_missable_days - days_missed_so_far;

    let status = if days_badged_in >= required_badge_ins {
        "Achieved".to_string()
    } else if days_still_needed > days_left {
        "Impossible".to_string()
    } else if required_future_average > target_pace_percentage * 100.0 {
        "At Risk".to_string()
    } else {
        "On Track".to_string()
    };

    if days_still_needed < 0 {
        days_still_needed = 0;
    }
    if remaining_missable_days < 0 {
        remaining_missable_days = 0;
    }

    let projected_completion_date = if days_still_needed > 0 && days_left > 0 && days_thus_far > 0
    {
        let average_rate = days_badged_in as f64 / days_thus_far as f64;
        if average_rate > 0.0 {
            let estimated_remaining = (days_still_needed as f64 / average_rate) as i64;
            today.checked_add_days(chrono::Days::new(estimated_remaining as u64))
        } else {
            None
        }
    } else {
        None
    };

    Ok(QuarterStats {
        quarter: quarter.key.clone(),
        start_date: start,
        end_date: end,
        days_badged_in,
        flex_days,
        days_thus_far,
        days_left,
        total_calendar_days,
        available_workdays,
        total_days: days_in_quarter,
        days_required: required_badge_ins,
        days_still_needed,
        days_off,
        holidays,
        vacation_days,
        current_average,
        required_future_average,
        compliance_status: status,
        days_ahead_of_pace,
        remaining_missable_days,
        projected_completion_date,
        workday_stats: workday_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{BadgeEntryData, HolidayData, VacationData};

    fn make_quarter(start: &str, end: &str) -> QuarterConfig {
        let mut q = QuarterConfig {
            key: "TEST_Q".to_string(),
            quarter: "Q1".to_string(),
            year: "2025".to_string(),
            start_date_raw: start.to_string(),
            end_date_raw: end.to_string(),
            start_date: None,
            end_date: None,
        };
        q.parse_dates().unwrap();
        q
    }

    #[test]
    fn test_on_track_status() {
        // 10-day quarter (Mon-Fri x 2 weeks), badge in 3 of first 5 days, today is day 6
        let q = make_quarter("2025-01-06", "2025-01-17");
        let mut badge = BadgeEntryData::default();
        badge.add(BadgeEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
            "Office",
            false,
        ));
        badge.add(BadgeEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(),
            "Office",
            false,
        ));
        badge.add(BadgeEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 8).unwrap(),
            "Office",
            false,
        ));
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = NaiveDate::from_ymd_opt(2025, 1, 13).unwrap();
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, Some(today)).unwrap();
        assert_eq!(stats.days_badged_in, 3);
        assert!(stats.compliance_status == "On Track" || stats.compliance_status == "At Risk");
    }

    #[test]
    fn test_achieved_status() {
        // Short quarter: 4 days, badge in all 4 → required = 2, achieved
        let q = make_quarter("2025-01-06", "2025-01-09");
        let mut badge = BadgeEntryData::default();
        for d in 6..=9 {
            badge.add(BadgeEntry::new(
                NaiveDate::from_ymd_opt(2025, 1, d).unwrap(),
                "Office",
                false,
            ));
        }
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = NaiveDate::from_ymd_opt(2025, 1, 20).unwrap();
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, Some(today)).unwrap();
        assert_eq!(stats.compliance_status, "Achieved");
    }

    #[test]
    fn test_impossible_status() {
        // 4-day quarter, badge in 0, today is past the end
        let q = make_quarter("2025-01-06", "2025-01-09");
        let badge = BadgeEntryData::default();
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = NaiveDate::from_ymd_opt(2025, 1, 20).unwrap();
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, Some(today)).unwrap();
        assert_eq!(stats.compliance_status, "Impossible");
    }

    #[test]
    fn test_days_ahead_of_pace_positive() {
        // Badge in more than expected
        let q = make_quarter("2025-01-06", "2025-01-17");
        let mut badge = BadgeEntryData::default();
        badge.add(BadgeEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
            "Office",
            false,
        ));
        badge.add(BadgeEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(),
            "Office",
            false,
        ));
        badge.add(BadgeEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 8).unwrap(),
            "Office",
            false,
        ));
        badge.add(BadgeEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 9).unwrap(),
            "Office",
            false,
        ));
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, Some(today)).unwrap();
        assert!(stats.days_ahead_of_pace > 0);
    }

    #[test]
    fn test_remaining_missable_days() {
        // 10 work days, badge in 5 of first 5 (all), remaining missable = total_missable - missed_so_far
        let q = make_quarter("2025-01-06", "2025-01-17");
        let mut badge = BadgeEntryData::default();
        for d in [6, 7, 8, 9, 10] {
            badge.add(BadgeEntry::new(
                NaiveDate::from_ymd_opt(2025, 1, d).unwrap(),
                "Office",
                false,
            ));
        }
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = NaiveDate::from_ymd_opt(2025, 1, 13).unwrap();
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, Some(today)).unwrap();
        assert!(stats.remaining_missable_days >= 0);
    }

    #[test]
    fn test_empty_quarter_no_divide_by_zero() {
        // Quarter with all holidays (all days off) → protect from div/0
        let q = make_quarter("2025-01-06", "2025-01-06");
        let badge = BadgeEntryData::default();
        let mut holiday = HolidayData::default();
        holiday.add(Holiday::new("Test", "2025-01-06"));
        let vacation = VacationData::default();
        let today = NaiveDate::from_ymd_opt(2025, 1, 7).unwrap();
        let stats =
            calculate_quarter_stats(&q, &badge, &holiday, &vacation, Some(today)).unwrap();
        assert_eq!(stats.total_days, 0);
        assert_eq!(stats.current_average, 0.0);
        assert_eq!(stats.flex_days, 0);
    }

    #[test]
    fn test_flex_credit_counted_separately() {
        // 4-day quarter, 2 office badge-ins and 1 flex credit — all count toward goal
        let q = make_quarter("2025-01-06", "2025-01-09");
        let mut badge = BadgeEntryData::default();
        badge.add(BadgeEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
            "Office",
            false,
        ));
        badge.add(BadgeEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(),
            "Flex Credit",
            true,
        ));
        badge.add(BadgeEntry::new(
            NaiveDate::from_ymd_opt(2025, 1, 8).unwrap(),
            "Office",
            false,
        ));
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = NaiveDate::from_ymd_opt(2025, 1, 20).unwrap();
        let stats = calculate_quarter_stats(&q, &badge, &holiday, &vacation, Some(today)).unwrap();
        assert_eq!(stats.days_badged_in, 3); // all 3 count toward goal
        assert_eq!(stats.flex_days, 1); // only 1 is a flex credit
        assert_eq!(stats.compliance_status, "Achieved");
    }

    #[test]
    fn test_working_days_and_available_workdays() {
        // 5-day week Mon–Fri 2025-01-06 to 2025-01-10 (7 calendar days with weekend).
        // Mark Mon as holiday and Wed as vacation.
        //
        // total_calendar_days = 5 (Mon–Fri, no weekend in this range)
        // available_workdays  = Total Working Days = weekdays not holiday
        //                     = Tue + Wed + Thu + Fri = 4
        // total_days          = Available Working Days = weekdays not holiday not vacation
        //                     = Tue + Thu + Fri = 3
        let q = make_quarter("2025-01-06", "2025-01-10");
        let badge = BadgeEntryData::default();
        let mut holiday = HolidayData::default();
        holiday.add(Holiday::new("Holiday", "2025-01-06")); // Monday
        let mut vacation = VacationData::default();
        vacation.add(Vacation::new("Trip", "2025-01-08", "2025-01-08", true)); // Wednesday
        let today = NaiveDate::from_ymd_opt(2025, 1, 20).unwrap();
        let stats = calculate_quarter_stats(&q, &badge, &holiday, &vacation, Some(today)).unwrap();
        assert_eq!(stats.total_calendar_days, 5, "calendar days Mon–Fri");
        assert_eq!(stats.available_workdays, 4, "total working days: Tue+Wed+Thu+Fri");
        assert_eq!(stats.total_days, 3, "available working days: Tue+Thu+Fri");
    }

    #[test]
    fn test_total_calendar_days_includes_weekends() {
        // 2025-01-06 (Mon) to 2025-01-12 (Sun) = 7 calendar days
        let q = make_quarter("2025-01-06", "2025-01-12");
        let badge = BadgeEntryData::default();
        let holiday = HolidayData::default();
        let vacation = VacationData::default();
        let today = NaiveDate::from_ymd_opt(2025, 1, 20).unwrap();
        let stats = calculate_quarter_stats(&q, &badge, &holiday, &vacation, Some(today)).unwrap();
        assert_eq!(stats.total_calendar_days, 7, "Mon–Sun = 7 calendar days");
        assert_eq!(stats.available_workdays, 5, "Mon–Fri = 5 working days, no holidays");
        assert_eq!(stats.total_days, 5, "no vacation, same as available");
    }

    use crate::data::badge_entry::BadgeEntry;
    use crate::data::holiday::Holiday;
    use crate::data::vacation::Vacation;
}
