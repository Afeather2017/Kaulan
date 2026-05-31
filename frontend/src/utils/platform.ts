import { invoke } from "@tauri-apps/api/core";

let cachedIsAndroid: boolean | null = null;

/**
 * Check if the current platform is Android
 * @returns true if running on Android, false otherwise
 */
export async function checkIsAndroid(): Promise<boolean> {
  if (cachedIsAndroid !== null) {
    return cachedIsAndroid;
  }

  try {
    const platform = await invoke<string>("get_platform");
    cachedIsAndroid = platform === "android";
    return cachedIsAndroid;
  } catch {
    cachedIsAndroid = false;
    return cachedIsAndroid;
  }
}

/**
 * Reset the cached platform detection
 * Useful for testing or when platform may change
 */
export function resetPlatformCache(): void {
  cachedIsAndroid = null;
}
