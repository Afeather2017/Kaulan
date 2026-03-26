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

import { getServerUrl, setServerUrl, removeServerUrl } from './storage'

const DEFAULT_API_BASE = 'http://localhost:2080/api'

function hasExplicitScheme(url: string): boolean {
  return /^[a-zA-Z][a-zA-Z\d+\-.]*:\/\//.test(url)
}

function appendApiPath(pathname: string): string {
  const trimmedPath = pathname.replace(/\/+$/, '')
  if (!trimmedPath || trimmedPath === '/') {
    return '/api'
  }
  if (trimmedPath.endsWith('/api')) {
    return trimmedPath
  }
  return `${trimmedPath}/api`
}

/**
 * Normalize user input into a full API base URL.
 *
 * Accepted inputs:
 * - `192.168.1.10` => `http://192.168.1.10:2080/api`
 * - `example.local` => `http://example.local:2080/api`
 * - `192.168.1.10:3000` => `http://192.168.1.10:3000/api`
 * - `https://example.local/service` => `https://example.local/service/api`
 *
 * @param input - Raw user input for the server address
 * @returns Normalized API base URL, or empty string for empty input
 */
export function normalizeApiBase(input: string): string {
  const trimmed = input.trim()
  if (!trimmed) {
    return ''
  }

  const candidate = hasExplicitScheme(trimmed) ? trimmed : `http://${trimmed}`
  const url = new URL(candidate)

  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    throw new Error('URL must use HTTP or HTTPS protocol')
  }

  if (!url.hostname) {
    throw new Error('URL must include a host')
  }

  if (!hasExplicitScheme(trimmed) && !url.port) {
    url.port = '2080'
  }

  url.pathname = appendApiPath(url.pathname)
  url.search = ''
  url.hash = ''

  return url.toString().replace(/\/$/, '')
}

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
  const normalized = normalizeApiBase(url)
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
