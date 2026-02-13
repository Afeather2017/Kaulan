/**
 * Cookie utilities for server URL persistence
 *
 * @module utils/cookies
 */

/**
 * Cookie keys used throughout the application
 */
export const COOKIE_KEYS = {
  SERVER_URL: 'kaulan_server_url'
} as const

/**
 * Set a cookie with expiration
 * @param name - Cookie name
 * @param value - Cookie value
 * @param days - Days until expiration
 */
function setCookie(name: string, value: string, days: number): void {
  const date = new Date()
  date.setTime(date.getTime() + days * 24 * 60 * 60 * 1000)
  const expires = `expires=${date.toUTCString()}`
  document.cookie = `${name}=${value};${expires};path=/`
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
  setCookie(COOKIE_KEYS.SERVER_URL, url, 365)
}

/**
 * Remove the stored server URL from cookies
 */
export function removeServerUrl(): void {
  deleteCookie(COOKIE_KEYS.SERVER_URL)
}
