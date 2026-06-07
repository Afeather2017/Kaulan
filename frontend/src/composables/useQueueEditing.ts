import type { Ref } from "vue";
import type { MusicInfo } from "@/composables/useAudioPlayer";

interface UseQueueEditingOptions {
  activeQueue: Ref<MusicInfo[]>;
  playbackSongs: Ref<MusicInfo[]>;
  currentSong: Ref<MusicInfo | null>;
  selectedSongMenuSong: Ref<MusicInfo | null>;
  replaceQueue: (nextQueue: MusicInfo[]) => Promise<void>;
  handlePlaySong: (song: MusicInfo, index?: number) => Promise<void>;
  closeSongMenu: () => void;
}

export function useQueueEditing(options: UseQueueEditingOptions) {
  const {
    activeQueue,
    playbackSongs,
    currentSong,
    selectedSongMenuSong,
    replaceQueue,
    handlePlaySong,
    closeSongMenu,
  } = options;

  const getSongMenuIdentity = (song: {
    id: number;
    name: string;
    stream_url?: string | null;
    source_key?: string | null;
  }): string => {
    return (
      song.stream_url || `${song.source_key || "local"}:${song.id}:${song.name}`
    );
  };

  const buildQueueWithSongInserted = (song: MusicInfo, insertIndex: number) => {
    const baseQueue =
      activeQueue.value.length > 0
        ? activeQueue.value.slice()
        : playbackSongs.value.slice();
    const songIdentity = getSongMenuIdentity(song);
    const existingQueue = baseQueue.filter(
      (item) => getSongMenuIdentity(item) !== songIdentity,
    );

    if (existingQueue.length === 0) {
      return [song];
    }

    const safeInsertIndex = Math.max(
      0,
      Math.min(insertIndex, existingQueue.length),
    );
    existingQueue.splice(safeInsertIndex, 0, song);
    return existingQueue;
  };

  const queueSongNextFromMenu = async () => {
    const song = selectedSongMenuSong.value;
    if (!song) {
      return;
    }

    if (!currentSong.value) {
      closeSongMenu();
      await handlePlaySong(song);
      return;
    }

    const currentQueue =
      activeQueue.value.length > 0 ? activeQueue.value : playbackSongs.value;
    const currentQueueSong = currentSong.value;
    const currentIndex = currentQueue.findIndex(
      (item) =>
        getSongMenuIdentity(item) ===
        (currentQueueSong ? getSongMenuIdentity(currentQueueSong) : ""),
    );
    const nextQueue = buildQueueWithSongInserted(
      song,
      currentIndex >= 0 ? currentIndex + 1 : 1,
    );
    await replaceQueue(nextQueue);
    closeSongMenu();
  };

  const addSongToQueueFromMenu = async () => {
    const song = selectedSongMenuSong.value;
    if (!song) {
      return;
    }

    const nextQueue = buildQueueWithSongInserted(
      song,
      activeQueue.value.length,
    );
    await replaceQueue(nextQueue);
    closeSongMenu();
  };

  return {
    getSongMenuIdentity,
    buildQueueWithSongInserted,
    queueSongNextFromMenu,
    addSongToQueueFromMenu,
  };
}
