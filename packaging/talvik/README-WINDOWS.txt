Talvik Windows x64 preview
=========================

Requirements: 64-bit Windows 10 or later. This ZIP includes the Microsoft
Visual C++ runtime DLLs alongside Talvik, so no separate runtime installation
or administrator rights are required.

Install (no administrator rights):

1. Extract this ZIP.
2. Run SETUP-WINDOWS.cmd from the extracted folder (double-click it or use
   PowerShell). It invokes the included installer for this process only and
   does not change the machine's PowerShell execution policy.

   If you renamed or moved the ZIP, run this from PowerShell instead:

   powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install-model-runner.ps1 -ArchivePath C:\path\to\talvik-windows-x86_64-preview.zip

The installer puts Talvik in %USERPROFILE%\.marina. It does not change PATH,
install a Windows service, or download a model. To use these commands in the
current PowerShell window:

   $env:Path = "$env:USERPROFILE\.marina\bin;$env:Path"

Start the daemon in PowerShell window 1:

   talvikd

In PowerShell window 2, check status and try generation:

   talvik status
   talvik models
   talvik run qwen2.5-0.5b-instruct-q4_k_m "What is 36 x 36? Answer with one number."

The first daemon startup may download the configured default model. Model
download and inference require an internet connection and adequate disk space
and memory. Stop the foreground daemon with Ctrl+C.

This is an unsigned preview ZIP, not an MSI or a Windows Service installer.
Windows inference, cancellation, and recovery are not yet fully qualified.
Please report the Windows version, CPU, command output, and any daemon errors.
Do not send API tokens or other secrets.

The app-local Visual C++ runtime is packaged with this preview. It is not
installed system-wide; future Talvik releases will carry their own updated
runtime files. See Microsoft's
https://learn.microsoft.com/en-us/cpp/windows/deployment-in-visual-cpp
for deployment details.
