import { ref } from "vue";
import { getLocalApiBase } from "@/utils/api";
import {
  fetchDiscoveredDevices,
  fetchSelfDeviceInfo,
  refreshDiscoveredDevices,
  type DiscoveredDevice,
  type SelfDevice,
} from "@/utils/discovery";

export interface SetDeviceNameResponse {
  success: boolean;
  message: string;
}

export function useDeviceDiscovery() {
  const devices = ref<DiscoveredDevice[]>([]);
  const selfDevice = ref<SelfDevice | null>(null);
  const isLoading = ref(false);
  const error = ref<string | null>(null);

  /**
   * Fetch all discovered devices from the server.
   */
  const fetchDevices = async (): Promise<void> => {
    devices.value = await fetchDiscoveredDevices();
  };

  /**
   * Refresh devices using manual 10-second request scan.
   *
   * Flow:
   * 1. Start scan transaction
   * 2. Send discovery request every 1 second for 10 seconds
   * 3. Commit scan on success, rollback on failure
   * 4. Fetch committed device list
   */
  const refreshDevices = async (): Promise<void> => {
    isLoading.value = true;
    error.value = null;

    try {
      devices.value = await refreshDiscoveredDevices();
    } catch (err) {
      error.value = err instanceof Error ? err.message : "Unknown error";
      console.error("Failed to refresh discovered devices:", err);
    } finally {
      isLoading.value = false;
    }
  };

  /**
   * Fetch this device's information.
   */
  const fetchSelfDevice = async (): Promise<void> => {
    try {
      selfDevice.value = await fetchSelfDeviceInfo(getLocalApiBase());
    } catch (err) {
      console.error("Failed to fetch self device info:", err);
    }
  };

  /**
   * Set this device's name.
   */
  const setDeviceName = async (name: string): Promise<boolean> => {
    try {
      const response = await fetch(`${getLocalApiBase()}/discovery/name`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name }),
      });

      if (!response.ok) {
        throw new Error(`Failed to set device name: ${response.statusText}`);
      }

      const result: SetDeviceNameResponse = await response.json();
      if (result.success) {
        await fetchSelfDevice();
        return true;
      }
      return false;
    } catch (err) {
      console.error("Failed to set device name:", err);
      return false;
    }
  };

  const formatLastSeen = (secs: number): string => {
    if (secs < 60) return `${secs}秒前`;
    if (secs < 3600) return `${Math.floor(secs / 60)}分钟前`;
    return `${Math.floor(secs / 3600)}小时前`;
  };

  return {
    devices,
    selfDevice,
    isLoading,
    error,
    fetchDevices,
    refreshDevices,
    fetchSelfDevice,
    setDeviceName,
    formatLastSeen,
  };
}
