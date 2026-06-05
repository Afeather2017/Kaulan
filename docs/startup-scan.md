# Startup Scan (Unified)

This document explains the unified startup scan flow for both desktop and Android.

## Overview

- Startup scanning is triggered by the frontend with `POST /api/database/update?startup=true`.
- The backend checks the `initial_scan_done` flag before scanning.
- Manual rescan uses `POST /api/database/update` without the `startup` flag and always runs.

## Why a unified flow

- Desktop can scan immediately, but Android requires runtime permissions.
- The MediaStore plugin handles permission requests internally.
- By using a single entry point, we avoid platform-specific code paths in the backend.

## Startup flow

1. Frontend calls `POST /api/database/update?startup=true` on app launch.
2. Backend reads `initial_scan_done` from `db_meta`.
3. If already done, the backend skips the scan.
4. If not done, the backend runs `initialize_database()` and sets `initial_scan_done = true` on success.
5. Frontend starts a local-network discovery refresh in background while refreshing library source groups from the currently saved source URLs.
6. When discovery finishes, the frontend reconciles saved manual devices and refetches only sources whose API URL changed.
7. If permissions are denied or the scan fails, the flag is not set, allowing a retry on next launch.
8. The frontend shows `扫描中...` while the update request is in flight.

Relevant code:
- `frontend/src/App.vue`
- `backend/src/handlers/database.rs`
- `backend/src/services/scanner.rs`

## Manual rescan

Users can trigger a full rescan at any time via Settings → Update Database:
- `POST /api/database/update` (no `startup` flag)

Relevant code:
- `frontend/src/components/modals/SettingsModal.vue`
- `backend/src/handlers/database.rs`
