import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { MusicInfo } from "@/composables/useAudioPlayer";
import {
  getLocalCollections,
  setLocalCollections,
  toStoredCollectionSong,
  type StoredCollectionSong,
  type StoredLocalCollection,
} from "@/utils/storage";
import { storedCollectionSongToMusicInfo } from "@/utils/songRestore";
import { useLibraryStore } from "@/stores/library";
import {
  buildSongRowKey,
  inferMediaType,
} from "@/composables/useLibrarySources";

const buildStoredSongRowKey = (song: StoredCollectionSong): string =>
  song.rowKey ||
  `${song.device_id || "local"}:${song.song_id}:${song.name ?? song.filename}`;

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

  // Materialize each stored song into a playable MusicInfo using the current
  // source list. Reactive on libraryStore.sourceGroups so collections
  // rehydrate automatically when a device's apiBase resolves or changes.
  const collectionPlaylists = computed<Record<string, MusicInfo[]>>(() => {
    const libraryStore = useLibraryStore();
    const sourceGroups = libraryStore.sourceGroups;
    return Object.fromEntries(
      localCollections.value.map((collection) => [
        collection.name,
        collection.songs.map((song) => {
          const info = storedCollectionSongToMusicInfo(song, sourceGroups);
          return {
            ...info,
            rowKey: buildStoredSongRowKey(song),
            mediaType: info.mediaType || inferMediaType(info),
          };
        }),
      ]),
    );
  });

  const syncLocalCollections = () => {
    setLocalCollections(localCollections.value);
  };

  const loadLocalCollections = () => {
    localCollections.value = getLocalCollections();
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
        collection.songs.map((song) => buildStoredSongRowKey(song)),
      );
      const nextSongs = collection.songs.slice();

      for (const song of selectedVisibleSongs) {
        const songKey = song.rowKey || buildSongRowKey(song);
        if (existingKeys.has(songKey)) {
          continue;
        }

        existingKeys.add(songKey);
        nextSongs.push(toStoredCollectionSong({ ...song, rowKey: songKey }));
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
          (song) => !selectedSongKeys.has(buildStoredSongRowKey(song)),
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
  ) => {
    const songKey = song.rowKey || buildSongRowKey(song);
    localCollections.value = localCollections.value.map((collection) => {
      if (collection.name !== collectionName) {
        return collection;
      }

      return {
        ...collection,
        songs: collection.songs.filter(
          (item) => buildStoredSongRowKey(item) !== songKey,
        ),
      };
    });

    syncLocalCollections();
  };

  const pruneSongsByKeys = (songKeys: Set<string>) => {
    if (songKeys.size === 0) {
      return;
    }

    localCollections.value = localCollections.value.map((collection) => ({
      ...collection,
      songs: collection.songs.filter(
        (song) => !songKeys.has(buildStoredSongRowKey(song)),
      ),
    }));

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

  // Replace all collections and persist. Used by the import flow in
  // SettingsModal, which computes the next state outside the store via
  // `mergeCollectionsFromImport` (it needs read access to the library store
  // too, so the merge can't live entirely inside this store).
  const replaceLocalCollections = (
    collections: StoredLocalCollection[],
  ): void => {
    localCollections.value = collections;
    syncLocalCollections();
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
    pruneSongsByKeys,
    showCreateCollectionModal,
    createCollectionFromAddModal,
    hideCreateCollectionModal,
    createCollection,
    openCollectionMenu,
    closeCollectionMenu,
    renameCollection,
    deleteCollectionByName,
    deleteCollectionFromMenu,
    replaceLocalCollections,
  };
});
