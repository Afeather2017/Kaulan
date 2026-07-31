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
        <h3>设置</h3>

        <div class="settings-section">
          <div class="section-title">播放</div>
        </div>
        <div class="setting-item">
          <label class="setting-label">定时停止播放</label>
          <div class="timer-status">{{ timerStatusDisplay }}</div>
          <div class="slider-container">
            <input
              type="range"
              class="volume-slider"
              :model-value="Math.min(timerMinutes, 120)"
              @input="handleTimerMinutesSlider"
              min="1"
              max="120"
              step="1"
            />
            <input
              type="text"
              class="value-input"
              :value="timerMinutesDisplay"
              @input="handleTimerMinutesInput"
              @blur="handleTimerMinutesBlur"
            />
            <span class="value-suffix">分钟</span>
          </div>
          <div class="timer-presets">
            <button
              v-for="preset in [15, 30, 60]"
              :key="preset"
              class="timer-preset-btn"
              @click="$emit('setTimerPreset', preset)"
            >
              {{ preset }}分钟
            </button>
          </div>
          <div class="timer-actions">
            <button
              v-if="timerActive"
              @click="$emit('cancelTimer')"
              class="cancel-timer-btn"
            >
              取消定时
            </button>
            <button v-else @click="$emit('startTimer')" class="start-timer-btn">
              开始定时
            </button>
          </div>
          <div
            v-if="runtimeCapabilities.supportsExitAppOnTimer"
            class="timer-option"
          >
            <label class="checkbox-label">
              <input
                type="checkbox"
                :checked="timerExitAppOnAndroid"
                @change="handleTimerExitAppOnAndroidChange"
                class="setting-checkbox"
              />
              <span>定时关闭程序</span>
            </label>
            <p class="setting-hint">
              启用后，定时器触发时会直接退出 Android 应用，而不仅是停止播放。
            </p>
          </div>
        </div>

        <hr class="settings-divider" />
        <div
          v-if="runtimeCapabilities.supportsLocalLyricsPermission"
          class="setting-item"
        >
          <label class="checkbox-label">
            <input
              type="checkbox"
              :checked="useLocalLyrics"
              @change="handleUseLocalLyricsChange"
              :disabled="isRequestingPermission"
              class="setting-checkbox"
            />
            <span>使用本地歌词</span>
          </label>
          <p
            v-if="permissionStatus"
            class="setting-hint"
            :class="{ 'setting-error': !permissionGranted }"
          >
            {{ permissionStatus }}
          </p>
        </div>

        <div
          v-if="runtimeCapabilities.supportsHeadsetMediaButtonControl"
          class="setting-item"
        >
          <label class="checkbox-label">
            <input
              type="checkbox"
              :checked="disableHeadsetMediaButton"
              @change="handleDisableHeadsetMediaButtonChange"
              class="setting-checkbox"
            />
            <span>禁用耳机媒体按钮</span>
          </label>
          <p class="setting-hint">
            启用后，耳机的播放/暂停/上一曲/下一曲按钮将被忽略。适用于耳机按钮故障时防止误触。
          </p>
        </div>

        <hr class="settings-divider" />
        <div class="settings-section">
          <div class="section-title">个人</div>
        </div>
        <div class="setting-item">
          <button
            class="manage-collections-btn"
            @click="$emit('manageCollections')"
          >
            管理我的收藏夹
          </button>
        </div>
        <div class="setting-item">
          <label class="checkbox-label">
            <input
              type="checkbox"
              :checked="allowTextSelection"
              @change="
                $emit(
                  'update:allowTextSelection',
                  ($event.target as HTMLInputElement).checked,
                )
              "
              class="setting-checkbox"
            />
            <span>允许选择文本</span>
          </label>
          <p class="setting-hint">
            默认关闭以保持界面整洁。需要复制歌词或歌曲名时打开此选项。
          </p>
        </div>
        <div class="setting-item">
          <label class="setting-label">备份收藏夹</label>
          <p class="setting-hint">
            导出当前收藏夹为 JSON
            文件，重装应用或换设备后可通过"导入"恢复。无法匹配的歌曲将被跳过。
          </p>
          <div class="collection-transfer-actions">
            <button class="transfer-btn" @click="handleExportCollections">
              <i class="fas fa-file-export"></i>
              导出收藏夹
            </button>
            <button
              class="transfer-btn transfer-btn-secondary"
              @click="triggerImportCollections"
              :disabled="isImporting"
            >
              <i class="fas fa-file-import"></i>
              {{ isImporting ? "导入中..." : "导入收藏夹" }}
            </button>
            <input
              ref="importFileInput"
              type="file"
              accept="application/json,.json"
              class="hidden-file-input"
              @change="handleImportFileChange"
            />
          </div>
        </div>

        <hr class="settings-divider" />
        <button class="advanced-toggle-btn" @click="toggleAdvancedSettings">
          <span class="advanced-toggle-copy">
            <span class="advanced-toggle-title">高级设置</span>
          </span>
          <span class="advanced-toggle-state">
            {{ showAdvancedSettings ? "收起" : "展开" }}
            <i
              :class="[
                'fas',
                showAdvancedSettings ? 'fa-chevron-up' : 'fa-chevron-down',
              ]"
            ></i>
          </span>
        </button>

        <div v-if="showAdvancedSettings" class="advanced-settings-panel">
          <div class="mode-toggle">
            <div class="mode-label">播放与响度</div>
          </div>
          <div class="setting-item">
            <label class="setting-label">音量模式</label>
            <div class="mode-option-group">
              <button
                v-for="mode in volumeModeOptions"
                :key="mode.value"
                class="mode-option-btn"
                :class="{ active: volumeMode === mode.value }"
                @click="$emit('update:volumeMode', mode.value)"
              >
                {{ mode.label }}
              </button>
            </div>
            <p class="setting-hint">
              自动模式按当前列表 LUFS 平衡音量，固定模式使用目标
              LUFS，手动模式直接使用固定音量。
            </p>
          </div>
          <div v-if="volumeMode === 'fixed'" class="setting-item">
            <label class="setting-label">目标 LUFS</label>
            <div class="slider-container">
              <input
                type="range"
                class="volume-slider"
                :value="fixedLufs"
                @input="
                  $emit(
                    'update:fixedLufs',
                    Number(($event.target as HTMLInputElement).value),
                  )
                "
                min="-40"
                max="-5"
                step="1"
              />
              <input
                type="number"
                class="value-input"
                :value="fixedLufsInput"
                @input="
                  $emit(
                    'update:fixedLufsInput',
                    Number(($event.target as HTMLInputElement).value),
                  )
                "
                min="-40"
                max="-5"
                step="1"
              />
              <span class="value-suffix">LUFS</span>
            </div>
          </div>
          <div v-else-if="volumeMode === 'manual'" class="setting-item">
            <label class="setting-label">手动音量</label>
            <div class="slider-container">
              <input
                type="range"
                class="volume-slider"
                :value="manualVolume"
                @input="
                  $emit(
                    'update:manualVolume',
                    Number(($event.target as HTMLInputElement).value),
                  )
                "
                min="0"
                max="1"
                step="0.01"
              />
              <input
                type="number"
                class="value-input"
                :value="manualVolumeInput"
                @input="
                  $emit(
                    'update:manualVolumeInput',
                    Number(($event.target as HTMLInputElement).value),
                  )
                "
                min="0"
                max="1"
                step="0.01"
              />
              <span class="value-suffix">x</span>
            </div>
          </div>
          <div class="setting-item">
            <label class="setting-label">LUFS 预缓存数量</label>
            <div class="slider-container">
              <input
                type="range"
                class="volume-slider"
                :value="lufsPrecacheCount"
                @input="handleLufsPrecacheCountInput"
                :min="MIN_LUFS_PRECACHE_COUNT"
                :max="MAX_LUFS_PRECACHE_COUNT"
                step="1"
              />
              <input
                type="number"
                class="value-input"
                :value="lufsPrecacheCount"
                @input="handleLufsPrecacheCountInput"
                :min="MIN_LUFS_PRECACHE_COUNT"
                :max="MAX_LUFS_PRECACHE_COUNT"
                step="1"
              />
              <span class="value-suffix">首</span>
            </div>
            <p class="setting-hint">
              点击播放时，从当前歌曲开始按队列顺序预缓存还没有 LUFS 的歌曲。
            </p>
          </div>
          <div class="mode-toggle">
            <div class="mode-label">播放诊断</div>
          </div>
          <div class="setting-item">
            <label class="checkbox-label">
              <input
                type="checkbox"
                :checked="showLufs"
                @change="
                  $emit(
                    'update:showLufs',
                    ($event.target as HTMLInputElement).checked,
                  )
                "
                class="setting-checkbox"
              />
              <span>显示歌曲 LUFS 数值</span>
            </label>
            <p class="setting-hint">仅用于查看响度数据，不影响播放音量计算。</p>
          </div>
          <div class="mode-toggle">
            <div class="mode-label">设备与来源</div>
          </div>
          <div class="setting-item">
            <label class="setting-label">设备名称</label>
            <div class="url-input-container">
              <input
                type="text"
                class="url-input"
                :value="deviceNameInput"
                @input="
                  deviceNameInput = ($event.target as HTMLInputElement).value
                "
                placeholder="My Kaulan Player"
                maxlength="64"
              />
            </div>
            <div class="url-actions">
              <button
                @click="saveDeviceName"
                class="save-url-btn"
                :disabled="isSavingDeviceName"
              >
                {{ isSavingDeviceName ? "保存中..." : "保存名称" }}
              </button>
            </div>
          </div>
          <div class="setting-item">
            <label class="checkbox-label">
              <input
                type="checkbox"
                class="setting-checkbox"
                :checked="periodicDiscoveryEnabled"
                :disabled="isSavingPeriodicDiscovery"
                @change="handlePeriodicDiscoveryChange"
              />
              <span>定期发现附近设备</span>
            </label>
            <p class="setting-hint">
              每 10 秒在局域网广播一次设备信息，用于自动发现设备和更新变化后的
              IP 地址。关闭可减少后台耗电，手动刷新仍然可用。
            </p>
            <p
              v-if="periodicDiscoveryMessage"
              class="setting-hint"
              :class="{ 'setting-error': periodicDiscoveryError }"
            >
              {{ periodicDiscoveryMessage }}
            </p>
          </div>
          <div class="mode-toggle">
            <div class="mode-label">媒体类型过滤</div>
          </div>
          <div class="setting-item">
            <label class="checkbox-label">
              <input
                type="checkbox"
                class="setting-checkbox"
                :checked="selectedMediaTypes.includes('audio')"
                :disabled="
                  isMediaTypeDisabled('audio') ||
                  isSavingMediaTypes ||
                  isLoadingMediaTypes
                "
                @change="
                  toggleMediaType(
                    'audio',
                    ($event.target as HTMLInputElement).checked,
                  )
                "
              />
              <span>扫描音频文件</span>
            </label>
          </div>
          <div class="setting-item">
            <label class="checkbox-label">
              <input
                type="checkbox"
                class="setting-checkbox"
                :checked="selectedMediaTypes.includes('video')"
                :disabled="
                  isMediaTypeDisabled('video') ||
                  isSavingMediaTypes ||
                  isLoadingMediaTypes
                "
                @change="
                  toggleMediaType(
                    'video',
                    ($event.target as HTMLInputElement).checked,
                  )
                "
              />
              <span>扫描视频文件</span>
            </label>
            <p class="setting-hint warning-hint">
              视频文件不会执行 LUFS
              音量标准化，保存后需要重新扫描数据库才会生效。
            </p>
            <p
              v-if="mediaTypesMessage"
              class="setting-hint"
              :class="{ 'setting-error': mediaTypesError }"
            >
              {{ mediaTypesMessage }}
            </p>
            <div class="url-actions">
              <button
                @click="saveMediaTypes"
                class="save-url-btn"
                :disabled="isSavingMediaTypes || isLoadingMediaTypes"
              >
                {{ isSavingMediaTypes ? "保存中..." : "保存媒体类型" }}
              </button>
            </div>
          </div>
        </div>

        <div class="modal-actions">
          <button @click="$emit('close')" class="confirm-btn">确认</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import { getLocalApiBase, isTauriWebview } from "@/utils/api";
