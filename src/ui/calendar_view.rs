use crate::calc::{calculate_quarter_stats, QuarterStats};
use crate::data::{
    AppSettings, BadgeEntry, BadgeEntryData, Event, EventData, HolidayData, QuarterConfig,
    QuarterData, VacationData,
};
use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate};
use crossterm::event::{self, Event as CEvent, KeyCode, KeyModifiers};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame, Terminal,
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
    quarter_data: &'a QuarterData,
    badge_data: &'a mut BadgeEntryData,
    holiday_data: &'a mut HolidayData,
    vacation_data: &'a mut VacationData,
    event_data: &'a mut EventData,
    current_quarter: Option<&'a QuarterConfig>,
    selected_date: NaiveDate,
    today: NaiveDate,
    /// Tracks the navigation position used by n/p. Updated on every n/p press
    /// even when current_quarter is None, so the user can always navigate back.
    nav_date: NaiveDate,
    mode: Mode,
    input_buffer: String,
    cursor_index: usize,
    quarter_stats: Option<QuarterStats>,
    /// Year-level aggregate stats spanning Q1 start → Q4 end for the viewed year.
    year_stats: Option<QuarterStats>,
    table_state: TableState,
    pub settings: AppSettings,
    /// When Some, the app is in what-if mode. Holds the original badge data
    /// so it can be restored when exiting the mode.
    what_if_snapshot: Option<BadgeEntryData>,
    /// Absolute path to the data directory, used for git backup.
    data_dir: PathBuf,
    /// Result of the last git backup (message, color). Cleared on next keypress.
    git_status: Option<(String, Color)>,
    /// Which top-level view is active.
    view_state: ViewState,
    /// Selected row in vacation/holiday list view.
    list_cursor: usize,
    /// 0 = browsing; 1–N = entering add/edit field N.
    list_add_stage: u8,
    /// Completed fields during a multi-stage add/edit operation.
    list_field_bufs: Vec<String>,
    /// When Some, we are editing the item at this index rather than adding a new one.
    list_edit_index: Option<usize>,
}

