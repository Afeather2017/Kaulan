# Kaulan - Music Player

A modern music player built with Rust (Actix Web) backend and Vue.js (TypeScript) frontend. Features a mobile-friendly interface with playlist management, audio streaming, and LUFS volume normalization.

## Features

- **Music Library Management** - Automatically scans and organizes music files
- **Mobile-First Design** - Responsive UI optimized for mobile devices
- **Audio Streaming** - Direct streaming from server to browser
- **File System Playlists** - Automatic playlist creation from folder structure
- **Collection Management** - User-defined playlists/collections
- **Volume Normalization** - LUFS support for consistent audio levels
- **Real-time Search** - Search across all songs instantly

## Quick Start

### Prerequisites

- Rust 1.70+ and Cargo
- Node.js 18+ and npm
- FFmpeg (for LUFS calculation during database updates)
- Music files organized in folders

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd kaulan

# Install frontend dependencies
cd frontend
npm install

# Build backend (will download dependencies on first run)
cd ../backend
cargo build
```

### Running the Application

```bash
cd backend

# Run with default music directory (~/Music or ./music)
cargo run run /path/to/music

# Or with a custom music directory
cargo run run /absolute/path/to/your/music
```

The backend API will start on `http://localhost:2080`

In a separate terminal, start the frontend:

```bash
cd frontend
npm run dev
```

The frontend will be available at `http://localhost:3000`

## How to Use

### 1. Setting Up Your Music Library

Organize your music files in folders. Each folder becomes a playlist:

```
music/
├── Rock/
│   ├── song1.mp3
│   └── song2.flac
├── Jazz/
│   ├── track1.ogg
│   └── track2.wav
└── Classical/
    └── piece.mp3
```

### 2. Starting the Server

```bash
cd backend
cargo run run ~/Music
```

On first run, the server will:
- Create `music.db` SQLite database in your music directory
- Scan all audio files recursively
- Insert new files into the database

### 3. Updating the Database

To scan for new files and calculate LUFS values:

```bash
cd backend
cargo run update ~/Music
```

This command will:
- Scan for new files and add them to the database
- Calculate LUFS values for files without proper values
- Remove database entries for deleted files

### 4. Using the Web Player

1. Open `http://localhost:3000` in your browser
2. Select a playlist from the sidebar
3. Click on a song to start playback
4. Use the search bar to filter songs
5. Control playback with the audio player at the bottom

## API Reference

### Base URL

```
http://localhost:2080/api
```

### Endpoints

#### GET /api/music

Get all music files from the database.

**Response:** `MusicResponse[]`

```json
[
  {
    "id": 1,
    "filename": "song.mp3",
    "file_path": "Rock/song.mp3",
    "lufs": -14.5,
    "created_at": "2024-01-15T10:30:00Z"
  }
]
```

#### GET /api/music/{filename}

Stream an audio file by filename.

**Parameters:**
- `filename` (path parameter) - The filename to stream

**Response:** Audio file binary data (audio/mpeg)

**Headers:**
- `Content-Type: audio/mpeg`
- `Cache-Control: no-store, no-cache, must-revalidate, max-age=0`

#### GET /api/playlists

Get all playlists with their songs.

**Response:** `Record<string, MusicInfo[]>`

```json
{
  "所有音乐": [
    {
      "name": "song.mp3",
      "lufs": -14.5,
      "path": "Rock/song.mp3"
    }
  ],
  "Rock": [
    {
      "name": "song.mp3",
      "lufs": -14.5,
      "path": "Rock/song.mp3"
    }
  ]
}
```

Note: "所有音乐" (All Music) is a special playlist containing all songs.

#### GET /api/playlists/{name}

Get a specific playlist by name.

**Parameters:**
- `name` (path parameter) - The playlist name (folder name or "所有音乐")

**Response:** `Playlist`

```json
{
  "name": "Rock",
  "songs": [
    {
      "name": "song.mp3",
      "lufs": -14.5,
      "path": "Rock/song.mp3"
    }
  ]
}
```

#### GET /api/settings/music-directory

Get the current music directory path.

**Response:** `MusicDirectoryResponse`

