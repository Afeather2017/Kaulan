<template>
  <div class="modal-overlay" @click="$emit('close')">
    <div class="modal-content" @click.stop>
      <div class="modal-top-bar">
        <button class="top-back-btn" @click="$emit('close')">
          <i class="fas fa-arrow-left"></i>
          返回
        </button>
      </div>
      <div class="modal-body">
        <h3>添加设备</h3>

        <div class="setting-item">
          <label class="setting-label">手动输入地址</label>
          <div class="manual-address-row">
            <input
              v-model="manualAddressInput"
              type="text"
              class="url-input"
              placeholder="192.168.1.10:2080"
              @keyup.enter="connectManualDevice"
            />
            <button
              class="primary-btn"
              :disabled="isConnectingManualDevice"
              @click="connectManualDevice"
            >
              {{ isConnectingManualDevice ? "连接中..." : "连接设备" }}
            </button>
          </div>
          <p
            v-if="manualAddressMessage"
            class="setting-hint"
            :class="{ 'setting-error': manualAddressError }"
          >
            {{ manualAddressMessage }}
          </p>
        </div>

        <hr class="settings-divider" />

        <div class="section-header">
          <div class="section-title">局域网中的设备</div>
          <button
            class="secondary-btn"
            :disabled="isLoadingDevices"
            @click="refreshDevices"
          >
            {{ isLoadingDevices ? "扫描中..." : "刷新" }}
          </button>
        </div>

        <div v-if="isLoadingDevices" class="loading-state">扫描中...</div>
        <div
          v-else-if="discoveryError"
          class="setting-hint setting-error discovery-error"
        >
          {{ discoveryError }}
        </div>
        <div v-else-if="displayDevices.length === 0" class="empty-state">
          未发现其他设备
        </div>
        <div v-else class="device-list">
          <div
            v-for="device in displayDevices"
            :key="device.device_id"
            class="device-item"
            @click="connectToDevice(device)"
          >
            <div class="device-info">
              <div class="device-header">
                <div class="device-name-row">
                  <div class="device-name">{{ device.device_name }}</div>
                  <span v-if="device.isManual" class="manual-badge"
                    >手动添加</span
                  >
                </div>
                <div class="device-actions">
                  <div class="device-last-seen">
                    {{
                      isLocalhostDevice(device)
                        ? "本机"
                        : formatLastSeen(device.last_seen_secs_ago)
                    }}
                  </div>
                  <button
                    v-if="device.isManual"
                    class="remove-device-btn"
                    @click.stop="removeManualDevice(device.api_url)"
                  >
                    <i class="fas fa-times"></i>
                  </button>
                </div>
              </div>
              <div class="device-url">{{ device.api_url }}</div>
            </div>
          </div>
        </div>

        <div class="modal-actions">
          <button class="confirm-btn" @click="$emit('close')">确认</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { normalizeApiBase, setApiBase } from "@/utils/api";
import {
  getManualDevices,
  setManualDevices,
  type ManualDevice,
} from "@/utils/storage";
import { validateServerUrl } from "@/utils/validation";
import {
  useDeviceDiscovery,
  type DiscoveredDevice,
} from "@/composables/useDeviceDiscovery";

defineEmits<{
  (e: "close"): void;
}>();

const {
  devices: discoveredDevices,
  isLoading: isLoadingDevices,
  error: discoveryError,
  fetchDevices,
  refreshDevices,
  connectToDevice,
  formatLastSeen,
} = useDeviceDiscovery();

const LOCALHOST_API_URL = "http://localhost:2080/api";
const manualDevices = ref<ManualDevice[]>([]);
const manualAddressInput = ref("");
const manualAddressMessage = ref("");
const manualAddressError = ref(false);
const isConnectingManualDevice = ref(false);

const localhostDevice = computed<DiscoveredDevice>(() => ({
  device_id: "localhost-self",
  device_name: "localhost(self)",
  api_url: LOCALHOST_API_URL,
  last_seen_secs_ago: 0,
}));

