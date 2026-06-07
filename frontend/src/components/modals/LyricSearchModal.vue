<template>
  <div class="modal-overlay" @click="$emit('close')">
    <div class="modal-content" @click.stop>
      <h3>{{ mode === "apply" ? "在线查找歌词" : "选择歌词" }}</h3>
      <p class="song-caption">{{ initialQuery }}</p>

      <LyricSearchPanel
        :api-base="apiBase"
        :initial-query="initialQuery"
        :selected-lyric="selectedLyric"
        :show-action-button="true"
        :is-applying="isApplying"
        :auto-search="true"
        :apply-label="actionLabel"
        :applying-label="applyingLabel"
        :helper-text="helperText"
        @update:selected-lyric="selectedLyric = $event"
        @apply="applyLyric"
      />

      <div v-if="statusMessage" :class="['status-message', statusType]">
        {{ statusMessage }}
      </div>

      <div class="modal-actions">
        <button class="close-btn" @click="$emit('close')">关闭</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import LyricSearchPanel, {
  type LyricCandidate,
} from "@/components/LyricSearchPanel.vue";

const emit = defineEmits<{
  (e: "close"): void;
  (e: "applied"): void;
  (e: "selected", lyric: LyricCandidate): void;
}>();

const props = defineProps<{
  apiBase: string;
  initialQuery: string;
  mode: "pick" | "apply";
  songId?: number;
}>();

const selectedLyric = ref<LyricCandidate | null>(null);
const isApplying = ref(false);
const statusMessage = ref("");
const statusType = ref<"success" | "error">("success");

const actionLabel = computed(() =>
  props.mode === "apply" ? "保存歌词" : "选择歌词",
);

const applyingLabel = computed(() =>
  props.mode === "apply" ? "保存中..." : "选择中...",
);

const helperText = computed(() =>
  props.mode === "apply"
    ? "选择歌词后保存到当前歌曲。"
    : "选择后会返回上一层并关联到当前搜索结果。",
);

const applyLyric = async (candidate: LyricCandidate) => {
  if (props.mode === "pick") {
    emit("selected", candidate);
    emit("close");
    return;
  }

  if (typeof props.songId !== "number") {
    statusType.value = "error";
    statusMessage.value = "缺少歌曲信息，无法保存歌词";
    return;
  }

  isApplying.value = true;
  statusMessage.value = "";

  try {
    const response = await fetch(props.apiBase + "/download/lyrics/apply", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        song_id: props.songId,
        lyric_selection: candidate.id,
      }),
    });
    const payload = await response.json();
    if (!response.ok || !payload.success) {
      throw new Error(payload.message || "歌词保存失败");
    }

    statusType.value = "success";
    statusMessage.value = payload.lyric_filename
      ? `歌词已保存: ${payload.lyric_filename}`
      : "歌词已保存";
    emit("applied");
    emit("close");
  } catch (error) {
    statusType.value = "error";
    statusMessage.value = `歌词保存失败: ${error}`;
  } finally {
    isApplying.value = false;
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
  z-index: 120;
}

.modal-content {
  background-color: #fff;
  padding: 20px;
  border-radius: 12px;
  width: min(92vw, 640px);
  max-height: 90vh;
  overflow-y: auto;
}

.modal-content h3 {
  margin: 0 0 10px;
  text-align: center;
}

.song-caption {
  margin: 0 0 16px;
  text-align: center;
  color: #687076;
}

.status-message {
  margin-top: 12px;
  font-size: 13px;
  text-align: center;
}

.status-message.success {
  color: #176b3a;
}

.status-message.error {
  color: #b42318;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 16px;
}

.close-btn {
  border: 1px solid #d7dce1;
  background: #fff;
  color: #1b1f24;
  border-radius: 10px;
  padding: 10px 16px;
  font-size: 14px;
  cursor: pointer;
}
</style>
