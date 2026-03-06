# Record: Agent Instructions

Privacy-first macOS activity tracker. Tauri v2 desktop app with a Rust backend and Solid.js frontend.

## Architecture

Monorepo with pnpm workspaces. Two workspace roots: `apps/` for applications and `packages/` for shared code.

```
apps/desktop/          Tauri v2 app (the only app for now)
  src/                 Solid.js frontend (Vite, TypeScript)
  src-tauri/           Rust backend (tracker, store, tray)
packages/types/        Shared TypeScript interfaces (@record/types)
```

Everything reusable goes in `packages/`. The types package defines the contract between Rust and TypeScript. If you add a new data shape that both sides need, put the TS interface in `packages/types/src/index.ts` and the matching Rust struct in `apps/desktop/src-tauri/src/types.rs`.

## Tech Stack

| Layer | Tool | Notes |
|-------|------|-------|
| Desktop framework | Tauri v2 | Not v1. API differences are significant. |
| Frontend | Solid.js + Vite | NOT SolidStart. No router. Signal-based view switching. |
| Backend | Rust | Tracker daemon, SQLite store, tray icon, app icon extraction. |
| Database | SQLite via rusqlite 0.31 (bundled) | WAL mode. RFC3339 timestamps as TEXT. |
| Linting/formatting (TS) | Biome | Single config at repo root. |
| Linting/formatting (Rust) | cargo fmt + clippy | Standard rustfmt, clippy with `-D warnings`. |
| Pre-commit hooks | Lefthook | Config in `lefthook.yml`. |
| CI | GitHub Actions | macOS runners (required for Tauri build). |

## Development Commands

All commands run from the repo root:

| Command | What it does |
|---------|-------------|
| `pnpm install` | Install all dependencies |
| `pnpm dev` | Start Tauri dev mode (frontend + backend hot reload) |
| `pnpm build` | Production build of the Tauri app |
| `pnpm lint` | Biome check (lint + format) |
| `pnpm lint:fix` | Biome auto-fix |
| `pnpm format` | Biome format only |
| `pnpm typecheck` | TypeScript type checking |
| `pnpm test` | Run Rust tests |
| `pnpm clippy` | Run cargo clippy |
| `pnpm fmt:rust` | Check Rust formatting |
| `pnpm check` | Run everything (lint, typecheck, fmt:rust, clippy, test) |

## Code Style

TypeScript/TSX: single quotes, no semicolons, 2-space indent, trailing commas. Biome enforces this automatically.

Rust: standard rustfmt (4-space indent). No custom rustfmt.toml.

No inline or block comments. JSDoc only for critical public APIs. Keep code self-documenting.

## Testing

Rust tests live alongside the code in `#[cfg(test)] mod tests {}` blocks. The `SessionStore` and `Tracker` modules have full test coverage. Use `FakeProbe` (implements `SystemProbe` trait) for tracker tests instead of mocking macOS APIs directly.

There are no frontend tests. The app is simple enough that Rust tests cover the important logic. If frontend complexity grows, set up Vitest with solid-testing-library.

When writing new Rust tests, follow the existing pattern: use `tempfile::NamedTempFile` for database tests, `FakeProbe` for tracker tests.

## Key Rust Patterns

The `SystemProbe` trait abstracts macOS system calls (active window, idle time). `MacOSProbe` is the real implementation; `FakeProbe` is for tests. This is the only mock boundary.

Bundle ID resolution: `process_path` from `active-win-pos-rs` gets walked up to find `.app`, then `Contents/Info.plist` is read for `CFBundleIdentifier`. Results are cached in a `HashMap`.

Heartbeat merge logic: if the current app+bundle+idle_state matches the most recent session AND the gap is under `merge_gap_secs` (10s), the existing session is extended. Otherwise a new row is inserted.

App icon extraction: `mdfind` to locate the `.app`, read `CFBundleIconFile` from plist, `sips` to convert `.icns` to `.png`, encode as base64. Cached on disk and in memory.

## Frontend Patterns

`createStore` + `reconcile` with a `key` field prevents list flickering on data refresh. Use `<For>` (not `<Index>`) for lists backed by stores.

The Today view uses dual-rate updates: 5-second backend fetch + 1-second frontend interpolation for live-ticking counters.

