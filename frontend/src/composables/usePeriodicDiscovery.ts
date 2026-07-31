/**
 * Reactive wrapper around the periodic-discovery toggle exposed by the backend.
 *
 * The actual UDP announcement cadence is owned by the backend; this composable
 * only manages the user-facing on/off switch and the optimistic UI state. See
 * `docs/device-discovery.md` for the protocol.
 */
import { ref } from "vue";
import { getLocalApiBase } from "@/utils/api";

interface PeriodicDiscoveryResponse {
  enabled: boolean;
}

interface OperationResponse {
  success: boolean;
  message?: string;
}

export function usePeriodicDiscovery() {
  const enabled = ref<boolean>(true);
  const isSaving = ref<boolean>(false);
  const message = ref<string>("");
  const hasError = ref<boolean>(false);

  const clearMessage = (): void => {
    message.value = "";
    hasError.value = false;
  };

  const load = async (): Promise<void> => {
    clearMessage();
    try {
      const response = await fetch(`${getLocalApiBase()}/discovery/periodic`);
      if (!response.ok) {
        throw new Error(`Request failed with status ${response.status}`);
      }
      const result = (await response.json()) as PeriodicDiscoveryResponse;
      enabled.value = result.enabled !== false;
    } catch (error) {
      console.error("Failed to load periodic discovery setting:", error);
      message.value = "读取定期发现设置失败。";
      hasError.value = true;
    }
  };

  const save = async (nextEnabled: boolean): Promise<boolean> => {
    const previous = enabled.value;
    enabled.value = nextEnabled;
    isSaving.value = true;
    clearMessage();

    try {
      const response = await fetch(`${getLocalApiBase()}/discovery/periodic`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ enabled: nextEnabled }),
      });
      const result = (await response.json()) as OperationResponse;
      if (!response.ok || !result.success) {
        throw new Error(
          result.message || `Request failed with status ${response.status}`,
        );
      }
      message.value = nextEnabled
        ? "定期发现已开启。"
        : "定期发现已关闭，手动刷新仍然可用。";
      return true;
    } catch (error) {
      console.error("Failed to save periodic discovery setting:", error);
      enabled.value = previous;
      message.value = `保存定期发现设置失败: ${error}`;
      hasError.value = true;
      return false;
    } finally {
      isSaving.value = false;
    }
  };

  return {
    enabled,
    isSaving,
    message,
    hasError,
    load,
    save,
  };
}
