# Startup Scan Behavior

This document describes how Kaulan performs the initial music scan when the backend starts.

## Summary

- The backend performs **one automatic scan** on the first run.
- On subsequent runs, the startup scan is skipped.
- Users must trigger `POST /api/database/update` to rescan.

## How It Works

Kaulan stores a single-row metadata table `db_meta` with the flag `initial_scan_done`.

1. On startup, the server reads `db_meta.initial_scan_done`.
2. If it is `false`, the server runs `initialize_database()` and then sets the flag to `true`.
3. If it is `true`, the server skips the startup scan.

## Related Source Files

- `backend/src/server/mod.rs`
- `backend/src/services/scanner.rs`
- `backend/src/database/mod.rs`
- `backend/src/entities/db_meta.rs`
