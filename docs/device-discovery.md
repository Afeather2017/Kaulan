# Device Discovery

## Overview

Kaulan uses identified UDP requests for on-demand scans and optional periodic
local-network announcements.

Discovery runs in these cases:

- during app startup, before library sources are refreshed
- when the **添加设备** sheet opens
- when the user presses **刷新** in the nearby-device section

Periodic discovery is enabled by default and can be disabled under
**设置 → 设备与来源 → 定期发现附近设备** to reduce background battery use.
Disabling it does not close the UDP listener or disable manual refresh.

For the local device name, backend uses this fallback order:

1. Saved `device_name` from config
2. Hostname, when it is not a generic value such as `localhost`
3. Generated fallback `Kaulan Player <short-device-id>`

## Protocol (Kaulan Discovery Protocol v1.1)

- Transport: UDP IPv4 on port `2082`
- Request target: broadcast `255.255.255.255:2082`
- Response target: unicast back to requester source address/port
- Payload: JSON UTF-8, max 1024 bytes

### Message Types

#### Discovery Request

```json
{
  "type": "kaulan-discovery-request",
  "version": "1.1",
  "device_id": "550e8400-e29b-41d4-a716-446655440000",
  "device_name": "Living Room Player",
  "api_port": 2080,
  "timestamp": 1678912345678
}
```

#### Discovery Response

```json
{
  "type": "kaulan-discovery-response",
  "version": "1.1",
  "device_id": "550e8400-e29b-41d4-a716-446655440000",
  "device_name": "Living Room Player",
  "api_port": 2080,
  "timestamp": 1678912345678
}
```

## Scan Behavior

Every 10 seconds, an enabled backend broadcasts an identified request. A peer
records the requester using the UDP source IP and request metadata, then sends
the normal unicast response. This bidirectional behavior supports router-hosted
servers whose own broadcast packets cannot enter the LAN. Anonymous requests
from older Kaulan versions remain supported.

Passively discovered devices are updated by stable `device_id` when their IP
changes. When an explicit scan commits, devices seen within the previous 30
seconds are merged into the scan result. This grace period prevents a healthy
device from disappearing merely because its 10-second periodic announcement
missed the 3-second scan window. Older devices absent from the scan are removed,
and failed scans restore the complete prior list.

When discovery refresh runs:

1. Frontend calls `POST /api/discovery/scan/start`
2. Frontend calls `POST /api/discovery/request` every 1 second during the scan window
3. Backend listener receives unicast responses and stores them in scan buffer
4. Frontend calls `POST /api/discovery/scan/finish` with `{ "success": true }`; backend commits the scan and retains devices seen in the 30-second grace period
5. Frontend calls `GET /api/discovery/devices` to render final results
6. Frontend reconciles saved manual devices by `device_id` when possible, updating stored API URLs if the device IP changed

If scan fails, frontend calls `POST /api/discovery/scan/finish` with `{ "success": false }` and backend rolls back to pre-scan list.

## Settings UI

- Device list always includes `localhost(self)` (`http://localhost:2080/api`), so users can switch back to local server quickly.
- The Add Device sheet triggers a nearby-device refresh when it opens, so users see current LAN devices instead of only the last committed scan result.
- Manual address input accepts:
  - IP or domain name, which is normalized to `http://host:2080/api`
  - IP or domain name with port, which is normalized to `http://host:port/api`
  - Full HTTP/HTTPS URL, which is normalized to end with `/api`
- Manual address input is required in the Add Device flow. Empty input is rejected instead of falling back to localhost.
- Nearby-device entries are identity-based (`device_id`). If a known device gets a new IP address, discovery can update the saved manual source URL to the new `api_url`.
- Manual address save stores the current `device_id` when the target server responds to `/discovery/self`.
- Non-local manual sources can be removed from the source `⋮` menu. The localhost source is permanent and cannot be deleted.
- In the device list, `last seen` and manual remove action are shown on the same row as the device name, while long API URLs wrap inside the card instead of overflowing.
- The library source page resolves each configured server independently. `localhost` can appear immediately while slow or dead manual servers continue in parallel and fall back to `Offline` after a short request timeout.
- Offline sources stay visible in the library source card list, so users can see the saved server and tap `重试` directly on that card after the source times out or becomes unreachable.

## API Endpoints

### `GET /api/discovery/devices`
Return committed discovered devices list.

### `GET /api/discovery/self`
Return current server's `device_id` and `device_name`.

### `GET /api/discovery/periodic`

Return `{ "enabled": true }` when periodic announcements are enabled.

### `PUT /api/discovery/periodic`

Enable or disable periodic announcements immediately and persist the setting.
Manual discovery remains available.

Request body:

```json
{ "enabled": false }
```

### `POST /api/discovery/name`
Set current server's device name.

### `POST /api/discovery/scan/start`
Start a new manual scan transaction.

### `POST /api/discovery/request`
Send one UDP discovery request packet.

### `POST /api/discovery/scan/finish`
Commit or rollback scan transaction.

Request body:

```json
{ "success": true }
```

## Sequence Diagram

```mermaid
sequenceDiagram
    participant User as User
    participant FE as Frontend
    participant API as Local Backend
    participant UDP as UDP LAN
    participant Peer as Peer Backend

    loop Every 10 seconds when enabled
        API->>UDP: Broadcast identified discovery-request
        UDP->>Peer: Receive identified request
        Peer->>Peer: Upsert API by sender device_id and source IP
        Peer-->>API: Unicast discovery-response
    end

    User->>FE: Click "刷新设备"
    FE->>API: POST /api/discovery/scan/start

    loop 3 times (1s interval)
        FE->>API: POST /api/discovery/request
        API->>UDP: Broadcast discovery-request
        UDP->>Peer: Receive request
        Peer->>UDP: Unicast discovery-response
        UDP->>API: Receive response
        API->>API: Upsert into scan buffer
    end

    FE->>API: POST /api/discovery/scan/finish {success:true}
    FE->>API: GET /api/discovery/devices
    API-->>FE: Committed device list
```

## Implementation Files

- `backend/src/discovery/types.rs`
- `backend/src/discovery/discovery.rs`
- `backend/src/handlers/discovery.rs`
- `frontend/src/composables/useDeviceDiscovery.ts`
- `frontend/src/components/modals/SettingsModal.vue`
