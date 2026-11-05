@echo off
if "%~1"=="auth" exit /b 0
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0provider.ps1" claude
