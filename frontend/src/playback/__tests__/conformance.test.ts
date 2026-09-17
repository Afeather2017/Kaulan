/**
 * Interface conformance suite: the SAME scenarios run against the web backend
 * (stateful HTMLAudioElement fake) and the Android backend (stateful native
 * service fake). Both must produce the same observable engine state — this is
 * the machine-checked "the two backends behave identically" guarantee.
 *
 * Scenarios encode the product contract whose absence caused the pre-refactor
 * bugs: seek-while-paused resumes, seek-before-metadata still lands, song end
 * advances per play mode, errors skip, rapid skips settle on the last target.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { createPlayback } from "@/playback";
import type { MusicInfo } from "@/types/music";
import {
  removeStoredPlaybackSession,
  setStoredPlaybackSession,
} from "@/utils/storage";

// vitest 0.30 has no vi.hoisted; mock factories read shared state off
// globalThis instead (factories run before the module body).
interface TestGlobals {
  platform: { usesAndroidPlaybackBackend: boolean };
  media: FakeMediaElement | null;
  native: FakeNativeService | null;
}
const testGlobals: TestGlobals = ((
  globalThis as unknown as {
    __playbackConformance?: TestGlobals;
  }
).__playbackConformance ??= {
  platform: { usesAndroidPlaybackBackend: false },
  media: null,
  native: null,
});

vi.mock("@/utils/platform", () => ({
  getRuntimeCapabilities: () =>
    Promise.resolve({
      usesAndroidPlaybackBackend: (
        globalThis as unknown as { __playbackConformance: TestGlobals }
      ).__playbackConformance.platform.usesAndroidPlaybackBackend,
      supportsAndroidBackHandler: false,
      supportsForegroundMusicService: false,
      supportsExitAppOnTimer: false,
      supportsLocalLyricsPermission: false,
      supportsHeadsetMediaButtonControl: false,
      supportsRawContentPlayback: false,
    }),
  isLocalhostApiBase: (apiBase: string) => {
    try {
      const hostname = new URL(apiBase).hostname;
      return (
        hostname === "localhost" ||
        hostname === "127.0.0.1" ||
        hostname === "::1"
      );
    } catch {
      return false;
    }
  },
}));

vi.mock("@/utils/api", () => ({
  getLocalApiBase: () => "http://localhost:2080/api",
  resolveSourceApiBase: (sourceKey?: string | null) =>
    sourceKey || "http://localhost:2080/api",
}));

vi.mock("@/utils/discovery", () => ({
  fetchDeviceResolution: vi.fn(async () => null),
}));

// ---------------------------------------------------------------------------
// Stateful HTMLAudioElement fake
// ---------------------------------------------------------------------------

type Listener = () => void;

class FakeMediaElement {
  paused = true;
  currentTime = 0;
  duration = 0;
  readyState = 0;
  volume = 1;
  error: { code: number; message: string } | null = null;
  failPlayWith: string | null = null;
  /** When false, tests call loadMetadata() manually (seek-before-metadata). */
  autoPrepare = true;
  private listeners = new Map<string, Set<Listener>>();
  private _src = "";

  addEventListener = vi.fn((type: string, fn: Listener) => {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(fn);
  });
  removeEventListener = vi.fn((type: string, fn: Listener) => {
    this.listeners.get(type)?.delete(fn);
  });

  /** Setting src mimics a fresh load: position and metadata reset. */
  get src(): string {
    return this._src;
  }
  set src(value: string) {
    this._src = value;
    this.currentTime = 0;
    this.duration = 0;
    this.readyState = 0;
    if (value && this.autoPrepare) {
      queueMicrotask(() => this.loadMetadata());
    }
  }

  emit(type: string) {
    this.listeners.get(type)?.forEach((fn) => fn());
  }

  loadMetadata(durationSec = 180) {
    if (!this._src) return;
    this.duration = durationSec;
    this.readyState = 2;
    this.emit("loadedmetadata");
    this.emit("canplay");
    if (!this.paused) {
      this.emit("playing");
    }
  }

  async play(): Promise<void> {
    if (this.failPlayWith) {
      const err = new Error(this.failPlayWith);
      (err as Error & { name: string }).name = this.failPlayWith;
      throw err;
    }
    if (!this.src) return;
    this.paused = false;
    this.emit("play");
    if (this.readyState >= 2) {
      this.emit("playing");
    }
  }

  pause() {
    if (this.paused) return;
    this.paused = true;
    this.emit("pause");
  }

  load() {}

  // ---- test helpers ----
  tickTo(sec: number) {
    this.currentTime = sec;
    this.emit("timeupdate");
  }

  naturalEnd() {
    this.currentTime = this.duration;
    this.paused = true;
    this.emit("ended");
  }

  mediaError() {
    this.error = { code: 2, message: "NETWORK_EMPTY" };
    this.paused = true;
    this.emit("error");
    this.error = null;
  }
}

