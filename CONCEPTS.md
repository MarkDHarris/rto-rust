# Concepts & Learning Guide

This document covers every major concept, library, and pattern used in the `rto` codebase. It is written for someone who wants to read and understand the code quickly, or who wants to learn Rust through a real-world project.

---

## Table of Contents

- [Mastery Checklist](#mastery-checklist)
- [Rust Language Concepts](#rust-language-concepts)
- [Libraries & Dependencies](#libraries--dependencies)
- [Architecture Patterns](#architecture-patterns)
- [Data Serialization](#data-serialization)
- [Terminal UI (TUI)](#terminal-ui-tui)
- [CLI Design](#cli-design)
- [Testing Patterns](#testing-patterns)
- [Error Handling](#error-handling)
- [Date & Time Handling](#date--time-handling)
- [Concurrency & Global State](#concurrency--global-state)
- [External Resources](#external-resources)

---

## Mastery Checklist

To immediately understand all the code in this project, you should be comfortable with every topic below. Each links to the section where it's explained.

### Rust Language Fundamentals

- [ ] [Ownership, borrowing, and references](#ownership-borrowing-and-references) (`&`, `&mut`, move semantics)
- [ ] [Lifetimes](#lifetimes) (`'a` annotations on structs and functions)
- [ ] [Structs and enums](#structs-and-enums) (product types, sum types, `match`)
- [ ] [Traits](#traits) (`impl Trait`, `dyn Trait`, trait bounds, default methods)
- [ ] [Generics](#generics) (`<T>`, `where` clauses, `impl Into<String>`)
- [ ] [Pattern matching](#pattern-matching) (`match`, `if let`, `let ... && let` chains)
- [ ] [Error handling](#the-result-type) (`Result<T, E>`, the `?` operator, `anyhow`)
- [ ] [Closures and iterators](#closures-and-iterators) (`.map()`, `.filter()`, `.collect()`)
- [ ] [Modules and visibility](#modules-and-visibility) (`mod`, `pub`, `pub(crate)`, `use`, re-exports)
- [ ] [String types](#string-types) (`String` vs `&str`, `format!`, `to_string()`)
- [ ] [Collections](#collections) (`Vec<T>`, `HashMap<K, V>`, `BTreeMap`)
- [ ] [Smart pointers and interior mutability](#smart-pointers) (`Box`, `Option`, `OnceLock`)

### Libraries

- [ ] [serde — serialization framework](#serde)
- [ ] [serde_json — JSON format](#serde_json)
- [ ] [serde_norway — YAML format](#serde_norway)
- [ ] [chrono — date and time](#chrono)
- [ ] [clap — CLI argument parsing](#clap)
- [ ] [ratatui — terminal UI rendering](#ratatui)
- [ ] [crossterm — terminal control](#crossterm)
- [ ] [anyhow — error handling](#anyhow)
- [ ] [tempfile — temporary directories for testing](#tempfile)

### Patterns

- [ ] [Layered architecture](#layered-architecture) (data / calc / cmd / ui)
- [ ] [Trait-based persistence](#the-persistable-trait)
- [ ] [Dependency injection for testability](#dependency-injection)
- [ ] [Custom serde deserialization](#custom-serde-implementations) (`FlexTime`)
- [ ] [Immediate-mode TUI rendering](#immediate-mode-rendering)
- [ ] [What-if snapshots via Clone](#what-if-via-clone)
- [ ] [YAML post-processing normalization](#yaml-normalization)

---

## Rust Language Concepts

### Ownership, Borrowing, and References

Rust's ownership system is the foundation of its memory safety guarantees. Every value has exactly one owner. When the owner goes out of scope, the value is dropped.

In this codebase:
- `App<'a>` in `calendar_view.rs` borrows mutable references to data sources (`&'a mut BadgeEntryData`, `&'a mut HolidayData`, etc.) rather than owning them. This allows `root.rs` to retain ownership and save data after the TUI exits.
- `TimePeriodData` is owned by `App` (not borrowed) because it can be swapped at runtime when switching time period views.
- The `what_if_snapshot: Option<BadgeEntryData>` is an owned clone used for what-if mode, so the original borrowed data stays untouched.

```rust
// Borrowed references (data lives in root.rs)
badge_data: &'a mut BadgeEntryData,

// Owned value (can be replaced at runtime)
time_period_data: TimePeriodData,
```

**Learn more:** [The Rust Book — Ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)

### Lifetimes

Lifetimes tell the compiler how long references are valid. The `App<'a>` struct uses the lifetime `'a` to express that the borrowed data must outlive the `App` instance.

```rust
pub struct App<'a> {
    badge_data: &'a mut BadgeEntryData,
    // ...
}
```

This means you cannot create an `App` and then drop the data it refers to while the `App` is still alive. The compiler enforces this at build time.

**In `root.rs`**, this manifests as a specific drop order: `app.settings.clone()` must be called before `drop(app)`, and only then can `badge_data.save()` be called — because `app` holds `&mut` borrows that must be released first.

**Learn more:** [The Rust Book — Lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)

### Structs and Enums

Structs are product types (contain multiple fields). Enums are sum types (exactly one variant at a time).

```rust
// Struct — every field is present
pub struct Holiday {
    pub name: String,
    pub date: String,
}

// Enum — one of these at a time
enum Mode {
    Normal,
    Add,
    Delete,
    Search,
}

// Enum with data
enum ViewState {
    Calendar,
    Vacations,
    Holidays,
    Settings,
}
```

**Learn more:** [The Rust Book — Structs](https://doc.rust-lang.org/book/ch05-00-structs.html) | [Enums](https://doc.rust-lang.org/book/ch06-00-enums.html)

### Traits

Traits define shared behavior. This project uses traits extensively:

- **`Persistable`** — Custom trait for data types that can be loaded from and saved to files. Provides default `load()`, `save()`, `load_from()`, and `save_to()` implementations. Types only need to specify `filename()` and `is_json()`.
- **`Serialize` / `Deserialize`** — From serde. Derived on all data structs via `#[derive(Serialize, Deserialize)]`.
- **`Default`** — Provides a default value. Used for `AppSettings`, `HolidayData`, etc.
- **`Clone`** — Deep copy. Used for what-if snapshots and stats passing.
- **`Debug`** — Formatted debug output. Derived on most structs.
- **`Display`** — Human-readable output. Manually implemented for `FlexTime`.

```rust
pub trait Persistable: Sized + Default + Serialize + for<'de> Deserialize<'de> {
    fn filename() -> &'static str;
    fn is_json() -> bool;

    // Default implementations provided for load/save
    fn load() -> Result<Self> { /* ... */ }
    fn save(&self) -> Result<()> { /* ... */ }
}
```

**Learn more:** [The Rust Book — Traits](https://doc.rust-lang.org/book/ch10-02-traits.html)

### Generics

Generics let functions and structs work with many types. Used throughout:

- `save_yaml_to<T: Serialize>()` — works with any serializable type
- `load_yaml_from<T: for<'de> Deserialize<'de>>()` — works with any deserializable type
- `write_stats<W: std::io::Write>()` — writes to any output sink (stdout, Vec<u8>, file)

The `for<'de> Deserialize<'de>` syntax is a Higher-Ranked Trait Bound (HRTB), meaning "deserializable for any lifetime." This is standard for serde.

**Learn more:** [The Rust Book — Generics](https://doc.rust-lang.org/book/ch10-01-syntax.html)

### Pattern Matching

Pattern matching with `match` and `if let` is used throughout for control flow:

```rust
// match on enum variant
match self.mode {
    Mode::Normal => { /* handle keys */ }
    Mode::Add => { /* handle text input */ }
    Mode::Delete => { /* handle deletion */ }
    Mode::Search => { /* handle search */ }
}

// if-let chains (Rust 2024 edition feature)
if let Some(period) = self.current_period()
    && let (Some(start), Some(end)) = (period.start_date, period.end_date)
{
    // both conditions must hold
}

// matches! macro for boolean checks
let is_init_command = matches!(cli.command, Some(Commands::Init));
```

**Learn more:** [The Rust Book — Pattern Matching](https://doc.rust-lang.org/book/ch06-02-match.html)

### Closures and Iterators

Rust's iterator chain style is used heavily for data transformation:

```rust
// Filter, map, collect pattern
let months: Vec<NaiveDate> = row_months
    .iter()
    .map(|&month_date| self.render_single_month(month_date, ...))
    .collect();

// Iterator methods for aggregation
let max_lines = month_renders.iter().map(|r| r.len()).max().unwrap_or(0);

// Chaining with closures
let vacation_map: HashMap<String, Vacation> = self.vacations
    .iter()
    .filter(|v| NaiveDate::parse_from_str(&v.start_date, "%Y-%m-%d").is_ok())
    .flat_map(|v| /* expand date range */)
    .collect();
```

**Learn more:** [The Rust Book — Closures](https://doc.rust-lang.org/book/ch13-01-closures.html) | [Iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html)

### Modules and Visibility

The project uses Rust's module system to organize code into layers:

```rust
// main.rs declares top-level modules
mod calc;
mod cmd;
mod data;
mod ui;

// data/mod.rs re-exports public types for convenience
pub use app_settings::AppSettings;
pub use badge_entry::{BadgeEntry, BadgeEntryData};
```

Visibility levels used:
- `pub` — accessible everywhere
- `pub(crate)` — accessible within the crate but not externally
- private (no keyword) — accessible only within the defining module

**Learn more:** [The Rust Book — Modules](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html)

### String Types

Rust has two main string types:

| Type | Ownership | Where used |
|---|---|---|
| `String` | Owned, heap-allocated, growable | Struct fields, return values |
| `&str` | Borrowed slice (reference) | Function parameters, string literals |

Common conversions in this codebase:
```rust
"hello".to_string()        // &str → String
format!("{} {}", a, b)     // creates a new String
&my_string                 // String → &str (auto-deref)
s.as_str()                 // explicit String → &str
```

**Learn more:** [The Rust Book — Strings](https://doc.rust-lang.org/book/ch08-02-strings.html)

### Collections

| Collection | Purpose | Example in code |
|---|---|---|
| `Vec<T>` | Ordered, growable list | `holidays: Vec<Holiday>`, `badge_data: Vec<BadgeEntry>` |
| `HashMap<K, V>` | Key-value lookup (unordered) | `workday_stats: HashMap<String, Workday>` |
| `BTreeMap<K, V>` | Key-value lookup (sorted) | Not used directly but available |

The `get_holiday_map()`, `get_vacation_map()`, and `get_badge_map()` methods build `HashMap` indexes keyed by `"YYYY-MM-DD"` date strings for O(1) lookups during calendar rendering.

**Learn more:** [The Rust Book — Collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html)

### Smart Pointers

| Type | Purpose | Where used |
|---|---|---|
| `Option<T>` | Nullable value | `active_stats: Option<QuarterStats>`, `projected_completion_date: Option<NaiveDate>` |
| `OnceLock<T>` | Write-once, lock-free global | `static DATA_DIR: OnceLock<PathBuf>` in `persistence.rs` |
| `Box<T>` | Heap allocation | Not used directly but underlying many types |

`Option` is Rust's alternative to null. You must explicitly handle both `Some(value)` and `None` — the compiler will not let you forget.

**Learn more:** [The Rust Book — Option](https://doc.rust-lang.org/std/option/) | [OnceLock](https://doc.rust-lang.org/std/sync/struct.OnceLock.html)

---

## Libraries & Dependencies

### serde

The foundational serialization framework for Rust. Does not perform I/O itself — it defines the `Serialize` and `Deserialize` traits that data formats (JSON, YAML, TOML, etc.) implement.

Most types use the derive macros: `#[derive(Serialize, Deserialize)]`.

Serde attributes used in this codebase:
- `#[serde(rename = "entry_date")]` — use a different field name in the serialized format
- `#[serde(default)]` — use `Default::default()` if the field is missing
- `#[serde(skip)]` — exclude a field from serialization entirely
- `#[serde(omitempty)]` — omit the field if it's the default value

**Docs:** [serde.rs](https://serde.rs/) | [Attributes reference](https://serde.rs/attributes.html)

### serde_json

Provides `to_string_pretty()` and `from_str()` for JSON. Used for `badge_data.json` and `events.json`.

**Docs:** [docs.rs/serde_json](https://docs.rs/serde_json)

### serde_norway

A fork of `serde_yaml` for YAML serialization. Provides `to_string()` and `from_str()`. Uses the `unsafe-libyaml` C library underneath for parsing and emitting.

In this project, YAML output from `serde_norway` is post-processed by `normalize_yaml_strings()` to ensure all string values are consistently double-quoted.

**Docs:** [docs.rs/serde_norway](https://docs.rs/serde_norway) | [YAML specification](https://yaml.org/spec/1.2.2/)

### chrono

Date and time library. The primary types used are:

| Type | Purpose |
|---|---|
| `NaiveDate` | Calendar date without timezone (e.g., `2025-03-15`) |
| `NaiveDateTime` | Date and time without timezone |
| `Local` | System local time (`Local::now()`) |
| `Datelike` | Trait providing `.year()`, `.month()`, `.day()`, `.weekday()` |
| `Duration` | Time span (e.g., `Duration::days(1)`) |

The `FlexTime` newtype wraps `NaiveDateTime` with a custom deserializer that tries multiple format strings, handling the various datetime formats encountered in the shared data files.

**Docs:** [docs.rs/chrono](https://docs.rs/chrono)

### clap

Command-line argument parser. Uses the derive API to define CLI structure as Rust types:

```rust
#[derive(Parser)]
#[command(name = "rto")]
struct Cli {
    #[arg(short = 'd', long, default_value = "./config")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Stats { period_key: Option<String> },
    Backup { #[arg(short, long)] remote: Option<String> },
    Vacations,
    Holidays,
}
```

**Docs:** [docs.rs/clap](https://docs.rs/clap) | [Derive tutorial](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html)

### ratatui

Terminal UI framework using an immediate-mode rendering model. Every frame, you describe the entire UI from scratch; ratatui diffs it against the previous frame and only updates changed cells.

Key types used in this codebase:

| Type | Purpose |
|---|---|
| `Frame` | Render target for a single frame |
| `Terminal` | Manages the terminal backend |
| `Layout` | Divides areas into sub-rectangles |
| `Constraint` | Sizing rules (`Length`, `Min`, `Max`, `Percentage`) |
| `Direction` | `Horizontal` or `Vertical` layout |
| `Block` | Border/title wrapper for widgets |
| `Paragraph` | Multi-line text widget (used for calendars) |
| `Table` | Tabular data widget (used for stats) |
| `Row`, `Cell` | Table building blocks |
| `Span`, `Line` | Styled text segments |
| `Style`, `Color`, `Modifier` | Visual styling (foreground, background, bold, underline) |
| `TableState` | Stateful table for scroll/selection tracking |

**Docs:** [ratatui.rs](https://ratatui.rs/) | [Widget gallery](https://ratatui.rs/showcase/widgets/)

### crossterm

Cross-platform terminal manipulation library. Handles raw mode, alternate screen, and keyboard events.

```rust
// Terminal setup
enable_raw_mode()?;
execute!(stdout, EnterAlternateScreen)?;

// Event handling
if event::poll(Duration::from_millis(16))? {
    if let CEvent::Key(key) = event::read()? {
        app.handle_key(key.code, key.modifiers);
    }
}

// Terminal teardown
disable_raw_mode()?;
execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
```

- **Raw mode**: Disables line buffering and echo so keypresses are delivered immediately.
- **Alternate screen**: Switches to a secondary terminal buffer so the original content is restored when the app exits.

**Docs:** [docs.rs/crossterm](https://docs.rs/crossterm) | [GitHub](https://github.com/crossterm-rs/crossterm)

### anyhow

Ergonomic error handling. Provides `anyhow::Result<T>` (alias for `Result<T, anyhow::Error>`) and `.context()` for adding human-readable error messages.

```rust
let contents = fs::read_to_string(&path)
    .with_context(|| format!("failed to read {}", path.display()))?;
```

Also provides `bail!()` for early returns with an error message:

```rust
if period.is_none() {
    bail!("Period key '{}' not found", key);
}
```

**Docs:** [docs.rs/anyhow](https://docs.rs/anyhow)

### tempfile

Creates temporary directories for tests. The directory and all its contents are automatically deleted when the `TempDir` value is dropped.

```rust
#[test]
fn test_roundtrip() {
    let tmp = TempDir::new().unwrap();
    data.save_to(tmp.path()).unwrap();
    let loaded = Data::load_from(tmp.path()).unwrap();
    assert_eq!(loaded, data);
} // tmp is dropped here → directory deleted
```

**Docs:** [docs.rs/tempfile](https://docs.rs/tempfile)

---

## Architecture Patterns

### Layered Architecture

The codebase is split into four layers with strict dependency rules:

```
  ui/  →  cmd/  →  calc/  →  data/
```

| Layer | Responsibility | I/O | State |
|---|---|---|---|
| `data/` | Models, serialization, file I/O | Read/write files | Global data dir |
| `calc/` | Pure math and statistics | None | None |
| `cmd/` | Wire CLI commands | Calls data + calc | Temporary |
| `ui/` | TUI rendering and input handling | Terminal I/O | `App` struct |

`calc/` never imports from `cmd/` or `ui/`. `data/` never imports from any other layer.

### The Persistable Trait

The `Persistable` trait provides a template method pattern for file persistence:

```rust
trait Persistable: Sized + Default + Serialize + Deserialize {
    fn filename() -> &'static str;  // e.g., "holidays.yaml"
    fn is_json() -> bool;           // JSON or YAML?

    // All these have default implementations:
    fn load() -> Result<Self>;
    fn save(&self) -> Result<()>;
    fn load_from(dir: &Path) -> Result<Self>;
    fn save_to(&self, dir: &Path) -> Result<()>;
}
```

Each data type (`HolidayData`, `VacationData`, `EventData`, `BadgeEntryData`) implements only `filename()` and `is_json()`. The load/save logic is inherited.

`AppSettings` and `TimePeriodData` use standalone `load_yaml_from` / `save_yaml_to` functions instead, because they have custom loading logic (default merging and file-per-view respectively).

### Dependency Injection

Functions are designed for testability by accepting abstract parameters:

| Pattern | Production | Test |
|---|---|---|
| `write_stats<W: Write>(out: &mut W, ...)` | `&mut stdout()` | `&mut Vec<u8>` |
| `calculate_quarter_stats(..., today: Option<NaiveDate>)` | `None` (uses `Local::now()`) | `Some(fixed_date)` |
| `data.save_to(dir)` / `data.load_from(dir)` | `get_data_dir()` | `tmp.path()` |
| `App::new(..., today: NaiveDate, ...)` | `Local::now().date_naive()` | Fixed `NaiveDate` |

### What-If via Clone

When the user enters what-if mode:
1. `self.badge_data.clone()` creates a deep copy stored in `what_if_snapshot`
2. The user makes changes to `self.badge_data` (the real mutable reference)
3. On exit, `*self.badge_data = snapshot` restores the original data in-place

This works because `BadgeEntryData` derives `Clone`, and the `App` holds a `&'a mut` reference that allows in-place assignment.

---

## Data Serialization

### Derive-Based Serialization

Most structs use serde's derive macros:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Holiday {
    pub name: String,
    pub date: String,
}
```

This generates `Serialize` and `Deserialize` implementations at compile time. The serialized field names match the Rust field names by default.

### Custom Serde Implementations

`FlexTime` implements `Serialize` and `Deserialize` manually to handle multiple datetime format strings:

```rust
impl<'de> Deserialize<'de> for FlexTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        for fmt in FLEX_TIME_FORMATS {
            if let Ok(dt) = NaiveDateTime::parse_from_str(&s, fmt) {
                return Ok(FlexTime(dt));
            }
        }
        Err(Error::custom("cannot parse datetime"))
    }
}
```

This allows badge data to contain datetimes in any of these formats: `2025-01-06T09:00:00-05:00`, `2025-01-06T09:00:00`, `2025-01-06T09:00:00Z`, or `2025-01-06`.

### YAML Normalization

`serde_norway::to_string()` produces valid YAML but with inconsistent quoting: plain scalars for simple strings, single quotes for strings with special characters, and no quotes for date-like values.

The `normalize_yaml_strings()` function in `persistence.rs` post-processes the output to use double quotes for all string values:

**Before normalization:**
```yaml
- name: New Year's Day
  date: 2025-01-01
- name: 'Thank You Day #1'
  date: 2025-03-14
```

**After normalization:**
```yaml
- name: "New Year's Day"
  date: "2025-01-01"
- name: "Thank You Day #1"
  date: "2025-03-14"
```

Booleans, integers, and floats are left as-is. The normalization is applied in all YAML save paths.

---

## Terminal UI (TUI)

### Immediate-Mode Rendering

Ratatui uses an immediate-mode rendering model. Every 16ms (60 FPS), the entire UI is described from scratch:

```rust
loop {
    terminal.draw(|f| app.render(f))?;
    if event::poll(Duration::from_millis(16))? {
        if let CEvent::Key(key) = event::read()? {
            if app.handle_key(key.code, key.modifiers) {
                break;
            }
        }
    }
}
```

There is no retained widget tree. The `render()` function builds layouts, creates widgets, and renders them every frame. Ratatui internally diffs the output and only sends changed cells to the terminal.

### Layout System

Layouts divide rectangular areas into sub-areas. This project uses a two-level layout:

```rust
// Top level: horizontal split (left panel + right panel)
let h_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Min(30),      // left: calendar + events
        Constraint::Length(68),   // right: stats panels
    ])
    .split(size);

// Left panel: vertical split (calendar on top, events below)
let left_chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(cal_height),  // dynamic calendar height
        Constraint::Min(10),             // events fill remaining
    ])
    .split(h_chunks[0]);
```

### Styled Text

Text is built from `Span` (a styled string segment) and `Line` (a row of spans):

```rust
let line = Line::from(vec![
    Span::styled("[b]", Style::default().fg(Color::Indexed(51))),
    Span::raw(" "),
    Span::styled("McLean, VA", Style::default().fg(Color::DarkGray)),
]);
```

### The App Struct

`App<'a>` is the central state object. It holds:
- Borrowed references to mutable data sources
- Owned UI state (selected date, mode, input buffer)
- Cached statistics (`active_stats`, `year_stats`)
- View state (`Calendar`, `Vacations`, `Holidays`, `Settings`)

The `render()` method dispatches to view-specific renderers. The `handle_key()` method dispatches to view-specific key handlers. Both switch on `view_state` first, then on `mode`.

### Terminal Setup and Teardown

The terminal must be properly initialized and restored:

```rust
// Setup: raw mode + alternate screen
enable_raw_mode()?;
execute!(stdout, EnterAlternateScreen)?;

// Teardown: restore original state (MUST happen even on panic)
disable_raw_mode()?;
execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
terminal.show_cursor()?;
```

Raw mode disables line buffering so individual keypresses are delivered. The alternate screen ensures the user's previous terminal content is restored when the app exits.

---

## CLI Design

### Derive-Based Parsing with clap

The CLI is defined as Rust types using clap's derive API:

```rust
#[derive(Parser)]
#[command(name = "rto")]
struct Cli {
    #[arg(short = 'd', long, default_value = "./config")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}
```

`Option<Commands>` means running `rto` with no subcommand is valid (it launches the TUI). Subcommands are defined as enum variants, each with their own arguments.

### Auto-Initialization

Before dispatching any command (except `init`), `main()` checks whether the data directory has been initialized by looking for `settings.yaml`. If missing, it automatically runs `rto init`.

---

## Testing Patterns

### Test Organization

Every module has a `#[cfg(test)] mod tests` block at the bottom of the file. Tests are co-located with the code they test.

### Temporary Directories

Tests that involve file I/O use `tempfile::TempDir`:

```rust
let tmp = TempDir::new().unwrap();
data.save_to(tmp.path()).unwrap();
let loaded = Data::load_from(tmp.path()).unwrap();
```

This avoids polluting the real filesystem and ensures test isolation.

### Write Trait for Output Testing

CLI output functions accept `&mut impl Write` instead of writing directly to stdout:

```rust
pub fn write_stats<W: std::io::Write>(out: &mut W, stats: &QuarterStats, ...) -> Result<()>
```

Tests pass a `Vec<u8>` as the writer and then inspect the output:

```rust
let mut buf = Vec::new();
write_stats(&mut buf, &stats, ...).unwrap();
let output = String::from_utf8(buf).unwrap();
assert!(output.contains("On Track"));
```

### Deterministic Date Injection

Functions that depend on "today's date" accept it as a parameter:

```rust
calculate_quarter_stats(&period, &badges, &holidays, &vacations, Some(fixed_date))
```

Passing `Some(date)` in tests ensures reproducible results. Passing `None` in production uses `Local::now()`.

### No-Terminal Testing

The TUI event loop (`run_app`) requires a real terminal and is not unit-tested. However, all pure helpers (`calendar_day_style`, `search_events`, `add_months`, `days_in_month`, `month_name`) are `pub(crate)` and have standalone unit tests. Key handling and data mutation logic are tested by calling `App::new()` with test data and then invoking `handle_key()`.

---

## Error Handling

### The Result Type

Rust's `Result<T, E>` is used for all fallible operations. The `?` operator propagates errors up the call stack:

```rust
fn load() -> Result<Self> {
    let path = get_file_path(Self::filename())?;  // propagates on error
    let contents = fs::read_to_string(&path)?;     // propagates on error
    let data = serde_norway::from_str(&contents)?;  // propagates on error
    Ok(data)
}
```

### anyhow for Application Errors

This project uses `anyhow::Result<T>` (which is `Result<T, anyhow::Error>`) throughout. The `anyhow::Error` type can wrap any error and adds context:

```rust
let contents = fs::read_to_string(&path)
    .with_context(|| format!("failed to read {}", path.display()))?;
```

This produces error messages like: `failed to read /path/to/file: No such file or directory`.

**Learn more:** [The Rust Book — Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)

---

## Date & Time Handling

All date keys throughout the application use the `"YYYY-MM-DD"` string format. This is the standard ISO 8601 format and is used as HashMap keys for O(1) lookups across badge data, holidays, vacations, and workday stats.

```rust
let date_key = date.format("%Y-%m-%d").to_string();
let badge = badge_map.get(&date_key);
let holiday = holiday_map.get(&date_key);
```

Date arithmetic uses `chrono::Duration`:

```rust
let next_day = current_date + Duration::days(1);
```

The `add_months` helper handles month arithmetic (which `chrono` does not provide natively) with end-of-month clamping.

---

## Concurrency & Global State

### OnceLock for Data Directory

The data directory path is stored in a `static OnceLock<PathBuf>`:

```rust
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_data_dir(path: PathBuf) {
    let _ = DATA_DIR.set(path);
}
```

`OnceLock` can be written to exactly once and then read lock-free. It is thread-safe but this project is single-threaded. The main use is allowing `main()` to set the path once, and then all `load()` / `save()` calls read it without passing the path explicitly.

**Important for testing:** `OnceLock` cannot be reset. Tests that need different data directories must use `load_from(dir)` / `save_to(dir)` instead of `load()` / `save()`.

---

## External Resources

### Rust Language

| Resource | Description |
|---|---|
| [The Rust Programming Language (The Book)](https://doc.rust-lang.org/book/) | The official, comprehensive Rust tutorial |
| [Rust By Example](https://doc.rust-lang.org/rust-by-example/) | Learn Rust through annotated examples |
| [The Rust Reference](https://doc.rust-lang.org/reference/) | Formal language specification |
| [Rust Standard Library Docs](https://doc.rust-lang.org/std/) | API documentation for std |
| [Rustlings](https://github.com/rust-lang/rustlings) | Small exercises to learn Rust syntax |

### Libraries Used in This Project

| Library | Documentation | Source |
|---|---|---|
| serde | [serde.rs](https://serde.rs/) | [GitHub](https://github.com/serde-rs/serde) |
| serde_json | [docs.rs/serde_json](https://docs.rs/serde_json) | [GitHub](https://github.com/serde-rs/json) |
| serde_norway | [docs.rs/serde_norway](https://docs.rs/serde_norway) | [GitHub](https://github.com/cafkafk/serde-norway) |
| chrono | [docs.rs/chrono](https://docs.rs/chrono) | [GitHub](https://github.com/chronotope/chrono) |
| clap | [docs.rs/clap](https://docs.rs/clap) | [GitHub](https://github.com/clap-rs/clap) |
| ratatui | [ratatui.rs](https://ratatui.rs/) | [GitHub](https://github.com/ratatui/ratatui) |
| crossterm | [docs.rs/crossterm](https://docs.rs/crossterm) | [GitHub](https://github.com/crossterm-rs/crossterm) |
| anyhow | [docs.rs/anyhow](https://docs.rs/anyhow) | [GitHub](https://github.com/dtolnay/anyhow) |
| tempfile | [docs.rs/tempfile](https://docs.rs/tempfile) | [GitHub](https://github.com/Stebalien/tempfile) |

### TUI Development

| Resource | Description |
|---|---|
| [Ratatui Book](https://ratatui.rs/introduction/) | Official guide for building TUIs with ratatui |
| [Ratatui Widget Gallery](https://ratatui.rs/showcase/widgets/) | Visual examples of every widget |
| [Awesome Ratatui](https://github.com/ratatui/awesome-ratatui) | Community projects and templates |

### YAML

| Resource | Description |
|---|---|
| [YAML 1.2 Specification](https://yaml.org/spec/1.2.2/) | The formal specification |
| [Learn YAML in Y Minutes](https://learnxinyminutes.com/docs/yaml/) | Quick reference |

### Cargo & Tooling

| Resource | Description |
|---|---|
| [The Cargo Book](https://doc.rust-lang.org/cargo/) | Build system and package manager |
| [Clippy Lints](https://rust-lang.github.io/rust-clippy/master/index.html) | All available lint rules |
| [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) | Code coverage tool used by this project |
