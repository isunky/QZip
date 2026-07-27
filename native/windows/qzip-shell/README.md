# QZip Windows 11 Shell integration

This native DLL provides the first-level **QZip** flyout through `IExplorerCommand`. It performs no archive work in Explorer. It sends at most 1,000 selected paths through a single-use JSON request under `%LOCALAPPDATA%\QZip\ShellRequests`, then starts `QZip.exe --shell-request <uuid>`.

The DLL must be registered by a signed sparse MSIX package. Use `scripts/build-windows-shell-integration.ps1` to compile and package it. That script does not install a development certificate unless `-InstallDevCertificate` is explicitly supplied. Formal release builds require the three `QZIP_WINDOWS_*` signing environment variables and fail closed if absent.
