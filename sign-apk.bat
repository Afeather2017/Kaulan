@echo off
setlocal enabledelayedexpansion

set "JAVA_HOME=C:\Program Files\Android\Android Studio\jbr"
set "KEYTOOL=C:\Program Files\Android\Android Studio\jbr\bin\keytool.exe"
set "BUILD_TOOLS=C:\Users\Administrator\AppData\Local\Android\Sdk\build-tools\35.0.0"
set KEYSTORE=%TEMP%\release.keystore
set KEY_ALIAS=release
set KEY_STOREPASS=123456
set KEY_KEYPASS=123456
set UNSIGNED_APK=E:\Kaulan\frontend\src-tauri\gen\android\app\build\outputs\apk\universal\release\app-universal-release-unsigned.apk
set ALIGNED_APK=%TEMP%\app-aligned.apk
set SIGNED_APK=%TEMP%\app-signed.apk

echo Checking keystore...
if not exist "%KEYSTORE%" (
    echo Generating keystore...
    "%KEYTOOL%" -genkey -v -keystore "%KEYSTORE%" -alias "%KEY_ALIAS%" -keyalg RSA -keysize 2048 -validity 10000 -storepass "%KEY_STOREPASS%" -keypass "%KEY_KEYPASS%" -dname "CN=Kaulan,O=Kaulan,C=US"
    if errorlevel 1 exit /b 1
) else (
    echo Using existing keystore: %KEYSTORE%
)

if exist "%ALIGNED_APK%" del "%ALIGNED_APK%"
if exist "%SIGNED_APK%" del "%SIGNED_APK%"

echo Zipaligning...
"%BUILD_TOOLS%\zipalign" -v -p 4 "%UNSIGNED_APK%" "%ALIGNED_APK%"
if errorlevel 1 exit /b 1

echo Signing...
"%BUILD_TOOLS%\apksigner" sign --ks "%KEYSTORE%" --ks-pass "pass:%KEY_STOREPASS%" --key-pass "pass:%KEY_KEYPASS%" --out "%SIGNED_APK%" "%ALIGNED_APK%"
if errorlevel 1 exit /b 1

echo.
echo === Sign Complete ===
echo Signed APK: %SIGNED_APK%
echo.
echo To install, run:
echo   adb install "%SIGNED_APK%"
echo.

endlocal
