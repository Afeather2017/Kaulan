import { getLocalApiBase, normalizeApiBase } from "@/utils/api";
import {
  getManualDevices,
  setManualDevices,
  type ManualDevice,
} from "@/utils/storage";

export interface DiscoveredDevice {
  device_id: string;
  device_name: string;
  api_url: string;
  last_seen_secs_ago: number;
  isManual?: boolean;
}

export interface SelfDevice {
  device_id: string;
  device_name: string;
}

interface OperationResponse {
  success: boolean;
  message: string;
}

const SCAN_SECONDS = 3;
const SCAN_INTERVAL_MS = 1000;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

export async function fetchDiscoveredDevices(): Promise<DiscoveredDevice[]> {
  const response = await fetch(`${getLocalApiBase()}/discovery/devices`);
  if (!response.ok) {
    throw new Error(`Failed to fetch devices: ${response.statusText}`);
  }

  return response.json();
}

export async function refreshDiscoveredDevices(): Promise<DiscoveredDevice[]> {
  let scanStarted = false;

  try {
    const startResponse = await fetch(
      `${getLocalApiBase()}/discovery/scan/start`,
      {
        method: "POST",
      },
    );
    if (!startResponse.ok) {
      throw new Error(`Failed to start scan: ${startResponse.statusText}`);
    }
    scanStarted = true;

    for (let i = 0; i < SCAN_SECONDS; i += 1) {
      const requestResponse = await fetch(
        `${getLocalApiBase()}/discovery/request`,
        {
          method: "POST",
        },
      );

      if (!requestResponse.ok) {
        throw new Error(
          `Failed to send discovery request: ${requestResponse.statusText}`,
        );
      }

      const requestResult: OperationResponse = await requestResponse.json();
      if (!requestResult.success) {
        throw new Error(requestResult.message);
      }

      if (i < SCAN_SECONDS - 1) {
        await sleep(SCAN_INTERVAL_MS);
      }
    }

    const finishResponse = await fetch(
      `${getLocalApiBase()}/discovery/scan/finish`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ success: true }),
      },
    );

    if (!finishResponse.ok) {
      throw new Error(`Failed to finish scan: ${finishResponse.statusText}`);
    }

    return fetchDiscoveredDevices();
  } catch (error) {
    if (scanStarted) {
      try {
        await fetch(`${getLocalApiBase()}/discovery/scan/finish`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ success: false }),
        });
      } catch (rollbackError) {
        console.error("Failed to rollback discovery scan:", rollbackError);
      }
    }

    throw error;
  }
}

export async function fetchSelfDeviceInfo(
  apiBase: string,
): Promise<SelfDevice | null> {
  try {
    const normalizedUrl = normalizeApiBase(apiBase);
    const response = await fetch(`${normalizedUrl}/discovery/self`);
    if (!response.ok) {
      return null;
    }

    const data = await response.json();
    if (
      typeof data.device_id !== "string" ||
      typeof data.device_name !== "string"
    ) {
      return null;
    }

    return data;
  } catch (error) {
    console.warn(`Failed to fetch device info from ${apiBase}:`, error);
    return null;
  }
}

function mergeManualDevice(
  device: ManualDevice,
  discoveredById: Map<string, DiscoveredDevice>,
  selfInfo: SelfDevice | null,
): ManualDevice {
  const discoveredMatch = device.device_id
    ? discoveredById.get(device.device_id)
    : null;

  if (discoveredMatch) {
    return {
      ...device,
      api_url: discoveredMatch.api_url,
      device_name: discoveredMatch.device_name,
      device_id: discoveredMatch.device_id,
      last_fetched: Date.now(),
    };
  }

  if (selfInfo) {
    return {
      ...device,
      device_id: selfInfo.device_id,
      device_name: selfInfo.device_name || device.device_name,
      last_fetched: Date.now(),
    };
  }

  return device;
}

export async function refreshStoredManualDevices(
  discoveredDevices: DiscoveredDevice[],
): Promise<ManualDevice[]> {
  const manualDevices = getManualDevices();
  const discoveredById = new Map(
    discoveredDevices.map((device) => [device.device_id, device]),
  );

  const updated = await Promise.all(
    manualDevices.map(async (device) => {
      const selfInfo = await fetchSelfDeviceInfo(device.api_url);
      return mergeManualDevice(device, discoveredById, selfInfo);
    }),
  );

  setManualDevices(updated);
  return updated;
}
