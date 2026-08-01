import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { useAudioPlayer, type MusicInfo } from "@/composables/useAudioPlayer";
import { useLibraryStore } from "@/stores/library";
import { useTimer } from "@/composables/useTimer";
import { useVolume } from "@/composables/useVolume";
import {
  getLufsPrecacheCount,
  getShowLufs,
  getTimerExitAppOnAndroid,
  normalizeLufsPrecacheCount,
  setLufsPrecacheCount,
  setShowLufs,
} from "@/utils/storage";

type PrepareSongHandler = (song: MusicInfo) => Promise<MusicInfo>;
type QueuePrecacheHandler = (
  queue: MusicInfo[],
  currentIndex: number,
  playMode: "sequential" | "shuffle" | "loop",
) => Promise<void>;
type SongStartHandler =
  | ((currentSongInfo: MusicInfo, nextSongInfo: MusicInfo | null) => void)
  | null;

export const usePlayerStore = defineStore("player", () => {
  const libraryStore = useLibraryStore();
  const playbackSource = ref<"playlist" | "search">("playlist");
  const playlistSongs = ref<MusicInfo[]>([]);
  const searchPlaybackSongs = ref<MusicInfo[]>([]);
  const prepareSongHandler = ref<PrepareSongHandler>(async (song) => song);
  const queuePrecacheHandler = ref<QueuePrecacheHandler | null>(null);
  const songStartHandler = ref<SongStartHandler>(null);
  const showLufs = ref(getShowLufs());
  const lufsPrecacheCount = ref(getLufsPrecacheCount());

  const {
    audioElement,
    activeQueue,
    currentSong,
    isPlaying,
    currentTime,
    duration,
    playMode,
    currentIndex,
    play,
    pause,
    playSong,
    playSongAtIndex,
    togglePlayMode,
    previousSong,
    nextSong,
    seekToTime,
    setTimedPause,
    resetPlaylist,
    formatTime,
    initAudio,
    refreshAndroidSession,
    isAndroidPlayer,
    syncAndroidQueueState,
    syncNormalizationConfig,
  } = useAudioPlayer({
    songs: () => {
      if (playbackSource.value === "search") {
        return searchPlaybackSongs.value;
      }
      return playlistSongs.value;
    },
    onSongEnd: () => {},
    onSongStart: (currentSongInfo, nextSongInfo) => {
      songStartHandler.value?.(currentSongInfo, nextSongInfo);
    },
    onPlaybackQueueStart: async (queue, currentIndex, mode) => {
      await queuePrecacheHandler.value?.(queue, currentIndex, mode);
    },
    prepareSong: async (song) => await prepareSongHandler.value(song),
    sourceGroups: () => libraryStore.sourceGroups,
  });

  const playbackSongs = computed(() => activeQueue.value);

  const {
    volumeMode,
    manualVolume,
    manualVolumeInput,
    fixedLufs,
    fixedLufsInput,
    volumeModeLabels,
    calculateVolume,
  } = useVolume(currentSong, playbackSongs);

  const handleAndroidTimerComplete = async () => {
    if (getTimerExitAppOnAndroid()) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("exit_android_app");
        return;
      } catch (error) {
        console.error("Failed to exit Android app from timer:", error);
      }
    }

    await refreshAndroidSession();
  };

  const {
    timerMinutes,
    timerMinutesInput,
    timerActive,
    timerStatusDisplay,
    startTimer,
    cancelTimer,
  } = useTimer(() => {
    if (isAndroidPlayer.value) {
      void handleAndroidTimerComplete();
    } else if (isPlaying.value) {
      void pause();
    } else if (audioElement.value) {
      audioElement.value.pause();
    }
  });

  const setPrepareSongHandler = (handler: PrepareSongHandler) => {
    prepareSongHandler.value = handler;
  };

  const setQueuePrecacheHandler = (handler: QueuePrecacheHandler | null) => {
    queuePrecacheHandler.value = handler;
  };

  const setSongStartHandler = (handler: SongStartHandler) => {
    songStartHandler.value = handler;
  };

  const setPlaylistSongs = (songs: MusicInfo[]) => {
    playlistSongs.value = songs.slice();
  };

  const setSearchPlaybackQueue = (songs: MusicInfo[]) => {
    searchPlaybackSongs.value = songs.slice();
  };

  const clearSearchPlaybackQueue = () => {
    searchPlaybackSongs.value = [];
  };

  const resetPlaybackSourceContext = () => {
    playbackSource.value = "playlist";
    clearSearchPlaybackQueue();
    resetPlaylist();
  };

  const playVisibleSong = async (
    song: MusicInfo,
    visibleQueue: MusicInfo[],
    index?: number,
  ) => {
    if (index !== undefined) {
      await playSongAtIndex(song, index, visibleQueue);
      return;
    }

    await playSong(song, undefined, visibleQueue);
  };

  const playSongFromPlaylist = async (
    song: MusicInfo,
    visibleQueue: MusicInfo[],
    index?: number,
  ) => {
    playbackSource.value = "playlist";
    clearSearchPlaybackQueue();
    await playVisibleSong(song, visibleQueue, index);
  };

  const playSongFromSearch = async (
    song: MusicInfo,
    visibleQueue: MusicInfo[],
    index?: number,
  ) => {
    playbackSource.value = "search";
    setSearchPlaybackQueue(visibleQueue);
    await playVisibleSong(song, visibleQueue, index);
  };

  const playQueueSong = async (song: MusicInfo, index: number) => {
    await playSongAtIndex(song, index, activeQueue.value);
  };

  const playPreviewTrack = async (song: MusicInfo) => {
    playbackSource.value = "search";
    setSearchPlaybackQueue([song]);
    await playSongAtIndex(song, 0, [song]);
  };

  const replaceQueue = async (nextQueue: MusicInfo[]) => {
    activeQueue.value = nextQueue;
    if (isAndroidPlayer.value) {
      await syncAndroidQueueState();
    }
  };

  const setShowLufsState = (value: boolean) => {
    showLufs.value = value;
    setShowLufs(value);
  };

  const setLufsPrecacheCountState = (value: number) => {
    const normalized = normalizeLufsPrecacheCount(value);
    lufsPrecacheCount.value = normalized;
    setLufsPrecacheCount(normalized);
  };

  const startSleepTimer = async () => {
    if (isAndroidPlayer.value) {
      await setTimedPause(timerMinutes.value * 60 * 1000);
    }
    startTimer();
  };

  const setTimerPreset = async (minutes: number) => {
    timerMinutes.value = minutes;
    timerMinutesInput.value = minutes;
    await startSleepTimer();
  };

  const cancelSleepTimer = async () => {
    cancelTimer();
    if (isAndroidPlayer.value) {
      await setTimedPause(0);
    }
  };

  const syncNormalization = async () => {
    await syncNormalizationConfig(
      volumeMode.value,
      manualVolume.value,
      fixedLufs.value,
      lufsPrecacheCount.value,
      calculateVolume(),
    );
  };

  return {
    audioElement,
    activeQueue,
    currentSong,
    isPlaying,
    currentTime,
    duration,
    playMode,
    currentIndex,
    isAndroidPlayer,
    playbackSource,
    playlistSongs,
    searchPlaybackSongs,
    playbackSongs,
    volumeMode,
    manualVolume,
    manualVolumeInput,
    fixedLufs,
    fixedLufsInput,
    volumeModeLabels,
    calculateVolume,
    showLufs,
    lufsPrecacheCount,
    timerMinutes,
    timerMinutesInput,
    timerActive,
    timerStatusDisplay,
    play,
    pause,
    playSong,
    playSongAtIndex,
    togglePlayMode,
    previousSong,
    nextSong,
    seekToTime,
    setTimedPause,
    resetPlaylist,
    formatTime,
    initAudio,
    refreshAndroidSession,
    syncAndroidQueueState,
    syncNormalizationConfig,
    setPrepareSongHandler,
    setQueuePrecacheHandler,
    setSongStartHandler,
    setPlaylistSongs,
    setSearchPlaybackQueue,
    clearSearchPlaybackQueue,
    resetPlaybackSourceContext,
    playSongFromPlaylist,
    playSongFromSearch,
    playQueueSong,
    playPreviewTrack,
    replaceQueue,
    setShowLufsState,
    setLufsPrecacheCountState,
    handleAndroidTimerComplete,
    startSleepTimer,
    setTimerPreset,
    cancelSleepTimer,
    syncNormalization,
  };
});
