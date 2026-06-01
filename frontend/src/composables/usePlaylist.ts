import { ref, computed } from "vue";
import { getApiBase } from "@/utils/api";
import { checkIsAndroid } from "@/utils/platform";

export interface MusicInfo {
  id: number;
  name: string;
  lufs: number | null;
  path: string;
  stream_url?: string | null;
}

export interface Playlist {
  name: string;
  songs: MusicInfo[];
}

function isLoopbackHostname(hostname: string): boolean {
  return (
    hostname === "localhost" || hostname === "127.0.0.1" || hostname === "::1"
  );
}

/** Query string to request content:// stream URLs from backend */
async function streamParam(): Promise<string> {
  if (!(await checkIsAndroid())) {
    return "";
  }

  try {
    const apiBase = new URL(getApiBase());
    return isLoopbackHostname(apiBase.hostname) ? "?stream=content" : "";
  } catch {
    return "";
  }
}

export function usePlaylist() {
  const playlists = ref<Record<string, MusicInfo[]>>({});
  const searchQuery = ref("");
  const currentView = ref<"playlists" | "songs" | "search">("playlists");
  const selectedPlaylist = ref<Playlist | null>(null);

  // Computed
  const playlistNames = computed(() => {
    return Object.keys(playlists.value);
  });

  const currentSongs = computed(() => {
    return selectedPlaylist.value?.songs || [];
  });

  // Search behavior docs: docs/search.md
  const searchResults = computed(() => {
    if (!searchQuery.value) return [];
    const query = searchQuery.value.toLowerCase();
    let scopeSongs: MusicInfo[] = [];
    if (selectedPlaylist.value) {
      scopeSongs = selectedPlaylist.value.songs;
    } else {
      const allMusic = playlists.value["所有音乐"];
      scopeSongs = allMusic || Object.values(playlists.value).flat();
    }
    return scopeSongs.filter((song) => song.name.toLowerCase().includes(query));
  });

  // Fetch playlists from backend (folder mode)
  const fetchPlaylists = async () => {
    try {
      const response = await fetch(
        `${getApiBase()}/playlists${await streamParam()}`,
      );
      if (response.ok) {
        playlists.value = await response.json();
      }
    } catch (error) {
      console.error("Failed to fetch playlists:", error);
    }
  };

  const refreshData = async () => {
    await fetchPlaylists();
  };

  const selectPlaylist = (playlistName: string) => {
    selectedPlaylist.value = {
      name: playlistName,
      songs: playlists.value[playlistName] || [],
    };
    currentView.value = "songs";
  };

  const backToPlaylists = () => {
    currentView.value = "playlists";
    selectedPlaylist.value = null;
    searchQuery.value = "";
  };

  const showSearchResults = () => {
    if (!searchQuery.value) return;
    currentView.value = "search";
  };

  const getAllMusic = async (): Promise<any[]> => {
    const response = await fetch(`${getApiBase()}/music`);
    if (response.ok) {
      return await response.json();
    }
    return [];
  };

  return {
    // State
    playlists,
    searchQuery,
    currentView,
    selectedPlaylist,
    // Computed
    playlistNames,
    currentSongs,
    searchResults,
    // Methods
    fetchPlaylists,
    refreshData,
    selectPlaylist,
    backToPlaylists,
    showSearchResults,
    getAllMusic,
  };
}
