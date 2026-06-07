import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { MusicInfo } from "@/composables/useAudioPlayer";
import {
  getLocalCollections,
  setLocalCollections,
  type StoredLocalCollection,
} from "@/utils/storage";

export const useCollectionsStore = defineStore("collections", () => {
  const localCollections = ref<StoredLocalCollection[]>([]);
  const showAddToCollection = ref(false);
  const selectedCollections = ref<number[]>([]);
  const pendingSongsForCollection = ref<MusicInfo[]>([]);
  const newCollectionName = ref("");
  const showCreateCollection = ref(false);
  const selectedCollectionMenuName = ref<string | null>(null);

  const collectionNames = computed(() =>
    localCollections.value.map((collection) => collection.name),
  );
  const collectionPlaylists = computed<Record<string, MusicInfo[]>>(() =>
    Object.fromEntries(
      localCollections.value.map((collection) => [
        collection.name,
        collection.songs,
      ]),
    ),
  );

  const syncLocalCollections = () => {
    setLocalCollections(localCollections.value);
  };

  const loadLocalCollections = (
    buildSongRowKey: (song: MusicInfo) => string,
    inferMediaType: (song: MusicInfo) => "audio" | "video",
  ) => {
    localCollections.value = getLocalCollections().map((collection) => ({
      ...collection,
      songs: collection.songs.map((song) => ({
        ...song,
        rowKey: buildSongRowKey(song),
        mediaType: song.mediaType || inferMediaType(song),
      })),
    }));
  };

  const showAddToCollectionModal = (songs: MusicInfo[] = []) => {
    selectedCollections.value = [];
    pendingSongsForCollection.value = songs.slice();
    showAddToCollection.value = true;
  };

  const hideAddToCollectionModal = () => {
    showAddToCollection.value = false;
    selectedCollections.value = [];
    pendingSongsForCollection.value = [];
  };

  const toggleCollectionSelection = (id: number) => {
    const index = selectedCollections.value.indexOf(id);
    if (index > -1) {
      selectedCollections.value.splice(index, 1);
      return;
    }
    selectedCollections.value.push(id);
  };

  const addSongToCollectionFromMenu = (song: MusicInfo) => {
    showAddToCollectionModal([song]);
  };

  const addToCollection = (
    currentSongs: MusicInfo[],
    selectedSongKeys: Set<string>,
    buildSongRowKey: (song: MusicInfo) => string,
    clearSongSelection: () => void,
  ) => {
    if (selectedCollections.value.length === 0) {
      alert("请选择至少一个收藏夹");
      return false;
    }

    const selectedVisibleSongs =
      pendingSongsForCollection.value.length > 0
        ? pendingSongsForCollection.value
        : currentSongs.filter((song) =>
            selectedSongKeys.has(song.rowKey || buildSongRowKey(song)),
          );

    if (selectedVisibleSongs.length === 0) {
      alert("没有选中的歌曲");
      return false;
    }

    localCollections.value = localCollections.value.map((collection) => {
      if (!selectedCollections.value.includes(collection.id)) {
        return collection;
      }

      const existingKeys = new Set(
        collection.songs.map(
          (song) => `${song.source_key || "local"}:${song.id}:${song.name}`,
        ),
      );
      const nextSongs = collection.songs.slice();

      for (const song of selectedVisibleSongs) {
        const songKey = `${song.source_key || "local"}:${song.id}:${song.name}`;
        if (existingKeys.has(songKey)) {
          continue;
        }

        existingKeys.add(songKey);
        nextSongs.push({
          ...song,
          rowKey: song.rowKey || songKey,
        });
      }

      return {
        ...collection,
        songs: nextSongs,
      };
    });

    syncLocalCollections();
    alert("添加成功");
    hideAddToCollectionModal();
    clearSongSelection();
    return true;
  };

  const removeSongsFromCollection = (
    collectionName: string,
    selectedSongKeys: Set<string>,
    buildSongRowKey: (song: MusicInfo) => string,
  ) => {
    if (selectedSongKeys.size === 0) {
      alert("没有选中的歌曲");
      return false;
    }

    localCollections.value = localCollections.value.map((collection) => {
      if (collection.name !== collectionName) {
        return collection;
      }

      return {
        ...collection,
        songs: collection.songs.filter(
          (song) => !selectedSongKeys.has(song.rowKey || buildSongRowKey(song)),
        ),
      };
    });

    syncLocalCollections();
    alert("移除成功");
    return true;
  };

  const removeSingleSongFromCollection = (
    collectionName: string,
    song: MusicInfo,
    buildSongRowKey: (item: MusicInfo) => string,
  ) => {
    const songKey = song.rowKey || buildSongRowKey(song);
    localCollections.value = localCollections.value.map((collection) => {
      if (collection.name !== collectionName) {
        return collection;
      }

      return {
        ...collection,
        songs: collection.songs.filter(
          (item) => (item.rowKey || buildSongRowKey(item)) !== songKey,
        ),
      };
    });

    syncLocalCollections();
  };

  const showCreateCollectionModal = () => {
    newCollectionName.value = "";
    showCreateCollection.value = true;
  };

  const createCollectionFromAddModal = () => {
    showAddToCollection.value = false;
    showCreateCollectionModal();
  };

  const hideCreateCollectionModal = () => {
    showCreateCollection.value = false;
    newCollectionName.value = "";
  };

  const createCollection = () => {
    const trimmedName = newCollectionName.value.trim();
    if (!trimmedName) {
      alert("请输入收藏夹名称");
      return false;
    }

    const shouldReturnToAddModal = pendingSongsForCollection.value.length > 0;
    localCollections.value = [
      ...localCollections.value,
      {
        id: Date.now(),
        name: trimmedName,
        created_at: new Date().toISOString(),
        songs: [],
      },
    ];

    syncLocalCollections();
    hideCreateCollectionModal();

    if (shouldReturnToAddModal) {
      showAddToCollection.value = true;
    }

    return true;
  };

  const openCollectionMenu = (collectionName: string) => {
    if (!collectionName) {
      return;
    }
    selectedCollectionMenuName.value = collectionName;
  };

  const closeCollectionMenu = () => {
    selectedCollectionMenuName.value = null;
  };

  const renameCollection = () => {
    const currentName = selectedCollectionMenuName.value;
    if (!currentName) {
      return false;
    }

    const newName = prompt("请输入新的收藏夹名称", currentName)?.trim();
    if (!newName || newName === currentName) {
      return false;
    }

    localCollections.value = localCollections.value.map((collection) => {
      if (collection.name !== currentName) {
        return collection;
      }

      return {
        ...collection,
        name: newName,
      };
    });

    syncLocalCollections();
    selectedCollectionMenuName.value = newName;
    return newName;
  };

  const deleteCollectionByName = (collectionName: string) => {
    const nextCollections = localCollections.value.filter(
      (collection) => collection.name !== collectionName,
    );

    if (nextCollections.length === localCollections.value.length) {
      return false;
    }

    localCollections.value = nextCollections;
    syncLocalCollections();
    return true;
  };

  const deleteCollectionFromMenu = () => {
    const currentName = selectedCollectionMenuName.value;
    if (!currentName) {
      return false;
    }

    if (!confirm(`确定要删除收藏夹 “${currentName}” 吗？`)) {
      return false;
    }

    const deleted = deleteCollectionByName(currentName);
    closeCollectionMenu();
    return deleted ? currentName : false;
  };

  return {
    localCollections,
    showAddToCollection,
    selectedCollections,
    pendingSongsForCollection,
    newCollectionName,
    showCreateCollection,
    selectedCollectionMenuName,
    collectionNames,
    collectionPlaylists,
    loadLocalCollections,
    showAddToCollectionModal,
    hideAddToCollectionModal,
    toggleCollectionSelection,
    addSongToCollectionFromMenu,
    addToCollection,
    removeSongsFromCollection,
    removeSingleSongFromCollection,
    showCreateCollectionModal,
    createCollectionFromAddModal,
    hideCreateCollectionModal,
    createCollection,
    openCollectionMenu,
    closeCollectionMenu,
    renameCollection,
    deleteCollectionByName,
    deleteCollectionFromMenu,
  };
});
