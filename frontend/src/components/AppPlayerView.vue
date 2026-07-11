<template>
  <template v-if="isPlayerPanelVisible">
    <div class="right-panel">
      <div class="right-panel-content">
        <div
          v-if="isLyricPanelVisible"
          class="lyric-panel"
          @click.self="handlePanelBackdropClick"
        >
          <div class="lyric-toolbar">
            <button
              v-if="!isWideLayout"
              type="button"
              class="lyric-toolbar-btn"
              @click="handleBackAction"
            >
              Back
            </button>
            <div v-else class="lyric-toolbar-title">
              {{ isLyricEditMode ? "Edit Lyrics" : "Lyrics" }}
            </div>
            <div class="lyric-toolbar-actions">
              <template v-if="isLyricEditMode">
                <button
                  type="button"
                  class="lyric-toolbar-btn"
                  :disabled="isSavingLyric"
                  @click="handleCancelLyricEdit"
                >
                  Cancel
                </button>
                <button
                  type="button"
                  class="lyric-toolbar-btn primary"
                  :disabled="isSavingLyric"
                  @click="handleDoneLyricEdit"
                >
                  {{ isSavingLyric ? "Saving..." : "Done" }}
                </button>
              </template>
              <template v-else-if="hasLyrics">
                <button
                  type="button"
                  class="lyric-toolbar-btn"
                  @click="enterLyricEditMode"
                >
                  Edit
                </button>
                <button
                  type="button"
                  class="lyric-toolbar-btn"
                  @click="openRawLyricEditor"
                >
                  Edit text
                </button>
              </template>
            </div>
          </div>
          <div v-if="isLyricsLoading" class="lyric-empty">歌词加载中...</div>
          <div v-else-if="!hasLyrics" class="lyric-empty">
            <div class="lyric-empty-content">
              <div>暂无歌词</div>
              <button
                v-if="currentSongName"
                type="button"
                class="lyric-search-btn"
                @click.stop="$emit('openOnlineLyricSearch')"
              >
                search online
              </button>
            </div>
          </div>
          <div v-else ref="lyricScrollRef" class="lyric-body">
            <div v-if="isLyricEditMode" class="lyric-edit-floating">
              <button
                type="button"
                class="lyric-shift-btn"
                :disabled="isSavingLyric"
                @click="applyLyricShift(-sanitizedLyricStepMs)"
              >
                -
              </button>
              <label class="lyric-step-input-wrap">
                <input
                  v-model="lyricShiftStepInput"
                  type="number"
                  inputmode="numeric"
                  min="0"
                  :max="maxLyricShiftMs"
                  step="1"
                  class="lyric-step-input"
                  aria-label="Lyric shift step in milliseconds"
                  :disabled="isSavingLyric"
                />
                <span class="lyric-step-unit">ms</span>
              </label>
              <button
                type="button"
                class="lyric-shift-btn"
                :disabled="isSavingLyric"
                @click="applyLyricShift(sanitizedLyricStepMs)"
              >
                +
              </button>
            </div>
            <div v-if="lyricSaveError" class="lyric-save-error">
              {{ lyricSaveError }}
            </div>
            <div
              ref="lyricContainerRef"
              class="lyric-container"
              :class="{ 'is-editing': isLyricEditMode }"
            >
              <div
                v-for="(line, index) in displayedLyrics"
                :key="index"
                :class="[
                  'lyric-line',
                  { active: index === displayedCurrentLyricIndex },
                ]"
                role="button"
                tabindex="0"
                @click="$emit('lyricLineClick', line.time)"
                @keydown.enter.prevent="$emit('lyricLineClick', line.time)"
                @keydown.space.prevent="$emit('lyricLineClick', line.time)"
              >
                <template
                  v-for="(text, textIndex) in line.texts"
                  :key="textIndex"
                >
                  <div :class="['lyric-text', `lyric-lang-${textIndex}`]">
                    {{ text || "\u00A0" }}
                  </div>
                </template>
              </div>
            </div>
          </div>
        </div>
        <div v-else class="cover-panel">
          <div v-if="!isWideLayout" class="cover-toolbar">
            <button
              type="button"
              class="cover-toolbar-btn"
              @click.stop="handleBackAction"
            >
              Back
            </button>
          </div>
          <div class="cover-body" @click="$emit('showLyricsPanel')">
            <div class="cover-panel-placeholder">
              <i class="fas fa-music"></i>
            </div>
            <img
              v-if="currentSongId && coverUrl"
              :src="coverUrl"
              :key="coverUrl || currentSongId"
              class="cover-image"
              @error="$emit('coverLoadError')"
              @load="($event.target as HTMLImageElement).style.display = ''"
              alt=""
            />
          </div>
        </div>

        <PlayerControls
          v-if="isWideLayout && !selectMode"
          :current-time="currentTime"
          :duration="duration"
          :is-playing="isPlaying"
          :play-mode="playMode"
          :current-song-name="currentSongName"
          :cover-url="coverUrl"
          @seek="$emit('seek', $event)"
          @toggle-play-mode="$emit('togglePlayMode')"
          @previous="$emit('previous')"
          @play="$emit('play')"
          @pause="$emit('pause')"
          @next="$emit('next')"
          @show-active-queue="$emit('showActiveQueue')"
          @show-player-panel="$emit('togglePanelMode')"
        />
      </div>
    </div>
  </template>

  <PlayerControls
    v-if="!isWideLayout && !selectMode"
    :current-time="currentTime"
    :duration="duration"
    :is-playing="isPlaying"
    :play-mode="playMode"
    :current-song-name="currentSongName"
    :cover-url="coverUrl"
    @seek="$emit('seek', $event)"
    @toggle-play-mode="$emit('togglePlayMode')"
    @previous="$emit('previous')"
    @play="$emit('play')"
    @pause="$emit('pause')"
    @next="$emit('next')"
    @show-active-queue="$emit('showActiveQueue')"
    @show-player-panel="$emit('togglePanelMode')"
  />

  <EditRawLyricsModal
    v-if="isRawLyricEditOpen"
    :music-id="currentSongId"
    :raw-lyrics-content="rawLyricsContent"
    :lyric-api-base="lyricApiBase"
    @close="isRawLyricEditOpen = false"
    @saved="handleRawLyricSaved"
  />
