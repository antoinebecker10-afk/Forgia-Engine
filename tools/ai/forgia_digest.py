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

# Retard du log sur les capteurs au-delà duquel il ne décrit plus la même
# partie. Large À DESSEIN : le log ne s'écrit qu'à chaque ligne émise, donc une
# session calme le laisse légitimement traîner de plusieurs minutes. Le cas réel
# qui a motivé ce contrôle était de 78 HEURES — on cherche l'abîme, pas l'écart.
RETARD_LOG_TOLERE_S = 1800.0

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


def retard_du_log(chemin: Path) -> float | None:
    """De combien le log est-il en retard sur le capteur le plus frais ?

    On compare le log aux CAPTEURS, pas à l'horloge système : quand rien ne
    tourne, tout est vieux et ce n'est pas une anomalie. Ce qui est anormal,
    c'est qu'une moitié du dispositif décrive une partie et l'autre moitié une
    partie d'il y a trois jours.
    """
    if not chemin.exists():
        return None
    capteurs = set(RACINE.glob("forgia_*.json")) | set(RACINE.glob("forgia2_*.json"))
    frais = max((f.stat().st_mtime for f in capteurs), default=0.0)
    if frais == 0.0:
        return None
    return frais - chemin.stat().st_mtime


def rendu_log(
    groupes: list[dict],
    lignes: int,
    octets: int,
    tout: bool = False,
    retard_s: float | None = None,
) -> str:
    """Ce qui ALERTE ou ce qui SE RÉPÈTE d'abord ; le démarrage se replie.

    Un `INFO` vu une seule fois au boot (« plugin chargé », « config lue ») ne
    dit rien d'un symptôme : c'est une centaine de lignes de bruit. Ce qui porte
    du signal, c'est ce qui ALERTE ou ce qui RECOMMENCE — une boucle qui tourne
    mal se voit à son compte. Le reste est compté par module, et `--tout` le
    rouvre quand on cherche justement une ligne de démarrage.
    """
    if not groupes:
        return "LOG — aucune ligne exploitable (fichier absent ou vide)."

    if retard_s is not None and retard_s > RETARD_LOG_TOLERE_S:
        # 🚨 Le défaut le plus coûteux du dispositif, et il ne se voit qu'ICI.
        # Le 2026-08-17 : capteurs de 0 minute, log de 78 HEURES. On lit les
        # deux dans le même digest, donc on lit le log comme s'il décrivait la
        # partie qu'on vient de jouer — alors qu'il décrit celle de mardi.
        #
        # Cause : `forgia2_run.log` n'est pas écrit par le jeu, c'est la
        # redirection de `run_debug.bat`. Lancer par `cargo run` à la main
        # donne les capteurs et AUCUN log, sans que rien ne le dise.
        h = retard_s / 3600.0
        entete = (
            f"⚠️  LOG PÉRIMÉ DE {h:.1f} h PAR RAPPORT AUX CAPTEURS — il ne décrit PAS "
            f"la partie que les capteurs décrivent.\n"
            f"    Ne rien conclure des lignes ci-dessous. Relancer par `run_debug.bat` "
            f"(seul chemin qui écrit le log)."
        )
    else:
        entete = None

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

    out = [entete] if entete else []
    out.append(
        f"LOG — {lignes} lignes / {octets} o → {len(groupes)} motifs "
        f"({len(saillants)} saillants, {len(boot)} de démarrage repliés)"
    )
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
    """Rend (alertes, tous). Un capteur sans `severity` n'est pas une alerte.

    🚨 Le motif couvre `forgia_*` ET `forgia2_*`. Il ne prenait que le second,
    et **30 capteurs étaient invisibles** — dont `forgia_bone_trace.json`
    (25 Ko, toute la hiérarchie d'os, écrit toutes les 2 s) et son
    `forgia_bone_trace_health.json`, qui criait `desync 8/8` depuis des jours
    sans que personne ne le voie.

    Le 2026-08-16, une session entière a diagnostiqué une pose de bras à l'œil
    sur une capture d'écran pendant que le relevé par os s'écrivait à côté. Pire :
    l'absence de `forgia2_hitscan.json` a été lue comme « aucun tir n'a été
    résolu » — alors que le fichier s'appelle `forgia_hitscan.json` et était
    frais. Un outil de découverte qui ne montre qu'une partie des sources ne
    rend pas la lecture incomplète : il la rend FAUSSE, parce qu'on conclut de
    l'absence.

    Les deux préfixes cohabitent parce que la V1 écrivait `forgia_` et la V2
    `forgia2_`. Renommer 30 fichiers casserait les capteurs qui les citent ;
    élargir le motif ne casse rien.
    """
    alertes, tous = [], []
    fichiers = sorted(set(racine.glob("forgia_*.json")) | set(racine.glob("forgia2_*.json")))
    # Le plus frais du lot sert de « maintenant ». Se fier à l'horloge système
    # ferait passer TOUS les capteurs pour périmés quand le jeu ne tourne pas,
    # ce qui noierait le seul cas utile : celui qui décroche des autres.
    frais = max((f.stat().st_mtime for f in fichiers), default=0.0)
    for f in fichiers:
        try:
            d = json.loads(f.read_text(encoding="utf-8", errors="replace"))
        except Exception as e:
            alertes.append({"id": f.stem, "severity": "illisible", "next_step": str(e)[:120]})
            continue
        if not isinstance(d, dict):
            continue
        e = {
            "id": d.get("id", f.stem),
            "severity": severite_de(d),
            "next_step": str(d.get("next_step", "-"))[:200],
            "retard_s": frais - f.stat().st_mtime,
            # 🚨 Un `.previous` est une COPIE conservée d'un état passé, pas un
            # capteur vivant. Le nom le dit ; encore faut-il le lire. Sans ça,
            # `forgia2_crash.previous.json` — la panique de la veille, déjà
            # corrigée — remonte en `critical` chaque fois qu'on dit « regarde »,
            # et envoie enquêter sur un bug qui n'existe plus.
            "artefact": ".previous" in f.name,
        }
        tous.append(e)
        if e["artefact"]:
            continue
        if e["severity"] == "sans_verdict":
            # Groupé plus bas, quelle que soit sa fraîcheur : un capteur qui ne
            # se prononce pas ne devient pas plus parlant en étant vieux, et le
            # décorer le sortirait de son groupe pour rien. Le décorer était ma
            # première version — elle rendait 14 lignes individuelles là où le
            # digest existe justement pour n'en rendre qu'une.
            alertes.append(e)
        elif e["severity"] not in ("info", "ok"):
            # 🚨 Une ALERTE périmée trompe autant qu'un vert périmé — et je l'ai
            # laissée passer en écrivant ce contrôle il y a une heure : le test
            # de fraîcheur ne portait que sur `info`/`ok`. Un `critical` de la
            # veille se lisait donc comme un incendie en cours.
            #
            # On ne l'efface pas — son message reste utile — on lui retire son
            # URGENCE, qui est la partie fausse.
            if e["retard_s"] > RETARD_TOLERE_S:
                e["severity"] = f"{e['severity']}·perime"
                e["next_step"] = (
                    f"⚠ DATE {e['retard_s'] / 3600:.1f} h AVANT le reste du lot — "
                    f"cette alerte ne decrit PAS la partie en cours. "
                    f"{e['next_step']}"
                )
            alertes.append(e)
        elif e["retard_s"] > RETARD_TOLERE_S:
            # 🚨 Un capteur figé se lit comme actuel. C'est la panne la plus
            # coûteuse du dispositif : on fonde un diagnostic sur une valeur
            # d'il y a une semaine sans que rien ne l'indique. Mesuré le
            # 2026-08-17 : les 4 `rex_bones` avaient 7 jours de retard et
            # passaient pour verts.
            e["severity"] = "perime"
            e["next_step"] = (
                f"CAPTEUR FIGE : {e['retard_s'] / 3600:.1f} h de retard sur le plus "
                f"frais du lot — sa valeur se lit comme actuelle et ne l'est pas. "
                f"Ne fonder aucun diagnostic dessus avant d'avoir relance le jeu."
            )
            alertes.append(e)
    return alertes, tous


