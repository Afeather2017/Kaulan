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
        刷新曲库
      </button>
      <button
        v-if="selectedSourceMenuGroup.capabilities.canUpload"
        class="source-menu-action"
        @click="$emit('uploadToSource', selectedSourceMenuGroup)"
      >
        上传音乐
      </button>
      <button
        v-if="selectedSourceMenuGroup.capabilities.isCurrentOnlineSearchSource"
        class="source-menu-action current-action"
        disabled
      >
        当前用于在线搜索
      </button>
      <button
        v-else-if="selectedSourceMenuGroup.capabilities.canUseForOnlineSearch"
        class="source-menu-action"
        @click="$emit('setOnlineSearchSource', selectedSourceMenuGroup)"
      >
        用于在线搜索
      </button>
      <button
        v-if="selectedSourceMenuGroup.capabilities.canChangeDirectory"
        class="source-menu-action"
        @click="$emit('changeSourceDirectory', selectedSourceMenuGroup)"
      >
        更改目录
      </button>
      <button
        v-if="selectedSourceMenuGroup.capabilities.canRetryConnection"
        class="source-menu-action"
        @click="$emit('retrySourceConnection', selectedSourceMenuGroup.apiBase)"
      >
        重试连接
      </button>
      <button
        v-if="selectedSourceMenuGroup.capabilities.canShowSourceDetails"
        class="source-menu-action"
        @click="$emit('showSourceDetails', selectedSourceMenuGroup)"
      >
        来源详情
      </button>
      <button
        v-if="selectedSourceMenuGroup.capabilities.canDeleteSource"
        class="source-menu-action danger-action"
        @click="$emit('deleteSource', selectedSourceMenuGroup)"
      >
        删除来源
      </button>
    </div>
  </div>

  <div
    v-if="selectedSongListMenuTitle"
    class="modal-overlay source-menu-overlay"
    @click="$emit('closeSongListMenu')"
  >
    <div class="source-menu-sheet" @click.stop>
      <div class="source-menu-title">
        {{ selectedSongListMenuTitle }}
      </div>
      <button
        class="source-menu-action"
        @click="$emit('startSongListCollectionSelection')"
      >
        多选
      </button>
      <button
        v-if="canDownloadToLocal"
        class="source-menu-action"
        @click="$emit('startSongListDownloadSelection')"
      >
        下载到本机
      </button>
      <button
        class="source-menu-action danger-action"
        @click="$emit('startSongListDeleteSelection')"
      >
        删除
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
</template>

<script setup lang="ts">
import type { LibrarySourceGroup } from "@/types/library";

defineProps<{
  activeTab: "library" | "collections";
  selectedSourceMenuGroup: LibrarySourceGroup | null;
  selectedSongListMenuTitle: string | null;
  selectedCollectionMenuName: string | null;
  canDownloadToLocal: boolean;
}>();

defineEmits<{
  (e: "closeSourceMenu"): void;
  (e: "refreshSource", group: LibrarySourceGroup): void;
  (e: "uploadToSource", group: LibrarySourceGroup): void;
  (e: "setOnlineSearchSource", group: LibrarySourceGroup): void;
  (e: "changeSourceDirectory", group: LibrarySourceGroup): void;
  (e: "retrySourceConnection", apiBase: string): void;
  (e: "showSourceDetails", group: LibrarySourceGroup): void;
  (e: "deleteSource", group: LibrarySourceGroup): void;
  (e: "closeSongListMenu"): void;
  (e: "startSongListCollectionSelection"): void;
  (e: "startSongListDownloadSelection"): void;
  (e: "startSongListDeleteSelection"): void;
  (e: "closeCollectionMenu"): void;
  (e: "renameCollection"): void;
  (e: "deleteCollection"): void;
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

.source-menu-action.current-action {
  color: #687076;
}
</style>
