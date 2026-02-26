use crate::data::app_settings::AppSettings;
use crate::data::badge_entry::{BadgeEntry, BadgeEntryData};
use crate::data::holiday::{Holiday, HolidayData};
use crate::data::persistence::{get_data_dir, Persistable};
use crate::data::quarter::{QuarterConfig, QuarterData};
use crate::data::vacation::{Vacation, VacationData};
use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveDateTime, Utc};
use std::fs;

pub fn init() -> Result<()> {
    let data_dir = get_data_dir()?;

    fs::create_dir_all(&data_dir)?;

    println!("Initializing data directory: {}", data_dir.display());
    println!("\nGenerating sample data...");

    generate_badge_entries()?;

    println!("✓ badge_data.json created");

    generate_holidays()?;

    println!("✓ holidays.yaml created");

    generate_quarter_config()?;

    println!("✓ config.yaml created");

    generate_vacations()?;

    println!("✓ vacations.yaml created");

    generate_app_settings()?;

    println!("✓ app_settings created");

    println!("\n✓ Initialization complete!");
    println!(
        "\nNote: You may need to run 'cargo run -- --data-dir {}' with this directory.",
        data_dir.display()
    );

    Ok(())
}

fn generate_badge_entries() -> Result<()> {
    let mut badge_data = BadgeEntryData::default();

    let current_date = Utc::now().naive_utc();

    let mut current = current_date - chrono::Duration::days(14);
    for _ in 0..12 {
        badge_data.add(BadgeEntry::new(current.date(), "Main Office", false));
        current = current + chrono::Duration::days(1);
    }

    let _ = badge_data.save();

    Ok(())
}

fn generate_holidays() -> Result<()> {
    let mut holiday_data = HolidayData::default();

    let mut current = Utc::now().naive_utc();

    holiday_data.add(Holiday::new(
        "New Year's Day",
        &current.format("%Y-%04-01").to_string(),
    ));
    holiday_data.add(Holiday::new(
        "Memorial Day",
        &current.format("%Y-%04-29").to_string(),
    ));

    let june = current.replace_month(current.month() + 5);
    if june.ok() == Some(june) {
        holiday_data.add(Holiday::new(
            "Juneteenth",
            &june.format("%Y-%06-19").to_string(),
        ));

        let independence = june.replace_month(june.month() + 4);
        if independence.ok() == Some(independence) {
            holiday_data.add(Holiday::new(
                "Independence Day",
                &independence.format("%Y-%07-04").to_string(),
            ));
        }

        let labor = june.replace_month(june.month() + 2);
        if labor.ok() == Some(labor) {
            holiday_data.add(Holiday::new(
                "Labor Day",
                &labor.format("%Y-%09-01").to_string(),
            ));
        }

        let november = independence.replace_month(independence.month() + 3);
        if november.ok() == Some(november) {
            holiday_data.add(Holiday::new(
                "Veterans Day",
                &november.format("%Y-%11-11").to_string(),
            ));
        }

        let december = november.replace_month(november.month() + 1);
        if december.ok() == Some(december) {
            holiday_data.add(Holiday::new(
                "Christmas",
                &december.format("%Y-%12-25").to_string(),
            ));
        }
    }

    let _ = holiday_data.save();

    Ok(())
}

fn generate_quarter_config() -> Result<()> {
    let mut quarter_data = QuarterData::default();

    let current_year = Utc::now().year();

    quarter_data.quarters.push(QuarterConfig {
        key: "Q1_{}_2026".to_string(),
        quarter: "1".to_string(),
        year: current_year.to_string(),
        start_date_raw: "2026-01-01".to_string(),
        end_date_raw: "2026-03-31".to_string(),
        ..Default::default()
    });

    quarter_data.quarters.push(QuarterConfig {
        key: "Q2_{}_2026".to_string(),
        quarter: "2".to_string(),
        year: current_year.to_string(),
        start_date_raw: "2026-04-01".to_string(),
        end_date_raw: "2026-06-30".to_string(),
        ..Default::default()
    });

    quarter_data.quarters.push(QuarterConfig {
        key: "Q3_{}_2026".to_string(),
        quarter: "3".to_string(),
        year: current_year.to_string(),
        start_date_raw: "2026-07-01".to_string(),
        end_date_raw: "2026-09-30".to_string(),
        ..Default::default()
    });

    quarter_data.quarters.push(QuarterConfig {
        key: "Q4_{}_2026".to_string(),
        quarter: "4".to_string(),
        year: current_year.to_string(),
        start_date_raw: "2026-10-01".to_string(),
        end_date_raw: "2026-12-31".to_string(),
        ..Default::default()
    });

    let _ = quarter_data.save();

    Ok(())
}

fn generate_vacations() -> Result<()> {
    let mut vacation_data = VacationData::default();

    let current = Utc::now().naive_utc();

    let next_month = current + chrono::Duration::days(30);

    vacation_data.add(Vacation::new(
        "Beach Vacation",
        &next_month.format("%Y-%m-%d").to_string(),
        &next_month
            .replace_day(next_month.day() + 7)
            .unwrap()
            .format("%Y-%m-%d")
            .to_string(),
        true,
    ));

    vacation_data.add(Vacation::new(
        "Mountain Retreat",
        &current.format("%Y-%03-01").to_string(),
        &current
            .replace_month(current.month() + 1)
            .unwrap()
            .format("%Y-%03-31")
            .to_string(),
        true,
    ));

    let _ = vacation_data.save();

    Ok(())
}

fn generate_app_settings() -> Result<()> {
    let settings = AppSettings::default();
    settings.save()
}