View switching uses persistent mounting, not a router or Dynamic. All views stay mounted once visited and are toggled via `display: none/block`. This eliminates loading flashes entirely. Hovering a sidebar nav item triggers both `lazy().preload()` (loads the JS chunk) and adds the view to the `visited` set (mounts the component hidden). By the time the user clicks, the view has already fetched its data and rendered. Never show "Loading..." text or empty fallbacks during view transitions.

When adding new views, follow this pattern: add the lazy import, add an entry to `viewOrder`, add a `Show when={visited().has('viewname')}` block in the content area with `display` toggled by the active view signal. Use `createResource` for initial data loads so the view renders only when data is ready (avoids flicker from signal defaults being overwritten on mount).

Keyboard shortcuts: Cmd+1/2/3/4 switch views. Arrow keys navigate dates in Today and Weekly views. Press `t` to jump to today.

## Project Detection Adapters

Each app that can reveal project context gets its own adapter implementing `ProjectAdapter`. Adding support for a new app means dropping in a new file under `src/project/adapters/`. No big match statements, no central registry logic.

The trait has two extraction methods. `extract(window_title)` is the primary path: parse the window title for a project name (most editors put it right there). `extract_at(window_title, timestamp)` is the fallback for apps where the window title is useless. This is where adapters get creative.

The goal is to detect what the user is working on by any signal available, not just window titles. Think outside the box. Examples of valid detection strategies:

- **Window title parsing**: Zed, VS Code, JetBrains IDEs all put `project — file` in the title bar. Parse the project name out.
- **Reading another app's database**: OpenCode's window title is just "OpenCode" (useless), but it stores every session with a directory path in `~/.local/share/opencode/opencode.db`. The adapter opens that DB read-only and finds which project was active at the heartbeat timestamp.
- **Process working directory**: Terminal apps show `user@host: ~/path` in the title, but you could also read the shell process's cwd via `libproc`.
- **State files**: Some apps write their current workspace to a JSON or plist file. If the format is stable enough, read it.

When choosing a detection strategy, prefer stability. A core database schema (like opencode's `session` table with `directory` and `time_updated`) is unlikely to break. A UI state file with nested JSON blobs will. Process cwd is OS-level and very stable. Window title formats are set by the app and rarely change.

Always open external databases with `SQLITE_OPEN_READ_ONLY`. Never modify another app's data.

## Agent Time Tracking

The `agent` module tracks AI agent work time alongside regular active time. A background scanner reads external agent databases (currently OpenCode at `~/.local/share/opencode/opencode.db`) every 30 seconds and stores computed work slices in the `agent_sessions` table.

The `AgentProvider` trait abstracts agent sources. Adding a new agent (Claude Code, Cursor, etc.) means implementing the trait and adding it to the providers vec in `start_agent_scanner()`. The store, types, and frontend are agent-agnostic.

Work slices are computed from assistant message timestamps: consecutive messages within 60s are merged into a single block. Sub-second slices are filtered out. Sessions that have been updated since the last scan are recomputed fully (delete + reinsert).

**Time model (mode 2, the default):** Active time and agent time are merged per project using interval union. If you're focused on a project while an agent is also working on it, that counts once (not double). Agent time that doesn't overlap with active time adds to the project total. Both active and agent time are billable. The project bars show the split visually: normal color for active time, lighter accent for agent-only time.

The `ProjectUsage` struct carries `active_secs` (your focus time), `agent_secs` (agent-only time that didn't overlap with focus), and `total_secs` (the union). `total_secs = active_secs + agent_secs` always holds.

## Design

Matches the stoff.dev palette. Figtree for body text, JetBrains Mono for numbers and code. Dark theme is primary (`#111` bg, `#5ba5f5` accent). Light mode via `prefers-color-scheme`.

## Gotchas

Port 5173/5174 conflicts: kill stale Vite processes before `pnpm dev` or the Tauri webview connects to the wrong port and shows a blank window.

`active-win-pos-rs` 0.9 has `process_path` not `bundle_id`. We resolve bundle IDs ourselves.

The `src-tauri/` directory must be a sibling to the frontend build output. Do not move it.

Window close hides to tray (does not quit). The "Quit" menu item in the tray is the real exit path.

The app needs macOS Accessibility permission for window titles. Without it, app names still work but `title` will be empty.
