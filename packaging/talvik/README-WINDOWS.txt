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

Two ZIPs are published:

   talvik-windows-x86_64-preview.zip         CPU only
   talvik-windows-x86_64-vulkan-preview.zip  CPU + Vulkan GPU backend

The Vulkan ZIP needs a Vulkan-capable GPU driver (Intel, AMD, or NVIDIA). It
installs a config.yaml selecting the Vulkan backend with automatic CPU
fallback, only when no config.yaml exists. Installing either ZIP replaces the
previously installed DLLs, so you can switch between them. Expect the first
request after each daemon start to be slow while GPU shaders compile.

The installer puts Talvik in %USERPROFILE%\.marina. It does not change PATH,
install a Windows service, or download a model. To use these commands in the
current PowerShell window:

   $env:Path = "$env:USERPROFILE\.marina\bin;$env:Path"

Start the daemon in PowerShell window 1:

   talvikd

In PowerShell window 2, check status and try generation:

   talvik status
   talvik models
   talvik run --default "What is 36 x 36? Answer with one number."

The first daemon startup downloads the configured Gemma 4 model and vision
projector if missing (about 4.8 GiB total). Download and inference require an
internet connection and adequate disk space and memory. Stop the foreground
daemon with Ctrl+C.

This is an unsigned preview ZIP, not an MSI or a Windows Service installer.
Windows inference, cancellation, and recovery are not yet fully qualified.
Please report the Windows version, CPU, command output, and any daemon errors.
Do not send API tokens or other secrets.

The app-local Visual C++ runtime is packaged with this preview. It is not
installed system-wide; future Talvik releases will carry their own updated
runtime files. See Microsoft's
https://learn.microsoft.com/en-us/cpp/windows/deployment-in-visual-cpp
for deployment details.
