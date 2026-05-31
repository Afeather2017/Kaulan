import { ref } from "vue";

export function useSelection() {
  // Song selection state
  const selectMode = ref(false);
  const selectedSongs = ref<Set<string>>(new Set());

  // Collection selection state
  const collectionSelectMode = ref(false);
  const selectedCollectionsList = ref<Set<string>>(new Set());

  const toggleSelectMode = () => {
    selectMode.value = !selectMode.value;
    selectedSongs.value.clear();
  };

  const toggleSongSelection = (songName: string) => {
    if (selectedSongs.value.has(songName)) {
      selectedSongs.value.delete(songName);
    } else {
      selectedSongs.value.add(songName);
    }
  };

  const clearSongSelection = () => {
    selectedSongs.value.clear();
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
    selectMode.value = false;
    selectedSongs.value.clear();
    collectionSelectMode.value = false;
    selectedCollectionsList.value.clear();
  };

  return {
    // Song selection state
    selectMode,
    selectedSongs,
    // Collection selection state
    collectionSelectMode,
    selectedCollectionsList,
    // Song selection methods
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
