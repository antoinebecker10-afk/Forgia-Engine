#!/usr/bin/env python3
"""sensor_honesty.py — quels capteurs se déclarent « ok » sans rien avoir mesuré ?

story-699. Un capteur de feature dont TOUS les compteurs sont à zéro et qui rapporte
`severity: "ok"` ment par omission : rien n'a échoué, mais rien ne s'est produit non
plus. Un système inerte ne lève aucune erreur — c'est ce qui le rend invisible.

Trois cas réels le 2026-08-12 : `gamefeel` (hitstop à 0), `weapon_vfx`
(`kill_bursts: 0`), `elements` (réactions à 0). Les trois disaient « ok », et deux
ont failli faire fermer automatiquement des stories prouvées cassées le matin même.

    python tools/ai/sensor_honesty.py

Ce script ne corrige rien : il **désigne les suspects**. La condition « censé
tourner » ne se devine pas depuis le JSON — c'est le travail à faire, capteur par
capteur, et il demande de lire le code producteur.
"""

import json
import time
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Champs qui ne sont pas des compteurs d'activité : les ignorer évite de prendre
# une config à zéro pour une inactivité.
NON_COMPTEURS = {
    "timestamp_secs", "severity", "id", "next_step", "schema_version",
    "reload_count", "enabled", "always_on", "expected", "present",
}
SUFFIXES_CONFIG = ("_mult", "_pct", "_m", "_ms", "_secs", "_s", "_max", "_min",
                   "_threshold", "_radius", "_gain", "_volume", "_ratio", "_deg")

# JAUGES vs COMPTEURS — la distinction qui a manqué à la v1 de ce script.
#
# Une **jauge** décrit l'instant : `active_count`, `shots_fired_last_sec`,
# `registry_size`, `avg_progress`. À zéro au menu, elle est parfaitement correcte —
# la signaler, c'est crier au loup, et un outil qui crie au loup finit ignoré.
#
# Un **compteur cumulatif** ne redescend jamais : `kill_sounds_played`,
# `burns_applied`, `pushes`. À zéro après une run active, il dit quelque chose.
#
# Mesuré le 2026-08-12 : sur 13 suspects après la run boss, **4 étaient des jauges**
# et un seul un vrai compteur muet (`forgia_arena_feedback.kill_sounds_played`,
# cinquième témoin du bug de son de kill). Sans cette distinction, le vrai se
# noyait dans les faux.
MOTIFS_JAUGE = ("_last_sec", "_now", "active_", "avg_", "mean_", "pending_",
                "_size", "_count", "_left", "_alive", "_current")


def compteurs(d, prefixe=""):
    """Champs qui sont des COMPTEURS CUMULATIFS — jauges et config exclues."""
    out = {}
    for k, v in (d or {}).items():
        nom = f"{prefixe}{k}"
        if isinstance(v, dict):
            out.update(compteurs(v, f"{nom}."))
        elif isinstance(v, bool) or k in NON_COMPTEURS:
            continue
        elif not isinstance(v, int) or nom.endswith(SUFFIXES_CONFIG):
            continue
        elif any(m in nom for m in MOTIFS_JAUGE):
            continue  # jauge : zéro au menu est correct, pas suspect
        else:
            out[nom] = v
    return out


def main():
    fichiers = sorted(ROOT.glob("forgia*.json"))
    frais = max((p.stat().st_mtime for p in fichiers), default=0)

    suspects, actifs, sans_compteur, perimes = [], [], [], []
    for p in fichiers:
        if ".previous." in p.name:
            continue
        try:
            d = json.loads(p.read_text(encoding="utf-8"))
        except Exception:
            continue
        if not isinstance(d, dict):
            continue
        sev = d.get("severity")
        c = compteurs(d)
        age = frais - p.stat().st_mtime
        entree = (p.name, sev, len(c), sum(c.values()))
        if age > 300:
            perimes.append((p.name, age / 86400))
        elif not c:
            sans_compteur.append(entree)
        elif sum(c.values()) == 0 and sev in ("ok", None):
            suspects.append(entree)
        else:
            actifs.append(entree)

    print(f"{len(fichiers)} capteurs · run la plus récente il y a "
          f"{(time.time()-frais)/60:.0f} min\n")

    print(f"{'='*76}\n⛔ SUSPECTS — « ok » avec TOUS les compteurs à zéro ({len(suspects)})\n{'='*76}")
    print("Ni erreur, ni activité. Impossible de distinguer « inerte » de « pas")
    print("encore sollicité » sans lire le code : c'est exactement le travail de 699.\n")
    for nom, sev, n, _ in sorted(suspects):
        print(f"  {nom:44} severity={sev!s:8} {n} compteur(s), tous à 0")

    print(f"\n{'='*76}\n🔍 SANS AUCUN COMPTEUR ({len(sans_compteur)})\n{'='*76}")
    print("Ne peuvent PAS prouver leur activité. Si leur sujet meurt, rien ne le dira.\n")
    for nom, sev, _, _ in sorted(sans_compteur)[:20]:
        print(f"  {nom:44} severity={sev}")

    print(f"\n{'='*76}\n✅ ACTIFS — au moins un compteur non nul ({len(actifs)})\n{'='*76}")

    if perimes:
        print(f"\n{'='*76}\n⏸️  NON RÉÉCRITS PAR LA DERNIÈRE RUN ({len(perimes)})\n{'='*76}")
        print("Mode non joué, ou producteur mort. Ni jugés, ni oubliés.\n")
        for nom, j in sorted(perimes, key=lambda x: -x[1])[:10]:
            print(f"  {nom:44} {j:5.1f} jours")

    print(f"\n── {len(suspects)} suspects · {len(sans_compteur)} sans compteur · "
          f"{len(actifs)} actifs · {len(perimes)} non réécrits ──")


if __name__ == "__main__":
    main()
