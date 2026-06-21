import {
  clearSessionLocalApiBaseOverride,
  resolveSourceApiBase,
  setSessionLocalApiBaseOverride,
} from "@/utils/api";
import type { MusicInfo } from "@/types/music";

// Related documentation: `docs/shared-song-links.md`

export type SharedLinkError = "invalid_id";

export interface SharedLinkIntent {
  apiBase: string;
  hasShareIntent: boolean;
  songId: number | null;
  error?: SharedLinkError;
}

export function parseSharedLinkIntent(
  locationLike: Pick<Location, "origin" | "search">,
): SharedLinkIntent {
  const apiBase = `${locationLike.origin}/api`;
  const params = new URLSearchParams(locationLike.search);
  const rawId = params.get("id");

  if (rawId === null) {
    return {
      apiBase,
      hasShareIntent: false,
      songId: null,
    };
  }

  const trimmedId = rawId.trim();
  if (!/^\d+$/.test(trimmedId)) {
    return {
      apiBase,
      hasShareIntent: true,
      songId: null,
      error: "invalid_id",
    };
  }

  return {
    apiBase,
    hasShareIntent: true,
    songId: Number.parseInt(trimmedId, 10),
  };
}

export function applySharedLinkApiBase(intent: SharedLinkIntent): void {
  if (typeof window === "undefined") {
    return;
  }

  if (intent.hasShareIntent) {
    setSessionLocalApiBaseOverride(intent.apiBase);
    return;
  }

  clearSessionLocalApiBaseOverride();
}

export function consumeSharedLinkQuery(): void {
  if (typeof window === "undefined") {
    return;
  }

  const url = new URL(window.location.href);
  if (!url.searchParams.has("id")) {
    return;
  }

  url.searchParams.delete("id");
  const nextSearch = url.searchParams.toString();
  const nextUrl = `${url.pathname}${nextSearch ? `?${nextSearch}` : ""}${url.hash}`;
  window.history.replaceState({}, "", nextUrl);
}

export function buildSharedSongUrl(
  song: Pick<MusicInfo, "id" | "source_key" | "is_temporary">,
): string {
  if (song.is_temporary) {
    return "";
  }

  const apiBase = resolveSourceApiBase(song.source_key);
  const url = new URL(apiBase);
  url.pathname = url.pathname.replace(/\/api\/?$/, "/") || "/";
  url.search = "";
  url.hash = "";
  url.searchParams.set("id", String(song.id));
  return url.toString();
}
