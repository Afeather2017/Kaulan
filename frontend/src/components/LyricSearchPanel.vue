<template>
  <div class="lyrics-candidates">
    <div class="lyric-search-row">
      <input
        v-model="searchInput"
        type="text"
        placeholder="输入歌词搜索关键字..."
        @keyup.enter="searchLyrics"
      />
      <button
        class="secondary-btn"
        @click="searchLyrics"
        :disabled="isLoading || !searchInput.trim()"
      >
        {{ isLoading ? "搜索中" : "搜索歌词" }}
      </button>
    </div>
    <div class="lyric-tip">
      {{ helperText }}
    </div>
    <div
      v-for="candidate in candidates"
      :key="candidate.id"
      :class="[
        'lyric-candidate',
        { selected: selectedLyric?.id === candidate.id },
      ]"
      @click="selectLyric(candidate)"
    >
      <div class="candidate-title">{{ candidate.title }}</div>
      <div class="candidate-meta">
        {{ candidate.artist }}
        <span v-if="candidate.album"> / {{ candidate.album }}</span>
      </div>
    </div>
    <div v-if="!isLoading && candidates.length === 0" class="empty-candidate">
      未找到可选歌词
    </div>
    <div v-if="showActionButton" class="panel-actions">
      <button
        class="apply-btn"
        :disabled="isApplying || !selectedLyric"
        @click="handleApply"
      >
        {{ isApplying ? applyingLabel : applyLabel }}
      </button>
    </div>
    <div v-if="statusMessage" :class="['status-message', statusType]">
      {{ statusMessage }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";

export interface LyricCandidate {
  source: "youtube" | "netease" | "bilibili";
  id: string;
  title: string;
  artist: string;
  album?: string | null;
}

const emit = defineEmits<{
  (e: "update:selectedLyric", value: LyricCandidate | null): void;
  (e: "apply", value: LyricCandidate): void;
}>();

const props = withDefaults(
  defineProps<{
    apiBase: string;
    initialQuery?: string;
    selectedLyric?: LyricCandidate | null;
    helperText?: string;
    showActionButton?: boolean;
    applyLabel?: string;
    applyingLabel?: string;
    isApplying?: boolean;
    autoSearch?: boolean;
  }>(),
  {
    initialQuery: "",
    selectedLyric: null,
    helperText: "选择歌词后即可保存。",
    showActionButton: false,
    applyLabel: "保存歌词",
    applyingLabel: "保存中...",
    isApplying: false,
    autoSearch: false,
  },
);

const searchInput = ref("");
const candidates = ref<LyricCandidate[]>([]);
const isLoading = ref(false);
const statusMessage = ref("");
const statusType = ref<"info" | "error">("info");

async function searchLyrics() {
  const query = searchInput.value.trim();
  if (!query) {
    statusType.value = "error";
    statusMessage.value = "请输入歌词搜索关键字";
    return;
  }

  isLoading.value = true;
  statusMessage.value = "";
  emit("update:selectedLyric", null);

  try {
    const response = await fetch(props.apiBase + "/download/lyrics/search", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ query }),
    });
    if (!response.ok) {
      const text = await response.text();
      throw new Error(text || "歌词搜索失败");
    }
    candidates.value = await response.json();
  } catch (error) {
    candidates.value = [];
    statusType.value = "error";
    statusMessage.value = `歌词搜索失败: ${error}`;
  } finally {
    isLoading.value = false;
  }
}

watch(
  () => props.initialQuery,
  (value) => {
    searchInput.value = value.trim();
    candidates.value = [];
    statusMessage.value = "";
    emit("update:selectedLyric", null);
    if (props.autoSearch && searchInput.value) {
      void searchLyrics();
    }
  },
  { immediate: true },
);

const selectLyric = (candidate: LyricCandidate) => {
  emit("update:selectedLyric", candidate);
};

const handleApply = () => {
  if (!props.selectedLyric) {
    return;
  }
  emit("apply", props.selectedLyric);
};
</script>

<style scoped>
.lyrics-candidates {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.lyric-search-row {
  display: flex;
  gap: 8px;
}

.lyric-search-row input {
  flex: 1;
  min-width: 0;
  padding: 10px 12px;
  border: 1px solid #d7dce1;
  border-radius: 10px;
  font-size: 14px;
}

.lyric-tip {
  color: #687076;
  font-size: 13px;
}

.lyric-candidate {
  padding: 12px 14px;
  border: 1px solid #d7dce1;
  border-radius: 10px;
  cursor: pointer;
  background: #fff;
}

.lyric-candidate.selected {
  border-color: #176b3a;
  background: #eef8f1;
}

.candidate-title {
  color: #1b1f24;
  font-weight: 600;
}

.candidate-meta {
  margin-top: 4px;
  color: #687076;
  font-size: 13px;
}

.empty-candidate {
  color: #687076;
  font-size: 14px;
  text-align: center;
  padding: 16px 0;
}

.panel-actions {
  display: flex;
  justify-content: flex-end;
}

.apply-btn,
.secondary-btn {
  border: 1px solid #176b3a;
  background: #176b3a;
  color: #fff;
  border-radius: 999px;
  padding: 10px 16px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}

.secondary-btn {
  border-radius: 10px;
}

.apply-btn:disabled,
.secondary-btn:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.status-message {
  font-size: 13px;
}

.status-message.error {
  color: #b42318;
}

.status-message.info {
  color: #176b3a;
}
</style>
