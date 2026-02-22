// Code injected into MainActivity - handles Android permissions
package afeather.kaulan

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.app.ActivityCompat

// Permission request result callback
private var permissionResultCallback: ((Map<String, Boolean>) -> Unit)? = null
private lateinit var permissionLauncher: ActivityResultLauncher<Array<String>>

// Initialize permission launcher in onCreate
fun initPermissionLauncher() {
    permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { permissions ->
        permissionResultCallback?.invoke(permissions)
        permissionResultCallback = null
    }
}

// Check if specific permissions are granted
fun checkPermissions(permissions: Array<String>): Map<String, Boolean> {
    val result = mutableMapOf<String, Boolean>()
    for (permission in permissions) {
        result[permission] = ActivityCompat.checkSelfPermission(
            this,
            permission
        ) == PackageManager.PERMISSION_GRANTED
    }
    return result
}

// Request permissions with callback
fun requestPermissions(permissions: Array<String>, callback: (Map<String, Boolean>) -> Unit) {
    permissionResultCallback = callback
    permissionLauncher.launch(permissions)
}

// Get the storage permissions needed for this Android version
// NOTE: We only need READ permissions - the app does NOT write to external storage.
// Database is stored in app's internal storage (no permission needed).
fun getStoragePermissions(): Array<String> {
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        // Android 13+ (API 33+): Use READ_MEDIA_AUDIO for audio files only
        arrayOf(Manifest.permission.READ_MEDIA_AUDIO)
    } else {
        // Android < 13: Use READ_EXTERNAL_STORAGE (read-only access is sufficient)
        arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE)
    }
}
