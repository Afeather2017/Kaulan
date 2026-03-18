/**
 * Persistent storage utilities for frontend settings.
 *
 * Note: file name remains `cookies.ts` for compatibility with existing imports,
 * but storage now uses WebView localStorage instead of document.cookie.
 *
 * @module utils/cookies
 */

/**
 * Storage keys used throughout the application
 */
export const COOKIE_KEYS = {
  SERVER_URL: 'kaulan_server_url',
  VIEW_MODE: 'kaulan_view_mode',
  SHOW_LUFS: 'kaulan_show_lufs'
} as const

/**
 * Legacy cookie reader for one-time migration to localStorage.
 */
function getCookie(name: string): string {
  const nameEQ = `${name}=`
  const cookies = document.cookie.split(';')
  for (let i = 0; i < cookies.length; i++) {
    let cookie = cookies[i]
    while (cookie.charAt(0) === ' ') {
      cookie = cookie.substring(1, cookie.length)
    }
    if (cookie.indexOf(nameEQ) === 0) {
      return cookie.substring(nameEQ.length, cookie.length)
    }
  }
  return ''
}

/**
 * Delete a legacy cookie by name after migration.
 */
function deleteCookie(name: string): void {
  document.cookie = `${name}=;expires=Thu, 01 Jan 1970 00:00:00 UTC;path=/`
}

function getStorageValue(key: string): string {
  try {
    const stored = localStorage.getItem(key)
    if (stored && stored.length > 0) {
      return stored
    }

    // Backward compatibility: migrate existing cookie value to localStorage.
    const legacyCookie = getCookie(key)
    if (legacyCookie) {
      localStorage.setItem(key, legacyCookie)
      deleteCookie(key)
      return legacyCookie
    }
  } catch (error) {
    console.error(`Failed to read localStorage key ${key}:`, error)
  }

  return ''
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

  // Cleanup legacy cookie if present.
  deleteCookie(key)
}

/**
 * Get the stored server URL from localStorage
 * @returns The server URL or empty string if not set
 */
export function getServerUrl(): string {
  return getStorageValue(COOKIE_KEYS.SERVER_URL)
}

/**
 * Save the server URL to localStorage
 * @param url - The server URL to save
 */
export function setServerUrl(url: string): void {
  setStorageValue(COOKIE_KEYS.SERVER_URL, url)
}

/**
 * Remove the stored server URL from localStorage
 */
export function removeServerUrl(): void {
  removeStorageValue(COOKIE_KEYS.SERVER_URL)
}

/**
 * Get the stored view mode from localStorage
 * @returns The view mode ('collection' or 'folder', defaults to 'collection')
 */
export function getViewMode(): string {
  return getStorageValue(COOKIE_KEYS.VIEW_MODE) || 'collection'
}

/**
 * Save the view mode to localStorage
 * @param mode - The view mode to save ('collection' or 'folder')
 */
export function setViewMode(mode: string): void {
  setStorageValue(COOKIE_KEYS.VIEW_MODE, mode)
}

/**
 * Remove the stored view mode from localStorage
 */
export function removeViewMode(): void {
  removeStorageValue(COOKIE_KEYS.VIEW_MODE)
}

/**
 * Get the stored show LUFS setting from localStorage
 * @returns Whether to show LUFS values (defaults to false)
 */
export function getShowLufs(): boolean {
  return getStorageValue(COOKIE_KEYS.SHOW_LUFS) === 'true'
}

/**
 * Save the show LUFS setting to localStorage
 * @param show - Whether to show LUFS values
 */
export function setShowLufs(show: boolean): void {
  setStorageValue(COOKIE_KEYS.SHOW_LUFS, show ? 'true' : 'false')
}