// ---------------------------------------------------------------------------
// Stateful native service fake (mirrors MusicPlayerService behavior)
// ---------------------------------------------------------------------------

import type { PlaybackSnapshotEvent, QueueSong } from "music-notification-api";

class FakeNativeService {
  queue: QueueSong[] = [];
  index = -1;
  playMode = "sequential";
  prepared = false;
  playing = false;
  positionMs = 0;
  durationMs = 0;
  pendingSeekMs: number | null = null;
  pendingResume = true;
  handler: ((event: PlaybackSnapshotEvent) => void) | null = null;
  queuePushCount = 0;

  onPlaybackEvent(cb: (event: PlaybackSnapshotEvent) => void) {
    this.handler = cb;
    return () => {
      this.handler = null;
    };
  }

  status(): PlaybackSnapshotEvent["status"] {
    if (!this.prepared) return "idle";
    return this.playing ? "playing" : "paused";
  }

  emitEvent(reason: PlaybackSnapshotEvent["reason"], message?: string) {
    this.handler?.({
      reason,
      status: this.status(),
      index: this.index >= 0 ? this.index : -1,
      songId: this.queue[this.index]?.id ?? null,
      positionMs: this.positionMs,
      durationMs: this.durationMs,
      positionAtMs: Date.now(),
      playMode: this.playMode as PlaybackSnapshotEvent["playMode"],
      message,
    });
  }

  async setPlayingQueue(
    payload: { songs: QueueSong[]; currentIndex: number | null },
    playMode: string,
  ) {
    this.queuePushCount += 1;
    this.queue = payload.songs;
    this.index = payload.currentIndex ?? 0;
    this.playMode = playMode;
    this.emitEvent("transition");
  }

  async playTrackAtIndex(index: number, autoPlay: boolean, startAtMs?: number) {
    this.index = index;
    this.pendingResume = autoPlay;
    this.pendingSeekMs = startAtMs ?? null;
    this.prepared = false;
    this.playing = false;
    this.emitEvent("transition");
    await this.prepareTrack();
  }

  private async prepareTrack() {
    this.prepared = true;
    this.durationMs = 180000;
    if (this.pendingSeekMs !== null) {
      this.positionMs = this.pendingSeekMs;
      this.pendingSeekMs = null;
    }
    if (this.pendingResume) {
      this.playing = true;
    }
    this.emitEvent("transition");
    this.pendingResume = true;
  }

  async seek(position: number, options?: { autoPlay?: boolean }) {
    const autoPlay = options?.autoPlay ?? false;
    if (this.prepared) {
      this.positionMs = position;
      if (autoPlay && !this.playing) {
        this.playing = true;
      }
      this.emitEvent("transition");
      return;
    }
    this.pendingSeekMs = position;
    if (autoPlay && this.index >= 0) {
      this.emitEvent("transition");
      await this.prepareTrack();
      return;
    }
    this.emitEvent("transition");
  }

  async seekAndPlay(position: number) {
    await this.seek(position, { autoPlay: true });
  }

  async resume() {
    if (this.prepared && !this.playing) {
      this.playing = true;
      this.emitEvent("transition");
    }
  }

  async pause() {
    if (this.playing) {
      this.playing = false;
      this.emitEvent("transition");
    }
  }

  async stop() {
    this.prepared = false;
    this.playing = false;
    this.positionMs = 0;
    this.durationMs = 0;
    this.emitEvent("transition");
  }

  async setPlayMode(playMode: string) {
    this.playMode = playMode;
    this.emitEvent("transition");
  }

