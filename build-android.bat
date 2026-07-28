@echo off
REM Build and sign Kaulan Android APK
REM This script builds and signs the APK
REM
REM Usage:
REM   build-android.bat              (builds aarch64 APK — matches staged FFmpeg)
REM   build-android.bat --target aarch64
REM   build-android.bat --target armv7
REM   build-android.bat --target x86_64

setlocal enabledelayedexpansion

REM Configuration
if defined ANDROID_HOME (
    set "ANDROID_HOME=%ANDROID_HOME%"
) else (
    set "ANDROID_HOME=%LOCALAPPDATA%\Android\Sdk"
)
set "BUILD_TOOLS_VERSION=35.0.0"
set "BUILD_TOOLS=%ANDROID_HOME%\build-tools\%BUILD_TOOLS_VERSION%"
set "KEYSTORE=%TEMP%\release.keystore"
set "KEY_ALIAS=release"
set "KEY_STOREPASS=123456"
set "KEY_KEYPASS=123456"
set "JAVA_HOME=C:\Program Files\Android\Android Studio\jbr"
set "KEYTOOL=C:\Program Files\Android\Android Studio\jbr\bin\keytool.exe"

REM Project paths
set "PROJECT_ROOT=%~dp0"
set "FRONTEND_DIR=%PROJECT_ROOT%frontend"
set "SRC_TAURI=%FRONTEND_DIR%\src-tauri"
set "APK_DIR=%SRC_TAURI%\gen\android\app\build\outputs\apk"
set "ALIGNED_APK=%TEMP%\app-aligned.apk"
set "SIGNED_APK=%TEMP%\app-signed.apk"
set "ANDROID_FFMPEG_ROOT=%PROJECT_ROOT%build\android-ffmpeg\android"

REM FFmpeg target triple matching the staged bundle. Default aarch64 — this
REM script is for local debug builds on physical devices. Forward all CLI args
REM to tauri; we only sniff --target here so we can point rusty_ffmpeg at the
REM right prebuilt libs.
set "RUST_TARGET=aarch64-linux-android"
if /i "%~1"=="--target" (
    if /i "%~2"=="armv7"   set "RUST_TARGET=armv7-linux-androideabi"
    if /i "%~2"=="i686"    set "RUST_TARGET=i686-linux-android"
    if /i "%~2"=="x86_64"  set "RUST_TARGET=x86_64-linux-android"
)
if /i "%~1"=="-t" (
    if /i "%~2"=="armv7"   set "RUST_TARGET=armv7-linux-androideabi"
    if /i "%~2"=="i686"    set "RUST_TARGET=i686-linux-android"
    if /i "%~2"=="x86_64"  set "RUST_TARGET=x86_64-linux-android"
)

set "FFMPEG_LIBS_DIR=%ANDROID_FFMPEG_ROOT%\%RUST_TARGET%\lib"
set "FFMPEG_INCLUDE_DIR=%ANDROID_FFMPEG_ROOT%\%RUST_TARGET%\prefix\include"
set "FFMPEG_BINDING_PATH=%ANDROID_FFMPEG_ROOT%\binding.rs"
set "FFMPEG_LINK_MODE=dynamic"

echo === Kaulan Android Build Script ===
echo Project root: %PROJECT_ROOT%
echo Android build tools: %BUILD_TOOLS%
echo FFmpeg target: %RUST_TARGET%
echo.

REM Verify staged FFmpeg bundle exists so we fail early with a clear message
REM instead of a rusty_ffmpeg panic deep in the cargo build.
if not exist "%FFMPEG_LIBS_DIR%" (
    echo ERROR: Staged FFmpeg libs not found at %FFMPEG_LIBS_DIR%
    echo Run scripts\build-android-ffmpeg.sh --target aarch64 first.
    exit /b 1
)
if not exist "%FFMPEG_BINDING_PATH%" (
    echo ERROR: FFmpeg binding.rs not found at %FFMPEG_BINDING_PATH%
    echo Run scripts\build-android-ffmpeg.sh --target aarch64 first.
    exit /b 1
)

