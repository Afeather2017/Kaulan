/**
 * Persistent storage utilities for frontend settings using localStorage.
 *
 * @module utils/storage
 */

import { getLocalApiBase } from "@/utils/api";

/**
 * Storage keys used throughout the application
 */
export const STORAGE_KEYS = {
  VIEW_MODE: "kaulan_view_mode",
  SHOW_LUFS: "kaulan_show_lufs",
  LUFS_PRECACHE_COUNT: "kaulan_lufs_precache_count",
  MEDIA_TYPES: "kaulan_media_types",
  PLAYBACK_SESSION: "kaulan_playback_session",
  DISABLE_HEADSET_MEDIA_BUTTON: "kaulan_disable_headset_media_button",
  TIMER_EXIT_APP_ON_ANDROID: "kaulan_timer_exit_app_on_android",
  MANUAL_DEVICES: "kaulan_manual_devices",
  LOCAL_COLLECTIONS: "kaulan_local_collections",
  DEFAULT_ONLINE_SEARCH_API_BASE: "kaulan_default_online_search_api_base",
  ALLOW_TEXT_SELECTION: "kaulan_allow_text_selection",
} as const;

export const DEFAULT_LUFS_PRECACHE_COUNT = 5;
export const MIN_LUFS_PRECACHE_COUNT = 0;
export const MAX_LUFS_PRECACHE_COUNT = 20;

export interface ManualDevice {
  api_url: string;
  device_id?: string;
  device_name?: string;
  added_at: number;
  last_fetched?: number;
}

export interface StoredCollectionSong {
  id: number;
  name: string;
  lufs: number | null;
  path: string;
  stream_url?: string | null;
  cover_url?: string | null;
  source_key?: string | null;
  sourceLabel?: string;
  rowKey?: string;
  mediaType?: "audio" | "video";
}

export interface StoredLocalCollection {
  id: number;
  name: string;
  created_at: string;
  songs: StoredCollectionSong[];
}

export interface StoredPlaybackQueueSong {
  id: number;
  name: string;
  path: string;
  url: string;
  lufs: number | null;
  coverUrl?: string | null;
  sourceKey?: string | null;
}

export interface StoredPlaybackSession {
  currentSongId: number | null;
  currentSongUrl?: string | null;
  queue: StoredPlaybackQueueSong[];
  timestamp: number;
}

function getStorageValue(key: string): string {
  try {
    const stored = localStorage.getItem(key);
    return stored || "";
  } catch (error) {
    console.error(`Failed to read localStorage key ${key}:`, error);
    return "";
  }
}

function setStorageValue(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch (error) {
    console.error(`Failed to write localStorage key ${key}:`, error);
  }
}

function removeStorageValue(key: string): void {
  try {
    localStorage.removeItem(key);
  } catch (error) {
    console.error(`Failed to remove localStorage key ${key}:`, error);
  }
}

/**
 * Get the stored view mode from localStorage
 * @returns The view mode ('collection' or 'folder', defaults to 'collection')
 */
export function getViewMode(): string {
  return getStorageValue(STORAGE_KEYS.VIEW_MODE) || "collection";
}

/**
 * Save the view mode to localStorage
 * @param mode - The view mode to save ('collection' or 'folder')
 */
export function setViewMode(mode: string): void {
  setStorageValue(STORAGE_KEYS.VIEW_MODE, mode);
}

/**
 * Remove the stored view mode from localStorage
 */
export function removeViewMode(): void {
  removeStorageValue(STORAGE_KEYS.VIEW_MODE);
}

/**
 * Get the stored show LUFS setting from localStorage
 * @returns Whether to show LUFS values (defaults to false)
 */
export function getShowLufs(): boolean {
  return getStorageValue(STORAGE_KEYS.SHOW_LUFS) === "true";
}

/**
 * Save the show LUFS setting to localStorage
 * @param show - Whether to show LUFS values
 */
export function setShowLufs(show: boolean): void {
  setStorageValue(STORAGE_KEYS.SHOW_LUFS, show ? "true" : "false");
}

export function normalizeLufsPrecacheCount(value: number): number {
  if (!Number.isFinite(value)) {
    return DEFAULT_LUFS_PRECACHE_COUNT;
  }

  return Math.min(
    MAX_LUFS_PRECACHE_COUNT,
    Math.max(MIN_LUFS_PRECACHE_COUNT, Math.round(value)),
  );
}

