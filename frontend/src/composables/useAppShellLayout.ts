import { computed, ref, type Ref } from "vue";
import type { MusicInfo } from "@/composables/useAudioPlayer";
import type { MainView, PlayerPanelMode } from "@/stores/ui";
import { resolveSourceApiBase } from "@/utils/api";

interface UseAppShellLayoutOptions {
  currentSong: Ref<MusicInfo | null>;
  currentView: Ref<MainView>;
  playerPanelMode: Ref<PlayerPanelMode>;
}

export function useAppShellLayout(options: UseAppShellLayoutOptions) {
  const { currentSong, currentView, playerPanelMode } = options;

  const isWideLayout = ref(false);
  const hasUserToggledLyric = ref(false);
  const failedCoverUrls = ref<Set<string>>(new Set());

  const showBackButton = computed(() => {
    return (
      currentView.value === "search" ||
      (playerPanelMode.value !== "collapsed" && !isWideLayout.value)
    );
  });

  const showActionBar = computed(() => showBackButton.value);

  const isPlayerPanelVisible = computed(
    () => isWideLayout.value || playerPanelMode.value !== "collapsed",
  );

  const isLyricPanelVisible = computed(
    () => playerPanelMode.value === "lyrics",
  );

  const resolveSongCoverUrl = (song: MusicInfo | null): string | null => {
    if (!song) {
      return null;
    }

    const coverUrl =
      song.cover_url ||
      `${resolveSourceApiBase(song.source_key)}/music/id/${song.id}/cover`;
    return failedCoverUrls.value.has(coverUrl) ? null : coverUrl;
  };

  const updateLayoutMode = () => {
    if (typeof window === "undefined") return;
    const isWide = window.matchMedia("(min-aspect-ratio: 1/1)").matches;
    const wasWide = isWideLayout.value;
    isWideLayout.value = isWide;
    if (isWide && (!wasWide || playerPanelMode.value === "collapsed")) {
      playerPanelMode.value = "cover";
      return;
    }
    if (!isWide && wasWide && !hasUserToggledLyric.value) {
      playerPanelMode.value = "collapsed";
    }
  };

  const togglePlayerPanelMode = () => {
    if (!currentSong.value) {
      return;
    }

    if (playerPanelMode.value === "collapsed") {
      playerPanelMode.value = "cover";
    } else if (playerPanelMode.value === "cover") {
      playerPanelMode.value = "lyrics";
    } else {
      playerPanelMode.value = "cover";
    }

    hasUserToggledLyric.value = true;
  };

  const showCoverPanel = () => {
    if (!currentSong.value) {
      return;
    }
    playerPanelMode.value = "cover";
    hasUserToggledLyric.value = true;
  };

  const showLyricsPanel = () => {
    if (!currentSong.value) {
      return;
    }
    playerPanelMode.value = "lyrics";
    hasUserToggledLyric.value = true;
  };

  const handleCoverLoadError = (song: MusicInfo | null) => {
    const coverUrl = resolveSongCoverUrl(song);
    if (!coverUrl) {
      return;
    }

    failedCoverUrls.value = new Set(failedCoverUrls.value).add(coverUrl);
  };

  return {
    isWideLayout,
    hasUserToggledLyric,
    failedCoverUrls,
    showBackButton,
    showActionBar,
    isPlayerPanelVisible,
    isLyricPanelVisible,
    resolveSongCoverUrl,
    updateLayoutMode,
    togglePlayerPanelMode,
    showCoverPanel,
    showLyricsPanel,
    handleCoverLoadError,
  };
}