impl<'a> App<'a> {
    pub fn new(
        quarter_data: &'a QuarterData,
        badge_data: &'a mut BadgeEntryData,
        holiday_data: &'a mut HolidayData,
        vacation_data: &'a mut VacationData,
        event_data: &'a mut EventData,
        settings: AppSettings,
        today: NaiveDate,
        data_dir: PathBuf,
    ) -> Self {
        let current_quarter = quarter_data.get_quarter_by_date(today);
        let selected_date = today;
        // Initialise nav_date to the start of the current quarter so that the
        // first n/p press moves exactly one quarter forward/backward.
        let nav_date = current_quarter
            .and_then(|q| q.start_date)
            .unwrap_or(today);
        let mut app = App {
            quarter_data,
            badge_data,
            holiday_data,
            vacation_data,
            event_data,
            current_quarter,
            selected_date,
            today,
            nav_date,
            mode: Mode::Normal,
            input_buffer: String::new(),
            cursor_index: 0,
            quarter_stats: None,
            year_stats: None,
            table_state: TableState::default(),
            settings,
            what_if_snapshot: None,
            data_dir,
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
        if let Some(q) = self.current_quarter {
            match calculate_quarter_stats(
                q,
                self.badge_data,
                self.holiday_data,
                self.vacation_data,
                None,
            ) {
                Ok(stats) => self.quarter_stats = Some(stats),
                Err(e) => {
                    self.quarter_stats = None;
                    eprintln!("Error calculating stats: {e}");
                }
            }
        } else {
            self.quarter_stats = None;
        }
        self.update_year_stats();
    }

    fn update_year_stats(&mut self) {
        let year_str = match self.current_quarter {
            Some(q) => q.year.clone(),
            None => {
                self.year_stats = None;
                return;
            }
        };

        // Find the earliest start and latest end among all quarters in this year.
        let year_start: Option<NaiveDate> = self
            .quarter_data
            .quarters
            .iter()
            .filter(|q| q.year == year_str)
            .filter_map(|q| q.start_date)
            .min();
        let year_end: Option<NaiveDate> = self
            .quarter_data
            .quarters
            .iter()
            .filter(|q| q.year == year_str)
            .filter_map(|q| q.end_date)
            .max();

        let (start, end) = match (year_start, year_end) {
            (Some(s), Some(e)) => (s, e),
            _ => {
                self.year_stats = None;
                return;
            }
        };

        // Build a synthetic QuarterConfig spanning the full year range.
        let year_q = QuarterConfig {
            key: format!("YEAR_{}", year_str),
            quarter: "YEAR".to_string(),
            year: year_str,
            start_date_raw: start.format("%Y-%m-%d").to_string(),
            end_date_raw: end.format("%Y-%m-%d").to_string(),
            start_date: Some(start),
            end_date: Some(end),
        };

        match calculate_quarter_stats(
            &year_q,
            self.badge_data,
            self.holiday_data,
            self.vacation_data,
            None,
        ) {
            Ok(stats) => self.year_stats = Some(stats),
            Err(_) => self.year_stats = None,
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
                self.git_status =
                    Some((format!("git commit error: {}", e), Color::Red));
                return;
            }
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                if stdout.contains("nothing to commit")
                    || stderr.contains("nothing to commit")
                    || stdout.contains("nothing added")
                {
                    self.git_status =
                        Some(("Nothing to commit — already up to date".to_string(), Color::Yellow));
                    return;
                }
                if !out.status.success() {
                    let detail = stdout.trim().to_string();
                    self.git_status =
                        Some((format!("git commit failed: {}", detail), Color::Red));
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
                self.git_status = Some((
                    format!("Backed up & pushed — {}", timestamp),
                    Color::Green,
                ));
            } else {
                self.git_status = Some((
                    format!("Committed locally (push failed) — {}", timestamp),
                    Color::Yellow,
                ));
            }
        } else {
            self.git_status = Some((
                format!("Backed up locally — {}", timestamp),
                Color::Cyan,
            ));
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
                        if self.current_quarter.is_some() {
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
                        if self.current_quarter.is_some() {
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
                        let target = add_months(self.nav_date, 3);
                        self.nav_date = target;
                        self.current_quarter =
                            self.quarter_data.get_quarter_by_date(target);
                        self.update_stats();
                    }
                    KeyCode::Char('p') => {
                        let target = add_months(self.nav_date, -3);
                        self.nav_date = target;
                        self.current_quarter =
                            self.quarter_data.get_quarter_by_date(target);
                        self.update_stats();
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

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(9),  // calendar (3 months, max 8 rows + 1 padding)
                        Constraint::Length(30), // quarter stats table (5 sections with spacers + working/available days)
                        Constraint::Length(11), // year stats table (8 data rows + header + borders)
                        Constraint::Min(12),    // events + help table
                    ])
                    .split(size);

                self.render_calendar(f, chunks[0]);
                self.render_stats(f, chunks[1]);
                self.render_year_stats(f, chunks[2]);
                self.render_events_and_help(f, chunks[3]);
            }
        }
    }

    fn render_calendar(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let q = match self.current_quarter {
            Some(q) => q,
            None => {
                let p = Paragraph::new("Configuration Error: Current quarter not set.");
                f.render_widget(p, area);
                return;
            }
        };
        let stats = match &self.quarter_stats {
            Some(s) => s,
            None => {
                let p = Paragraph::new("Loading stats...");
                f.render_widget(p, area);
                return;
            }
        };

        let start = match q.start_date {
            Some(s) => s,
            None => return,
        };

        let today = self.today;
        let event_map = self.event_data.get_event_map();

        // Fixed-width columns: 21-char months, 11-char gaps, Min(0) absorbs leftover
        const MONTH_WIDTH: u16 = 21;
        const GAP_WIDTH: u16 = 11;
        let month_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(MONTH_WIDTH),
                Constraint::Length(GAP_WIDTH),
                Constraint::Length(MONTH_WIDTH),
                Constraint::Length(GAP_WIDTH),
                Constraint::Length(MONTH_WIDTH),
                Constraint::Min(0),
            ])
            .split(area);
        let month_rects = [month_chunks[0], month_chunks[2], month_chunks[4]];

        for i in 0..3 {
            let month_date = add_months(start, i as i32);
            let year = month_date.year();
            let month = month_date.month();

            let title = format!("{} {}", month_name(month), year);
            let header = "Su Mo Tu We Th Fr Sa";

            let first_of_month = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
            let days_in_month = days_in_month(year, month);
            let start_dow = first_of_month.weekday().num_days_from_sunday() as usize;

            let mut lines: Vec<Line> = vec![
                Line::from(Span::styled(
                    format!("{:^21}", title),
                    Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )),
                Line::from(header),
            ];

            let mut day = 1usize;
            for _row in 0..6 {
                if day > days_in_month as usize {
                    break;
                }
                let mut spans = Vec::new();
                for col in 0..7usize {
                    if (_row == 0 && col < start_dow) || day > days_in_month as usize {
                        spans.push(Span::raw("   "));
                        continue;
                    }
                    let date = NaiveDate::from_ymd_opt(year, month, day as u32).unwrap();
                    let date_key = date.format("%Y-%m-%d").to_string();
                    let day_str = format!("{:2}", day);

                    let workday_entry = stats.workday_stats.get(&date_key);
                    let is_today = date == today;
                    let is_selected = date == self.selected_date;
                    let is_badged = workday_entry.map(|w| w.is_badged_in).unwrap_or(false);
                    let is_flex = workday_entry.map(|w| w.is_flex_credit).unwrap_or(false);
                    let is_holiday_or_vacation = workday_entry
                        .map(|w| w.is_holiday || w.is_vacation)
                        .unwrap_or(false);
                    let is_weekend = workday_entry.is_none();
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

                    spans.push(Span::styled(day_str, style));
                    spans.push(Span::raw(" "));
                    day += 1;
                }
                lines.push(Line::from(spans));
            }

            let calendar_widget =
                Paragraph::new(lines).block(Block::default().borders(Borders::NONE));
            f.render_widget(calendar_widget, month_rects[i]);
        }
    }

    fn render_stats(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let stats = match &self.quarter_stats {
            Some(s) => s.clone(),
            None => return,
        };

        // ── Computed display values ───────────────────────────────────────────────

        let status_color = match stats.compliance_status.as_str() {
            "Achieved" => Color::Green,
            "On Track" => Color::Cyan,
            "At Risk" => Color::Yellow,
            "Impossible" | _ => Color::Red,
        };

        let pace_str = if stats.days_ahead_of_pace > 0 {
            format!("+{} days ahead", stats.days_ahead_of_pace)
        } else if stats.days_ahead_of_pace < 0 {
            format!("{} days behind", stats.days_ahead_of_pace)
        } else {
            "On pace".to_string()
        };
        let pace_color = if stats.days_ahead_of_pace >= 0 {
            Color::Green
        } else {
            Color::Red
        };

        let skip_color = if stats.remaining_missable_days > 5 {
            Color::Green
        } else if stats.remaining_missable_days > 0 {
            Color::Yellow
        } else {
            Color::Red
        };

        let required_pct = if stats.total_days > 0 {
            100.0 * stats.days_required as f64 / stats.total_days as f64
        } else {
            0.0
        };

        let still_needed_color = if stats.days_still_needed == 0 {
            Color::Green
        } else if stats.days_still_needed <= 5 {
            Color::Yellow
        } else {
            Color::White
        };

        let rate_so_far_color = if stats.current_average >= 50.0 {
            Color::Green
        } else if stats.current_average >= 45.0 {
            Color::Yellow
        } else {
            Color::Red
        };

        let rate_needed_color = if stats.required_future_average <= 50.0 {
            Color::Green
        } else if stats.required_future_average <= 70.0 {
            Color::Yellow
        } else {
            Color::Red
        };
        let rate_needed_val = format!("{} / {}", stats.days_still_needed, stats.days_left);
        let rate_needed_pct = if stats.days_left > 0 {
            format!("{:.1}%", stats.required_future_average)
        } else if stats.days_still_needed > 0 {
            "Infinite".to_string()
        } else {
            "N/A".to_string()
        };

        let office_days = stats.days_badged_in - stats.flex_days;

        // ── Build rows ────────────────────────────────────────────────────────────
        let header_style = Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);
        let header = Row::new(vec![
            Cell::from("Metric").style(header_style),
            Cell::from("Value").style(header_style),
            Cell::from("%").style(header_style),
        ]);

        let mut rows: Vec<Row> = vec![
            // ── STATUS ────────────────────────────────────────────────────────────
            section_header("STATUS"),
            data_row(
                "Status",
                colored(stats.compliance_status.clone(), status_color),
                plain(""),
            ),
            data_row("Days Ahead of Pace", colored(pace_str, pace_color), plain("")),
            data_row(
                "Skippable Days Left",
                colored(format!("{}", stats.remaining_missable_days), skip_color),
                plain(""),
            ),
            spacer(),
            // ── PROGRESS ─────────────────────────────────────────────────────────
            section_header("PROGRESS"),
            data_row(
                "Total Days in Qtr",
                plain(format!("{}", stats.total_calendar_days)),
                plain(""),
            ),
            data_row(
                "Total Working Days",
                plain(format!("{}", stats.available_workdays)),
                plain(""),
            ),
            data_row(
                "Available Working Days",
                plain(format!("{}", stats.total_days)),
                plain(""),
            ),
            data_row(
                "Goal (50% Required)",
                plain(format!("{} / {}", stats.days_required, stats.total_days)),
                plain(format!("{:.1}%", required_pct)),
            ),
            data_row(
                "Badged In Days",
                plain(format!("{}", stats.days_badged_in)),
                plain(""),
            ),
            data_row(
                "Still Needed",
                colored(format!("{}", stats.days_still_needed), still_needed_color),
                plain(""),
            ),
            data_row(
                "Rate So Far",
                plain(format!("{} / {}", stats.days_badged_in, stats.days_thus_far)),
                colored(format!("{:.1}%", stats.current_average), rate_so_far_color),
            ),
            spacer(),
            // ── BADGE BREAKDOWN ───────────────────────────────────────────────────
            section_header("BADGE BREAKDOWN"),
            data_row("Office Days", plain(format!("{}", office_days)), plain("")),
            data_row(
                "Flex Credits",
                colored(format!("{}", stats.flex_days), FLEX_COLOR),
                plain(""),
            ),
            data_row(
                "Total Badged In",
                plain(format!("{}", stats.days_badged_in)),
                plain(""),
            ),
            spacer(),
            // ── LOOKING AHEAD ─────────────────────────────────────────────────────
            section_header("LOOKING AHEAD"),
            data_row(
                "Rate Needed (Remaining)",
                plain(rate_needed_val),
                colored(rate_needed_pct, rate_needed_color),
            ),
        ];

        if let Some(proj) = stats.projected_completion_date {
            rows.push(data_row(
                "Projected Completion",
                plain(proj.format("%Y-%m-%d").to_string()),
                plain(""),
            ));
        }

        rows.extend_from_slice(&[
            spacer(),
            // ── DAYS OFF ──────────────────────────────────────────────────────────
            section_header("DAYS OFF"),
            data_row("Holidays", plain(format!("{}", stats.holidays)), plain("")),
            data_row(
                "Vacation Days",
                plain(format!("{}", stats.vacation_days)),
                plain(""),
            ),
            data_row(
                "Total Days Off",
                plain(format!("{}", stats.days_off)),
                plain(""),
            ),
        ]);

        let quarter_key = self
            .current_quarter
            .map(|q| q.key.as_str())
            .unwrap_or("N/A");
        let (title_text, title_style) = if self.is_what_if() {
            (
                format!(" Quarter Stats: {} [What-If Mode] ", quarter_key),
                Style::default()
                    .fg(FLEX_COLOR)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                format!(" Quarter Stats: {} ", quarter_key),
                Style::default(),
            )
        };

        let table = Table::new(
            rows,
            [
                Constraint::Length(26),
                Constraint::Length(16),
                Constraint::Length(8),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title_text)
                .title_style(title_style),
        )
        .row_highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

        f.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_year_stats(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let stats = match &self.year_stats {
            Some(s) => s.clone(),
            None => return,
        };
        let year = match self.current_quarter {
            Some(q) => q.year.as_str().to_string(),
            None => return,
        };

        let office_days = stats.days_badged_in - stats.flex_days;

        let header_style = Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);
        let header = Row::new(vec![
            Cell::from("Metric").style(header_style),
            Cell::from("Value").style(header_style),
            Cell::from("").style(header_style),
        ]);

        let rows = vec![
            data_row(
                "Total Calendar Days",
                plain(format!("{}", stats.total_calendar_days)),
                plain(""),
            ),
            data_row(
                "Total Working Days",
                plain(format!("{}", stats.available_workdays)),
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
            data_row("Office Days", plain(format!("{}", office_days)), plain("")),
            data_row(
                "Flex Credits",
                colored(format!("{}", stats.flex_days), FLEX_COLOR),
                plain(""),
            ),
            data_row(
                "Total Badged In",
                plain(format!("{}", stats.days_badged_in)),
                plain(""),
            ),
        ];

        let table = Table::new(
            rows,
            [
                Constraint::Length(26),
                Constraint::Length(16),
                Constraint::Length(8),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Year Stats: {} ", year)),
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

        // ── Git backup status (shown until next keypress) ─────────────────────
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

        // ── Events for selected date ──────────────────────────────────────────
        lines.push(Line::from(format!("Events for {}:", date_key)));

        match self.mode {
            Mode::Add => {
                lines.push(Line::from(format!(
                    "  Adding: {}_",
                    self.input_buffer
                )));
            }
            Mode::Delete => {
                lines.push(Line::from("  Select event to delete:"));
                if events.is_empty() {
                    lines.push(Line::from("  (no events)"));
                } else {
                    for (i, e) in events.iter().enumerate() {
                        let prefix = if i == self.cursor_index { "  > " } else { "    " };
                        lines.push(Line::from(format!("{}{}", prefix, e.description)));
                    }
                    lines.push(Line::from("  Enter=delete  Esc=cancel  ↑↓=move"));
                }
            }
            Mode::Search => {
                lines.push(Line::from(format!("  Search: {}_", self.input_buffer)));
                for event in search_events(&self.event_data.events, &self.input_buffer) {
                    lines.push(Line::from(format!(
                        "  {} — {}",
                        event.date, event.description
                    )));
                }
            }
            _ => {
                if events.is_empty() {
                    lines.push(Line::from("  (no events)"));
                } else {
                    for e in &events {
                        lines.push(Line::from(format!("  • {}", e.description)));
                    }
                }
            }
        }

        // ── Key bindings as a Table ───────────────────────────────────────────
        let what_if_action = if self.is_what_if() {
            "Exit What-If Mode"
        } else {
            "Enter What-If Mode"
        };

        let key_rows: Vec<Row> = vec![
            Row::new(vec!["← → ↑ ↓", "Move date", "n / p", "Next/prev quarter"]),
            Row::new(vec!["Space", "Badge (office)", "f", "Flex credit"]),
            Row::new(vec!["a", "Add event", "d", "Delete event"]),
            Row::new(vec!["s", "Search", "w", what_if_action]),
            Row::new(vec!["g", "Git backup", "v", "Vacations"]),
            Row::new(vec!["h", "Holidays", "o", "Settings"]),
            Row::new(vec!["q/Ctrl+C", "Quit", "", ""]),
        ];

        let help_table = Table::new(
            key_rows,
            [
                Constraint::Length(12),
                Constraint::Length(24),
                Constraint::Length(12),
                Constraint::Length(24),
            ],
        )
        .block(Block::default().borders(Borders::NONE))
        .column_spacing(1);

        // Split the area: events content on top, help table + footer on bottom
        let bottom_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(2),     // events content
                Constraint::Length(8),  // help table (6 rows + 1 blank + footer)
            ])
            .split(area);

        let p = Paragraph::new(lines).block(Block::default().borders(Borders::NONE));
        f.render_widget(p, bottom_chunks[0]);

        // Render help table + footer in the bottom chunk
        let help_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(6),    // help table
                Constraint::Length(1), // data dir footer
            ])
            .split(bottom_chunks[1]);

        f.render_widget(help_table, help_chunks[0]);

        let footer = Paragraph::new(Line::from(vec![
            Span::styled("Data  ", Style::default().add_modifier(Modifier::DIM)),
            Span::styled(
                self.data_dir.to_string_lossy().to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        f.render_widget(footer, help_chunks[1]);
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
            let labels = ["Destination", "Start date (YYYY-MM-DD)", "End date (YYYY-MM-DD)", "Approved? (y/n)"];
            let form_title = if self.list_edit_index.is_some() {
                "── Edit Vacation ─────────────────────────────────"
            } else {
                "── Add Vacation ─────────────────────────────────"
            };
            let mut form_lines: Vec<Line> = vec![
                Line::from(Span::styled(
                    form_title,
                    Style::default().add_modifier(Modifier::BOLD),
                )),
            ];
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
            form_lines.push(Line::from(
                Span::styled("Enter=confirm  Esc=cancel", Style::default().fg(Color::DarkGray)),
            ));
            let p = Paragraph::new(form_lines).block(Block::default().borders(Borders::NONE));
            f.render_widget(p, bottom);
        } else {
            let hints = Paragraph::new(vec![
                Line::from(Span::styled(
                    "↑↓=move  a=add  Enter/e=edit  Del/x=delete  Esc=back",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
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
                    if self.list_add_stage == 2 || self.list_add_stage == 3 {
                        if NaiveDate::parse_from_str(&self.input_buffer, "%Y-%m-%d").is_err() {
                            self.input_buffer = "Invalid date — use YYYY-MM-DD".to_string();
                            return;
                        }
                    }
                    self.list_field_bufs.push(self.input_buffer.clone());

                    if self.list_add_stage == 4 {
                        // All fields gathered — build vacation
                        let approved = self.list_field_bufs
                            .get(3)
                            .map(|s| s.to_lowercase().starts_with('y'))
                            .unwrap_or(false);
                        let v = Vacation::new(
                            self.list_field_bufs.get(0).map(String::as_str).unwrap_or(""),
                            self.list_field_bufs.get(1).map(String::as_str).unwrap_or(""),
                            self.list_field_bufs.get(2).map(String::as_str).unwrap_or(""),
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
                                    4 => if v.approved { "y".to_string() } else { "n".to_string() },
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
            let mut form_lines: Vec<Line> = vec![
                Line::from(Span::styled(
                    form_title,
                    Style::default().add_modifier(Modifier::BOLD),
                )),
            ];
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
            form_lines.push(Line::from(
                Span::styled("Enter=confirm  Esc=cancel", Style::default().fg(Color::DarkGray)),
            ));
            let p = Paragraph::new(form_lines).block(Block::default().borders(Borders::NONE));
            f.render_widget(p, bottom);
        } else {
            let hints = Paragraph::new(vec![
                Line::from(Span::styled(
                    "↑↓=move  a=add  Enter/e=edit  Del/x=delete  Esc=back",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
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
                    if self.list_add_stage == 1 {
                        if NaiveDate::parse_from_str(&self.input_buffer, "%Y-%m-%d").is_err() {
                            self.input_buffer = "Invalid date — use YYYY-MM-DD".to_string();
                            return;
                        }
                    }
                    self.list_field_bufs.push(self.input_buffer.clone());

                    if self.list_add_stage == 2 {
                        // field_bufs: [0]=date, [1]=name
                        let h = Holiday::new(
                            self.list_field_bufs.get(1).map(String::as_str).unwrap_or(""),
                            self.list_field_bufs.get(0).map(String::as_str).unwrap_or(""),
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
                Row::new(vec![
                    Cell::from(format!("  {}", label)),
                    Cell::from(value),
                ])
            })
            .collect();

        let mut table_state = TableState::default();
        table_state.select(Some(self.list_cursor));

        let table = Table::new(
            rows,
            [Constraint::Length(22), Constraint::Min(30)],
        )
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
        Cell::from(title.to_string())
            .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
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
fn data_row(
    metric: impl Into<String>,
    value: Cell<'static>,
    pct: Cell<'static>,
) -> Row<'static> {
    Row::new(vec![
        Cell::from(format!("  {}", metric.into())),
        value,
        pct,
    ])
}

/// Plain (unstyled) cell.
fn plain(s: impl Into<String>) -> Cell<'static> {
    Cell::from(s.into())
}

/// Colored cell.
fn colored(s: impl Into<String>, color: Color) -> Cell<'static> {
    Cell::from(s.into()).style(Style::default().fg(color))
}

// ── App event loop ────────────────────────────────────────────────────────────

pub fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| app.render(f))?;
        if event::poll(StdDuration::from_millis(16))? {
            if let CEvent::Key(key) = event::read()? {
                if app.handle_key(key.code, key.modifiers) {
                    break;
                }
            }
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
        .filter(|e| {
            e.description.to_lowercase().contains(&q) || e.date.contains(query)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{AppSettings, BadgeEntryData, EventData, HolidayData, QuarterData, VacationData};
    use chrono::NaiveDate;
    use crossterm::event::{KeyCode, KeyModifiers};
    use std::path::PathBuf;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    /// Build a QuarterData with two adjacent quarters for navigation tests.
    fn make_quarter_data() -> QuarterData {
        use crate::data::quarter::QuarterConfig;
        let mut q1 = QuarterConfig {
            key: "Q1_2025".to_string(),
            quarter: "Q1".to_string(),
            year: "2025".to_string(),
            start_date_raw: "2025-01-01".to_string(),
            end_date_raw: "2025-03-31".to_string(),
            start_date: None,
            end_date: None,
        };
        q1.parse_dates().unwrap();
        let mut q2 = QuarterConfig {
            key: "Q2_2025".to_string(),
            quarter: "Q2".to_string(),
            year: "2025".to_string(),
            start_date_raw: "2025-04-01".to_string(),
            end_date_raw: "2025-06-30".to_string(),
            start_date: None,
            end_date: None,
        };
        q2.parse_dates().unwrap();
        QuarterData {
            quarters: vec![q1, q2],
        }
    }

    fn make_test_app<'a>(
        quarter_data: &'a QuarterData,
        badge_data: &'a mut BadgeEntryData,
        holiday_data: &'a mut HolidayData,
        vacation_data: &'a mut VacationData,
        event_data: &'a mut EventData,
        today: NaiveDate,
    ) -> App<'a> {
        App::new(
            quarter_data,
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
        assert_eq!(s, Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD));
    }

    #[test]
    fn test_style_selected_badged_flex() {
        let s = calendar_day_style(true, true, true, false, false, false, false);
        assert_eq!(s, Style::default().fg(Color::Black).bg(FLEX_COLOR).add_modifier(Modifier::BOLD));
    }

    #[test]
    fn test_style_selected_holiday() {
        let s = calendar_day_style(true, false, false, true, false, false, false);
        assert_eq!(s, Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD));
    }

    #[test]
    fn test_style_selected_plain() {
        let s = calendar_day_style(true, false, false, false, false, false, false);
        assert_eq!(s, Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD));
    }

    #[test]
    fn test_style_badged_office_not_selected() {
        let s = calendar_day_style(false, true, false, false, false, false, false);
        assert_eq!(
            s,
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        );
    }

    #[test]
    fn test_style_badged_flex_not_selected() {
        let s = calendar_day_style(false, true, true, false, false, false, false);
        assert_eq!(
            s,
            Style::default().fg(FLEX_COLOR).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
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
        assert_eq!(s, Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD));
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
        Event { date: date.to_string(), description: desc.to_string() }
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
        let events = vec![ev("2025-01-01", "Team Lunch"), ev("2025-01-02", "Team Meeting")];
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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

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
    fn test_space_toggles_office_badge() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        let key = today.format("%Y-%m-%d").to_string();
        assert!(!app.badge_data.has(&key));

        app.handle_key(KeyCode::Char(' '), KeyModifiers::empty());
        assert!(app.badge_data.has(&key));

        app.handle_key(KeyCode::Char(' '), KeyModifiers::empty());
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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        let key = today.format("%Y-%m-%d").to_string();
        app.handle_key(KeyCode::Char('f'), KeyModifiers::empty());
        assert!(app.badge_data.has(&key));
        let entry = app.badge_data.data.iter().find(|e| e.key == key).unwrap();
        assert_eq!(entry.office, "Flex Credit");
    }

    #[test]
    fn test_space_does_nothing_outside_quarter() {
        let qd = QuarterData::default(); // no quarters
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        app.handle_key(KeyCode::Char(' '), KeyModifiers::empty());
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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

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
        ed.add(Event { date: "2025-02-10".to_string(), description: "To remove".to_string() });
        let today = d(2025, 2, 10);
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

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
        ed.add(Event { date: "2025-02-10".to_string(), description: "First".to_string() });
        ed.add(Event { date: "2025-02-10".to_string(), description: "Second".to_string() });
        let today = d(2025, 2, 10);
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert!(!app.is_what_if());

        // Enter what-if, add a badge
        app.handle_key(KeyCode::Char('w'), KeyModifiers::empty());
        assert!(app.is_what_if());
        app.handle_key(KeyCode::Char(' '), KeyModifiers::empty());
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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert_eq!(app.current_quarter.map(|q| q.key.as_str()), Some("Q1_2025"));
        app.handle_key(KeyCode::Char('n'), KeyModifiers::empty());
        assert_eq!(app.current_quarter.map(|q| q.key.as_str()), Some("Q2_2025"));
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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert_eq!(app.current_quarter.map(|q| q.key.as_str()), Some("Q2_2025"));
        app.handle_key(KeyCode::Char('p'), KeyModifiers::empty());
        assert_eq!(app.current_quarter.map(|q| q.key.as_str()), Some("Q1_2025"));
    }

    #[test]
    fn test_q_returns_true() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

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
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        // Enter what-if, remove the badge
        app.handle_key(KeyCode::Char('w'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char(' '), KeyModifiers::empty()); // toggle off
        assert_eq!(app.badge_data.data.len(), 0);

        // Quit — should restore original data
        let quit = app.handle_key(KeyCode::Char('q'), KeyModifiers::empty());
        assert!(quit);
        assert_eq!(app.badge_data.data.len(), 1);
    }

    #[test]
    fn test_n_past_last_quarter_then_p_returns() {
        // make_quarter_data has Q1 (Jan-Mar) and Q2 (Apr-Jun) 2025 only.
        // Navigate past Q2 with n, verify None, then press p to come back.
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 5, 10);
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert_eq!(app.current_quarter.map(|q| q.key.as_str()), Some("Q2_2025"));
        // Navigate past the last configured quarter
        app.handle_key(KeyCode::Char('n'), KeyModifiers::empty());
        assert!(app.current_quarter.is_none(), "should be None past last quarter");
        // Navigate back — must work even though current_quarter was None
        app.handle_key(KeyCode::Char('p'), KeyModifiers::empty());
        assert_eq!(
            app.current_quarter.map(|q| q.key.as_str()),
            Some("Q2_2025"),
            "should return to Q2 after pressing p"
        );
    }

    #[test]
    fn test_p_past_first_quarter_then_n_returns() {
        // Navigate before Q1 with p, verify None, then press n to come back.
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 2, 10);
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert_eq!(app.current_quarter.map(|q| q.key.as_str()), Some("Q1_2025"));
        // Navigate before the first configured quarter
        app.handle_key(KeyCode::Char('p'), KeyModifiers::empty());
        assert!(app.current_quarter.is_none(), "should be None before first quarter");
        // Navigate forward — must work even though current_quarter was None
        app.handle_key(KeyCode::Char('n'), KeyModifiers::empty());
        assert_eq!(
            app.current_quarter.map(|q| q.key.as_str()),
            Some("Q1_2025"),
            "should return to Q1 after pressing n"
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
        let app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        assert!(app.year_stats.is_some(), "year_stats should be populated");
    }

    #[test]
    fn test_year_stats_cleared_when_no_quarter() {
        let qd = make_quarter_data();
        let mut bd = BadgeEntryData::default();
        let mut hd = HolidayData::default();
        let mut vd = VacationData::default();
        let mut ed = EventData::default();
        let today = d(2025, 5, 10);
        let mut app = make_test_app(&qd, &mut bd, &mut hd, &mut vd, &mut ed, today);

        // Navigate past last quarter
        app.handle_key(KeyCode::Char('n'), KeyModifiers::empty());
        assert!(app.current_quarter.is_none());
        assert!(app.year_stats.is_none(), "year_stats should be None when no quarter");
    }
}
