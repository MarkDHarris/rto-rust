# AGENTS.md

Guidance for AI agents working in this repository.

---

## Commands

```bash
cargo build                              # compile (dev)
cargo build --release                    # compile (optimized)
cargo test                               # run all 194 tests
cargo test quarter_calc                  # run tests whose name contains "quarter_calc"
cargo test -- --nocapture               # show println! output during tests
cargo clippy                             # lint
cargo fmt                                # format
cargo run -- --data-dir ./config         # run TUI (requires a real terminal)
cargo run -- --data-dir ./config stats Q1_2026    # print stats to stdout (no terminal needed)
cargo run -- --data-dir ./config vacations        # print vacation list
cargo run -- --data-dir ./config holidays         # print holiday list
make coverage-summary                    # print line coverage percentages
make coverage-html                       # open HTML coverage report in browser
```

---

## Architecture

Four layers, each a subdirectory under `src/`:

```
data/   → persistence + domain structs
calc/   → pure statistics functions (no I/O)
cmd/    → one run() fn per subcommand
ui/     → ratatui TUI (calendar_view.rs is the largest file, ~2,500 lines)
```

### Data layer (`src/data/`)

Two persistence mechanisms coexist:

1. **`Persistable` trait** — used by types with fixed filenames (`BadgeEntryData`, `HolidayData`,
   `VacationData`, `EventData`). Provides default `load()` / `save()` / `load_from(dir)` /
   `save_to(dir)` implementations; the only required methods are `filename()` and `is_json()`.

2. **Generic helpers** — `load_yaml_from<T>(dir, filename)`, `save_yaml_to<T>(dir, filename)`,
   `load_json_from<T>(dir, filename)`, `save_json_to<T>(dir, filename)` — used by types with
   dynamic filenames (`AppSettings`, `TimePeriodData`).

All file I/O routes through `persistence::get_data_dir()`, which reads from a
`static OnceLock<PathBuf>` set once in `main()` via `persistence::set_data_dir()`.

For unit tests, use `load_from(dir)` / `save_to(dir)` instead of `load()` / `save()` —
these bypass the `OnceLock` and accept an explicit directory.

**`settings.yaml`** is a standalone file containing `AppSettings` (goal, default_office,
flex_credit, time_periods list). It is loaded/saved independently from time period files.

**Time period files** (e.g. `workday-fiscal-quarters.yaml`) are referenced by filename in
`settings.yaml`. `TimePeriodData::load_from(dir, filename)` loads and parses a single file.
Each `TimePeriod` has a `key`, `name`, `start_date`, `end_date`, and parsed `NaiveDate` fields.

`BadgeEntry` has two boolean fields:
- `is_badged_in: bool` — `#[serde(default)]` (missing → `false`, matching Go behavior)
- `is_flex_credit: bool` — `#[serde(default)]` (missing → `false`)

`BadgeEntry.date_time` uses `FlexTime`, a newtype around `NaiveDateTime` that deserializes
from multiple formats: RFC3339 with offset, naive datetime, explicit UTC Z suffix, or
date-only (parsed as midnight).

Flex credit detection uses **only** `badge_entry.is_flex_credit`. The `flex_credit` string in
`AppSettings` is a display label only — it is never used to classify entries.

### Calculation layer (`src/calc/`)

`calculate_quarter_stats()` is a pure function — takes references to data sources plus a
`goal_pct: i32` and returns a `QuarterStats` value with no side effects.

`calculate_year_stats()` aggregates across multiple time periods for annual statistics.

**`QuarterStats` key fields:**

| Field | Meaning |
|-------|---------|
| `total_calendar_days` | All calendar days (including weekends) from start to end inclusive |
| `available_workdays` | All weekdays in the range (including holidays — matches Go behavior) |
| `total_days` | Weekdays that are **not holidays and not vacation days** |
| `days_required` | `⌈total_days × goal_pct / 100⌉` |
| `days_badged_in` | Total badge entries in the period (office + flex combined) |
| `flex_days` | Badge entries where `is_flex_credit = true` |
| `days_thus_far` | Workdays elapsed before today |
| `days_left` | Workdays remaining after today |
| `days_still_needed` | `max(0, days_required − days_badged_in)` |
| `days_ahead_of_pace` | Positive = ahead, negative = behind |
| `remaining_missable_days` | `days_left − days_still_needed` |
| `compliance_status` | `Achieved`, `On Track`, `At Risk`, or `Impossible` |
| `projected_completion_date` | Estimated date to reach goal at current rate |
| `workday_stats` | `HashMap<String, Workday>` — per-day flags for the calendar renderer |

