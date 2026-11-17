# AGENTS.md — Digital Mess Cleaner

Guidance for coding agents working in this repo. Read fully before making changes.

## What this is

A desktop app for **Windows and macOS** (Tauri v2) that helps people find, understand, organise and safely clean their digital files from one trusted workspace.
Tagline: *Your files. Understood. Organised. Clean.*

Feature spec: `docs/feature-spec.pdf`. If this file and the spec disagree, ask before proceeding.

## Stack

- Shell + backend: Tauri v2, Rust (stable)
- Frontend: React + TypeScript + Vite
- Index/storage: SQLite in the app data directory (FTS5 for text search)
- Styling: plain CSS tokens in `src/styles/theme.css`; theme logic in `src/lib/theme.ts`

## Commands

```bash
npm install
npm run tauri dev          # run the app
npm run tauri build        # production bundle for the current OS
npm run typecheck && npm run lint && npm test

cd src-tauri
cargo fmt
cargo clippy -- -D warnings
cargo test
```

## Layout

```
src/                 React UI
  pages/             one per nav item (see Navigation)
  components/        shared UI (ReviewDialog, FileRow, StorageBar, ...)
  lib/               theme.ts, tauri command wrappers, helpers
  styles/theme.css   design tokens — the only place colors/radii/spacing are defined
src-tauri/
  src/commands/      #[tauri::command] entry points (thin)
  src/scanner/       walking, metadata, change detection
  src/index/         SQLite schema, queries
  src/duplicates/    size → partial hash → full hash grouping
  src/cleanup/       quarantine, restore, operation log
  capabilities/      Tauri permissions (least privilege)
```

## Scope: build in release order

Do not start a later release until the earlier one is dependable. Do not add features outside the current release unless asked.

1. **Release 1 (current):** scanner/indexer, storage dashboard, metadata search, large and old file finder, exact duplicates, similar images, basic categories, safe cleanup centre, quarantine and restore.
2. Release 2: automatic categorisation, folder suggestions, Downloads organiser, bulk moves, renaming, rules, full-text search, OCR, semantic search.
3. Release 3: summaries, document Q&A, invoice/receipt/resume extraction, comparison, source references.
4. Release 4: folder monitoring, scheduled reports, video/audio duplicates, external drives, cloud connectors.

## Safety rules (never break these)

1. **Never delete, move or rename a file without showing a preview and getting explicit user approval.** AI or heuristic guesses never act on their own.
2. **Quarantine before permanent delete.** Selected files go to an app-managed holding area with a manifest so they can be restored to the original path. Permanent delete is a separate, clearly labelled step.
3. **Every file operation is logged** (what, from, to, when, reversible or not) and undoable where the OS allows. Say plainly when an action cannot be reversed.
4. **Only scan folders and drives the user has chosen.** Never walk the whole disk by default.
5. **Exact duplicates mean byte-identical.** Group by size, then partial hash, then full cryptographic hash (BLAKE3 or SHA-256). Similar images and near-duplicates are shown as *similar*, never as exact.
6. **Local-first.** File contents are processed on the device. Any feature that sends content off-device must be opt-in and clearly labelled in the UI.
7. **Show evidence and confidence** for every classification, grouping or recommendation. Flag low-confidence OCR/extraction and ask the user to verify.
8. **Tests never touch real user data.** Use temp directories only.

## Rust conventions

- All filesystem access happens in Rust commands. The webview never gets broad fs access.
- Long work (scan, hash, index) runs off the main thread, reports progress via events, and is cancellable.
- Be incremental: skip files whose size and mtime are unchanged.
- Never panic on user files. Handle permission denied, locked files, symlink loops, vanished files, very long Windows paths, and case-insensitive name conflicts.
- Use `PathBuf`, never string-concatenated paths. Account for macOS folder-permission prompts and Windows path length limits.
- Typed errors (`thiserror`) returned to the UI as `{ code, message }`.

## Frontend conventions

- **Theme:** use tokens from `theme.css` only. No hardcoded colors, radii or font stacks in components. Every screen must look right in light and dark, and on both macOS and Windows (`data-platform` on `<html>`).
- Use `.btn-caution` for quarantine, `.btn-danger` only for permanent delete.
- Never use color alone to convey state; pair it with text or an icon.
- Large file lists must be virtualised.
- Every destructive or move action goes through the shared `ReviewDialog` showing item count, total size, and what happens next (and whether it is reversible).
- Keyboard accessible everywhere, visible focus ring, WCAG AA contrast.
- Titlebar element carries `data-tauri-drag-region`.
- Copy is plain language: say what will happen to the files.

## Navigation

Dashboard · Smart Search · My Files · Duplicates · Media Cleanup · Smart Organizer · Document Intelligence · Ask My Files · Cleanup Centre · Storage Insights.
Only Release 1 screens are functional now; later ones may exist as empty states.

## Tauri security

- Least-privilege permissions in `src-tauri/capabilities/`. No wildcard fs or shell scopes.
- Strict CSP. No remote scripts or content.
- No network calls by default.
- `core:window:allow-set-theme` is the only extra window permission the theme needs.

## Quality targets

- Exact duplicate detection: 100% precision.
- Search: under 2 seconds on an established local index.
- Zero accidental permanent deletions in validation.
- Organisation suggestions: at least 85% acceptance in pilot.
- Remembered file found within 10 seconds in usability tests.

## Definition of done

- Typecheck, lint, `cargo clippy -D warnings`, and tests pass.
- Checked in light and dark, and with macOS and Windows platform attributes.
- Tried against: empty folder, permission denied, 100k+ files, name conflicts, file changed mid-scan.
- No hardcoded colors; no new dependency without a reason in the PR description.
- Safety rules above still hold.
