use crate::data::{
    AppSettings, BadgeEntryData, EventData, HolidayData, Persistable, TimePeriodData, VacationData,
    persistence::get_data_dir,
};
use crate::ui::calendar_view::{App, run_app};
use crate::ui::{restore_terminal, setup_terminal};
use anyhow::Result;
use chrono::Local;

pub fn run() -> Result<()> {
    let settings = AppSettings::load()?;
    let data_dir = get_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("./config"));

    let tp_file = settings.active_time_period_file(0);
    let time_period_data = TimePeriodData::load_from(&data_dir, tp_file)?;
    let mut badge_data = BadgeEntryData::load()?;
    let mut holiday_data = HolidayData::load()?;
    let mut vacation_data = VacationData::load()?;
    let mut event_data = EventData::load()?;

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        original_hook(info);
    }));

    let mut terminal = setup_terminal()?;

    let today = Local::now().date_naive();
    let mut app = App::new(
        time_period_data,
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

    let final_settings = app.settings.clone();
    drop(app);

    badge_data.save()?;
    event_data.save()?;
    vacation_data.save()?;
    holiday_data.save()?;
    final_settings.save_to(&data_dir)?;

    result
}
