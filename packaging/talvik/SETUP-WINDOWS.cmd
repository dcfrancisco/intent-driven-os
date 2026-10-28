@echo off
setlocal
set "ARCHIVE=%~dp0..\talvik-windows-x86_64-preview.zip"
if not exist "%ARCHIVE%" (
    echo Extract the Talvik ZIP with its original filename, then run this file from the extracted folder.
    exit /b 2
)
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-model-runner.ps1" -ArchivePath "%ARCHIVE%"
exit /b %ERRORLEVEL%
