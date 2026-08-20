@echo off
REM ===================================================================
REM Forgia V2 - Lanceur de debug : le jeu AVEC ses assets ET son log.
REM Usage : double-clic, ou `run_debug.bat` dans cmd.
REM ===================================================================
REM
REM POURQUOI `cargo run` ET PAS L'EXE DIRECTEMENT
REM
REM   Ce script lancait `target\release-fast\forgia.exe` en direct. Bevy
REM   resout sa racine d'assets par `CARGO_MANIFEST_DIR`, une variable que
REM   SEUL cargo pose au lancement. Lance en direct, le jeu cherche donc ses
REM   assets a cote de l'exe -- et n'en trouve aucun, SANS le dire.
REM   (cf. memoire `reference_forgia_asset_root_is_exe_relative`)
REM
REM POURQUOI LA REDIRECTION, ET POURQUOI ELLE MANQUAIT
REM
REM   `forgia2_run.log` n'est pas ecrit par le jeu : c'est cette redirection
REM   de shell, et rien d'autre. Lancer par `cargo run` a la main donne donc
REM   les assets mais AUCUN log.
REM
REM   Constate le 2026-08-17 : le log datait de 71,9 heures pendant que les
REM   capteurs dataient de 2,1 heures. Quatre jours de diagnostic menes sans
REM   log, sur la conviction qu'un crash de fermeture empechait le vidage --
REM   alors que personne n'ecrivait le fichier. Les deux facons de lancer
REM   perdaient chacune une moitie, et personne ne les avait reconciliees.
REM
REM   > Un instrument qu'on croit casse alors qu'il n'est pas branche coute
REM   > plus cher qu'un instrument absent : on cesse de le chercher.
REM
REM ===================================================================

cd /d "%~dp0"

REM Le log precedent est garde : comparer deux runs est la moitie d'un
REM diagnostic, et `forgia_digest.py` sait lire le `.previous`.
if exist forgia2_run.log (
    copy /Y forgia2_run.log forgia2_run.log.previous >nul
)

REM Tous les crates forgia_* a `info` ; le bruit des dependances est baisse.
set RUST_BACKTRACE=full
set RUST_LOG=info,bevy_render=warn,wgpu=warn,naga=warn,bevy_winit=info,bevy_diagnostic=warn,bevy_egui::render=warn

echo [run_debug] Compilation puis lancement (profil release-fast)...
echo [run_debug] Log        : forgia2_run.log
echo [run_debug] Precedent  : forgia2_run.log.previous
echo [run_debug] Backtrace  : full
echo.

REM 🚨 `cargo run`, JAMAIS l'exe. La compilation est incluse dans le log :
REM une erreur de build y sera visible au lieu de disparaitre.
cargo run -p forgia --profile release-fast > forgia2_run.log 2>&1

echo.
echo [run_debug] Ferme (code %ERRORLEVEL%)
echo [run_debug] Lire le log SANS l'ouvrir en entier :
echo              python tools\ai\forgia_digest.py all
pause
