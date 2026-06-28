@echo off
chcp 65001 >nul
echo.
echo  ========================================
echo   IGC Orchestrator - Test Suite
echo  ========================================
echo.

cd /d "C:\Users\Antoi\Desktop\Forgia"

echo  [TEST 1] --help
echo  ----------------------------------------
node tools/orchestrator.js --help
echo.

echo  [TEST 2] --agents (liste les 14 agents)
echo  ----------------------------------------
node tools/orchestrator.js --agents
echo.

echo  [TEST 3] --dry-run ping test
echo  ----------------------------------------
echo  Ceci va appeler claude -p via Archibald pour decomposer,
echo  puis les agents assigns, puis Sentinel pour valider.
echo  Timeout: ~2min par agent.
echo.
node tools/orchestrator.js --dry-run "Ping test: chaque agent repond PONG avec son nom et emoji"
echo.

echo  ========================================
echo   Tests termines.
echo  ========================================
pause
