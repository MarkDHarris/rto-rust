use crate::calc::{QuarterStats, calculate_quarter_stats, calculate_year_stats};
use crate::data::{
    AppSettings, BadgeEntry, BadgeEntryData, Event, EventData, Holiday, HolidayData, TimePeriod,
    TimePeriodData, Vacation, VacationData,
};
use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate};
use crossterm::event::{self, Event as CEvent, KeyCode, KeyModifiers};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};
use std::io::Stdout;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration as StdDuration;

// Calendar cell colors
const FLEX_COLOR: Color = Color::Indexed(208); // reddish-orange

// Stats section header style
const SECTION_BG: Color = Color::Rgb(40, 44, 52);

#[derive(PartialEq)]
enum Mode {
    Normal,
    Add,
    Delete,
    Search,
}

#[derive(PartialEq, Default)]
enum ViewState {
    #[default]
    Calendar,
    Vacations,
    Holidays,
    Settings,
}

pub struct App<'a> {
    time_period_data: TimePeriodData,
    badge_data: &'a mut BadgeEntryData,
    holiday_data: &'a mut HolidayData,
    vacation_data: &'a mut VacationData,
    event_data: &'a mut EventData,
    selected_date: NaiveDate,
    today: NaiveDate,
    nav_date: NaiveDate,
    mode: Mode,
    input_buffer: String,
    cursor_index: usize,
    active_stats: Option<QuarterStats>,
    year_stats: Option<QuarterStats>,
    table_state: TableState,
    pub settings: AppSettings,
    what_if_snapshot: Option<BadgeEntryData>,
    data_dir: PathBuf,
    active_time_period_idx: usize,
    git_status: Option<(String, Color)>,
    view_state: ViewState,
    list_cursor: usize,
    list_add_stage: u8,
    list_field_bufs: Vec<String>,
    list_edit_index: Option<usize>,
}

impl<'a> App<'a> {
    fn current_period(&self) -> Option<&TimePeriod> {
        self.time_period_data.get_period_by_date(self.nav_date)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        time_period_data: TimePeriodData,
        badge_data: &'a mut BadgeEntryData,
        holiday_data: &'a mut HolidayData,
        vacation_data: &'a mut VacationData,
        event_data: &'a mut EventData,
        settings: AppSettings,
        today: NaiveDate,
        data_dir: PathBuf,
    ) -> Self {
        let period = time_period_data.get_period_by_date(today);
        let selected_date = today;
        let nav_date = period.and_then(|q| q.start_date).unwrap_or(today);
        let mut app = App {
            time_period_data,
            badge_data,
            holiday_data,
            vacation_data,
            event_data,
            selected_date,
            today,
            nav_date,
            mode: Mode::Normal,
            input_buffer: String::new(),
            cursor_index: 0,
            active_stats: None,
            year_stats: None,
            table_state: TableState::default(),
            settings,
            what_if_snapshot: None,
            data_dir,
            active_time_period_idx: 0,
            git_status: None,
            view_state: ViewState::Calendar,
            list_cursor: 0,
            list_add_stage: 0,
            list_field_bufs: Vec::new(),
            list_edit_index: None,
        };
        app.update_stats();
        app
    }

    fn update_stats(&mut self) {
        if let Some(q) = self.current_period() {
            match calculate_quarter_stats(
                q,
                self.badge_data,
                self.holiday_data,
                self.vacation_data,
                self.settings.goal,
                None,
            ) {
                Ok(stats) => self.active_stats = Some(stats),
                Err(e) => {
                    self.active_stats = None;
                    eprintln!("Error calculating stats: {e}");
                }
            }
        } else {
            self.active_stats = None;
        }
        self.update_year_stats();
    }

    fn update_year_stats(&mut self) {
        let year = match self.current_period() {
            Some(q) => q.start_date.map(|d| d.year()).unwrap_or(self.today.year()),
            None => {
                self.year_stats = None;
                return;
            }
        };

        let all = self.time_period_data.all();
        let year_periods: Vec<&TimePeriod> = all
            .iter()
            .filter(|tp| tp.start_date.map(|d| d.year()) == Some(year))
            .collect();

        if year_periods.is_empty() {
            self.year_stats = None;
            return;
        }

        match calculate_year_stats(
            &year_periods,
            self.badge_data,
            self.holiday_data,
            self.vacation_data,
            self.settings.goal,
            None,
        ) {
            Ok(Some(stats)) => self.year_stats = Some(stats),
            _ => self.year_stats = None,
        }
    }

    fn switch_time_period_view(&mut self, dir: i32) {
        let n = self.settings.time_periods.len();
        if n <= 1 {
            return;
        }
        let new_idx = ((self.active_time_period_idx as i32 + dir).rem_euclid(n as i32)) as usize;
        self.active_time_period_idx = new_idx;
        let tp_file = self.settings.active_time_period_file(new_idx);
        match TimePeriodData::load_from(&self.data_dir, tp_file) {
            Ok(td) => {
                self.time_period_data = td;
                self.nav_date = self.today;
                self.selected_date = self.today;
                if let Some(p) = self.time_period_data.get_period_by_date(self.selected_date) {
                    if let Some(start) = p.start_date {
                        self.nav_date = start;
                    }
                } else if let Ok(p) = self.time_period_data.nearest_period(self.selected_date)
                    && let Some(start) = p.start_date
                {
                    self.nav_date = start;
                    self.selected_date = start;
                }
                self.update_stats();
            }
            Err(_e) => {
                self.git_status = Some(("Error loading time period file".to_string(), Color::Red));
            }
        }
    }

    fn navigate_to_adjacent_period(&mut self, dir: i32) {
        let current = match self.current_period() {
            Some(p) => p.key.clone(),
            None => return,
        };
        let all = self.time_period_data.all();
        for (i, tp) in all.iter().enumerate() {
            if tp.key == current {
                let next = i as i32 + dir;
                if next >= 0 && (next as usize) < all.len() {
                    let np = &all[next as usize];
                    if let Some(start) = np.start_date {
                        self.selected_date = start;
                        self.nav_date = start;
                        self.update_stats();
                    }
                }
                return;
            }
        }
    }

    fn is_what_if(&self) -> bool {
        self.what_if_snapshot.is_some()
    }

    fn enter_what_if(&mut self) {
        self.what_if_snapshot = Some(self.badge_data.clone());
    }

    fn exit_what_if(&mut self) {
        if let Some(original) = self.what_if_snapshot.take() {
            *self.badge_data = original;
            self.update_stats();
        }
    }

