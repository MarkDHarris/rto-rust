# AGENTS.md

Guidance for AI agents working in this repository.

---

## Commands

```bash
cargo build                              # compile (dev)
cargo build --release                    # compile (optimized)
cargo test                               # run all 162 tests
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

Every persistent type follows an identical pattern: an entity struct (e.g. `BadgeEntry`) plus
a container struct (e.g. `BadgeEntryData`) that implements `Persistable`. The trait provides
default `load()` / `save()` implementations; the only required methods are `filename()` and
`is_json()`. All file I/O routes through `persistence::get_data_dir()`, which reads from a
`static OnceLock<PathBuf>` set once in `main()` via `persistence::set_data_dir()`.

For unit tests, use `load_from(dir)` / `save_to(dir)` instead of `load()` / `save()` —
these bypass the `OnceLock` and accept an explicit directory.

`config.yaml` is read by two independent types: `QuarterData` (reads the `quarters` key) and
`SettingsWrapper` (reads the `settings` key). Serde ignores unknown keys, so both deserialize
from the same file without conflict. **Writing** uses a combined `ConfigFile` struct
(`cmd/init.rs::save_settings_to`) to prevent either type from overwriting the other's section.

`QuarterConfig` stores raw date strings for serialization and `#[serde(skip)]`
`Option<NaiveDate>` fields populated by `QuarterData::load_and_parse()`. Always call
`load_and_parse()` (not `load()`) when quarter dates are needed.

`BadgeEntry` has two boolean fields added for correctness:
- `is_badged_in: bool` — `#[serde(default = "default_true")]` (old entries → `true`)
- `is_flex_credit: bool` — `#[serde(default)]` (old entries → `false`)

Flex credit detection uses **only** `badge_entry.is_flex_credit`. The `flex_credit` string in
`AppSettings` is a display label only — it is never used to classify entries.

### Calculation layer (`src/calc/`)

`calculate_quarter_stats()` is a pure function — takes references to all four data sources and
returns a `QuarterStats` value with no side effects.

**`QuarterStats` key fields:**

| Field | Meaning |
|-------|---------|
| `total_calendar_days` | All calendar days (including weekends) from start to end inclusive |
| `available_workdays` | Weekdays in the range that are **not holidays** (vacation days count) |
| `total_days` | Weekdays that are **not holidays and not vacation days** (= "Available Working Days") |
| `days_required` | `(total_days + 1) / 2` — ceiling of 50%, the badge-in goal |
| `days_badged_in` | Total badge entries in the quarter (office + flex combined) |
| `flex_days` | Badge entries where `is_flex_credit = true` |
| `days_thus_far` | Workdays elapsed (up to today, including today if badged) |
| `days_left` | Workdays remaining after today |
| `days_still_needed` | `max(0, days_required − days_badged_in)` |
| `days_ahead_of_pace` | Positive = ahead, negative = behind |
| `remaining_missable_days` | Days you can still miss and still hit 50% |
| `workday_stats` | `HashMap<String, Workday>` — per-day flags for the calendar renderer |

**Display labels used in TUI (render_stats):**
- `total_calendar_days` → "Total Days in Qtr"
- `available_workdays` → "Total Working Days"
- `total_days` → "Available Working Days"

The `workday_stats` map is returned inside `QuarterStats` so the calendar renderer can color
each day without re-querying any data source.

Formulas:
- `total_calendar_days = (end − start).num_days() as i32 + 1`
- `days_required = (total_days + 1) / 2` (integer ceiling of 50%)
- `pace = days_required * days_thus_far / total_days` (expected badge-ins by now)

### UI layer (`src/ui/calendar_view.rs`)

`App<'a>` holds borrowed references (`&'a QuarterData`, `&'a mut BadgeEntryData`, etc.) plus
owned state (`what_if_snapshot`, `git_status`, `settings`, `nav_date`, `year_stats`, etc.).
The lifetime `'a` is enforced by the compiler — `App` cannot outlive the data it borrows.

**Four views** dispatched by `ViewState`:

```rust
enum ViewState { Calendar, Vacations, Holidays, Settings }
```

`render()` and `handle_key()` both dispatch on `view_state` first, then on `mode` (for
Calendar) or `list_add_stage` (for Vacations/Holidays/Settings).

**Calendar render layout (top to bottom):**
```
Length(9)   — three-month calendar side by side
Length(30)  — quarter stats Table widget (5 sections)
Length(11)  — year stats Table widget
Min(12)     — events for selected date + key bindings help
```

**Calendar modes:** `Normal`, `Add`, `Delete`, `Search`. (The `Edit` variant was removed —
it was declared but never constructed.)

**Navigation safety:** `nav_date: NaiveDate` is always updated by `n`/`p` regardless of
whether a `current_quarter` is found. This means the user can navigate past all configured
quarters and press the opposite key to return — there is no dead-lock on an empty state.

**What-if mode:** Entering clones `badge_data` into `what_if_snapshot`. Exiting does
`*self.badge_data = snapshot` to restore in-place. Both `q` and `Ctrl-C` call
`exit_what_if()` before returning `true` so `root.rs` always saves clean data.

