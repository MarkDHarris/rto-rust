use crate::data::{HolidayData, Persistable};
use anyhow::Result;

pub fn run() -> Result<()> {
    let holiday_data = HolidayData::load()?;
    write_holidays(&holiday_data, &mut std::io::stdout())
}

pub(crate) fn write_holidays<W: std::io::Write>(data: &HolidayData, out: &mut W) -> Result<()> {
    let all = data.all();
    if all.is_empty() {
        writeln!(out, "No holidays recorded.")?;
        return Ok(());
    }

    writeln!(out, "{:<12}  Name", "Date")?;
    writeln!(
        out,
        "{:<12}  ------------------------------",
        "------------"
    )?;

    for h in &all {
        writeln!(out, "{:<12}  {}", h.date, h.name)?;
    }
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
        assert!(out.contains("No holidays recorded"));
    }

    #[test]
    fn test_write_holidays_single() {
        let data = make_data(vec![Holiday::new("New Year's Day", "2025-01-01")]);
        let mut buf = Vec::new();
        write_holidays(&data, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("New Year's Day"));
        assert!(out.contains("2025-01-01"));
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