import {
  buildRuntimeCapabilities,
  getRuntimeCapabilities,
  type RuntimeCapabilities,
} from "@/utils/platform";
import {
  getMediaTypes,
  setMediaTypes,
  getDisableHeadsetMediaButton,
  setDisableHeadsetMediaButton,
  getTimerExitAppOnAndroid,
  setTimerExitAppOnAndroid,
  normalizeLufsPrecacheCount,
  MIN_LUFS_PRECACHE_COUNT,
  MAX_LUFS_PRECACHE_COUNT,
} from "@/utils/storage";
import { useDeviceDiscovery } from "@/composables/useDeviceDiscovery";
import { usePeriodicDiscovery } from "@/composables/usePeriodicDiscovery";
import { useCollectionsStore } from "@/stores/collections";
import { useLibraryStore } from "@/stores/library";
import {
  buildCollectionsExport,
  mergeCollectionsFromImport,
  parseCollectionsExport,
} from "@/utils/collectionTransfer";
import { downloadBlob } from "@/utils/browserDownload";

type VolumeMode = "auto" | "manual" | "fixed";

const props = defineProps<{
  volumeMode: VolumeMode;
  manualVolume: number;
  manualVolumeInput: number;
  fixedLufs: number;
  fixedLufsInput: number;
  showLufs: boolean;
  lufsPrecacheCount: number;
  allowTextSelection: boolean;
  timerMinutes: number;
  timerMinutesInput: number;
  timerActive: boolean;
  timerStatusDisplay: string;
  volumeModeLabels: Record<VolumeMode, string>;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "update:volumeMode", value: VolumeMode): void;
  (e: "update:manualVolume", value: number): void;
  (e: "update:manualVolumeInput", value: number): void;
  (e: "update:fixedLufs", value: number): void;
  (e: "update:fixedLufsInput", value: number): void;
  (e: "update:showLufs", value: boolean): void;
  (e: "update:lufsPrecacheCount", value: number): void;
  (e: "update:allowTextSelection", value: boolean): void;
  (e: "update:timerMinutes", value: number): void;
  (e: "update:timerMinutesInput", value: number): void;
  (e: "setTimerPreset", minutes: number): void;
  (e: "startTimer"): void;
  (e: "cancelTimer"): void;
  (e: "directoryChanged"): void;
  (e: "databaseUpdated"): void;
  (e: "databaseUpdateStart"): void;
  (e: "databaseUpdateEnd"): void;
  (e: "openUploadModal"): void;
  (e: "manageCollections"): void;
}>();

