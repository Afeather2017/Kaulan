# Android Build

## Overview

Kaulan can be built for Android using Tauri's mobile support. The Android app wraps the web frontend in a WebView and communicates with a locally-running backend server.

## Development Build Configuration

### Cleartext Traffic for Local Development

By default, Android 9 (API 28) and later block cleartext (HTTP) traffic for security reasons. Since the Kaulan development setup uses a local backend server at `http://localhost:2080`, cleartext traffic must be explicitly allowed.

**Note:** This configuration is ONLY for development. Production builds should use HTTPS.

### Configuration Files

#### 1. Network Security Config

File: `frontend/src-tauri/gen/android/app/src/main/res/xml/network_security_config.xml`

```xml
<?xml version="1.0" encoding="utf-8"?>
<network-security-config>
    <!-- Allow cleartext traffic for localhost in debug builds -->
    <base-config cleartextTrafficPermitted="false">
        <trust-anchors>
            <certificates src="system" />
        </trust-anchors>
    </base-config>
    <domain-config cleartextTrafficPermitted="true">
        <domain includeSubdomains="true">localhost</domain>
        <domain includeSubdomains="true">127.0.0.1</domain>
        <domain includeSubdomains="true">10.0.2.2</domain>
    </domain-config>
</network-security-config>
```

**Domains explained:**
- `localhost` - Standard local loopback
- `127.0.0.1` - Direct IP loopback
- `10.0.2.2` - Special Android emulator IP that forwards to the host machine (useful when backend runs on host)

#### 2. Android Manifest

File: `frontend/src-tauri/gen/android/app/src/main/AndroidManifest.xml`

The manifest references the network security config:

```xml
<application
    android:icon="@mipmap/ic_launcher"
    android:label="@string/app_name"
    android:theme="@style/Theme.app"
    android:usesCleartextTraffic="${usesCleartextTraffic}"
    android:networkSecurityConfig="@xml/network_security_config">
    <!-- ... -->
</application>
```

#### 3. Gradle Build Configuration

File: `frontend/src-tauri/gen/android/app/build.gradle.kts`

The build system sets `usesCleartextTraffic` based on build type:

```kotlin
android {
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            // ...
        }
        getByName("release") {
            // usesCleartextTraffic stays false for release
        }
    }
}
```

This ensures:
- **Debug builds**: Cleartext traffic allowed via manifest placeholder
- **Release builds**: Cleartext traffic blocked by default

## Troubleshooting

### ERR_CLEARTEXT_NOT_PERMITTED

If you see this error in the WebView console:

**Symptoms:**
- DevTools shows `ERR_CLEARTEXT_NOT_PERMITTED` for `http://localhost:2080`
- API calls fail without clear error messages

**Solutions:**
1. Ensure `network_security_config.xml` exists in `res/xml/`
2. Verify `android:networkSecurityConfig="@xml/network_security_config"` is in AndroidManifest.xml
3. Check that debug build is being used (release builds block cleartext)
4. Rebuild the app after making changes

### Backend Not Accessible from Emulator

If the Android app can't reach the backend running on your host machine:

- Use `10.0.2.2` instead of `localhost` in your frontend API calls
- Ensure backend is running on the host machine
- Check that firewall isn't blocking the connection

## Related Source Files

- **`frontend/src-tauri/gen/android/app/src/main/AndroidManifest.xml`** - App manifest with network security config reference
- **`frontend/src-tauri/gen/android/app/src/main/res/xml/network_security_config.xml`** - Cleartext traffic allowances
- **`frontend/src-tauri/gen/android/app/build.gradle.kts`** - Build configuration with manifest placeholders

## Best Practices

### Security Considerations

1. **Never deploy release builds with cleartext traffic enabled** - The configuration uses build variants to prevent this
2. **Only use cleartext for local development** - All domains in `network_security_config.xml` are localhost addresses
3. **Use HTTPS in production** - When deploying to production, configure proper SSL certificates

### Development Workflow

1. Start backend: `cd backend && cargo run`
2. Build Android app: `cd frontend && npm run tauri android build`
3. For faster iteration during development: `npm run tauri android dev`