export function getLufsPrecacheCount(): number {
  const stored = getStorageValue(STORAGE_KEYS.LUFS_PRECACHE_COUNT);
  if (!stored) {
    return DEFAULT_LUFS_PRECACHE_COUNT;
  }

  return normalizeLufsPrecacheCount(Number(stored));
}

export function setLufsPrecacheCount(count: number): void {
  setStorageValue(
    STORAGE_KEYS.LUFS_PRECACHE_COUNT,
    String(normalizeLufsPrecacheCount(count)),
  );
}

export function getDisableHeadsetMediaButton(): boolean {
  return getStorageValue(STORAGE_KEYS.DISABLE_HEADSET_MEDIA_BUTTON) === "true";
}

export function setDisableHeadsetMediaButton(disabled: boolean): void {
  setStorageValue(
    STORAGE_KEYS.DISABLE_HEADSET_MEDIA_BUTTON,
    disabled ? "true" : "false",
  );
}

export function getTimerExitAppOnAndroid(): boolean {
  return getStorageValue(STORAGE_KEYS.TIMER_EXIT_APP_ON_ANDROID) === "true";
}

export function setTimerExitAppOnAndroid(enabled: boolean): void {
  setStorageValue(
    STORAGE_KEYS.TIMER_EXIT_APP_ON_ANDROID,
    enabled ? "true" : "false",
  );
}

/**
 * Get the stored media type filter from localStorage.
 * @returns Enabled media types, defaulting to ['audio']
 */
export function getMediaTypes(): string[] {
  const stored = getStorageValue(STORAGE_KEYS.MEDIA_TYPES);
  if (!stored) {
    return ["audio"];
  }

  try {
    const parsed = JSON.parse(stored);
    if (!Array.isArray(parsed)) {
      return ["audio"];
    }

    const valid = parsed.filter(
      (value): value is string => value === "audio" || value === "video",
    );
    return valid.length > 0 ? valid : ["audio"];
  } catch (error) {
    console.error("Failed to parse stored media types:", error);
    return ["audio"];
  }
}

/**
 * Save the enabled media type filter to localStorage.
 * @param mediaTypes - Enabled media types
 */
export function setMediaTypes(mediaTypes: string[]): void {
  setStorageValue(STORAGE_KEYS.MEDIA_TYPES, JSON.stringify(mediaTypes));
}

export function getManualDevices(): ManualDevice[] {
  const stored = getStorageValue(STORAGE_KEYS.MANUAL_DEVICES);
  if (!stored) {
    return [];
  }

  try {
    const parsed = JSON.parse(stored);
    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed.filter((item): item is ManualDevice => {
      if (!item || typeof item !== "object") {
        return false;
      }

      const device = item as Record<string, unknown>;
      return (
        typeof device.api_url === "string" &&
        typeof device.added_at === "number" &&
        (typeof device.device_id === "string" ||
          typeof device.device_id === "undefined") &&
        (typeof device.device_name === "string" ||
          typeof device.device_name === "undefined") &&
        (typeof device.last_fetched === "number" ||
          typeof device.last_fetched === "undefined")
      );
    });
  } catch (error) {
    console.error("Failed to parse manual devices:", error);
    return [];
  }
}

export function setManualDevices(devices: ManualDevice[]): void {
  setStorageValue(STORAGE_KEYS.MANUAL_DEVICES, JSON.stringify(devices));
}

export function getDefaultOnlineSearchApiBase(): string {
  const stored = getStorageValue(STORAGE_KEYS.DEFAULT_ONLINE_SEARCH_API_BASE);
  return stored || getLocalApiBase();
}

export function setDefaultOnlineSearchApiBase(apiBase: string): void {
  const trimmed = apiBase.trim();
  setStorageValue(
    STORAGE_KEYS.DEFAULT_ONLINE_SEARCH_API_BASE,
    trimmed || getLocalApiBase(),
  );
}

export function removeDefaultOnlineSearchApiBase(): void {
  removeStorageValue(STORAGE_KEYS.DEFAULT_ONLINE_SEARCH_API_BASE);
}

/**
 * Whether the user has opted in to mouse-based text selection in the UI.
 *
 * The web app disables text selection by default to keep the music-player UI
 * clean (issue #30). When a user wants to select lyrics, song names, or other
 * text, they flip this setting on.
 *
 * @returns Whether text selection is allowed (defaults to false)
 */
export function getAllowTextSelection(): boolean {
  return getStorageValue(STORAGE_KEYS.ALLOW_TEXT_SELECTION) === "true";
}

/**
 * Persist the text-selection preference.
 * @param allowed - Whether the user may select text/pictures with the mouse
 */