Formulas:
- `total_calendar_days = (end − start).num_days() as i32 + 1`
- `days_required = ⌈total_days × goal_pct / 100⌉`
- `pace = round(days_thus_far × days_required / total_days)` (expected badge-ins by now)
- `projected_date = today + ⌈days_still_needed / current_rate⌉`

### UI layer (`src/ui/calendar_view.rs`)

`App<'a>` holds borrowed references (`&'a TimePeriodData`, `&'a mut BadgeEntryData`, etc.)
plus owned state (`what_if_snapshot`, `git_status`, `settings`, `nav_date`, `year_stats`, etc.).

**Four views** dispatched by `ViewState`:

```rust
enum ViewState { Calendar, Vacations, Holidays, Settings }
```

**Calendar modes:** `Normal`, `Add`, `Delete`, `Search`.

**Time period view switching:** `Space` / `Shift+→` / `Shift+←` cycles through the time
period files listed in `settings.time_periods`. Each file defines its own
`calendar_display_columns` which adjusts the calendar grid automatically.

**Navigation safety:** `nav_date: NaiveDate` is always updated by `n`/`p` regardless of
whether a `current_period` is found. The user can navigate past all configured periods and
press the opposite key to return.

**What-if mode:** Entering clones `badge_data` into `what_if_snapshot`. Exiting does
`*self.badge_data = snapshot` to restore in-place.

**Year stats:** `update_year_stats()` finds all periods in the current year, computes
`min(start_date)` / `max(end_date)`, builds a synthetic `TimePeriod`, and calls
`calculate_year_stats`.

**Git backup** (`g` key): delegates to `cmd::backup::perform()`.

### Command layer (`src/cmd/`)

| File | Subcommand | Notes |
|------|-----------|-------|
| `root.rs` | *(none)* | Loads all data, starts TUI, saves all data on exit |
| `init.rs` | `init` | `run_in_dir(dir)` bypasses OnceLock for testability |
| `stats.rs` | `stats [KEY]` | `write_stats<W: Write>` for testability; optional period key |
| `vacations.rs` | `vacations` | `write_vacations<W: Write>` for testability |
| `holidays.rs` | `holidays` | `write_holidays<W: Write>` for testability |
| `backup.rs` | `backup` | `perform(dir, remote)` and `status(dir)` functions |

---

## Key Conventions

- **All date keys are `"YYYY-MM-DD"` strings.** Badge, holiday, vacation, and workday maps all
  use this format — joining them is a single `.get(&key)` lookup with no date parsing.

- **`stats.rs` (CLI) and `render_stats()` (TUI) must stay in sync** when `QuarterStats` fields
  change. Both display the same metrics; the TUI adds color and the CLI adds alignment.

- **Flex credit is boolean only.** `day.is_flex_credit = badge_entry.is_flex_credit`. The
  `flex_credit` setting string is a label and must never be used for classification.

- **`calculate_quarter_stats` signature:** takes `(period, badge, holiday, vacation, goal_pct, today)`.
  Pass `None` for `today` in production (uses `Local::now()`); pass `Some(date)` in tests.

- **`calculate_year_stats`** aggregates across multiple periods for a given year.

- **No terminal in tests.** `App::new()` accepts `today: NaiveDate` and `data_dir: PathBuf`
  for dependency injection. `run_app()` requires a real terminal and is not unit-tested.

- **`serde_norway`** is used for all YAML (not `serde_yaml`). Do not add `serde_yaml`.

- **Data format compatibility:** This Rust implementation reads and writes the same files as
  the Go implementation (`rto-go`). Both can operate on the same data directory.

---

## Common Pitfalls

- **NLL borrow conflict in `root.rs`:** After `run_app()`, `app` still holds `&mut` borrows.
  Extract `app.settings.clone()` and `drop(app)` before calling `badge_data.save()` etc.

- **`DATA_DIR` OnceLock cannot be reset.** Integration tests that need different data
  directories must use `load_from(dir)` / `save_to(dir)` / `run_in_dir(dir)` rather than
  `load()` / `save()` / `run()`.

- **Period navigation past configured range** returns `current_period = None` and
  `active_stats = None`, but `nav_date` is always updated — this is intentional.

- **Year stats require at least one period** for the current `nav_date` year.

- **Settings are saved independently** to `settings.yaml` via `AppSettings::save_to()`.
  Time period files are separate YAML files referenced by `settings.time_periods`.
