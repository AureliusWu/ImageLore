@echo off
chcp 65001 >nul
setlocal
set IL=%LOCALAPPDATA%\ImageLore
set PD=%LOCALAPPDATA%\PromptDock

echo This cleanup is ONLY for pre-v0.10 preview data.
echo It will NOT delete ImageLore v0.10 library.sqlite3.
echo.
choice /M "Delete old PromptDock data and old ImageLore imagelore.db/cache"
if errorlevel 2 exit /b 0

if exist "%PD%" rmdir /S /Q "%PD%"
if exist "%IL%\imagelore.db" del /Q "%IL%\imagelore.db"
if exist "%IL%\thumbnails" rmdir /S /Q "%IL%\thumbnails"

echo Legacy preview data cleaned.
pause
