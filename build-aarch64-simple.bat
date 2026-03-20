@echo off
setlocal enabledelayedexpansion

set "JAVA_HOME=C:\Program Files\Android\Android Studio\jbr"
set "KEYTOOL=C:\Program Files\Android\Android Studio\jbr\bin\keytool.exe"
set "BUILD_TOOLS=C:\Users\Administrator\AppData\Local\Android\Sdk\build-tools\35.0.0"
set "KEYSTORE=%TEMP%\release.keystore"
set "KEY_ALIAS=release"
set "KEY_STOREPASS=123456"
set "KEY_KEYPASS=123456"

set "PROJECT_ROOT=%~dp0"
set "FRONTEND_DIR=%PROJECT_ROOT%frontend"
set "SRC_TAURI=%FRONTEND_DIR%\src-tauri"
set "APK_DIR=%SRC_TAURI%\gen\android\app\build\outputs\apk"
set "WORK_DIR=%TEMP%\apk-work"
set "SIGNED_APK=%PROJECT_ROOT%Kaulan-aarch64.apk"

echo === Kaulan aarch64-only APK Build ===
echo This creates an APK with only arm64-v8a libraries
echo.

REM Step 1: Build universal APK
echo [1/5] Building universal APK...
cd /d "%FRONTEND_DIR%"
call npx tauri android build
if errorlevel 1 (
    echo Build failed!
    exit /b 1
)

REM Find the universal APK
echo.
echo [2/5] Locating universal APK...
set "UNSIGNED_APK=%APK_DIR%\universal\release\app-universal-release-unsigned.apk"
if not exist "%UNSIGNED_APK%" (
    echo ERROR: Could not find universal APK at %UNSIGNED_APK%
    exit /b 1
)
:found_apk
if "%UNSIGNED_APK%"=="" (
    echo ERROR: Could not find universal APK
    exit /b 1
)
echo Found: %UNSIGNED_APK%

REM Step 2: Extract and create aarch64-only APK
echo.
echo [3/5] Creating aarch64-only APK...
if exist "%WORK_DIR%" rd /s /q "%WORK_DIR%"
mkdir "%WORK_DIR%\extract"
mkdir "%WORK_DIR%\aarch64"

REM Extract APK (rename to .zip first for PowerShell)
echo Extracting APK...
copy "%UNSIGNED_APK%" "%WORK_DIR%\temp.zip" >nul
powershell -Command "Expand-Archive -Path '%WORK_DIR%\temp.zip' -DestinationPath '%WORK_DIR%\extract' -Force"
if errorlevel 1 (
    echo Extraction failed!
    exit /b 1
)

REM Copy only arm64-v8a files
echo Copying arm64-v8a libraries...
xcopy /q /y "%WORK_DIR%\extract\*" "%WORK_DIR%\aarch64\" /e /i >nul
rd /s /q "%WORK_DIR%\aarch64%\lib\armeabi-v7a"
rd /s /q "%WORK_DIR%\aarch64%\lib\x86"
rd /s /q "%WORK_DIR%\aarch64%\lib\x86_64"

REM Repackage APK
echo Repackaging APK...
powershell -Command "Compress-Archive -Path '%WORK_DIR%\aarch64\*' -DestinationPath '%WORK_DIR%\app-aarch64-unsigned.zip' -Force"
move /y "%WORK_DIR%\app-aarch64-unsigned.zip" "%WORK_DIR%\app-aarch64-unsigned.apk" >nul
if errorlevel 1 (
    echo Repackaging failed!
    exit /b 1
)

REM Step 4: Zipalign
echo.
echo [4/5] Zipaligning...
"%BUILD_TOOLS%\zipalign" -v -p 4 "%WORK_DIR%\app-aarch64-unsigned.apk" "%WORK_DIR%\app-aligned.apk"
if errorlevel 1 exit /b 1

REM Step 5: Sign
echo.
echo [5/5] Signing...
"%BUILD_TOOLS%\apksigner" sign ^
    --ks "%KEYSTORE%" ^
    --ks-pass "pass:%KEY_STOREPASS%" ^
    --key-pass "pass:%KEY_KEYPASS%" ^
    --out "%SIGNED_APK%" ^
    "%WORK_DIR%\app-aligned.apk"
if errorlevel 1 exit /b 1

REM Clean up
rd /s /q "%WORK_DIR%"

echo.
echo === Build Complete ===
echo Signed APK: %SIGNED_APK%
dir "%SIGNED_APK%" | find "Kaulan-aarch64.apk"
echo.
echo To install, run:
echo   adb install "%SIGNED_APK%"
echo.

endlocal
