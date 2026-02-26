use crate::data::{Persistable, VacationData};
use anyhow::Result;

pub fn run() -> Result<()> {
    let vacation_data = VacationData::load()?;
    write_vacations(&vacation_data, &mut std::io::stdout())
}

pub(crate) fn write_vacations<W: std::io::Write>(data: &VacationData, out: &mut W) -> Result<()> {
    writeln!(out, "Vacations")?;
    writeln!(out, "---")?;
    writeln!(
        out,
        "  {:<4} {:<24} {:<14} {:<14} {}",
        "#", "Destination", "Start", "End", "Approved"
    )?;
    for (i, v) in data.vacations.iter().enumerate() {
        writeln!(
            out,
            "  {:<4} {:<24} {:<14} {:<14} {}",
            i + 1,
            v.destination,
            v.start_date,
            v.end_date,
            if v.approved { "Yes" } else { "No" }
        )?;
    }
    writeln!(out, "---")?;
    writeln!(out, "Total: {} vacation(s)", data.vacations.len())?;
    Ok(())
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
        assert!(out.contains("Total: 0 vacation(s)"));
    }

    #[test]
    fn test_write_vacations_single_entry() {
        let data = make_data(vec![Vacation::new("Hawaii", "2025-05-10", "2025-05-17", true)]);
        let mut buf = Vec::new();
        write_vacations(&data, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("Hawaii"));
        assert!(out.contains("2025-05-10"));
        assert!(out.contains("Yes"));
        assert!(out.contains("Total: 1 vacation(s)"));
    }

    #[test]
    fn test_write_vacations_unapproved() {
        let data = make_data(vec![Vacation::new("Paris", "2025-06-01", "2025-06-07", false)]);
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
        assert!(out.contains("Total: 2 vacation(s)"));
        assert!(out.contains("Paris"));
    }
}
