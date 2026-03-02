use crate::data::{Persistable, VacationData};
use anyhow::Result;

pub fn run() -> Result<()> {
    let vacation_data = VacationData::load()?;
    write_vacations(&vacation_data, &mut std::io::stdout())
}

pub(crate) fn write_vacations<W: std::io::Write>(data: &VacationData, out: &mut W) -> Result<()> {
    let all = data.all();
    if all.is_empty() {
        writeln!(out, "No vacations recorded.")?;
        return Ok(());
    }

    writeln!(
        out,
        "{:<4}  {:<30}  {:<12}  {:<12}  Approved",
        "#", "Destination", "Start", "End"
    )?;
    writeln!(
        out,
        "{:<4}  {:<30}  {:<12}  {:<12}  --------",
        "----", "------------------------------", "------------", "------------"
    )?;

    for (i, v) in all.iter().enumerate() {
        let approved = if v.approved { "Yes" } else { "No" };
        writeln!(
            out,
            "{:<4}  {:<30}  {:<12}  {:<12}  {}",
            i + 1,
            truncate(&v.destination, 30),
            v.start_date,
            v.end_date,
            approved,
        )?;
    }
    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_len {
        return s.to_string();
    }
    let truncated: String = chars[..max_len - 3].iter().collect();
    format!("{}...", truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::vacation::Vacation;

    fn make_data(vacations: Vec<Vacation>) -> VacationData {
        VacationData { vacations }
    }

    #[test]
    fn test_write_vacations_empty() {
        let data = make_data(vec![]);
        let mut buf = Vec::new();
        write_vacations(&data, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("No vacations recorded"));
    }

    #[test]
    fn test_write_vacations_single_entry() {
        let data = make_data(vec![Vacation::new(
            "Hawaii",
            "2025-05-10",
            "2025-05-17",
            true,
        )]);
        let mut buf = Vec::new();
        write_vacations(&data, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("Hawaii"));
        assert!(out.contains("2025-05-10"));
        assert!(out.contains("Yes"));
    }

    #[test]
    fn test_write_vacations_unapproved() {
        let data = make_data(vec![Vacation::new(
            "Paris",
            "2025-06-01",
            "2025-06-07",
            false,
        )]);
        let mut buf = Vec::new();
        write_vacations(&data, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("No"));
    }

    #[test]
    fn test_write_vacations_multiple() {
        let data = make_data(vec![
            Vacation::new("Hawaii", "2025-05-10", "2025-05-17", true),
            Vacation::new("Paris", "2025-08-01", "2025-08-14", true),
        ]);
        let mut buf = Vec::new();
        write_vacations(&data, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("Paris"));
    }

    #[test]
    fn test_truncate_long_string() {
        assert_eq!(truncate("Hello World", 30), "Hello World");
        let result = truncate("A very long destination name that exceeds the limit", 30);
        assert_eq!(result.chars().count(), 30);
        assert!(result.ends_with("..."));
    }
}
