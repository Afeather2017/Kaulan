import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import type { Ref } from "vue";

import type { MusicInfo } from "@/types/music";
import type { LibrarySourceGroup } from "@/types/library";
import * as librarySourcesModule from "@/composables/useLibrarySources";
import * as audioPlayerModule from "@/composables/useAudioPlayer";
import { usePlayerStore } from "@/stores/player";
import { useLibraryStore } from "@/stores/library";

// Related documentation:
// - `docs/android/playback-session.md` (tap racing the initial library load)

const LOCAL = "http://localhost:2080/api";
const LOCAL_DEVICE = "local-device";

type LibraryMockState = {
  resolvers: Array<() => void>;
  setGroups: (groups: LibrarySourceGroup[]) => void;
};

type PlayerMockState = {
  playSongAtIndex: ReturnType<typeof vi.fn>;
  playSong: ReturnType<typeof vi.fn>;
};

// vi.mock factories are hoisted above every declaration in vitest 0.30 (no
// vi.hoisted yet), so shared state must live inside the factory closure and
// be exposed as a test-only module export.
vi.mock("@/composables/useLibrarySources", async () => {
  const { ref } = await import("vue");
  const groupsRef = ref<LibrarySourceGroup[]>([]) as Ref<LibrarySourceGroup[]>;
  const state: LibraryMockState = {
    resolvers: [],
    setGroups: (groups: LibrarySourceGroup[]) => {
      groupsRef.value = groups;
    },
  };
  return {
    __mockState: state,
    useLibrarySources: () => ({
      sourceGroups: groupsRef,
      refreshSourceGroups: () =>
        new Promise<void>((resolve) => {
          state.resolvers.push(resolve);
        }),
    }),
  };
});

vi.mock("@/composables/useAudioPlayer", async () => {
  const { ref } = await import("vue");
  const state: PlayerMockState = {
    playSongAtIndex: vi.fn(async () => {}),
    playSong: vi.fn(async () => {}),
  };
  return {
    __mockState: state,
    useAudioPlayer: () => ({
      audioElement: ref(null),
      activeQueue: ref([]),
      currentSong: ref(null),
      isPlaying: ref(false),
      currentTime: ref(0),
      duration: ref(0),
      playMode: ref("sequential"),
      currentIndex: ref(-1),
      play: vi.fn(async () => {}),
      pause: vi.fn(async () => {}),
      playSong: state.playSong,
      playSongAtIndex: state.playSongAtIndex,
      togglePlayMode: vi.fn(),
      previousSong: vi.fn(async () => {}),
      nextSong: vi.fn(async () => {}),
      seekToTime: vi.fn(async () => {}),
      setTimedPause: vi.fn(async () => {}),
      resetPlaylist: vi.fn(),
      formatTime: vi.fn(() => "0:00"),
      initAudio: vi.fn(async () => {}),
      refreshAndroidSession: vi.fn(async () => {}),
      isAndroidPlayer: ref(false),
      syncAndroidQueueState: vi.fn(async () => {}),
      syncNormalizationConfig: vi.fn(async () => {}),
    }),
  };
});

const libraryMock = (
  librarySourcesModule as unknown as {
    __mockState: LibraryMockState;
  }
).__mockState;
const playerMock = (
  audioPlayerModule as unknown as {
    __mockState: PlayerMockState;
  }
).__mockState;

const liveSong: MusicInfo = {
  id: 42,
  name: "song.mp3",
  lufs: -12,
  path: "content://media/external/audio/media/42",
  stream_url: `${LOCAL}/music/id/42`,
  cover_url: `${LOCAL}/music/id/42/cover`,
  source_key: LOCAL,
  device_id: LOCAL_DEVICE,
  is_temporary: false,
};

