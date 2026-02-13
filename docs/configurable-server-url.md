# Configurable Server URL

## Overview

The configurable server URL feature allows users to specify a custom backend server address through the settings panel. This is useful for:

- Connecting to a remote server instead of localhost
- Development environments with different backend addresses
- Testing against staging/production servers

The server URL is saved to browser cookies and persists across page reloads.

## User Instructions

### Setting a Custom Server URL

1. Open the Settings panel (齿轮 icon)
2. Scroll to the "服务器地址" (Server Address) section
3. Enter your server URL in the format: `http://your-server:port/api`
4. Click "保存地址" (Save Address)
5. The page will reload and connect to your custom server

### Resetting to Default

1. Open the Settings panel
2. In the Server Address section, click "重置为默认" (Reset to Default)
3. Confirm the reset
4. The page will reload and connect to `http://localhost:2080/api`

## Validation Behavior

The URL input validates the format as you type:

- **Empty input**: Not allowed (shows validation error)
- **Invalid format**: Shows error message (e.g., "Invalid URL format")
- **Non-HTTP/HTTPS protocol**: Shows error (must use http:// or https://)
- **Valid URL**: Input border turns green, save button becomes enabled

## Technical Details

### Data Flow

```mermaid
sequenceDiagram
    participant User
    participant SettingsModal
    participant CookieUtils
    participant ApiUtils
    participant Browser

    User->>SettingsModal: Opens settings
    SettingsModal->>ApiUtils: getApiBase()
    ApiUtils->>CookieUtils: getServerUrl()
    CookieUtils->>Browser: Read cookie 'kaulan_server_url'
    Browser-->>CookieUtils: Return value or empty
    CookieUtils-->>ApiUtils: Return URL or default
    ApiUtils-->>SettingsModal: Display URL

    User->>SettingsModal: Enters custom URL
    User->>SettingsModal: Clicks Save
    SettingsModal->>SettingsModal: validateServerUrl()
    SettingsModal->>ApiUtils: setApiBase(url)
    ApiUtils->>CookieUtils: setServerUrl(url)
    CookieUtils->>Browser: Set cookie 'kaulan_server_url'
    SettingsModal->>Browser: window.location.reload()

    Browser->>ApiUtils: getApiBase()
    ApiUtils->>CookieUtils: getServerUrl()
    CookieUtils->>Browser: Read cookie 'kaulan_server_url'
    Browser-->>CookieUtils: Return saved URL
    CookieUtils-->>ApiUtils: Return saved URL
    ApiUtils-->>Browser: Use saved URL for all API calls
```

### Cookie Storage

| Property | Value |
|----------|-------|
| Cookie Name | `kaulan_server_url` |
| Expiration | 365 days |
| Storage | Browser cookies (httpOnly: false, secure: false) |

### File Structure

```
frontend/src/
├── utils/
│   ├── api.ts           # Dynamic API base with cookie support
│   ├── cookies.ts       # Cookie CRUD operations
│   └── validation.ts    # URL validation logic
├── components/modals/
│   └── SettingsModal.vue   # UI for server URL configuration
└── composables/
    ├── useAudioPlayer.ts    # Uses getApiBase()
    └── usePlaylist.ts       # Uses getApiBase()
```

### API Changes

All API consumers now use `getApiBase()` instead of the static `API_BASE` constant:

**Before:**
```typescript
import { API_BASE } from '@/utils/api'
const response = await fetch(`${API_BASE}/music`)
```

**After:**
```typescript
import { getApiBase } from '@/utils/api'
const response = await fetch(`${getApiBase()}/music`)
```

### Related Source Files

- `frontend/src/utils/api.ts` - API base URL configuration
- `frontend/src/utils/cookies.ts` - Cookie operations
- `frontend/src/utils/validation.ts` - URL validation
- `frontend/src/components/modals/SettingsModal.vue` - Settings UI
- `frontend/src/composables/useAudioPlayer.ts` - Audio player API calls
- `frontend/src/composables/usePlaylist.ts` - Playlist API calls
- `frontend/src/components/modals/UploadModal.vue` - Upload API calls
- `frontend/src/views/Home.vue` - Home API calls
- `frontend/src/views/Library.vue` - Library API calls
- `frontend/src/views/Playlists.vue` - Playlists API calls

## Android Cleartext Traffic

To support HTTP URLs (not just HTTPS) on Android, the network security configuration has been updated:

**File:** `frontend/src-tauri/gen/android/app/src/main/res/xml/network_security_config.xml`

The `base-config` now has `cleartextTrafficPermitted="true"` to allow HTTP traffic to any domain.

**Security Note:** This configuration allows cleartext HTTP traffic to all domains. For production releases, consider restricting this to specific domains or enforcing HTTPS.

## Default URL

- **Default**: `http://localhost:2080/api`
- **Fallback**: If no cookie is set, uses default automatically

## API Endpoints

All API endpoints are relative to the configured base URL:

- `GET /api/music` - Get all music
- `GET /api/music/{filename}` - Stream audio
- `GET /api/playlists` - Get playlists
- `GET /api/collections` - Get collections
- `POST /api/collections` - Create collection
- etc.
