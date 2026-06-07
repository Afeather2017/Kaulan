<template>
  <div class="modal-overlay" @click="$emit('close')">
    <div class="modal-content" @click.stop>
      <h3>在线查找</h3>
      <p class="source-caption">当前来源：{{ sourceNameLabel }}</p>

      <div class="search-section">
        <div class="search-input-row">
          <input
            v-model="searchInput"
            type="text"
            placeholder="搜索歌曲、视频或歌词关键字..."
            @keyup.enter="handleSearch"
          />
          <button
            class="search-btn"
            @click="handleSearch"
            :disabled="
              isSearching || !searchInput.trim() || selectedSources.length === 0
            "
          >
            {{ isSearching ? "搜索中..." : "搜索" }}
          </button>
        </div>

        <div v-if="hasEnabledProvider" class="source-row">
          <label
            v-for="source in providerOptions"
            :key="source.value"
            class="source-checkbox"
          >
            <input
              type="checkbox"
              :checked="selectedSources.includes(source.value)"
              :disabled="!providerStatus[source.value].enabled"
              @change="toggleSource(source.value)"
            />
            <span>{{ source.label }}</span>
          </label>
        </div>
      </div>

      <div v-if="!hasEnabledProvider" class="provider-empty-state">
        <div class="provider-empty-title">当前来源还不能用于在线搜索</div>
        <div class="provider-empty-copy">
          请先为这个来源配置至少一个可用的下载来源，然后再搜索在线内容。
        </div>
      </div>
      <div
        v-else-if="selectedSources.length === 0"
        class="provider-empty-state"
      >
        <div class="provider-empty-title">请选择要搜索的来源</div>
        <div class="provider-empty-copy">
          只会显示当前服务器已可用的来源，未登录或不可用的来源无法勾选。
        </div>
      </div>

      <div class="provider-settings">
        <button class="provider-toggle" @click="toggleProviderSettings">
          <span>来源状态</span>
          <span>{{ showProviderSettings ? "▴" : "▾" }}</span>
        </button>
        <div v-if="showProviderSettings" class="provider-settings-panel">
          <div
            v-for="provider in providerOptions"
            :key="provider.value"
            class="provider-row"
          >
            <div class="provider-row-info">
              <div class="provider-title">{{ provider.label }}</div>
              <div class="provider-summary">
                {{ providerStatus[provider.value].summary }}
              </div>
            </div>
            <button
              v-if="supportsProviderAccountActions"
              class="secondary-btn provider-manage-btn"
              @click="toggleManagingProvider(provider.value)"
            >
              {{ managingProvider === provider.value ? "收起" : "管理" }}
            </button>
            <div
              v-if="managingProvider === provider.value"
              class="provider-manage-panel"
            >
              <button class="login-btn" @click="openLogin(provider.value)">
                登录
              </button>
              <button
                class="secondary-btn"
                @click="captureLogin(provider.value)"
              >
                同步登录
              </button>
              <button
                v-if="providerStatus[provider.value].enabled"
                class="secondary-btn"
                @click="logout(provider.value)"
              >
                退出
              </button>
            </div>
          </div>
        </div>
      </div>

      <div class="download-destination">
        <div class="setting-label">下载位置</div>
        <div class="download-target-row">
          <div class="download-target-value">
            {{ selectedDownloadDirectoryLabel }}
          </div>
          <button class="secondary-btn" @click="toggleDownloadFolderPicker">
            {{ showDownloadFolderPicker ? "收起" : "选择文件夹" }}
          </button>
        </div>
        <p class="permission-message">
          下载内容会先保存到当前在线来源的曲库。若当前来源不是本机，下载时会额外询问是否同时保存一份到本机。
        </p>
        <div v-if="showDownloadFolderPicker" class="download-folder-picker">
          <div v-if="isLoadingDownloadDirectories" class="directory-loading">
            读取目录中...
          </div>
          <div v-else-if="downloadDirectoryError" class="directory-error">
            {{ downloadDirectoryError }}
          </div>
          <div v-else class="download-folder-options">
            <button
              v-for="option in downloadDirectoryOptions"
              :key="option.path"
              :class="[
                'download-folder-option',
                { selected: selectedDownloadSubdir === option.path },
              ]"
              @click="selectedDownloadSubdir = option.path"
            >
              {{ option.label }}
            </button>
          </div>
        </div>
      </div>

      <div class="results-section" v-if="searchResults.length > 0">
        <div class="results-list">
          <div
            v-for="result in searchResults"
            :key="result.source + ':' + result.id"
            class="result-item"
          >
            <img
              v-if="result.thumbnail_url"
              :src="result.thumbnail_url"
              class="result-thumbnail"
              loading="lazy"
            />
            <div v-else class="result-thumbnail placeholder"></div>

            <div class="result-info">
              <div class="result-header">
                <div class="result-title">{{ result.title }}</div>
                <span class="source-badge">{{
                  sourceLabel(result.source)
                }}</span>
              </div>
              <div class="result-meta">
                {{ result.artist }}
                <span v-if="result.duration" class="result-duration">{{
                  result.duration
                }}</span>
              </div>
              <div
                v-if="selectedLyrics[resultKey(result)]"
                class="selected-lyric"
              >
                歌词: {{ selectedLyrics[resultKey(result)]?.title }} /
                {{ selectedLyrics[resultKey(result)]?.artist }}
              </div>
            </div>

            <div class="result-actions">
              <button
                class="action-btn preview-btn"
                @click="handlePreview(result)"
                :disabled="previewingKey === resultKey(result)"
              >
                {{ previewingKey === resultKey(result) ? "准备中" : "试听" }}
              </button>
              <button
                class="action-btn lyric-btn"
                @click="toggleLyrics(result)"
              >
                歌词
              </button>
              <button
                class="action-btn download-btn"
                @click="handleDownload(result)"
                :disabled="
                  downloadingKey === resultKey(result) || !result.can_download
                "
              >
                {{
                  downloadingKey === resultKey(result)
                    ? "下载中"
                    : result.can_download
                      ? "下载到曲库"
                      : "不可下载"
                }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <div v-if="statusMessage" :class="['status-message', statusType]">
        {{ statusMessage }}
      </div>

      <div class="modal-actions">
        <button @click="$emit('close')" class="close-btn">关闭</button>
      </div>
    </div>

    <LyricSearchModal
      v-if="lyricPickerState"
      :api-base="resolvedApiBase()"
      :initial-query="buildDefaultLyricSearchQuery(lyricPickerState)"
      mode="pick"
      @close="lyricPickerState = null"
      @selected="handleLyricSelected"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, onMounted, watch } from "vue";
