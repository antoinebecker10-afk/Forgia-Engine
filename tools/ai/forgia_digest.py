#!/usr/bin/env python3
"""forgia_digest.py — lire les logs et les capteurs SANS brûler de tokens.

Problème résolu
---------------
Un `forgia2_run.log` fait 30 Ko pour ~190 lignes, dont 180 répètent le même
avertissement à la frame près. Le lire en entier coûte cher et noie le signal.
`rtk log` compresse déjà (98 %) mais garde les codes ANSI, tronque le message
et **jette les lignes INFO** — or dans Forgia le signal y est presque toujours
(`[arena-backdrop] 17 props`, `[avatar] rebranchée`, `[cosmetics] +20 Éclats`).

Principe : **réduire d'abord, interpréter ensuite.**

1. Réduction DÉTERMINISTE (gratuite, exacte, instantanée) — c'est 95 % du gain.
   Les messages sont normalisés (ids d'entité, nombres, chemins → jokers) puis
   regroupés : on garde le compte, la première et la dernière occurrence, et un
   exemplaire lisible. 30 Ko deviennent ~2 Ko.
2. Il n'y a PAS d'étape 2.

   Un maillon « interprétation par un modèle local » a été essayé le
   2026-08-07 puis RETIRÉ : sur le premier cas réel il a paraphrasé le digest
   en moins précis, en écrivant « 21h31 » là où le log dit 21:19:31 — sur un
   bug de cycle de vie qui se joue à la milliseconde, ça envoie chercher au
   mauvais endroit. Un résumé approximatif d'une donnée exacte est une
   régression, pas un service.

   Ce fichier ne contient donc aucune IA. Sa sortie est reproductible et
   vérifiable ligne à ligne — ce qu'aucun modèle ne garantit.

Usage
-----
    python tools/ai/forgia_digest.py log                    # digest du run
    python tools/ai/forgia_digest.py log --module avatar    # filtré
    python tools/ai/forgia_digest.py sensors                # les 97 capteurs
    python tools/ai/forgia_digest.py log --tout       # rouvre le démarrage
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import OrderedDict
from pathlib import Path

RACINE = Path(__file__).resolve().parents[2]
LOG_DEFAUT = RACINE / "forgia2_run.log"

# Séquences ANSI de `tracing` — invisibles à l'œil, coûteuses en tokens.
ANSI = re.compile(r"\x1b\[[0-9;]*m|\[[0-9]{1,2}m")

LIGNE = re.compile(
    r"^(?P<ts>\d{4}-\d{2}-\d{2}T[\d:.]+Z)\s+"
    r"(?P<lvl>TRACE|DEBUG|INFO|WARN|ERROR)\s+"
    r"(?P<mod>[\w:]+)\s*:\s*(?P<msg>.*)$"
)

# Ce qui varie d'une occurrence à l'autre sans changer la NATURE du message.
# Sans ça, « corps 450v2 » et « corps 630v3 » comptent pour deux problèmes.
JOKERS = [
    (re.compile(r"\b\d+v\d+\b"), "<entité>"),       # Entity Bevy : 450v2
    (re.compile(r"\b\d+\.\d+\b"), "<n>"),           # flottants
    (re.compile(r"\b\d+\b"), "<n>"),                # entiers
    (re.compile(r"«[^»]*»"), "«…»"),                # noms cités
    (re.compile(r"[A-Za-z]:[\\/][^\s]+"), "<chemin>"),
    # Les cinq pièces d'armure émettent la MÊME ligne cinq fois. Les grouper
    # transforme cinq alertes identiques en une seule, comptée — ce qui est la
    # vérité : c'est UN défaut, pas cinq.
    (re.compile(r"\bavatar_(?:boots|legs|helmet|chest|gloves)\b"), "avatar_<pièce>"),
]


def sans_ansi(s: str) -> str:
    return ANSI.sub("", s)


def motif(msg: str) -> str:
    """La forme d'un message, indépendante de ses valeurs."""
    out = msg
    for rx, rep in JOKERS:
        out = rx.sub(rep, out)
    return out.strip()


def lire_log(chemin: Path, module: str | None) -> tuple[list[dict], int, int]:
    """Rend (groupes, lignes_lues, octets_lus). Groupes triés par gravité."""
    if not chemin.exists():
        return [], 0, 0
    brut = chemin.read_text(encoding="utf-8", errors="replace")
    lignes = brut.splitlines()
    groupes: "OrderedDict[tuple, dict]" = OrderedDict()
    for ligne in lignes:
        propre = sans_ansi(ligne).strip()
        if not propre:
            continue
        m = LIGNE.match(propre)
        if not m:
            continue
        mod = m.group("mod")
        if module and module.lower() not in mod.lower():
            continue
        msg = m.group("msg").strip()
        cle = (m.group("lvl"), mod, motif(msg))
        g = groupes.get(cle)
        if g is None:
            groupes[cle] = {
                "lvl": m.group("lvl"),
                "mod": mod,
                "n": 1,
                "premier": m.group("ts"),
                "dernier": m.group("ts"),
                "exemple": msg,
            }
        else:
            g["n"] += 1
            g["dernier"] = m.group("ts")
    ordre = {"ERROR": 0, "WARN": 1, "INFO": 2, "DEBUG": 3, "TRACE": 4}
    tri = sorted(groupes.values(), key=lambda g: (ordre.get(g["lvl"], 9), -g["n"]))
    return tri, len(lignes), len(brut)