# Au-delà de quoi un capteur a décroché du reste du lot. Cinq minutes : le plus
# lent des capteurs Forgia écrit toutes les 2 s, et un run dure des dizaines de
# minutes — un écart de cet ordre ne peut pas être une simple différence de
# cadence.
RETARD_TOLERE_S = 300.0


def severite_de(d: dict) -> str:
    """La sévérité du capteur, y compris quand elle est imbriquée ou absente.

    🚨 Deux failles corrigées le 2026-08-17, qui rendaient le digest RASSURANT.

    1. `severity` absente valait `"—"`, et `"—"` était classé non-alerte. Un
       capteur qui ne se prononce pas était donc compté au vert — alors qu'on
       ne sait précisément rien de lui. « Zéro mesuré n'est pas vert, c'est
       aveugle » (map-design-patterns §13) : ici, c'est le lecteur du capteur
       qui violait la règle, pas le capteur.
    2. Certains capteurs portent leur verdict dans un sous-objet
       (`budget.severity`) — invisible au niveau racine, donc jamais remonté.

    Un outil de découverte qui répond « tout va bien » sur une tranche qu'il ne
    sait pas lire ne rend pas la lecture incomplète : il la rend FAUSSE.
    """
    s = d.get("severity")
    if isinstance(s, str) and s:
        return s
    # Une severity imbriquée d'un cran, la seule profondeur observée.
    pires = [
        v["severity"]
        for v in d.values()
        if isinstance(v, dict) and isinstance(v.get("severity"), str) and v["severity"]
    ]
    for niveau in ("critical", "error", "warn", "warning"):
        if niveau in pires:
            return niveau
    if pires:
        return pires[0]
    return "sans_verdict"