    /// Git-add, commit, and optionally push the data directory.
    /// Sets self.git_status with a result message. Never panics.
    fn git_backup(&mut self) {
        let dir = self.data_dir.to_string_lossy().to_string();

        // 1. Confirm it's a git repo
        let is_repo = Command::new("git")
            .args(["-C", &dir, "rev-parse", "--is-inside-work-tree"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !is_repo {
            self.git_status = Some((
                format!("'{}' is not a git repo — backup skipped", dir),
                Color::DarkGray,
            ));
            return;
        }

        // 2. git add .
        let add_ok = Command::new("git")
            .args(["-C", &dir, "add", "."])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !add_ok {
            self.git_status = Some(("git add failed".to_string(), Color::Red));
            return;
        }

        // 3. git commit with a unique timestamp
        let now = Local::now();
        let timestamp = now.format("%Y-%m-%d-%H-%M-%S-%3f").to_string();
        let msg = format!("backup {}", timestamp);

        let commit_out = Command::new("git")
            .args(["-C", &dir, "commit", "-m", &msg])
            .output();

        match &commit_out {
            Err(e) => {
                self.git_status = Some((format!("git commit error: {}", e), Color::Red));
                return;
            }
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                if stdout.contains("nothing to commit")
                    || stderr.contains("nothing to commit")
                    || stdout.contains("nothing added")
                {
                    self.git_status = Some((
                        "Nothing to commit — already up to date".to_string(),
                        Color::Yellow,
                    ));
                    return;
                }
                if !out.status.success() {
                    let detail = stdout.trim().to_string();
                    self.git_status = Some((format!("git commit failed: {}", detail), Color::Red));
                    return;
                }
            }
        }

        // 4. Check for a remote named "origin"
        let has_remote = Command::new("git")
            .args(["-C", &dir, "remote", "get-url", "origin"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if has_remote {
            // 5. git push
            let push_ok = Command::new("git")
                .args(["-C", &dir, "push"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if push_ok {
                self.git_status =
                    Some((format!("Backed up & pushed — {}", timestamp), Color::Green));
            } else {
                self.git_status = Some((
                    format!("Committed locally (push failed) — {}", timestamp),
                    Color::Yellow,
                ));
            }
        } else {
            self.git_status = Some((format!("Backed up locally — {}", timestamp), Color::Cyan));
        }
    }

    /// Returns true if the app should quit.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        // Dispatch to view-specific handlers when not in Calendar view
        match self.view_state {
            ViewState::Vacations => {
                self.handle_vacation_key(code);
                return false;
            }
            ViewState::Holidays => {
                self.handle_holiday_key(code);
                return false;
            }
            ViewState::Settings => {
                self.handle_settings_key(code);
                return false;
            }
            ViewState::Calendar => {}
        }

        // Clear the git status message on every keypress
        self.git_status = None;

        match self.mode {
            Mode::Add => {
                match code {
                    KeyCode::Enter => {
                        if !self.input_buffer.is_empty() {
                            let date_key = self.selected_date.format("%Y-%m-%d").to_string();
                            let event = Event {
                                date: date_key,
                                description: self.input_buffer.clone(),
                            };
                            self.event_data.add(event);
                        }
                        self.input_buffer.clear();
                        self.mode = Mode::Normal;
                    }
                    KeyCode::Esc => {
                        self.input_buffer.clear();
                        self.mode = Mode::Normal;
                    }
                    KeyCode::Backspace => {
                        self.input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        self.input_buffer.push(c);
                    }
                    _ => {}
                }
                false
            }

            Mode::Delete => {
                let date_key = self.selected_date.format("%Y-%m-%d").to_string();
                let events: Vec<_> = self
                    .event_data
                    .events
                    .iter()
                    .filter(|e| e.date == date_key)
                    .cloned()
                    .collect();
                match code {
                    KeyCode::Enter => {
                        if !events.is_empty() && self.cursor_index < events.len() {
                            let desc = events[self.cursor_index].description.clone();
                            self.event_data.remove(&date_key, &desc);
                            let new_len = events.len() - 1;
                            if self.cursor_index > 0 && self.cursor_index >= new_len {
                                self.cursor_index -= 1;
                            }
                        }
                        self.mode = Mode::Normal;
                    }
                    KeyCode::Esc => {
                        self.mode = Mode::Normal;
                    }
                    KeyCode::Up => {
                        if self.cursor_index > 0 {
                            self.cursor_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if !events.is_empty() && self.cursor_index < events.len() - 1 {
                            self.cursor_index += 1;
                        }
                    }
                    _ => {}
                }
                false
            }

            Mode::Search => {
                match code {
                    KeyCode::Enter | KeyCode::Esc => {
                        self.mode = Mode::Normal;
                        self.input_buffer.clear();
                    }
                    KeyCode::Backspace => {
                        self.input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        self.input_buffer.push(c);
                    }
                    _ => {}
                }
                false
            }

            Mode::Normal => {
                match code {
                    KeyCode::Right if modifiers.contains(KeyModifiers::SHIFT) => {
                        self.switch_time_period_view(1);
                    }
                    KeyCode::Left if modifiers.contains(KeyModifiers::SHIFT) => {
                        self.switch_time_period_view(-1);
                    }
                    KeyCode::Left => {
                        if let Some(d) = self.selected_date.checked_sub_signed(Duration::days(1)) {
                            self.selected_date = d;
                        }
                    }
                    KeyCode::Right => {
                        if let Some(d) = self.selected_date.checked_add_signed(Duration::days(1)) {
                            self.selected_date = d;
                        }
                    }
                    KeyCode::Up => {
                        if let Some(d) = self.selected_date.checked_sub_signed(Duration::days(7)) {
                            self.selected_date = d;
                        }
                    }
                    KeyCode::Down => {
                        if let Some(d) = self.selected_date.checked_add_signed(Duration::days(7)) {
                            self.selected_date = d;
                        }
                    }
                    KeyCode::Char(' ') => {
                        self.switch_time_period_view(1);
                    }
                    KeyCode::Char('b') => {
                        if self.current_period().is_some() {
                            let date_key = self.selected_date.format("%Y-%m-%d").to_string();
                            if self.badge_data.has(&date_key) {
                                self.badge_data.remove(&date_key);
                            } else {
                                let office = self.settings.default_office.clone();
                                let entry = BadgeEntry::new(self.selected_date, &office, false);
                                self.badge_data.add(entry);
                            }
                            self.update_stats();
                        }
                    }
                    KeyCode::Char('f') => {
                        if self.current_period().is_some() {
                            let date_key = self.selected_date.format("%Y-%m-%d").to_string();
                            if self.badge_data.has(&date_key) {
                                self.badge_data.remove(&date_key);
                            } else {
                                let flex = self.settings.flex_credit.clone();
                                let entry = BadgeEntry::new(self.selected_date, &flex, true);
                                self.badge_data.add(entry);
                            }
                            self.update_stats();
                        }
                    }
                    KeyCode::Char('g') => {
                        self.git_backup();
                        // Don't clear git_status — we just set it
                    }
                    KeyCode::Char('w') => {
                        if self.is_what_if() {
                            self.exit_what_if();
                        } else {
                            self.enter_what_if();
                        }
                    }
                    KeyCode::Char('n') => {
                        self.navigate_to_adjacent_period(1);
                    }
                    KeyCode::Char('p') => {
                        self.navigate_to_adjacent_period(-1);
                    }
                    KeyCode::Char('a') => {
                        self.mode = Mode::Add;
                        self.input_buffer.clear();
                    }
                    KeyCode::Char('d') => {
                        self.mode = Mode::Delete;
                        self.cursor_index = 0;
                    }
                    KeyCode::Char('s') => {
                        self.mode = Mode::Search;
                        self.input_buffer.clear();
                    }
                    KeyCode::Char('v') => {
                        self.view_state = ViewState::Vacations;
                        self.list_cursor = 0;
                        self.list_add_stage = 0;
                        self.list_field_bufs.clear();
                        self.list_edit_index = None;
                        self.input_buffer.clear();
                    }
                    KeyCode::Char('h') => {
                        self.view_state = ViewState::Holidays;
                        self.list_cursor = 0;
                        self.list_add_stage = 0;
                        self.list_field_bufs.clear();
                        self.list_edit_index = None;
                        self.input_buffer.clear();
                    }
                    KeyCode::Char('o') => {
                        self.view_state = ViewState::Settings;
                        self.list_cursor = 0;
                        self.list_add_stage = 0;
                        self.input_buffer.clear();
                    }
                    KeyCode::Char('q') => {
                        // Restore original data before quitting so what-if entries aren't saved
                        if self.is_what_if() {
                            self.exit_what_if();
                        }
                        return true;
                    }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        if self.is_what_if() {
                            self.exit_what_if();
                        }
                        return true;
                    }
                    _ => {}
                }
                false
            }
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        match self.view_state {
            ViewState::Vacations => {
                let area = f.area();
                self.render_vacation_view(f, area);
            }
            ViewState::Holidays => {
                let area = f.area();
                self.render_holiday_view(f, area);
            }
            ViewState::Settings => {
                let area = f.area();
                self.render_settings_view(f, area);
            }
            ViewState::Calendar => {
                let size = f.area();

                // Horizontal split: left (calendar + events/help), right (stats panels)
                let h_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Min(30),    // left: calendar + events
                        Constraint::Length(68), // right: stats panels (40+14+8 cols + borders)
                    ])
                    .split(size);

                // Left panel: calendar on top, events+help below
                let months = self.period_months();
                let cols = self.time_period_data.calendar_display_columns() as usize;
                let month_rows = if cols > 0 {
                    months.len().div_ceil(cols)
                } else {
                    1
                };
                let cal_height = (month_rows as u16 * 10) + 1;

                let left_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(cal_height), Constraint::Min(10)])
                    .split(h_chunks[0]);

                self.render_calendar(f, left_chunks[0]);
                self.render_events_and_help(f, left_chunks[1]);

                // Right panel: period stats on top, year stats below
                let right_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(18), Constraint::Min(12)])
                    .split(h_chunks[1]);

                self.render_stats(f, right_chunks[0]);
                self.render_year_stats(f, right_chunks[1]);
            }
        }
    }

    fn period_months(&self) -> Vec<NaiveDate> {
        if let Some(period) = self.current_period()
            && let (Some(start), Some(end)) = (period.start_date, period.end_date)
        {
            let start_month = NaiveDate::from_ymd_opt(start.year(), start.month(), 1).unwrap();
            let end_month = NaiveDate::from_ymd_opt(end.year(), end.month(), 1).unwrap();
            let mut months = Vec::new();
            let mut mo = start_month;
            while mo <= end_month {
                months.push(mo);
                mo = add_months(mo, 1);
            }
            return months;
        }
        vec![
            self.nav_date,
            add_months(self.nav_date, 1),
            add_months(self.nav_date, 2),
        ]
    }

    fn render_single_month(
        &self,
        month_date: NaiveDate,
        stats: &Option<QuarterStats>,
        event_map: &std::collections::HashMap<String, Vec<&Event>>,
        holiday_map: &std::collections::HashMap<String, &Holiday>,
        vacation_map: &std::collections::HashMap<String, Vacation>,
        today: NaiveDate,
    ) -> Vec<Line<'static>> {
        let year = month_date.year();
        let month = month_date.month();
        let title = format!("{} {}", month_name(month), year);
        let header_str = " Su Mo Tu We Th Fr Sa   ";

        let mut lines: Vec<Line<'static>> = vec![
            Line::from(Span::styled(
                format!("{:^24}", title),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                header_str.to_string(),
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let first_of_month = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
        let dim = days_in_month(year, month);
        let start_dow = first_of_month.weekday().num_days_from_sunday() as usize;

        let mut day_cells: Vec<Span<'static>> = Vec::new();
        for _ in 0..start_dow {
            day_cells.push(Span::raw("  ".to_string()));
        }

        for d in 1..=dim {
            let date = NaiveDate::from_ymd_opt(year, month, d).unwrap();
            let date_key = date.format("%Y-%m-%d").to_string();

            let is_selected = date == self.selected_date;
            let is_today = date == today;
            let is_weekend =
                date.weekday() == chrono::Weekday::Sat || date.weekday() == chrono::Weekday::Sun;

            let (is_badged, is_flex) = if let Some(s) = stats {
                let w = s.workday_stats.get(&date_key);
                (
                    w.map(|wd| wd.is_badged_in).unwrap_or(false),
                    w.map(|wd| wd.is_flex_credit).unwrap_or(false),
                )
            } else {
                (false, false)
            };

            let is_holiday_or_vacation = if let Some(s) = stats {
                s.workday_stats
                    .get(&date_key)
                    .map(|w| w.is_holiday || w.is_vacation)
                    .unwrap_or(false)
            } else {
                holiday_map.contains_key(&date_key) || vacation_map.contains_key(&date_key)
            };

            let has_event = event_map.contains_key(&date_key);

            let style = calendar_day_style(
                is_selected,
                is_badged,
                is_flex,
                is_holiday_or_vacation,
                is_today,
                is_weekend,
                has_event,
            );
            day_cells.push(Span::styled(format!("{:2}", d), style));
        }

        let mut idx = 0;
        while idx < day_cells.len() {
            let end = (idx + 7).min(day_cells.len());
            let mut row_spans: Vec<Span<'static>> = Vec::new();
            row_spans.push(Span::raw(" ".to_string()));
            for (i, cell) in day_cells[idx..end].iter().enumerate() {
                row_spans.push(cell.clone());
                if i < end - idx - 1 {
                    row_spans.push(Span::raw(" ".to_string()));
                }
            }
            let cells_in_row = end - idx;
            for _ in cells_in_row..7 {
                row_spans.push(Span::raw("   ".to_string()));
            }
            row_spans.push(Span::raw("   ".to_string()));
            lines.push(Line::from(row_spans));
            idx += 7;
        }

        lines
    }

    fn render_calendar(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let stats = &self.active_stats;
        let today = self.today;
        let event_map = self.event_data.get_event_map();
        let holiday_map = self.holiday_data.get_holiday_map();
        let vacation_map = self.vacation_data.get_vacation_map();

        let months = self.period_months();
        let cols = self.time_period_data.calendar_display_columns() as usize;

        let mut all_lines: Vec<Line> = Vec::new();

        if self.is_what_if() {
            all_lines.push(Line::from(Span::styled(
                " ⚠ WHAT-IF MODE  (press w to exit, q to discard & quit) ",
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Indexed(52))
                    .add_modifier(Modifier::BOLD),
            )));
        }

        if let Some(period) = self.current_period()
            && let (Some(start), Some(end)) = (period.start_date, period.end_date)
        {
            all_lines.push(Line::from(Span::styled(
                format!(
                    " {}  [{} – {}]",
                    period.key,
                    start.format("%b %-d, %Y"),
                    end.format("%b %-d, %Y"),
                ),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            all_lines.push(Line::from(""));
        }

        let cols = if cols == 0 { 3 } else { cols };
        for chunk_start in (0..months.len()).step_by(cols) {
            let chunk_end = (chunk_start + cols).min(months.len());
            let row_months = &months[chunk_start..chunk_end];

            let month_renders: Vec<Vec<Line>> = row_months
                .iter()
                .map(|&month_date| {
                    self.render_single_month(
                        month_date,
                        stats,
                        &event_map,
                        &holiday_map,
                        &vacation_map,
                        today,
                    )
                })
                .collect();

            let max_lines = month_renders.iter().map(|r| r.len()).max().unwrap_or(0);

            for line_idx in 0..max_lines {
                let mut spans: Vec<Span> = Vec::new();
                for (m_idx, month_lines) in month_renders.iter().enumerate() {
                    if m_idx > 0 {
                        spans.push(Span::raw("  "));
                    }
                    if line_idx < month_lines.len() {
                        spans.extend(month_lines[line_idx].spans.clone());
                    } else {
                        spans.push(Span::raw("                        "));
                    }
                }
                all_lines.push(Line::from(spans));
            }
            all_lines.push(Line::from(""));
        }

        let widget = Paragraph::new(all_lines).block(Block::default().borders(Borders::NONE));
        f.render_widget(widget, area);
    }

    fn render_stats(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let stats = match &self.active_stats {
            Some(s) => s.clone(),
            None => return,
        };

        let status_style = match stats.compliance_status.as_str() {
            "Achieved" => Style::default()
                .fg(Color::Indexed(46))
                .add_modifier(Modifier::BOLD),
            "On Track" => Style::default().fg(Color::Indexed(40)),
            "At Risk" => Style::default().fg(Color::Indexed(208)),
            "Impossible" => Style::default()
                .fg(Color::Indexed(196))
                .add_modifier(Modifier::BOLD),
            _ => Style::default(),
        };

        let pace_str = format!("{:+} days", stats.days_ahead_of_pace);
        let pace_str = if stats.days_ahead_of_pace > 0 {
            format!("{} ahead", pace_str)
        } else {
            pace_str
        };

        let office_days = stats.days_badged_in - stats.flex_days;
        let goal_pct = if stats.total_days > 0 {
            format!(
                "{:.1}%",
                stats.days_required as f64 / stats.total_days as f64 * 100.0
            )
        } else {
            String::new()
        };
        let office_pct = if stats.days_required > 0 {
            format!(
                "{:.1}%",
                stats.days_badged_in as f64 / stats.days_required as f64 * 100.0
            )
        } else {
            String::new()
        };
        let (badge_pct, flex_pct) = if stats.days_badged_in > 0 {
            (
                format!(
                    "{:.1}%",
                    office_days as f64 / stats.days_badged_in as f64 * 100.0
                ),
                format!(
                    "{:.1}%",
                    stats.flex_days as f64 / stats.days_badged_in as f64 * 100.0
                ),
            )
        } else {
            (String::new(), String::new())
        };
        let needed_pct = if stats.days_required > 0 {
            format!(
                "{:.1}%",
                stats.days_still_needed as f64 / stats.days_required as f64 * 100.0
            )
        } else {
            String::new()
        };

        let skippable_label = format!(
            "Skippable Days ({} left - {} needed)",
            stats.days_left, stats.days_still_needed
        );

        let rows: Vec<Row> = vec![
            section_header("STATUS"),
            data_row(
                "Status",
                Cell::from(stats.compliance_status.clone()).style(status_style),
                plain(""),
            ),
            data_row("Days Ahead of Pace", plain(pace_str), plain("")),
            data_row(
                &skippable_label,
                plain(format!("{}", stats.remaining_missable_days)),
                plain(""),
            ),
            spacer(),
            section_header("PROGRESS"),
            data_row(
                "Total Days",
                plain(format!("{}", stats.total_calendar_days)),
                plain(""),
            ),
            data_row(
                "Total Working Days",
                plain(format!("{}", stats.available_workdays - stats.holidays)),
                plain(""),
            ),
            data_row(
                "Available Working Days",
                plain(format!("{}", stats.total_days)),
                plain(""),
            ),
            data_row(
                format!("Goal ({}% Required)", self.settings.goal),
                plain(format!("{} / {}", stats.days_required, stats.total_days)),
                plain(goal_pct),
            ),
            data_row(
                "Office Days",
                plain(format!(
                    "{} / {}",
                    stats.days_badged_in, stats.days_required
                )),
                plain(office_pct),
            ),
            data_row(
                " Badge-In Days",
                plain(format!("{}", office_days)),
                plain(badge_pct),
            ),
            data_row(
                " Flex Credits",
                plain(format!("{}", stats.flex_days)),
                plain(flex_pct),
            ),
            data_row(
                "Still Needed",
                plain(format!(
                    "{} / {}",
                    stats.days_still_needed, stats.days_required
                )),
                plain(needed_pct),
            ),
        ];

        let quarter_key = self
            .current_period()
            .map(|q| q.key.as_str())
            .unwrap_or("N/A");
        let bold_white = Style::default()
            .fg(Color::Indexed(231))
            .add_modifier(Modifier::BOLD);
        let (title_text, title_style) = if self.is_what_if() {
            (
                format!(" Period Stats: {} [What-If Mode] ", quarter_key),
                Style::default().fg(FLEX_COLOR).add_modifier(Modifier::BOLD),
            )
        } else {
            (format!(" Period Stats: {} ", quarter_key), bold_white)
        };

        let table = Table::new(
            rows,
            [
                Constraint::Length(40),
                Constraint::Length(14),
                Constraint::Length(8),
            ],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(bold_white)
                .title(title_text)
                .title_style(title_style),
        );

        f.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_year_stats(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let stats = match &self.year_stats {
            Some(s) => s.clone(),
            None => return,
        };
        let year = match self.current_period() {
            Some(q) => q
                .start_date
                .map(|d| d.year().to_string())
                .unwrap_or_default(),
            None => return,
        };

        let office_days = stats.days_badged_in - stats.flex_days;
        let (badge_pct, flex_pct) = if stats.days_badged_in > 0 {
            (
                format!(
                    "{:.1}%",
                    office_days as f64 / stats.days_badged_in as f64 * 100.0
                ),
                format!(
                    "{:.1}%",
                    stats.flex_days as f64 / stats.days_badged_in as f64 * 100.0
                ),
            )
        } else {
            (String::new(), String::new())
        };

        let rows = vec![
            data_row(
                "Total Calendar Days",
                plain(format!("{}", stats.total_calendar_days)),
                plain(""),
            ),
            data_row(
                "Total Working Days",
                plain(format!("{}", stats.available_workdays - stats.holidays)),
                plain(""),
            ),
            data_row(
                "Available Working Days",
                plain(format!("{}", stats.total_days)),
                plain(""),
            ),
            data_row("Holidays", plain(format!("{}", stats.holidays)), plain("")),
            data_row(
                "Vacation Days",
                plain(format!("{}", stats.vacation_days)),
                plain(""),
            ),
            data_row(
                "Office Days",
                plain(format!("{}", stats.days_badged_in)),
                plain(""),
            ),
            data_row(
                " Badge-In Days",
                plain(format!("{}", office_days)),
                plain(badge_pct),
            ),
            data_row(
                " Flex Credits",
                plain(format!("{}", stats.flex_days)),
                plain(flex_pct),
            ),
        ];

        let bold_white = Style::default()
            .fg(Color::Indexed(231))
            .add_modifier(Modifier::BOLD);
        let table = Table::new(
            rows,
            [
                Constraint::Length(40),
                Constraint::Length(14),
                Constraint::Length(8),
            ],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(bold_white)
                .title(format!(" Year Stats: {} ", year))
                .title_style(bold_white),
        );

        f.render_widget(table, area);
    }

    fn render_events_and_help(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let event_map = self.event_data.get_event_map();
        let date_key = self.selected_date.format("%Y-%m-%d").to_string();
        let events: Vec<_> = event_map
            .get(&date_key)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
            .to_vec();

        let mut lines: Vec<Line> = Vec::new();

        if let Some((msg, color)) = &self.git_status {
            lines.push(Line::from(vec![
                Span::styled(
                    "[ git ] ",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    msg.clone(),
                    Style::default().fg(*color).add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(""));
        }

        let event_style = Style::default().fg(Color::Yellow);
        lines.push(Line::from(Span::styled(
            format!(
                " Events for {}:",
                self.selected_date.format("%a %b %-d, %Y")
            ),
            event_style.add_modifier(Modifier::BOLD),
        )));

        match self.mode {
            Mode::Add => {
                lines.push(Line::from(Span::styled(
                    format!(" Add event: {}_", self.input_buffer),
                    event_style,
                )));
            }
            Mode::Delete => {
                lines.push(Line::from("  Select event to delete:"));
                if events.is_empty() {
                    lines.push(Line::from("  (no events)"));
                } else {
                    for (i, e) in events.iter().enumerate() {
                        let prefix = if i == self.cursor_index {
                            "  > "
                        } else {
                            "    "
                        };
                        lines.push(Line::from(format!("{}{}", prefix, e.description)));
                    }
                    lines.push(Line::from("  Enter=delete  Esc=cancel  ↑↓=move"));
                }
            }
            Mode::Search => {
                lines.push(Line::from(Span::styled(
                    format!(" Search: {}_", self.input_buffer),
                    event_style,
                )));
                for event in search_events(&self.event_data.events, &self.input_buffer) {
                    lines.push(Line::from(format!(
                        "  {} — {}",
                        event.date, event.description
                    )));
                }
            }
            _ => {
                if events.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "  (none)",
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    for e in &events {
                        lines.push(Line::from(format!("  • {}", e.description)));
                    }
                }
            }
        }

        lines.push(Line::from(""));

        let key_style = Style::default().fg(Color::Indexed(51));
        let help_style = Style::default().fg(Color::DarkGray);

        let mut view_label = self
            .settings
            .active_time_period_file(self.active_time_period_idx)
            .to_string();
        if self.settings.time_periods.len() > 1 {
            view_label += &format!(
                "  ({} of {})",
                self.active_time_period_idx + 1,
                self.settings.time_periods.len()
            );
        }
        lines.push(Line::from(vec![
            Span::styled("[space/shift+←→]", key_style),
            Span::raw(" "),
            Span::styled(view_label, help_style),
        ]));

        let bindings: Vec<(&str, String)> = vec![
            ("←→↑↓", "Navigate".to_string()),
            ("b", self.settings.default_office.clone()),
            ("f", self.settings.flex_credit.clone()),
            ("n/p", "Next/Prev period".to_string()),
            ("a", "Add event".to_string()),
            ("d", "Delete event".to_string()),
            ("s", "Search".to_string()),
            ("w", "What-if".to_string()),
            ("g", "Git backup".to_string()),
            ("v", "Vacations".to_string()),
            ("h", "Holidays".to_string()),
            ("o", "Settings".to_string()),
            ("q", "Quit".to_string()),
        ];

        const KEY_COL_WIDTH: usize = 24;
        for chunk in bindings.chunks(3) {
            let mut spans: Vec<Span> = Vec::new();
            for (key, desc) in chunk.iter() {
                let bracket_key = format!("[{}]", key);
                let cell_text = format!("{} {}", bracket_key, desc);
                let visible_len = cell_text.chars().count();
                let pad = if KEY_COL_WIDTH > visible_len {
                    KEY_COL_WIDTH - visible_len
                } else {
                    1
                };
                spans.push(Span::styled(bracket_key, key_style));
                spans.push(Span::raw(" "));
                spans.push(Span::styled(desc.clone(), help_style));
                spans.push(Span::raw(" ".repeat(pad)));
            }
            lines.push(Line::from(spans));
        }

        lines.push(Line::from(Span::styled(
            format!("Data: {}", self.data_dir.to_string_lossy()),
            help_style,
        )));

        let git_info = crate::cmd::backup::status(&self.data_dir);
        if git_info.is_repo {
            let mut parts: Vec<Span> = vec![Span::styled("  Git: ", help_style)];
            let mut status_parts: Vec<String> = Vec::new();
            if git_info.modified > 0 {
                status_parts.push(format!("{} modified", git_info.modified));
            }
            if git_info.untracked > 0 {
                status_parts.push(format!("{} untracked", git_info.untracked));
            }
            if status_parts.is_empty() {
                parts.push(Span::styled(
                    "clean",
                    Style::default().fg(Color::Indexed(34)),
                ));
            } else {
                let status_str = status_parts.join(", ");
                parts.push(Span::styled(
                    status_str,
                    Style::default().fg(Color::Indexed(196)),
                ));
            }
            if git_info.has_remote {
                parts.push(Span::styled("  (remote: origin)", help_style));
            }
            if git_info.modified > 0 || git_info.untracked > 0 {
                parts.push(Span::styled("  [press g to backup]", help_style));
            }
            lines.push(Line::from(parts));

            if !git_info.last_commit.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("  Last: {}", git_info.last_commit),
                    help_style,
                )));
            }
        }

        let p = Paragraph::new(lines).block(Block::default().borders(Borders::NONE));
        f.render_widget(p, area);
    }

    // ── Vacation View ─────────────────────────────────────────────────────────

    fn render_vacation_view(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),    // vacation list table
                Constraint::Length(8), // add form or key hints
            ])
            .split(area);

        // Build table rows
        let header = Row::new(vec![
            Cell::from("#").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Destination").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Start").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("End").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Approved").style(Style::default().add_modifier(Modifier::BOLD)),
        ]);

        let rows: Vec<Row> = self
            .vacation_data
            .vacations
            .iter()
            .enumerate()
            .map(|(i, v)| {
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)),
                    Cell::from(v.destination.clone()),
                    Cell::from(v.start_date.clone()),
                    Cell::from(v.end_date.clone()),
                    Cell::from(if v.approved { "Yes" } else { "No" }),
                ])
            })
            .collect();

        let mut table_state = TableState::default();
        if !self.vacation_data.vacations.is_empty() {
            table_state.select(Some(self.list_cursor));
        }

        let table = Table::new(
            rows,
            [
                Constraint::Length(3),
                Constraint::Length(30),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Vacations  (a=add  Enter/e=edit  Del/x=delete  Esc=back) "),
        )
        .row_highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

        f.render_stateful_widget(table, chunks[0], &mut table_state);

        // Bottom panel: add/edit form or key hints
        let bottom = chunks[1];
        if self.list_add_stage > 0 {
            let labels = [
                "Destination",
                "Start date (YYYY-MM-DD)",
                "End date (YYYY-MM-DD)",
                "Approved? (y/n)",
            ];
            let form_title = if self.list_edit_index.is_some() {
                "── Edit Vacation ─────────────────────────────────"
            } else {
                "── Add Vacation ─────────────────────────────────"
            };
            let mut form_lines: Vec<Line> = vec![Line::from(Span::styled(
                form_title,
                Style::default().add_modifier(Modifier::BOLD),
            ))];
            for (i, label) in labels.iter().enumerate() {
                let field_num = (i + 1) as u8;
                let value = if field_num < self.list_add_stage {
                    self.list_field_bufs.get(i).cloned().unwrap_or_default()
                } else if field_num == self.list_add_stage {
                    format!("{}_", self.input_buffer)
                } else {
                    String::new()
                };
                form_lines.push(Line::from(format!("{}: {}", label, value)));
            }
            form_lines.push(Line::from(""));
            form_lines.push(Line::from(Span::styled(
                "Enter=confirm  Esc=cancel",
                Style::default().fg(Color::DarkGray),
            )));
            let p = Paragraph::new(form_lines).block(Block::default().borders(Borders::NONE));
            f.render_widget(p, bottom);
        } else {
            let hints = Paragraph::new(vec![Line::from(Span::styled(
                "↑↓=move  a=add  Enter/e=edit  Del/x=delete  Esc=back",
                Style::default().fg(Color::DarkGray),
            ))])
            .block(Block::default().borders(Borders::NONE));
            f.render_widget(hints, bottom);
        }
    }

    fn handle_vacation_key(&mut self, code: KeyCode) {
        use crate::data::vacation::Vacation;
        if self.list_add_stage == 0 {
            // ── Browse mode ───────────────────────────────────────────────────
            match code {
                KeyCode::Up => {
                    if self.list_cursor > 0 {
                        self.list_cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.list_cursor + 1 < self.vacation_data.vacations.len() {
                        self.list_cursor += 1;
                    }
                }
                KeyCode::Char('a') => {
                    self.list_edit_index = None;
                    self.list_field_bufs.clear();
                    self.input_buffer.clear();
                    self.list_add_stage = 1;
                }
                KeyCode::Char('e') | KeyCode::Enter => {
                    // Edit the selected vacation
                    if !self.vacation_data.vacations.is_empty()
                        && self.list_cursor < self.vacation_data.vacations.len()
                    {
                        let v = &self.vacation_data.vacations[self.list_cursor];
                        self.input_buffer = v.destination.clone();
                        self.list_field_bufs.clear();
                        self.list_edit_index = Some(self.list_cursor);
                        self.list_add_stage = 1;
                    }
                }
                KeyCode::Delete | KeyCode::Char('x') => {
                    // Only Delete and x delete; Enter now edits
                    if !self.vacation_data.vacations.is_empty()
                        && self.list_cursor < self.vacation_data.vacations.len()
                    {
                        self.vacation_data.vacations.remove(self.list_cursor);
                        if self.list_cursor > 0
                            && self.list_cursor >= self.vacation_data.vacations.len()
                        {
                            self.list_cursor -= 1;
                        }
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view_state = ViewState::Calendar;
                }
                _ => {}
            }
        } else {
            // ── Add/Edit mode: field entry ─────────────────────────────────────
            match code {
                KeyCode::Char(c) => {
                    self.input_buffer.push(c);
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Enter => {
                    // Validate date fields (stages 2 and 3)
                    if (self.list_add_stage == 2 || self.list_add_stage == 3)
                        && NaiveDate::parse_from_str(&self.input_buffer, "%Y-%m-%d").is_err()
                    {
                        self.input_buffer = "Invalid date — use YYYY-MM-DD".to_string();
                        return;
                    }
                    self.list_field_bufs.push(self.input_buffer.clone());

                    if self.list_add_stage == 4 {
                        // All fields gathered — build vacation
                        let approved = self
                            .list_field_bufs
                            .get(3)
                            .map(|s| s.to_lowercase().starts_with('y'))
                            .unwrap_or(false);
                        let v = Vacation::new(
                            self.list_field_bufs
                                .first()
                                .map(String::as_str)
                                .unwrap_or(""),
                            self.list_field_bufs
                                .get(1)
                                .map(String::as_str)
                                .unwrap_or(""),
                            self.list_field_bufs
                                .get(2)
                                .map(String::as_str)
                                .unwrap_or(""),
                            approved,
                        );
                        if let Some(idx) = self.list_edit_index {
                            // Edit: replace in-place
                            if idx < self.vacation_data.vacations.len() {
                                self.vacation_data.vacations[idx] = v;
                            }
                        } else {
                            self.vacation_data.add(v);
                        }
                        self.list_add_stage = 0;
                        self.list_edit_index = None;
                        self.list_field_bufs.clear();
                        self.input_buffer.clear();
                    } else {
                        // Advance to next field, pre-fill from existing data when editing
                        let next_stage = self.list_add_stage + 1;
                        if let Some(idx) = self.list_edit_index {
                            if let Some(v) = self.vacation_data.vacations.get(idx) {
                                self.input_buffer = match next_stage {
                                    2 => v.start_date.clone(),
                                    3 => v.end_date.clone(),
                                    4 => {
                                        if v.approved {
                                            "y".to_string()
                                        } else {
                                            "n".to_string()
                                        }
                                    }
                                    _ => String::new(),
                                };
                            } else {
                                self.input_buffer.clear();
                            }
                        } else {
                            self.input_buffer.clear();
                        }
                        self.list_add_stage = next_stage;
                    }
                }
                KeyCode::Esc => {
                    self.list_add_stage = 0;
                    self.list_edit_index = None;
                    self.input_buffer.clear();
                    self.list_field_bufs.clear();
                }
                _ => {}
            }
        }
    }

    // ── Holiday View ──────────────────────────────────────────────────────────

    fn render_holiday_view(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),    // holiday list table
                Constraint::Length(6), // add form or key hints
            ])
            .split(area);

        let header = Row::new(vec![
            Cell::from("#").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Date").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        ]);

        let rows: Vec<Row> = self
            .holiday_data
            .holidays
            .iter()
            .enumerate()
            .map(|(i, h)| {
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)),
                    Cell::from(h.date.clone()),
                    Cell::from(h.name.clone()),
                ])
            })
            .collect();

        let mut table_state = TableState::default();
        if !self.holiday_data.holidays.is_empty() {
            table_state.select(Some(self.list_cursor));
        }

        let table = Table::new(
            rows,
            [
                Constraint::Length(3),
                Constraint::Length(14),
                Constraint::Length(50),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Holidays  (a=add  Enter/e=edit  Del/x=delete  Esc=back) "),
        )
        .row_highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

        f.render_stateful_widget(table, chunks[0], &mut table_state);

        let bottom = chunks[1];
        if self.list_add_stage > 0 {
            let labels = ["Date (YYYY-MM-DD)", "Name"];
            let form_title = if self.list_edit_index.is_some() {
                "── Edit Holiday ─────────────────────────────────"
            } else {
                "── Add Holiday ──────────────────────────────────"
            };
            let mut form_lines: Vec<Line> = vec![Line::from(Span::styled(
                form_title,
                Style::default().add_modifier(Modifier::BOLD),
            ))];
            for (i, label) in labels.iter().enumerate() {
                let field_num = (i + 1) as u8;
                let value = if field_num < self.list_add_stage {
                    self.list_field_bufs.get(i).cloned().unwrap_or_default()
                } else if field_num == self.list_add_stage {
                    format!("{}_", self.input_buffer)
                } else {
                    String::new()
                };
                form_lines.push(Line::from(format!("{}: {}", label, value)));
            }
            form_lines.push(Line::from(""));
            form_lines.push(Line::from(Span::styled(
                "Enter=confirm  Esc=cancel",
                Style::default().fg(Color::DarkGray),
            )));
            let p = Paragraph::new(form_lines).block(Block::default().borders(Borders::NONE));
            f.render_widget(p, bottom);
        } else {
            let hints = Paragraph::new(vec![Line::from(Span::styled(
                "↑↓=move  a=add  Enter/e=edit  Del/x=delete  Esc=back",
                Style::default().fg(Color::DarkGray),
            ))])
            .block(Block::default().borders(Borders::NONE));
            f.render_widget(hints, bottom);
        }
    }

    fn handle_holiday_key(&mut self, code: KeyCode) {
        use crate::data::holiday::Holiday;
        if self.list_add_stage == 0 {
            // ── Browse mode ───────────────────────────────────────────────────
            match code {
                KeyCode::Up => {
                    if self.list_cursor > 0 {
                        self.list_cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.list_cursor + 1 < self.holiday_data.holidays.len() {
                        self.list_cursor += 1;
                    }
                }
                KeyCode::Char('a') => {
                    self.list_edit_index = None;
                    self.list_field_bufs.clear();
                    self.input_buffer.clear();
                    self.list_add_stage = 1;
                }
                KeyCode::Char('e') | KeyCode::Enter => {
                    // Edit the selected holiday
                    if !self.holiday_data.holidays.is_empty()
                        && self.list_cursor < self.holiday_data.holidays.len()
                    {
                        let h = &self.holiday_data.holidays[self.list_cursor];
                        self.input_buffer = h.date.clone();
                        self.list_field_bufs.clear();
                        self.list_edit_index = Some(self.list_cursor);
                        self.list_add_stage = 1;
                    }
                }
                KeyCode::Delete | KeyCode::Char('x') => {
                    if !self.holiday_data.holidays.is_empty()
                        && self.list_cursor < self.holiday_data.holidays.len()
                    {
                        self.holiday_data.holidays.remove(self.list_cursor);
                        if self.list_cursor > 0
                            && self.list_cursor >= self.holiday_data.holidays.len()
                        {
                            self.list_cursor -= 1;
                        }
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view_state = ViewState::Calendar;
                }
                _ => {}
            }
        } else {
            // ── Add/Edit mode: field entry ─────────────────────────────────────
            match code {
                KeyCode::Char(c) => {
                    self.input_buffer.push(c);
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Enter => {
                    // Validate date for stage 1 (date field)
                    if self.list_add_stage == 1
                        && NaiveDate::parse_from_str(&self.input_buffer, "%Y-%m-%d").is_err()
                    {
                        self.input_buffer = "Invalid date — use YYYY-MM-DD".to_string();
                        return;
                    }
                    self.list_field_bufs.push(self.input_buffer.clone());

                    if self.list_add_stage == 2 {
                        // field_bufs: [0]=date, [1]=name
                        let h = Holiday::new(
                            self.list_field_bufs
                                .get(1)
                                .map(String::as_str)
                                .unwrap_or(""),
                            self.list_field_bufs
                                .first()
                                .map(String::as_str)
                                .unwrap_or(""),
                        );
                        if let Some(idx) = self.list_edit_index {
                            if idx < self.holiday_data.holidays.len() {
                                self.holiday_data.holidays[idx] = h;
                            }
                        } else {
                            self.holiday_data.add(h);
                        }
                        self.list_add_stage = 0;
                        self.list_edit_index = None;
                        self.list_field_bufs.clear();
                        self.input_buffer.clear();
                    } else {
                        // Advance to name field; pre-fill from existing data when editing
                        if let Some(idx) = self.list_edit_index {
                            if let Some(h) = self.holiday_data.holidays.get(idx) {
                                self.input_buffer = h.name.clone();
                            } else {
                                self.input_buffer.clear();
                            }
                        } else {
                            self.input_buffer.clear();
                        }
                        self.list_add_stage += 1;
                    }
                }
                KeyCode::Esc => {
                    self.list_add_stage = 0;
                    self.list_edit_index = None;
                    self.input_buffer.clear();
                    self.list_field_bufs.clear();
                }
                _ => {}
            }
        }
    }

    // ── Settings View ─────────────────────────────────────────────────────────

    fn render_settings_view(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),    // settings table
                Constraint::Length(3), // hints
            ])
            .split(area);

        let header = Row::new(vec![
            Cell::from("Setting").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("Value").style(Style::default().add_modifier(Modifier::BOLD)),
        ]);

        let fields = [
            ("Default Office", self.settings.default_office.as_str()),
            ("Flex Credit Label", self.settings.flex_credit.as_str()),
        ];

        let rows: Vec<Row> = fields
            .iter()
            .enumerate()
            .map(|(i, (label, current_val))| {
                let value = if self.list_add_stage == 1 && self.list_cursor == i {
                    format!("{}_", self.input_buffer)
                } else {
                    current_val.to_string()
                };
                Row::new(vec![Cell::from(format!("  {}", label)), Cell::from(value)])
            })
            .collect();

        let mut table_state = TableState::default();
        table_state.select(Some(self.list_cursor));

        let table = Table::new(rows, [Constraint::Length(22), Constraint::Min(30)])
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Settings  (↑↓=select  Enter/e=edit  Esc=back) "),
            )
            .row_highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_stateful_widget(table, chunks[0], &mut table_state);

        let hint_text = if self.list_add_stage == 1 {
            "Type new value  Enter=save  Esc=cancel"
        } else {
            "↑↓=select  Enter/e=edit  Esc=back to calendar"
        };
        let hints = Paragraph::new(Line::from(Span::styled(
            hint_text,
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(hints, chunks[1]);
    }

    fn handle_settings_key(&mut self, code: KeyCode) {
        if self.list_add_stage == 0 {
            // ── Browse mode ───────────────────────────────────────────────────
            match code {
                KeyCode::Up => {
                    if self.list_cursor > 0 {
                        self.list_cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.list_cursor < 1 {
                        self.list_cursor += 1;
                    }
                }
                KeyCode::Enter | KeyCode::Char('e') => {
                    // Pre-fill input_buffer with current value
                    self.input_buffer = match self.list_cursor {
                        0 => self.settings.default_office.clone(),
                        _ => self.settings.flex_credit.clone(),
                    };
                    self.list_add_stage = 1;
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view_state = ViewState::Calendar;
                }
                _ => {}
            }
        } else {
            // ── Edit mode ─────────────────────────────────────────────────────
            match code {
                KeyCode::Char(c) => {
                    self.input_buffer.push(c);
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Enter => {
                    let value = self.input_buffer.clone();
                    match self.list_cursor {
                        0 => self.settings.default_office = value,
                        _ => self.settings.flex_credit = value,
                    }
                    self.input_buffer.clear();
                    self.list_add_stage = 0;
                }
                KeyCode::Esc => {
                    self.input_buffer.clear();
                    self.list_add_stage = 0;
                }
                _ => {}
            }
        }
    }
}

// ── Row construction helpers ──────────────────────────────────────────────────

/// A section header row with a dark background and bold text.
fn section_header(title: &str) -> Row<'static> {
    Row::new(vec![
        Cell::from(title.to_string()).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(""),
        Cell::from(""),
    ])
    .style(Style::default().bg(SECTION_BG))
}

/// An empty spacer row for visual breathing room between sections.
fn spacer() -> Row<'static> {
    Row::new(vec![Cell::from(""), Cell::from(""), Cell::from("")])
}

/// A data row with a two-space indent on the metric label.
fn data_row(metric: impl Into<String>, value: Cell<'static>, pct: Cell<'static>) -> Row<'static> {
    Row::new(vec![Cell::from(format!("  {}", metric.into())), value, pct])
}

