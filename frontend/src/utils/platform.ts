export type RuntimePlatform = "android" | "web";

// Related documentation:
// - `docs/runtime-platform-capabilities.md`

export interface RuntimeCapabilities {
  usesAndroidPlaybackBackend: boolean;
  supportsAndroidBackHandler: boolean;
  supportsForegroundMusicService: boolean;
  supportsExitAppOnTimer: boolean;
  supportsLocalLyricsPermission: boolean;
  supportsHeadsetMediaButtonControl: boolean;
  supportsRawContentPlayback: boolean;
}

export interface RuntimeProfile {
  platform: RuntimePlatform;
  capabilities: RuntimeCapabilities;
}

let cachedRuntimeProfile: RuntimeProfile | null = null;
let runtimeProfilePromise: Promise<RuntimeProfile> | null = null;

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

export function isCurrentOriginApiBase(apiBase: string): boolean {
  if (typeof window === "undefined") {
    return false;
  }

  try {
    return new URL(apiBase).origin === window.location.origin;
  } catch {
    return false;
  }
}

export function buildRuntimeCapabilities(
  platform: RuntimePlatform,
): RuntimeCapabilities {
  if (platform === "android") {
    return {
      usesAndroidPlaybackBackend: true,
      supportsAndroidBackHandler: true,
      supportsForegroundMusicService: true,
      supportsExitAppOnTimer: true,
      supportsLocalLyricsPermission: true,
      supportsHeadsetMediaButtonControl: true,
      supportsRawContentPlayback: true,
    };
  }

  return {
    usesAndroidPlaybackBackend: false,
    supportsAndroidBackHandler: false,
    supportsForegroundMusicService: false,
    supportsExitAppOnTimer: false,
    supportsLocalLyricsPermission: false,
    supportsHeadsetMediaButtonControl: false,
    supportsRawContentPlayback: false,
  };
}

export async function getRuntimeProfile(): Promise<RuntimeProfile> {
  if (cachedRuntimeProfile) {
    return cachedRuntimeProfile;
  }

  if (runtimeProfilePromise) {
    return runtimeProfilePromise;
  }

  runtimeProfilePromise = (async () => {
    let platform: RuntimePlatform = "web";

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const detected = await invoke<string>("get_platform");
      if (detected === "android") {
        platform = "android";
      }
    } catch {
      platform = "web";
    }

    const profile = {
      platform,
      capabilities: buildRuntimeCapabilities(platform),
    };
    cachedRuntimeProfile = profile;
    runtimeProfilePromise = null;
    return profile;
  })();

  return runtimeProfilePromise;
}

export async function getRuntimeCapabilities(): Promise<RuntimeCapabilities> {
  return (await getRuntimeProfile()).capabilities;
}

export async function shouldUseRawContentPlayback(
  apiBase: string,
): Promise<boolean> {
  const capabilities = await getRuntimeCapabilities();
  return capabilities.supportsRawContentPlayback && isLocalhostApiBase(apiBase);
}

/**
 * Check if the current platform is Android
 * @returns true if running on Android, false otherwise
 */
export async function checkIsAndroid(): Promise<boolean> {
  return (await getRuntimeProfile()).platform === "android";
}

/**
 * Reset the cached platform detection
 * Useful for testing or when platform may change
 */
export function resetPlatformCache(): void {
  cachedRuntimeProfile = null;
  runtimeProfilePromise = null;
}
