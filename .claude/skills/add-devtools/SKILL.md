---
name: add-devtools
description: Add devtools feature to Tauri app. Enable webview devtools for debugging in Tauri apps.
argument-hint: [path-to-tauri-project]
allowed-tools: Read, Edit
---

# Add Devtools to Tauri App

This skill adds the `devtools` feature to the Tauri dependency in Cargo.toml, enabling webview devtools for debugging.

## Instructions

1. Find the `Cargo.toml` file in the `src-tauri` directory at: `$ARGUMENTS/src-tauri/Cargo.toml`
   - If `$ARGUMENTS` is not provided, use `./src-tauri/Cargo.toml`

2. Read the Cargo.toml file

3. Find the `tauri` dependency in `[dependencies]` section. It typically looks like:
   ```toml
   tauri = { version = "2.10.0", features = [] }
   ```

4. Modify the `features` array to include `"devtools"`:
   ```toml
   tauri = { version = "2.10.0", features = ["devtools"] }
   ```

5. If the dependency doesn't have a `features` key, add it

## Note

For Android specifically, the devtools feature is enabled via Cargo.toml, not via `tauri.conf.json`. The `devtools: true` in tauri.conf.json only works for desktop platforms.
