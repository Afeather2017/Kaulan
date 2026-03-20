@echo off
REM Build and sign Kaulan Android APK
REM This script builds and signs the APK
REM
REM Usage:
REM   build-android.bat              (builds universal APK)
REM   build-android.bat --target aarch64
REM   build-android.bat --target armv7
REM   build-android.bat --target x86_64

setlocal enabledelayedexpansion

REM Configuration
set "ANDROID_HOME=%ANDROID_HOME:%=%LOCALAPPDATA%\Android\Sdk%"
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

echo === Kaulan Android Build Script ===
echo Project root: %PROJECT_ROOT%
echo Android build tools: %BUILD_TOOLS%
echo.

REM Step 1: Build unsigned APK
echo [1/4] Building APK with Tauri...
cd /d "%FRONTEND_DIR%"
if "%~1"=="" (
    echo   No target specified, building universal APK...
    call npx tauri android build
) else (
    echo   Building with custom options: %*
    call npx tauri android build %*
)
if errorlevel 1 exit /b 1

REM Detect the unsigned APK
echo.
echo [2/4] Locating built APK...
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
echo To install, run:
echo   adb install "%SIGNED_APK%"
echo.

REM Optionally install immediately
set /p INSTALL="Install to device now? [y/N] "
if /i "%INSTALL%"=="y" (
    adb install "%SIGNED_APK%"
    if errorlevel 1 (
        echo Installation failed!
        exit /b 1
    )
    echo Installation complete!
)

endlocal