</template>

<script setup lang="ts">
// Related documentation: `docs/lyric-editing.md`
import { computed, ref, watch } from "vue";
import PlayerControls from "@/components/PlayerControls.vue";
import EditRawLyricsModal from "@/components/modals/EditRawLyricsModal.vue";
import { shiftLyricsContent, type LyricLine } from "@/composables/useLyrics";

const props = defineProps<{
  isPlayerPanelVisible: boolean;
  isWideLayout: boolean;
  isLyricPanelVisible: boolean;
  selectMode: boolean;
  isLyricsLoading: boolean;
  hasLyrics: boolean;
  lyrics: LyricLine[];
  rawLyricsContent: string | null;
  currentSongId: number | null;
  lyricApiBase: string;
  currentSongName?: string;
  coverUrl?: string | null;
  currentTime: number;
  duration: number;
  isPlaying: boolean;
  playMode: "sequential" | "shuffle" | "loop";
}>();

const emit = defineEmits<{
  (e: "lyricLineClick", time: number): void;
  (e: "openOnlineLyricSearch"): void;
  (e: "showCoverPanel"): void;
  (e: "showLyricsPanel"): void;
  (e: "requestPlayerBack"): void;
  (e: "coverLoadError"): void;
  (e: "seek", time: number): void;
  (e: "togglePlayMode"): void;
  (e: "previous"): void;
  (e: "play"): void;
  (e: "pause"): void;
  (e: "next"): void;
  (e: "showActiveQueue"): void;
  (e: "togglePanelMode"): void;
  (e: "lyricsSaved"): void;
}>();

