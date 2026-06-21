<template>
  <template v-if="isPlayerPanelVisible">
    <div class="right-panel">
      <div class="right-panel-content">
        <div
          v-if="isLyricPanelVisible"
          class="lyric-panel"
          @click.self="$emit('showCoverPanel')"
        >
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
          <div v-else class="lyric-container" ref="lyricContainerRef">
            <div
              v-for="(line, index) in lyrics"
              :key="index"
              :class="['lyric-line', { active: index === currentLyricIndex }]"
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
        <div v-else class="cover-panel" @click="$emit('showLyricsPanel')">
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
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import PlayerControls from "@/components/PlayerControls.vue";
import type { LyricLine } from "@/composables/useLyrics";

const props = defineProps<{
  isPlayerPanelVisible: boolean;
  isWideLayout: boolean;
  isLyricPanelVisible: boolean;
  selectMode: boolean;
  isLyricsLoading: boolean;
  hasLyrics: boolean;
  lyrics: LyricLine[];
  currentLyricIndex: number;
  currentSongId: number | null;
  currentSongName?: string;
  coverUrl?: string | null;
  currentTime: number;
  duration: number;
  isPlaying: boolean;
  playMode: "sequential" | "shuffle" | "loop";
}>();

defineEmits<{
  (e: "lyricLineClick", time: number): void;
  (e: "openOnlineLyricSearch"): void;
  (e: "showCoverPanel"): void;
  (e: "showLyricsPanel"): void;
  (e: "coverLoadError"): void;
  (e: "seek", time: number): void;
  (e: "togglePlayMode"): void;
  (e: "previous"): void;
  (e: "play"): void;
  (e: "pause"): void;
  (e: "next"): void;
  (e: "showActiveQueue"): void;
  (e: "togglePanelMode"): void;
}>();

const lyricContainerRef = ref<HTMLElement | null>(null);

const scrollToCurrentLyric = () => {
  if (!lyricContainerRef.value || props.currentLyricIndex < 0) return;

  const lyricLines = lyricContainerRef.value.querySelectorAll(".lyric-line");
  if (lyricLines.length === 0) return;

  const activeLine = lyricLines[props.currentLyricIndex] as HTMLElement;
  if (!activeLine) return;

  activeLine.scrollIntoView({
    behavior: "smooth",
    block: "center",
  });
};

watch(() => props.currentLyricIndex, scrollToCurrentLyric);

watch(
  () => props.isLyricPanelVisible,
  (isShown) => {
    if (isShown) {
      setTimeout(scrollToCurrentLyric, 50);
    }
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
  overflow-y: auto;
  background-color: #fafafa;
}

.lyric-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
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

.lyric-line {
  text-align: center;
  cursor: pointer;
  transition:
    color 0.2s,
    opacity 0.2s;
  opacity: 0.64;
  outline: none;
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
</style>
