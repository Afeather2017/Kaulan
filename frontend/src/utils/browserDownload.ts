/**
 * Browser-side file download helpers.
 *
 * Used by the plain-browser runtime to save remote tracks directly to the
 * user's device. Audio files are downloaded NATIVELY: the browser issues the
 * GET and streams the body straight to disk via its download manager (the
 * backend sends `Content-Disposition: attachment` when `?download=1` is set),
 * so large files are never buffered into page memory. Lyrics sidecars are tiny,
 * so they are still fetched as text and saved as a blob.
 *
 * Tauri runtimes do not use this — they import via the local backend.
 *
 * Related documentation: `docs/library-import.md`
 *
 * @module utils/browserDownload
 */

const OBJECT_URL_REVOKE_DELAY_MS = 30_000;

/**
 * Save a blob to the user's device with the given filename. Used for small,
 * already-in-memory payloads (e.g. lyrics text).
 */
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  triggerDownload(url, filename);
  // Give the browser a moment to start the navigation before revoking.
  window.setTimeout(() => URL.revokeObjectURL(url), OBJECT_URL_REVOKE_DELAY_MS);
}

/**
 * Trigger a NATIVE browser download of `url`. The browser's download manager
 * fetches and streams the response to disk directly — no `fetch`, no blob, no
 * page-memory buffering — which is why this is used for (potentially large)
 * audio files. The remote endpoint must send
 * `Content-Disposition: attachment; filename=...` (via `?download=1`) so the
 * browser saves rather than plays the cross-origin response; the `download`
 * attribute is intentionally omitted because browsers ignore it cross-origin.
 */
export function triggerAnchorDownload(url: string): void {
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.rel = "noopener";
  anchor.style.display = "none";
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
}

function triggerDownload(url: string, filename: string): void {
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.rel = "noopener";
  anchor.style.display = "none";
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
}
