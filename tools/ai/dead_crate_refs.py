#!/usr/bin/env python3
"""dead_crate_refs.py — les crates supprimées encore citées ont-elles perdu leur code ?

ADR-0002 a supprimé ~200 crates (266 → 62). Beaucoup sont encore nommées dans le
code, presque toujours par des commentaires « migré vers X ». La plupart sont
inoffensives : le code a été ré-absorbé, il vit juste ailleurs.

**Le cas dangereux est celui où la citation n'a pas de code d'accueil.** C'est ce
qui est arrivé au hitstop (story-696) : migré vers `forgia-juice-hit-stop` le
2026-05-17, crate supprimée neuf jours plus tard, et trois commentaires ont continué
d'affirmer pendant trois mois que le plugin était câblé.

    python tools/ai/dead_crate_refs.py

Le discriminant : le **concept** de la crate morte (son suffixe) existe-t-il comme
module quelque part ? `forgia-juice-camera-shake` → `camera_shake.rs` existe → le
code a survécu. `forgia-juice-hit-stop` → aucun `hit_stop.rs` → il est parti.

Heuristique, pas oracle : une ré-implémentation sous un nom différent passera pour
un orphelin. C'est le bon sens de l'erreur — mieux vaut vérifier à tort que rater.
"""

import os
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATES = ROOT / "crates"
CRATE_RE = re.compile(r"\bforgia-[a-z0-9-]{3,}\b")


def concepts(crate_name):
    """Noms de module plausibles pour une crate `forgia-a-b-c`.

    On essaie du plus spécifique au plus large : `a_b_c`, `b_c`, `c`. Un seul
    suffit à considérer le code présent.
    """
    parts = crate_name.split("-")[1:]  # retire « forgia »
    out = []
    for i in range(len(parts)):
        out.append("_".join(parts[i:]))
    return [c for c in out if len(c) >= 3]


def main():
    vivantes = set(os.listdir(CRATES))
    modules = {p.stem for p in CRATES.rglob("*.rs")}
    # Un dossier de module compte aussi (`weapon_vfx/mod.rs` → `weapon_vfx`).
    modules |= {p.parent.name for p in CRATES.rglob("mod.rs")}
    # …et les modules INLINE. `forgia-core` en est plein (`pub mod sensor_io {`),
    # et les ignorer faisait passer `forgia-sensor-io` pour un orphelin alors que
    # son code est bien là. Un inventaire qui sur-signale finit ignoré.
    inline = re.compile(r"^\s*(?:pub\s+)?mod\s+([a-z0-9_]+)\s*\{", re.M)
    for p in CRATES.rglob("*.rs"):
        modules |= set(inline.findall(p.read_text(encoding="utf-8", errors="replace")))

    cites = defaultdict(set)
    for p in CRATES.rglob("*.rs"):
        txt = p.read_text(encoding="utf-8", errors="replace")
        for m in set(CRATE_RE.findall(txt)):
            if m not in vivantes:
                cites[m].add(p.relative_to(ROOT).as_posix())

    sains, orphelins = [], []
    for crate, fichiers in cites.items():
        trouves = [c for c in concepts(crate) if c in modules]
        (sains if trouves else orphelins).append((crate, fichiers, trouves))

    print(f"{len(cites)} crates supprimées encore citées dans crates/\n")

    print(f"{'='*78}\n⛔ SANS CODE D'ACCUEIL — à instruire ({len(orphelins)})\n{'='*78}")
    print("Aucun module ne porte leur concept. Soit la feature est partie avec la")
    print("crate, soit elle a été réécrite sous un autre nom.\n")
    for crate, fichiers, _ in sorted(orphelins):
        print(f"  {crate}")
        for f in sorted(fichiers)[:3]:
            print(f"       cité dans {f}")

    print(f"\n{'='*78}\n✅ CODE RÉ-ABSORBÉ — note historique ({len(sains)})\n{'='*78}")
    for crate, fichiers, trouves in sorted(sains):
        print(f"  {crate:36} → module `{trouves[0]}` présent  ({len(fichiers)} citation(s))")

    print(f"\n── {len(orphelins)} à instruire · {len(sains)} inoffensives ──")
    if orphelins:
        print("\nPour chacune : soit le concept existe sous un autre nom (corriger le")
        print("commentaire), soit il a disparu (décider s'il revient — cf hitstop,")
        print("retiré définitivement le 2026-08-12).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
