let cachedIsAndroid: boolean | null = null;

export function isLoopbackHostname(hostname: string): boolean {
  return (
    hostname === "localhost" || hostname === "127.0.0.1" || hostname === "::1"
  );
}

export function isLocalhostApiBase(apiBase: string): boolean {
  try {
    return isLoopbackHostname(new URL(apiBase).hostname);
  } catch {
    return false;
  }
}

/**
 * Check if the current platform is Android
 * @returns true if running on Android, false otherwise
 */
export async function checkIsAndroid(): Promise<boolean> {
  if (cachedIsAndroid !== null) {
    return cachedIsAndroid;
  }

  try {
    const { invoke } = await import("@tauri-apps/api/core");
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
