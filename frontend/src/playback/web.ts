import type { MusicInfo } from "@/types/music";
import { getLocalApiBase, resolveSourceApiBase } from "@/utils/api";
import { fetchDeviceResolution } from "@/utils/discovery";
import { loadStoredQueue, resolveQueueState } from "@/playback/shared";
import { PlaybackStartError } from "@/playback/engine";
import type {
  BackendEvent,
  LoadOptions,
  PlaybackBackend,
  PlaybackError,
  PlaybackSnapshot,
  PlayMode,
} from "@/playback/types";

export interface WebBackendOptions {
  sourceGroups: () => import("@/types/library").LibrarySourceGroup[];
}

const ERROR_NAME_BY_CODE: Record<number, PlaybackError["kind"]> = {
  2: "network",
  3: "decode",
  4: "decode",
};

/**
 * Web backend: a single reused HTMLAudioElement is the transport authority.
 * Media events are mapped to snapshots; commands act on the element directly.
 * The old isPlaying-watcher command channel, isPlayingInternal suppression,
 * and the 50 ms settle delay are gone — the generation token survives only
 * inside `load`, where overlapping switches genuinely race one element.
 */
export function createWebBackend(options: WebBackendOptions): PlaybackBackend {
  const listeners = new Set<(event: BackendEvent) => void>();
  let audio: HTMLAudioElement | null = null;
  let mirror: MusicInfo[] = [];
  let mirrorIndex = -1;
  let mirrorPlayMode: PlayMode = "sequential";
  let resolvedSong: MusicInfo | null = null;
  let pendingPlaySeekTime: number | undefined;
  let playbackGeneration = 0;
  let disposed = false;

  const emit = (event: BackendEvent) => {
    listeners.forEach((listener) => listener(event));
  };

  const buildAudioUrl = (
    songId: number,
    sourceKey?: string | null,
    seekTime?: number,
  ): string => {
    const apiBase = resolveSourceApiBase(sourceKey);
    let url: URL;
    try {
      url = new URL(`${apiBase}/music/id/${songId}`);
    } catch {
      url = new URL(`${apiBase}/music/id/${songId}`, window.location.origin);
    }
    if (seekTime !== undefined && audio && audio.duration > 0) {
      url.searchParams.set("position", (seekTime / audio.duration).toString());
    }
    return url.toString();
  };

  const buildSongPlaybackUrl = (song: MusicInfo, seekTime?: number): string => {
    if (seekTime !== undefined) {
      return buildAudioUrl(song.id, song.source_key, seekTime);
    }
    return song.stream_url ?? buildAudioUrl(song.id, song.source_key);
  };

  const resolveSongForWebPlayback = async (
    song: MusicInfo,
  ): Promise<MusicInfo | null> => {
    if (song.source || (song.is_temporary && song.id <= 0)) {
      return song;
    }
    const deviceId = song.device_id ?? "";
    const apiBase = deviceId
      ? (await fetchDeviceResolution(deviceId))?.api_url
      : getLocalApiBase();
    if (!apiBase) {
      return null;
    }
    return {
      ...song,
      source_key: apiBase,
      stream_url: `${apiBase}/music/id/${song.id}`,
      cover_url: `${apiBase}/music/id/${song.id}/cover`,
      is_temporary: false,
    };
  };

  const ensureAudioElement = (): HTMLAudioElement => {
    if (audio) {
      return audio;
    }
    const element = new Audio();
    element.addEventListener("timeupdate", () => {
      emit({
        type: "snapshot",
        snapshot: snapshotFromElement(),
        reason: "tick",
      });
    });
    element.addEventListener("loadedmetadata", () => {
      if (pendingPlaySeekTime !== undefined) {
        const clamped = Math.max(
          0,
          Math.min(
            pendingPlaySeekTime,
            element.duration || pendingPlaySeekTime,
          ),
        );
        element.currentTime = clamped;
        pendingPlaySeekTime = undefined;
      }
      emit({
        type: "snapshot",
        snapshot: snapshotFromElement(),
        reason: "transition",
      });
    });
    element.addEventListener("seeked", () => {
      emit({
        type: "snapshot",
        snapshot: snapshotFromElement(),
        reason: "seek-acked",
      });
    });
    // `play` fires the moment paused flips to false; `playing` only fires once
    // data is buffered. After a seek while paused the element can emit `play`
    // without ever reaching `playing`, so both drive the status.
    element.addEventListener("play", () => {
      emit({
        type: "snapshot",
        snapshot: snapshotFromElement("playing"),
        reason: "transition",
      });
    });
    element.addEventListener("playing", () => {
      emit({
        type: "snapshot",
        snapshot: snapshotFromElement("playing"),
        reason: "transition",
      });
    });
    element.addEventListener("pause", () => {
      emit({
        type: "snapshot",
        snapshot: snapshotFromElement(),
        reason: "transition",
      });
    });
    element.addEventListener("ended", () => {
      const song = resolvedSong;
      const index = mirrorIndex;
      emit({
        type: "snapshot",
        snapshot: snapshotFromElement("paused"),
        reason: "transition",
      });
      if (song && index >= 0 && !disposed) {
        emit({ type: "songEnded", song, index, endedBy: "completed" });
      }
    });
    element.addEventListener("error", () => {
      // MEDIA_ERR_ABORTED (code 1) is expected when a src swap aborts an
      // in-flight load (rapid skip); it is not a failure and must not
      // trigger the error-skip chain.
      if (element.error && element.error.code === 1) {
        return;
      }
      const song = resolvedSong;
      const index = mirrorIndex;
      const playbackError: PlaybackError = {
        kind:
          (element.error && ERROR_NAME_BY_CODE[element.error.code]) ||
          "unknown",
        message: element.error?.message,
        recoverable: true,
      };
      emit({
        type: "snapshot",
        snapshot: snapshotFromElement("error", playbackError),
        reason: "error",
      });
      if (song && index >= 0 && !disposed) {
        emit({ type: "songEnded", song, index, endedBy: "error" });
      }
    });
    audio = element;
    return element;
  };

  const snapshotFromElement = (
    statusOverride?: PlaybackSnapshot["status"],
    playbackError?: PlaybackError,
  ): PlaybackSnapshot => {
    const element = audio;
    let status = statusOverride;
    if (!status) {
      if (!element || !element.src) {
        status = "idle";
      } else {
        status = element.paused ? "paused" : "playing";
      }
    }
    return {
      status,
      song: resolvedSong,
      positionSec: element?.currentTime ?? 0,
      positionAtMs: Date.now(),
      durationSec: element?.duration ?? 0,
      currentIndex: mirrorIndex,
      playMode: mirrorPlayMode,
      error: playbackError,
    };
  };

  // Resolves once the element has buffered enough to play, with a timeout
  // fallback. Used to retry play() after an AbortError: on auto-advance there
  // is no user gesture to resume an interrupted element.
  const waitForAudioReady = (
    element: HTMLAudioElement,
    timeoutMs: number,
  ): Promise<void> =>
    new Promise((resolve) => {
      if (element.readyState >= 2) {
        resolve();
        return;
      }
      let settled = false;
      const finish = () => {
        if (settled) return;
        settled = true;
        element.removeEventListener("canplay", finish);
        resolve();
      };
      element.addEventListener("canplay", finish);
      setTimeout(finish, timeoutMs);
    });

  const loadInternal = async (
    queue: MusicInfo[],
    index: number,
    loadOptions?: LoadOptions,
  ): Promise<void> => {
    const playWhenReady = loadOptions?.playWhenReady ?? true;
    const startAtSec = loadOptions?.startAtSec;
    const myGeneration = ++playbackGeneration;
    const target = queue[index];

    if (!target) {
      return;
    }

    // Resolve the target's stream URL; if the device is unreachable, walk the
    // queue for the next resolvable song (preserved failure-skip behavior).
    const resolved = await resolveSongForWebPlayback(target);
    if (myGeneration !== playbackGeneration || disposed) return;
    if (!resolved) {
      const failedIndex = index;
      for (let offset = 1; offset <= queue.length; offset += 1) {
        const candidateIndex = (failedIndex + offset) % queue.length;
        const candidate = queue[candidateIndex];
        if (!candidate || candidate === target) continue;
        const resolvedCandidate = await resolveSongForWebPlayback(candidate);
        if (myGeneration !== playbackGeneration || disposed) return;
        if (!resolvedCandidate) continue;
        await loadInternal(queue, candidateIndex, {
          playWhenReady,
        });
        return;
      }
      resolvedSong = null;
      emit({
        type: "snapshot",
        snapshot: snapshotFromElement("error", {
          kind: "network",
          message: "No reachable source for queue",
          recoverable: false,
        }),
        reason: "error",
      });
      return;
    }

    // Reuse the single unlocked element: Safari/WebKit block play() on a
    // brand-new element without a user gesture, so swapping src keeps
    // auto-advance going.
    const element = ensureAudioElement();
    if (!element.paused) {
      element.pause();
    }

    mirror = queue;
    mirrorIndex = index;
    resolvedSong = resolved;
    const sourceUrl = buildSongPlaybackUrl(resolved, startAtSec);
    pendingPlaySeekTime = startAtSec;
    element.src = sourceUrl;
    element.preload = "auto";

    emit({
      type: "snapshot",
      snapshot: snapshotFromElement("loading"),
      reason: "transition",
    });

    try {
      await element.play();
      if (myGeneration !== playbackGeneration || disposed) return;
    } catch (error) {
      if (myGeneration !== playbackGeneration || disposed) return;
      const errorName = error instanceof Error ? error.name : String(error);
      if (errorName === "AbortError") {
        // Interrupted before playback began — typically a still-loading
        // source. Wait for readiness, then start explicitly.
        await waitForAudioReady(element, 2000);
        if (myGeneration !== playbackGeneration || disposed) return;
        try {
          await element.play();
        } catch {
          if (myGeneration !== playbackGeneration || disposed) return;
          emit({
            type: "snapshot",
            snapshot: snapshotFromElement(),
            reason: "transition",
          });
        }
        return;
      }
      emit({
        type: "snapshot",
        snapshot: snapshotFromElement("error", {
          kind: "autoplay_blocked",
          message: error instanceof Error ? error.message : String(error),
          recoverable: false,
        }),
        reason: "error",
      });
      throw new PlaybackStartError("Autoplay was blocked by the browser");
    }
  };

  const backend: PlaybackBackend = {
    kind: "web",

    init: async () => {
      const element = ensureAudioElement();
      const stored = loadStoredQueue(options.sourceGroups());
      if (stored) {
        const restored = resolveQueueState(
          stored.queue,
          stored.currentDeviceId,
          stored.currentSongId,
          null,
        );
        mirror = restored.queue;
        mirrorIndex = restored.currentIndex;
        resolvedSong = restored.currentSong;
      }
      return {
        status: "idle",
        song: resolvedSong,
        positionSec: element.currentTime || 0,
        positionAtMs: Date.now(),
        durationSec: element.duration || 0,
        currentIndex: mirrorIndex,
        playMode: mirrorPlayMode,
      };
    },

    getSession: async () => snapshotFromElement(),

    load: async (queue, index, loadOptions) => {
      await loadInternal(queue, index, loadOptions);
    },

    play: async () => {
      const element = audio;
      if (!element || !element.src) {
        return;
      }
      try {
        await element.play();
      } catch (error) {
        const errorName = error instanceof Error ? error.name : String(error);
        if (errorName === "AbortError") {
          // Interrupted (usually pause-before-buffer). The play/pause
          // listeners mirror the element's real state; reconcile only.
          emit({
            type: "snapshot",
            snapshot: snapshotFromElement(),
            reason: "transition",
          });
          return;
        }
        throw error;
      }
    },

    pause: async () => {
      audio?.pause();
    },

    seek: async (positionSec, seekOptions) => {
      const playWhenReady = seekOptions?.playWhenReady ?? true;
      const element = audio;
      if (!element || !element.src) {
        return;
      }
      if (element.duration > 0) {
        element.currentTime = Math.max(
          0,
          Math.min(positionSec, element.duration),
        );
      } else {
        // Metadata not loaded yet: park the target for loadedmetadata.
        pendingPlaySeekTime = positionSec;
      }
      emit({
        type: "snapshot",
        snapshot: snapshotFromElement(),
        reason: "transition",
      });
      if (playWhenReady && element.paused) {
        try {
          await element.play();
        } catch (error) {
          if (error instanceof Error && error.name === "AbortError") {
            return;
          }
          throw error;
        }
      }
    },

    skipTo: async (index, loadOptions) => {
      await loadInternal(mirror, index, loadOptions);
    },

    setPlayMode: async (mode) => {
      mirrorPlayMode = mode;
    },

    syncQueue: async (queue, currentIndexValue) => {
      mirror = queue;
      mirrorIndex = currentIndexValue;
    },

    setVolume: async (volume) => {
      if (audio) {
        audio.volume = Math.min(1, Math.max(0, volume));
      }
    },

    syncNormalizationConfig: async (config) => {
      if (audio) {
        audio.volume = Math.min(1, Math.max(0, config.currentVolume));
      }
    },

    pauseAfter: async () => {
      // Web sleep timer lives in the store (useTimer pauses via engine.pause).
    },

    clearPlaybackState: async () => {
      if (audio) {
        audio.pause();
        audio.removeAttribute("src");
        audio.load();
      }
      resolvedSong = null;
      mirrorIndex = -1;
    },

    on: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },

    dispose: () => {
      disposed = true;
      listeners.clear();
      if (audio) {
        audio.pause();
        audio = null;
      }
    },
  };

  return backend;
}
