# rto-rust — Return to Office Tracker

A terminal-based Return-to-Office (RTO) compliance tracker written in Rust. It reads badge
entries, holidays, and vacations from a configurable data directory, calculates your progress
toward the 50% attendance requirement, and renders an interactive full-screen TUI with live
statistics, multiple views, and a what-if planning mode.

---

## Quick Start

```bash
# Build the binary
cargo build --release

# Seed the data directory with sample data (auto-runs if the directory is missing)
./target/release/rustrto --data-dir ./config init

# Open the interactive TUI
./target/release/rustrto --data-dir ./config

# Print text stats for a specific quarter
./target/release/rustrto --data-dir ./config stats Q1_2026

# List all vacations
./target/release/rustrto --data-dir ./config vacations

# List all holidays
./target/release/rustrto --data-dir ./config holidays
```

> **Auto-init:** If `--data-dir` points to a missing or empty directory and you did not run
> `init` explicitly, the binary auto-initializes the directory with sample data before
> continuing.

---

## Table of Contents

1. [CLI Subcommands](#1-cli-subcommands)
2. [TUI Views and Key Bindings](#2-tui-views-and-key-bindings)
3. [Statistics and Metrics](#3-statistics-and-metrics)
4. [Project Layout](#4-project-layout)
5. [Data Files](#5-data-files)
6. [Crates (Dependencies)](#6-crates-dependencies)
7. [Rust Concepts — C# Mapping](#7-rust-concepts--c-mapping)
8. [Architecture Walkthrough](#8-architecture-walkthrough)
9. [Design Decisions](#9-design-decisions)
10. [How to Modify](#10-how-to-modify)
11. [Running Tests](#11-running-tests)
12. [Learning Resources](#12-learning-resources)

---

## 1. CLI Subcommands

All subcommands accept `--data-dir <path>` (default: `./config`).

| Subcommand | Description |
|------------|-------------|
| *(none)* | Launch the interactive TUI |
| `init` | Seed the data directory with sample config, holidays, vacations, and badge data |
| `stats <QUARTER_KEY>` | Print formatted statistics for one quarter to stdout (e.g. `stats Q1_2026`) |
| `vacations` | Print all configured vacations to stdout |
| `holidays` | Print all configured holidays to stdout |
| `backup` | Git commit + push the data directory (same as pressing `g` in the TUI) |

### Example output: `stats Q1_2026`

```
Quarter Stats for Q1_2026
Range: [2026-02-01 - 2026-04-30]
---
Status:                    On Track
Days Ahead of Pace:        3 days ahead
Skippable Days Left:       8
---
Goal (50% Required):       (28 / 56)  = 50.00%
Badged In:                 23
Still Needed:              5
Rate So Far:               (23 / 17)  = 135.29%
---
Rate Needed (Remaining):   (5 / 39)  = 12.82%
Projected Completion:      2026-03-15
---
Holidays:                  4
Vacation Days:             2
Total Days Off:            6
---
Office Days:               18
Flex Credits:              5
Total Badged In:           23
---
```

### Example output: `vacations`

```
Vacations
---
  #    Destination              Start          End            Approved
  1    Hawaii                   2025-05-10     2025-05-17     Yes
---
Total: 1 vacation(s)
```

### Example output: `holidays`

```
Holidays
---
  Date           Name
  2025-01-01     New Year's Day
  2025-01-20     Martin Luther King Jr. Day
  ...
---
Total: 32 holiday(s)
```

---

## 2. TUI Views and Key Bindings

The TUI has four full-screen views. All views dispatch key events independently; pressing
`Esc` or `q` in any non-calendar view returns to the Calendar view.

### Calendar View (default)

The main view: a three-month calendar on top, quarter statistics table below, and events/help
at the bottom.

#### Calendar View — Normal mode

| Key | Action |
|-----|--------|
| `←` `→` | Move selected date by one day |
| `↑` `↓` | Move selected date by one week |
| `n` | Advance to the next quarter (wraps `nav_date` forward; navigates past configured quarters safely) |
| `p` | Go back to the previous quarter |
| `Space` | Toggle office badge-in for the selected date |
| `f` | Toggle flex-credit badge for the selected date |
| `w` | Enter / exit What-If mode (changes are discarded when you exit What-If) |
| `g` | Git commit + push the data directory |
| `a` | Enter Add mode — type a note/event and press Enter to save |
| `d` | Enter Delete mode — confirm deletion of the event on the selected date |
| `s` | Enter Search mode — type to filter events by text |
| `v` | Switch to Vacations view |
| `h` | Switch to Holidays view |
| `o` | Switch to Settings view |
| `q` / `Ctrl-C` | Quit — saves all data (badge entries, events, vacations, holidays, settings) |

#### Calendar View — Add / Delete / Search modes

| Key | Action |
|-----|--------|
| *(any char)* | Append to input buffer |
| `Backspace` | Delete last character |
| `Enter` | Confirm (save event / confirm delete / apply search filter) |
| `Esc` | Cancel and return to Normal mode |

#### Calendar color coding

| Color | Meaning |
|-------|---------|
| Green | Badged in (office day) |
| Orange | Flex credit day |
| Red | Today (no badge) |
| Yellow | Selected date |
| Blue | Vacation day |
| Gray | Holiday |
| White | Future workday |
| Dark gray | Weekend |

---

### Vacations View (`v`)

Full-screen table of all configured vacations. Supports browsing, adding, editing, and deleting.

| Key | Action |
|-----|--------|
| `↑` `↓` | Move cursor through the list |
| `a` | Start adding a new vacation (prompts for 4 fields sequentially) |
| `Enter` / `e` | Edit the selected vacation |
| `Delete` / `x` | Delete the selected vacation |
| `Esc` / `q` | Return to Calendar view |

**Add/Edit fields (entered one at a time, confirmed with Enter):**
1. Destination name (free text)
2. Start date (`YYYY-MM-DD`)
3. End date (`YYYY-MM-DD`)
4. Approved? (`y` / `n`)

Date fields are validated; invalid dates show an inline error and stay on the same field.
Vacation changes are saved when you quit the TUI.

---

### Holidays View (`h`)

Full-screen table of all configured holidays. Supports browsing, adding, and deleting.

| Key | Action |
|-----|--------|
| `↑` `↓` | Move cursor through the list |
| `a` | Start adding a new holiday (prompts for 2 fields sequentially) |
| `Enter` / `e` | Edit the selected holiday |
| `Delete` / `x` | Delete the selected holiday |
| `Esc` / `q` | Return to Calendar view |

**Add/Edit fields:**
1. Date (`YYYY-MM-DD`)
2. Name (free text)

Holiday changes are saved when you quit the TUI.

---

### Settings View (`o`)

Full-screen table showing the two configurable application settings.

| Key | Action |
|-----|--------|
| `↑` `↓` | Move cursor to select a setting |
| `Enter` / `e` | Edit the selected setting (pre-fills the current value) |
| `Esc` / `q` | Return to Calendar view |

**Settings:**

| Setting | Description |
|---------|-------------|
| `default_office` | The office name stamped into badge entries when you press `Space` (e.g. `"McLean, VA"`) |
| `flex_credit` | The label displayed for flex-credit entries (e.g. `"Flex Credit"`). This is a display label only — whether an entry counts as flex credit is determined by the `is_flex_credit` boolean field on each `BadgeEntry`, not by string comparison. |

Settings are persisted to `config.yaml` when you quit the TUI.

---

## 3. Statistics and Metrics

The quarter statistics table is divided into five sections. The year statistics table below it
shows aggregate totals for the entire fiscal year (Q1 start → Q4 end).

### STATUS section

| Metric | Description |
|--------|-------------|
| Status | `Achieved`, `On Track`, `At Risk`, or `Impossible` |
| Days Ahead of Pace | Positive = ahead; negative = behind; 0 = on pace |
| Skippable Days Left | Days you can still miss and achieve 50% |

### PROGRESS section

| Metric | Description |
|--------|-------------|
| Total Days in Qtr | All calendar days (weekdays + weekends) in the quarter range |
| Total Working Days | Weekdays in the quarter, **excluding holidays** (vacation days count as working days) |
| Available Working Days | Weekdays in the quarter, **excluding both holidays and vacation days** |
| Goal (50% Required) | `ceil(Available Working Days / 2)` — the minimum days you must be badged in |
| Badged In Days | Total days with a badge entry (office + flex credits combined) |
| Still Needed | `Goal − Badged In` (0 if already achieved) |
| Rate So Far | `(Badged In) / (days_thus_far)` — your current attendance rate |

### BADGE BREAKDOWN section

| Metric | Description |
|--------|-------------|
| Office Days | Badge entries where `is_flex_credit = false` |
| Flex Credits | Badge entries where `is_flex_credit = true` |
| Total Badged In | Office Days + Flex Credits |

### LOOKING AHEAD section

| Metric | Description |
|--------|-------------|
| Rate Needed (Remaining) | `Still Needed / Days Left` — the rate you need going forward |
| Projected Completion | Estimated date you will hit 50% at your current rate |

### DAYS OFF section

| Metric | Description |
|--------|-------------|
| Holidays | Company holidays within the quarter |
| Vacation Days | Approved vacation days within the quarter |
| Total Days Off | Holidays + Vacation Days |

### Year Stats table

Shows aggregate totals from the first day of Q1 through the last day of Q4 for the currently
viewed year:

| Row | Description |
|-----|-------------|
| Total Calendar Days | All calendar days in the year range |
| Total Working Days | Weekdays excluding holidays |
| Available Working Days | Weekdays excluding holidays and vacations |
| Holidays | Total holidays in the year |
| Vacation Days | Total vacation days in the year |
| Office Days | Total non-flex badge entries |
| Flex Credits | Total flex-credit badge entries |
| Total Badged In | Office Days + Flex Credits |

---

## 4. Project Layout

```
rustrto/
├── Cargo.toml              # Package manifest (like a .csproj)
├── Makefile                # Convenience targets: test, coverage, etc.
├── config/                 # Default data directory
│   ├── config.yaml         # Quarter definitions + app settings
│   ├── holidays.yaml       # Company holidays
│   ├── vacations.yaml      # Approved vacations
│   ├── badge_data.json     # Badge-in records (written by the TUI on quit)
│   └── events.json         # Notes/events (written by the TUI on quit)
└── src/
    ├── main.rs             # CLI entry point (clap), auto-init logic
    ├── calc/
    │   ├── mod.rs
    │   ├── workday.rs      # Weekday/weekend detection + workday map builder
    │   └── quarter_calc.rs # All RTO statistics formulas (QuarterStats struct)
    ├── cmd/
    │   ├── mod.rs
    │   ├── root.rs         # Launches the TUI; loads + saves all data
    │   ├── init.rs         # Seeds the data directory with sample data
    │   ├── stats.rs        # `rustrto stats <KEY>` — prints stats to stdout
    │   ├── backup.rs       # `rustrto backup` — git init/add/commit/push
    │   ├── vacations.rs    # `rustrto vacations` — prints vacation list
    │   └── holidays.rs     # `rustrto holidays` — prints holiday list
    ├── data/
    │   ├── mod.rs          # Re-exports + public API surface
    │   ├── persistence.rs  # Persistable trait + global data-dir (OnceLock)
    │   ├── app_settings.rs # AppSettings — default_office + flex_credit label
    │   ├── badge_entry.rs  # BadgeEntry (is_badged_in, is_flex_credit booleans)
    │   ├── event.rs        # Event / EventData — date-keyed notes
    │   ├── holiday.rs      # Holiday / HolidayData
    │   ├── vacation.rs     # Vacation / VacationData
    │   └── quarter.rs      # QuarterConfig / QuarterData
    └── ui/
        ├── mod.rs          # Terminal setup / teardown helpers
        └── calendar_view.rs # Full TUI: App struct, 4 views, rendering, key handling
```

> **C# analogy:** Each `src/` subdirectory is roughly a "project" or namespace. Rust uses
> *modules* (`mod`) rather than separate assemblies, so everything compiles into a single binary.

---

## 5. Data Files

| File | Format | Written by | Purpose |
|------|--------|-----------|---------|
| `config.yaml` | YAML | `init` / Settings view / TUI quit | Quarter date ranges + `settings` block |
| `holidays.yaml` | YAML | `init` / Holidays TUI view | Company holidays (name + date) |
| `vacations.yaml` | YAML | `init` / Vacations TUI view | Vacation ranges (destination, start, end, approved) |
| `badge_data.json` | JSON | TUI on quit | Badge-in records with office and boolean flags |
| `events.json` | JSON | TUI on quit | Date-keyed notes/events |

All files live in the directory specified by `--data-dir`. The directory can be a git
repository; pressing `g` in the TUI (or running `rustrto backup`) commits and pushes it.

### `config.yaml` structure

```yaml
settings:
  default_office: "McLean, VA"
  flex_credit: "Flex Credit"

quarters:
  - key: Q1_2026
    quarter: Q1
    year: "2026"
    start_date: "2026-02-01"
    end_date: "2026-04-30"
  # ... more quarters
```

Both `settings` and `quarters` live in the same file but are deserialized independently by
two different structs — serde silently ignores keys it does not recognize.

### `badge_data.json` structure

```json
{
  "data": [
    {
      "entry_date": "2026-02-13",
      "date_time": "2026-02-13T00:00:00",
      "office": "McLean, VA",
      "is_badged_in": true,
      "is_flex_credit": false
    }
  ]
}
```

- `is_badged_in` defaults to `true` for entries written before this field existed
- `is_flex_credit` defaults to `false` for entries written before this field existed
- The `office` field is purely a display label; flex credit is determined by `is_flex_credit`

### `vacations.yaml` structure

```yaml
vacations:
  - destination: "Hawaii"
    start_date: "2025-05-10"
    end_date: "2025-05-17"
    approved: true
```

### `holidays.yaml` structure

```yaml
holidays:
  - name: "New Year's Day"
    date: "2025-01-01"
```

---

## 6. Crates (Dependencies)

> **C# analogy:** A *crate* is a NuGet package. `Cargo.toml` is your `packages.config` /
> `<PackageReference>` section.

### `clap` v4 — CLI argument parsing
**[docs.rs/clap](https://docs.rs/clap/latest/clap/) · [github.com/clap-rs/clap](https://github.com/clap-rs/clap) · [derive tutorial](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html)**

Rust's standard CLI parser. The `derive` feature lets you annotate a struct and clap generates
all the parsing, `--help`, and validation automatically.

```rust
// C# equivalent: System.CommandLine or Cocona
#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "./config")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Stats { quarter_key: String },
    Vacations,
    Holidays,
}
```

`#[derive(Parser)]` is a **procedural macro** — it runs at compile time and generates the
`parse()` implementation. Think of it as a source generator in C#.

---

### `ratatui` v0.30 — Terminal UI framework
**[docs.rs/ratatui](https://docs.rs/ratatui/latest/ratatui/) · [ratatui.rs](https://ratatui.rs) · [book/tutorials](https://ratatui.rs/tutorials/) · [examples](https://github.com/ratatui/ratatui/tree/main/examples)**

Ratatui renders widgets (tables, paragraphs, charts) into a terminal using an
*immediate-mode render loop*:

```
loop {
    terminal.draw(|frame| app.render(frame))?;   // redraw entire screen each frame
    if key_pressed {
        if app.handle_key(key) { break; }        // mutate state
    }
}
```

Every call to `terminal.draw()` redraws the entire screen from scratch into a diff buffer
(like React's virtual DOM for terminals). There is no widget tree that persists between frames.

The layout system uses `Constraint::Length(N)`, `Constraint::Min(N)`, and `Constraint::Ratio`
to divide screen area — similar to CSS flexbox but operating on character cells.

This project's layout (top to bottom):
```
┌─────────────────────────────────────────┐
│  Calendar (3 months side by side)       │  Length(9)
├─────────────────────────────────────────┤
│  Quarter Stats table                    │  Length(30)
├─────────────────────────────────────────┤
│  Year Stats table                       │  Length(11)
├─────────────────────────────────────────┤
│  Events + Key Bindings help             │  Min(12)
└─────────────────────────────────────────┘
```

> **C# analogy:** WinForms/WPF draw on a `Graphics` surface; ratatui draws on a `Frame`.
> The approach is closest to a game loop — state drives rendering, not event handlers.

**Key ratatui concepts used in this project:**
- `Table` + `TableState` — scrollable, stateful tables (stats, vacation/holiday lists)
- `Paragraph` — plain text blocks (calendar cells, hints)
- `Block` — border/title wrapper for any widget
- `Layout::vertical` + `Constraint` — divide screen into zones
- `Style` + `Color` + `Modifier::BOLD` — text styling
- `Span` + `Line` + `Text` — composable styled text primitives

---

### `crossterm` v0.29 — Cross-platform terminal control
**[docs.rs/crossterm](https://docs.rs/crossterm/latest/crossterm/) · [github.com/crossterm-rs/crossterm](https://github.com/crossterm-rs/crossterm)**

Handles the low-level work: enabling raw mode, capturing keystrokes, switching to the
alternate screen buffer. Ratatui uses crossterm as its back-end.

```rust
enable_raw_mode()?;                               // Stop terminal echoing keypresses
execute!(stdout, EnterAlternateScreen)?;          // Switch to full-screen overlay
// ... run the app ...
disable_raw_mode()?;
execute!(stdout, LeaveAlternateScreen)?;
```

A panic hook in `root.rs` calls `disable_raw_mode()` before the panic message prints,
ensuring the terminal is always restored even on crashes.

---

### `serde` v1 — Serialization framework
**[docs.rs/serde](https://docs.rs/serde/latest/serde/) · [serde.rs](https://serde.rs) · [serde attributes reference](https://serde.rs/attributes.html)**

Serde is a *framework*, not a format library. It defines `Serialize` and `Deserialize` traits;
separate crates implement actual formats. The `derive` feature generates implementations:

```rust
// C# equivalent: [JsonSerializable] + System.Text.Json source generators
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BadgeEntry {
    #[serde(rename = "entry_date")]     // [JsonPropertyName("entry_date")]
    pub key: String,
    pub office: String,
    #[serde(default = "default_true")] // deserialize as true if key is absent
    pub is_badged_in: bool,
    #[serde(default)]                   // deserialize as false if key is absent
    pub is_flex_credit: bool,
    #[serde(skip)]                      // never serialize/deserialize this field
    pub parsed_date: Option<NaiveDate>,
}
fn default_true() -> bool { true }
```

Serde attributes used in this project:
- `#[serde(rename = "...")]` — map a different JSON/YAML key to a field name
- `#[serde(skip)]` — exclude from serialization (parsed date fields computed after load)
- `#[serde(default)]` — use `Default::default()` when the key is missing
- `#[serde(default = "fn_name")]` — call a function when the key is missing

---

### `serde_json` v1 — JSON format for serde
**[docs.rs/serde_json](https://docs.rs/serde_json/latest/serde_json/) · [github.com/serde-rs/json](https://github.com/serde-rs/json)**

Used for `badge_data.json` and `events.json`:

```rust
let json = serde_json::to_string_pretty(&badge_data)?;
fs::write(path, json)?;

let loaded: BadgeEntryData = serde_json::from_str(&text)?;
```

---

### `serde_norway` v0.9.42 — YAML format for serde
**[docs.rs/serde_norway](https://docs.rs/serde_norway/latest/serde_norway/) · [crates.io/serde_norway](https://crates.io/crates/serde_norway)**

Used for `config.yaml`, `holidays.yaml`, and `vacations.yaml`. A fork of `serde_yaml` that
resolves long-standing soundness issues in the original crate:

```rust
let yaml = serde_norway::to_string(&config)?;
fs::write(dir.join("config.yaml"), yaml)?;

let loaded: QuarterData = serde_norway::from_str(&text)?;
```

---

### `chrono` v0.4 — Date and time
**[docs.rs/chrono](https://docs.rs/chrono/latest/chrono/) · [strftime format codes](https://docs.rs/chrono/latest/chrono/format/strftime/index.html)**

Provides `NaiveDate` (date without timezone) and `NaiveDateTime`. The "Naive" prefix means
no timezone offset is stored — dates are compared purely by calendar value, avoiding the
timezone arithmetic bugs common in languages that always store timezone info.

```rust
// C#: DateTime.Parse("2026-02-01")
let date = NaiveDate::parse_from_str("2026-02-01", "%Y-%m-%d")?;

// C#: date.DayOfWeek == DayOfWeek.Saturday
let is_weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);

// C#: date.ToString("yyyy-MM-dd")
let key = date.format("%Y-%m-%d").to_string();

// Advance by one day
let tomorrow = date.succ_opt().unwrap();

// Duration arithmetic
let total_days = (end_date - start_date).num_days() as i32 + 1;
```

The `features = ["serde"]` Cargo feature enables automatic serialization of date types.

---

### `anyhow` v1 — Ergonomic error handling
**[docs.rs/anyhow](https://docs.rs/anyhow/latest/anyhow/) · [github.com/dtolnay/anyhow](https://github.com/dtolnay/anyhow)**

Provides a single `Error` type that wraps any error, with optional context messages. Every
`run()` function in `src/cmd/` returns `Result<()>` from anyhow.

```rust
// C#: throw new InvalidOperationException("failed to read file", inner);
let contents = fs::read_to_string(&path)
    .with_context(|| format!("failed to read {}", path.display()))?;
//                                                                   ^
//        If Err, attach context and return early — like C# throw

// C#: throw new ArgumentException("unknown quarter key");
bail!("Quarter key '{}' not found.", quarter_key);

// Functions return Result<()> — () is Rust's equivalent of void
fn run() -> Result<()> { ... }
```

---

### `tempfile` v3 — Temporary directories (dev dependency)
**[docs.rs/tempfile](https://docs.rs/tempfile/latest/tempfile/) · [github.com/Stebalien/tempfile](https://github.com/Stebalien/tempfile)**

Used in unit and integration tests to create temporary directories that are automatically
deleted when they go out of scope.

```rust
let tmp = TempDir::new().unwrap();
run_in_dir(tmp.path()).unwrap();  // writes files into tmp
assert!(tmp.path().join("config.yaml").exists());
// tmp is dropped here → directory deleted automatically
```

C# equivalent: `Path.GetTempPath()` + manual cleanup in `[TestCleanup]`.

---

## 7. Rust Concepts — C# Mapping

### Ownership and `&mut`

Rust tracks who *owns* each value at compile time. There is no garbage collector.

```rust
// C#: void Add(List<BadgeEntry> data, BadgeEntry entry)
// Rust:
fn add_entry(data: &mut BadgeEntryData, entry: BadgeEntry) {
//                  ^^^^                       ^
//           mutable borrow             owned value — moved in, cannot be used by caller
    data.data.push(entry);
}
```

- `&T` — shared reference (read-only), like `in T` in C#
- `&mut T` — exclusive mutable reference; the compiler guarantees nothing else holds a
  reference to the same data simultaneously
- No `&` — the value is *moved* (ownership transferred); the caller can no longer use it

### `Option<T>` and `Result<T, E>`

```rust
// C#: T? (nullable) or null
let quarter: Option<&QuarterConfig> = data.get_quarter_by_date(today);
if let Some(q) = quarter { /* use q */ }

// C#: (bool ok, T value) or try/catch
fn load() -> Result<Self> { ... }
let data = load()?;   // ? = propagate error up the call stack (like throw/await)
```

### Traits vs Interfaces

```rust
// C# interface with default method implementations:
// interface IPersistable { string Filename { get; } }

pub trait Persistable: Sized + Default + Serialize + for<'de> Deserialize<'de> {
    fn filename() -> &'static str;     // implementors must provide this
    fn is_json() -> bool;              // implementors must provide this
    fn load() -> Result<Self> { ... } // default implementation (like C# DIM)
    fn save(&self) -> Result<()> { ... }
}
```

The `: Sized + Default + Serialize + ...` are *trait bounds* — like `where T : ISerializable, new()`
in C#.

### `derive` Macros

```rust
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BadgeEntryData {
    pub data: Vec<BadgeEntry>,
}
```

`derive` is a compile-time code generator (like C# source generators):
- `Serialize`/`Deserialize` — serde implementations
- `Clone` — `.clone()` method (deep copy, like `ICloneable`)
- `Debug` — `{:?}` formatter for logging/debugging
- `Default` — `BadgeEntryData::default()` returning an empty struct (like `new()`)

### Pattern Matching

```rust
// C# switch expression:
// compliance_status switch { "Achieved" => Green, "On Track" => Cyan, _ => Red }

let color = match stats.compliance_status.as_str() {
    "Achieved" => Color::Green,
    "On Track" => Color::Cyan,
    "At Risk"  => Color::Yellow,
    _          => Color::Red,
};

// Destructuring with if let:
if let Some(date) = stats.projected_completion_date {
    writeln!(out, "Projected Completion: {}", date.format("%Y-%m-%d"))?;
}
```

### Lifetimes in Structs

The `App<'a>` struct holds *references* into data that lives in `root.rs`:

```rust
pub struct App<'a> {
    quarter_data:  &'a QuarterData,          // read-only borrow
    badge_data:    &'a mut BadgeEntryData,   // mutable borrow — can add/remove entries
    holiday_data:  &'a mut HolidayData,      // mutable — editable in Holidays view
    vacation_data: &'a mut VacationData,     // mutable — editable in Vacations view
    // ...
}
```

`'a` is a *lifetime parameter* — the compiler verifies `App` cannot outlive the data it
borrows. There is no C# equivalent; C# relies on the GC to prevent use-after-free.

### `OnceLock<T>` — Thread-safe singleton initialization

```rust
// C#: private static readonly Lazy<PathBuf> _dir = new(() => "./config");

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_data_dir(path: PathBuf) { let _ = DATA_DIR.set(path); }
pub fn get_data_dir() -> Result<PathBuf> {
    DATA_DIR.get().cloned().ok_or_else(|| anyhow!("data dir not set"))
}
```

`OnceLock<T>` allows external initialization, guarantees set-at-most-once, and allows
lock-free reads after initialization.

### `String` vs `&str`

```rust
// String  = owned, heap-allocated, growable — like C# string but mutable
// &str    = borrowed string slice (read-only) — like ReadOnlySpan<char>
// &'static str = string literal in the binary — like const string

fn filename() -> &'static str { "config.yaml" }   // literal, lives forever
let key: String = date.format("%Y-%m-%d").to_string(); // heap allocation
let slice: &str = &key;                                // borrow as read-only slice
```

---

## 8. Architecture Walkthrough

### 8.1 Data Layer (`src/data/`)

Every persistent type follows an identical pattern:

```
Entity struct   (e.g., BadgeEntry)
  └── Container struct  (e.g., BadgeEntryData)
        └── impl Persistable  → load() and save() for free
```

**`persistence.rs`** — The `Persistable` trait is the backbone:

```rust
pub trait Persistable: Sized + Default + Serialize + for<'de> Deserialize<'de> {
    fn filename() -> &'static str;  // "badge_data.json"
    fn is_json() -> bool;           // true → JSON, false → YAML

    // Default implementations use the global DATA_DIR OnceLock:
    fn load() -> Result<Self> { ... }
    fn save(&self) -> Result<()> { ... }

    // Testable variants that bypass OnceLock (pass an explicit directory):
    fn load_from(dir: &Path) -> Result<Self> { ... }
    fn save_to(&self, dir: &Path) -> Result<()> { ... }
}
```

To add a new data type, implement `filename()` and `is_json()` and the trait gives you
`load()` and `save()` automatically.

**`config.yaml` dual-deserialization** — `QuarterData` reads the `quarters` key;
`SettingsWrapper` reads the `settings` key. Both call `serde_norway::from_str()` on the same
file content; serde ignores keys it does not recognize. Writing uses a combined `ConfigFile`
struct to avoid each type overwriting the other's section.

**`BadgeEntry` booleans** — `is_badged_in` and `is_flex_credit` are `bool` fields with
`#[serde(default)]` / `#[serde(default = "default_true")]` so that entries in existing JSON
files (written before these fields existed) deserialize correctly.

### 8.2 Calculation Layer (`src/calc/`)

**`workday.rs`** — Builds a `HashMap<String, Workday>` covering every weekday in a date range.
Each `Workday` holds resolved boolean flags:

```rust
pub struct Workday {
    pub date: NaiveDate,
    pub is_workday: bool,       // weekday + not holiday + not vacation
    pub is_badged_in: bool,
    pub is_flex_credit: bool,   // subset of is_badged_in
    pub is_holiday: bool,
    pub is_vacation: bool,
}
```

The map is keyed on `"YYYY-MM-DD"` strings that match the keys in badge, holiday, and vacation
data — so joining them is a single `.get(&key)` lookup.

**`quarter_calc.rs`** — `calculate_quarter_stats()` is a pure function:

```
BadgeEntryData  ──┐
HolidayData     ──┤──► calculate_quarter_stats() ──► QuarterStats
VacationData    ──┤       (no side effects)
QuarterConfig   ──┘
```

Key formulas:
- `total_calendar_days = (end − start).num_days() + 1` — all calendar days including weekends
- `available_workdays` — weekdays where `!day.is_holiday` (vacation days included)
- `total_days` — weekdays where `!day.is_holiday && !day.is_vacation`
- `days_required = (total_days + 1) / 2` — integer ceiling of 50%
- `days_ahead_of_pace = days_badged_in − (days_required * days_thus_far / total_days)`

The returned `QuarterStats.workday_stats` contains the fully resolved `HashMap<String, Workday>`
so the calendar renderer can color each day without any additional business logic.

### 8.3 Command Layer (`src/cmd/`)

Each file exports a single `pub fn run()`:

| File | Triggered by | Description |
|------|-------------|-------------|
| `root.rs` | `rustrto` (no subcommand) | Loads all data, starts TUI, saves all data on exit |
| `init.rs` | `rustrto init` | Writes hardcoded sample data files |
| `stats.rs` | `rustrto stats <KEY>` | Prints formatted stats via testable `write_stats<W: Write>` |
| `vacations.rs` | `rustrto vacations` | Prints vacation list via testable `write_vacations<W: Write>` |
| `holidays.rs` | `rustrto holidays` | Prints holiday list via testable `write_holidays<W: Write>` |
| `backup.rs` | `rustrto backup` | Shells out to `git -C <dir> add/commit/push` |

All CLI output functions follow the `write_*<W: Write>(data, out: &mut W)` pattern so they
can be tested by passing `&mut Vec<u8>` instead of `stdout`.

**`root.rs` save order** — After `run_app()` returns, App is still holding `&mut` borrows on
all data structs. Settings are extracted with `.clone()` before `drop(app)`, then all data is
saved in order:

```rust
let final_settings = app.settings.clone();
drop(app);   // release &mut borrows

badge_data.save()?;
event_data.save()?;
vacation_data.save()?;
holiday_data.save()?;
cmd::init::save_settings_to(&final_settings, &data_dir)?;
```

### 8.4 UI Layer (`src/ui/calendar_view.rs`)

The largest file (~2,500 lines). Contains the `App<'a>` struct, all four views, and the
`run_app()` loop.

#### App state

```rust
pub struct App<'a> {
    // Data references (lifetimes enforced by compiler)
    quarter_data:  &'a QuarterData,
    badge_data:    &'a mut BadgeEntryData,
    holiday_data:  &'a mut HolidayData,
    vacation_data: &'a mut VacationData,
    event_data:    &'a mut EventData,

    // Navigation
    selected_date:    NaiveDate,    // highlighted calendar cell
    nav_date:         NaiveDate,    // quarter navigation position (independent of selected_date)
    current_quarter:  Option<&'a QuarterConfig>,

    // View state
    view_state:    ViewState,  // Calendar | Vacations | Holidays | Settings
    mode:          Mode,       // Normal | Add | Delete | Search

    // Stats (recomputed whenever badge data or navigation changes)
    quarter_stats: Option<QuarterStats>,
    year_stats:    Option<QuarterStats>,

    // List views (vacation/holiday/settings)
    list_cursor:     usize,
    list_add_stage:  u8,         // 0=browsing; 1-4=entering field N
    list_field_bufs: Vec<String>, // completed fields during add/edit

    // What-if mode
    what_if_mode:     bool,
    what_if_snapshot: Option<BadgeEntryData>,

    // Settings (persisted on quit)
    settings: AppSettings,

    // Misc
    input_buffer: String,
    today:        NaiveDate,
    data_dir:     PathBuf,
    git_status:   Option<(String, Color)>,
}
```

#### Navigation safety

The `nav_date: NaiveDate` field tracks quarter navigation independently of whether a
configured quarter is found. Pressing `n` / `p` always updates `nav_date`, so if the user
navigates past all configured quarters and gets a "Configuration Error" message, pressing
`p` / `n` in the opposite direction always returns them to valid quarters.

```rust
KeyCode::Char('n') => {
    let target = add_months(self.nav_date, 3);
    self.nav_date = target;
    self.current_quarter = self.quarter_data.get_quarter_by_date(target);
    self.update_stats();
}
```

#### What-if mode

When the user presses `w`, a snapshot of `badge_data` is cloned:

```rust
self.what_if_snapshot = Some(self.badge_data.clone());
self.what_if_mode = true;
```

All subsequent Space/`f` presses mutate the live data (so stats update in real time).
Exiting what-if restores the snapshot:

```rust
*self.badge_data = self.what_if_snapshot.take().unwrap();
```

Because `App` holds `&mut BadgeEntryData`, the dereference-assign replaces the caller's data
in place. When `root.rs` calls `badge_data.save()`, it saves the restored data — not the
what-if changes.

#### Git backup (`g` key)

Shells out to `git` using `std::process::Command`:

```rust
Command::new("git")
    .args(["-C", &dir, "add", "."])
    .stdout(Stdio::null())
    .status()?;
```

Commit message timestamp format: `"%Y-%m-%d-%H-%M-%S-%3f"` (millisecond precision ensures
unique messages in rapid succession).

---

## 9. Design Decisions

### Why `NaiveDate` instead of `DateTime` with timezone?

The original Go version used `time.Time` with local timezone, causing subtle bugs with
midnight comparisons (needing `endDate.Add(24 * time.Hour)` to make ranges inclusive).
`NaiveDate` has no timezone, so `date >= start && date <= end` is always correct.

### Why `HashMap<String, ...>` keyed on `"YYYY-MM-DD"`?

String keys are the simplest way to join badge, holiday, vacation, and workday data without
complex date-equality logic. All sources format dates identically, so a lookup is a single
`map.get(&key)` call. The overhead is negligible for a few hundred entries.

### Why `is_flex_credit` as a boolean instead of string comparison?

Earlier versions detected flex entries by comparing `badge_entry.office` to the configured
`flex_credit` label string. This caused bugs when users changed the label — existing entries
no longer matched. The boolean field `is_flex_credit` is set at badge time and is unaffected
by label changes. The `flex_credit` setting is now purely a display label.

### Why store `workday_stats` inside `QuarterStats`?

The renderer needs per-day flags (badged/holiday/vacation/flex) to color each calendar cell.
Recomputing or re-querying in the render path would require passing all four data sources
to `render_calendar()`. By storing the resolved `HashMap<String, Workday>` in `QuarterStats`,
the render function gets everything it needs from one place.

### Why `OnceLock` for the data directory?

The alternative was threading a `PathBuf` through every function signature. Since the data
directory is set once at startup and never changes, a process-global is appropriate.
`OnceLock` is preferred over `Mutex<Option<>>` because reads are lock-free after initialization.
Tests use `load_from()` / `save_to()` which bypass the global and take an explicit path.

### Why separate entity and container structs?

Following the same `Entity` + `EntityData` pattern across all types makes the code
predictable. The entity struct (`BadgeEntry`) maps 1:1 to the JSON object; the container
struct (`BadgeEntryData`) maps to the top-level JSON structure and owns the collection.
`Persistable` is implemented on the container only.

---

## 10. How to Modify

### Add a new quarter

Edit `config/config.yaml`:
```yaml
quarters:
  - key: Q1_2027
    quarter: Q1
    year: "2027"
    start_date: "2027-02-01"
    end_date: "2027-04-30"
```
No code changes needed.

### Change the office name or flex credit label

Either edit `config/config.yaml` directly, or use the Settings view (`o` in the TUI) to edit
interactively. New badge entries will use the new values; existing entries are unaffected
(office name is purely decorative; flex status is stored as a boolean).

### Add a new metric to `QuarterStats`

1. Add the field to `QuarterStats` in `src/calc/quarter_calc.rs`
2. Compute it inside `calculate_quarter_stats()`
3. Add a row in `render_stats()` in `src/ui/calendar_view.rs`
4. Add it to the `write_stats()` output in `src/cmd/stats.rs` if it should appear in the CLI

### Add a new persistent data type

1. Create `src/data/my_type.rs`:
   ```rust
   #[derive(Serialize, Deserialize, Clone, Debug)]
   pub struct MyItem { pub name: String }

   #[derive(Serialize, Deserialize, Default, Debug)]
   pub struct MyItemData { pub items: Vec<MyItem> }

   impl Persistable for MyItemData {
       fn filename() -> &'static str { "my_items.yaml" }
       fn is_json() -> bool { false }
   }
   ```
2. Add `pub mod my_type;` and `pub use my_type::{MyItem, MyItemData};` to `src/data/mod.rs`
3. Load it in `src/cmd/root.rs` alongside the other loads
4. Pass it to `App::new()` if the TUI needs access

### Add a new key binding

In `src/ui/calendar_view.rs`, find the `Mode::Normal => { match code {` block:

```rust
KeyCode::Char('x') => {
    // your logic here
    self.update_stats(); // call this if badge_data changed
}
```

Then update the help table in `render_events_and_help()` to document the key.

### Change a color constant

At the top of `calendar_view.rs`:

```rust
const FLEX_COLOR: Color    = Color::Indexed(208);  // 256-color orange
const SECTION_BG: Color    = Color::Indexed(235);  // dark gray background
const BADGE_COLOR: Color   = Color::Green;
const TODAY_COLOR: Color   = Color::Red;
```

Use `Color::Indexed(N)` for [256-color terminals](https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit)
or `Color::Rgb(r, g, b)` for 24-bit color.

---

## 11. Running Tests

```bash
cargo test                           # run all 162 tests
cargo test quarter_calc              # run tests whose name contains "quarter_calc"
cargo test -- --nocapture            # show println! output during tests
cargo test cmd::stats                # run tests in a specific module
make coverage-summary                # print line coverage to terminal
make coverage-html                   # open HTML coverage report in browser
```

Tests live at the bottom of their respective source files in `#[cfg(test)] mod tests { ... }`.

| Module | Tests cover |
|--------|------------|
| `calc/workday.rs` | Weekday detection, map boundary dates, holiday/vacation flags |
| `calc/quarter_calc.rs` | All four compliance statuses, pace calculation, flex tracking, edge cases |
| `cmd/stats.rs` | All output variants (Achieved, Infinite, N/A, pace ahead/behind/on) |
| `cmd/vacations.rs` | Empty list, single entry, unapproved, multiple entries |
| `cmd/holidays.rs` | Empty list, single entry, multiple entries, column alignment |
| `cmd/backup.rs` | Git init/commit in temp dir; push to bare repo |
| `cmd/init.rs` | All files created, correct counts, parseable YAML/JSON |
| `ui/calendar_view.rs` | Arrow key navigation, Space/f toggle, what-if mode, add/delete events, n/p past configured quarters, year stats, search |
| `main.rs` | `dir_needs_init` logic |

The `#[cfg(test)]` attribute means test code compiles only when running `cargo test` —
equivalent to `[TestClass]` isolation in C#. The `tempfile` crate provides isolated temp
directories for file I/O tests.

---

## 12. Learning Resources

### Rust Language

| Resource | Notes |
|----------|-------|
| [The Rust Book](https://doc.rust-lang.org/book/) | The official free book. Start here. Chapters 1–10 cover everything needed to understand this codebase. |
| [Rust by Example](https://doc.rust-lang.org/rust-by-example/) | Concept + code snippet pairs — good for quick reference. |
| [Rustlings](https://github.com/rust-lang/rustlings) | Interactive exercises you run in your terminal — the fastest way to internalize ownership. |
| [Rust for .NET Developers](https://microsoft.github.io/rust-for-dotnet-devs/latest/) | Microsoft's own C# → Rust migration guide. Directly relevant to this project. |
| [Jon Gjengset — Crust of Rust](https://www.youtube.com/playlist?list=PLqbS7AVVErFiWDOAVrPt7aYmnuuOLYvOa) | Deep dives on lifetimes, iterators, and advanced topics. Excellent for experienced developers. |

### Ownership and Borrowing (the hardest C# → Rust transition)

| Resource | Notes |
|----------|-------|
| [Book ch. 4: Ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html) | The canonical explanation. Read it twice. |
| [Book ch. 10: Lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html) | Explains `'a` annotations like those in `App<'a>`. |
| [Common Rust Lifetime Misconceptions](https://github.com/pretzelhammer/rust-blog/blob/master/posts/common-rust-lifetime-misconceptions.md) | Corrects the most frequent wrong mental models. Read after the Book chapter. |

### ratatui (Terminal UI)

| Resource | Notes |
|----------|-------|
| [Ratatui Book](https://ratatui.rs/introduction/) | Official guide covering widgets, layout, and the render loop. |
| [Ratatui Tutorials](https://ratatui.rs/tutorials/) | Step-by-step tutorials for counter apps, JSON editor, and more. |
| [Ratatui Examples](https://github.com/ratatui/ratatui/tree/main/examples) | Runnable examples for every widget type. Clone the repo and run them. |
| [docs.rs/ratatui](https://docs.rs/ratatui/latest/ratatui/) | Full API documentation. |
| [Awesome Ratatui](https://github.com/ratatui/awesome-ratatui) | Curated list of apps built with ratatui — great for inspiration and patterns. |

### Serde and Serialization

| Resource | Notes |
|----------|-------|
| [serde.rs](https://serde.rs) | Official guide. The [attributes page](https://serde.rs/attributes.html) documents every `#[serde(...)]` option. |
| [serde.rs derive tutorial](https://serde.rs/derive.html) | How to derive `Serialize` and `Deserialize` with examples. |
| [serde_norway on crates.io](https://crates.io/crates/serde_norway) | The YAML format crate used in this project (a soundness-fixed fork of serde_yaml). |

### Other Crates

| Resource | Notes |
|----------|-------|
| [Clap derive guide](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html) | Step-by-step tutorial for the derive API used in `main.rs`. |
| [Chrono format codes](https://docs.rs/chrono/latest/chrono/format/strftime/index.html) | All `%Y`, `%m`, `%d`, etc. format specifiers. |
| [Anyhow README](https://github.com/dtolnay/anyhow) | Short README covering all usage patterns in ~5 minutes. |
| [crossterm docs](https://docs.rs/crossterm/latest/crossterm/) | Terminal control primitives. Mostly you only need this for setup/teardown. |

### Tooling

```bash
cargo doc --open          # generate + browse docs for this crate and all dependencies
cargo clippy              # Rust's built-in linter (stricter than rustc warnings alone)
cargo fmt                 # auto-format all source files (like dotnet format)
cargo add <crate>         # add a dependency to Cargo.toml (like dotnet add package)
```

[docs.rs](https://docs.rs) — every published crate has auto-generated documentation here.
Equivalent to MSDN for .NET APIs — always start here when exploring an unfamiliar crate.
