# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Kaulan is a music player with a Rust Actix Web backend and Vue.js (TypeScript) frontend. The app features mobile-first UI, audio streaming, folder-based playlists, user-defined collections (favorites), and LUFS volume normalization.

## Develop steps

### New feature

A new feature need change backend and frontend. You should:

#### Check the feature

1. Think if the feature should be provided in the project, if not, refuse.
2. If we don't provide the way we use UI, you should stop from generate. Unless it is nothing to do with UI.

#### Get things done.

1. Figure out what API should be provided, the JSON format, etc. And, use RESTful mostly.
2. Design the database tables.
3. Generate the unit test for the backend. This makes sure you make the API works.
4. After backend done, the test pass, implement the frontend.
5. Brief check. Use curl to access APIs, to check if things work.

### Documentations

Documentations should always in English.

1. README.md, briefly introduced the project, say key features.
2. docs/ , which introduce the feature, how features works(in sequence diagram), how user to use the UI. The features introducton should include all API.
3. API reference, use rust-builtin reference system, this should be done in comments.
4. CLAUDE.md, Introduce the project framework by path.

### Log

The log system of rust should be tracing. and the ts/js, console.log is enough.

Rules:

| Scenario / Purpose | Recommended Log Level | Explanation & Example |
| ------------------ | --------------------- | --------------------- |
| 1. Development Environment Debugging | DEBUG or TRACE | Used during development to track the program's execution path, internal state, and variable values. Essential for debugging. |
| 2. Production Environment Routine Operation | INFO or WARN | Higher levels are used in production to reduce performance overhead and prevent important messages from being drowned out by less important ones. INFO records key system status and business processes. |
| 3. Recording Key Business Information | INFO | Used for important business milestones such as user login, order creation, or successful payment. This is useful for business analysis and auditing. Example: `User ID:{} logged in successfully`. Music files are large, log if someone access it. |
| 4. Recording Potential Issues | WARN | Used when the system encounters an abnormal situation that can be automatically recovered from or does not immediately affect core functionality. Examples: using a default configuration value, an API call timeout that was retried successfully. Alerts developers to a situation that may need attention but does not require immediate intervention. |
| 5. Recording Errors and Exceptions | ERROR | Used when a system function is impacted and a manual intervention is required. Key point: When logging exceptions, always output the complete exception stack trace, not just `e.getMessage()`, for quick root cause analysis. Example: `An exception occurred while processing user ID:{}`. |
| 6. Controlling Log Volume (in loops or batch operations) | INFO (with conditional control) | Avoid logging on every iteration within a loop. Use conditional checks (e.g., log every 1000 records) or concatenate messages with `StringBuilder`and log once after the loop to reduce I/O pressure. |
| 7. Logging Method Entry/Exit and Call Chains | DEBUG / INFO | For important methods, log parameters at the entry and results at the exit (DEBUG). For long operations, ensure each step in the chain has a log (INFO) to trace the problematic link. The article suggests using AOP for uniform implementation. |
| 8. Avoiding Sensitive Information | All Levels | General Rule: Regardless of the level, log content must never contain sensitive information like user passwords or ID numbers to prevent security issues in case of a log leak. |
| 9. Dynamic Log Level Adjustment | Adjustable | If a problem occurs in production but the current log level (e.g., INFO) doesn't provide enough information, you can temporarily lower the level (e.g., to DEBUG) to get more detailed logs for troubleshooting. |

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
├── main.rs          # Binary entry point
├── lib.rs           # Library with API endpoints and business logic
├── database/mod.rs  # SQLite connection, table creation
├── entities/        # SeaORM entities
│   ├── music.rs           # Music table (id, filename, file_path, lufs, created_at)
│   ├── collection.rs      # Collection table (id, name, created_at)
│   ├── collection_item.rs # Collection-Item junction table (id, collection_id, music_id, created_at)
│   ├── mod.rs
│   └── prelude.rs
└── lufsgen.rs       # FFmpeg-based LUFS analysis utility
```

**Key Architecture Points:**
- Uses SeaORM with SQLite for persistence
- Database file: `music.db` (auto-created)
- `music` table stores all audio files with LUFS values
- `collection` table stores user-defined collections (favorites/playlists)
- `collection_item` table provides many-to-many relationship between collections and music
- Backend scans music directory on startup via `initialize_database()`
- Two view modes: folder-based playlists and user-defined collections

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

**Note:** `App.vue` contains the actual player implementation including collections feature. `Library.vue` and `Playlists.vue` are older stub files.

### Data Flow

**Folder Mode (default):**
1. Backend scans music directory on startup → populates `music` table
2. Frontend calls `GET /api/playlists` → gets folder-based playlist structure
3. Frontend calls `GET /api/music/{filename}` → streams audio file
4. LUFS values are stored in DB

**Collection Mode:**
1. Frontend calls `GET /api/collections` → gets all user-defined collections
2. Frontend calls `GET /api/playlists/collection-mode` → gets playlists keyed by collection names
3. Frontend calls `GET /api/collections/{id}/items` → gets songs in a specific collection
4. Users can create/delete collections via `POST /api/collections` and `DELETE /api/collections/{id}`
5. Users can add/remove songs via `POST /api/collections/{id}/items` and `DELETE /api/collections/{id}/items`

## Code Standards (from AGENTS.md)

1. Avoid deeply nested code
2. For long calling chains, draw Sequence Diagrams in Mermaid
3. Use English for code/comments unless instructed to translate
4. Always use strict mode in TypeScript and JavaScript
5. When you need to notify me, like confirm something: run `zenity --info --text="AI notify you"`
6. Ensure both frontend and backend compile and run before finishing

## API Endpoints

### Music Endpoints
- `GET /api/music/{filename}` - Stream audio file
- `GET /api/music` - Get all music from database

### Playlist Endpoints (Folder-based)
- `GET /api/playlists` - Get all playlists (folder-based)
- `GET /api/playlists/{name}` - Get specific playlist by name

### Collection Endpoints
- `GET /api/collections` - Get all collections
- `GET /api/collections/{id}` - Get single collection metadata (without songs)
- `GET /api/collections/{id}/items` - Get collection with its songs
- `POST /api/collections` - Create new collection
- `DELETE /api/collections/{id}` - Delete collection (also removes all associated items)
- `POST /api/collections/{id}/items` - Add songs to collection
- `DELETE /api/collections/{id}/items` - Remove songs from collection

### Collection Mode Endpoints
- `GET /api/playlists/collection-mode` - Get playlists in collection mode (returns HashMap with collection names as keys)

**Note:** Route order matters in Actix-web. Specific routes like `/api/playlists/collection-mode` and `/api/collections/{id}/items` must be registered before parameterized routes like `/api/playlists/{name}` and `/api/collections/{id}`.

### Other Endpoints
- `POST /api/generate-lufs` - Generate LUFS values via FFmpeg (debug mode only)

## Known Issues / Work in Progress

- Frontend `App.vue` uses `/api/music/{filename}` for streaming. The music lookup is filename-based which works but is inconsistent with the DB's ID-based primary keys.
- `Library.vue` and `Playlists.vue` views reference non-existent endpoints and are outdated stubs. The actual implementation is in `App.vue`.

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