const { selfDevice, fetchSelfDevice, setDeviceName } = useDeviceDiscovery();
const {
  enabled: periodicDiscoveryEnabled,
  isSaving: isSavingPeriodicDiscovery,
  message: periodicDiscoveryMessage,
  hasError: periodicDiscoveryError,
  load: loadPeriodicDiscovery,
  save: savePeriodicDiscovery,
} = usePeriodicDiscovery();

const deviceNameInput = ref<string>("");
const isSavingDeviceName = ref<boolean>(false);

const selectedMediaTypes = ref<string[]>(getMediaTypes());
const isLoadingMediaTypes = ref<boolean>(false);
const isSavingMediaTypes = ref<boolean>(false);
const mediaTypesMessage = ref<string>("");
const mediaTypesError = ref<boolean>(false);

// Local lyrics setting (Android only)
const useLocalLyrics = ref<boolean>(false);
const isRequestingPermission = ref<boolean>(false);
const permissionGranted = ref<boolean>(false);
const permissionStatus = ref<string>("");

// Disable headset media button (Android only)
const disableHeadsetMediaButton = ref<boolean>(false);
const timerExitAppOnAndroid = ref<boolean>(false);

const runtimeCapabilities = ref<RuntimeCapabilities>(
  buildRuntimeCapabilities("web"),
);
const showAdvancedSettings = ref<boolean>(false);
const volumeModeOptions: Array<{ value: VolumeMode; label: string }> = [
  { value: "auto", label: "自动" },
  { value: "fixed", label: "固定 LUFS" },
  { value: "manual", label: "手动" },
];