import LyricSearchModal from "@/components/modals/LyricSearchModal.vue";
import { type LyricCandidate } from "@/components/LyricSearchPanel.vue";

type DownloadSource = "youtube" | "netease" | "bilibili";
type OnlineProvider = DownloadSource;

interface SearchResult {
  source: DownloadSource;
  id: string;
  title: string;
  artist: string;
  duration: string | null;
  thumbnail_url: string | null;
  can_preview: boolean;
  can_download: boolean;
  requires_login: boolean;
}

interface ProviderStatus {
  provider: OnlineProvider;
  enabled: boolean;
  summary: string;
}

interface PreviewSong {
  id: number;
  name: string;
  path: string;
  stream_url: string;
  lufs: number | null;
  cover_url?: string | null;
  source: DownloadSource;
  is_temporary: boolean;
}

interface DirectoryNode {
  name: string;
  path: string;
  node_type: string;
  children?: DirectoryNode[] | null;
}

const emit = defineEmits<{
  (e: "close"): void;
  (e: "downloadComplete"): void;
  (e: "previewTrack", song: PreviewSong): void;
}>();

const props = defineProps<{
  initialQuery?: string;
  apiBase?: string;
  sourceName?: string;
}>();

const providerOptions: Array<{ value: OnlineProvider; label: string }> = [
  { value: "youtube", label: "YouTube" },
  { value: "netease", label: "网易云" },
  { value: "bilibili", label: "Bilibili" },
];