def rendu_log(groupes: list[dict], lignes: int, octets: int, tout: bool = False) -> str:
    """Ce qui ALERTE ou ce qui SE RÉPÈTE d'abord ; le démarrage se replie.

    Un `INFO` vu une seule fois au boot (« plugin chargé », « config lue ») ne
    dit rien d'un symptôme : c'est une centaine de lignes de bruit. Ce qui porte
    du signal, c'est ce qui ALERTE ou ce qui RECOMMENCE — une boucle qui tourne
    mal se voit à son compte. Le reste est compté par module, et `--tout` le
    rouvre quand on cherche justement une ligne de démarrage.
    """
    if not groupes:
        return "LOG — aucune ligne exploitable (fichier absent ou vide)."

    def fenetre(g: dict) -> str:
        # L'HEURE compte sur un bug de cycle de vie : « 68 os morts » 3 ms après
        # une reconstruction ne dit pas la même chose qu'isolé.
        f = g["premier"][11:23]
        return f if g["dernier"] == g["premier"] else f + f"→{g['dernier'][11:23]}"

    saillants, boot = [], []
    for g in groupes:
        if tout or g["lvl"] in ("ERROR", "WARN") or g["n"] > 1:
            saillants.append(g)
        else:
            boot.append(g)

    out = [
        f"LOG — {lignes} lignes / {octets} o → {len(groupes)} motifs "
        f"({len(saillants)} saillants, {len(boot)} de démarrage repliés)"
    ]
    for g in saillants:
        court = g["mod"].split("::")[-1]
        # Une alerte mérite sa phrase entière ; un INFO récurrent, son idée.
        largeur = 200 if g["lvl"] in ("ERROR", "WARN") else 100
        out.append(f"  [{g['lvl']:5}] x{g['n']:<3} {fenetre(g)} {court}: {g['exemple'][:largeur]}")
    if boot:
        mods = sorted({g["mod"].split("::")[-1] for g in boot})
        out.append(
            f"  … {len(boot)} lignes de démarrage repliées sur {len(mods)} modules "
            f"(--tout pour les ouvrir)"
        )
    return "\n".join(out)


def lire_capteurs(racine: Path) -> tuple[list[dict], list[dict]]:
    """Rend (alertes, tous). Un capteur sans `severity` n'est pas une alerte."""
    alertes, tous = [], []
    for f in sorted(racine.glob("forgia2_*.json")):
        try:
            d = json.loads(f.read_text(encoding="utf-8", errors="replace"))
        except Exception as e:
            alertes.append({"id": f.stem, "severity": "illisible", "next_step": str(e)[:120]})
            continue
        if not isinstance(d, dict):
            continue
        e = {
            "id": d.get("id", f.stem),
            "severity": d.get("severity", "—"),
            "next_step": str(d.get("next_step", "-"))[:200],
        }
        tous.append(e)
        if e["severity"] not in ("info", "ok", "—"):
            alertes.append(e)
    return alertes, tous


def rendu_capteurs(alertes: list[dict], tous: list[dict]) -> str:
    out = [f"CAPTEURS — {len(tous)} lus, {len(alertes)} en alerte"]
    if not alertes:
        out.append("  tous au vert (severity info/ok)")
    for a in alertes:
        out.append(f"  [{a['severity']}] {a['id']} → {a['next_step']}")
    return "\n".join(out)


def main() -> int:
    p = argparse.ArgumentParser(description="Digest des logs et capteurs Forgia.")
    p.add_argument("quoi", choices=["log", "sensors", "all"], nargs="?", default="all")
    p.add_argument("--file", type=Path, default=LOG_DEFAUT)
    p.add_argument("--module", help="ne garder que ce module (ex: avatar, cosmetics)")
    p.add_argument("--tout", action="store_true", help="ouvrir les lignes de démarrage repliées")
    a = p.parse_args()

    morceaux = []
    if a.quoi in ("log", "all"):
        groupes, lignes, octets = lire_log(a.file, a.module)
        morceaux.append(rendu_log(groupes, lignes, octets, a.tout))
    if a.quoi in ("sensors", "all"):
        alertes, tous = lire_capteurs(RACINE)
        morceaux.append(rendu_capteurs(alertes, tous))

    digest = "\n\n".join(morceaux)
    print(digest)
    return 0


if __name__ == "__main__":
    sys.exit(main())