const toggleAdvancedSettings = () => {
  showAdvancedSettings.value = !showAdvancedSettings.value;
};

// Handle use local lyrics checkbox change
const handleUseLocalLyricsChange = async (e: Event) => {
  const checked = (e.target as HTMLInputElement).checked;

  if (!checked) {
    useLocalLyrics.value = false;
    permissionStatus.value = "";
    return;
  }

  isRequestingPermission.value = true;
  permissionStatus.value = "正在请求权限...";

  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const granted = await invoke<boolean>(
      "request_external_storage_permission",
    );

    permissionGranted.value = granted;
    useLocalLyrics.value = granted;

    if (granted) {
      permissionStatus.value = "权限已授予，可以读取本地歌词文件";
    } else {
      permissionStatus.value = "权限未授予，无法读取本地歌词文件";
      useLocalLyrics.value = false;
    }
  } catch (error) {
    console.error("Failed to request external storage permission:", error);
    permissionStatus.value = "请求权限失败: " + error;
    useLocalLyrics.value = false;
  } finally {
    isRequestingPermission.value = false;
  }
};

const handleDisableHeadsetMediaButtonChange = async (e: Event) => {
  const checked = (e.target as HTMLInputElement).checked;
  disableHeadsetMediaButton.value = checked;
  setDisableHeadsetMediaButton(checked);

  try {
    const plugin = await import("music-notification-api");
    await plugin.setHeadsetMediaButtonDisabled(checked);
  } catch (error) {
    console.error(
      "Failed to sync headset media button setting to native:",
      error,
    );
  }
};

const handleTimerExitAppOnAndroidChange = (e: Event) => {
  const checked = (e.target as HTMLInputElement).checked;
  timerExitAppOnAndroid.value = checked;
  setTimerExitAppOnAndroid(checked);
};

const handleLufsPrecacheCountInput = (e: Event) => {
  emit(
    "update:lufsPrecacheCount",
    normalizeLufsPrecacheCount(Number((e.target as HTMLInputElement).value)),
  );
};

const sortMediaTypes = (mediaTypes: string[]): string[] => {
  return ["audio", "video"].filter((type) => mediaTypes.includes(type));
};

const isMediaTypeDisabled = (mediaType: string): boolean => {
  return (
    selectedMediaTypes.value.length === 1 &&
    selectedMediaTypes.value.includes(mediaType)
  );
};

const toggleMediaType = (mediaType: string, enabled: boolean) => {
  const next = new Set(selectedMediaTypes.value);

  if (enabled) {
    next.add(mediaType);
  } else if (!(next.size === 1 && next.has(mediaType))) {
    next.delete(mediaType);
  }

  selectedMediaTypes.value = sortMediaTypes(Array.from(next));
  mediaTypesMessage.value = "";
  mediaTypesError.value = false;
};

