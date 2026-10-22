## Target structure

```
src/
  main.rs        entry: terminal init/teardown, run loop
  config.rs      connection profiles (done)
  db.rs          pool, queries: tables, table page, arbitrary SQL, metadata
  app.rs         App state, Screen enum, Focus enum
  events.rs      key + mouse dispatch
  grid.rs        grid state: rows, selection, scroll offsets, column widths
  input.rs       multi-line query editor state
  ui/
    mod.rs       root draw + layout
    sidebar.rs   schema/table tree
    grid_view.rs the data grid
    editor.rs    query editor popup
    viewer.rs    cell expand / focus mode
    statusbar.rs hints, errors, progress
```

## M1 — Terminal foundation

- Add `ratatui` + `crossterm`
- Terminal init: raw mode, alt screen, mouse capture on, panic-safe restore
- `App` struct with `Screen` enum + `running` flag; placeholder layout (sidebar block / main block / status bar)
- Event loop: 33ms tick, poll events, `q`/`Ctrl-C` quits
- **Done when:** `cargo run` shows the layout, quits cleanly, terminal intact even after Ctrl-C mid-frame

## M2 — Profile picker + async connect

- Startup screen listing saved profiles (`config.rs`); Enter to connect, `d` to delete, `a` documented as the CLI stopgap
- Connect spawns a tokio task; UI shows "connecting…" state; failures show inline with retry
- On success: load schemas + tables into `Vec<SchemaInfo>`, transition to main screen
- **Done when:** picking your Neon profile lands you on a populated main screen

## M3 — Sidebar tree

- Collapsible schema groups, tables beneath; `j`/`k` move, Enter expands/collapses or selects table, `h`/`l` for collapse/expand
- Focus model: `Tab` cycles sidebar ↔ main; active pane visibly highlighted
- **Done when:** fully keyboard-navigable tree, mouse click also works

## M4 — Data grid (the core — spend the most time here)

- Query layer: `SELECT * FROM t LIMIT page_size OFFSET n`, plus row count for pagination display
- `grid.rs`: column widths from content (capped), row/col selection, scroll offsets, page state
- Keys: `hjkl`/arrows, `gg`/`G`, `Ctrl+d`/`Ctrl+u` pages, `[`/`]` prev/next page
- Mouse: click cell to select + focus grid, wheel scrolls rows, click column header selects column
- Loading indicator while fetching; error → status bar
- **Done when:** you can comfortably page through `public.Player` fully by keys alone and by mouse alone

## M5 — Query editor (multi-line popup)

- `i` opens centered popup editor; type multi-line SQL, `Ctrl+Enter` runs, `Esc` cancels
- Results render into the grid; grid title shows the query (truncated)
- Session history, `↑`/`↓` recall in editor
- **Done when:** arbitrary SQL including joins runs and displays correctly

## M6 — Zoom: cell viewer + focus mode

- Enter on a cell → full-screen scrollable viewer, word-wrapped, for long text/JSON
- `f` → focus mode: current row expands full-width, `Tab`/`h`/`l` between columns
- **Done when:** inspecting a big JSONB cell or long text is pleasant

## M7 — Metadata + sorting

- `m` toggles metadata panel: columns, types, nullable, defaults, PK/FK, indexes (information_schema + pg_catalog queries in `db.rs`)
- `s` on a column cycles ORDER BY off → asc → desc; refetches
- **Done when:** you can inspect `public.ClubMatch`'s columns and sort by date without typing SQL

## M8 — Polish

- Column width adjust (`<`/`>` on selected column), `x` hides column, `0` resets layout
- `e` exports current grid to CSV in cwd
- History persisted to `~/.local/share/pluto/history`
- `w` opens a WHERE filter bar appended to table queries
- Status bar: connection, rows, page x/y, context-aware key hints
- **Done when:** it survives a week as your daily driver on the Neon DB

## Rules across all milestones

- No DB call ever blocks the draw loop — spawn + channel, always
- Every new keybinding appears in the status bar hint line
- Run `cargo clippy` before calling a milestone done
