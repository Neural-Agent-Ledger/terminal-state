@echo off
REM ===================================================================
REM  TERMINAL STATE launcher
REM  Double-click this file to play. %~dp0 is this script's own folder,
REM  so it works no matter where the folder lives, as long as build\ and
REM  levels\ / assets\ sit next to this .bat. Runs in THIS console window
REM  (no second window, no Windows Terminal required).
REM ===================================================================
cd /d "%~dp0"
build\terminal-state.exe
if errorlevel 1 (
  echo.
  echo TERMINAL STATE exited with an error. The message above, if any, says what went wrong.
  pause
)
