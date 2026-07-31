import { computed, ref, type Ref } from "vue";
import type { MusicInfo } from "@/composables/useAudioPlayer";
import { getLocalApiBase, isSessionLocalApiBase } from "@/utils/api";
import {
  refreshDiscoveredDevices,
  refreshStoredManualDevices,
  type DiscoveredDevice,
} from "@/utils/discovery";
import { loadItemsIncrementally, upsertSortedItem } from "@/utils/sourceGroups";
import {
  getDefaultOnlineSearchApiBase,
  getManualDevices,
  setDefaultOnlineSearchApiBase,
  setManualDevices,
} from "@/utils/storage";
import { isCurrentOriginApiBase, isLocalhostApiBase } from "@/utils/platform";
import type {
  LibrarySourceGroup,
  LibrarySourceGroupSummary,
  OnlineSearchSourceOption,
  OnlineProviderStatus,
  SourceCapabilities,
} from "@/types/library";

const SOURCE_REQUEST_TIMEOUT_MS = 3000;

const buildSongApiUrl = (apiBase: string, suffix: string): string => {
  return `${apiBase}${suffix}`;
};

export const buildSongRowKey = (song: {
  id: number;
  name: string;
  device_id?: string | null;
}): string => {
  return `${song.device_id || "local"}:${song.id}:${song.name}`;
};

export const inferMediaType = (song: {
  name: string;
  path: string;
}): "audio" | "video" => {
  const candidate = `${song.name} ${song.path}`.toLowerCase();
  const audioExtensions = [
    ".mp3",
    ".flac",
    ".wav",
    ".ogg",
    ".mka",
    ".m4a",
    ".aac",
    ".opus",
  ];
  return audioExtensions.some((extension) => candidate.includes(extension))
    ? "audio"
    : "video";
};

const normalizeSourceSong = (
  apiBase: string,
  deviceId: string,
  sourceLabel: string,
  song: MusicInfo,
): MusicInfo => ({
  ...song,
  device_id: song.device_id || deviceId,
  stream_url:
    song.stream_url || buildSongApiUrl(apiBase, `/music/id/${song.id}`),
  cover_url:
    song.cover_url || buildSongApiUrl(apiBase, `/music/id/${song.id}/cover`),
  source_key: apiBase,
  sourceLabel,
  rowKey: `${deviceId || "local"}:${song.id}:${song.name}`,
  mediaType: song.mediaType || inferMediaType(song),
});

interface UseLibrarySourcesOptions {
  supportsRawContentPlayback: Ref<boolean>;
}