REM Derive release version from latest git tag (e.g. v1.1.0 -> 1.1.0).
REM Tauri's --config flag merges this over tauri.conf.json so the bundled
REM APK reports the tag's version without hand-editing source files.
REM See CLAUDE.md "Versioning" for the model.
set "RELEASE_VERSION="
set "LATEST_TAG="
for /f "delims=" %%i in ('git describe --tags --abbrev=0 2^>nul') do set "LATEST_TAG=%%i"
if not defined LATEST_TAG (
    echo No git tag found; using dev version from source files.
) else (
    set "TAG_VALUE=!LATEST_TAG:v=!"
    echo !TAG_VALUE! | findstr /R "^[0-9][0-9]*[.][0-9][0-9]*[.][0-9][0-9]*" >nul
    if !errorlevel! equ 0 (
        set "RELEASE_VERSION=!TAG_VALUE!"
        echo Injecting release version from git tag: !RELEASE_VERSION!
    ) else (
        echo Tag "!LATEST_TAG!" is not semver; skipping version override.
    )
)

REM Step 1: Build unsigned APK. Pass --config inline when a release version was
REM resolved; cmd.exe's `\"` escapes the inner quotes so the C runtime that
REM parses argv in npx/tauri sees valid JSON: {"version":"X.Y.Z"}.
echo [1/5] Building APK with Tauri...
cd /d "%FRONTEND_DIR%"
if "%~1"=="" (
    echo   No target specified, building aarch64 APK to match staged FFmpeg...
    if defined RELEASE_VERSION (
        call npx tauri android build --target aarch64 --config "{\"version\":\"!RELEASE_VERSION!\"}"
    ) else (
        call npx tauri android build --target aarch64
    )
) else (
    echo   Building with custom options: %*
    if defined RELEASE_VERSION (
        call npx tauri android build %* --config "{\"version\":\"!RELEASE_VERSION!\"}"
    ) else (
        call npx tauri android build %*
    )
)
if errorlevel 1 exit /b 1

REM Detect the unsigned APK
echo.
echo [2/5] Locating built APK...
set "UNSIGNED_APK="
for /r "%APK_DIR%" %%F in (*-release-unsigned.apk) do (
    set "UNSIGNED_APK=%%F"
    goto :found_apk
)
:found_apk
if "%UNSIGNED_APK%"=="" (
    echo ERROR: Could not find unsigned APK in %APK_DIR%
    exit /b 1
)
echo   Found: %UNSIGNED_APK%

REM Step 3: Generate keystore if needed
echo.
echo [3/5] Checking keystore...
if not exist "%KEYSTORE%" (
    echo   Generating new keystore...
    "%KEYTOOL%" -genkey -v -keystore "%KEYSTORE%" -alias "%KEY_ALIAS%" ^
        -keyalg RSA -keysize 2048 -validity 10000 ^
        -storepass "%KEY_STOREPASS%" -keypass "%KEY_KEYPASS%" ^
        -dname "CN=Kaulan,O=Kaulan,C=US"
    if errorlevel 1 exit /b 1
) else (
    echo   Using existing keystore: %KEYSTORE%
)

if exist "%ALIGNED_APK%" del "%ALIGNED_APK%"
if exist "%SIGNED_APK%" del "%SIGNED_APK%"

REM Step 4: Zipalign
echo.
echo [4/5] Zipaligning APK...
"%BUILD_TOOLS%\zipalign" -v -p 4 "%UNSIGNED_APK%" "%ALIGNED_APK%"
if errorlevel 1 exit /b 1

REM Step 5: Sign APK
echo.
echo [5/5] Signing APK...
"%BUILD_TOOLS%\apksigner" sign ^
    --ks "%KEYSTORE%" ^
    --ks-pass "pass:%KEY_STOREPASS%" ^
    --key-pass "pass:%KEY_KEYPASS%" ^
    --out "%SIGNED_APK%" ^
    "%ALIGNED_APK%"
if errorlevel 1 exit /b 1

REM Done
echo.
echo === Build Complete ===
echo Signed APK: %SIGNED_APK%
echo.

REM Install to device
echo [6/6] Installing to device...
adb install -r "%SIGNED_APK%"
if errorlevel 1 (
    echo Installation failed!
    exit /b 1
)
echo Installation complete!

endlocal
