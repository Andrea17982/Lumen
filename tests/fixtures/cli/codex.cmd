@echo off
if "%~1"=="--version" (echo codex offline-fixture& exit /b 0)
if "%~1"=="login" exit /b 0
if "%~2"=="generate-json-schema" (
  powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0schema.ps1" "%~4"
  exit /b
)
if "%~1"=="app-server" (
  powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0catalog.ps1" gpt
  exit /b
)
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0provider.ps1" gpt
