use crate::data::{
    persistence::get_data_dir, AppSettings, BadgeEntryData, EventData, HolidayData, Persistable,
    QuarterData, VacationData,
};
use crate::ui::calendar_view::{run_app, App};
use crate::ui::{restore_terminal, setup_terminal};
use anyhow::Result;
use chrono::Local;

pub fn run() -> Result<()> {
    let quarter_data = QuarterData::load_and_parse()?;
    let settings = AppSettings::load()?;
    let mut badge_data = BadgeEntryData::load()?;
    let mut holiday_data = HolidayData::load()?;
    let mut vacation_data = VacationData::load()?;
    let mut event_data = EventData::load()?;

    // Install panic hook to restore terminal on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen
        );
        original_hook(info);
    }));

    let mut terminal = setup_terminal()?;

    let today = Local::now().date_naive();
    let data_dir = get_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("./config"));
    let mut app = App::new(
        &quarter_data,
        &mut badge_data,
        &mut holiday_data,
        &mut vacation_data,
        &mut event_data,
        settings,
        today,
        data_dir.clone(),
    );

    let result = run_app(&mut terminal, &mut app);

    restore_terminal(&mut terminal)?;

    // Extract settings before dropping app (which holds borrows on the data fields)
    let final_settings = app.settings.clone();
    drop(app);

    // Save all modified data
    badge_data.save()?;
    event_data.save()?;
    vacation_data.save()?;
    holiday_data.save()?;
    crate::cmd::init::save_settings_to(&final_settings, &data_dir)?;

    result
}
