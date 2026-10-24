# pluto

A terminal UI for PostgreSQL. Browse schemas and tables, page through data, and run arbitrary SQL — all from the keyboard, without leaving your terminal.

Built with [ratatui](https://github.com/ratatui-org/ratatui) and [sqlx](https://github.com/launchbakery/sqlx).

---

## Features

- **Profile picker** — save and manage multiple connection profiles; connect with a keypress
- **Schema sidebar** — collapsible tree of schemas and tables, fully keyboard and mouse navigable
- **Data grid** — page through table data with vim-style keys, mouse scrolling, and column selection
- **Query editor** — multi-line SQL popup (`i` to open, `Ctrl+Enter` to run, `Esc` to cancel)
- **Async everything** — no DB call ever blocks the draw loop
- **Panic-safe** — terminal is restored cleanly even on a crash

---

## Installation

You need Rust (stable) and a PostgreSQL server to connect to.

```sh
cargo build --release
# binary lands at ./target/release/pluto
```

---

## Adding a connection profile

```sh
pluto add <name> <host> <user> <password> <database> [sslmode]
```

Example:

```sh
pluto add neon-prod db.example.neon.tech alice hunter2 mydb require
```

Profiles are stored at `~/.config/pluto/config.toml` (Unix) with `0600` permissions.

---

## Usage

```sh
pluto
```

Pick a profile with `j`/`k`, press `Enter` to connect. Once connected you land on the main screen.

### Global

| Key | Action |
|-----|--------|
| `q` / `Ctrl+C` | Quit |
| `Tab` | Toggle focus: sidebar ↔ grid |

### Sidebar (schema/table tree)

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `Enter` | Expand/collapse schema, or load table |
| `h` / `l` | Collapse / expand |
| `g` / `G` | Jump to first / last entry |
| Mouse click | Focus + select |
| Mouse scroll | Scroll list |

### Data grid

| Key | Action |
|-----|--------|
| `h j k l` / arrows | Move cell selection |
| `g` / `G` | First / last row |
| `Ctrl+d` / `Ctrl+u` | Half-page down / up |
| `[` / `]` | Previous / next page |
| `r` | Refresh current table |
| Mouse click | Select cell, focus grid |
| Mouse scroll | Scroll rows |

### Query editor

| Key | Action |
|-----|--------|
| `i` | Open editor |
| `Ctrl+Enter` / `F5` | Execute query |
| `Esc` | Close without running |

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

---

## Project layout

```
src/
  main.rs      entry point — terminal init/teardown, event loop
  app.rs       App state, Screen enum, Focus enum
  config.rs    connection profile load/save
  db.rs        connection pool, async queries
  grid.rs      grid state: rows, columns, selection, scroll
  input.rs     multi-line editor state
  ui/
    mod.rs       root layout + draw
    grid_view.rs data grid widget
    editor.rs    query editor popup
```

---

## Roadmap

- [ ] Cell viewer — full-screen scrollable view for long text/JSON (M6)
- [ ] Metadata panel — column types, nullable, PK/FK, indexes (M7)
- [ ] Column sorting via `s` (M7)
- [ ] Column width adjust, hide, reset (M8)
- [ ] CSV export (M8)
- [ ] WHERE filter bar (M8)
- [ ] Query history persisted to disk (M8)

---

## License

MIT
