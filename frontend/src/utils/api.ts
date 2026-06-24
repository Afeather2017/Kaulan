/**
 * API configuration for Kaulan frontend
 *
 * In development, the Vite proxy forwards /api to the backend
 * In production (Tauri build), we need to use the full URL
 *
 * Multi-source routing helpers for local and remote API bases.
 *
 * @module utils/api
 */

export const LOCALHOST_API_BASE = "http://localhost:2080/api";

let sessionLocalApiBaseOverride: string | null = null;

function hasExplicitScheme(url: string): boolean {
  return /^[a-zA-Z][a-zA-Z\d+\-.]*:\/\//.test(url);
}

function appendApiPath(pathname: string): string {
  const trimmedPath = pathname.replace(/\/+$/, "");
  if (!trimmedPath || trimmedPath === "/") {
    return "/api";
  }
  if (trimmedPath.endsWith("/api")) {
    return trimmedPath;
  }
  return `${trimmedPath}/api`;
}

function isTauriWebview(): boolean {
  if (typeof window === "undefined") {
    return false;
  }

  return (
    typeof (window as typeof window & { __TAURI_INTERNALS__?: unknown })
      .__TAURI_INTERNALS__ !== "undefined"
  );
}

function getBrowserOriginApiBase(): string | null {
  if (typeof window === "undefined") {
    return null;
  }

  // Inside a Tauri webview (Android/iOS/native desktop) the page loads from a
  // virtual origin such as `http(s)://tauri.localhost`, not from the backend
  // HTTP server. The session override or LOCALHOST_API_BASE fallback must be
  // used in that case; otherwise every API call is misrouted to tauri.localhost.
  if (isTauriWebview()) {
    return null;
  }

  try {
    const url = new URL(window.location.origin);
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      return null;
    }
    url.pathname = "/api";
    url.search = "";
    url.hash = "";
    return url.toString().replace(/\/$/, "");
  } catch {
    return null;
  }
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
  const trimmed = input.trim();
  if (!trimmed) {
    return "";
  }

  const candidate = hasExplicitScheme(trimmed) ? trimmed : `http://${trimmed}`;
  const url = new URL(candidate);

  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error("URL must use HTTP or HTTPS protocol");
  }

  if (!url.hostname) {
    throw new Error("URL must include a host");
  }

  if (!hasExplicitScheme(trimmed) && !url.port) {
    url.port = "2080";
  }

  url.pathname = appendApiPath(url.pathname);
  url.search = "";
  url.hash = "";

  return url.toString().replace(/\/$/, "");
}

export function getLocalApiBase(): string {
  return (
    sessionLocalApiBaseOverride ||
    getBrowserOriginApiBase() ||
    LOCALHOST_API_BASE
  );
}

export function setSessionLocalApiBaseOverride(apiBase: string): void {
  sessionLocalApiBaseOverride = normalizeApiBase(apiBase);
}

export function clearSessionLocalApiBaseOverride(): void {
  sessionLocalApiBaseOverride = null;
}

export function isSessionLocalApiBase(apiBase: string): boolean {
  return apiBase === getLocalApiBase();
}

export function isAbsoluteHttpApiBase(
  value: string | null | undefined,
): boolean {
  if (!value) {
    return false;
  }

  try {
    const parsed = new URL(value);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}

export function resolveSourceApiBase(
  sourceKey: string | null | undefined,
): string {
  return isAbsoluteHttpApiBase(sourceKey) ? sourceKey! : getLocalApiBase();
}
