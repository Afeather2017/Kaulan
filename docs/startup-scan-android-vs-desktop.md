# Startup Scan: Android vs Desktop

This document explains why startup scanning differs between desktop and Android, and how the permission-gated scan works on Android.

## Why the flows differ

- Desktop platforms can scan files immediately at startup because filesystem access does not require a runtime permission prompt.
- Android requires runtime storage permissions before MediaStore can return audio files. If the backend scans before permissions are granted, it will see zero files and incorrectly initialize the database as empty.

## Desktop flow (startup scan enabled)

1. Backend starts and checks the `initial_scan_done` flag in the database.
2. If not done, backend performs `initialize_database()` in the background.
3. Scan completes and the flag is set to avoid re-scanning on every launch.

Relevant code:
- `backend/src/server/mod.rs`
- `backend/src/services/scanner.rs`

## Android flow (permission-gated scan)

1. Backend starts but skips the startup scan.
2. Frontend calls `POST /api/database/update` on startup.
3. MediaStore plugin requests permissions internally if needed.
4. Backend scans via MediaStore and inserts files.

Relevant code:
- `frontend/src/App.vue`
- `backend/src/handlers/database.rs`
- `backend/src/services/scanner.rs`

## Manual rescan

Users can trigger a rescan at any time via the Settings "Update Database" action, which calls:
- `POST /api/database/update`

Relevant UI code:
- `frontend/src/components/modals/SettingsModal.vue`