const loadMediaTypes = async () => {
  isLoadingMediaTypes.value = true;
  mediaTypesMessage.value = "";
  mediaTypesError.value = false;

  try {
    const response = await fetch(`${getLocalApiBase()}/settings/media-types`);
    if (!response.ok) {
      throw new Error(`Request failed with status ${response.status}`);
    }

    const data = await response.json();
    const mediaTypes = Array.isArray(data.media_types)
      ? sortMediaTypes(data.media_types)
      : ["audio"];
    selectedMediaTypes.value = mediaTypes.length > 0 ? mediaTypes : ["audio"];
    setMediaTypes(selectedMediaTypes.value);
  } catch (error) {
    console.error("Failed to load media types:", error);
    selectedMediaTypes.value = sortMediaTypes(getMediaTypes());
    mediaTypesMessage.value = "读取媒体类型失败，已使用本地缓存。";
    mediaTypesError.value = true;
  } finally {
    isLoadingMediaTypes.value = false;
  }
};

const saveMediaTypes = async () => {
  isSavingMediaTypes.value = true;
  mediaTypesMessage.value = "";
  mediaTypesError.value = false;

  try {
    const response = await fetch(`${getLocalApiBase()}/settings/media-types`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ media_types: selectedMediaTypes.value }),
    });
    const result = await response.json();

    if (!response.ok || !result.success) {
      throw new Error(
        result.message || `Request failed with status ${response.status}`,
      );
    }

    setMediaTypes(selectedMediaTypes.value);
    mediaTypesMessage.value = "媒体类型已保存，重新扫描数据库后生效。";
  } catch (error) {
    console.error("Failed to save media types:", error);
    mediaTypesMessage.value = `保存媒体类型失败: ${error}`;
    mediaTypesError.value = true;
  } finally {
    isSavingMediaTypes.value = false;
  }
};

// Temporary state for user input (before blur/commit)
const timerMinutesInputTemp = ref("");

// Computed display values
const timerMinutesDisplay = computed(() => `${props.timerMinutes}`);

// Timer handlers
const handleTimerMinutesSlider = (e: Event) => {
  const value = Number((e.target as HTMLInputElement).value);
  emit("update:timerMinutes", value);
};

const handleTimerMinutesInput = (e: Event) => {
  timerMinutesInputTemp.value = (e.target as HTMLInputElement).value;
};

const handleTimerMinutesBlur = () => {
  let valueStr = timerMinutesInputTemp.value.trim();
  // Remove Chinese time units if present
  valueStr = valueStr.replace(/分钟/g, "").replace(/小时/g, " ").trim();

  // Parse hours if present
  const parts = valueStr.split(" ");
  let totalMinutes = 0;
  if (parts.length === 2) {
    const hours = Number(parts[0]);
    const mins = Number(parts[1]);
    if (!isNaN(hours)) totalMinutes += hours * 60;
    if (!isNaN(mins)) totalMinutes += mins;
  } else {
    totalMinutes = Number(valueStr);
  }

  // Allow any positive value (slider is limited to 1-120, but manual input can be larger)
  if (!isNaN(totalMinutes) && totalMinutes >= 1) {
    emit("update:timerMinutes", Math.min(totalMinutes, 999));
  }
  // Reset temp to current display value
  timerMinutesInputTemp.value = "";
};

onMounted(async () => {
  // Initialize device name from localStorage first (local device's name)
  const localDeviceName = localStorage.getItem("kaulan_local_device_name");
  if (localDeviceName) {
    deviceNameInput.value = localDeviceName;
  } else {
    // Fallback: fetch from current API and save as local device name
    await fetchSelfDevice();
    if (selfDevice.value) {
      deviceNameInput.value = selfDevice.value.device_name;
      localStorage.setItem(
        "kaulan_local_device_name",
        selfDevice.value.device_name,
      );
    }
  }

  await loadMediaTypes();
  await loadPeriodicDiscovery();

  runtimeCapabilities.value = await getRuntimeCapabilities();

  // Load local lyrics setting (Android only) - query actual permission state
  if (runtimeCapabilities.value.supportsLocalLyricsPermission) {
    timerExitAppOnAndroid.value = getTimerExitAppOnAndroid();

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const granted = await invoke<boolean>(
        "check_external_storage_permission",
      );
      useLocalLyrics.value = granted;
      if (granted) {
        permissionStatus.value = "本地歌词已启用";
      }
    } catch (e) {
      console.error("Failed to check permission status:", e);
    }

    // Sync headset media button setting to native plugin
    const localHeadsetValue = getDisableHeadsetMediaButton();
    disableHeadsetMediaButton.value = localHeadsetValue;
    try {
      const plugin = await import("music-notification-api");
      await plugin.setHeadsetMediaButtonDisabled(localHeadsetValue);
    } catch (e) {
      console.error("Failed to sync headset media button setting:", e);
    }
  }
});

