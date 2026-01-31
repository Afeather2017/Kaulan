# Os environment

apt install libglib2.0-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libwebkit2gtk-4.1-dev libssl-dev libglib2.0-dev libgtk-3-dev zsh 

# Frontend


npm should be prepared.

```
# Download and install nvm:
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
# in lieu of restarting the shell
\. "$HOME/.nvm/nvm.sh"
# Download and install Node.js:
nvm install 24
# Verify the Node.js version:
node -v # Should print "v24.13.0".
# Verify npm version:
npm -v # Should print "11.6.2".
```

Then prepare npm dependencies:

```
cd frontend/
npm install
```

**WARN**: Close all proxy settings so that frontend could works! Say http_proxy environment variable and proxy settings in KDE.

## frontend -- website mode

```
cd frontend/
npm run build
# or npm run dev
```

## frontend -- binary distribution

```
cd frontend/
npx tauri build
# or npx tauri dev
```

# Backend

rust should be prepared

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then:

```
cd backend/
cargo build
# say music files are located in ~/Music
# update the database
cargo run update ~/Music
# run the server
cargo run run ~/Music
```