const searchInput = ref("");
const isSearching = ref(false);
const searchResults = ref<SearchResult[]>([]);
const downloadingKey = ref<string | null>(null);
const previewingKey = ref<string | null>(null);
const showProviderSettings = ref(false);
const managingProvider = ref<OnlineProvider | null>(null);
const statusMessage = ref("");
const statusType = ref<"info" | "success" | "error">("info");
const selectedSources = ref<DownloadSource[]>([]);
const downloadDirectoryTree = ref<DirectoryNode | null>(null);
const isLoadingDownloadDirectories = ref(false);
const downloadDirectoryError = ref("");
const showDownloadFolderPicker = ref(false);
const selectedDownloadSubdir = ref("");
const selectedLyrics = reactive<Record<string, LyricCandidate | null>>({});
const supportsProviderAccountActions = ref(false);
const lyricPickerState = ref<SearchResult | null>(null);
const providerStatus = reactive<Record<OnlineProvider, ProviderStatus>>({
  youtube: {
    provider: "youtube",
    enabled: false,
    summary: "未读取登录状态",
  },
  netease: {
    provider: "netease",
    enabled: false,
    summary: "未读取登录状态",
  },
  bilibili: {
    provider: "bilibili",
    enabled: false,
    summary: "未读取登录状态",
  },
});

const LOCALHOST_API_BASE = "http://localhost:2080/api";

const resolvedApiBase = (): string => {
  const candidate = props.apiBase?.trim();
  return candidate && candidate.length > 0 ? candidate : LOCALHOST_API_BASE;
};

const sourceNameLabel = computed(() => props.sourceName?.trim() || "本机来源");

const collectDirectoryOptions = (
  node: DirectoryNode | null,
  options: Array<{ path: string; label: string }>,
) => {
  if (!node) {
    return;
  }

  options.push({
    path: node.path || "",
    label: node.path || "下载根目录",
  });

  for (const child of node.children || []) {
    collectDirectoryOptions(child, options);
  }
};

const downloadDirectoryOptions = computed(() => {
  const options: Array<{ path: string; label: string }> = [];
  collectDirectoryOptions(downloadDirectoryTree.value, options);
  return options;
});

const selectedDownloadDirectoryLabel = computed(() => {
  const selected = downloadDirectoryOptions.value.find(
    (option) => option.path === selectedDownloadSubdir.value,
  );
  return selected?.label || "下载根目录";
});

const enabledSources = computed<DownloadSource[]>(() =>
  providerOptions
    .map((provider) => provider.value)
    .filter((source) => providerStatus[source].enabled),
);

const hasEnabledProvider = computed(() => enabledSources.value.length > 0);

watch(
  () => props.initialQuery,
  (value) => {
    if (typeof value === "string" && value.trim()) {
      searchInput.value = value.trim();
    }
  },
  { immediate: true },
);

onMounted(async () => {
  supportsProviderAccountActions.value =
    resolvedApiBase() === LOCALHOST_API_BASE &&
    typeof window !== "undefined" &&
    typeof (window as typeof window & { __TAURI_INTERNALS__?: unknown })
      .__TAURI_INTERNALS__ !== "undefined";

  await Promise.all([loadDownloadDirectoryTree(), loadProviderStatuses()]);
});

watch(
  () => props.apiBase,
  async () => {
    supportsProviderAccountActions.value =
      resolvedApiBase() === LOCALHOST_API_BASE &&
      typeof window !== "undefined" &&
      typeof (window as typeof window & { __TAURI_INTERNALS__?: unknown })
        .__TAURI_INTERNALS__ !== "undefined";
    await Promise.all([loadDownloadDirectoryTree(), loadProviderStatuses()]);
  },
);