**Year stats:** `update_year_stats()` filters `quarter_data.quarters` by `year` string,
computes `min(start_date)` / `max(end_date)`, builds a synthetic `QuarterConfig`, and calls
`calculate_quarter_stats`. Cleared to `None` when `current_quarter` is `None`.

**Git backup** (`g` key): shells out to `git -C <data_dir>` using `std::process::Command`
with `Stdio::null()`. Commit message timestamp format: `"%Y-%m-%d-%H-%M-%S-%3f"`.

**Settings view:** Two editable rows — `default_office` and `flex_credit`. Selecting a row
and pressing `Enter`/`e` pre-fills `input_buffer` with the current value for editing.
Settings are cloned out of `App` before `drop(app)` in `root.rs` and saved via
`cmd::init::save_settings_to()`.

**Vacations / Holidays views:** Full-screen Table widgets. Browse with `↑`/`↓`; `a` starts
a sequential add/edit (4 fields for vacation, 2 for holiday); `Delete`/`x` removes the
selected entry; `Esc`/`q` returns to Calendar. Date fields are validated with
`NaiveDate::parse_from_str`; invalid input shows an inline error and stays on the same field.

### Command layer (`src/cmd/`)

| File | Subcommand | Notes |
|------|-----------|-------|
| `root.rs` | *(none)* | Loads all data, starts TUI, saves all data on exit |
| `init.rs` | `init` | `run_in_dir(dir)` bypasses OnceLock for testability |
| `stats.rs` | `stats <KEY>` | `write_stats<W: Write>` for testability |
| `vacations.rs` | `vacations` | `write_vacations<W: Write>` for testability |
| `holidays.rs` | `holidays` | `write_holidays<W: Write>` for testability |
| `backup.rs` | `backup` | Shells out to git |

All testable output functions write to `&mut W: std::io::Write`. Pass `&mut Vec<u8>` in tests
and `&mut std::io::stdout()` in production.

---

## Key Conventions

- **All date keys are `"YYYY-MM-DD"` strings.** Badge, holiday, vacation, and workday maps all
  use this format — joining them is a single `.get(&key)` lookup with no date parsing.

- **`stats.rs` (CLI) and `render_stats()` (TUI) must stay in sync** when `QuarterStats` fields
  change. Both display the same metrics; the TUI adds color and the CLI adds alignment.

- **Section headers** in the stats table are produced by the free function `section_header()`
  at the bottom of `calendar_view.rs`. Data rows use `data_row()`, which adds a two-space
  indent automatically. The table header row uses bold white style
  (`Style::default().fg(Color::White).add_modifier(Modifier::BOLD)`).

- **Flex credit is boolean only.** `day.is_flex_credit = badge_entry.is_flex_credit`. The
  `flex_credit` setting string is a label and must never be used for classification.
  Tests for flex credit set `is_flex_credit: true` on the `BadgeEntry` directly.

- **`calculate_quarter_stats` signature:** takes `(quarter, badge, holiday, vacation, today: Option<NaiveDate>)`.
  Pass `None` for `today` in production (uses `Local::now()`); pass `Some(date)` in tests for
  deterministic results.

- **No terminal in tests.** `App::new()` accepts `today: NaiveDate` and `data_dir: PathBuf`
  for dependency injection. `run_app()` requires a real terminal and is not unit-tested.
  All pure helpers (`calendar_day_style`, `search_events`, `add_months`, `days_in_month`,
  `month_name`) are `pub(crate)` and have standalone unit tests.

- **`make_stats` test helper in `cmd/stats.rs`** must be kept current whenever `QuarterStats`
  gains new fields. It constructs a minimal valid `QuarterStats` for output format tests.

- **`serde_norway`** is used for all YAML (not `serde_yaml`). Import as `serde_norway::to_string`
  / `serde_norway::from_str`. Do not add `serde_yaml` as a dependency.

- **`data/init.rs`** (if it exists) is dead code from an earlier draft — do not use it.
  The canonical init implementation is `cmd/init.rs`.

---

## Common Pitfalls

- **NLL borrow conflict in `root.rs`:** After `run_app()`, `app` still holds `&mut` borrows.
  Extract `app.settings.clone()` and `drop(app)` before calling `badge_data.save()` etc.

- **`DATA_DIR` OnceLock cannot be reset.** Integration tests that need different data
  directories must use `load_from(dir)` / `save_to(dir)` / `run_in_dir(dir)` rather than
  `load()` / `save()` / `run()`.

- **Quarter navigation past configured range** returns `current_quarter = None` and
  `quarter_stats = None`, but `nav_date` is always updated — this is intentional and allows
  recovery via the opposite key.

- **Year stats require at least one quarter** for the current `nav_date` year. If no quarters
  match the year string, `year_stats` is `None` and the year stats table renders empty.

- **Settings save path:** Settings are written by `cmd::init::save_settings_to()`, which
  re-reads the current quarters before writing. This prevents overwriting the `quarters` key
  when only settings changed. Do not write `config.yaml` using `AppSettings::save()` directly.
