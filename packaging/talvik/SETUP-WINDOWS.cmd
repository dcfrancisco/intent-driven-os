@echo off
setlocal
set "ARCHIVE="
rem Prefer the ZIP matching the extracted folder name so the CPU and Vulkan
rem packages cannot be confused when both ZIPs sit in the same directory.
for %%I in ("%~dp0.") do set "FOLDER=%%~nxI"
if exist "%~dp0..\%FOLDER%.zip" set "ARCHIVE=%~dp0..\%FOLDER%.zip"
if not defined ARCHIVE if exist "%~dp0..\talvik-windows-x86_64-preview.zip" set "ARCHIVE=%~dp0..\talvik-windows-x86_64-preview.zip"
if not defined ARCHIVE (
    echo Extract the Talvik ZIP with its original filename, then run this file from the extracted folder.
    exit /b 2
)
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-model-runner.ps1" -ArchivePath "%ARCHIVE%"
exit /b %ERRORLEVEL%
