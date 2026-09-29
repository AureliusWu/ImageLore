@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo.
echo ========================================
echo   ImageLore Windows Setup EXE Builder
echo ========================================
echo.
where node >nul 2>nul || goto :missing
where npm >nul 2>nul || goto :missing
where cargo >nul 2>nul || goto :missing
where rustc >nul 2>nul || goto :missing

echo [1/4] Sync version...
call npm run version:sync || goto :failed
echo [2/4] Install frontend dependencies...
call npm ci || goto :failed
echo [3/4] Verify version...
call npm run version:check || goto :failed
echo [4/4] Build NSIS installer...
call npm run tauri:build || goto :failed

echo.
echo Build complete. Opening installer folder...
start "" "%~dp0src-tauri\target\release\bundle\nsis"
pause
exit /b 0
:missing
echo Missing Node.js or Rust build tools.
echo End users do not need them. Use the GitHub Actions generated setup.exe instead.
pause
exit /b 1
:failed
echo Build failed. Review the log above.
pause
exit /b 1
