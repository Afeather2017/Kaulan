import type { MusicInfo } from "@/types/music";
import type { NormalizationMode } from "music-notification-api";

export type PlayMode = "sequential" | "shuffle" | "loop";

export type PlaybackStatus =
  | "idle"
  | "loading"
  | "playing"
  | "paused"
  | "error";

export interface PlaybackError {
  kind: "decode" | "network" | "autoplay_blocked" | "unknown";
  message?: string;
  recoverable: boolean;
}

/**
 * Backend-observed transport state. This is the single authority for status,
 * position, duration, and the loaded song — the engine only mirrors it via
 * applySnapshot and layers its own queue authority on top.
 */
export interface PlaybackSnapshot {
  status: PlaybackStatus;
  /** Resolved metadata of the loaded song; null when nothing is loaded. */
  song: MusicInfo | null;
  positionSec: number;
  /** Wall clock (Date.now()) at which `positionSec` was sampled, so the UI can interpolate between snapshots. */
  positionAtMs: number;
  durationSec: number;
  /** Index into the engine queue for `song`; -1 when unknown/none. */
  currentIndex: number;
  playMode: PlayMode;
  error?: PlaybackError;
  /**
   * Full queue when the backend owns a converted copy the engine should adopt
   * (Android restore / native-side queue conversion). Optional on web.
   */
  queue?: MusicInfo[];
}

export type SnapshotReason =
  | "init"
  | "restore"
  | "tick"
  | "transition"
  | "seek-acked"
  | "error";

export type SongEndCause = "completed" | "error";

export type BackendEvent =
  | { type: "snapshot"; snapshot: PlaybackSnapshot; reason: SnapshotReason }
  | {
      type: "songEnded";
      song: MusicInfo;
      index: number;
      endedBy: SongEndCause;
    };

/**
 * Product rule: seeking while paused resumes playback. `playWhenReady`
 * defaults to true everywhere; features that want seek-without-resume must
 * pass false explicitly (no current caller does).
 */
export interface SeekOptions {
  playWhenReady?: boolean;
}

export interface LoadOptions {
  startAtSec?: number;
  playWhenReady?: boolean;
}

export interface NormalizationConfigPayload {
  mode: NormalizationMode;
  manualVolume: number;
  fixedLufs: number;
  lufsPrecacheCount: number;
  currentVolume: number;
}

/**
 * The one interface both the web (HTMLAudioElement) and Android (native
 * service) backends implement. Commands express intent; backends report
 * observed state exclusively through `on` events. Backends never read app
 * state and never write engine refs.
 */
export interface PlaybackBackend {
  readonly kind: "web" | "android";
  /** Prepare the backend and restore the persisted session, if any. */
  init(): Promise<PlaybackSnapshot>;
  /** One-shot full state fetch (init/reconnect/visibility resync). */
  getSession(): Promise<PlaybackSnapshot>;
  /** Set the queue and start (or prepare) the song at `index`. */
  load(queue: MusicInfo[], index: number, options?: LoadOptions): Promise<void>;
  /** Resume the current song. Only called when a song is loaded. */
  play(): Promise<void>;
  pause(): Promise<void>;
  seek(positionSec: number, options?: SeekOptions): Promise<void>;
  /** Switch to an existing queue index without re-pushing the queue. */
  skipTo(index: number, options?: LoadOptions): Promise<void>;
  setPlayMode(mode: PlayMode): Promise<void>;
  /** Mirror queue mutations (order edits, LUFS patches) to the backend. */
  syncQueue(queue: MusicInfo[], currentIndex: number): Promise<void>;
  setVolume(volume: number): Promise<void>;
  syncNormalizationConfig(config: NormalizationConfigPayload): Promise<void>;
  /** Sleep timer. Android: native deadline; web: no-op (store timer pauses). */
  pauseAfter(delayMs: number): Promise<void>;
  clearPlaybackState(): Promise<void>;
  on(listener: (event: BackendEvent) => void): () => void;
  dispose(): void;
}
