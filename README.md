# copy-diff

Cross-VCS commit replay desktop tool (Tauri + React).

Replay selected commits from a source repository (SVN/Git) into a target repository with full diff preview, fail-stop policy, and audit logging.

## Prerequisites

- Node.js 18+
- Rust (MSVC toolchain on Windows: `rustup default stable-x86_64-pc-windows-msvc`)
- `git` and `svn` CLI on PATH

## Development

```bash
npm install
npm run tauri dev
```

## Quality checks

```bash
npm test              # Vitest (frontend)
npm run lint          # ESLint
npm run format:check  # Prettier
cd src-tauri && cargo test   # Rust unit tests
cd src-tauri && cargo clippy # Rust lints
```

## Project layout

- `src/` — React frontend (pages, components, `lib/invoke.ts`)
- `src-tauri/src/` — Rust backend (model, VCS adapters, commands)