export function setAllowTextSelection(allowed: boolean): void {
  setStorageValue(
    STORAGE_KEYS.ALLOW_TEXT_SELECTION,
    allowed ? "true" : "false",
  );
}

function isStoredCollectionSong(value: unknown): value is StoredCollectionSong {
  if (!value || typeof value !== "object") {
    return false;
  }

  const song = value as Record<string, unknown>;
  return (
    typeof song.id === "number" &&
    typeof song.name === "string" &&
    (typeof song.lufs === "number" || song.lufs === null) &&
    typeof song.path === "string" &&
    (typeof song.stream_url === "string" ||
      song.stream_url === null ||
      typeof song.stream_url === "undefined") &&
    (typeof song.cover_url === "string" ||
      song.cover_url === null ||
      typeof song.cover_url === "undefined") &&
    (typeof song.source_key === "string" ||
      song.source_key === null ||
      typeof song.source_key === "undefined") &&
    (typeof song.sourceLabel === "string" ||
      typeof song.sourceLabel === "undefined") &&
    (typeof song.rowKey === "string" || typeof song.rowKey === "undefined") &&
    (song.mediaType === "audio" ||
      song.mediaType === "video" ||
      typeof song.mediaType === "undefined")
  );
}

export function getLocalCollections(): StoredLocalCollection[] {
  const stored = getStorageValue(STORAGE_KEYS.LOCAL_COLLECTIONS);
  if (!stored) {
    return [];
  }

  try {
    const parsed = JSON.parse(stored);
    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed.filter((value): value is StoredLocalCollection => {
      if (!value || typeof value !== "object") {
        return false;
      }

      const collection = value as Record<string, unknown>;
      return (
        typeof collection.id === "number" &&
        typeof collection.name === "string" &&
        typeof collection.created_at === "string" &&
        Array.isArray(collection.songs) &&
        collection.songs.every(isStoredCollectionSong)
      );
    });
  } catch (error) {
    console.error("Failed to parse local collections:", error);
    return [];
  }
}

export function setLocalCollections(
  collections: StoredLocalCollection[],
): void {
  setStorageValue(STORAGE_KEYS.LOCAL_COLLECTIONS, JSON.stringify(collections));
}

function isStoredPlaybackQueueSong(
  value: unknown,
): value is StoredPlaybackQueueSong {
  if (!value || typeof value !== "object") {
    return false;
  }

  const song = value as Record<string, unknown>;
  return (
    typeof song.id === "number" &&
    typeof song.name === "string" &&
    typeof song.path === "string" &&
    typeof song.url === "string" &&
    (typeof song.lufs === "number" || song.lufs === null) &&
    (typeof song.coverUrl === "string" ||
      song.coverUrl === null ||
      typeof song.coverUrl === "undefined") &&
    (typeof song.sourceKey === "string" ||
      song.sourceKey === null ||
      typeof song.sourceKey === "undefined")
  );
}

/**
 * Get the stored playback session from localStorage.
 * @returns Stored playback session or null when missing/invalid
 */
export function getStoredPlaybackSession(): StoredPlaybackSession | null {
  const stored = getStorageValue(STORAGE_KEYS.PLAYBACK_SESSION);
  if (!stored) {
    return null;
  }

  try {
    const parsed = JSON.parse(stored) as Record<string, unknown>;
    if (!Array.isArray(parsed.queue) || typeof parsed.timestamp !== "number") {
      return null;
    }

    const queue = parsed.queue.filter(isStoredPlaybackQueueSong);
    if (queue.length !== parsed.queue.length) {
      return null;
    }

    const currentSongId =
      typeof parsed.currentSongId === "number" ? parsed.currentSongId : null;
    const currentSongUrl =
      typeof parsed.currentSongUrl === "string" ? parsed.currentSongUrl : null;

    return {
      currentSongId,
      currentSongUrl,
      queue,
      timestamp: parsed.timestamp,
    };
  } catch (error) {
    console.error("Failed to parse stored playback session:", error);
    return null;
  }
}

/**
 * Save the current playback session to localStorage.
 * @param session - Playback session snapshot to persist
 */
export function setStoredPlaybackSession(session: StoredPlaybackSession): void {
  setStorageValue(STORAGE_KEYS.PLAYBACK_SESSION, JSON.stringify(session));
}

/**
 * Remove the stored playback session from localStorage.
 */
export function removeStoredPlaybackSession(): void {
  removeStorageValue(STORAGE_KEYS.PLAYBACK_SESSION);
}
