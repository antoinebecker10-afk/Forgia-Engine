@echo off
REM ===================================================================
REM Forgia V2 — Debug launcher
REM Lance forgia-game.exe avec capture complète dans forgia2_run.log
REM Usage : double-click ce fichier OU `run_debug.bat` dans cmd
REM L'IA peut ensuite lire forgia2_run.log pour diagnostic
REM ===================================================================

cd /d "%~dp0"

REM Backup ancien log si existe
if exist forgia2_run.log (
    copy /Y forgia2_run.log forgia2_run.log.previous >nul
)

REM Variables d'env debug — tous les crates forgia_* à info minimum (catch-all)
set RUST_BACKTRACE=full
set RUST_LOG=info,bevy_render=warn,wgpu=warn,naga=warn,bevy_winit=info,bevy_diagnostic=warn,bevy_egui::render=warn

echo [run_debug] Starting Forgia V2...
echo [run_debug] Log : forgia2_run.log
echo [run_debug] Backtrace : full
echo [run_debug] ESC pour quitter, Alt+F4 si menu mort
echo.

REM Lance forgia.exe (Pattern A workspace root, post-Rewrite).
REM forgia-game.exe = legacy backward-compat, ne contient pas les fixes recents.
target\release-fast\forgia.exe > forgia2_run.log 2>&1

echo.
echo [run_debug] Forgia V2 closed (exit %ERRORLEVEL%)
echo [run_debug] Log saved to : forgia2_run.log
echo [run_debug] Previous log : forgia2_run.log.previous
pause
