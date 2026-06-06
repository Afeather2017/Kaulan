<template>
  <div class="library-source-list">
    <div v-for="group in groups" :key="group.sourceKey" class="source-group">
      <div class="source-header">
        <div class="source-title">
          <div class="source-name">{{ group.name }}</div>
          <div
            :class="[
              'source-status',
              group.isLoading
                ? 'loading'
                : group.isOnline
                  ? 'online'
                  : 'offline',
            ]"
          >
            {{
              group.isLoading
                ? "Loading"
                : group.isOnline
                  ? "Online"
                  : "Offline"
            }}
          </div>
        </div>
        <button class="source-menu-btn" @click="$emit('openMenu', group)">
          ⋮
        </button>
      </div>

      <div v-if="group.isLoading" class="loading-card">
        <div>Checking this source...</div>
      </div>

      <div v-else-if="group.isOnline" class="playlist-list">
        <button
          v-for="playlist in group.playlists"
          :key="`${group.sourceKey}:${playlist.name}`"
          class="playlist-row"
          @click="$emit('selectPlaylist', group, playlist.name)"
        >
          <span class="playlist-name">{{ playlist.name }}</span>
          <span class="playlist-count">{{ playlist.songCount }} 首</span>
        </button>
      </div>

      <div v-else class="offline-card">
        <div>Cannot reach this source right now</div>
        <button class="retry-btn" @click="$emit('retry', group)">Retry</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
export interface LibrarySourcePlaylistSummary {
  name: string;
  songCount: number;
}

export interface LibrarySourceGroupSummary {
  sourceKey: string;
  name: string;
  isLoading: boolean;
  isOnline: boolean;
  playlists: LibrarySourcePlaylistSummary[];
}

defineProps<{
  groups: LibrarySourceGroupSummary[];
}>();

defineEmits<{
  (
    e: "selectPlaylist",
    group: LibrarySourceGroupSummary,
    playlistName: string,
  ): void;
  (e: "openMenu", group: LibrarySourceGroupSummary): void;
  (e: "retry", group: LibrarySourceGroupSummary): void;
}>();
</script>

<style scoped>
.library-source-list {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 12px 0 20px;
}

.source-group {
  border: 1px solid #ececec;
  border-radius: 16px;
  background: #fff;
  overflow: hidden;
}

.source-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 16px;
  background: #fafafa;
  border-bottom: 1px solid #f0f0f0;
}

.source-title {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.source-name {
  font-size: 16px;
  font-weight: 600;
  overflow-wrap: anywhere;
  line-height: 1.35;
}

.source-status {
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
}

.source-status.online {
  color: #126b37;
  background: #e8f7ee;
}

.source-status.loading {
  color: #7a5a00;
  background: #fff4d6;
}

.source-status.offline {
  color: #9d2b2b;
  background: #fdecec;
}

.source-menu-btn {
  width: 36px;
  height: 36px;
  flex: none;
  border: none;
  background: #ededed;
  border-radius: 10px;
  font-size: 22px;
  line-height: 1;
  cursor: pointer;
}

.playlist-list {
  display: flex;
  flex-direction: column;
}

.loading-card {
  padding: 16px;
  color: #8a6a00;
}

.playlist-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  border: none;
  border-top: 1px solid #f5f5f5;
  background: transparent;
  padding: 14px 16px;
  text-align: left;
  cursor: pointer;
}

.playlist-row:first-child {
  border-top: none;
}

.playlist-row:hover {
  background: #fafafa;
}

.playlist-name {
  font-size: 15px;
  color: #222;
  overflow-wrap: anywhere;
}

.playlist-count {
  color: #777;
  font-size: 12px;
  white-space: nowrap;
}

.offline-card {
  padding: 16px;
  color: #666;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.retry-btn {
  align-self: flex-start;
  border: 1px solid #d7d7d7;
  background: #fff;
  color: #333;
  border-radius: 999px;
  padding: 8px 14px;
  cursor: pointer;
}
</style>
