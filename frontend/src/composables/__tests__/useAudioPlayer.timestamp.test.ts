//! Unit tests for timestamp-based music streaming functionality in useAudioPlayer
//!
//! Tests the current URL-building and seek behavior.

import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@/utils/api", () => ({
  getLocalApiBase: () => "http://localhost:2080/api",
  resolveSourceApiBase: (sourceKey?: string | null) =>
    sourceKey || "http://localhost:2080/api",
}));

describe("useAudioPlayer - Timestamp Seek Functionality", () => {
  let useAudioPlayer: any;

  beforeEach(async () => {
    const storage = new Map<string, string>();
    Object.defineProperty(globalThis, "localStorage", {
      value: {
        getItem: vi.fn((key: string) => storage.get(key) ?? null),
        setItem: vi.fn((key: string, value: string) => {
          storage.set(key, value);
        }),
        removeItem: vi.fn((key: string) => {
          storage.delete(key);
        }),
      },
      configurable: true,
    });
    const module = await import("../useAudioPlayer");
    useAudioPlayer = module.useAudioPlayer;
  });

  describe("buildAudioUrl behavior (tested through playSong)", () => {
    it("should create URL with position parameter when duration is known", async () => {
      const mockSongs = [
        { id: 1, name: "Test Song", lufs: -12, path: "/path/to/song.mp3" },
      ];

      const { playSong, duration } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn(),
      });

      const mockAudio = {
        src: "",
        preload: "",
        addEventListener: vi.fn(),
        play: vi.fn(() => Promise.resolve()),
        pause: vi.fn(),
      };
      global.Audio = vi.fn(() => mockAudio) as any;

      duration.value = 180;
      await playSong(mockSongs[0], 45);

      expect(mockAudio.src).toContain("/music/id/1");
      expect(mockAudio.src).toContain("position=0.25");
      expect(mockAudio.src).not.toContain("t=");
    });

    it("should apply seekTime after metadata loads when duration was unknown at startup", async () => {
      const mockSongs = [
        { id: 1, name: "Test Song", lufs: -12, path: "/path/to/song.mp3" },
      ];

      const { playSong, currentTime, duration } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn(),
      });

      const listeners = new Map<string, () => void>();
      const mockAudio = {
        src: "",
        preload: "",
        duration: 180,
        currentTime: 0,
        paused: true,
        addEventListener: vi.fn((event: string, callback: () => void) => {
          listeners.set(event, callback);
        }),
        play: vi.fn(() => Promise.resolve()),
        pause: vi.fn(),
      };
      global.Audio = vi.fn(() => mockAudio) as any;

      await playSong(mockSongs[0], 45);

      expect(mockAudio.src).toContain("/music/id/1");
      expect(mockAudio.src).not.toContain("position=");

      listeners.get("loadedmetadata")?.();

      expect(duration.value).toBe(180);
      expect(mockAudio.currentTime).toBe(45);
      expect(currentTime.value).toBe(45);
    });

    it("should create URL without position when resume timestamp is not provided", async () => {
      const mockSongs = [
        { id: 1, name: "Test Song", lufs: -12, path: "/path/to/song.mp3" },
      ];

      const { playSong } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn(),
      });

      const mockAudio = {
        src: "",
        preload: "",
        addEventListener: vi.fn(),
        play: vi.fn(() => Promise.resolve()),
        pause: vi.fn(),
      };
      global.Audio = vi.fn(() => mockAudio) as any;

      await playSong(mockSongs[0]);

      expect(mockAudio.src).not.toContain("position=");
      expect(mockAudio.src).not.toContain("t=");
    });
  });

  describe("seekToTime behavior", () => {
    it("should update currentTime for small jumps", () => {
      const mockSongs = [
        { id: 1, name: "Test Song", lufs: -12, path: "/path/to/song.mp3" },
      ];

      const {
        seekToTime,
        audioElement,
        currentTime,
        duration,
        isPlaying,
        currentSong,
      } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn(),
      });

      const mockAudio = {
        currentTime: 30,
        pause: vi.fn(),
        play: vi.fn(() => Promise.resolve()),
      };
      audioElement.value = mockAudio as any;
      currentTime.value = 30;
      duration.value = 180;
      isPlaying.value = false;
      currentSong.value = mockSongs[0];

      void seekToTime(35);

      expect(mockAudio.currentTime).toBe(35);
      expect(currentTime.value).toBe(35);
    });

    it("should update currentTime for large jumps while playing", () => {
      const mockSongs = [
        { id: 1, name: "Test Song", lufs: -12, path: "/path/to/song.mp3" },
      ];

      const onSongStart = vi.fn();
      const {
        seekToTime,
        audioElement,
        currentTime,
        duration,
        isPlaying,
        currentSong,
      } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart,
      });

      const mockAudio = {
        currentTime: 10,
        pause: vi.fn(),
        play: vi.fn(() => Promise.resolve()),
      };
      audioElement.value = mockAudio as any;
      currentTime.value = 10;
      duration.value = 180;
      isPlaying.value = true;
      currentSong.value = mockSongs[0];

      void seekToTime(120);

      expect(mockAudio.currentTime).toBe(120);
      expect(currentTime.value).toBe(120);
      expect(onSongStart).not.toHaveBeenCalled();
    });

    it("should update currentTime for large jumps while paused", () => {
      const mockSongs = [
        { id: 1, name: "Test Song", lufs: -12, path: "/path/to/song.mp3" },
      ];

      const {
        seekToTime,
        audioElement,
        currentTime,
        duration,
        isPlaying,
        currentSong,
      } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn(),
      });

      const mockAudio = {
        currentTime: 0,
        pause: vi.fn(),
        play: vi.fn(() => Promise.resolve()),
      };
      audioElement.value = mockAudio as any;
      currentTime.value = 0;
      duration.value = 180;
      isPlaying.value = false;
      currentSong.value = mockSongs[0];

      void seekToTime(90);

      expect(mockAudio.currentTime).toBe(90);
      expect(currentTime.value).toBe(90);
    });

    it("should allow seeking to a lyric timestamp before playback starts", async () => {
      const mockSongs = [
        { id: 1, name: "Test Song", lufs: -12, path: "/path/to/song.mp3" },
      ];

      const { playSong, currentTime, duration } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn(),
      });

      const listeners = new Map<string, () => void>();
      const mockAudio = {
        src: "",
        preload: "",
        duration: 180,
        currentTime: 0,
        paused: true,
        addEventListener: vi.fn((event: string, callback: () => void) => {
          listeners.set(event, callback);
        }),
        pause: vi.fn(),
        play: vi.fn(() => Promise.resolve()),
      };
      global.Audio = vi.fn(() => mockAudio) as any;

      await playSong(mockSongs[0], 48.5);
      listeners.get("loadedmetadata")?.();

      expect(mockAudio.currentTime).toBe(48.5);
      expect(currentTime.value).toBe(48.5);
      expect(duration.value).toBe(180);
      expect(mockAudio.play).toHaveBeenCalled();
    });

    it("should preserve playing state when seeking to a lyric timestamp", () => {
      const mockSongs = [
        { id: 1, name: "Test Song", lufs: -12, path: "/path/to/song.mp3" },
      ];

      const {
        seekToTime,
        audioElement,
        currentTime,
        duration,
        isPlaying,
        currentSong,
      } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn(),
      });

      const mockAudio = {
        currentTime: 12,
        pause: vi.fn(),
        play: vi.fn(() => Promise.resolve()),
      };
      audioElement.value = mockAudio as any;
      currentTime.value = 12;
      duration.value = 180;
      isPlaying.value = true;
      currentSong.value = mockSongs[0];

      void seekToTime(96.25);

      expect(mockAudio.currentTime).toBe(96.25);
      expect(currentTime.value).toBe(96.25);
      expect(mockAudio.play).not.toHaveBeenCalled();
      expect(mockAudio.pause).not.toHaveBeenCalled();
    });
  });

  describe("edge cases", () => {
    it("should not seek if audio element is not available", () => {
      const { seekToTime, audioElement, currentTime, duration } =
        useAudioPlayer({
          songs: () => [],
          onSongEnd: vi.fn(),
          onSongStart: vi.fn(),
        });

      audioElement.value = null;
      currentTime.value = 30;
      duration.value = 180;

      expect(() => seekToTime(60)).not.toThrow();
    });

    it("should not seek if duration is not known", () => {
      const mockSongs = [
        { id: 1, name: "Test Song", lufs: -12, path: "/path/to/song.mp3" },
      ];

      const { seekToTime, audioElement, currentTime, duration, currentSong } =
        useAudioPlayer({
          songs: () => mockSongs,
          onSongEnd: vi.fn(),
          onSongStart: vi.fn(),
        });

      const mockAudio = {
        currentTime: 30,
        pause: vi.fn(),
        play: vi.fn(() => Promise.resolve()),
      };
      audioElement.value = mockAudio as any;
      currentTime.value = 30;
      duration.value = 0;
      currentSong.value = mockSongs[0];

      void seekToTime(60);
      expect(mockAudio.currentTime).toBe(30);
      expect(currentTime.value).toBe(30);
    });

    it("should handle seeking to beginning", () => {
      const mockSongs = [
        { id: 1, name: "Test Song", lufs: -12, path: "/path/to/song.mp3" },
      ];

      const {
        seekToTime,
        audioElement,
        currentTime,
        duration,
        isPlaying,
        currentSong,
      } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn(),
      });

      const mockAudio = {
        currentTime: 60,
        pause: vi.fn(),
        play: vi.fn(() => Promise.resolve()),
      };
      audioElement.value = mockAudio as any;
      currentTime.value = 60;
      duration.value = 180;
      isPlaying.value = false;
      currentSong.value = mockSongs[0];

      void seekToTime(0);

      expect(mockAudio.currentTime).toBe(0);
      expect(currentTime.value).toBe(0);
    });
  });
});
