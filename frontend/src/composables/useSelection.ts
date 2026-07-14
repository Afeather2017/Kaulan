import { ref } from "vue";

export type SongSelectionAction =
  | "collection"
  | "remove"
  | "delete"
  | "download";

export function useSelection() {
  // Song selection state
  const selectMode = ref(false);
  const selectedSongs = ref<Set<string>>(new Set());
  const songSelectionAction = ref<SongSelectionAction | null>(null);

  // Collection selection state
  const collectionSelectMode = ref(false);
  const selectedCollectionsList = ref<Set<string>>(new Set());

  const startSongSelection = (action: SongSelectionAction) => {
    selectMode.value = true;
    songSelectionAction.value = action;
    selectedSongs.value.clear();
  };

  const stopSongSelection = () => {
    selectMode.value = false;
    songSelectionAction.value = null;
    selectedSongs.value.clear();
  };

  const toggleSelectMode = () => {
    if (selectMode.value) {
      stopSongSelection();
      return;
    }

    startSongSelection("collection");
  };

  const toggleSongSelection = (songName: string) => {
    if (selectedSongs.value.has(songName)) {
      selectedSongs.value.delete(songName);
    } else {
      selectedSongs.value.add(songName);
    }
  };

  const clearSongSelection = () => {
    stopSongSelection();
  };

  const toggleCollectionSelectMode = () => {
    collectionSelectMode.value = !collectionSelectMode.value;
    selectedCollectionsList.value.clear();
  };

  const toggleCollectionSelection = (collectionName: string) => {
    if (selectedCollectionsList.value.has(collectionName)) {
      selectedCollectionsList.value.delete(collectionName);
    } else {
      selectedCollectionsList.value.add(collectionName);
    }
  };

  const clearCollectionSelection = () => {
    selectedCollectionsList.value.clear();
  };

  const hasSelectedNonAllMusicCollection = () => {
    return Array.from(selectedCollectionsList.value).some(
      (name) => name !== "所有音乐",
    );
  };

  const resetAll = () => {
    stopSongSelection();
    collectionSelectMode.value = false;
    selectedCollectionsList.value.clear();
  };

  return {
    // Song selection state
    selectMode,
    selectedSongs,
    songSelectionAction,
    // Collection selection state
    collectionSelectMode,
    selectedCollectionsList,
    // Song selection methods
    startSongSelection,
    stopSongSelection,
    toggleSelectMode,
    toggleSongSelection,
    clearSongSelection,
    // Collection selection methods
    toggleCollectionSelectMode,
    toggleCollectionSelection,
    clearCollectionSelection,
    hasSelectedNonAllMusicCollection,
    // Reset all
    resetAll,
  };
}
