@echo off
if "%~1"=="login" exit /b 0
if "%~1"=="app-server" (
  powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0catalog.ps1" gpt
  exit /b
)
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0provider.ps1" gpt
