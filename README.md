# Digital Mess Cleaner

**Your files. Understood. Organised. Clean.**

A local-first desktop app for Windows and macOS that helps you find, understand,
organise and safely clean up your files from one trusted workspace.

Built with Tauri v2, Rust, React and TypeScript.

## What it does (Release 1)

- **Storage dashboard** — how much space is used, by category and folder.
- **Smart Search** — search the local index by name, type, size and date.
- **Large and forgotten files** — find the files worth reviewing first.
- **Exact duplicates** — byte-identical only, verified with cryptographic hashes.
- **Similar images** — shown as *similar*, never as exact.
- **Cleanup centre** — review first, quarantine, restore, and a full operation log.

Later releases (organiser rules, full-text and OCR search, document Q&A,
scheduled monitoring) are present as clearly-labelled empty states.

## Safety

- Nothing is deleted, moved or renamed without a preview and your explicit approval.
- Files are **quarantined before permanent delete**, with a manifest so they can be
  restored to their original path.
- Every file operation is logged and undoable where the OS allows.
- Only folders and drives you add to your library are ever scanned.
- Everything stays on your device. There are no network calls and no telemetry.

## Development

```bash
npm install
npm run tauri dev          # run the app

npm run typecheck && npm run lint && npm test
cd src-tauri
cargo fmt
cargo clippy -- -D warnings
cargo test
```

### Build installers

```bash
npm run tauri build
```

Outputs land in `src-tauri/target/release/bundle/` (NSIS `.exe` and MSI on
Windows, `.dmg`/`.app` on macOS).

## Project layout

```
src/                 React UI
  pages/             one per navigation item
  components/        shared UI (ReviewDialog, FileTable, ...)
  lib/               bridge wrappers, hooks, theme
  styles/theme.css   design tokens (the only place colours are defined)
src-tauri/
  src/commands/      thin Tauri command entry points
  src/scanner/       walking, metadata, incremental change detection
  src/index/         SQLite schema and queries
  src/duplicates/    size → partial hash → full hash grouping
  src/cleanup/       quarantine, restore, operation log
  capabilities/      least-privilege Tauri permissions
```

The index lives in the per-user app data directory. File contents never leave the
machine.

## Status

Release 1 is functional. Feature details live in `AGENTS.md` and
`docs/feature-spec.pdf`.
