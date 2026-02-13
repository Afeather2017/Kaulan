/**
 * URL validation utilities
 *
 * @module utils/validation
 */

/**
 * Result of URL validation
 */
export interface UrlValidationResult {
  valid: boolean
  error?: string
}

/**
 * Validate a server URL
 * @param url - The URL to validate
 * @returns Validation result with error message if invalid
 */
export function validateServerUrl(url: string): UrlValidationResult {
  const trimmed = url.trim()
  // Empty string is valid (will use default)
  if (!trimmed) {
    return { valid: true }
  }

  try {
    const urlObj = new URL(trimmed)
    if (urlObj.protocol !== 'http:' && urlObj.protocol !== 'https:') {
      return { valid: false, error: 'URL must use HTTP or HTTPS protocol' }
    }
    return { valid: true }
  } catch {
    return { valid: false, error: 'Invalid URL format' }
  }
}
