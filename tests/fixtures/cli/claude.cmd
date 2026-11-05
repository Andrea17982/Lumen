@echo off
if "%~1"=="auth" exit /b 0
if "%~3"=="--input-format" (
  powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0catalog.ps1" claude
  exit /b
)
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0provider.ps1" claude