const resultKey = (result: SearchResult): string =>
  `${result.source}:${result.id}`;

const buildDefaultLyricSearchQuery = (result: SearchResult): string => {
  const manualQuery = searchInput.value.trim();
  if (manualQuery.length > 0) {
    return manualQuery;
  }
  return `${result.title} ${result.artist}`.trim();
};

const sourceLabel = (source: DownloadSource): string => {
  switch (source) {
    case "youtube":
      return "YouTube";
    case "netease":
      return "网易云";
    case "bilibili":
      return "Bilibili";
  }
};

const syncSelectedSources = () => {
  selectedSources.value = selectedSources.value.filter(
    (source) => providerStatus[source].enabled,
  );
};

const toggleSource = (source: DownloadSource) => {
  if (!providerStatus[source].enabled) {
    return;
  }

  if (selectedSources.value.includes(source)) {
    selectedSources.value = selectedSources.value.filter(
      (item) => item !== source,
    );
    return;
  }

  selectedSources.value = [...selectedSources.value, source];
};

const toggleProviderSettings = () => {
  showProviderSettings.value = !showProviderSettings.value;
  if (!showProviderSettings.value) {
    managingProvider.value = null;
  }
};

const toggleDownloadFolderPicker = () => {
  showDownloadFolderPicker.value = !showDownloadFolderPicker.value;
};

const toggleManagingProvider = (provider: OnlineProvider) => {
  managingProvider.value =
    managingProvider.value === provider ? null : provider;
};

const providerLabel = (provider: OnlineProvider): string => {
  switch (provider) {
    case "youtube":
      return "YouTube";
    case "netease":
      return "网易云";
    case "bilibili":
      return "Bilibili";
  }
};

const loadProviderStatuses = async () => {
  try {
    const response = await fetch(resolvedApiBase() + "/download/providers", {
      cache: "no-store",
    });
    if (!response.ok) {
      const text = await response.text();
      throw new Error(text || "读取在线来源状态失败");
    }

    const payload = (await response.json()) as Array<{
      source: OnlineProvider;
      enabled: boolean;
      summary: string;
    }>;

    for (const provider of providerOptions) {
      const status = payload.find((item) => item.source === provider.value);
      providerStatus[provider.value] = {
        provider: provider.value,
        enabled: status?.enabled ?? false,
        summary: status?.summary ?? "未提供状态信息",
      };
    }
    syncSelectedSources();
  } catch (error) {
    for (const provider of providerOptions) {
      providerStatus[provider.value] = {
        provider: provider.value,
        enabled: false,
        summary: "无法读取来源状态",
      };
    }
    syncSelectedSources();
    console.warn("Failed to load provider statuses:", error);
  }
};

const loadDownloadDirectoryTree = async () => {
  isLoadingDownloadDirectories.value = true;
  downloadDirectoryError.value = "";

  try {
    const response = await fetch(
      resolvedApiBase() + "/download/directory-tree",
      {
        cache: "no-store",
      },
    );
    if (!response.ok) {
      const text = await response.text();
      throw new Error(text || "读取下载目录失败");
    }

    downloadDirectoryTree.value = await response.json();
    if (
      !downloadDirectoryOptions.value.some(
        (option) => option.path === selectedDownloadSubdir.value,
      )
    ) {
      selectedDownloadSubdir.value = "";
    }
  } catch (error) {
    downloadDirectoryError.value = `读取下载目录失败: ${error}`;
  } finally {
    isLoadingDownloadDirectories.value = false;
  }
};

const openLogin = async (provider: OnlineProvider) => {
  if (!supportsProviderAccountActions.value) {
    statusType.value = "info";
    statusMessage.value = "浏览器调试模式下无法管理登录状态";
    return;
  }

  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("online_open_login", { provider });
    statusType.value = "info";
    statusMessage.value = `已打开 ${providerLabel(provider)} 登录页面`;
  } catch (error) {
    statusType.value = "error";
    statusMessage.value = `打开登录失败: ${error}`;
  }
};

