/**
 * Launch-file handoff for the "open as default music app" flow.
 *
 * When the OS launches Kaulan with a file the user double-clicked, the Tauri
 * shell pushes the path into the backend's launch broker. The frontend picks
 * it up via `consumeLaunchFile()` and plays it.
 *
 * - Cold start: `consumeLaunchFile()` is called once on mount (the broker was
 *   seeded before the page loaded).
 * - Warm start: `subscribeToLaunchEvents()` keeps an `EventSource` open for
 *   the lifetime of the page; each event triggers a fresh `consumeLaunchFile()`.
 *
 * Related documentation: `docs/default-music-app.md`.
 */

import type { MusicInfo } from "@/types/music";
import { resolveSourceApiBase } from "@/utils/api";

export interface LaunchFileResult {
  hasLaunch: boolean;
  song: MusicInfo | null;
}

function filenameFromPath(absolutePath: string): string {
  // Handle both POSIX `/` and Windows `\` separators.
  const parts = absolutePath.split(/[\\/]/);
  return parts[parts.length - 1] ?? absolutePath;
}

function nameFromFilename(filename: string): string {
  const dot = filename.lastIndexOf(".");
  return dot > 0 ? filename.slice(0, dot) : filename;
}

/**
 * Build a synthetic `MusicInfo` for an arbitrary absolute audio path or
 * Android `content://` URI.
 *
 * `displayName` is the friendly filename Android's ContentResolver returned
 * for the URI (e.g. "song.mp3"). When absent (desktop paths, or Android URIs
 * without a `_display_name` column), the name is derived from the last path
 * segment.
 */
export function buildLaunchSong(
  absolutePath: string,
  displayName?: string | null,
): MusicInfo {
  const filename =
    displayName && displayName.trim().length > 0
      ? displayName
      : filenameFromPath(absolutePath);
  const apiBase = resolveSourceApiBase(null);
  const encodedPath = encodeURIComponent(absolutePath);
  return {
    id: -1,
    name: nameFromFilename(filename),
    path: absolutePath,
    lufs: null,
    // Bypasses the DB-id playback URL — useAudioPlayer prefers stream_url when
    // present. The backend's /api/music/path handler streams via StdFs without
    // a DB lookup.
    stream_url: `${apiBase}/music/path?p=${encodedPath}`,
    // Sidecar lyrics and embedded cover art for launch files go through the
    // same path-based endpoints so the click-open flow shows the same metadata
    // as regular playlist playback. Both endpoints return 404/400 gracefully
    // when the file has no lyric sidecar or cover art, which the composables
    // treat as "no lyrics/cover" without surfacing an error.
    lyrics_url: `${apiBase}/lyrics/path?p=${encodedPath}`,
    cover_url: `${apiBase}/music/path/cover?p=${encodedPath}`,
    source_key: null,
    is_temporary: true,
    mediaType: "audio",
  };
}

/**
 * Atomically consume the pending launch file (if any).
 *
 * Returns `{hasLaunch: false, song: null}` when there's no pending file or the
 * request fails — callers don't need to handle errors separately.
 *
 * Retries network errors (fetch throws) up to `MAX_CONSUME_ATTEMPTS` times so a
 * cold-start launch file survives a brief boot race where the backend hasn't
 * bound :2080 yet. HTTP responses (even non-2xx) are not retried — the broker
 * endpoint is too simple to produce 5xx under normal load, and a 200 with
 * `path: null` genuinely means "no pending launch".
 */
const MAX_CONSUME_ATTEMPTS = 3;
const CONSUME_RETRY_DELAY_MS = 500;

export async function consumeLaunchFile(): Promise<LaunchFileResult> {
  const apiBase = resolveSourceApiBase(null);
  const url = `${apiBase}/launch/pending`;

  let resp: Response | null = null;
  for (let attempt = 1; attempt <= MAX_CONSUME_ATTEMPTS; attempt += 1) {
    try {
      resp = await fetch(url);
      break;
    } catch {
      // Network error — backend may not have bound :2080 yet. Retry after a
      // short delay so cold-start launches survive the boot race.
      if (attempt === MAX_CONSUME_ATTEMPTS) {
        return { hasLaunch: false, song: null };
      }
      await new Promise((resolve) =>
        setTimeout(resolve, CONSUME_RETRY_DELAY_MS),
      );
    }
  }

  if (!resp || !resp.ok) {
    return { hasLaunch: false, song: null };
  }
  try {
    const data = (await resp.json()) as {
      path: string | null;
      display_name?: string | null;
    };
    if (!data.path) {
      return { hasLaunch: false, song: null };
    }
    return {
      hasLaunch: true,
      song: buildLaunchSong(data.path, data.display_name),
    };
  } catch {
    return { hasLaunch: false, song: null };
  }
}

/**
 * Subscribe to backend launch events via Server-Sent Events.
 *
 * `onLaunch` is called once per event the backend pushes (one per warm-start
 * launch). The browser's `EventSource` API auto-reconnects on disconnect.
 * Returns a disposer that closes the connection.
 */
export function subscribeToLaunchEvents(onLaunch: () => void): () => void {
  if (typeof window === "undefined") {
    return () => {};
  }

  const apiBase = resolveSourceApiBase(null);
  // EventSource takes a URL relative to the page origin. In the Tauri webview
  // the page origin is `tauri.localhost`, so we need an absolute URL pointing
  // at the backend. In dev (browser on localhost:3000) the same absolute URL
  // is fine because the Vite proxy is bypassed for non-/api paths but here we
  // include /api explicitly.
  const url = `${apiBase}/launch/events`;

  let es: EventSource | null = null;
  try {
    es = new EventSource(url);
    es.onmessage = () => {
      onLaunch();
    };
    // Browser auto-reconnects on error; nothing to do here.
    es.onerror = () => {
      // Swallow — EventSource reconnects automatically.
    };
  } catch {
    return () => {};
  }

  return () => {
    es?.close();
    es = null;
  };
}