// Device discovery functions
const saveDeviceName = async () => {
  if (!deviceNameInput.value.trim()) return;

  isSavingDeviceName.value = true;
  const success = await setDeviceName(deviceNameInput.value.trim());
  isSavingDeviceName.value = false;

  if (success) {
    // Save to localStorage so it persists when connecting to other devices
    localStorage.setItem(
      "kaulan_local_device_name",
      deviceNameInput.value.trim(),
    );
    alert("设备名称已更新");
  } else {
    alert("保存设备名称失败");
  }
};

const handlePeriodicDiscoveryChange = async (event: Event): Promise<void> => {
  const nextEnabled = (event.target as HTMLInputElement).checked;
  await savePeriodicDiscovery(nextEnabled);
};

// Collection export/import
const collectionsStore = useCollectionsStore();
const libraryStore = useLibraryStore();
const importFileInput = ref<HTMLInputElement | null>(null);
const isImporting = ref<boolean>(false);
const isExporting = ref<boolean>(false);

const buildExportFilename = (): string => {
  const date = new Date().toISOString().slice(0, 10);
  return `kaulan-collections-${date}.json`;
};

const buildExportPayloadJson = (): string => {
  const collections = collectionsStore.localCollections;
  if (collections.length === 0) {
    throw new Error("当前没有收藏夹可导出");
  }
  return JSON.stringify(buildCollectionsExport(collections), null, 2);
};

const runImportFromText = (text: string): void => {
  const payload = parseCollectionsExport(text);
  const { collections, result } = mergeCollectionsFromImport(
    payload,
    collectionsStore.localCollections,
    libraryStore.allLibrarySongs,
  );
  collectionsStore.replaceLocalCollections(collections);

  alert(
    [
      `新收藏夹：${result.importedCollections}`,
      `合并到已有收藏夹：${result.mergedCollections}`,
      `匹配歌曲：${result.matchedSongs}`,
      `未匹配已跳过：${result.skippedSongs}`,
    ].join("\n"),
  );
};