```json
{
  "path": "/path/to/music"
}
```

#### POST /api/database/update

Trigger a database update to scan for new files, update LUFS values, and remove deleted files.

**Response:** `UpdateResponse`

```json
{
  "success": true,
  "message": "Database updated successfully"
}
```

### Data Types

```typescript
// Music response with database metadata
interface MusicResponse {
  id: number;
  filename: string;
  file_path: string;
  lufs: number | null;
  created_at: string;  // ISO 8601 datetime
}

// Music info for playback
interface MusicInfo {
  name: string;
  lufs: number;
  path: string;
}

// Playlist structure
interface Playlist {
  name: string;
  songs: MusicInfo[];
}
```

## Database Schema

### music Table

The application uses SQLite with SeaORM. The database file (`music.db`) is created in your music directory.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY, AUTO INCREMENT | Unique identifier |
| `filename` | TEXT | NOT NULL | Original filename |
| `file_path` | TEXT | UNIQUE, NOT NULL | Relative path from music directory |
| `lufs` | REAL | NULLABLE | LUFS value for volume normalization |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp (UTC) |

### Database Location

```
<path-to-music-directory>/music.db
```

### Database Operations

The application performs these operations automatically:

- **On server start**: Scans for new files and inserts them (if `file_path` doesn't exist)
- **On update command**: Scans for new/deleted files, calculates LUFS, updates existing entries

## Supported Audio Formats

- MP3 (`.mp3`)
- OGG Vorbis (`.ogg`)
- WAV (`.wav`)
- AAC (`.aac`)
- FLAC (`.flac`)

## Configuration

### Backend Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| Port | 2080 | HTTP server port |
| Bind Address | 0.0.0.0 | Server bind address |
| Music Directory | `~/Music` or `./music` | Path to music files |
| Database | `music.db` | SQLite database in music directory |

### Frontend Configuration

The frontend uses Vite with a proxy to the backend:

```typescript
// vite.config.ts
server: {
  port: 3000,
  proxy: {
    '/api': 'http://localhost:2080'
  }
}
```

## Development

### Backend Development

```bash
cd backend

# Type check without building
cargo check

# Run tests
cargo test

# Build release version
cargo build --release

# Run with debug logging
RUST_LOG=debug cargo run run ~/Music
```

### Frontend Development

```bash
cd frontend

# Development server with hot reload
npm run dev

# Type checking
npm run check

# Build for production
npm run build

# Preview production build
npm run preview

# Run tests
npm run test
```

## Architecture

### Backend (Rust/Actix Web)

```
backend/src/
├── main.rs          # HTTP server, API endpoints, file scanning
├── database/mod.rs  # SQLite connection, table creation
├── entities/
│   ├── music.rs     # Music table entity definition
│   └── mod.rs
└── lufsgen.rs       # FFmpeg-based LUFS calculation
```

### Frontend (Vue.js/TypeScript)

```
frontend/src/
├── main.ts          # Application entry point
├── App.vue          # Main music player component
├── router/
│   └── index.ts     # Vue Router configuration
└── views/
    ├── Home.vue     # Dashboard with statistics
    ├── Library.vue  # Music library view
    └── Playlists.vue # Playlist management view
```

## LUFS Volume Normalization

LUFS (Loudness Units Full Scale) values are calculated using FFmpeg and stored in the database. The application uses these values to normalize playback volume across tracks with different mastering levels.

During the `update` command, LUFS is calculated for:
- New files being added to the database
- Existing files with missing or default (0.5) LUFS values

## Troubleshooting

### Audio files not showing up

1. Ensure files have supported extensions (`.mp3`, `.ogg`, `.wav`, `.aac`, `.flac`)
2. Run the update command: `cargo run update ~/Music`
3. Check server logs for database errors

### Database errors

The database file is created automatically in your music directory. If you encounter corruption:

```bash
# Delete and recreate the database
rm ~/Music/music.db
cargo run run ~/Music
```

### FFmpeg not found

LUFS calculation requires FFmpeg. Install it:

```bash
# Arch Linux
sudo pacman -S ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# macOS
brew install ffmpeg
```

## License

MIT License