const captureLogin = async (provider: OnlineProvider) => {
  if (!supportsProviderAccountActions.value) {
    statusType.value = "info";
    statusMessage.value = "浏览器调试模式下无法同步登录信息";
    return;
  }

  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("online_capture_login", { provider });
    await loadProviderStatuses();
    statusType.value = "success";
    statusMessage.value = `${providerLabel(provider)} 登录信息已保存`;
  } catch (error) {
    statusType.value = "error";
    statusMessage.value = `读取登录信息失败: ${error}`;
  }
};

const logout = async (provider: OnlineProvider) => {
  if (!supportsProviderAccountActions.value) {
    statusType.value = "info";
    statusMessage.value = "浏览器调试模式下无法退出登录";
    return;
  }

  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("online_logout", { provider });
    await loadProviderStatuses();
    statusType.value = "success";
    statusMessage.value = `${providerLabel(provider)} 已退出`;
  } catch (error) {
    statusType.value = "error";
    statusMessage.value = `退出失败: ${error}`;
  }
};

const handleSearch = async () => {
  if (!searchInput.value.trim() || selectedSources.value.length === 0) {
    statusType.value = "error";
    statusMessage.value = "请先选择至少一个可用来源";
    return;
  }

  isSearching.value = true;
  statusMessage.value = "";
  searchResults.value = [];
  try {
    const response = await fetch(resolvedApiBase() + "/download/search", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        query: searchInput.value.trim(),
        max_results: 8,
        sources: selectedSources.value,
      }),
    });
    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(errorText || "搜索失败");
    }

    const payload = (await response.json()) as SearchResult[];
    searchResults.value = payload.filter((item) => item.can_download);
  } catch (error) {
    statusType.value = "error";
    statusMessage.value = `搜索失败: ${error}`;
  } finally {
    isSearching.value = false;
  }
};

const handlePreview = async (result: SearchResult) => {
  previewingKey.value = resultKey(result);
  statusType.value = "info";
  statusMessage.value = `正在准备试听: ${result.title}`;

  try {
    const response = await fetch(resolvedApiBase() + "/download/preview", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        source: result.source,
        id: result.id,
        title: result.title,
        artist: result.artist,
      }),
    });
    const payload = await response.json();
    if (!response.ok || !payload.success || !payload.song) {
      throw new Error(payload.message || "试听准备失败");
    }

    emit("previewTrack", {
      ...payload.song,
      lufs: null,
    });
    statusType.value = "success";
    statusMessage.value = `已开始试听: ${result.title}`;
  } catch (error) {
    statusType.value = "error";
    statusMessage.value = `试听失败: ${error}`;
  } finally {
    previewingKey.value = null;
  }
};

const toggleLyrics = (result: SearchResult) => {
  lyricPickerState.value = result;
};

const handleLyricSelected = (candidate: LyricCandidate) => {
  if (!lyricPickerState.value) {
    return;
  }
  selectedLyrics[resultKey(lyricPickerState.value)] = candidate;
  lyricPickerState.value = null;
};

const downloadTrackToApiBase = async (
  apiBase: string,
  result: SearchResult,
  lyricId: string | null,
  targetSubdir: string | null,
) => {
  const response = await fetch(apiBase + "/download/track", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      source: result.source,
      id: result.id,
      title: result.title,
      artist: result.artist,
      target_subdir: targetSubdir,
      lyric_selection: lyricId,
    }),
  });
  const data = await response.json();
  if (!response.ok || !data.success) {
    throw new Error(data.message || "下载失败");
  }
  return data as {
    filename?: string | null;
    warning?: string | null;
  };
};