const lyricContainerRef = ref<HTMLElement | null>(null);
const lyricScrollRef = ref<HTMLElement | null>(null);
const isLyricEditMode = ref(false);
const draftLyricShiftMs = ref(0);
const lyricShiftStepInput = ref("100");
const lyricContentBase = ref<string | null>(null);
const isSavingLyric = ref(false);
const lyricSaveError = ref<string | null>(null);
const isRawLyricEditOpen = ref(false);

const maxLyricShiftMs = computed(() =>
  Math.max(0, Math.round(props.duration * 1000)),
);

const findLyricIndex = (lines: LyricLine[], time: number) => {
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    if (lines[index].time <= time) {
      return index;
    }
  }

  return -1;
};

const sanitizedLyricStepMs = computed(() => {
  const parsed = Number.parseInt(lyricShiftStepInput.value, 10);
  if (Number.isNaN(parsed)) {
    return 100;
  }

  return Math.min(maxLyricShiftMs.value, Math.max(0, parsed));
});

const effectiveLyricShiftMs = computed(() =>
  isLyricEditMode.value ? draftLyricShiftMs.value : 0,
);

const displayedLyrics = computed<LyricLine[]>(() => {
  const shiftSeconds = effectiveLyricShiftMs.value / 1000;
  if (shiftSeconds === 0) {
    return props.lyrics;
  }

  return props.lyrics.map((line) => ({
    ...line,
    time: Math.max(0, line.time + shiftSeconds),
  }));
});

const displayedCurrentLyricIndex = computed(() =>
  findLyricIndex(displayedLyrics.value, props.currentTime),
);

const resetLyricEditState = () => {
  isLyricEditMode.value = false;
  draftLyricShiftMs.value = 0;
  lyricShiftStepInput.value = "100";
  isSavingLyric.value = false;
  lyricSaveError.value = null;
  isRawLyricEditOpen.value = false;
};

const scrollToCurrentLyric = () => {
  const scrollContainer = lyricScrollRef.value;
  const lyricContainer = lyricContainerRef.value;
  if (
    !scrollContainer ||
    !lyricContainer ||
    displayedCurrentLyricIndex.value < 0
  ) {
    return;
  }

  const lyricLines = lyricContainer.querySelectorAll(".lyric-line");
  if (lyricLines.length === 0) {
    return;
  }

  const activeLine = lyricLines[displayedCurrentLyricIndex.value] as
    | HTMLElement
    | undefined;
  if (!activeLine) {
    return;
  }

  const containerRect = scrollContainer.getBoundingClientRect();
  const activeRect = activeLine.getBoundingClientRect();
  const targetTop =
    scrollContainer.scrollTop +
    (activeRect.top - containerRect.top) -
    scrollContainer.clientHeight / 2 +
    activeRect.height / 2;

  scrollContainer.scrollTo({
    top: Math.max(0, targetTop),
    behavior: "smooth",
  });
};

const enterLyricEditMode = () => {
  if (!props.hasLyrics || isLyricEditMode.value || !lyricContentBase.value) {
    return;
  }

  if (props.isPlaying) {
    emit("pause");
  }

  isLyricEditMode.value = true;
  draftLyricShiftMs.value = 0;
  lyricSaveError.value = null;
};

const applyLyricShift = (deltaMs: number) => {
  if (!isLyricEditMode.value) {
    return;
  }

  draftLyricShiftMs.value = Math.max(
    -maxLyricShiftMs.value,
    Math.min(maxLyricShiftMs.value, draftLyricShiftMs.value + deltaMs),
  );
};

const handleCancelLyricEdit = () => {
  if (
    draftLyricShiftMs.value !== 0 &&
    typeof window !== "undefined" &&
    !window.confirm("Discard lyric timing changes?")
  ) {
    return;
  }

  isLyricEditMode.value = false;
  draftLyricShiftMs.value = 0;
};

