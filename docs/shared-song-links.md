# Shared Song Links

Kaulan can open a specific song directly in the backend-served web player by
using a query-arg share link:

```text
http://server_ip:2080/?id=42
```

Related source files:

- `frontend/src/utils/sharedLink.ts`
- `frontend/src/composables/useAppShell.ts`
- `frontend/src/composables/useLibrarySources.ts`
- `frontend/src/composables/useAudioPlayer.ts`
- `frontend/src/App.vue`

## Behavior

- The backend serves the same `index.html` shell at `/`.
- The frontend reads `window.location.search` and parses `id`.
- The frontend treats `window.location.origin + "/api"` as the session-local
  source for this page load.
- The app refreshes library sources, finds the requested song id, opens the
  containing playlist, shows the player panel, and attempts playback.
- If the browser blocks autoplay, the song stays selected and the page shows a
  manual play button.
- If the `id` value is invalid or the song is missing, the page remains usable
  and shows an in-app status message.

## Link Format

| URL | Result |
| --- | --- |
| `/?id=42` | Load song `42` from the current server and try to play it |
| `/?id=abc` | Show invalid-link message and keep the app usable |
| `/` | Normal app startup without shared playback intent |

Song sharing is id-based only. There is no filename fallback contract.

## Request Flow

```mermaid
sequenceDiagram
    participant Browser
    participant Backend as Actix Backend
    participant Frontend as Vue App
    participant API as Backend API

    Browser->>Backend: GET /?id=42
    Backend-->>Browser: index.html
    Browser->>Frontend: Boot app
    Frontend->>Frontend: Parse window.location.search
    Frontend->>Frontend: Set session API base to window.location.origin + /api
    Frontend->>API: POST /api/database/update?startup=true
    Frontend->>API: GET /api/discovery/self
    Frontend->>API: GET /api/playlists
    API-->>Frontend: Device info + playlists
    Frontend->>Frontend: Resolve song id and open player panel
    Frontend->>Browser: Attempt HTMLAudioElement.play()
    alt autoplay allowed
        Browser-->>Frontend: playback starts
    else autoplay blocked
        Browser-->>Frontend: play() rejected
        Frontend->>Frontend: Show manual play button
    end
```

## API Notes

This feature does not add new JSON endpoints. It reuses the existing routes:

- `POST /api/database/update?startup=true`
- `GET /api/discovery/self`
- `GET /api/playlists`
- `GET /api/music/id/{id}`

## UI Notes

- The link opens the normal Kaulan web player, not a separate share page.
- The shared server is labeled as `This Device` for that browser session.
- After startup, the frontend removes `id` from the address bar with
  `history.replaceState()` so the share intent is only consumed once per page
  load.
