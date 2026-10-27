# pluto

A keyboard-first terminal UI for PostgreSQL. Browse schemas and tables, page through data,
filter, sort, inspect joins without writing SQL, and export CSV — all from the keyboard,
without leaving your terminal.

Built with [ratatui](https://github.com/ratatui-org/ratatui) and
[sqlx](https://github.com/launchbadge/sqlx).

---

## Features

- **Profile picker** — save and manage multiple connection profiles; connect with a keypress
- **Schema sidebar** — collapsible tree of schemas and tables, keyboard and mouse navigable
- **Data grid** — paged table browsing with vim-style keys, mouse scrolling, column selection
- **Sorting & filtering** — cycle ASC/DESC per column (`s`), WHERE filter per table (`w`)
- **Cell viewer** — full-screen scrollable view of a single cell (long text, JSON)
- **Focus mode** — one row, all its fields stacked vertically for easy scanning
- **Metadata panel** — column types, nullability, PK, defaults, and indexes (`m`)
- **Joins, no SQL required** — foreign-key picker plus a join builder: pick the related
  table (or any table), edit the ON column pairs, toggle exactly which columns from each
  table appear in the result
- **CSV export** — dump the current view to a timestamped CSV file (`e`)
- **Query editor** — multi-line SQL popup with persisted history
- **Async everything** — no DB call ever blocks the draw loop
- **Panic-safe** — terminal is restored cleanly even on a crash

---

## Installation

### Linux — install script

```sh
curl -fsSL https://raw.githubusercontent.com/darlingson/pluto/main/install.sh | bash
```

Downloads the latest release binary into `~/.local/bin` (pass `PLUTO_INSTALL_DIR` to
override). If that directory isn't on your `PATH`, the installer tells you.

### Windows

Download `pluto-*-x86_64-windows.zip` from the
[releases page](https://github.com/darlingson/pluto/releases), extract `pluto.exe`
somewhere on your `PATH`, and run it from Windows Terminal (or any terminal emulator).

### From source

```sh
cargo install --git https://github.com/darlingson/pluto
# or, for a working copy:
git clone https://github.com/darlingson/pluto && cd pluto
cargo build --release   # binary lands at ./target/release/pluto
```

You need a PostgreSQL server to connect to (9.4 or newer).

---

## Adding a connection profile

```sh
pluto add <name> <host> <user> <password> <database> [sslmode]
```

Example:

```sh
pluto add neon-prod db.example.neon.tech alice hunter2 mydb require
```

Profiles are stored at:

- Linux/macOS: `~/.config/pluto/config.toml` (permissions `0600`)
- Windows: `%APPDATA%\pluto\config.toml`

Query history lives next to it in `history.toml`.

---

## Usage

```sh
pluto
```

Pick a profile with `j`/`k`, press `Enter` to connect. Once connected you land on the
main screen: schema tree on the left, data grid on the right, status bar at the bottom
showing the current status and key hints for whatever popup is open.

### Profiles screen

| Key | Action |
|-----|--------|
| `j` / `k` | Select profile |
| `i` | Toggle profile details (host, user, database, ssl) |
| `Enter` | Connect |
| `d` | Delete profile |

### Global (main screen)

| Key | Action |
|-----|--------|
| `q` / `Ctrl+C` | Quit |
| `Tab` | Toggle focus: sidebar ↔ grid |
| `i` | Open SQL editor |
| `w` | Open filter editor (WHERE clause for the current table) |
| `m` | Toggle metadata panel |
| `o` | Open join picker |
| `f` | Toggle focus mode |
| `Esc` | Leave focus mode / close popup |

### Sidebar (schema/table tree)

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `Enter` | Expand/collapse schema, or load table |
| `h` / `l` | Collapse / expand |
| `g` / `G` | Jump to first / last entry |
| Mouse click / scroll | Focus + select / scroll |

### Data grid

| Key | Action |
|-----|--------|
| `h j k l` / arrows | Move cell selection |
| `g` / `G` | First / last row |
| `Ctrl+d` / `Ctrl+u` | Half-page down / up |
| `[` / `]` | Previous / next page |
| `r` | Refresh current table |
| `s` | Cycle sort: ASC → DESC → off (selected column) |
| `Enter` | Open cell viewer |
| `-` / `=` | Widen / narrow selected column |
| `x` | Toggle-hide selected column |
| `0` | Reset column layout |
| `e` | Export visible columns/rows to `pluto-<label>-<timestamp>.csv` |
| Mouse click / scroll | Select cell / scroll rows |

### Cell viewer (from `Enter` on a cell)

| Key | Action |
|-----|--------|
| `j` / `k` | Scroll |
| `Ctrl+d` / `Ctrl+u` | Half-page scroll |
| `g` / `G` | Top / bottom |
| `q` / `Esc` / `Enter` | Close |

### Query editor

| Key | Action |
|-----|--------|
| `Ctrl+Enter` / `Ctrl+R` / `Ctrl+E` / `F5` | Execute |
| `Enter` | Newline (it's multi-line) |
| `↑` / `↓` | History |
| `Esc` | Close without running |

### Joins (`o`)

The picker lists every foreign key on the current table — outgoing (`→`, this table
references another) and incoming (`←`, other tables reference this one) — each with its
ON clause preview. The last row is `+ any table…` for manual joins with no FK constraint
(e.g. a schema that skipped FK constraints).

`Enter` opens the **join builder**:

```
 join builder — public.PlayerClub ⋈ public.Club
─────────────────────────────────────────────────
 join on
   ON  b."ClubId" = t."ClubId"        ← editable, addable, removable
   + add join column pair…
 columns
   [x] b."PlayerClubId"               ← space/Enter toggles inclusion
   [x] b."PlayerId"
   [x] t."Name"
   [ ] t."FoundedYear"
 → run join · 4 cols · 1 pair
```

| Key | Action |
|-----|--------|
| `j` / `k` | Move |
| `space` / `Enter` | Toggle column inclusion (on column rows) |
| `Enter` | Pick/edit an ON pair — choose base column, then joined column |
| `d` | Remove the selected ON pair |
| `r` | Run the join now |
| `Esc` | Back out: column pick → builder → picker |

The result lands in the grid as a normal query — sorting-free but focus mode, cell
viewer, and CSV export all work on it. Duplicate column names (two `Id`s from
`b.*, t.*`-style matches) are suffixed `Id_2`, `Id_3`, …

---

## Filtering

`w` opens an editor for a WHERE fragment applied to the current table:

```sql
is_current = true AND created_at > '2025-01-01'
```

Empty filter clears it. Filtering composes with sorting and paging.

---

## Configuration file

`~/.config/pluto/config.toml` (created automatically on first `add`):

```toml
[[connections]]
name     = "local"
host     = "localhost"
port     = 5432
user     = "postgres"
password = "secret"
database = "mydb"
ssl_mode = "disable"
```

`ssl_mode` is one of `disable`, `prefer`, `require`.

---

## Project layout

```
src/
  main.rs       entry point — terminal init/teardown, event loop, key/mouse handling
  app.rs        App state, screens, profile picker, join picker, join builder
  config.rs     connection profiles + query history load/save
  db.rs         connection pool, async queries, FK lookup, SQL builders
  grid.rs       grid state: rows, columns, selection, scroll, layout
  input.rs      multi-line editor state
  ui/
    mod.rs        root layout + popups (viewer, join picker, join builder)
    grid_view.rs  data grid + focus mode widgets
    editor.rs     query editor popup
```

---

## Roadmap

- [ ] Table editing (INSERT/UPDATE/DELETE rows)
- [ ] Saved named queries
- [ ] MySQL/SQLite backends behind the same UI
- [ ] arm64 Linux/Windows builds

---

## License

[Apache-2.0](LICENSE)
