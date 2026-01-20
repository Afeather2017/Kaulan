# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Kaulan is a music player with a Rust Actix Web backend and Vue.js (TypeScript) frontend. The app features mobile-first UI, audio streaming, folder-based playlists, and LUFS volume normalization.

## Development Commands

### Backend (Rust)
```bash
cd backend

# Run development server (with custom or default music directory)
cargo run /path/to/music/directory  # or just `cargo run` for ./music
cargo build                         # Build
cargo check                         # Type check without building
cargo test                          # Run tests
```

### Frontend (Vue.js/TypeScript)
```bash
cd frontend

npm install         # Install dependencies
npm run dev         # Development server (runs on port 3000)
npm run build       # Production build
npm run preview     # Preview production build
npm run test        # Run tests (vitest)
```

### Full Stack Development
- Backend API runs on `http://localhost:2080`
- Frontend dev server runs on `http://localhost:3000`
- Vite proxy forwards `/api` requests to backend (see `frontend/vite.config.ts`)

## Architecture

### Backend Structure
```
backend/src/
├── main.rs          # Actix Web server, API endpoints, file scanning
├── database/mod.rs  # SQLite connection, table creation
├── entities/        # SeaORM entities
│   ├── music.rs     # Music table definition (id, filename, file_path, lufs, created_at)
│   ├── mod.rs
│   └── prelude.rs
└── lufsgen.rs       # FFmpeg-based LUFS analysis utility
```

**Key Architecture Points:**
- Uses SeaORM with SQLite for persistence
- Database file: `music.db` (auto-created)
- `music` table stores all audio files with LUFS values
- Backend scans music directory on startup via `initialize_database()` in `main.rs:185`
- Folder structure in music directory = playlists (not yet implemented in DB)
- LUFS generation endpoint only available in debug mode

### Frontend Structure
```
frontend/src/
├── main.ts          # App entry point
├── App.vue          # Main music player component (contains most UI)
├── router/
│   └── index.ts     # Vue Router configuration
└── views/
    ├── Home.vue     # Dashboard with stats
    ├── Library.vue  # Music library view
    └── Playlists.vue # Playlist view
```

**Note:** `App.vue` contains the actual player implementation. `Library.vue` and `Playlists.vue` appear to be outdated/stubs that reference non-existent API endpoints (`/api/music/songs`, `/api/music/playlists`).

### Data Flow
1. Backend scans music directory on startup → populates `music` table
2. Frontend calls `GET /api/playlists` → gets playlist structure (folder-based)
3. Frontend calls `GET /api/music/{filename}` → streams audio file (note: this is filename-based, not ID-based, despite the DB refactor)
4. LUFS values are stored in DB, loaded from optional `music.info` file

## Code Standards (from AGENTS.md)

1. Avoid deeply nested code
2. For long calling chains, draw Sequence Diagrams in Mermaid
3. Use English for code/comments unless instructed to translate
4. Always use strict mode in TypeScript and JavaScript
5. When you need to notify me, like confirm something: run `zenity --info --text="AI notify you"`
6. Ensure both frontend and backend compile and run before finishing

## API Endpoints

- `GET /api/music/{filename}` - Stream audio file
- `GET /api/playlists` - Get all playlists (folder-based)
- `GET /api/playlists/{name}` - Get specific playlist
- `GET /api/music` - Get all music from database (DB refactored endpoint)
- `POST /api/generate-lufs` - Generate LUFS values via FFmpeg (debug mode only)

## Known Issues / Work in Progress

- Frontend `App.vue` uses `/api/music/{filename}` for streaming, but backend DB refactor uses ID-based access (`/api/music/{id}`). These are inconsistent.
- Playlist functionality uses folder structure but there's no playlist table in the database yet (see DATABASE.md for planned schema).
- `Library.vue` and `Playlists.vue` views reference non-existent endpoints.

## Dependencies

**Backend:**
- actix-web 4
- sea-orm 0.12 with SQLite
- serde, tokio, chrono
- actix-cors

**Frontend:**
- Vue 3 with Composition API
- TypeScript
- Vite
- Vue Router

**External Tools:**
- FFmpeg required for LUFS generation
