import type { MusicInfo } from "@/composables/useAudioPlayer";

export interface LibrarySourcePlaylistSummary {
  name: string;
  songCount: number;
}

export interface LibrarySourceGroupSummary {
  sourceKey: string;
  name: string;
  isLoading: boolean;
  isOnline: boolean;
  playlists: LibrarySourcePlaylistSummary[];
}

export interface LibraryPlaylistGroup {
  name: string;
  songs: MusicInfo[];
}

export interface SourceCapabilities {
  canRefresh: boolean;
  canUpload: boolean;
  canChangeDirectory: boolean;
  canUseForOnlineSearch: boolean;
  isCurrentOnlineSearchSource: boolean;
  canRetryConnection: boolean;
  canShowSourceDetails: boolean;
  canDeleteSource: boolean;
}

export interface OnlineProviderStatus {
  source: "youtube" | "netease" | "bilibili";
  enabled: boolean;
  summary: string;
}

export interface LibrarySourceGroup {
  apiBase: string;
  sourceKey: string;
  name: string;
  isLoading: boolean;
  isOnline: boolean;
  playlists: LibraryPlaylistGroup[];
  onlineProviderStatuses: OnlineProviderStatus[];
  capabilities: SourceCapabilities;
}
