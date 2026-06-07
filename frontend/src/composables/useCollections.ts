import { computed, ref, type ComputedRef, type Ref } from "vue";
import type { MusicInfo } from "@/composables/useAudioPlayer";
import {
  getLocalCollections,
  setLocalCollections,
  type StoredLocalCollection,
} from "@/utils/storage";

interface PlaylistSelection {
  name: string;
  songs: MusicInfo[];
}

interface UseCollectionsOptions {
  currentSongs: ComputedRef<MusicInfo[]>;
  selectedSongs: Ref<Set<string>>;
  selectedPlaylist: Ref<PlaylistSelection | null>;
  buildSongRowKey: (song: MusicInfo) => string;
  inferMediaType: (song: MusicInfo) => "audio" | "video";
  onSelectCollection: (name: string) => void;
  onBackToPlaylists: () => void;
  clearSongSelection: () => void;
}

export function useCollections(options: UseCollectionsOptions) {
  const {
    currentSongs,
    selectedSongs,
    selectedPlaylist,
    buildSongRowKey,
    inferMediaType,
    onSelectCollection,
    onBackToPlaylists,
    clearSongSelection,
  } = options;

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

  const loadLocalCollections = () => {
    localCollections.value = getLocalCollections().map((collection) => ({
      ...collection,
      songs: collection.songs.map((song) => ({
        ...song,
        rowKey: buildSongRowKey(song),
        mediaType: song.mediaType || inferMediaType(song),
      })),
    }));
  };

  const handleShowAddToCollectionModal = () => {
    selectedCollections.value = [];
    pendingSongsForCollection.value = [];
    showAddToCollection.value = true;
  };

  const hideAddToCollectionModal = () => {
    showAddToCollection.value = false;
    selectedCollections.value = [];
    pendingSongsForCollection.value = [];
  };

  const handleToggleCollectionSelection = (id: number) => {
    const index = selectedCollections.value.indexOf(id);
    if (index > -1) {
      selectedCollections.value.splice(index, 1);
      return;
    }
    selectedCollections.value.push(id);
  };

  const addSongToCollectionFromMenu = (song: MusicInfo) => {
    selectedCollections.value = [];
    pendingSongsForCollection.value = [song];
    showAddToCollection.value = true;
  };

  const addToCollection = async () => {
    if (selectedCollections.value.length === 0) {
      alert("请选择至少一个收藏夹");
      return;
    }

    const selectedVisibleSongs =
      pendingSongsForCollection.value.length > 0
        ? pendingSongsForCollection.value
        : currentSongs.value.filter((song) =>
            selectedSongs.value.has(song.rowKey || buildSongRowKey(song)),
          );

    if (selectedVisibleSongs.length === 0) {
      alert("没有选中的歌曲");
      return;
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
  };

  const handleRemoveFromCollection = async () => {
    if (!selectedPlaylist.value) {
      return;
    }

    const selectedNames = new Set(selectedSongs.value);
    if (selectedNames.size === 0) {
      alert("没有选中的歌曲");
      return;
    }

    localCollections.value = localCollections.value.map((collection) => {
      if (collection.name !== selectedPlaylist.value?.name) {
        return collection;
      }

      return {
        ...collection,
        songs: collection.songs.filter(
          (song) => !selectedNames.has(song.rowKey || buildSongRowKey(song)),
        ),
      };
    });

    syncLocalCollections();
    onSelectCollection(selectedPlaylist.value.name);
    alert("移除成功");
    clearSongSelection();
  };

  const removeSingleSongFromCollection = (song: MusicInfo) => {
    if (!selectedPlaylist.value) {
      return;
    }

    const songKey = song.rowKey || buildSongRowKey(song);
    localCollections.value = localCollections.value.map((collection) => {
      if (collection.name !== selectedPlaylist.value?.name) {
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
    onSelectCollection(selectedPlaylist.value.name);
  };

  const handleShowCreateModal = () => {
    newCollectionName.value = "";
    showCreateCollection.value = true;
  };

  const handleCreateCollectionFromAddModal = () => {
    showAddToCollection.value = false;
    handleShowCreateModal();
  };

  const hideCreateCollectionModal = () => {
    showCreateCollection.value = false;
    newCollectionName.value = "";
  };

  const handleCreateCollection = async () => {
    const trimmedName = newCollectionName.value.trim();
    if (!trimmedName) {
      alert("请输入收藏夹名称");
      return;
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
      return;
    }

    const nextName = prompt("请输入新的收藏夹名称:", currentName);
    if (nextName === null) {
      return;
    }

    const trimmedName = nextName.trim();
    if (!trimmedName) {
      alert("请输入收藏夹名称");
      return;
    }

    if (
      trimmedName !== currentName &&
      localCollections.value.some(
        (collection) => collection.name === trimmedName,
      )
    ) {
      alert("已存在同名收藏夹");
      return;
    }

    localCollections.value = localCollections.value.map((collection) =>
      collection.name === currentName
        ? {
            ...collection,
            name: trimmedName,
          }
        : collection,
    );

    syncLocalCollections();

    if (selectedPlaylist.value?.name === currentName) {
      selectedPlaylist.value = {
        ...selectedPlaylist.value,
        name: trimmedName,
      };
    }

    selectedCollectionMenuName.value = trimmedName;
  };

  const deleteCollectionByName = (collectionName: string) => {
    const before = localCollections.value.length;
    localCollections.value = localCollections.value.filter(
      (collection) => collection.name !== collectionName,
    );

    if (localCollections.value.length === before) {
      return false;
    }

    syncLocalCollections();
    if (selectedPlaylist.value?.name === collectionName) {
      onBackToPlaylists();
    }
    return true;
  };

  const deleteCollectionFromMenu = () => {
    const collectionName = selectedCollectionMenuName.value;
    if (!collectionName) {
      return;
    }

    if (!confirm(`确定要删除收藏夹 “${collectionName}” 吗？`)) {
      return;
    }

    if (deleteCollectionByName(collectionName)) {
      closeCollectionMenu();
    }
  };

  return {
    localCollections,
    collectionNames,
    collectionPlaylists,
    showAddToCollection,
    selectedCollections,
    pendingSongsForCollection,
    newCollectionName,
    showCreateCollection,
    selectedCollectionMenuName,
    loadLocalCollections,
    handleShowAddToCollectionModal,
    hideAddToCollectionModal,
    handleToggleCollectionSelection,
    addSongToCollectionFromMenu,
    addToCollection,
    handleRemoveFromCollection,
    removeSingleSongFromCollection,
    handleShowCreateModal,
    handleCreateCollectionFromAddModal,
    hideCreateCollectionModal,
    handleCreateCollection,
    openCollectionMenu,
    closeCollectionMenu,
    renameCollection,
    deleteCollectionByName,
    deleteCollectionFromMenu,
  };
}