/// Plain (unstyled) cell.
fn plain(s: impl Into<String>) -> Cell<'static> {
    Cell::from(s.into())
}

// ── App event loop ────────────────────────────────────────────────────────────

pub fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| app.render(f))?;
        if event::poll(StdDuration::from_millis(16))?
            && let CEvent::Key(key) = event::read()?
            && app.handle_key(key.code, key.modifiers)
        {
            break;
        }
    }
    Ok(())
}

// ── Calendar helpers ──────────────────────────────────────────────────────────

pub(crate) fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

pub(crate) fn days_in_month(year: i32, month: u32) -> u32 {
    let next_month = if month == 12 { 1 } else { month + 1 };
    let next_year = if month == 12 { year + 1 } else { year };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .unwrap()
        .signed_duration_since(NaiveDate::from_ymd_opt(year, month, 1).unwrap())
        .num_days() as u32
}

pub(crate) fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let year = date.year();
    let month = date.month() as i32;
    let new_total = month - 1 + months;
    let new_month = ((new_total % 12 + 12) % 12 + 1) as u32;
    let year_delta = new_total.div_euclid(12);
    let new_year = year + year_delta;
    let max_day = days_in_month(new_year, new_month);
    let new_day = date.day().min(max_day);
    NaiveDate::from_ymd_opt(new_year, new_month, new_day).unwrap_or(date)
}

