@echo off
setlocal EnableExtensions

rem Now Playing -> VRChat GUI: icon generate + Debug/Release build
rem Usage: gui\scripts\build-gui.bat

set "SCRIPT_DIR=%~dp0"
for %%I in ("%SCRIPT_DIR%..") do set "GUI_DIR=%%~fI"
for %%I in ("%GUI_DIR%\..") do set "REPO_ROOT=%%~fI"

set "ICON_PNG=%REPO_ROOT%\resources\icon1024.png"
if not exist "%ICON_PNG%" set "ICON_PNG=%REPO_ROOT%\docs\icon1024.png"

echo.
echo === [1/4] app.ico を生成 ===
if not exist "%ICON_PNG%" (
    echo エラー: icon1024.png が見つかりません。
    echo   %REPO_ROOT%\resources\icon1024.png
    echo   %REPO_ROOT%\docs\icon1024.png
    exit /b 1
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%generate-app-icon.ps1" -InputPath "%ICON_PNG%"
if errorlevel 1 (
    echo エラー: アイコン生成に失敗しました。
    exit /b 1
)

echo.
echo === [2/4] 実行中のアプリを終了（ロック回避） ===
taskkill /IM NowPlayingVrcOsc.App.exe /F >nul 2>&1

echo.
echo === [3/4] Debug ビルド ===
pushd "%GUI_DIR%"
dotnet build NowPlayingVrcOsc.slnx -c Debug
if errorlevel 1 (
    popd
    echo エラー: Debug ビルドに失敗しました。
    exit /b 1
)

echo.
echo === [4/4] Release ビルド ===
dotnet build NowPlayingVrcOsc.slnx -c Release
if errorlevel 1 (
    popd
    echo エラー: Release ビルドに失敗しました。
    exit /b 1
)
popd

echo.
echo === 完了 ===
echo Debug:   %GUI_DIR%\NowPlayingVrcOsc.App\bin\Debug\net10.0-windows\NowPlayingVrcOsc.App.exe
echo Release: %GUI_DIR%\NowPlayingVrcOsc.App\bin\Release\net10.0-windows\NowPlayingVrcOsc.App.exe
echo.

endlocal
exit /b 0