def rendu_capteurs(alertes: list[dict], tous: list[dict]) -> str:
    muets = [a for a in alertes if a["severity"] == "sans_verdict"]
    perimes = [a for a in alertes if a["severity"] == "perime"]
    vrais = [
        a
        for a in alertes
        if a["severity"] != "perime" and not a["severity"].startswith("sans_verdict")
    ]
    artefacts = sum(1 for a in tous if a.get("artefact"))
    out = [
        f"CAPTEURS — {len(tous)} lus, {len(vrais)} en alerte, "
        f"{len(perimes)} figes, {len(muets)} sans verdict"
        + (f", {artefacts} artefact(s) .previous ecarte(s)" if artefacts else "")
    ]
    if not alertes:
        out.append("  tous au vert (severity info/ok, aucun retard)")
    for a in vrais:
        out.append(f"  [{a['severity']}] {a['id']} → {a['next_step']}")
    if perimes:
        # Groupés, comme le reste du digest groupe les répétitions : vingt fois
        # la même phrase n'apprend rien de plus qu'une fois avec le compte, et
        # noie les alertes vivantes juste au-dessus.
        pire = max(perimes, key=lambda a: a["retard_s"])
        noms = ", ".join(
            f"{a['id']}({a['retard_s'] / 3600:.0f}h)"
            for a in sorted(perimes, key=lambda a: -a["retard_s"])[:10]
        )
        reste = f" (+{len(perimes) - 10})" if len(perimes) > 10 else ""
        out.append(
            f"  [perime] {len(perimes)} capteur(s) FIGES, jusqu'a "
            f"{pire['retard_s'] / 3600:.0f} h de retard — leur valeur se lit comme "
            f"actuelle et ne l'est pas. Ne fonder aucun diagnostic dessus : "
            f"{noms}{reste}"
        )
    if muets:
        # Groupés : ils n'ont rien à dire individuellement, mais leur NOMBRE
        # dit combien de la surface n'est pas jugée.
        noms = ", ".join(sorted(a["id"] for a in muets)[:12])
        reste = f" (+{len(muets) - 12})" if len(muets) > 12 else ""
        out.append(
            f"  [sans_verdict] {len(muets)} capteur(s) ne publient AUCUNE severity — "
            f"on ne sait pas s'ils vont bien : {noms}{reste}"
        )
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
        morceaux.append(rendu_log(groupes, lignes, octets, a.tout, retard_du_log(a.file)))
    if a.quoi in ("sensors", "all"):
        alertes, tous = lire_capteurs(RACINE)
        morceaux.append(rendu_capteurs(alertes, tous))

    digest = "\n\n".join(morceaux)
    print(digest)
    return 0


if __name__ == "__main__":
    sys.exit(main())
