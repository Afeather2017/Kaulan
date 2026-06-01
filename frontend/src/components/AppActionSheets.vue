<template>
  <div
    v-if="selectedSourceMenuGroup"
    class="modal-overlay source-menu-overlay"
    @click="$emit('closeSourceMenu')"
  >
    <div class="source-menu-sheet" @click.stop>
      <div class="source-menu-title">
        {{ selectedSourceMenuGroup.name }}
      </div>
      <button
        v-if="selectedSourceMenuGroup.capabilities.canRefresh"
        class="source-menu-action"
        @click="$emit('refreshSource', selectedSourceMenuGroup)"
      >
        Refresh library
      </button>
      <button
        v-if="selectedSourceMenuGroup.capabilities.canUpload"
        class="source-menu-action"
        @click="$emit('uploadToSource', selectedSourceMenuGroup)"
      >
        Upload music
      </button>
      <button
        v-if="selectedSourceMenuGroup.capabilities.canOnlineDownload"
        class="source-menu-action"
        @click="$emit('openOnlineSearchForSource', selectedSourceMenuGroup)"
      >
        Online search/download
      </button>
      <button
        v-if="selectedSourceMenuGroup.capabilities.canChangeDirectory"
        class="source-menu-action"
        @click="$emit('changeSourceDirectory', selectedSourceMenuGroup)"
      >
        Change directory
      </button>
      <button
        v-if="selectedSourceMenuGroup.capabilities.canRetryConnection"
        class="source-menu-action"
        @click="$emit('retrySourceConnection', selectedSourceMenuGroup.apiBase)"
      >
        Retry connection
      </button>
      <button
        v-if="selectedSourceMenuGroup.capabilities.canShowSourceDetails"
        class="source-menu-action"
        @click="$emit('showSourceDetails', selectedSourceMenuGroup)"
      >
        Source details
      </button>
    </div>
  </div>

  <div
    v-if="selectedCollectionMenuName"
    class="modal-overlay source-menu-overlay"
    @click="$emit('closeCollectionMenu')"
  >
    <div class="source-menu-sheet" @click.stop>
      <div class="source-menu-title">
        {{ selectedCollectionMenuName }}
      </div>
      <button class="source-menu-action" @click="$emit('renameCollection')">
        Rename collection
      </button>
      <button
        class="source-menu-action danger-action"
        @click="$emit('deleteCollection')"
      >
        Delete collection
      </button>
    </div>
  </div>

  <div
    v-if="selectedSongMenuSong"
    class="modal-overlay source-menu-overlay"
    @click="$emit('closeSongMenu')"
  >
    <div class="source-menu-sheet" @click.stop>
      <div class="source-menu-title">
        {{ selectedSongMenuSong.name }}
      </div>
      <button class="source-menu-action" @click="$emit('queueSongNext')">
        Play next
      </button>
      <button class="source-menu-action" @click="$emit('addSongToQueue')">
        Add to queue
      </button>
      <button
        v-if="activeTab === 'library'"
        class="source-menu-action"
        @click="$emit('addSongToCollection')"
      >
        Add to collection
      </button>
      <button
        v-if="activeTab === 'collections'"
        class="source-menu-action danger-action"
        @click="$emit('removeSongFromCollection')"
      >
        Remove from collection
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { MusicInfo } from "@/composables/useAudioPlayer";

interface SourceCapabilities {
  canRefresh: boolean;
  canUpload: boolean;
  canChangeDirectory: boolean;
  canOnlineDownload: boolean;
  canRetryConnection: boolean;
  canShowSourceDetails: boolean;
}

interface LibrarySourceGroup {
  apiBase: string;
  sourceKey: string;
  name: string;
  isLoading: boolean;
  isOnline: boolean;
  playlists: Array<{ name: string; songs: MusicInfo[] }>;
  capabilities: SourceCapabilities;
}

defineProps<{
  activeTab: "library" | "collections";
  selectedSourceMenuGroup: LibrarySourceGroup | null;
  selectedCollectionMenuName: string | null;
  selectedSongMenuSong: MusicInfo | null;
}>();

defineEmits<{
  (e: "closeSourceMenu"): void;
  (e: "refreshSource", group: LibrarySourceGroup): void;
  (e: "uploadToSource", group: LibrarySourceGroup): void;
  (e: "openOnlineSearchForSource", group: LibrarySourceGroup): void;
  (e: "changeSourceDirectory", group: LibrarySourceGroup): void;
  (e: "retrySourceConnection", apiBase: string): void;
  (e: "showSourceDetails", group: LibrarySourceGroup): void;
  (e: "closeCollectionMenu"): void;
  (e: "renameCollection"): void;
  (e: "deleteCollection"): void;
  (e: "closeSongMenu"): void;
  (e: "queueSongNext"): void;
  (e: "addSongToQueue"): void;
  (e: "addSongToCollection"): void;
  (e: "removeSongFromCollection"): void;
}>();
</script>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  left: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  justify-content: center;
  z-index: 1000;
}

.source-menu-overlay {
  align-items: flex-end;
}

.source-menu-sheet {
  width: 100%;
  max-width: 560px;
  background: #fff;
  border-radius: 18px 18px 0 0;
  padding: 12px 16px 24px;
  box-sizing: border-box;
}

.source-menu-title {
  padding: 8px 4px 14px;
  font-size: 16px;
  font-weight: 700;
  color: #222;
  overflow-wrap: anywhere;
}

.source-menu-action {
  width: 100%;
  border: none;
  background: transparent;
  color: #222;
  text-align: left;
  padding: 14px 4px;
  font-size: 15px;
  cursor: pointer;
  border-top: 1px solid #f1f1f1;
}

.source-menu-action.danger-action {
  color: #b42318;
}
</style>