const displayDevices = computed<DiscoveredDevice[]>(() => {
  const manualDeviceEntries: DiscoveredDevice[] = manualDevices.value.map(
    (device, index) => ({
      device_id: `manual-${index}-${device.added_at}`,
      device_name: device.device_name || "手动添加",
      api_url: device.api_url,
      last_seen_secs_ago: 0,
      isManual: true,
    }),
  );

  const merged = [
    localhostDevice.value,
    ...discoveredDevices.value,
    ...manualDeviceEntries,
  ];
  const unique = new Map<string, DiscoveredDevice>();
  for (const device of merged) {
    if (!unique.has(device.api_url)) {
      unique.set(device.api_url, device);
    }
  }
  return Array.from(unique.values());
});

const isLocalhostDevice = (device: DiscoveredDevice): boolean =>
  device.api_url === LOCALHOST_API_URL;

const saveManualDevices = () => {
  setManualDevices(manualDevices.value);
};

const loadManualDevices = () => {
  manualDevices.value = getManualDevices();
};

const fetchDeviceName = async (url: string): Promise<string | null> => {
  try {
    const normalizedUrl = normalizeApiBase(url);
    const response = await fetch(`${normalizedUrl}/discovery/self`);
    if (!response.ok) {
      return null;
    }
    const data = await response.json();
    return typeof data.device_name === "string" ? data.device_name : null;
  } catch (error) {
    console.warn(`Failed to fetch device name from ${url}:`, error);
    return null;
  }
};

const refreshManualDeviceNames = async () => {
  const updated = await Promise.all(
    manualDevices.value.map(async (device) => {
      const deviceName = await fetchDeviceName(device.api_url);
      return {
        ...device,
        device_name: deviceName || device.device_name,
        last_fetched: deviceName ? Date.now() : device.last_fetched,
      };
    }),
  );
  manualDevices.value = updated;
  saveManualDevices();
};

const removeManualDevice = (url: string) => {
  manualDevices.value = manualDevices.value.filter(
    (device) => device.api_url !== url,
  );
  saveManualDevices();
};

const connectManualDevice = async () => {
  const trimmed = manualAddressInput.value.trim();
  const validation = validateServerUrl(trimmed);
  if (!validation.valid) {
    manualAddressMessage.value = validation.error || "地址无效";
    manualAddressError.value = true;
    return;
  }

  isConnectingManualDevice.value = true;
  manualAddressMessage.value = "";
  manualAddressError.value = false;

  try {
    const normalizedUrl = normalizeApiBase(trimmed);
    const existing = manualDevices.value.find(
      (device) => device.api_url === normalizedUrl,
    );
    const deviceName = await fetchDeviceName(normalizedUrl);

    if (existing) {
      existing.device_name = deviceName || existing.device_name;
      existing.last_fetched = deviceName ? Date.now() : existing.last_fetched;
    } else {
      manualDevices.value = [
        ...manualDevices.value,
        {
          api_url: normalizedUrl,
          device_name: deviceName || undefined,
          added_at: Date.now(),
          last_fetched: deviceName ? Date.now() : undefined,
        },
      ];
    }

    saveManualDevices();
    setApiBase(normalizedUrl);
    window.location.reload();
  } catch (error) {
    console.error("Failed to connect manual device:", error);
    manualAddressMessage.value = `连接设备失败: ${error}`;
    manualAddressError.value = true;
  } finally {
    isConnectingManualDevice.value = false;
  }
};

onMounted(async () => {
  loadManualDevices();
  await Promise.allSettled([fetchDevices(), refreshManualDeviceNames()]);
});
</script>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: flex-start;
  justify-content: flex-start;
  z-index: 100;
}

.modal-content {
  background-color: #fff;
  width: 500px;
  max-width: 85vw;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 2px 0 10px rgba(0, 0, 0, 0.1);
}

