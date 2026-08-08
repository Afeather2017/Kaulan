# Standalone Server Builds

The standalone server packages contain Kaulan's Rust HTTP backend without the
desktop UI or Tauri shell. They are intended for a machine that hosts a music
library and is accessed through a browser or another Kaulan client.

Four portable server artifacts are published for each release:

- Linux x86_64 AppImage
- Linux ARM64 AppImage
- Windows x86_64 `.zip`
- Windows ARM64 `.zip`

The Linux AppImages and Windows archives include the production frontend and
the FFmpeg runtime libraries used by the backend, so no separate frontend
build or FFmpeg installation is needed. These packages do not install a
desktop file, icon, service, or other host configuration.

## Start the server

On Linux, make the downloaded AppImage executable and run it directly:

```bash
chmod +x kaulan-server-linux-x86_64-<version>.AppImage
./kaulan-server-linux-x86_64-<version>.AppImage run /path/to/music
```

On Windows, extract the archive and run:

```bash
kaulan-server.exe run C:\path\to\music
```

The server listens on port `2080` by default. The music directory can also be
provided through `KAULAN_MUSIC_DIR` or the Kaulan config file. See the main
README for the complete configuration and standalone provider-auth options.

The bundled frontend is served at `/`. A separately built frontend can override
it by setting `KAULAN_FRONTEND_DIST`.

## Build flow

The release matrix builds natively on Linux and Windows x86_64/ARM64 runners.
[`scripts/package-server-linux.sh`](../scripts/package-server-linux.sh) uses
`linuxdeploy` to collect the server's runtime dependency graph into an
AppImage. Windows release archives copy `frontend/dist` as `frontend/` beside
the server executable and copy the matching FFmpeg DLLs beside it.