export function useLibrarySources(options: UseLibrarySourcesOptions) {
  const { supportsRawContentPlayback } = options;

  const searchQuery = ref("");
  const sourceGroups = ref<LibrarySourceGroup[]>([]);
  const showFilterSheet = ref(false);
  const appliedSourceFilterKey = ref("all");
  const draftSourceFilterKey = ref("all");
  const appliedMediaTypes = ref<Array<"audio" | "video">>(["audio", "video"]);
  const draftMediaTypes = ref<Array<"audio" | "video">>(["audio", "video"]);
  const selectedLibrarySourceKey = ref<string | null>(null);
  const selectedLibraryPlaylistName = ref<string | null>(null);
  const onlineSearchApiBase = ref<string>(getDefaultOnlineSearchApiBase());

  let sourceRefreshToken = 0;
  let discoveryRefreshPromise: Promise<void> | null = null;
  const pendingRecoveryDeviceIds = new Set<string>();
  let hasLegacyRecoveryTarget = false;

  const filteredSourceGroups = computed<LibrarySourceGroup[]>(() =>
    sourceGroups.value
      .filter(
        (group) =>
          appliedSourceFilterKey.value === "all" ||
          group.sourceKey === appliedSourceFilterKey.value,
      )
      .map((group) => ({
        ...group,
        playlists: group.playlists
          .map((playlist) => ({
            ...playlist,
            songs: playlist.songs.filter((song) =>
              appliedMediaTypes.value.includes(song.mediaType || "audio"),
            ),
          }))
          .filter((playlist) => playlist.songs.length > 0 || !group.isOnline),
      })),
  );

  const allLibrarySongs = computed<MusicInfo[]>(() =>
    filteredSourceGroups.value.flatMap((group) =>
      group.playlists.flatMap((playlist) => playlist.songs),
    ),
  );

  const libraryGroupSummaries = computed<LibrarySourceGroupSummary[]>(() =>
    filteredSourceGroups.value.map((group) => ({
      sourceKey: group.sourceKey,
      name: group.name,
      isLoading: group.isLoading,
      isOnline: group.isOnline,
      errorMessage: group.errorMessage,
      playlists: group.playlists.map((playlist) => ({
        name: playlist.name,
        songCount: playlist.songs.length,
      })),
    })),
  );

  const searchResults = computed(() => {
    const query = searchQuery.value.trim().toLowerCase();
    if (!query) {
      return [];
    }

    const seenRowKeys = new Set<string>();

    return allLibrarySongs.value.filter((song) => {
      if (!song.name.toLowerCase().includes(query)) {
        return false;
      }

      const rowKey = song.rowKey || buildSongRowKey(song);
      if (seenRowKeys.has(rowKey)) {
        return false;
      }

      seenRowKeys.add(rowKey);
      return true;
    });
  });

  const filterSources = computed(() =>
    sourceGroups.value.map((group) => ({
      sourceKey: group.sourceKey,
      name: group.name,
    })),
  );

  const onlineSearchSources = computed<OnlineSearchSourceOption[]>(() =>
    sourceGroups.value.map((group) => ({
      apiBase: group.apiBase,
      name: group.name,
      canUseForOnlineSearch: group.capabilities.canUseForOnlineSearch,
      isOnline: group.isOnline,
    })),
  );

  const trimmedSearchQuery = computed(() => searchQuery.value.trim());

  const buildSourceLabel = (apiBase: string): string => {
    const manualMatch = getManualDevices().find(
      (device) => device.api_url === apiBase,
    );
    if (manualMatch?.device_name?.trim()) {
      return manualMatch.device_name.trim();
    }

    try {
      const parsed = new URL(apiBase);
      return isSessionLocalApiBase(apiBase) ||
        parsed.hostname === "localhost" ||
        parsed.hostname === "127.0.0.1"
        ? "This Device"
        : parsed.hostname;
    } catch {
      return apiBase;
    }
  };

  const onlineSearchSourceName = computed(() => {
    const current = sourceGroups.value.find(
      (group) => group.apiBase === onlineSearchApiBase.value,
    );
    return current?.name || buildSourceLabel(onlineSearchApiBase.value);
  });

  const buildPlaylistRequestUrl = (apiBase: string): string => {
    const shouldRequestRawPlaybackPath =
      supportsRawContentPlayback.value &&
      (isLocalhostApiBase(apiBase) || isCurrentOriginApiBase(apiBase));
    return shouldRequestRawPlaybackPath
      ? buildSongApiUrl(apiBase, "/playlists?stream=content")
      : buildSongApiUrl(apiBase, "/playlists");
  };

  const getSourceApiBases = (): string[] => {
    const manual = getManualDevices().map((device) => device.api_url);
    return Array.from(new Set([getLocalApiBase(), ...manual]));
  };

  const sortSourceGroups = (
    groups: LibrarySourceGroup[],
  ): LibrarySourceGroup[] =>
    [...groups].sort((left, right) => {
      const leftIsLocal =
        isSessionLocalApiBase(left.apiBase) || isLocalhostApiBase(left.apiBase);
      const rightIsLocal =
        isSessionLocalApiBase(right.apiBase) ||
        isLocalhostApiBase(right.apiBase);

      if (leftIsLocal && !rightIsLocal) {
        return -1;
      }
      if (rightIsLocal && !leftIsLocal) {
        return 1;
      }
      if (left.isLoading && !right.isLoading) return -1;
      if (!left.isLoading && right.isLoading) return 1;
      return left.name.localeCompare(right.name);
    });

  const buildSourceCapabilities = (options: {
    apiBase: string;
    isOnline: boolean;
    canUpload: boolean;
    canChangeDirectory: boolean;
    canUseForOnlineSearch: boolean;
    canRetryConnection: boolean;
  }): SourceCapabilities => ({
    canRefresh: options.isOnline,
    canUpload: options.canUpload,
    canChangeDirectory: options.canChangeDirectory,
    canUseForOnlineSearch: options.canUseForOnlineSearch,
    isCurrentOnlineSearchSource: onlineSearchApiBase.value === options.apiBase,
    canRetryConnection: options.canRetryConnection,
    canShowSourceDetails: true,
    canDeleteSource:
      !isSessionLocalApiBase(options.apiBase) &&
      !isLocalhostApiBase(options.apiBase),
  });

  const buildLoadingSourceGroup = (apiBase: string): LibrarySourceGroup => ({
    sourceKey: apiBase,
    apiBase,
    device_id: "",
    name: buildSourceLabel(apiBase),
    isLoading: true,
    isOnline: false,
    errorMessage: undefined,
    playlists: [],
    onlineProviderStatuses: [],
    capabilities: buildSourceCapabilities({
      apiBase,
      isOnline: false,
      canUpload: false,
      canChangeDirectory: false,
      canUseForOnlineSearch: false,
      canRetryConnection: false,
    }),
  });

  const fetchWithTimeout = async (
    input: RequestInfo | URL,
    init?: RequestInit,
  ): Promise<Response> => {
    const controller = new AbortController();
    const timeoutId = window.setTimeout(() => {
      controller.abort();
    }, SOURCE_REQUEST_TIMEOUT_MS);

    try {
      return await fetch(input, {
        ...init,
        signal: controller.signal,
      });
    } finally {
      window.clearTimeout(timeoutId);
    }
  };

  const fetchSourceGroup = async (
    apiBase: string,
    recoverOnFailure = true,
  ): Promise<LibrarySourceGroup> => {
    const fallbackName = buildSourceLabel(apiBase);

    try {
      const [
        selfResponse,
        playlistsResponse,
        directoryTreeResponse,
        musicDirectoryResponse,
        onlineProvidersResponse,
      ] = await Promise.all([
        fetchWithTimeout(buildSongApiUrl(apiBase, "/discovery/self"), {
          cache: "no-store",
        }),
        fetchWithTimeout(buildPlaylistRequestUrl(apiBase), {
          cache: "no-store",
        }),
        fetchWithTimeout(buildSongApiUrl(apiBase, "/files/directory-tree"), {
          cache: "no-store",
        }).catch(() => null),
        fetchWithTimeout(
          buildSongApiUrl(apiBase, "/settings/music-directory"),
          {
            cache: "no-store",
          },
        ).catch(() => null),
        fetchWithTimeout(buildSongApiUrl(apiBase, "/download/providers"), {
          cache: "no-store",
        }).catch(() => null),
      ]);

      if (!selfResponse.ok || !playlistsResponse.ok) {
        throw new Error("source unavailable");
      }

      const selfData = await selfResponse.json();
      const playlistMap = (await playlistsResponse.json()) as Record<
        string,
        MusicInfo[]
      >;
      const deviceId =
        typeof selfData.device_id === "string" ? selfData.device_id : "";
      const sourceLabel = selfData.device_name || fallbackName;
      const canUpload = !!directoryTreeResponse?.ok;
      const canChangeDirectory = !!musicDirectoryResponse?.ok;
      const onlineProviderStatuses = onlineProvidersResponse?.ok
        ? ((await onlineProvidersResponse.json()) as OnlineProviderStatus[])
        : [];
      const canUseForOnlineSearch = onlineProviderStatuses.some(
        (provider) => provider.enabled,
      );

      const playlists = Object.entries(playlistMap).map(([name, songs]) => ({
        name,
        songs: songs.map((song) =>
          normalizeSourceSong(apiBase, deviceId, sourceLabel, song),
        ),
      }));

      return {
        sourceKey: apiBase,
        apiBase,
        device_id: deviceId,
        name: sourceLabel,
        isLoading: false,
        isOnline: true,
        errorMessage: undefined,
        playlists,
        onlineProviderStatuses,
        capabilities: buildSourceCapabilities({
          apiBase,
          isOnline: true,
          canUpload,
          canChangeDirectory,
          canUseForOnlineSearch,
          canRetryConnection: false,
        }),
      };
    } catch (error) {
      console.warn("Failed to load source group:", apiBase, error);
      if (
        recoverOnFailure &&
        getManualDevices().some((device) => device.api_url === apiBase)
      ) {
        const failedDevice = getManualDevices().find(
          (device) => device.api_url === apiBase,
        );
        if (failedDevice?.device_id) {
          pendingRecoveryDeviceIds.add(failedDevice.device_id);
        } else {
          hasLegacyRecoveryTarget = true;
        }
        void recoverUnreachableSources();
      }
      return {
        sourceKey: apiBase,
        apiBase,
        device_id: "",
        name: fallbackName,
        isLoading: false,
        isOnline: false,
        errorMessage: "Current source is unreachable.",
        playlists: [],
        onlineProviderStatuses: [],
        capabilities: buildSourceCapabilities({
          apiBase,
          isOnline: false,
          canUpload: false,
          canChangeDirectory: false,
          canUseForOnlineSearch: false,
          canRetryConnection: true,
        }),
      };
    }
  };

  const syncSourceGroupCapabilities = () => {
    sourceGroups.value = sourceGroups.value.map((group) => ({
      ...group,
      capabilities: buildSourceCapabilities({
        apiBase: group.apiBase,
        isOnline: group.isOnline,
        canUpload: group.capabilities.canUpload,
        canChangeDirectory: group.capabilities.canChangeDirectory,
        canUseForOnlineSearch: group.capabilities.canUseForOnlineSearch,
        canRetryConnection: group.capabilities.canRetryConnection,
      }),
    }));
  };

  const setOnlineSearchSource = (apiBase: string) => {
    onlineSearchApiBase.value = apiBase;
    setDefaultOnlineSearchApiBase(apiBase);
    syncSourceGroupCapabilities();
  };

  const resetOnlineSearchSourceToLocal = () => {
    setOnlineSearchSource(getLocalApiBase());
  };

  const ensureOnlineSearchSourceExists = () => {
    const knownApiBases = new Set([
      ...getSourceApiBases(),
      ...sourceGroups.value.map((group) => group.apiBase),
    ]);

    if (!knownApiBases.has(onlineSearchApiBase.value)) {
      resetOnlineSearchSourceToLocal();
      return;
    }

    syncSourceGroupCapabilities();
  };

  const refreshSourceGroups = async () => {
    const apiBases = getSourceApiBases();
    const refreshToken = sourceRefreshToken + 1;
    sourceRefreshToken = refreshToken;

    await loadItemsIncrementally({
      keys: apiBases,
      buildLoadingItem: buildLoadingSourceGroup,
      fetchItem: fetchSourceGroup,
      getItemKey: (group) => group.sourceKey,
      sortItems: sortSourceGroups,
      isActive: () => sourceRefreshToken === refreshToken,
      onUpdate: (groups) => {
        sourceGroups.value = groups;
        ensureOnlineSearchSourceExists();
      },
    });
  };

  const refreshSingleSource = async (
    apiBase: string,
    recoverOnFailure = true,
  ) => {
    sourceGroups.value = upsertSortedItem(
      sourceGroups.value,
      buildLoadingSourceGroup(apiBase),
      (group) => group.sourceKey,
      sortSourceGroups,
    );

    const updated = await fetchSourceGroup(apiBase, recoverOnFailure);
    sourceGroups.value = upsertSortedItem(
      sourceGroups.value,
      updated,
      (group) => group.sourceKey,
      sortSourceGroups,
    );
    ensureOnlineSearchSourceExists();
  };

  const applyDiscoveredRecoveryTargets = async (
    discoveredDevices: DiscoveredDevice[],
  ) => {
    const manualDevices = getManualDevices();
    let changed = false;
    const refreshes: Promise<void>[] = [];

    for (const discovered of discoveredDevices) {
      if (!pendingRecoveryDeviceIds.has(discovered.device_id)) {
        continue;
      }
      const index = manualDevices.findIndex(
        (device) => device.device_id === discovered.device_id,
      );
      if (index < 0) {
        pendingRecoveryDeviceIds.delete(discovered.device_id);
        continue;
      }

      const previousApiBase = manualDevices[index].api_url;
      manualDevices[index] = {
        ...manualDevices[index],
        api_url: discovered.api_url,
        device_name: discovered.device_name,
        last_fetched: Date.now(),
      };
      changed = true;
      pendingRecoveryDeviceIds.delete(discovered.device_id);

      if (previousApiBase !== discovered.api_url) {
        sourceGroups.value = sourceGroups.value.filter(
          (group) => group.sourceKey !== previousApiBase,
        );
        if (selectedLibrarySourceKey.value === previousApiBase) {
          selectedLibrarySourceKey.value = discovered.api_url;
        }
        if (onlineSearchApiBase.value === previousApiBase) {
          setOnlineSearchSource(discovered.api_url);
        }
      }
      refreshes.push(refreshSingleSource(discovered.api_url, false));
    }

    if (changed) {
      setManualDevices(manualDevices);
    }
    void Promise.all(refreshes);
  };

  const refreshDiscoveryState = async (recoveryWindowMs?: number) => {
    const previousManualDevices = getManualDevices();

    try {
      const discoveredDevices = await refreshDiscoveredDevices({
        windowMs: recoveryWindowMs,
        onUpdate: applyDiscoveredRecoveryTargets,
        shouldStop:
          recoveryWindowMs === undefined
            ? undefined
            : () =>
                pendingRecoveryDeviceIds.size === 0 && !hasLegacyRecoveryTarget,
      });
      const updatedManualDevices =
        await refreshStoredManualDevices(discoveredDevices);

      const previousByDeviceId = new Map(
        previousManualDevices
          .filter((device) => device.device_id)
          .map((device) => [device.device_id!, device.api_url]),
      );

      const previousByApiUrl = new Map(
        previousManualDevices.map((device) => [device.api_url, device.api_url]),
      );

      for (const device of updatedManualDevices) {
        const previousApiBase = device.device_id
          ? previousByDeviceId.get(device.device_id)
          : previousByApiUrl.get(device.api_url);

        if (!previousApiBase) {
          continue;
        }

        if (previousApiBase === device.api_url) {
          const currentGroup = sourceGroups.value.find(
            (group) => group.apiBase === device.api_url,
          );
          if (currentGroup && !currentGroup.isOnline) {
            await refreshSingleSource(device.api_url, false);
          }
          continue;
        }

        sourceGroups.value = sourceGroups.value.filter(
          (group) => group.sourceKey !== previousApiBase,
        );

        if (selectedLibrarySourceKey.value === previousApiBase) {
          selectedLibrarySourceKey.value = device.api_url;
        }

        if (onlineSearchApiBase.value === previousApiBase) {
          setOnlineSearchSource(device.api_url);
        }

        await refreshSingleSource(device.api_url, false);
      }
    } catch (error) {
      console.warn("[app] startup discovery refresh failed:", error);
    }
  };

  const recoverUnreachableSources = async (): Promise<void> => {
    if (discoveryRefreshPromise) {
      return await discoveryRefreshPromise;
    }

    discoveryRefreshPromise = refreshDiscoveryState(20_000).finally(() => {
      discoveryRefreshPromise = null;
      hasLegacyRecoveryTarget = false;
    });
    return await discoveryRefreshPromise;
  };

  const openFilterSheet = () => {
    draftSourceFilterKey.value = appliedSourceFilterKey.value;
    draftMediaTypes.value = [...appliedMediaTypes.value];
    showFilterSheet.value = true;
  };

  const toggleDraftMediaType = (
    mediaType: "audio" | "video",
    enabled: boolean,
  ) => {
    const next = new Set(draftMediaTypes.value);
    if (enabled) {
      next.add(mediaType);
    } else if (next.size > 1) {
      next.delete(mediaType);
    }
    draftMediaTypes.value = Array.from(next) as Array<"audio" | "video">;
  };

  const applyLibraryFilter = () => {
    appliedSourceFilterKey.value = draftSourceFilterKey.value;
    appliedMediaTypes.value =
      draftMediaTypes.value.length > 0 ? [...draftMediaTypes.value] : ["audio"];
    showFilterSheet.value = false;
  };

  const resetLibraryFilter = () => {
    draftSourceFilterKey.value = "all";
    draftMediaTypes.value = ["audio", "video"];
    appliedSourceFilterKey.value = "all";
    appliedMediaTypes.value = ["audio", "video"];
    showFilterSheet.value = false;
  };

  const getLibraryPlaylist = (
    sourceKey: string,
    playlistName: string,
  ): {
    source: LibrarySourceGroup;
    playlist: LibrarySourceGroup["playlists"][number];
  } | null => {
    const source = sourceGroups.value.find(
      (item) => item.sourceKey === sourceKey,
    );
    const playlist = source?.playlists.find(
      (item) => item.name === playlistName,
    );
    if (!source || !playlist) {
      return null;
    }

    return { source, playlist };
  };

  const syncSelectedLibraryPlaylist = (
    currentView: string,
    selectedPlaylist: Ref<{ name: string; songs: MusicInfo[] } | null>,
  ) => {
    if (
      currentView !== "songs" ||
      !selectedLibrarySourceKey.value ||
      !selectedLibraryPlaylistName.value
    ) {
      return;
    }

    const resolved = getLibraryPlaylist(
      selectedLibrarySourceKey.value,
      selectedLibraryPlaylistName.value,
    );
    if (!resolved) {
      return;
    }

    selectedPlaylist.value = {
      name: `曲库 / ${resolved.playlist.name} [${resolved.source.name}]`,
      songs: resolved.playlist.songs,
    };
  };

  const retrySourceConnection = async (apiBase: string) => {
    await refreshSingleSource(apiBase);
  };

  const showSourceDetails = (group: LibrarySourceGroup) => {
    const lines = [
      `Name: ${group.name}`,
      `API: ${group.apiBase}`,
      `Status: ${group.isOnline ? "Online" : "Offline"}`,
      `Playlists: ${group.playlists.length}`,
      `Online search: ${group.capabilities.canUseForOnlineSearch ? "Ready" : "Unavailable"}`,
    ];
    alert(lines.join("\n"));
  };

  const setOnlineSearchSourceFromMenu = (group: LibrarySourceGroup) => {
    if (!group.capabilities.canUseForOnlineSearch) {
      alert(`来源 “${group.name}” 当前无法用于在线搜索`);
      return;
    }

    setOnlineSearchSource(group.apiBase);
  };

  const deleteSource = (group: LibrarySourceGroup): boolean => {
    if (
      isSessionLocalApiBase(group.apiBase) ||
      isLocalhostApiBase(group.apiBase)
    ) {
      alert("本机来源不能删除");
      return false;
    }

    const isCurrentOnlineSearchSource =
      onlineSearchApiBase.value === group.apiBase;
    const confirmed = window.confirm(
      isCurrentOnlineSearchSource
        ? `删除来源 “${group.name}” 吗？\n\n它当前用于在线搜索。删除后会自动切换回当前本机来源。`
        : `删除来源 “${group.name}” 吗？`,
    );
    if (!confirmed) {
      return false;
    }

    setManualDevices(
      getManualDevices().filter((device) => device.api_url !== group.apiBase),
    );

    sourceGroups.value = sourceGroups.value.filter(
      (item) => item.sourceKey !== group.sourceKey,
    );

    if (isCurrentOnlineSearchSource) {
      resetOnlineSearchSourceToLocal();
    } else {
      ensureOnlineSearchSourceExists();
    }

    return true;
  };

  const updateSourceDatabase = async (group: LibrarySourceGroup) => {
    const response = await fetch(`${group.apiBase}/database/update`, {
      method: "POST",
    });
    const result = await response.json();
    if (!response.ok || !result.success) {
      throw new Error(result.message || "更新失败");
    }
    await refreshSingleSource(group.apiBase);
  };

  const changeSourceDirectory = async (group: LibrarySourceGroup) => {
    const newPath = prompt("请输入新的音乐目录路径:");
    if (!newPath || !newPath.trim()) {
      return;
    }

    const response = await fetch(`${group.apiBase}/settings/music-directory`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ path: newPath.trim() }),
    });
    const result = await response.json();
    if (!response.ok || !result.success) {
      throw new Error(result.message || "更改目录失败");
    }
    await refreshSingleSource(group.apiBase);
  };

  const triggerDatabaseUpdate = async (isScanning: Ref<boolean>) => {
    try {
      isScanning.value = true;
      console.log("[app] onMounted: triggering startup database scan");
      const response = await fetch(
        `${getLocalApiBase()}/database/update?startup=true`,
        { method: "POST" },
      );
      if (!response.ok) {
        const errorText = await response.text();
        console.warn(
          "[app] onMounted: database update failed:",
          response.status,
          errorText,
        );
        return;
      }
      const result = await response.json();
      if (!result.success) {
        console.warn(
          "[app] onMounted: database update returned failure:",
          result.message,
        );
      } else {
        console.log("[app] onMounted: database update completed");
      }
    } catch (error) {
      console.error("[app] onMounted: database update error:", error);
    } finally {
      isScanning.value = false;
    }
  };

  return {
    searchQuery,
    sourceGroups,
    showFilterSheet,
    draftSourceFilterKey,
    draftMediaTypes,
    selectedLibrarySourceKey,
    selectedLibraryPlaylistName,
    onlineSearchApiBase,
    allLibrarySongs,
    libraryGroupSummaries,
    searchResults,
    filterSources,
    onlineSearchSources,
    trimmedSearchQuery,
    onlineSearchSourceName,
    buildSourceLabel,
    ensureOnlineSearchSourceExists,
    setOnlineSearchSource,
    resetOnlineSearchSourceToLocal,
    refreshSourceGroups,
    refreshSingleSource,
    refreshDiscoveryState,
    recoverUnreachableSources,
    openFilterSheet,
    toggleDraftMediaType,
    applyLibraryFilter,
    resetLibraryFilter,
    getLibraryPlaylist,
    syncSelectedLibraryPlaylist,
    retrySourceConnection,
    showSourceDetails,
    setOnlineSearchSourceFromMenu,
    deleteSource,
    updateSourceDatabase,
    changeSourceDirectory,
    triggerDatabaseUpdate,
  };
}
