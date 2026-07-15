export type MediaType = "audio" | "video";
export type OnlineMusicSource = "youtube" | "netease" | "bilibili";

export interface BackendMusicInfo {
  id: number;
  name: string;
  lufs: number | null;
  path: string;
  stream_url?: string | null;
}

export interface MusicInfo extends BackendMusicInfo {
  cover_url?: string | null;
  lyrics_url?: string | null;
  source_key?: string | null;
  sourceLabel?: string;
  rowKey?: string;
  mediaType?: MediaType;
  source?: OnlineMusicSource;
  is_temporary?: boolean;
}

export interface Playlist {
  name: string;
  songs: MusicInfo[];
}

export interface MusicResponse {
  id: number;
  filename: string;
  file_path: string;
  lufs: number | null;
  created_at: string;
}
