@echo off
if "%~1"=="--version" (echo 2026.06.04-offline-fixture& exit /b 0)
if "%~1"=="--list-models" (echo composer-2.5 - Composer 2.5& echo gpt-6.1-sol - GPT 6.1 Sol& exit /b 0)
if "%~1"=="login" exit /b 0
setlocal
set "fixtureDir=%~dp0"
set "session="
set force=false
set trust=false
:args
if "%~1"=="" goto run
if "%~1"=="--resume" set "session=%~2"
if "%~1"=="--force" set force=true
if "%~1"=="--trust" set trust=true
shift
goto args
:run
if not "%force%"=="true" exit /b 7
if not "%trust%"=="true" exit /b 8
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%fixtureDir%cursor-agent.ps1" "%session%"
