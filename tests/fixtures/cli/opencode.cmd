@echo off
if "%~1"=="--version" (echo opencode v2.2.7 offline-fixture& exit /b 0)
if "%~1"=="models" (echo openai/gpt-6.1-sol& exit /b 0)
if "%~1"=="auth" exit /b 0
if "%~1"=="api" (node "%~dp0opencode-acp.mjs" %*& exit /b %errorlevel%)
if "%~1"=="acp" (node "%~dp0opencode-acp.mjs" %*& exit /b %errorlevel%)
exit /b 10
