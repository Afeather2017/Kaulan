/**
 * Permission handling composable for Android file access.
 *
 * This composable handles Android storage permissions for music file access.
 * It only works in the Tauri Android environment - on web it's a no-op.
 *
 * References:
 * - Android permissions plugin: frontend/src-tauri/plugins/android-permissions/
 * - MainActivityInjection.kt: frontend/src-tauri/injections/android/MainActivityInjection.kt
 */

import { ref } from 'vue'

export interface PermissionStatus {
  permissions: Record<string, boolean>
}

export interface PermissionsResponse {
  permissions: Record<string, boolean>
}

// Check if we're running in Tauri environment
const isTauri = () => {
  return typeof window !== 'undefined' && '__TAURI__' in window
}

// Check if we're running on Android
const isAndroid = () => {
  if (!isTauri()) {
    return false
  }
  try {
    return (window as any).__TAURI_INTERNALS__?.platform === 'android'
  } catch {
    return false
  }
}

/**
 * Composable for handling Android permissions
 */
export function usePermissions() {
  const hasPermission = ref(false)
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  /**
   * Check current permission status
   */
  const checkPermissions = async (): Promise<PermissionStatus | null> => {
    if (!isAndroid()) {
      // Web or other platform - assume we have "permission"
      hasPermission.value = true
      return null
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core')

      // First get the storage permissions we should check
      const storagePerms = await invoke<string[]>('plugin:permissions|get_storage_permissions_to_request')

      if (storagePerms.length === 0) {
        // No permissions needed - already granted
        hasPermission.value = true
        return { permissions: {} }
      }

      // Check the permissions
      const result = await invoke<PermissionsResponse>(
        'plugin:permissions|check_permissions',
        { permissions: storagePerms }
      )

      // Check if all permissions are granted
      const allGranted = Object.values(result.permissions).every(v => v === true)
      hasPermission.value = allGranted

      return { permissions: result.permissions }
    } catch (e) {
      console.error('Failed to check permissions:', e)
      return null
    }
  }

  /**
   * Request storage permissions from the user
   * Only needed on Android - web doesn't need file permissions
   */
  const requestPermissions = async (): Promise<boolean> => {
    if (!isAndroid()) {
      // Web or other platform - no permissions needed
      hasPermission.value = true
      return true
    }

    isLoading.value = true
    error.value = null

    try {
      const { invoke } = await import('@tauri-apps/api/core')

      // First get the storage permissions we should request
      const storagePerms = await invoke<string[]>('plugin:permissions|get_storage_permissions_to_request')

      if (storagePerms.length === 0) {
        // No permissions needed - already granted
        hasPermission.value = true
        return true
      }

      // Request the permissions
      const result = await invoke<PermissionsResponse>(
        'plugin:permissions|request_permissions',
        { permissions: storagePerms }
      )

      // Check if all permissions are granted
      const allGranted = Object.values(result.permissions).every(v => v === true)
      hasPermission.value = allGranted

      if (!allGranted) {
        error.value = 'Storage permission is required to access music files'
      }

      return allGranted
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      error.value = errorMsg
      console.error('Failed to request permissions:', errorMsg)
      return false
    } finally {
      isLoading.value = false
    }
  }

  return {
    hasPermission,
    isLoading,
    error,
    checkPermissions,
    requestPermissions
  }
}
