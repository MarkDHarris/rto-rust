use crate::data::{HolidayData, Persistable};
use anyhow::Result;

pub fn run() -> Result<()> {
    let holiday_data = HolidayData::load()?;
    write_holidays(&holiday_data, &mut std::io::stdout())
}

pub(crate) fn write_holidays<W: std::io::Write>(data: &HolidayData, out: &mut W) -> Result<()> {
    writeln!(out, "Holidays")?;
    writeln!(out, "---")?;
    writeln!(out, "  {:<14} {}", "Date", "Name")?;
    for h in &data.holidays {
        writeln!(out, "  {:<14} {}", h.date, h.name)?;
    }
    writeln!(out, "---")?;
    writeln!(out, "Total: {} holiday(s)", data.holidays.len())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::holiday::Holiday;

    fn make_data(holidays: Vec<Holiday>) -> HolidayData {
        HolidayData { holidays }
    }

    #[test]
    fn test_write_holidays_empty() {
        let data = make_data(vec![]);
        let mut buf = Vec::new();
        write_holidays(&data, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("Total: 0 holiday(s)"));
    }

    #[test]
    fn test_write_holidays_single() {
        let data = make_data(vec![Holiday::new("New Year's Day", "2025-01-01")]);
        let mut buf = Vec::new();
        write_holidays(&data, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("New Year's Day"));
        assert!(out.contains("2025-01-01"));
        assert!(out.contains("Total: 1 holiday(s)"));
    }

    #[test]
    fn test_write_holidays_multiple() {
        let data = make_data(vec![
            Holiday::new("New Year's Day", "2025-01-01"),
            Holiday::new("Independence Day", "2025-07-04"),
        ]);
        let mut buf = Vec::new();
        write_holidays(&data, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("Total: 2 holiday(s)"));
        assert!(out.contains("Independence Day"));
    }

    #[test]
    fn test_write_holidays_date_column_aligned() {
        let data = make_data(vec![Holiday::new("MLK Day", "2025-01-20")]);
        let mut buf = Vec::new();
        write_holidays(&data, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("2025-01-20"));
        assert!(out.contains("MLK Day"));
    }
}