const exportViaTauri = async (): Promise<void> => {
  const json = buildExportPayloadJson();
  const { save } = await import("@tauri-apps/plugin-dialog");
  const { writeTextFile } = await import("@tauri-apps/plugin-fs");
  const path = await save({
    defaultPath: buildExportFilename(),
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!path) {
    // User cancelled the save dialog.
    return;
  }
  await writeTextFile(path, json);
  alert(`已导出到：${path}`);
};

const exportViaBrowser = (): void => {
  const json = buildExportPayloadJson();
  downloadBlob(
    new Blob([json], { type: "application/json" }),
    buildExportFilename(),
  );
};

const handleExportCollections = async (): Promise<void> => {
  if (isExporting.value) {
    return;
  }
  isExporting.value = true;
  try {
    if (isTauriWebview()) {
      await exportViaTauri();
    } else {
      exportViaBrowser();
    }
  } catch (error) {
    console.error("Failed to export collections:", error);
    alert(
      `导出失败：${error instanceof Error ? error.message : String(error)}`,
    );
  } finally {
    isExporting.value = false;
  }
};

const importViaTauri = async (): Promise<void> => {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const { readTextFile } = await import("@tauri-apps/plugin-fs");
  const selected = await open({
    multiple: false,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  const path = Array.isArray(selected) ? selected[0] : selected;
  if (!path) {
    // User cancelled the open dialog.
    return;
  }
  const text = await readTextFile(path);
  runImportFromText(text);
};

const triggerImportCollections = async (): Promise<void> => {
  if (isImporting.value) {
    return;
  }
  if (isTauriWebview()) {
    isImporting.value = true;
    try {
      await importViaTauri();
    } catch (error) {
      console.error("Failed to import collections:", error);
      alert(
        `导入失败：${error instanceof Error ? error.message : String(error)}`,
      );
    } finally {
      isImporting.value = false;
    }
    return;
  }
  // Browser path: pop the hidden file input. The actual read+merge happens
  // in handleImportFileChange (isImporting is tracked there).
  importFileInput.value?.click();
};

const handleImportFileChange = async (e: Event): Promise<void> => {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  // Reset so the same file can be picked again later.
  input.value = "";
  if (!file) {
    return;
  }

  isImporting.value = true;
  try {
    const text = await file.text();
    runImportFromText(text);
  } catch (error) {
    console.error("Failed to import collections:", error);
    alert(
      `导入失败：${error instanceof Error ? error.message : String(error)}`,
    );
  } finally {
    isImporting.value = false;
  }
};
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

.mode-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
  padding: 12px 15px;
  background-color: #f9f9f9;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.3s;
}

.mode-toggle:hover {
  background-color: #f0f0f0;
}

.mode-label {
  font-size: 17px;
  font-weight: 600;
}

.mode-value {
  font-size: 16px;
  color: #1db954;
  font-weight: 500;
  min-width: 100px;
  text-align: right;
}

.settings-divider {
  border: none;
  border-top: 1px solid #eee;
  margin: 20px 0;
}

.setting-panel {
  background-color: #f9f9f9;
  border-radius: 8px;
  padding: 20px;
  margin-top: 15px;
  display: none;
}

.setting-panel.active {
  display: block;
}

.setting-item {
  margin-bottom: 20px;
}

.setting-item:last-child {
  margin-bottom: 0;
}

.setting-label {
  display: block;
  margin-bottom: 8px;
  font-weight: 500;
  font-size: 15px;
  color: #555;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 15px;
  color: #333;
  cursor: pointer;
}

.setting-checkbox {
  width: 18px;
  height: 18px;
  accent-color: #1db954;
}

.setting-hint {
  margin: 10px 0 0;
  font-size: 13px;
  line-height: 1.5;
  color: #666;
}

.warning-hint {
  color: #b45f06;
}

.setting-error {
  color: #c0392b;
}

.slider-container {
  display: flex;
  align-items: center;
  gap: 15px;
  min-width: 0;
}

.mode-option-group {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.mode-option-btn {
  border: 1px solid #d6d6d6;
  background-color: #fff;
  color: #333;
  border-radius: 12px;
  padding: 10px 12px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition:
    border-color 0.2s,
    background-color 0.2s,
    color 0.2s,
    box-shadow 0.2s;
}

.mode-option-btn.active {
  border-color: #1db954;
  background-color: #eaf8ef;
  color: #169344;
  box-shadow: 0 0 0 1px rgba(29, 185, 84, 0.12);
}

.value-input {
  width: 88px;
  border: 1px solid #d6d6d6;
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 14px;
  color: #333;
  background-color: #fff;
}

.value-suffix {
  min-width: 44px;
  font-size: 14px;
  color: #666;
}

.volume-slider {
  flex: 1;
  height: 8px;
  appearance: none;
  background: #e0e0e0;
  border-radius: 4px;
  outline: none;
}

.volume-slider::-webkit-slider-thumb {
  appearance: none;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
  transition: all 0.2s;
}

.volume-slider::-webkit-slider-thumb:hover {
  transform: scale(1.2);
  box-shadow: 0 0 0 4px rgba(29, 185, 84, 0.2);
}

.volume-slider::-moz-range-thumb {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
  border: none;
}

.volume-input {
  width: 70px;
  padding: 8px 12px;
  border: 1px solid #ddd;
  border-radius: 5px;
  font-size: 15px;
  text-align: center;
  transition: border-color 0.2s;
}

.volume-input:focus {
  border-color: #1db954;
  outline: none;
  box-shadow: 0 0 0 2px rgba(29, 185, 84, 0.2);
}

.suffix {
  font-size: 15px;
  color: #777;
  min-width: 30px;
}

.value-input {
  width: 72px;
  flex: 0 0 72px;
  min-width: 0;
  padding: 8px 12px;
  border: 1px solid #1db954;
  border-radius: 5px;
  font-size: 18px;
  font-weight: bold;
  color: #1db954;
  text-align: center;
  transition: all 0.2s;
  background-color: #f0fff4;
}

.value-input:focus {
  outline: none;
  box-shadow: 0 0 0 3px rgba(29, 185, 84, 0.3);
  border-color: #1db954;
}

.value-input::placeholder {
  color: #1db954;
  opacity: 0.5;
}

.value-suffix {
  flex: 0 0 auto;
  white-space: nowrap;
  font-size: 15px;
  color: #777;
}

.timer-status {
  margin-bottom: 10px;
  color: #1db954;
  font-weight: 500;
}

.timer-presets {
  display: flex;
  gap: 10px;
  margin-top: 10px;
}

.timer-preset-btn {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid #ddd;
  border-radius: 5px;
  background-color: #fff;
  cursor: pointer;
  font-size: 14px;
  transition: all 0.2s;
}

.timer-preset-btn:hover {
  background-color: #1db954;
  color: white;
  border-color: #1db954;
}

.timer-actions {
  margin-top: 15px;
}

.timer-option {
  margin-top: 14px;
}

.start-timer-btn,
.cancel-timer-btn {
  width: 100%;
  padding: 10px 20px;
  border: none;
  border-radius: 5px;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  background-color: #1db954;
  color: white;
}

.start-timer-btn:hover,
.cancel-timer-btn:hover {
  background-color: #1ed760;
}

.cancel-timer-btn {
  background-color: #e74c3c;
}

.cancel-timer-btn:hover {
  background-color: #c0392b;
}

.modal-actions {
  display: flex;
  gap: 12px;
  justify-content: center;
  margin-top: 25px;
}

.confirm-btn {
  padding: 10px 20px;
  border: none;
  border-radius: 5px;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  background-color: #1db954;
  color: white;
}

.confirm-btn:hover {
  background-color: #1ed760;
}

.directory-display {
  background-color: #f9f9f9;
  border: 1px solid #ddd;
  border-radius: 5px;
  padding: 12px;
  margin-bottom: 10px;
  font-size: 14px;
  color: #555;
  word-break: break-all;
  max-height: 100px;
  overflow-y: auto;
}

.select-directory-btn {
  width: 100%;
  padding: 10px 20px;
  border: 1px solid #ddd;
  border-radius: 5px;
  background-color: #fff;
  color: #333;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.select-directory-btn:hover {
  background-color: #f0f0f0;
  border-color: #ccc;
}

.select-directory-btn:active {
  background-color: #e0e0e0;
}

.update-database-btn {
  width: 100%;
  padding: 10px 20px;
  border: 1px solid #1db954;
  border-radius: 5px;
  background-color: #1db954;
  color: white;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.update-database-btn:hover:not(:disabled) {
  background-color: #1ed760;
}

.update-database-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.upload-music-btn {
  width: 100%;
  padding: 10px 20px;
  border: 1px solid #3498db;
  border-radius: 5px;
  background-color: #3498db;
  color: white;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.upload-music-btn:hover {
  background-color: #2980b9;
}

/* Server URL styles */
.url-input-container {
  margin-bottom: 10px;
}

.url-input {
  width: 100%;
  padding: 12px 15px;
  border: 1px solid #ddd;
  border-radius: 5px;
  font-size: 14px;
  font-family: monospace;
  transition: all 0.2s;
  box-sizing: border-box;
}

.url-input:focus {
  border-color: #1db954;
  outline: none;
  box-shadow: 0 0 0 3px rgba(29, 185, 84, 0.2);
}

.url-input.url-invalid {
  border-color: #e74c3c;
}

.url-input.url-invalid:focus {
  box-shadow: 0 0 0 3px rgba(231, 76, 60, 0.2);
}

.url-error {
  color: #e74c3c;
  font-size: 13px;
  margin-top: -8px;
  margin-bottom: 10px;
  padding-left: 2px;
}

.url-actions {
  display: flex;
  gap: 10px;
  margin-top: 15px;
}

.save-url-btn {
  flex: 1;
  padding: 10px 20px;
  border: none;
  border-radius: 5px;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  background-color: #1db954;
  color: white;
}

.save-url-btn:hover:not(:disabled) {
  background-color: #1ed760;
}

.save-url-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.manage-collections-btn {
  width: 100%;
  padding: 10px 20px;
  border: none;
  border-radius: 5px;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  background-color: #1db954;
  color: white;
}

.manage-collections-btn:hover {
  background-color: #1ed760;
}

.collection-transfer-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.transfer-btn {
  flex: 1;
  padding: 10px 16px;
  border: 1px solid #1db954;
  border-radius: 5px;
  background-color: #1db954;
  color: white;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.transfer-btn:hover:not(:disabled) {
  background-color: #1ed760;
}

.transfer-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.transfer-btn-secondary {
  background-color: #fff;
  color: #1db954;
}

.transfer-btn-secondary:hover:not(:disabled) {
  background-color: #eaf8ef;
}

.hidden-file-input {
  display: none;
}

.advanced-toggle-btn {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 16px;
  border: 1px solid #d9e1e7;
  border-radius: 14px;
  background: linear-gradient(180deg, #ffffff 0%, #f6f9fb 100%);
  color: #22313f;
  cursor: pointer;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease,
    transform 0.2s ease,
    background-color 0.2s ease;
}

.advanced-toggle-btn:hover {
  border-color: #b8c8d6;
  box-shadow: 0 8px 20px rgba(34, 49, 63, 0.08);
  transform: translateY(-1px);
}

.advanced-toggle-btn:active {
  transform: translateY(0);
  box-shadow: none;
}

.advanced-toggle-copy {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  min-width: 0;
}

.advanced-toggle-title {
  font-size: 15px;
  font-weight: 700;
}

.advanced-toggle-hint {
  font-size: 12px;
  color: #6b7b88;
  text-align: left;
}

.advanced-toggle-state {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  font-size: 13px;
  font-weight: 700;
  color: #176b3a;
}

.mode-value {
  font-size: 16px;
  color: #1db954;
  font-weight: 500;
  min-width: 100px;
  text-align: right;
}

/* Checkbox styles */
.checkbox-label {
  display: flex;
  align-items: center;
  cursor: pointer;
  font-size: 15px;
  color: #555;
  user-select: none;
}

.setting-checkbox {
  width: 20px;
  height: 20px;
  margin-right: 10px;
  cursor: pointer;
  accent-color: #1db954;
}

.checkbox-label:hover .setting-checkbox {
  transform: scale(1.1);
}
</style>
