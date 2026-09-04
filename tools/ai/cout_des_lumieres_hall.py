#!/usr/bin/env python3
"""Combien coutent VRAIMENT les lumieres du Hall ? Un A/B par interrupteur.

    python tools/ai/cout_des_lumieres_hall.py

Pourquoi cet outil : le Hall porte 154 lumieres ponctuelles et spots, deux sondes
d'environnement recalculees a chaque image, et une lumiere cuite — et le cout de
chacune n'a JAMAIS ete mesure. Le genome expose pourtant trois interrupteurs en
rechargement a chaud : couper, attendre, relire le capteur.

Prerequis : le jeu tourne, DANS le Hall, joueur immobile. Le plus simple :

    FORGIA_BOOT_MODE=castle_hub ./target/release-fast/forgia.exe

⚠ PORTEE : la mesure vaut pour CE point de vue. Le frame time du Hall depend de
ce que la camera voit ; deux releves a des endroits differents ne se comparent
pas. Le joueur doit rester ou le jeu l'a pose.

⚠ Le capteur `forgia2_perf.json` moyenne sur un anneau de 120 echantillons, soit
environ 3 s a 40 images/s. On attend donc largement plus que ca entre deux
bascules, sinon on lit un melange des deux etats.
"""

from __future__ import annotations

import json
import re
import statistics
import sys
import time
from pathlib import Path

RACINE = Path(__file__).resolve().parents[2]
GENOME = RACINE / "assets/genomes/castle_hub_lighting.toml"
PERF = RACINE / "forgia2_perf.json"
LAG = RACINE / "forgia2_lag_events.json"

# Largement plus que la fenetre de 120 echantillons du capteur.
ATTENTE_S = 14.0
# Nombre de relevés moyennés par état — un seul point est du bruit.
ECHANTILLONS = 5
INTERVALLE_S = 1.2


def basculer(section: str, cle: str, valeur: str) -> str:
    """Ecrit `cle = valeur` dans `[section]`. Rend l'ancienne valeur."""
    lignes = GENOME.read_text(encoding="utf-8").split("\n")
    debut = next(i for i, l in enumerate(lignes) if l.strip() == "[%s]" % section)
    for j in range(debut + 1, len(lignes)):
        if lignes[j].startswith("["):
            break
        if re.match(r"\s*%s\s*=" % re.escape(cle), lignes[j]):
            ancienne = lignes[j].split("=", 1)[1].strip()
            # On conserve tout commentaire de fin de ligne : ce fichier est
            # documente, et une mesure ne doit pas effacer une explication.
            lignes[j] = "%s = %s" % (cle, valeur)
            GENOME.write_text("\n".join(lignes), encoding="utf-8")
            return ancienne
    raise SystemExit("cle %s introuvable dans [%s]" % (cle, section))


def releve() -> dict | None:
    """Moyenne de plusieurs lectures du capteur, pour ne pas juger sur un point."""
    avgs, mins, fps = [], [], []
    debut_total = None
    for _ in range(ECHANTILLONS):
        try:
            d = json.loads(PERF.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            time.sleep(INTERVALLE_S)
            continue
        avgs.append(d["frame_time_avg_ms"])
        mins.append(d["frame_time_min_ms"])
        fps.append(d["fps_smoothed"])
        if debut_total is None:
            try:
                debut_total = json.loads(LAG.read_text(encoding="utf-8"))["total_recorded"]
            except Exception:
                debut_total = None
        time.sleep(INTERVALLE_S)
    if not avgs:
        return None
    fin_total = None
    try:
        fin_total = json.loads(LAG.read_text(encoding="utf-8"))["total_recorded"]
    except Exception:
        pass
    pics_par_s = None
    if debut_total is not None and fin_total is not None:
        duree = ECHANTILLONS * INTERVALLE_S
        pics_par_s = (fin_total - debut_total) / duree
    return {
        "frame_avg": statistics.mean(avgs),
        "frame_avg_ecart": statistics.pstdev(avgs),
        "frame_min": min(mins),
        "fps": statistics.mean(fps),
        "pics_par_s": pics_par_s,
    }


def attendre(motif: str) -> None:
    print("   … %s (%.0f s)" % (motif, ATTENTE_S), flush=True)
    time.sleep(ATTENTE_S)


def main() -> int:
    if not PERF.exists():
        raise SystemExit("forgia2_perf.json absent — le jeu tourne-t-il ?")

    print("PORTEE : un seul point de vue (celui ou le jeu pose le joueur au Hall),")
    print("         joueur immobile. Chaque etat est moyenne sur %d lectures espacees"
          % ECHANTILLONS)
    print("         de %.1f s, apres %.0f s de stabilisation. Ce qui n'est PAS couvert :"
          % (INTERVALLE_S, ATTENTE_S))
    print("         les autres points de vue, l'exterieur, et le cout CPU par systeme")
    print("         (aucun instrument du depot ne le nomme — il faut Tracy).")
    print()

    # Les trois interrupteurs gratuits, dans l'ordre du moins au plus invasif.
    essais = [
        ("temoin", None, None, None),
        ("sans les 96 bougies", "flames", "enabled", "false"),
        ("sans les 56 lumieres du createur", "creator_lights", "enabled", "false"),
        ("sans l'eclairage par image", "environment", "enabled", "false"),
        ("sans la lumiere cuite", "lightmaps", "enabled", "false"),
    ]

    a_rendre: list[tuple[str, str, str]] = []
    resultats = []
    try:
        for nom, section, cle, valeur in essais:
            if section is not None:
                ancienne = basculer(section, cle, valeur)
                a_rendre.append((section, cle, ancienne))
                attendre("bascule %s.%s = %s" % (section, cle, valeur))
            else:
                attendre("stabilisation du temoin")
            r = releve()
            if r is None:
                print("!! %-34s AVEUGLE — capteur illisible" % nom)
                continue
            resultats.append((nom, r))
            print("   %-34s %6.2f ms (+/- %.2f)  min %5.2f  %5.1f fps  %s"
                  % (nom, r["frame_avg"], r["frame_avg_ecart"], r["frame_min"], r["fps"],
                     ("%.2f pic/s" % r["pics_par_s"]) if r["pics_par_s"] is not None else ""))
            # On rend l'interrupteur avant le suivant : on isole UNE variable.
            if section is not None:
                section_, cle_, ancienne_ = a_rendre.pop()
                basculer(section_, cle_, ancienne_)
                time.sleep(2.0)
    finally:
        for section, cle, ancienne in reversed(a_rendre):
            basculer(section, cle, ancienne)
        print("\ngenome rendu a son etat d'origine.")

    if len(resultats) >= 2:
        base = resultats[0][1]["frame_avg"]
        print()
        print("%-34s %10s %10s" % ("", "gain ms", "gain %"))
        for nom, r in resultats[1:]:
            g = base - r["frame_avg"]
            print("%-34s %+10.2f %9.1f %%" % (nom, g, 100.0 * g / base))
        print()
        print("Un gain sous l'ecart-type du temoin (%.2f ms) n'est PAS un gain."
              % resultats[0][1]["frame_avg_ecart"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
