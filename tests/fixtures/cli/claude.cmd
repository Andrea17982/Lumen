@echo off
setlocal
set fixtureDir=%~dp0
if "%~1"=="auth" exit /b 0
if "%~1"=="--version" (echo claude offline-fixture& exit /b 0)
set LUMEN_FIXTURE_SESSION=
:sessionargs
if "%~1"=="" goto run
if "%~1"=="--session-id" set LUMEN_FIXTURE_SESSION=%~2
if "%~1"=="--resume" set LUMEN_FIXTURE_SESSION=%~2
shift
goto sessionargs
:run
if not "%LUMEN_FIXTURE_SESSION%"=="" goto provider
rem Catalog discovery has no provider session id.
goto catalog
:catalog
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%fixtureDir%catalog.ps1" claude
exit /b
:provider
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%fixtureDir%provider.ps1" claude
exit /b