.modal-top-bar {
  flex: none;
  padding: 12px 20px;
  border-bottom: 1px solid #eee;
  display: flex;
  align-items: center;
  background-color: #fff;
}

.top-back-btn {
  border: 1px solid #ddd;
  background-color: #f8f8f8;
  color: #333;
  font-size: 15px;
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  border-radius: 999px;
  padding: 6px 12px;
  transition: all 0.2s;
}

.top-back-btn:hover {
  background-color: #f0f0f0;
  border-color: #ccc;
}

.modal-body {
  flex: 1;
  overflow-y: auto;
  padding: 24px 28px 32px;
}

.modal-body h3 {
  margin: 0 0 20px 0;
  font-size: 22px;
  font-weight: 600;
  color: #333;
}

.setting-item {
  margin-bottom: 20px;
}

.setting-label {
  display: block;
  margin-bottom: 8px;
  font-weight: 500;
  font-size: 15px;
  color: #555;
}

.manual-address-row {
  display: flex;
  gap: 10px;
}

.url-input {
  flex: 1;
  padding: 12px 14px;
  border: 1px solid #d0d7de;
  border-radius: 10px;
  font-size: 15px;
}

.primary-btn,
.secondary-btn,
.confirm-btn,
.remove-device-btn {
  cursor: pointer;
}

.primary-btn {
  border: 1px solid #1db954;
  background: #1db954;
  color: #fff;
  border-radius: 10px;
  padding: 0 16px;
  font-size: 14px;
  font-weight: 600;
}

.primary-btn:disabled,
.secondary-btn:disabled {
  opacity: 0.6;
  cursor: default;
}

.secondary-btn {
  border: 1px solid #d0d7de;
  background: #fff;
  color: #31414f;
  border-radius: 999px;
  padding: 8px 14px;
  font-size: 14px;
  font-weight: 600;
}

.settings-divider {
  border: none;
  border-top: 1px solid #eee;
  margin: 20px 0;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}

.section-title {
  font-size: 17px;
  font-weight: 600;
  color: #222;
}

.loading-state,
.empty-state {
  padding: 20px 0;
  color: #666;
  font-size: 14px;
}

.discovery-error {
  margin-bottom: 12px;
}

.setting-hint {
  margin: 10px 0 0;
  color: #666;
  font-size: 13px;
  line-height: 1.5;
}

.setting-error {
  color: #c0362c;
}

.device-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.device-item {
  border: 1px solid #e7edf2;
  border-radius: 14px;
  padding: 14px 16px;
  background: #fff;
  cursor: pointer;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.device-item:hover {
  border-color: #1db954;
  box-shadow: 0 8px 20px rgba(29, 185, 84, 0.08);
}

.device-header {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
}

.device-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.device-name {
  font-size: 16px;
  font-weight: 600;
  color: #222;
}

.manual-badge {
  background: #e7f6ed;
  color: #176b3a;
  border-radius: 999px;
  padding: 3px 8px;
  font-size: 12px;
  font-weight: 700;
}

.device-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.device-last-seen,
.device-url {
  font-size: 13px;
  color: #667585;
}

.device-url {
  margin-top: 8px;
  overflow-wrap: anywhere;
}

.remove-device-btn {
  border: 1px solid #e7edf2;
  background: #fff;
  color: #667585;
  width: 28px;
  height: 28px;
  border-radius: 999px;
}

.modal-actions {
  margin-top: 24px;
  display: flex;
  justify-content: flex-end;
}

.confirm-btn {
  border: none;
  background: #31414f;
  color: #fff;
  border-radius: 999px;
  padding: 10px 20px;
  font-size: 14px;
  font-weight: 600;
}

@media (max-width: 720px) {
  .modal-content {
    width: 100%;
    max-width: none;
  }

  .modal-body {
    padding: 20px 18px 28px;
  }

  .manual-address-row {
    flex-direction: column;
  }

  .primary-btn {
    min-height: 44px;
  }
}
</style>
