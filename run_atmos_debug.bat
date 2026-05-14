@echo off
cd /d "%~dp0"
set RUST_BACKTRACE=full
set RUST_LOG=info,bevy_pbr=debug,bevy_pbr::atmosphere=trace,bevy_render::renderer=info,wgpu=warn
echo Running with atmosphere debug logs...
target\release-fast\forgia-game.exe > forgia2_atmos_debug.log 2>&1
echo Done. Exit: %ERRORLEVEL%
