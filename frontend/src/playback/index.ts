import {
  createPlaybackEngine,
  type PlaybackEngineOptions,
} from "@/playback/engine";
import { createWebBackend } from "@/playback/web";
import { createAndroidBackend } from "@/playback/android";

export type { PlaybackEngine, PlaybackEngineOptions } from "@/playback/engine";
export { PlaybackStartError } from "@/playback/engine";
export type {
  BackendEvent,
  LoadOptions,
  NormalizationConfigPayload,
  PlayMode,
  PlaybackBackend,
  PlaybackError,
  PlaybackSnapshot,
  PlaybackStatus,
  SeekOptions,
  SnapshotReason,
  SongEndCause,
} from "@/playback/types";

/**
 * Factory used by the player store: builds both backends over the shared
 * interface and lets the engine pick per runtime platform in initAudio.
 */
export function createPlayback(options: PlaybackEngineOptions) {
  const sourceGroups = options.sourceGroups ?? (() => []);
  return createPlaybackEngine(options, () => ({
    web: createWebBackend({ sourceGroups }),
    android: createAndroidBackend({ sourceGroups }),
  }));
}
