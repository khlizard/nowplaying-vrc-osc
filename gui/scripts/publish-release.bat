@echo off
setlocal EnableExtensions

rem Now Playing -> VRChat: self-contained release ZIP
rem Usage: gui\scripts\publish-release.bat [Single|Folder]

set "SCRIPT_DIR=%~dp0"
set "ARGS=%*"

where pwsh >nul 2>&1
if %ERRORLEVEL%==0 (
    pwsh -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%publish-release.ps1" %ARGS%
    exit /b %ERRORLEVEL%
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%publish-release.ps1" %ARGS%
exit /b %ERRORLEVEL%
