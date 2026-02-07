import { invoke } from '@tauri-apps/api/core'

let cachedIsAndroid: boolean | null = null

/**
 * Check if the current platform is Android
 * @returns true if running on Android, false otherwise
 */
export async function checkIsAndroid(): Promise<boolean> {
  if (cachedIsAndroid !== null) {
    return cachedIsAndroid
  }

  // Check if we're in a Tauri context
  try {
    // Try to invoke a command that only works on Android
    // If it fails with "command not found", we're not on Android
    const platform = await invoke<string>('get_platform')
    cachedIsAndroid = platform === 'android'
    return cachedIsAndroid
  } catch {
    // Not in Tauri context or command not found
    // Fall back to user agent detection
    cachedIsAndroid = /android/i.test(navigator.userAgent)
    return cachedIsAndroid
  }
}

/**
 * Synchronous check for Android platform
 * Uses cached value or user agent detection
 * @returns true if running on Android, false otherwise
 */
export function isAndroid(): boolean {
  if (cachedIsAndroid !== null) {
    return cachedIsAndroid
  }

  // Synchronous user agent detection
  cachedIsAndroid = /android/i.test(navigator.userAgent)
  return cachedIsAndroid
}

/**
 * Reset the cached platform detection
 * Useful for testing or when platform may change
 */
export function resetPlatformCache(): void {
  cachedIsAndroid = null
}
