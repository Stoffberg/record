# Record

A privacy-first activity tracker for macOS. It runs as a background daemon, tracks which apps you use and for how long, stores everything locally in SQLite, and shows daily and monthly breakdowns through a native desktop app.

No cloud. No accounts. Your data stays on your machine.

## Features

- Foreground app tracking with idle detection
- Live-ticking counters (1s frontend interpolation, 5s backend poll)
- Per-app breakdown with icons, session counts, and progress bars
- Monthly view with daily activity bars
- System tray with show/hide and quit
- Window close hides to tray (keeps tracking)
- Dark and light mode (follows system preference)

## Prerequisites

- macOS (Apple Silicon or Intel)
- [Rust](https://rustup.rs/) (1.77+)
- [Node.js](https://nodejs.org/) (22+)
- [pnpm](https://pnpm.io/) (10+)
- Xcode Command Line Tools (`xcode-select --install`)

## Setup

```sh
pnpm install
pnpm dev
```

On first launch, macOS will prompt for Accessibility permission. Grant it so Record can read window titles.

## Commands

```sh
pnpm dev          # Start in development mode
pnpm build        # Production build
pnpm check        # Run all checks (lint, typecheck, fmt, clippy, test)

pnpm lint         # Biome lint + format check
pnpm lint:fix     # Auto-fix lint and format issues
pnpm test         # Run Rust tests
pnpm typecheck    # TypeScript type checking
pnpm clippy       # Rust linting
```

## Project Structure

```
record/
├── apps/
│   └── desktop/           # Tauri v2 app
│       ├── src/           # Solid.js frontend
│       └── src-tauri/     # Rust backend
├── packages/
│   └── types/             # Shared TypeScript interfaces
├── biome.json             # Biome config (lint + format)
├── lefthook.yml           # Pre-commit hooks
└── package.json           # Root workspace scripts
```

## Stack

Rust backend for memory safety and small footprint during 24/7 uptime. Solid.js frontend for a tiny bundle and reactive primitives. SQLite for local storage with WAL mode. Tauri v2 ties it all together as a native macOS app with system tray support.

## Data

Activity data is stored in `~/Library/Application Support/dev.stoff.record/record.db`. App icon cache lives in the same directory under `icon_cache/`.

## License

MIT
