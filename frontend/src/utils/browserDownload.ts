/**
 * Browser-side file download helpers.
 *
 * Used by the plain-browser runtime to save remote tracks directly to the
 * user's device. The bytes are fetched from the remote server (CORS is open)
 * and handed to the browser as a blob, so the hosting/local backend is never
 * involved. Tauri runtimes do not use this — they import via the local backend.
 *
 * Related documentation: `docs/library-import.md`
 *
 * @module utils/browserDownload
 */

const OBJECT_URL_REVOKE_DELAY_MS = 30_000;

/**
 * Save a blob to the user's device with the given filename.
 */
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  triggerDownload(url, filename);
  // Give the browser a moment to start the navigation before revoking.
  window.setTimeout(() => URL.revokeObjectURL(url), OBJECT_URL_REVOKE_DELAY_MS);
}

/**
 * Fetch `url` (cross-origin OK when the remote backend allows any origin) and
 * save the response body to the user's device. Throws on a non-OK response.
 */
export async function downloadFromUrl(
  url: string,
  filename: string,
): Promise<void> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`下载失败 (${response.status})`);
  }
  const blob = await response.blob();
  downloadBlob(blob, filename);
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
