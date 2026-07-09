/**
 * Tests for the raw lyric text editor helpers.
 *
 * @module utils/__tests__/lyricText.test
 *
 * Related documentation: `docs/lyric-editing.md`
 */

import { describe, it, expect } from "vitest";
import {
  canSubmitLyricText,
  extractLyricPutError,
  hasLrcTimestamps,
} from "../lyricText";

describe("canSubmitLyricText", () => {
  it("returns false for empty draft", () => {
    expect(canSubmitLyricText("", "[00:01.00] line")).toBe(false);
  });

  it("returns false for whitespace-only draft", () => {
    expect(canSubmitLyricText("   \n\t  ", "[00:01.00] line")).toBe(false);
  });

  it("returns false when draft equals original", () => {
    const original = "[00:01.00] same";
    expect(canSubmitLyricText(original, original)).toBe(false);
  });

  it("returns false when draft equals null original coerced to empty", () => {
    expect(canSubmitLyricText("", null)).toBe(false);
  });

  it("returns true when draft differs and is non-empty", () => {
    expect(canSubmitLyricText("[00:02.00] changed", "[00:01.00] line")).toBe(
      true,
    );
  });

  it("returns true when original is null and draft has content", () => {
    expect(canSubmitLyricText("[00:01.00] line", null)).toBe(true);
  });
});

describe("extractLyricPutError", () => {
  it("returns payload message on 409", () => {
    expect(extractLyricPutError(409, { message: "Source is read-only" })).toBe(
      "Source is read-only",
    );
  });

  it("returns read-only fallback on 409 without payload", () => {
    expect(extractLyricPutError(409, null)).toBe(
      "This source is read-only — lyrics can't be saved from this device.",
    );
  });

  it("returns payload message on 409 when payload has no message", () => {
    expect(extractLyricPutError(409, {})).toBe(
      "This source is read-only — lyrics can't be saved from this device.",
    );
  });

  it("returns payload message on 500", () => {
    expect(extractLyricPutError(500, { message: "Internal error" })).toBe(
      "Internal error",
    );
  });

  it("returns generic fallback on 500 without payload", () => {
    expect(extractLyricPutError(500, null)).toBe("Failed to save lyrics (500)");
  });

  it("returns generic fallback on 400 with empty payload", () => {
    expect(extractLyricPutError(400, {})).toBe("Failed to save lyrics (400)");
  });
});

describe("hasLrcTimestamps", () => {
  it("returns false for plain text", () => {
    expect(hasLrcTimestamps("just some words\nno timestamps here")).toBe(false);
  });

  it("returns false for empty content", () => {
    expect(hasLrcTimestamps("")).toBe(false);
  });

  it("returns false for metadata-only LRC", () => {
    expect(hasLrcTimestamps("[ti: Song]\n[ar: Artist]\n[al: Album]")).toBe(
      false,
    );
  });

  it("returns true for a single LRC timestamp", () => {
    expect(hasLrcTimestamps("[00:01.23] First line")).toBe(true);
  });

  it("returns true for a single LRC timestamp with 3-digit ms", () => {
    expect(hasLrcTimestamps("[01:12.345] Line")).toBe(true);
  });

  it("returns true for multiple LRC timestamps", () => {
    expect(
      hasLrcTimestamps("[00:01.23] First\n[00:05.67] Second\n[00:09.00] Third"),
    ).toBe(true);
  });

  it("returns true when timestamp is mid-line", () => {
    expect(hasLrcTimestamps("noise [00:01.23] noise")).toBe(true);
  });
});
