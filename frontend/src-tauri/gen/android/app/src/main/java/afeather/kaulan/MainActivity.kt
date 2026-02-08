package afeather.kaulan

import android.Manifest
import android.os.Bundle
import android.content.pm.PackageManager
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.app.ActivityCompat

class MainActivity : TauriActivity() {
  // Permission request launcher
  private val requestPermissionLauncher = registerForActivityResult(
    ActivityResultContracts.RequestMultiplePermissions()
  ) { permissions ->
    // Handle permission results
    permissions.entries.forEach { entry ->
      val permissionName = entry.key
      val isGranted = entry.value
      if (isGranted) {
        android.util.Log.d("MainActivity", "Permission granted: $permissionName")
      } else {
        android.util.Log.d("MainActivity", "Permission denied: $permissionName")
      }
    }
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)

    // Check and request storage permissions on startup
    checkAndRequestStoragePermissions()
  }

  private fun checkAndRequestStoragePermissions() {
    val permissionsToRequest = getRequiredPermissions().filter { permission ->
      ActivityCompat.checkSelfPermission(this, permission) != PackageManager.PERMISSION_GRANTED
    }

    if (permissionsToRequest.isNotEmpty()) {
      android.util.Log.d("MainActivity", "Requesting permissions: ${permissionsToRequest.joinToString()}")
      requestPermissionLauncher.launch(permissionsToRequest.toTypedArray())
    } else {
      android.util.Log.d("MainActivity", "All storage permissions already granted")
    }
  }

  private fun getRequiredPermissions(): Array<String> {
    return if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
      // Android 13+ (API 33+): Use READ_MEDIA_AUDIO
      arrayOf(Manifest.permission.READ_MEDIA_AUDIO)
    } else {
      // Android < 13: Use READ_EXTERNAL_STORAGE
      mutableListOf<String>().apply {
        add(Manifest.permission.READ_EXTERNAL_STORAGE)
        if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.Q) {
          add(Manifest.permission.WRITE_EXTERNAL_STORAGE)
        }
      }.toTypedArray()
    }
  }

  // Check if storage permissions are granted
  fun hasStoragePermission(): Boolean {
    val requiredPermissions = getRequiredPermissions()
    return requiredPermissions.all { permission ->
      ActivityCompat.checkSelfPermission(this, permission) == PackageManager.PERMISSION_GRANTED
    }
  }
}
