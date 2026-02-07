// Copyright 2025 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

package app.tauri.plugin.permissions

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.result.ActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.app.ActivityCompat
import app.tauri.Logger
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class PermissionRequestOptions {
  var permissions: Array<String>? = null
}

@TauriPlugin
class PermissionsPlugin(private val activity: Activity): Plugin(activity) {
  private val permissionRequests = mutableMapOf<String, Array<String>>()

  @Command
  fun checkPermissions(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(PermissionRequestOptions::class.java)
      val permissions = args.permissions ?: emptyArray()

      val result = JSObject()
      val permissionsStatus = JSObject()

      for (permission in permissions) {
        val granted = ActivityCompat.checkSelfPermission(
          activity,
          permission
        ) == PackageManager.PERMISSION_GRANTED
        permissionsStatus.put(permission, granted)
      }

      result.put("permissions", permissionsStatus)
      invoke.resolve(result)
    } catch (ex: Exception) {
      val message = ex.message ?: "Failed to check permissions"
      Logger.error(message)
      invoke.reject(message)
    }
  }

  @Command
  fun requestPermissions(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(PermissionRequestOptions::class.java)
      val permissions = args.permissions ?: emptyArray()

      // Filter permissions that are not already granted
      val permissionsToRequest = permissions.filter { permission ->
        ActivityCompat.checkSelfPermission(
          activity,
          permission
        ) != PackageManager.PERMISSION_GRANTED
      }.toTypedArray()

      if (permissionsToRequest.isEmpty()) {
        // All permissions already granted
        val result = JSObject()
        val permissionsStatus = JSObject()
        for (permission in permissions) {
          permissionsStatus.put(permission, true)
        }
        result.put("permissions", permissionsStatus)
        invoke.resolve(result)
        return
      }

      // Store the invoke key for callback
      val invokeKey = "permission_request_${System.currentTimeMillis()}"
      permissionRequests[invokeKey] = permissionsToRequest

      // Request permissions
      val intent = android.content.Intent()
      intent.putExtra("permissions", permissionsToRequest)
      startActivityForResult(invoke, intent, "permissionResult")
    } catch (ex: Exception) {
      val message = ex.message ?: "Failed to request permissions"
      Logger.error(message)
      invoke.reject(message)
    }
  }

  @ActivityCallback
  fun permissionResult(invoke: Invoke, result: ActivityResult) {
    try {
      val resultIntent = result.data
      val permissions = resultIntent?.getStringArrayExtra("permissions") ?: emptyArray()

      val resultObj = JSObject()
      val permissionsStatus = JSObject()

      for (permission in permissions) {
        val granted = ActivityCompat.checkSelfPermission(
          activity,
          permission
        ) == PackageManager.PERMISSION_GRANTED
        permissionsStatus.put(permission, granted)
      }

      resultObj.put("permissions", permissionsStatus)
      invoke.resolve(resultObj)
    } catch (ex: Exception) {
      val message = ex.message ?: "Failed to process permission result"
      Logger.error(message)
      invoke.reject(message)
    }
  }

  @Command
  fun getStoragePermissionsToRequest(invoke: Invoke) {
    try {
      val permissions = mutableListOf<String>()

      // For Android 13+ (API 33+), use READ_MEDIA_AUDIO
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        if (ActivityCompat.checkSelfPermission(
            activity,
            Manifest.permission.READ_MEDIA_AUDIO
          ) != PackageManager.PERMISSION_GRANTED
        ) {
          permissions.add(Manifest.permission.READ_MEDIA_AUDIO)
        }
      } else {
        // For Android < 13, use READ_EXTERNAL_STORAGE
        if (ActivityCompat.checkSelfPermission(
            activity,
            Manifest.permission.READ_EXTERNAL_STORAGE
          ) != PackageManager.PERMISSION_GRANTED
        ) {
          permissions.add(Manifest.permission.READ_EXTERNAL_STORAGE)
        }
      }

      // For Android < 10 (API 29), also request WRITE_EXTERNAL_STORAGE
      if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
        if (ActivityCompat.checkSelfPermission(
            activity,
            Manifest.permission.WRITE_EXTERNAL_STORAGE
          ) != PackageManager.PERMISSION_GRANTED
        ) {
          permissions.add(Manifest.permission.WRITE_EXTERNAL_STORAGE)
        }
      }

      val result = JSObject()
      result.put("permissions", permissions.toTypedArray())
      invoke.resolve(result)
    } catch (ex: Exception) {
      val message = ex.message ?: "Failed to get storage permissions"
      Logger.error(message)
      invoke.reject(message)
    }
  }
}
