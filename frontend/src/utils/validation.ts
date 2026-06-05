import { normalizeApiBase } from "./api";

/**
 * URL validation utilities
 *
 * @module utils/validation
 */

/**
 * Result of URL validation
 */
export interface UrlValidationResult {
  valid: boolean;
  error?: string;
}

/**
 * Validate a server URL
 * @param url - The URL to validate
 * @returns Validation result with error message if invalid
 */
export function validateServerUrl(url: string): UrlValidationResult {
  const trimmed = url.trim();
  // Empty string is valid (will use default)
  if (!trimmed) {
    return { valid: true };
  }

  try {
    normalizeApiBase(trimmed);
    return { valid: true };
  } catch (error) {
    return {
      valid: false,
      error: error instanceof Error ? error.message : "Invalid URL format",
    };
  }
}

/**
 * Validate a required server URL for manual device entry.
 * @param url - The URL to validate
 * @returns Validation result with error message if invalid
 */
export function validateRequiredServerUrl(url: string): UrlValidationResult {
  if (!url.trim()) {
    return {
      valid: false,
      error: "请输入设备地址",
    };
  }

  return validateServerUrl(url);
}