  async getPlaybackSession() {
    return {
      queue: {
        songs: this.queue,
        currentIndex: this.index >= 0 ? this.index : null,
      },
      runtime: {
        isPlaying: this.playing,
        positionMs: this.positionMs,
        durationMs: this.durationMs,
        status: this.status(),
      },
      playMode: this.playMode as PlaybackSnapshotEvent["playMode"],
      currentSongId: this.queue[this.index]?.id ?? null,
    };
  }

  async setVolume(_options: { volume: number }) {}
  async setNormalizationConfig(_options: unknown) {}
  async pauseAfter(_delayMs: number) {}

  /** Natural completion advances natively, like onCompletion -> playNextTrack. */
  async naturalEnd() {
    this.playing = false;
    this.positionMs = this.durationMs;
    this.emitEvent("transition");
    if (this.queue.length > 0) {
      await this.playTrackAtIndex((this.index + 1) % this.queue.length, true);
    }
  }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

vi.mock("music-notification-api", () => {
  const services = () =>
    (globalThis as unknown as { __playbackConformance: TestGlobals })
      .__playbackConformance;
  return {
    onPlaybackEvent: (cb: (event: PlaybackSnapshotEvent) => void) =>
      services().native!.onPlaybackEvent(cb),
    getPlaybackSession: () => services().native!.getPlaybackSession(),
    setPlayingQueue: (
      payload: { songs: QueueSong[]; currentIndex: number | null },
      playMode: string,
    ) => services().native!.setPlayingQueue(payload, playMode),
    playTrackAtIndex: (index: number, autoPlay: boolean, startAtMs?: number) =>
      services().native!.playTrackAtIndex(index, autoPlay, startAtMs),
    seek: (position: number, options?: { autoPlay?: boolean }) =>
      services().native!.seek(position, options),
    seekAndPlay: (position: number) => services().native!.seekAndPlay(position),
    resume: () => services().native!.resume(),
    pause: () => services().native!.pause(),
    stop: () => services().native!.stop(),
    setPlayMode: (mode: string) => services().native!.setPlayMode(mode),
    setVolume: (options: { volume: number }) =>
      services().native!.setVolume(options),
    setNormalizationConfig: (options: unknown) =>
      services().native!.setNormalizationConfig(options),
    pauseAfter: (delayMs: number) => services().native!.pauseAfter(delayMs),
  };
});

function makeSongs(): MusicInfo[] {
  return [
    { id: 1, name: "Song 1", lufs: -12, path: "/test/song1.mp3" },
    { id: 2, name: "Song 2", lufs: -14, path: "/test/song2.mp3" },
    { id: 3, name: "Song 3", lufs: -16, path: "/test/song3.mp3" },
  ];
}

interface Harness {
  engine: ReturnType<typeof createPlayback>;
  media: FakeMediaElement;
  native: FakeNativeService;
  songStarts: Array<{ songId: number | null; nextId: number | null }>;
}

async function setupHarness(
  kind: "web" | "android",
  opts?: { keepServices?: boolean },
): Promise<Harness> {
  testGlobals.platform.usesAndroidPlaybackBackend = kind === "android";
  if (!opts?.keepServices) {
    const media = new FakeMediaElement();
    testGlobals.media = media;
    if (kind === "web") {
      global.Audio = vi.fn(() => media) as unknown as typeof Audio;
    }
    testGlobals.native = new FakeNativeService();
  }

  const songStarts: Harness["songStarts"] = [];
  const songs = makeSongs();
  const engine = createPlayback({
    songs: () => songs,
    onSongStart: (song, next) =>
      songStarts.push({ songId: song.id, nextId: next?.id ?? null }),
    prepareSong: async (song) => song,
    sourceGroups: () => [],
  });
  await engine.initAudio();
  return {
    engine,
    media: testGlobals.media!,
    native: testGlobals.native!,
    songStarts,
  };
}

const eachBackend = (
  name: string,
  fn: (kind: "web" | "android", harnessKind: string) => Promise<void>,
) => {
  describe(name, () => {
    it("web", () => fn("web", "web"));
    it("android", () => fn("android", "android"));
  });
};

describe("playback interface conformance", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    const storage = new Map<string, string>();
    Object.defineProperty(globalThis, "localStorage", {
      value: {
        getItem: (key: string) => storage.get(key) ?? null,
        setItem: (key: string, value: string) => {
          storage.set(key, value);
        },
        removeItem: (key: string) => {
          storage.delete(key);
        },
      },
      configurable: true,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    removeStoredPlaybackSession();
  });

  eachBackend("playing a song loads and plays it", async (kind) => {
    const h = await setupHarness(kind);
    await h.engine.playSong(makeSongs()[0], undefined, makeSongs());

    expect(h.engine.isPlaying.value).toBe(true);
    expect(h.engine.status.value).toBe("playing");
    expect(h.engine.currentSong.value?.id).toBe(1);
    expect(h.engine.currentIndex.value).toBe(0);
    expect(h.engine.duration.value).toBe(180);
    if (kind === "web") {
      expect(h.media.src).toContain("/music/id/1");
      expect(h.media.paused).toBe(false);
    } else {
      expect(h.native.playing).toBe(true);
      expect(h.native.index).toBe(0);
    }
  });

  eachBackend("seek while paused auto-resumes (product rule)", async (kind) => {
    const h = await setupHarness(kind);
    await h.engine.playSong(makeSongs()[0], undefined, makeSongs());
    await h.engine.pause();
    expect(h.engine.isPlaying.value).toBe(false);

    await h.engine.seekToTime(60);

    expect(h.engine.isPlaying.value).toBe(true);
    expect(h.engine.currentTime.value).toBe(60);
    if (kind === "android") {
      expect(h.native.positionMs).toBe(60000);
      expect(h.native.playing).toBe(true);
    }
  });

  eachBackend(
    "lyric click on a loaded song seeks and resumes",
    async (kind) => {
      const h = await setupHarness(kind);
      await h.engine.playSong(makeSongs()[0], undefined, makeSongs());
      await h.engine.pause();

      // Exact path of useAppShell.handleLyricLineClick.
      await h.engine.seekToTime(42);

      expect(h.engine.isPlaying.value).toBe(true);
      expect(h.engine.currentTime.value).toBe(42);
    },
  );

  eachBackend("pause then play round-trips", async (kind) => {
    const h = await setupHarness(kind);
    await h.engine.playSong(makeSongs()[0], undefined, makeSongs());
    await h.engine.pause();
    expect(h.engine.isPlaying.value).toBe(false);
    await h.engine.play();
    expect(h.engine.isPlaying.value).toBe(true);
  });

  eachBackend(
    "rapid next taps settle on the last target, playing",
    async (kind) => {
      const h = await setupHarness(kind);
      await h.engine.playSong(makeSongs()[0], undefined, makeSongs());

      await h.engine.nextSong();
      await h.engine.nextSong();
      await h.engine.nextSong();

      // 0 -> 1 -> 2 -> wrap to 0
      expect(h.engine.currentSong.value?.id).toBe(1);
      expect(h.engine.isPlaying.value).toBe(true);
    },
  );

  eachBackend("next/previous wrap sequentially", async (kind) => {
    const h = await setupHarness(kind);
    await h.engine.playSong(makeSongs()[2], undefined, makeSongs(), 2);
    expect(h.engine.currentIndex.value).toBe(2);

    await h.engine.nextSong();
    expect(h.engine.currentSong.value?.id).toBe(1);
    await h.engine.previousSong();
    expect(h.engine.currentSong.value?.id).toBe(3);
    expect(h.engine.isPlaying.value).toBe(true);
  });

  eachBackend(
    "song start callback fires per song with next-song context",
    async (kind) => {
      const h = await setupHarness(kind);
      await h.engine.playSong(makeSongs()[0], undefined, makeSongs());
      await h.engine.nextSong();

      const started = h.songStarts.map((entry) => entry.songId);
      expect(started).toEqual([1, 2]);
      expect(h.songStarts[1].nextId).toBe(3);
    },
  );

  describe("web completion path (element events drive advance)", () => {
    it("song end advances to the next song", async () => {
      const h = await setupHarness("web");
      await h.engine.playSong(makeSongs()[0], undefined, makeSongs());

      h.media.naturalEnd();
      await vi.advanceTimersByTimeAsync(0);

      expect(h.engine.currentSong.value?.id).toBe(2);
      expect(h.engine.isPlaying.value).toBe(true);
    });

    it("loop mode replays the same song from the start", async () => {
      const h = await setupHarness("web");
      await h.engine.playSong(makeSongs()[0], undefined, makeSongs());
      await h.engine.togglePlayMode();
      await h.engine.togglePlayMode(); // sequential -> shuffle -> loop
      expect(h.engine.playMode.value).toBe("loop");

      h.media.tickTo(100);
      h.media.naturalEnd();
      await vi.advanceTimersByTimeAsync(0);

      expect(h.engine.currentSong.value?.id).toBe(1);
      expect(h.engine.currentTime.value).toBe(0);
      expect(h.engine.isPlaying.value).toBe(true);
    });

    it("media error skips to the next song", async () => {
      const h = await setupHarness("web");
      await h.engine.playSong(makeSongs()[0], undefined, makeSongs());

      h.media.mediaError();
      await vi.advanceTimersByTimeAsync(0);

      expect(h.engine.currentSong.value?.id).toBe(2);
      expect(h.engine.isPlaying.value).toBe(true);
    });
  });

  describe("android native path (events mirror the service)", () => {
    it("native completion auto-advance reconciles the engine", async () => {
      const h = await setupHarness("android");
      await h.engine.playSong(makeSongs()[0], undefined, makeSongs());
      expect(h.engine.currentSong.value?.id).toBe(1);

      await h.native.naturalEnd();

      expect(h.engine.currentSong.value?.id).toBe(2);
      expect(h.engine.isPlaying.value).toBe(true);
      expect(h.engine.currentIndex.value).toBe(1);
    });

    it("skipTo uses playTrackAtIndex without a queue re-push", async () => {
      const h = await setupHarness("android");
      await h.engine.playSong(makeSongs()[0], undefined, makeSongs());
      const pushesAfterLoad = h.native.queuePushCount;
      expect(pushesAfterLoad).toBe(1);

      await h.engine.nextSong();

      expect(h.native.queuePushCount).toBe(1); // no re-push
      expect(h.native.index).toBe(1);
      expect(h.engine.currentSong.value?.id).toBe(2);
      expect(h.engine.isPlaying.value).toBe(true);
    });

    it("webview restart restores session state and can resume", async () => {
      const first = await setupHarness("android");
      await first.engine.playSong(makeSongs()[0], undefined, makeSongs());
      await first.engine.pause();
      expect(first.engine.status.value).toBe("paused");

      // New engine + backend instances over the SAME native service state.
      const second = await setupHarness("android", { keepServices: true });

      expect(second.engine.currentSong.value?.id).toBe(1);
      expect(second.engine.status.value).toBe("paused");
      expect(second.engine.duration.value).toBe(180);

      await second.engine.play();
      expect(second.engine.isPlaying.value).toBe(true);
    });
  });

  describe("web restore + pre-metadata seeks", () => {
    it("restored session starts idle and a seek loads from the target", async () => {
      setStoredPlaybackSession({
        currentDeviceId: "",
        currentSongId: 1,
        queue: [
          {
            device_id: "",
            song_id: 1,
            filename: "song1.mp3",
            name: "Song 1",
            lufs: -12,
          },
        ],
        timestamp: Date.now(),
      });

      const h = await setupHarness("web");
      expect(h.engine.currentSong.value?.id).toBe(1);
      expect(h.engine.status.value).toBe("idle");

      // Lyric click on a restored-but-unloaded song.
      await h.engine.seekToTime(30);

      expect(h.engine.currentSong.value?.id).toBe(1);
      expect(h.engine.isPlaying.value).toBe(true);
      expect(h.engine.currentTime.value).toBe(30);
    });

    it("seek before metadata lands once metadata arrives", async () => {
      const h = await setupHarness("web");
      h.media.autoPrepare = false;
      await h.engine.playSong(makeSongs()[0], undefined, makeSongs());
      expect(h.engine.duration.value).toBe(0);

      // Metadata arrives after the seek was requested.
      h.engine.seekToTime(75);
      await vi.advanceTimersByTimeAsync(0);
      h.media.loadMetadata();

      expect(h.media.currentTime).toBe(75);
      expect(h.engine.currentTime.value).toBe(75);
      expect(h.engine.isPlaying.value).toBe(true);
    });
  });
});
