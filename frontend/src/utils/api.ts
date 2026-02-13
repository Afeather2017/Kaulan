/**
 * API configuration for Kaulan frontend
 *
 * In development, the Vite proxy forwards /api to the backend
 * In production (Tauri build), we need to use the full URL
 *
 * The server URL can be configured by the user and is stored in cookies.
 *
 * @module utils/api
 */

import { getServerUrl, setServerUrl, removeServerUrl } from './cookies'

const DEFAULT_API_BASE = 'http://localhost:2080/api'

/**
 * Get the current API base URL
 * Returns the user-configured URL if set, otherwise the default
 * @returns The API base URL
 */
export function getApiBase(): string {
  const saved = getServerUrl()
  return saved || DEFAULT_API_BASE
}

/**
 * Set and normalize the API base URL
 * Automatically appends '/api' if not present
 * @param url - The URL to set as API base
 */
export function setApiBase(url: string): void {
  let normalized = url
  if (!normalized.endsWith('/api')) {
    normalized = normalized.endsWith('/') ? normalized + 'api' : normalized + '/api'
  }
  setServerUrl(normalized)
}

/**
 * Reset the API base URL to default
 */
export function resetApiBase(): void {
  removeServerUrl()
}

/**
 * Default API base URL for backwards compatibility
 * Use getApiBase() for new code to get the user-configured URL
 */
export const API_BASE = getApiBase()
