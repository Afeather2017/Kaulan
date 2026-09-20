import {
  computed,
  getCurrentScope,
  onScopeDispose,
  ref,
  watch,
  type Ref,
} from "vue";
import type { MusicInfo } from "@/composables/useAudioPlayer";
import type { MainView, PlayerPanelMode } from "@/stores/ui";
import { resolveSourceApiBase } from "@/utils/api";

interface UseAppShellLayoutOptions {
  currentSong: Ref<MusicInfo | null>;
  playerPanelMode: Ref<PlayerPanelMode>;
  canGoBack: Ref<boolean>;
  currentView: Ref<MainView>;
}

export function useAppShellLayout(options: UseAppShellLayoutOptions) {
  const { currentSong, playerPanelMode, canGoBack, currentView } = options;

  const isWideLayout = ref(false);
  const hasUserToggledLyric = ref(false);
  const failedCoverUrls = ref<Set<string>>(new Set());
  const failedCoverAttempts = new Map<string, number>();
  let coverRetryTimer: ReturnType<typeof setTimeout> | null = null;

  const getSongCoverUrl = (song: MusicInfo | null): string | null => {
    if (!song || song.id <= 0) {
      return song?.cover_url || null;
    }

    return (
      song.cover_url ||
      `${resolveSourceApiBase(song.source_key)}/music/id/${song.id}/cover`
    );
  };

  const showBackButton = computed(() => {
    if (!canGoBack.value) {
      return false;
    }

    return !isWideLayout.value || currentView.value === "search";
  });

  const isPlayerPanelVisible = computed(
    () => isWideLayout.value || playerPanelMode.value !== "collapsed",
  );

  const isLyricPanelVisible = computed(
    () => playerPanelMode.value === "lyrics",
  );

  const resolveSongCoverUrl = (song: MusicInfo | null): string | null => {
    const coverUrl = getSongCoverUrl(song);
    if (!coverUrl) return null;
    return failedCoverUrls.value.has(coverUrl) ? null : coverUrl;
  };

  watch(
    () => getSongCoverUrl(currentSong.value),
    (coverUrl, previousCoverUrl) => {
      if (coverRetryTimer !== null) {
        clearTimeout(coverRetryTimer);
        coverRetryTimer = null;
      }
      if (!coverUrl || coverUrl === previousCoverUrl) return;

      failedCoverAttempts.delete(coverUrl);
      if (failedCoverUrls.value.has(coverUrl)) {
        const nextFailedUrls = new Set(failedCoverUrls.value);
        nextFailedUrls.delete(coverUrl);
        failedCoverUrls.value = nextFailedUrls;
      }
    },
  );

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
    const coverUrl = getSongCoverUrl(song);
    if (!coverUrl) {
      return;
    }

    const attempt = (failedCoverAttempts.get(coverUrl) ?? 0) + 1;
    failedCoverAttempts.set(coverUrl, attempt);
    failedCoverUrls.value = new Set(failedCoverUrls.value).add(coverUrl);

    // The localhost server and restored playback session start independently
    // on Android. Retry a failed cover twice so an early connection failure
    // cannot suppress otherwise-valid artwork for the whole app session.
    if (attempt <= 2) {
      coverRetryTimer = setTimeout(() => {
        coverRetryTimer = null;
        const nextFailedUrls = new Set(failedCoverUrls.value);
        nextFailedUrls.delete(coverUrl);
        failedCoverUrls.value = nextFailedUrls;
      }, attempt * 1_000);
    }
  };

  const handleCoverLoadSuccess = (song: MusicInfo | null) => {
    const coverUrl = getSongCoverUrl(song);
    if (!coverUrl) return;
    failedCoverAttempts.delete(coverUrl);
  };

  if (getCurrentScope()) {
    onScopeDispose(() => {
      if (coverRetryTimer !== null) {
        clearTimeout(coverRetryTimer);
      }
    });
  }

  return {
    isWideLayout,
    hasUserToggledLyric,
    failedCoverUrls,
    showBackButton,
    isPlayerPanelVisible,
    isLyricPanelVisible,
    resolveSongCoverUrl,
    updateLayoutMode,
    togglePlayerPanelMode,
    showCoverPanel,
    showLyricsPanel,
    handleCoverLoadError,
    handleCoverLoadSuccess,
  };
}
