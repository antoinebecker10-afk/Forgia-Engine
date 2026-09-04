"""Auto-test du harnais de debug BRP : `key`, `look`, `release_all` et le CLI.

Les scénarios prouvent le JEU ; celui-ci prouve l'INSTRUMENT. Il lance le vrai
binaire, pilote par `tools/ai/brp.py` (donc le CLI est testé lui aussi), et
affirme des effets OBSERVABLES dans le snapshot — pas seulement que la requête a
été acceptée. À rejouer après toute modification de `dev_brp_scenario.rs` :

    python tools/ai/tests/valider_brp_debug.py

Durée ~1 min (build à jour supposé). Sortie : 7 contrôles nommés, PASS/FAIL.
"""

import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.request

ROOT = str(Path(__file__).resolve().parents[3])
URL = "http://127.0.0.1:15702/"
LOG = os.path.join(ROOT, "target", "forgia_agent", "brp_debug_validation.log")


def brp(method, params=None):
    payload = {"jsonrpc": "2.0", "id": 1, "method": method}
    if params is not None:
        payload["params"] = params
    req = urllib.request.Request(
        URL,
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=3) as r:
        body = json.load(r)
    if "error" in body:
        raise RuntimeError(body["error"])
    return body["result"]


def cli(*args):
    sortie = subprocess.run(
        [sys.executable, "tools/ai/brp.py", *args],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if sortie.returncode != 0:
        raise RuntimeError(f"brp.py {' '.join(args)} -> {sortie.stdout}{sortie.stderr}")
    return sortie.stdout.strip()


def ecart_angle(a, b):
    d = (a - b + 180.0) % 360.0 - 180.0
    return d


def main():
    env = os.environ.copy()
    env["FORGIA_BOOT_MODE"] = "expedition"
    log = open(LOG, "w", encoding="utf-8")
    jeu = subprocess.Popen(
        ["cargo", "run", "-p", "forgia", "--features", "dev-brp", "--"],
        cwd=ROOT,
        env=env,
        stdout=log,
        stderr=subprocess.STDOUT,
    )
    resultats = []
    try:
        fin = time.monotonic() + 120
        snap = None
        while time.monotonic() < fin:
            try:
                snap = brp("forgia.scenario.snapshot")
                if snap.get("player") and snap.get("game_mode") == "Expedition":
                    break
            except Exception:
                pass
            time.sleep(1.0)
        else:
            raise TimeoutError("le jeu n'a jamais publie de joueur en Expedition")

        # 1. Le CLI parle-t-il ?
        ligne = cli("snapshot")
        resultats.append(("cli_snapshot_compact", "mode=Expedition" in ligne, ligne[:120]))

        # 2. look : le cap doit VRAIMENT tourner de ~90 deg, par mouse_look.
        avant = brp("forgia.scenario.snapshot")["player"]["yaw_deg"]
        cli("look", "--yaw", "90", "--frames", "10")
        time.sleep(0.8)
        apres = brp("forgia.scenario.snapshot")["player"]["yaw_deg"]
        tourne = ecart_angle(apres, avant)
        resultats.append(
            ("look_tourne_de_90_deg", abs(tourne - 90.0) < 8.0, f"{avant:.1f} -> {apres:.1f} ({tourne:+.1f} deg)")
        )

        # 3. key --hold : la touche doit apparaitre dans les entrees tenues.
        cli("key", "ShiftLeft", "--hold")
        time.sleep(0.3)
        tenues = brp("forgia.scenario.snapshot")["inputs"]["held_keys"]
        resultats.append(("key_hold_tenue", "ShiftLeft" in tenues, tenues))

        # 4. key --release : elle doit disparaitre.
        cli("key", "ShiftLeft", "--release")
        time.sleep(0.3)
        tenues = brp("forgia.scenario.snapshot")["inputs"]["held_keys"]
        resultats.append(("key_release_relachee", "ShiftLeft" not in tenues, tenues))

        # 5. tap + release_all : rien ne reste colle.
        cli("key", "KeyR", "--frames", "600")
        time.sleep(0.3)
        tenues_avant = brp("forgia.scenario.snapshot")["inputs"]["held_keys"]
        cli("release-all")
        time.sleep(0.3)
        tenues_apres = brp("forgia.scenario.snapshot")["inputs"]["held_keys"]
        resultats.append(
            (
                "release_all_vide_tout",
                "KeyR" in tenues_avant and tenues_apres == [],
                f"{tenues_avant} -> {tenues_apres}",
            )
        )

        # 6. Le harnais ne se declare jamais en panne pendant tout ca.
        panne = brp("forgia.scenario.snapshot")["inputs"]["failure"]
        resultats.append(("harnais_sans_panne", panne is None, panne))

        # 7. watch rend bien des echantillons.
        sortie = cli("watch", "--seconds", "2", "--interval", "0.2")
        resultats.append(("watch_echantillonne", "echantillons" in sortie, sortie.splitlines()[-1]))
    finally:
        try:
            jeu.terminate()
            jeu.wait(timeout=15)
        except Exception:
            jeu.kill()
        log.close()

    print()
    for nom, ok, observe in resultats:
        print(("PASS " if ok else "FAIL "), nom, "|", observe)
    total = sum(1 for _, ok, _ in resultats if ok)
    print(f"\n{total}/{len(resultats)} controles verts (echantillon declare : 7 controles)")
    return 0 if total == len(resultats) else 1


if __name__ == "__main__":
    sys.exit(main())