const handleDownload = async (result: SearchResult) => {
  downloadingKey.value = resultKey(result);
  statusType.value = "info";
  statusMessage.value = `正在下载: ${result.title}`;
  try {
    const selectedLyric = selectedLyrics[resultKey(result)];
    const shouldAlsoSaveLocal =
      resolvedApiBase() !== LOCALHOST_API_BASE &&
      window.confirm(
        "是否同时保存一份到本机？\n\n选择“确定”会同时保存到当前来源曲库和本机。\n选择“取消”则只保存到当前来源曲库。",
      );

    const sharedResult = await downloadTrackToApiBase(
      resolvedApiBase(),
      result,
      selectedLyric?.id ?? null,
      selectedDownloadSubdir.value || null,
    );

    let warningMessage = sharedResult.warning || "";
    if (shouldAlsoSaveLocal) {
      try {
        await downloadTrackToApiBase(
          LOCALHOST_API_BASE,
          result,
          selectedLyric?.id ?? null,
          null,
        );
      } catch (error) {
        warningMessage = warningMessage
          ? `${warningMessage}；本机副本失败: ${error}`
          : `本机副本失败: ${error}`;
      }
    }

    statusType.value = warningMessage ? "info" : "success";
    statusMessage.value = warningMessage
      ? `下载完成: ${sharedResult.filename}，${warningMessage}`
      : shouldAlsoSaveLocal
        ? `下载完成: ${sharedResult.filename}，并已请求保存本机副本`
        : `下载完成: ${sharedResult.filename}`;
    emit("downloadComplete");
  } catch (error) {
    statusType.value = "error";
    statusMessage.value = `下载失败: ${error}`;
  } finally {
    downloadingKey.value = null;
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
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.modal-content {
  background-color: #fff;
  padding: 20px;
  border-radius: 12px;
  width: min(92vw, 760px);
  max-height: 90vh;
  overflow-y: auto;
}

.modal-content h3 {
  margin: 0 0 18px;
  text-align: center;
}

.source-caption {
  margin: -10px 0 16px;
  text-align: center;
  color: #687076;
  font-size: 14px;
}

.search-section,
.results-section {
  margin-bottom: 18px;
}

.search-input-row {
  display: flex;
  gap: 10px;
}

.provider-empty-state {
  margin-bottom: 16px;
  padding: 14px 16px;
  border-radius: 10px;
  background: #fff4e5;
  color: #7a4b00;
}

.provider-empty-title {
  font-weight: 700;
  margin-bottom: 4px;
}

.provider-empty-copy {
  line-height: 1.5;
}

.search-input-row input {
  flex: 1;
  padding: 10px 12px;
  border: 1px solid #d0d7de;
  border-radius: 8px;
}

.search-btn,
.action-btn,
.login-btn,
.secondary-btn,
.close-btn {
  border: none;
  border-radius: 8px;
  padding: 10px 14px;
  cursor: pointer;
}

.search-btn,
.download-btn,
.preview-btn,
.login-btn {
  background: #1db954;
  color: #fff;
}

.lyric-btn,
.secondary-btn,
.close-btn {
  background: #eceff3;
  color: #223;
}

.search-btn:disabled,
.action-btn:disabled {
  opacity: 0.65;
  cursor: not-allowed;
}

.source-row {
  display: flex;
  gap: 14px;
  flex-wrap: wrap;
  margin-top: 12px;
}

.source-checkbox {
  display: flex;
  align-items: center;
  gap: 6px;
}

.provider-settings,
.download-destination {
  margin-bottom: 18px;
}

.download-target-row {
  display: flex;
  align-items: center;
  gap: 10px;
  justify-content: space-between;
  margin-top: 8px;
}

.download-target-value {
  min-width: 0;
  flex: 1;
  font-size: 14px;
  color: #223;
  overflow-wrap: anywhere;
}

.download-folder-picker {
  margin-top: 12px;
  border: 1px solid #e4e8ee;
  border-radius: 10px;
  padding: 12px;
  background: #f9fbfc;
}

.directory-loading,
.directory-error {
  font-size: 14px;
  color: #556;
}

.directory-error {
  color: #b42318;
}

.download-folder-options {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.download-folder-option {
  border: 1px solid #d7dde6;
  border-radius: 999px;
  background: #fff;
  color: #223;
  padding: 8px 12px;
  cursor: pointer;
}

.download-folder-option.selected {
  background: #1db954;
  border-color: #1db954;
  color: #fff;
}

.provider-toggle {
  width: 100%;
  border: 1px solid #d7dde6;
  border-radius: 10px;
  background: #f8fafc;
  padding: 12px 14px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  font-weight: 600;
  color: #223;
}

.provider-settings-panel {
  margin-top: 10px;
  border: 1px solid #e4e8ee;
  border-radius: 10px;
  padding: 12px;
  background: #f9fbfc;
}

.provider-row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: 12px;
  padding: 10px 0;
  border-bottom: 1px solid #e7edf4;
}

.provider-row:last-child {
  border-bottom: none;
}

.provider-row-info {
  flex: 1;
  min-width: 0;
}

.provider-title {
  font-weight: 700;
  margin-bottom: 4px;
  overflow-wrap: anywhere;
}

.provider-summary {
  font-size: 13px;
  color: #4a5565;
  overflow-wrap: anywhere;
}

.provider-manage-btn {
  flex: none;
  margin-left: auto;
}

.provider-manage-panel {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  width: 100%;
  padding-top: 4px;
}

.setting-label {
  display: block;
  margin-bottom: 8px;
  font-weight: 600;
}

.permission-message {
  margin: 0;
  font-size: 13px;
  color: #556372;
}

.results-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.result-item {
  border: 1px solid #e5e7eb;
  border-radius: 10px;
  padding: 12px;
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  align-items: flex-start;
}

.result-thumbnail {
  width: 72px;
  height: 72px;
  flex-shrink: 0;
  object-fit: cover;
  border-radius: 8px;
  background: #e5e7eb;
}

.result-thumbnail.placeholder {
  background: linear-gradient(135deg, #dae3e8, #eef3f6);
}

.result-info {
  flex: 1;
  min-width: 0;
}

.result-header {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}

.result-title {
  font-weight: 700;
  color: #1f2937;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.source-badge {
  font-size: 11px;
  background: #d8f4e1;
  color: #126b37;
  padding: 2px 8px;
  border-radius: 999px;
}

.result-meta,
.selected-lyric,
.candidate-meta,
.empty-candidate {
  font-size: 13px;
  color: #576475;
  margin-top: 4px;
}

.result-duration {
  margin-left: 8px;
}

.result-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.lyrics-candidates {
  width: calc(100% - 84px);
  margin-left: 84px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.lyric-search-row {
  display: flex;
  gap: 8px;
}

.lyric-search-row input {
  flex: 1;
  min-width: 0;
  padding: 10px 12px;
  border: 1px solid #d0d7de;
  border-radius: 8px;
}

.lyric-tip {
  font-size: 13px;
  color: #576475;
}

.lyric-candidate {
  border: 1px solid #dde3ea;
  border-radius: 8px;
  padding: 8px 10px;
  cursor: pointer;
}

.lyric-candidate.selected {
  border-color: #1db954;
  background: #eefaf2;
}

.candidate-title {
  font-weight: 600;
}

.status-message {
  border-radius: 8px;
  padding: 10px 12px;
  font-size: 14px;
  margin-top: 12px;
}

.status-message.info {
  background: #e8f3ff;
  color: #0e4f96;
}

.status-message.success {
  background: #eaf8ef;
  color: #1b6a39;
}

.status-message.error {
  background: #fdecec;
  color: #a12626;
}

.modal-actions {
  display: flex;
  justify-content: center;
  margin-top: 18px;
}

@media (max-width: 640px) {
  .result-thumbnail {
    width: 56px;
    height: 56px;
  }

  .result-info {
    flex: 1;
    min-width: 0;
  }

  .result-actions {
    flex-basis: 100%;
    flex-direction: row;
    flex-wrap: wrap;
  }

  .lyrics-candidates {
    flex-basis: 100%;
    width: 100%;
    margin-left: 0;
  }
}
</style>
