# rto — Return-to-Office Tracker

`rto` is an interactive terminal application written in [Rust](https://www.rust-lang.org/) for tracking daily office badge-ins and calculating compliance with Return-to-Office (RTO) attendance policies. It provides a full-featured TUI with color-coded calendars, real-time statistics, what-if simulation, and one-key git backup — all driven from plain YAML and JSON files you own and control.

---

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [TUI Overview](#tui-overview)
- [TUI Key Bindings](#tui-key-bindings)
- [Data Directory & File Formats](#data-directory--file-formats)
- [Time Period Views](#time-period-views)
- [Compliance Calculation](#compliance-calculation)
- [What-If Mode](#what-if-mode)
- [Git Backup](#git-backup)
- [CLI Commands](#cli-commands)
- [Architecture](#architecture)
- [Running Tests](#running-tests)
- [Dependencies](#dependencies)
- [Contributing](#contributing)
- [License](#license)

---

## Features

- **Interactive TUI** — A [ratatui](https://ratatui.rs)-powered calendar interface with [crossterm](https://github.com/crossterm-rs/crossterm) back-end. Navigate dates, toggle badge-ins, and manage events without leaving the terminal.
- **Configurable attendance goal** — Set any target percentage (default 50%). The required days are computed as `⌈total_days × goal / 100⌉`.
- **Multiple time period views** — Define quarterly, half-year, or full-year period files and cycle between them at runtime with a single keypress.
- **What-if mode** — Simulate future badge-ins to see how they affect your statistics, then discard the changes when you're done exploring.
- **Git backup** — Commit and optionally push your data directory to a git remote with one key (`g`) from the TUI, or via `rto backup` on the command line.
- **Vacation & holiday management** — Add, edit, and delete entries directly in the TUI. Vacation and holiday days are automatically excluded from attendance calculations.
- **Calendar events** — Annotate any date with a free-text note. Event days are highlighted on the calendar.
- **Year-level statistics** — Aggregate stats spanning all periods in the current year, displayed alongside per-period stats.
- **Pace tracking & projections** — See whether you're ahead of or behind pace, how many days you can still miss, and an estimated completion date.
- **Flex credit support** — Track alternative attendance (e.g., work-from-home credits) distinctly from in-office badge-ins.
- **CLI commands** — Print statistics, list vacations and holidays, run backups, and initialize data — all without launching the TUI.
- **Auto-initialization** — On first run, `rto` detects a missing data directory and creates one with sensible defaults.
- **Shared data format** — Reads and writes the same YAML/JSON files as the [Go implementation](https://github.com/your-user/rto-go), so both versions can be used interchangeably on the same data directory.

---

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.85+ (edition 2024)
- [Git](https://git-scm.com/) (required only for the backup feature)

### Build from source

```bash
git clone <repo-url>
cd rto-rust
cargo build --release
```

The binary is produced at `target/release/rto`.

### Install via cargo

```bash
cargo install --path .
```

This places the `rto` binary in `~/.cargo/bin/`. Make sure that directory is on your `PATH`.

---

## Quick Start

```bash
# 1. Initialize a data directory with sample files
rto init

# 2. Launch the interactive TUI
rto

# 3. Use a custom data directory
rto --data-dir ~/my-rto-data

# 4. Print stats for the current time period
rto stats

# 5. Print stats for a specific period (defined in your yaml config)
rto stats Q1_2025

# 6. List holidays
rto holidays

# 7. List vacations
rto vacations

# 8. Backup data to git
rto backup --remote https://github.com/your-user/rto-data.git
```

If you run `rto` (or any subcommand other than `init`) before initializing, the tool automatically detects the missing data directory and runs `rto init` for you.

---

## TUI Overview

The TUI uses a two-panel horizontal layout:

```
┌──────────────────────────────┬──────────────────────────────────────────────┐
│     January 2025             │┌ Period Stats: 2025-Q1 ─────────────────────┐│
│  Su Mo Tu We Th Fr Sa        ││  STATUS                                    ││
│            1  2  3  4        ││    Status              On Track             ││
│   5  6  7  8  9 10 11        ││    Days Ahead of Pace  +3 days ahead       ││
│  12 13 14 15 16 17 18        ││    Skippable Days      12                  ││
│  19 20 21 22 23 24 25        ││  PROGRESS                                  ││
│  26 27 28 29 30 31           ││    Total Days          90                  ││
│                              ││    Goal (50% Required) 23 / 45   51.1%     ││
│     February 2025            ││    Office Days         15 / 23   65.2%     ││
│  Su Mo Tu We Th Fr Sa        ││    ...                                     ││
│                     1        │└────────────────────────────────────────────┘│
│   2  3  4  5  6  7  8        │┌ Year Stats: 2025 ──────────────────────────┐│
│  ...                         ││    Total Calendar Days  365                ││
│                              ││    ...                                     ││
├──────────────────────────────│└────────────────────────────────────────────┘│
│ Events for Mon Mar 3, 2025:  │                                              │
│   (none)                     │                                              │
│ [space/shift+←→] fy-qtr.yaml│                                              │
│ [←→↑↓] Navigate  [b] Office │                                              │
│ [n/p] Next/Prev  [s] Search │                                              │
│ Data: ~/rto-data             │                                              │
│   Git: clean (remote: origin)│                                              │
└──────────────────────────────┴──────────────────────────────────────────────┘
```

- **Left panel** — Dynamic multi-month calendar (columns adjust per time period file) and the events/key legend section below.
- **Right panel** — Period statistics and year statistics, each in a bordered table with bold white borders.
- **Stats tables** — Section headers (STATUS, PROGRESS) are bold. Compliance status is color-coded: green for Achieved/On Track, orange for At Risk, red for Impossible.

---

## TUI Key Bindings

### Calendar View (default)

| Key | Action |
|---|---|
| `← → ↑ ↓` | Navigate by day (left/right) or week (up/down) |
| `Space` | Cycle to the next time period view |
| `Shift+→` | Cycle to the next time period view |
| `Shift+←` | Cycle to the previous time period view |
| `b` | Toggle office badge-in on the selected date |
| `f` | Toggle flex credit on the selected date |
| `n` | Jump to the next time period |
| `p` | Jump to the previous time period |
| `a` | Add an event (free-text note) to the selected date |
| `d` | Delete an event from the selected date |
| `s` | Search events |
| `w` | Enter / exit what-if mode |
| `g` | Git backup |
| `v` | Switch to vacations view |
| `h` | Switch to holidays view |
| `o` | Switch to settings view |
| `q` | Quit (exits what-if first if active) |
| `Ctrl+C` | Force quit |

### Vacations / Holidays Views

| Key | Action |
|---|---|
| `↑ / ↓` | Select an entry |
| `a` | Add a new entry |
| `e` or `Enter` | Edit the selected entry |
| `Delete` or `x` | Delete the selected entry |
| `q` | Return to the calendar view |

In add/edit forms, use `Tab` to move between fields, `Enter` to save, and `Esc` to cancel.

### Settings View

| Key | Action |
|---|---|
| `↑ / ↓` | Select a setting |
| `e` or `Enter` | Edit the selected value |
| `Esc` | Cancel editing |
| `q` | Return to the calendar view |

### Calendar Color Legend

| Color | Meaning |
|---|---|
| **Red (bold)** | Badged in (office day) |
| **Orange (bold)** | Flex credit day |
| **Green** | Holiday or vacation day |
| **Yellow** | Date has an event/note |
| **Dim gray** | Weekend day |
| **Underlined** | Today's date |
| **Reversed** | Currently selected date |

### Status Colors

| Status | Color |
|---|---|
| **Achieved** | Bright green (bold) |
| **On Track** | Green |
| **At Risk** | Orange |
| **Impossible** | Red (bold) |

---

## Data Directory & File Formats

All data is stored in plain text files within a single directory. The default location is `./config/`, but you can override it with `--data-dir` or the `-d` flag.

All YAML files written by `rto` use consistent double-quoting for string values. This avoids ambiguity with date-like strings and ensures compatibility across YAML parsers. The files are fully compatible with the Go implementation, which can read both quoted and unquoted formats.

### File Overview

| File | Format | Description |
|---|---|---|
| `settings.yaml` | YAML | Application settings and list of time period files |
| `*.yaml` (time periods) | YAML | One or more time period definition files |
| `badge_data.json` | JSON | Badge-in entries |
| `holidays.yaml` | YAML | Holiday definitions |
| `vacations.yaml` | YAML | Vacation periods |
| `events.json` | JSON | Free-text calendar events |

### settings.yaml

Controls application behavior and references the time period files to use.

```yaml
default_office: "McLean, VA"
flex_credit: "Flex Credit"
goal: 50
time_periods:
- "workday-fy-qtr.yaml"
- "workday-fy-halfyear.yaml"
- "calendar-qtr.yaml"
```

| Field | Type | Default | Description |
|---|---|---|---|
| `default_office` | string | `"McLean, VA"` | Label for office badge-ins |
| `flex_credit` | string | `"Flex Credit"` | Label for flex/WFH credits |
| `goal` | integer | `50` | Attendance goal as a percentage |
| `time_periods` | list | `["workday-fiscal-quarters.yaml"]` | Ordered list of time period YAML files. The first entry is the default view at startup. |

### Time Period Files

Each file defines a set of date ranges and how many calendar columns to display. You can create as many of these as you like — quarterly, half-year, full-year, fiscal vs. calendar, etc.

```yaml
calendar_display_columns: 3
timeperiods:
- key: "Q1_2025"
  name: "Q1"
  start_date: "2025-01-01"
  end_date: "2025-03-31"
- key: "Q2_2025"
  name: "Q2"
  start_date: "2025-04-01"
  end_date: "2025-06-30"
```

| Field | Type | Default | Description |
|---|---|---|---|
| `calendar_display_columns` | integer | `3` | Number of month columns in the calendar grid (e.g., `3` for quarters, `4` or `6` for half-years) |
| `timeperiods[].key` | string | — | Unique identifier (e.g., `Q1_2025`, `2025-H1`) |
| `timeperiods[].name` | string | — | Display label (e.g., `Q1`, `2025`) |
| `timeperiods[].start_date` | string | — | Period start in `YYYY-MM-DD` format |
| `timeperiods[].end_date` | string | — | Period end in `YYYY-MM-DD` format |

### badge_data.json

Stores badge-in events. Managed automatically by the TUI when you press `b` or `f`.

```json
{
  "badge_data": [
    {
      "entry_date": "2025-01-06",
      "date_time": "2025-01-06T09:00:00",
      "office": "McLean, VA",
      "is_badged_in": true,
      "is_flex_credit": false
    }
  ]
}
```

### holidays.yaml

```yaml
holidays:
- name: "New Year's Day"
  date: "2025-01-01"
- name: "MLK Day"
  date: "2025-01-20"
```

### vacations.yaml

```yaml
vacations:
- destination: "Beach Trip"
  start_date: "2025-07-04"
  end_date: "2025-07-11"
  approved: true
```

### events.json

```json
{
  "events": [
    {
      "date": "2025-03-15",
      "description": "Team offsite"
    }
  ]
}
```

---

## Time Period Views

One of `rto`'s distinguishing features is support for multiple time period configurations. This lets you view the same badge data through different lenses — fiscal quarters, calendar quarters, half-years, or full years — without duplicating anything.

### Setup

1. Create one YAML file per view in your data directory (e.g., `workday-fy-qtr.yaml`, `calendar-halfyear.yaml`, `calendar-year.yaml`).
2. List them in `settings.yaml` under `time_periods`. The first entry is the default view.
3. Each file specifies its own `calendar_display_columns` so the calendar grid adjusts automatically.

### Switching views at runtime

| Key | Direction |
|---|---|
| `Space` | Next view |
| `Shift+→` | Next view |
| `Shift+←` | Previous view |

The active view filename and position (e.g., `workday-fy-qtr.yaml (1 of 6)`) are shown in the key legend below the calendar.

---

## Compliance Calculation

Statistics are computed per time period and aggregated for the full year.

### Key metrics

| Metric | Formula |
|---|---|
| **Available workdays** | All weekdays (Mon–Fri) in the period |
| **Total days** | Available workdays minus holidays and vacation days |
| **Days required** | `⌈total_days × goal% / 100⌉` |
| **Days still needed** | `max(0, days_required − days_badged_in)` |
| **Days ahead of pace** | `days_badged_in − round(days_thus_far × days_required / total_days)` |
| **Remaining missable days** | `days_left − days_still_needed` |
| **Current average** | `days_badged_in / days_thus_far` |
| **Required future average** | `days_still_needed / days_left` |

### Compliance statuses

| Status | Condition | Color |
|---|---|---|
| **Achieved** | You've already met the required number of days | Bright green (bold) |
| **On Track** | Ahead of or at expected pace | Green |
| **At Risk** | Behind pace but mathematically achievable | Orange |
| **Impossible** | Cannot reach the requirement even if you badge in every remaining day | Red (bold) |

### Projected completion

When you have an established badge-in rate and days still remaining, `rto` estimates the date you'll reach the requirement:

```
projected_date = today + ⌈days_still_needed / current_rate⌉
```

---

## What-If Mode

Press `w` to enter what-if mode. A banner appears at the top of the screen:

```
⚠ WHAT-IF MODE  (press w to exit, q to discard & quit)
```

While in what-if mode, you can toggle badge-ins and flex credits freely. The statistics update in real time to reflect your hypothetical changes. When you exit (`w` again), all simulated changes are discarded and your data is restored to its original state. No changes are written to disk.

---

## Git Backup

`rto` can back up your entire data directory to a git repository.

### From the TUI

Press `g` to run the backup. The status bar shows the result.

### From the command line

```bash
# Backup using the configured remote
rto backup

# Specify a remote URL (set once, remembered by git)
rto backup --remote https://github.com/user/rto-data.git

# Backup a specific directory
rto backup --dir ~/my-rto-data
```

### What the backup does

1. Initializes a git repo in the data directory if one doesn't exist
2. Configures or updates the `origin` remote if `--remote` is provided
3. Stages all files (`git add .`)
4. Commits with an auto-generated timestamp message (e.g., `backup: 2025-03-15-14-30-00-123`)
5. Pushes to `origin main` if a remote is configured

If there are no changes, the backup reports "Nothing to commit — backup up to date."

---

## CLI Commands

```
Usage:
  rto [flags]
  rto [command]

Available Commands:
  init        Initialize data files with defaults
  stats       Print statistics for a time period
  vacations   List all vacations
  holidays    List all holidays
  backup      Backup data directory to git
  help        Help about any command

Flags:
  -d, --data-dir <path>   Data directory (default: ./config)
  -h, --help              Help for rto
```

### rto

Launches the interactive TUI. Auto-initializes the data directory if `settings.yaml` is not found.

### rto init

Creates the data directory and populates it with default files: `settings.yaml`, `workday-fiscal-quarters.yaml`, `badge_data.json`, `holidays.yaml`, `vacations.yaml`, and `events.json`. Existing files are never overwritten.

### rto stats [PERIOD_KEY]

Prints compliance statistics for the given period key (e.g., `Q1_2025`). If no key is provided, uses the current date to determine the active period.

```
Period: Q1_2025  [Jan 1, 2025 – Mar 31, 2025]
Goal: 50% attendance required

STATUS
  Status:            On Track
  Days Ahead:        +3 days ahead
  Skippable Days:    12

PROGRESS
  Total Days:        90
  Working Days:      64
  Available Days:    58  (6 holidays, 0 vacation days)
  Goal (50%):        29 / 58  (50.0%)
  Office Days:       15 / 29  (51.7%)
    Badge-In Days:   13  (86.7%)
    Flex Credits:    2   (13.3%)
  Still Needed:      14 / 29  (48.3%)

Projected Completion: Mar 14, 2025
```

### rto vacations

Prints all vacation entries from `vacations.yaml`.

### rto holidays

Prints all holiday entries from `holidays.yaml`.

### rto backup [flags]

Runs the git backup workflow. Flags:
- `-r, --remote` — Git remote URL
- `--dir` — Directory to back up (defaults to the data directory)

---

## Architecture

```
rto-rust/
├── Cargo.toml                     Package manifest
├── Makefile                       Convenience targets: test, coverage, etc.
├── CONCEPTS.md                    Learning guide: concepts, libraries, resources
│
├── src/
│   ├── main.rs                    CLI entry point (clap), auto-init logic
│   │
│   ├── data/                      Data models and persistence (YAML/JSON I/O)
│   │   ├── mod.rs                 Module exports
│   │   ├── persistence.rs         Persistable trait, YAML normalization, global data dir
│   │   ├── app_settings.rs        AppSettings — goal, time_periods, office/flex labels
│   │   ├── time_period.rs         TimePeriod, TimePeriodData, file-level display columns
│   │   ├── badge_entry.rs         BadgeEntry with FlexTime (multi-format datetime parsing)
│   │   ├── holiday.rs             Holiday model
│   │   ├── vacation.rs            Vacation model with date-range expansion (weekdays only)
│   │   └── event.rs               Event model
│   │
│   ├── calc/                      Pure calculation functions (no I/O, no side effects)
│   │   ├── mod.rs                 Module exports
│   │   ├── workday.rs             Workday struct, create_workday_map, is_workday
│   │   └── quarter_calc.rs        calculate_quarter_stats, calculate_year_stats
│   │
│   ├── cmd/                       CLI command implementations
│   │   ├── mod.rs                 Module exports
│   │   ├── root.rs                Loads all data, starts TUI, saves all data on exit
│   │   ├── init.rs                rto init — non-destructive file creation
│   │   ├── stats.rs               rto stats — writes to impl Write for testability
│   │   ├── vacations.rs           rto vacations
│   │   ├── holidays.rs            rto holidays
│   │   └── backup.rs              rto backup — git init/add/commit/push
│   │
│   └── ui/                        Terminal UI
│       ├── mod.rs                 Terminal setup/teardown (raw mode, alternate screen)
│       └── calendar_view.rs       Full TUI: App struct, 4 views, rendering, key handling
```

### Design Principles

1. **Separation of concerns** — `data/` handles file I/O only. `calc/` contains pure functions with no I/O. `cmd/` wires CLI commands. `ui/` manages TUI state and rendering.

2. **Testability** — Output functions accept `&mut impl Write`. Calculation functions accept `Option<NaiveDate>` for deterministic "today" injection. Data functions accept a directory path so tests use `tempfile::TempDir`.

3. **Non-destructive initialization** — `rto init` checks for each file individually and only creates those that are missing. Running init against an existing data directory never overwrites user data.

4. **What-if isolation** — `BadgeEntryData.clone()` creates a deep copy on entry. The original reference is restored on exit so no simulated changes leak into saved data.

5. **Global data directory** — Set once via `persistence::set_data_dir()` before any load/save calls. `OnceLock` provides lock-free reads after initialization. Tests use `load_from()` / `save_to()` which bypass the global and take an explicit path.

6. **Consistent YAML output** — All YAML files written by `rto` pass through a normalization step that double-quotes every string value. This prevents ambiguity with date-like strings and produces clean, consistent output.

---

## Running Tests

```bash
# Run all tests (202 tests)
cargo test

# Run tests whose name contains a keyword
cargo test quarter_calc

# Show println! output during tests
cargo test -- --nocapture

# Run tests in a specific module
cargo test cmd::stats

# Lint
cargo clippy

# Format
cargo fmt

# Coverage summary (requires cargo-llvm-cov)
make coverage-summary

# HTML coverage report
make coverage-html
```

---

## Dependencies

| Crate | Purpose |
|---|---|
| [clap](https://docs.rs/clap) v4 | CLI argument parsing with derive macros |
| [ratatui](https://ratatui.rs) v0.30 | Terminal UI framework (immediate-mode rendering) |
| [crossterm](https://docs.rs/crossterm) v0.29 | Cross-platform terminal control (raw mode, key events) |
| [serde](https://serde.rs) v1 | Serialization/deserialization framework |
| [serde_json](https://docs.rs/serde_json) v1 | JSON format for badge data and events |
| [serde_norway](https://crates.io/crates/serde_norway) v0.9 | YAML format for settings, time periods, holidays, vacations |
| [chrono](https://docs.rs/chrono) v0.4 | Date and time handling (`NaiveDate`, `NaiveDateTime`) |
| [anyhow](https://docs.rs/anyhow) v1 | Ergonomic error handling with context |
| [tempfile](https://docs.rs/tempfile) v3 | Temporary directories for tests (dev dependency) |

---

## Contributing

Contributions are welcome. Here's how to get started:

1. **Fork and clone** the repository.

2. **Create a branch** for your feature or fix:
   ```bash
   git checkout -b my-feature
   ```

3. **Make your changes.** Follow the existing code conventions:
   - Keep `calc/` free of I/O — only pure functions.
   - Keep `data/` focused on models and persistence.
   - Add tests for new behavior. All modules have test sections.

4. **Run the full test suite** before submitting:
   ```bash
   cargo test
   cargo clippy
   cargo fmt -- --check
   ```

5. **Open a pull request** with a clear description of what changed and why.

### Project structure guidelines

- **Adding a new CLI command**: Define the subcommand variant in the `Commands` enum in `main.rs`, implement the logic in `cmd/`.
- **Adding a new data type**: Create a model file in `data/` following the pattern of `holiday.rs` or `vacation.rs` — an entity struct, a container struct, and persistence via the `Persistable` trait or the generic `load_yaml_from` / `save_yaml_to` helpers.
- **Adding a new TUI view**: Add a `ViewState` variant in `calendar_view.rs`, a `render_*` function, and a `handle_*_key` branch in `handle_key()`.
- **Modifying calculations**: All math lives in `calc/quarter_calc.rs`. Update the corresponding test cases.

---

## License

See [LICENSE](LICENSE) for details.
