/**
 * Persistent storage utilities for frontend settings using localStorage.
 *
 * @module utils/storage
 */

/**
 * Storage keys used throughout the application
 */
export const STORAGE_KEYS = {
  SERVER_URL: 'kaulan_server_url',
  VIEW_MODE: 'kaulan_view_mode',
  SHOW_LUFS: 'kaulan_show_lufs',
  MEDIA_TYPES: 'kaulan_media_types',
  PLAYBACK_SESSION: 'kaulan_playback_session'
} as const

export interface StoredPlaybackQueueSong {
  id: number
  name: string
  path: string
  url: string
  lufs: number | null
}

export interface StoredPlaybackSession {
  currentSongId: number | null
  queue: StoredPlaybackQueueSong[]
  timestamp: number
}

function getStorageValue(key: string): string {
  try {
    const stored = localStorage.getItem(key)
    return stored || ''
  } catch (error) {
    console.error(`Failed to read localStorage key ${key}:`, error)
    return ''
  }
}

function setStorageValue(key: string, value: string): void {
  try {
    localStorage.setItem(key, value)
  } catch (error) {
    console.error(`Failed to write localStorage key ${key}:`, error)
  }
}

function removeStorageValue(key: string): void {
  try {
    localStorage.removeItem(key)
  } catch (error) {
    console.error(`Failed to remove localStorage key ${key}:`, error)
  }
}

/**
 * Get the stored server URL from localStorage
 * @returns The server URL or empty string if not set
 */
export function getServerUrl(): string {
  return getStorageValue(STORAGE_KEYS.SERVER_URL)
}

/**
 * Save the server URL to localStorage
 * @param url - The server URL to save
 */
export function setServerUrl(url: string): void {
  setStorageValue(STORAGE_KEYS.SERVER_URL, url)
}

/**
 * Remove the stored server URL from localStorage
 */
export function removeServerUrl(): void {
  removeStorageValue(STORAGE_KEYS.SERVER_URL)
}

/**
 * Get the stored view mode from localStorage
 * @returns The view mode ('collection' or 'folder', defaults to 'collection')
 */
export function getViewMode(): string {
  return getStorageValue(STORAGE_KEYS.VIEW_MODE) || 'collection'
}

/**
 * Save the view mode to localStorage
 * @param mode - The view mode to save ('collection' or 'folder')
 */
export function setViewMode(mode: string): void {
  setStorageValue(STORAGE_KEYS.VIEW_MODE, mode)
}

/**
 * Remove the stored view mode from localStorage
 */
export function removeViewMode(): void {
  removeStorageValue(STORAGE_KEYS.VIEW_MODE)
}

/**
 * Get the stored show LUFS setting from localStorage
 * @returns Whether to show LUFS values (defaults to false)
 */
export function getShowLufs(): boolean {
  return getStorageValue(STORAGE_KEYS.SHOW_LUFS) === 'true'
}

/**
 * Save the show LUFS setting to localStorage
 * @param show - Whether to show LUFS values
 */
export function setShowLufs(show: boolean): void {
  setStorageValue(STORAGE_KEYS.SHOW_LUFS, show ? 'true' : 'false')
}

/**
 * Get the stored media type filter from localStorage.
 * @returns Enabled media types, defaulting to ['audio']
 */
export function getMediaTypes(): string[] {
  const stored = getStorageValue(STORAGE_KEYS.MEDIA_TYPES)
  if (!stored) {
    return ['audio']
  }

  try {
    const parsed = JSON.parse(stored)
    if (!Array.isArray(parsed)) {
      return ['audio']
    }

    const valid = parsed.filter((value): value is string => value === 'audio' || value === 'video')
    return valid.length > 0 ? valid : ['audio']
  } catch (error) {
    console.error('Failed to parse stored media types:', error)
    return ['audio']
  }
}

/**
 * Save the enabled media type filter to localStorage.
 * @param mediaTypes - Enabled media types
 */
export function setMediaTypes(mediaTypes: string[]): void {
  setStorageValue(STORAGE_KEYS.MEDIA_TYPES, JSON.stringify(mediaTypes))
}

function isStoredPlaybackQueueSong(value: unknown): value is StoredPlaybackQueueSong {
  if (!value || typeof value !== 'object') {
    return false
  }

  const song = value as Record<string, unknown>
  return (
    typeof song.id === 'number' &&
    typeof song.name === 'string' &&
    typeof song.path === 'string' &&
    typeof song.url === 'string' &&
    (typeof song.lufs === 'number' || song.lufs === null)
  )
}

/**
 * Get the stored playback session from localStorage.
 * @returns Stored playback session or null when missing/invalid
 */
export function getStoredPlaybackSession(): StoredPlaybackSession | null {
  const stored = getStorageValue(STORAGE_KEYS.PLAYBACK_SESSION)
  if (!stored) {
    return null
  }

  try {
    const parsed = JSON.parse(stored) as Record<string, unknown>
    if (!Array.isArray(parsed.queue) || typeof parsed.timestamp !== 'number') {
      return null
    }

    const queue = parsed.queue.filter(isStoredPlaybackQueueSong)
    if (queue.length !== parsed.queue.length) {
      return null
    }

    const currentSongId = typeof parsed.currentSongId === 'number'
      ? parsed.currentSongId
      : null

    return {
      currentSongId,
      queue,
      timestamp: parsed.timestamp
    }
  } catch (error) {
    console.error('Failed to parse stored playback session:', error)
    return null
  }
}

/**
 * Save the current playback session to localStorage.
 * @param session - Playback session snapshot to persist
 */
export function setStoredPlaybackSession(session: StoredPlaybackSession): void {
  setStorageValue(STORAGE_KEYS.PLAYBACK_SESSION, JSON.stringify(session))
}

/**
 * Remove the stored playback session from localStorage.
 */
export function removeStoredPlaybackSession(): void {
  removeStorageValue(STORAGE_KEYS.PLAYBACK_SESSION)
}
