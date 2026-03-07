/**
 * Cookie utilities for server URL persistence
 *
 * @module utils/cookies
 */

/**
 * Cookie keys used throughout the application
 */
export const COOKIE_KEYS = {
  SERVER_URL: 'kaulan_server_url',
  VIEW_MODE: 'kaulan_view_mode'
} as const

/**
 * Set a cookie with expiration
 * @param name - Cookie name
 * @param value - Cookie value
 * @param days - Days until expiration (use 3650 for ~10 years / "permanent")
 */
function setCookie(name: string, value: string, days: number): void {
  const date = new Date()
  date.setTime(date.getTime() + days * 24 * 60 * 60 * 1000)
  const expires = `expires=${date.toUTCString()}`
  document.cookie = `${name}=${value};${expires};path=/`
}

/**
 * Set a "permanent" cookie (10 years expiration)
 * @param name - Cookie name
 * @param value - Cookie value
 */
function setPermanentCookie(name: string, value: string): void {
  setCookie(name, value, 3650)
}

/**
 * Get a cookie value by name
 * @param name - Cookie name
 * @returns The cookie value or empty string if not found
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
 * Delete a cookie by name
 * @param name - Cookie name
 */
function deleteCookie(name: string): void {
  document.cookie = `${name}=;expires=Thu, 01 Jan 1970 00:00:00 UTC;path=/`
}

/**
 * Get the stored server URL from cookies
 * @returns The server URL or empty string if not set
 */
export function getServerUrl(): string {
  return getCookie(COOKIE_KEYS.SERVER_URL) || ''
}

/**
 * Save the server URL to cookies
 * @param url - The server URL to save
 */
export function setServerUrl(url: string): void {
  setPermanentCookie(COOKIE_KEYS.SERVER_URL, url)
}

/**
 * Remove the stored server URL from cookies
 */
export function removeServerUrl(): void {
  deleteCookie(COOKIE_KEYS.SERVER_URL)
}

/**
 * Get the stored view mode from cookies
 * @returns The view mode ('collection' or 'folder', defaults to 'collection')
 */
export function getViewMode(): string {
  return getCookie(COOKIE_KEYS.VIEW_MODE) || 'collection'
}

/**
 * Save the view mode to cookies
 * @param mode - The view mode to save ('collection' or 'folder')
 */
export function setViewMode(mode: string): void {
  setPermanentCookie(COOKIE_KEYS.VIEW_MODE, mode)
}

/**
 * Remove the stored view mode from cookies
 */
export function removeViewMode(): void {
  deleteCookie(COOKIE_KEYS.VIEW_MODE)
}