const handleDoneLyricEdit = async () => {
  if (!props.currentSongId || !lyricContentBase.value || isSavingLyric.value) {
    return;
  }

  const updatedContent = shiftLyricsContent(
    lyricContentBase.value,
    draftLyricShiftMs.value,
  );

  isSavingLyric.value = true;
  lyricSaveError.value = null;

  try {
    const response = await fetch(
      `${props.lyricApiBase}/lyrics/id/${props.currentSongId}`,
      {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          content: updatedContent,
        }),
      },
    );

    if (!response.ok) {
      const payload = (await response.json().catch(() => null)) as {
        message?: string;
      } | null;
      lyricSaveError.value =
        payload?.message || `Failed to save lyrics (${response.status})`;
      return;
    }

    lyricContentBase.value = updatedContent;
    draftLyricShiftMs.value = 0;
    isLyricEditMode.value = false;
    emit("lyricsSaved");
  } catch (error) {
    console.error("Failed to save lyric content:", error);
    lyricSaveError.value = "Failed to save lyrics";
  } finally {
    isSavingLyric.value = false;
  }
};

const handleBackAction = () => {
  if (isLyricEditMode.value) {
    const shouldClosePanel =
      draftLyricShiftMs.value === 0 ||
      typeof window === "undefined" ||
      window.confirm("Discard lyric timing changes and leave the editor?");
    if (!shouldClosePanel) {
      return;
    }

    isLyricEditMode.value = false;
    draftLyricShiftMs.value = 0;
  }

  emit("requestPlayerBack");
};

const handlePanelBackdropClick = () => {
  if (isLyricEditMode.value) {
    return;
  }

  emit("showCoverPanel");
};

const openRawLyricEditor = () => {
  if (!props.hasLyrics || isLyricEditMode.value || isRawLyricEditOpen.value) {
    return;
  }

  if (props.isPlaying) {
    emit("pause");
  }

  isRawLyricEditOpen.value = true;
};

const handleRawLyricSaved = () => {
  isRawLyricEditOpen.value = false;
  emit("lyricsSaved");
};

watch(displayedCurrentLyricIndex, scrollToCurrentLyric);

watch(
  () => props.rawLyricsContent,
  (content) => {
    lyricContentBase.value = content;
  },
  { immediate: true },
);

watch(
  () => props.isLyricPanelVisible,
  (isShown) => {
    if (isShown) {
      setTimeout(scrollToCurrentLyric, 50);
      return;
    }

    resetLyricEditState();
  },
);

watch(
  () => props.currentSongId,
  () => {
    resetLyricEditState();
  },
);
</script>

<style scoped>
.right-panel {
  flex: 1;
  min-height: 0;
  background-color: #fafafa;
  border-left: 1px solid #eee;
}

.right-panel-content {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.lyric-panel {
  flex: 1;
  min-height: 0;
  background-color: #fafafa;
  display: flex;
  flex-direction: column;
}

.lyric-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 18px 12px;
  border-bottom: 1px solid #eee;
  background: rgba(250, 250, 250, 0.96);
  flex-shrink: 0;
}

.lyric-toolbar-title {
  color: #31414f;
  font-size: 14px;
  font-weight: 700;
}

.lyric-toolbar-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
}

.lyric-toolbar-btn {
  border: 1px solid #d7dee6;
  background: #fff;
  color: #31414f;
  border-radius: 999px;
  padding: 8px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.lyric-toolbar-btn:disabled,
.lyric-shift-btn:disabled,
.lyric-step-input:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.lyric-toolbar-btn.primary {
  border-color: #176b3a;
  background: #176b3a;
  color: #fff;
}

.lyric-body {
  flex: 1;
  min-height: 0;
  position: relative;
  overflow-y: auto;
}

.lyric-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: #888;
  font-size: 16px;
}

.lyric-empty-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.lyric-search-btn {
  border: 1px solid #176b3a;
  background: #176b3a;
  color: #fff;
  border-radius: 999px;
  padding: 10px 18px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}

.lyric-search-btn:hover,
.lyric-search-btn:focus-visible {
  background: #145a31;
  border-color: #145a31;
  outline: none;
}

