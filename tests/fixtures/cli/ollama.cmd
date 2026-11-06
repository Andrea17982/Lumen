@echo off
rem Offline Ollama fixture: lists two local models and answers with plain text.
if "%~1"=="list" (
  echo NAME                 ID              SIZE      MODIFIED
  echo tiny-fixture:latest  aaaaaaaaaaaa    637 MB    2 days ago
  echo qwen-fixture:7b      bbbbbbbbbbbb    4.7 GB    1 day ago
  exit /b 0
)
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0provider.ps1" ollama