/// Determines the ratatui `Style` for a calendar day cell based on its state.
pub(crate) fn calendar_day_style(
    is_selected: bool,
    is_badged: bool,
    is_flex: bool,
    is_holiday_or_vacation: bool,
    is_today: bool,
    is_weekend: bool,
    has_event: bool,
) -> Style {
    if is_selected {
        let bg = if is_badged && is_flex {
            FLEX_COLOR
        } else if is_badged {
            Color::Yellow
        } else if is_holiday_or_vacation {
            Color::Green
        } else {
            Color::White
        };
        Style::default()
            .fg(Color::Black)
            .bg(bg)
            .add_modifier(Modifier::BOLD)
    } else if is_badged {
        let color = if is_flex { FLEX_COLOR } else { Color::Yellow };
        let mut s = Style::default()
            .fg(color)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        if is_today {
            s = s.add_modifier(Modifier::REVERSED);
        }
        s
    } else if is_holiday_or_vacation {
        let mut s = Style::default().fg(Color::Green);
        if is_today {
            s = s.add_modifier(Modifier::REVERSED);
        }
        s
    } else if is_today {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else if is_weekend {
        Style::default().add_modifier(Modifier::DIM)
    } else if has_event {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    }
}

/// Filters events by a search query (case-insensitive description, case-sensitive date).
pub(crate) fn search_events<'a>(events: &'a [Event], query: &str) -> Vec<&'a Event> {
    let q = query.to_lowercase();
    events
        .iter()
        .filter(|e| e.description.to_lowercase().contains(&q) || e.date.contains(query))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::time_period::TimePeriod;
    use crate::data::{
        AppSettings, BadgeEntryData, EventData, HolidayData, TimePeriodData, VacationData,
    };
    use chrono::NaiveDate;
    use crossterm::event::{KeyCode, KeyModifiers};
    use std::path::PathBuf;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn make_quarter_data() -> TimePeriodData {
        let mut q1 = TimePeriod {
            key: "Q1_2025".to_string(),
            name: "Q1".to_string(),
            start_date_raw: "2025-01-01".to_string(),
            end_date_raw: "2025-03-31".to_string(),
            start_date: None,
            end_date: None,
        };
        q1.parse_dates().unwrap();
        let mut q2 = TimePeriod {
            key: "Q2_2025".to_string(),
            name: "Q2".to_string(),
            start_date_raw: "2025-04-01".to_string(),
            end_date_raw: "2025-06-30".to_string(),
            start_date: None,
            end_date: None,
        };
        q2.parse_dates().unwrap();
        let mut data = TimePeriodData::new();
        data.add(q1);
        data.add(q2);
        data
    }

    fn make_test_app<'a>(
        time_period_data: TimePeriodData,
        badge_data: &'a mut BadgeEntryData,
        holiday_data: &'a mut HolidayData,
        vacation_data: &'a mut VacationData,
        event_data: &'a mut EventData,
        today: NaiveDate,
    ) -> App<'a> {
        App::new(
            time_period_data,
            badge_data,
            holiday_data,
            vacation_data,
            event_data,
            AppSettings::default(),
            today,
            PathBuf::from("/tmp/test"),
        )
    }

    // ── calendar_day_style tests ──────────────────────────────────────────────

    #[test]
    fn test_style_selected_badged_office() {
        let s = calendar_day_style(true, true, false, false, false, false, false);
        assert_eq!(
            s,
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        );
    }

    #[test]
    fn test_style_selected_badged_flex() {
        let s = calendar_day_style(true, true, true, false, false, false, false);
        assert_eq!(
            s,
            Style::default()
                .fg(Color::Black)
                .bg(FLEX_COLOR)
                .add_modifier(Modifier::BOLD)
        );
    }

    #[test]
    fn test_style_selected_holiday() {
        let s = calendar_day_style(true, false, false, true, false, false, false);
        assert_eq!(
            s,
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        );
    }

    #[test]
    fn test_style_selected_plain() {
        let s = calendar_day_style(true, false, false, false, false, false, false);
        assert_eq!(
            s,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        );
    }

    #[test]
    fn test_style_badged_office_not_selected() {
        let s = calendar_day_style(false, true, false, false, false, false, false);
        assert_eq!(
            s,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        );
    }

    #[test]
    fn test_style_badged_flex_not_selected() {
        let s = calendar_day_style(false, true, true, false, false, false, false);
        assert_eq!(
            s,
            Style::default()
                .fg(FLEX_COLOR)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        );
    }

    #[test]
    fn test_style_badged_today() {
        let s = calendar_day_style(false, true, false, false, true, false, false);
        assert_eq!(
            s,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED | Modifier::REVERSED)
        );
    }

    #[test]
    fn test_style_holiday_not_selected() {
        let s = calendar_day_style(false, false, false, true, false, false, false);
        assert_eq!(s, Style::default().fg(Color::Green));
    }

    #[test]
    fn test_style_today_plain() {
        let s = calendar_day_style(false, false, false, false, true, false, false);
        assert_eq!(
            s,
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        );
    }

    #[test]
    fn test_style_weekend() {
        let s = calendar_day_style(false, false, false, false, false, true, false);
        assert_eq!(s, Style::default().add_modifier(Modifier::DIM));
    }

    #[test]
    fn test_style_has_event() {
        let s = calendar_day_style(false, false, false, false, false, false, true);
        assert_eq!(s, Style::default().fg(Color::Cyan));
    }

    #[test]
    fn test_style_plain_workday() {
        let s = calendar_day_style(false, false, false, false, false, false, false);
        assert_eq!(s, Style::default());
    }

    // ── search_events tests ───────────────────────────────────────────────────

    fn ev(date: &str, desc: &str) -> Event {
        Event {
            date: date.to_string(),
            description: desc.to_string(),
        }
    }

    #[test]
    fn test_search_empty_query_returns_all() {
        let events = vec![ev("2025-01-01", "Alpha"), ev("2025-01-02", "Beta")];
        let result = search_events(&events, "");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_search_case_insensitive_description() {
        let events = vec![ev("2025-01-01", "Team Lunch"), ev("2025-01-02", "Meeting")];
        let result = search_events(&events, "lunch");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].description, "Team Lunch");
    }

    #[test]
    fn test_search_no_match_returns_empty() {
        let events = vec![ev("2025-01-01", "Alpha"), ev("2025-01-02", "Beta")];
        let result = search_events(&events, "zzz");
        assert!(result.is_empty());
    }

    #[test]
    fn test_search_partial_match() {
        let events = vec![
            ev("2025-01-01", "Team Lunch"),
            ev("2025-01-02", "Team Meeting"),
        ];
        let result = search_events(&events, "team");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_search_by_date() {
        let events = vec![ev("2025-03-15", "Review"), ev("2025-04-01", "Launch")];
        let result = search_events(&events, "2025-03");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].date, "2025-03-15");
    }

    // ── add_months tests ──────────────────────────────────────────────────────

    #[test]
    fn test_add_months_forward() {
        assert_eq!(add_months(d(2025, 1, 15), 1), d(2025, 2, 15));
    }

    #[test]
    fn test_add_months_across_year() {
        assert_eq!(add_months(d(2025, 11, 15), 2), d(2026, 1, 15));
    }

    #[test]
    fn test_add_months_backward() {
        assert_eq!(add_months(d(2025, 3, 10), -2), d(2025, 1, 10));
    }

    #[test]
    fn test_add_months_backward_across_year() {
        assert_eq!(add_months(d(2025, 1, 10), -1), d(2024, 12, 10));
    }

    #[test]
    fn test_add_months_clamps_month_end() {
        // Jan 31 + 1 month = Feb 28 (2025 is not a leap year)
        assert_eq!(add_months(d(2025, 1, 31), 1), d(2025, 2, 28));
    }

    // ── days_in_month tests ───────────────────────────────────────────────────

    #[test]
    fn test_days_in_month_january() {
        assert_eq!(days_in_month(2025, 1), 31);
    }

    #[test]
    fn test_days_in_month_february_non_leap() {
        assert_eq!(days_in_month(2025, 2), 28);
    }

    #[test]
    fn test_days_in_month_february_leap() {
        assert_eq!(days_in_month(2024, 2), 29);
    }

    #[test]
    fn test_days_in_month_april() {
        assert_eq!(days_in_month(2025, 4), 30);
    }

    #[test]
    fn test_days_in_month_december() {
        assert_eq!(days_in_month(2025, 12), 31);
    }

    // ── month_name tests ──────────────────────────────────────────────────────

    #[test]
    fn test_month_name_known_values() {
        assert_eq!(month_name(1), "January");
        assert_eq!(month_name(6), "June");
        assert_eq!(month_name(12), "December");
    }

    #[test]
    fn test_month_name_unknown() {
        assert_eq!(month_name(0), "Unknown");
        assert_eq!(month_name(13), "Unknown");
    }

    // ── handle_key tests ──────────────────────────────────────────────────────

    #[test]
    fn test_arrow_keys_move_selected_date() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        app.handle_key(KeyCode::Right, KeyModifiers::empty());
        assert_eq!(app.selected_date, d(2025, 2, 11));

        app.handle_key(KeyCode::Left, KeyModifiers::empty());
        assert_eq!(app.selected_date, d(2025, 2, 10));

        app.handle_key(KeyCode::Down, KeyModifiers::empty());
        assert_eq!(app.selected_date, d(2025, 2, 17));

        app.handle_key(KeyCode::Up, KeyModifiers::empty());
        assert_eq!(app.selected_date, d(2025, 2, 10));
    }

    #[test]
    fn test_b_toggles_office_badge() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        let key = today.format("%Y-%m-%d").to_string();
        assert!(!app.badge_data.has(&key));

        app.handle_key(KeyCode::Char('b'), KeyModifiers::empty());
        assert!(app.badge_data.has(&key));

        app.handle_key(KeyCode::Char('b'), KeyModifiers::empty());
        assert!(!app.badge_data.has(&key));
    }

    #[test]
    fn test_f_toggles_flex_badge() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        let key = today.format("%Y-%m-%d").to_string();
        app.handle_key(KeyCode::Char('f'), KeyModifiers::empty());
        assert!(app.badge_data.has(&key));
        let entry = app.badge_data.data.iter().find(|e| e.key == key).unwrap();
        assert_eq!(entry.office, "Flex Credit");
    }

    #[test]
    fn test_b_does_nothing_outside_quarter() {
        let qd = TimePeriodData::new();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        app.handle_key(KeyCode::Char('b'), KeyModifiers::empty());
        assert_eq!(app.badge_data.data.len(), 0);
    }

    #[test]
    fn test_add_mode_enter_saves_event() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        app.handle_key(KeyCode::Char('a'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('T'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('e'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('s'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('t'), KeyModifiers::empty());
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());

        assert_eq!(app.event_data.events.len(), 1);
        assert_eq!(app.event_data.events[0].description, "Test");
        assert_eq!(app.event_data.events[0].date, "2025-02-10");
    }

    #[test]
    fn test_add_mode_esc_discards() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        app.handle_key(KeyCode::Char('a'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('x'), KeyModifiers::empty());
        app.handle_key(KeyCode::Esc, KeyModifiers::empty());

        assert!(app.event_data.events.is_empty());
    }

    #[test]
    fn test_add_mode_empty_buffer_no_event() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        app.handle_key(KeyCode::Char('a'), KeyModifiers::empty());
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());

        assert!(app.event_data.events.is_empty());
    }

    #[test]
    fn test_add_mode_backspace_in_add_mode() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        app.handle_key(KeyCode::Char('a'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('x'), KeyModifiers::empty());
        app.handle_key(KeyCode::Backspace, KeyModifiers::empty());
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());

        // buffer was "x", backspace removed it, enter on empty buffer → no event
        assert!(app.event_data.events.is_empty());
    }

    #[test]
    fn test_delete_mode_enter_removes_event() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        ed.add(Event {
            date: "2025-02-10".to_string(),
            description: "To remove".to_string(),
        });
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        app.handle_key(KeyCode::Char('d'), KeyModifiers::empty());
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());

        assert!(app.event_data.events.is_empty());
    }

    #[test]
    fn test_delete_mode_cursor_navigation() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        ed.add(Event {
            date: "2025-02-10".to_string(),
            description: "First".to_string(),
        });
        ed.add(Event {
            date: "2025-02-10".to_string(),
            description: "Second".to_string(),
        });
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        app.handle_key(KeyCode::Char('d'), KeyModifiers::empty());
        assert_eq!(app.cursor_index, 0);

        app.handle_key(KeyCode::Down, KeyModifiers::empty());
        assert_eq!(app.cursor_index, 1);

        app.handle_key(KeyCode::Up, KeyModifiers::empty());
        assert_eq!(app.cursor_index, 0);
    }

    #[test]
    fn test_search_mode_char_fills_buffer() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        app.handle_key(KeyCode::Char('s'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('q'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('1'), KeyModifiers::empty());

        assert_eq!(app.input_buffer, "q1");
    }

    #[test]
    fn test_what_if_toggle_w() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert!(!app.is_what_if());

        // Enter what-if, add a badge
        app.handle_key(KeyCode::Char('w'), KeyModifiers::empty());
        assert!(app.is_what_if());
        app.handle_key(KeyCode::Char('b'), KeyModifiers::empty());
        assert_eq!(app.badge_data.data.len(), 1);

        // Exit what-if — badge changes discarded
        app.handle_key(KeyCode::Char('w'), KeyModifiers::empty());
        assert!(!app.is_what_if());
        assert_eq!(app.badge_data.data.len(), 0);
    }

    #[test]
    fn test_n_advances_quarter() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert_eq!(
            app.current_period().map(|q| q.key.as_str()),
            Some("Q1_2025")
        );
        app.handle_key(KeyCode::Char('n'), KeyModifiers::empty());
        assert_eq!(
            app.current_period().map(|q| q.key.as_str()),
            Some("Q2_2025")
        );
    }

    #[test]
    fn test_p_retreats_quarter() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        // Start in Q2
        let today = d(2025, 5, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert_eq!(
            app.current_period().map(|q| q.key.as_str()),
            Some("Q2_2025")
        );
        app.handle_key(KeyCode::Char('p'), KeyModifiers::empty());
        assert_eq!(
            app.current_period().map(|q| q.key.as_str()),
            Some("Q1_2025")
        );
    }

    #[test]
    fn test_q_returns_true() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        let quit = app.handle_key(KeyCode::Char('q'), KeyModifiers::empty());
        assert!(quit);
    }

    #[test]
    fn test_ctrl_c_returns_true() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        let quit = app.handle_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(quit);
    }

    #[test]
    fn test_q_in_what_if_restores_data() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        // Pre-badge a date
        bd.add(BadgeEntry::new(today, "McLean, VA", false));
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        // Enter what-if, remove the badge
        app.handle_key(KeyCode::Char('w'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('b'), KeyModifiers::empty()); // toggle off
        assert_eq!(app.badge_data.data.len(), 0);

        // Quit — should restore original data
        let quit = app.handle_key(KeyCode::Char('q'), KeyModifiers::empty());
        assert!(quit);
        assert_eq!(app.badge_data.data.len(), 1);
    }

    #[test]
    fn test_n_at_last_quarter_stays() {
        // make_quarter_data has Q1 (Jan-Mar) and Q2 (Apr-Jun) 2025 only.
        // At last quarter, n keeps us there (Go behavior: adjacent in list).
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 5, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert_eq!(
            app.current_period().map(|q| q.key.as_str()),
            Some("Q2_2025")
        );
        app.handle_key(KeyCode::Char('n'), KeyModifiers::empty());
        assert_eq!(
            app.current_period().map(|q| q.key.as_str()),
            Some("Q2_2025"),
            "n at last quarter keeps us there"
        );
    }

    #[test]
    fn test_p_at_first_quarter_stays() {
        // At first quarter, p keeps us there (Go behavior: adjacent in list).
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert_eq!(
            app.current_period().map(|q| q.key.as_str()),
            Some("Q1_2025")
        );
        app.handle_key(KeyCode::Char('p'), KeyModifiers::empty());
        assert_eq!(
            app.current_period().map(|q| q.key.as_str()),
            Some("Q1_2025"),
            "p at first quarter keeps us there"
        );
    }

    #[test]
    fn test_year_stats_computed_for_current_year() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert!(app.year_stats.is_some(), "year_stats should be populated");
    }

    #[test]
    fn test_year_stats_cleared_when_no_quarter() {
        let qd = TimePeriodData::new();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 5, 10);
        let app = make_test_app(qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert!(app.current_period().is_none());
        assert!(
            app.year_stats.is_none(),
            "year_stats should be None when no quarter"
        );
    }
}