const liveGroup: LibrarySourceGroup = {
  apiBase: LOCAL,
  sourceKey: LOCAL,
  device_id: LOCAL_DEVICE,
  name: "local",
  isLoading: false,
  isOnline: true,
  playlists: [{ name: "All", songs: [liveSong] }],
  onlineProviderStatuses: [],
  capabilities: {
    canRefresh: true,
    canUpload: true,
    canChangeDirectory: true,
    canUseForOnlineSearch: false,
    isCurrentOnlineSearchSource: false,
    canRetryConnection: false,
    canShowSourceDetails: true,
    canDeleteSource: false,
  },
};

// What a collection tap looks like before the library settles: basename path,
// rebuilt HTTP stream_url (buildRestoredSong output shape).
const preLoadSong: MusicInfo = {
  id: 42,
  name: "song.mp3",
  lufs: null,
  path: "song.mp3",
  stream_url: `${LOCAL}/music/id/42`,
  cover_url: `${LOCAL}/music/id/42/cover`,
  source_key: LOCAL,
  device_id: LOCAL_DEVICE,
  is_temporary: false,
};

const isPending = (promise: Promise<unknown>): Promise<boolean> =>
  Promise.race([
    promise.then(
      () => false,
      () => false,
    ),
    new Promise<boolean>((resolve) => setTimeout(() => resolve(true), 10)),
  ]);

describe("player store gate-and-adopt on playback entry points", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    playerMock.playSongAtIndex.mockClear();
    playerMock.playSong.mockClear();
    libraryMock.resolvers = [];
    libraryMock.setGroups([]);
  });

  it("waits for the initial library load, then adopts the tapped song and queue", async () => {
    // Arm the gate the way useAppShell's onMounted does: first refresh in
    // flight, not yet settled.
    const libraryStore = useLibraryStore();
    const load = libraryStore.refreshSourceGroups();

    const playerStore = usePlayerStore();
    const played = playerStore.playSongFromPlaylist(
      preLoadSong,
      [preLoadSong],
      0,
    );

    // Gate is armed (library store still loading): playback must not start
    // from the pre-load fallback shape.
    expect(playerMock.playSongAtIndex).not.toHaveBeenCalled();
    expect(await isPending(played)).toBe(true);

    // Real ordering: groups update lands before the load promise resolves.
    libraryMock.setGroups([liveGroup]);
    libraryMock.resolvers[0]();
    await Promise.all([played, load]);

    expect(playerMock.playSongAtIndex).toHaveBeenCalledTimes(1);
    const [song, index, queue] = playerMock.playSongAtIndex.mock.calls[0];
    expect(song.path).toBe("content://media/external/audio/media/42");
    expect(queue[0].path).toBe("content://media/external/audio/media/42");
    expect(index).toBe(0);
  });

  it("adopts without waiting once the initial load has settled", async () => {
    const libraryStore = useLibraryStore();
    const load = libraryStore.refreshSourceGroups();
    libraryMock.setGroups([liveGroup]);
    libraryMock.resolvers[0]();
    await load;

    const playerStore = usePlayerStore();
    await playerStore.playSongFromPlaylist(preLoadSong, [preLoadSong], 0);

    expect(playerMock.playSongAtIndex).toHaveBeenCalledTimes(1);
    const [song] = playerMock.playSongAtIndex.mock.calls[0];
    expect(song.path).toBe("content://media/external/audio/media/42");
  });

  it("passes online songs through the gate untouched", async () => {
    const libraryStore = useLibraryStore();
    const load = libraryStore.refreshSourceGroups();
    libraryMock.setGroups([liveGroup]);
    libraryMock.resolvers[0]();
    await load;

    const onlineSong: MusicInfo = {
      ...preLoadSong,
      path: "yt",
      source: "youtube",
      is_temporary: true,
    };
    const playerStore = usePlayerStore();
    await playerStore.playSongFromPlaylist(onlineSong, [onlineSong], 0);

    expect(playerMock.playSongAtIndex).toHaveBeenCalledTimes(1);
    const [song, , queue] = playerMock.playSongAtIndex.mock.calls[0];
    expect(song.path).toBe("yt");
    expect(queue[0].path).toBe("yt");
  });
});
