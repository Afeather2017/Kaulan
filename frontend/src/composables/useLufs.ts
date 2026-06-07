import type { Ref } from "vue";
import type { MusicInfo } from "@/composables/useAudioPlayer";
import { resolveSourceApiBase } from "@/utils/api";
import type { LibrarySourceGroup } from "@/types/library";

interface PlaylistSelection {
  name: string;
  songs: MusicInfo[];
}

interface PrecacheLufsResult {
  success: boolean;
  lufs: number | null;
  cached?: boolean;
  error?: string;
}

interface UseLufsOptions {
  currentSong: Ref<MusicInfo | null>;
  activeQueue: Ref<MusicInfo[]>;
  searchPlaybackSongs: Ref<MusicInfo[]>;
  selectedPlaylist: Ref<PlaylistSelection | null>;
  sourceGroups: Ref<LibrarySourceGroup[]>;
  isAndroidPlayer: Ref<boolean>;
  syncAndroidQueueState: () => Promise<void>;
  syncSelectedLibraryPlaylist: (
    view: string,
    selectedPlaylist: Ref<PlaylistSelection | null>,
  ) => void;
  currentView: Ref<string>;
}

const LUFS_POLL_DELAY_MS = 1000;
const LUFS_POLL_MAX_ATTEMPTS = 8;

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export function useLufs(options: UseLufsOptions) {
  const {
    currentSong,
    activeQueue,
    searchPlaybackSongs,
    selectedPlaylist,
    sourceGroups,
    isAndroidPlayer,
    syncAndroidQueueState,
    syncSelectedLibraryPlaylist,
    currentView,
  } = options;

  const pendingLufsPolls = new Set<number>();

  const patchSongLufsInList = (
    songs: MusicInfo[],
    songId: number,
    lufs: number,
  ): MusicInfo[] => {
    let changed = false;
    const updatedSongs = songs.map((song) => {
      if (song.id !== songId || song.lufs === lufs) {
        return song;
      }

      changed = true;
      return {
        ...song,
        lufs,
      };
    });

    return changed ? updatedSongs : songs;
  };

  const patchSongLufs = (songId: number, lufs: number) => {
    if (currentSong.value?.id === songId && currentSong.value.lufs !== lufs) {
      currentSong.value = {
        ...currentSong.value,
        lufs,
      };
    }

    activeQueue.value = patchSongLufsInList(activeQueue.value, songId, lufs);
    searchPlaybackSongs.value = patchSongLufsInList(
      searchPlaybackSongs.value,
      songId,
      lufs,
    );

    if (selectedPlaylist.value) {
      selectedPlaylist.value = {
        ...selectedPlaylist.value,
        songs: patchSongLufsInList(selectedPlaylist.value.songs, songId, lufs),
      };
    }

    let groupsChanged = false;
    const nextGroups = sourceGroups.value.map((group) => {
      let groupChanged = false;
      const playlists = group.playlists.map((playlist) => {
        const updatedSongs = patchSongLufsInList(playlist.songs, songId, lufs);
        if (updatedSongs === playlist.songs) {
          return playlist;
        }

        groupChanged = true;
        return {
          ...playlist,
          songs: updatedSongs,
        };
      });

      if (!groupChanged) {
        return group;
      }

      groupsChanged = true;
      return {
        ...group,
        playlists,
      };
    });

    if (groupsChanged) {
      sourceGroups.value = nextGroups;
      syncSelectedLibraryPlaylist(currentView.value, selectedPlaylist);
    }
  };

  const pollSongLufs = async (
    songId: number,
    sourceKey: string | null | undefined,
    context: "current" | "next",
  ) => {
    if (pendingLufsPolls.has(songId)) {
      console.log(
        `[app] LUFS ${context} poll already in flight for song ID:`,
        songId,
      );
      return;
    }

    pendingLufsPolls.add(songId);

    try {
      for (let attempt = 1; attempt <= LUFS_POLL_MAX_ATTEMPTS; attempt++) {
        await wait(LUFS_POLL_DELAY_MS);

        try {
          console.log(
            `[app] LUFS ${context} poll request attempt ${attempt} for song ID:`,
            songId,
          );
          const response = await fetch(
            `${resolveSourceApiBase(sourceKey)}/music/${songId}/precache-lufs`,
            {
              method: "POST",
            },
          );

          if (!response.ok) {
            console.warn(`[app] LUFS ${context} poll failed:`, response.status);
            return;
          }

          const result: PrecacheLufsResult = await response.json();
          if (result.success && result.lufs !== null) {
            console.log(
              `[app] LUFS ${context} resolved from poll attempt ${attempt}:`,
              result.lufs,
            );
            patchSongLufs(songId, result.lufs);
            if (isAndroidPlayer.value) {
              await syncAndroidQueueState();
            }
            return;
          }

          if (result.cached !== false) {
            return;
          }
        } catch (error) {
          console.error(
            `[app] LUFS ${context} poll error on attempt ${attempt}:`,
            error,
          );
          return;
        }
      }
    } finally {
      pendingLufsPolls.delete(songId);
    }
  };

  const requestSongLufs = async (
    song: MusicInfo,
    context: "current" | "next",
  ): Promise<MusicInfo> => {
    if (song.lufs !== null) {
      console.log(
        `[app] LUFS ${context} already cached for song ID:`,
        song.id,
        "value:",
        song.lufs,
      );
      return song;
    }

    try {
      console.log(`[app] LUFS ${context} request for song ID:`, song.id);
      const response = await fetch(
        `${resolveSourceApiBase(song.source_key)}/music/${song.id}/precache-lufs`,
        {
          method: "POST",
        },
      );

      if (!response.ok) {
        console.warn(
          `[app] LUFS ${context} pre-cache failed:`,
          response.status,
        );
        return song;
      }

      const result: PrecacheLufsResult = await response.json();
      if (result.success && result.lufs !== null) {
        console.log(`[app] LUFS ${context} resolved immediately:`, result.lufs);
        patchSongLufs(song.id, result.lufs);
        if (context === "next" && isAndroidPlayer.value) {
          await syncAndroidQueueState();
        }
        return {
          ...song,
          lufs: result.lufs,
        };
      }

      if (result.success && result.cached === false) {
        console.log(
          `[app] LUFS ${context} started in background (non-blocking)`,
        );
        void pollSongLufs(song.id, song.source_key, context);
      }
    } catch (error) {
      console.error(`[app] LUFS ${context} pre-cache error:`, error);
    }

    return song;
  };

  const resolveSongForPlayback = async (
    song: MusicInfo,
  ): Promise<MusicInfo> => {
    if (isAndroidPlayer.value) {
      return song;
    }

    return await requestSongLufs(song, "current");
  };

  const syncPlaybackMetadataFromBackend = () => {
    if (!isAndroidPlayer.value || activeQueue.value.length === 0) {
      return;
    }

    for (const song of activeQueue.value) {
      if (song.lufs !== null) {
        patchSongLufs(song.id, song.lufs);
      }
    }
  };

  return {
    requestSongLufs,
    resolveSongForPlayback,
    syncPlaybackMetadataFromBackend,
  };
}