.lyric-edit-floating {
  position: sticky;
  top: 16px;
  width: max-content;
  margin: 16px 16px 0 auto;
  z-index: 2;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border: 1px solid rgba(23, 107, 58, 0.14);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 8px 24px rgba(20, 36, 46, 0.12);
}

.lyric-shift-btn {
  width: 34px;
  height: 34px;
  border: 1px solid #cfd8df;
  border-radius: 999px;
  background: #fff;
  color: #31414f;
  font-size: 18px;
  font-weight: 700;
  line-height: 1;
  cursor: pointer;
}

.lyric-step-input-wrap {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 10px;
  height: 34px;
  border: 1px solid #cfd8df;
  border-radius: 999px;
  background: #fff;
}

.lyric-step-input {
  width: 52px;
  border: none;
  background: transparent;
  color: #1b1f24;
  font-size: 14px;
  font-weight: 600;
  text-align: center;
  outline: none;
}

.lyric-step-input::-webkit-outer-spin-button,
.lyric-step-input::-webkit-inner-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.lyric-step-unit {
  color: #5b6670;
  font-size: 12px;
  font-weight: 600;
}

.lyric-save-error {
  position: sticky;
  top: 86px;
  z-index: 1;
  width: max-content;
  max-width: calc(100% - 32px);
  margin: 8px 16px 0 auto;
  padding: 8px 12px;
  border: 1px solid #efc6c6;
  border-radius: 12px;
  background: rgba(255, 245, 245, 0.96);
  color: #9f2f2f;
  font-size: 12px;
  font-weight: 600;
}

.lyric-container {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  justify-content: center;
  min-height: 100%;
  padding: 32px 18px;
  box-sizing: border-box;
  gap: 12px;
}

.lyric-container.is-editing {
  align-items: flex-start;
  justify-content: flex-start;
  padding-top: 16px;
}

.lyric-line {
  text-align: center;
  cursor: pointer;
  transition:
    color 0.2s,
    opacity 0.2s;
  opacity: 0.64;
  outline: none;
}

.lyric-container.is-editing .lyric-line {
  width: 100%;
  text-align: left;
}

.lyric-line:hover,
.lyric-line:focus-visible {
  opacity: 0.9;
}

.lyric-text {
  line-height: 1.55;
  color: #5b6670;
}

.lyric-lang-0 {
  font-size: 17px;
  font-weight: 600;
}

.lyric-lang-1 {
  font-size: 14px;
}

.lyric-line.active .lyric-text {
  color: #1b1f24;
  opacity: 1;
}

.lyric-line.active .lyric-lang-0 {
  color: #176b3a;
}

.lyric-line.active .lyric-lang-1 {
  color: #31414f;
}

.cover-panel {
  flex: 1;
  width: 100%;
  border-top: 1px solid #eee;
  border-bottom: 1px solid #eee;
  background-color: #fff;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}

.cover-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 18px 12px;
  border-bottom: 1px solid #eee;
  background: rgba(250, 250, 250, 0.96);
  flex-shrink: 0;
}

.cover-toolbar-btn {
  border: 1px solid #d7dee6;
  background: #fff;
  color: #31414f;
  border-radius: 999px;
  padding: 8px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.cover-body {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  position: relative;
}

.cover-panel-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  color: #c1cad4;
  font-size: 92px;
}

.cover-image {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  left: 0;
  margin: auto;
}

@media (max-width: 720px) {
  .lyric-toolbar,
  .cover-toolbar {
    padding: 12px 14px 10px;
  }

  .lyric-edit-floating {
    top: 12px;
    margin: 12px 12px 0 auto;
    max-width: calc(100% - 24px);
  }

  .lyric-save-error {
    top: 78px;
    max-width: calc(100% - 24px);
    margin: 8px 12px 0 auto;
  }

  .lyric-container {
    padding: 28px 14px;
  }

  .lyric-container.is-editing {
    padding-top: 12px;
  }
}
</style>
