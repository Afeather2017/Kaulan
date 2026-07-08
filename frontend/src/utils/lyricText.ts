// Related documentation: `docs/lyric-editing.md`

/**
 * Non-empty and changed check for the raw lyric text editor.
 *
 * @param draft - Current textarea content
 * @param original - The sidecar content the modal opened with, or null
 */
export function canSubmitLyricText(
  draft: string,
  original: string | null,
): boolean {
  if (draft.trim().length === 0) {
    return false;
  }
  return draft !== (original ?? "");
}

const READ_ONLY_FALLBACK =
  "This source is read-only — lyrics can't be saved from this device.";

/**
 * Map a failed `PUT /api/lyrics/id/{id}` response to a user-facing message.
 * 409 gets a read-only-specific fallback because the backend omits a useful
 * message for that case.
 *
 * @param status - HTTP status code
 * @param payload - Parsed JSON body, if the response had one
 */
export function extractLyricPutError(
  status: number,
  payload: { message?: string } | null,
): string {
  if (status === 409) {
    return payload?.message ?? READ_ONLY_FALLBACK;
  }
  return payload?.message || `Failed to save lyrics (${status})`;
}

const LRC_TIMESTAMP_REGEX = /\[(\d{2}):(\d{2})\.(\d{2,3})\]/;

/**
 * Best-effort check that the edited content still has at least one LRC
 * timestamp. Used to surface a non-blocking warning — empty result does not
 * block saving because the user owns the format.
 *
 * @param content - Raw textarea content
 */
export function hasLrcTimestamps(content: string): boolean {
  return LRC_TIMESTAMP_REGEX.test(content);
}
