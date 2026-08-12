#!/usr/bin/env python3
"""phase0_check.py — une run, un verdict par story ouverte.

Les 4 stories ouvertes (696, 697, 698, 699) se vérifient toutes dans les capteurs
déjà écrits par le jeu. Ce script les rassemble et applique, pour chacune, SON
critère d'acceptation — pas la valeur brute.

    python tools/ai/phase0_check.py

Trois verdicts, jamais deux :

    PASS     le critère est atteint, mesuré
    ECHEC    le critère n'est pas atteint, et on avait de quoi le mesurer
    AVEUGLE  on n'avait PAS de quoi mesurer — ni vert, ni rouge

Le troisième est le plus important, et c'est la leçon de la journée : un contrôle
qui n'a rien mesuré ne doit jamais se déclarer vert (`map-design-patterns.md` §13).
Un capteur figé rend AVEUGLE, pas PASS.
"""

import json
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PASS, FAIL, BLIND = "PASS", "ECHEC", "AVEUGLE"

# Un fichier est périmé **relativement à la dernière run**, pas dans l'absolu :
# on ne sait pas quand l'utilisateur lance ce script après avoir joué. On compare
# donc chaque capteur au PLUS FRAIS du lot — tous ceux d'une même run sont écrits
# à la même seconde. 5 min de marge couvre un capteur à faible cadence.
#
# Un seuil absolu (première version : 120 s) déclarait périmés des fichiers vieux
# de 33 minutes qui venaient pourtant de la run — et rendait les 4 contrôles
# AVEUGLES. Le relatif isole exactement ce qu'on cherche : `forgia2_gamefeel.json`,
# figé depuis le 2026-07-21 quand les autres datent de l'instant.
STALE_MARGIN_SECS = 300


def load(name):
    """(données, âge en secondes) — None si absent."""
    p = ROOT / name
    if not p.exists():
        return None, None
    try:
        return json.loads(p.read_text(encoding="utf-8")), time.time() - p.stat().st_mtime
    except Exception as e:
        return {"_erreur": str(e)}, time.time() - p.stat().st_mtime


def g(d, *path, default=0):
    for k in path:
        if not isinstance(d, dict) or k not in d:
            return default
        d = d[k]
    return d


# ── Les 4 contrôles ─────────────────────────────────────────────────────────


def check_696(S):
    """Le hitstop se déclenche-t-il, et son capteur suit-il la run ?"""
    d, age = S["gamefeel"]
    if d is None:
        return BLIND, "forgia2_gamefeel.json absent — le producteur n'a jamais tourné"
    del age  # la fraîcheur est jugée globalement, au même titre que les autres
    c = g(d, "hitstop_counts", default={})
    total = sum(v for v in c.values() if isinstance(v, int))
    detail = (f"hit {c.get('hit',0)} · crit {c.get('crit',0)} · "
              f"kill {c.get('kill',0)} · multi {c.get('multikill',0)}")
    if total == 0:
        return FAIL, f"{detail} — zéro hitstop alors que le capteur suit la run : hypothèse (a)"
    return PASS, f"{detail} — dernier palier « {g(d,'last_tier',default='?')} »"


def check_697(S):
    """Une réaction élémentaire s'est-elle déclenchée ?"""
    d, age = S["elements"]
    if d is None:
        return BLIND, "forgia2_elements.json absent"
    r = g(d, "reactions", default={})
    total = sum(v for v in r.values() if isinstance(v, int))
    hits = g(d, "hits", default={})
    distincts = [k for k, v in hits.items() if isinstance(v, int) and v > 0]
    bursts = g(S["element_vfx"][0] or {}, "reaction_bursts")
    detail = (f"combustions {r.get('combustions',0)} · miasmas {r.get('miasmas',0)} · "
              f"surcharges {r.get('surcharges',0)} · bursts VFX {bursts} · "
              f"éléments ayant touché : {', '.join(distincts) or 'aucun'}")
    if len(distincts) < 2:
        return BLIND, (f"{detail} — moins de 2 éléments distincts ont touché, "
                       "aucune réaction n'était POSSIBLE. Ce n'est pas un échec.")
    if total == 0:
        return FAIL, (f"{detail} — 2+ éléments ont touché, aucune réaction. "
                      "Cf story-697 : alterner 2 armes SUR LA MÊME CIBLE, un boss de préférence.")
    if bursts == 0:
        return FAIL, f"{detail} — la logique part mais le VISUEL ne suit pas"
    return PASS, detail


def check_698(S):
    """Les morts produisent-elles un burst et un son ?"""
    morts = {
        "knockback.kill_pushes": g(S["knockback"][0] or {}, "kill_pushes"),
        "audio.kills": g(S["audio"][0] or {}, "kills"),
        "elements.executes": g(S["elements"][0] or {}, "executes"),
    }
    bursts = g(S["weapon_vfx"][0] or {}, "kill_bursts")
    ref = max(morts.values())
    detail = " · ".join(f"{k} {v}" for k, v in morts.items()) + f" · kill_bursts {bursts}"
    if ref == 0:
        return BLIND, f"{detail} — aucune mort enregistrée : rien à juger"
    # Le doute de la story : ces compteurs ne comptent peut-être pas la même chose.
    ecart = max(morts.values()) - min(morts.values())
    if ecart > max(2, ref * 0.25):
        return FAIL, (f"{detail} — les compteurs de morts DIVERGENT (écart {ecart}). "
                      "Établir d'abord lequel dit vrai : c'est le 1er AC de story-698.")
    if bursts == 0:
        return FAIL, f"{detail} — {ref} morts, zéro burst"
    return PASS, detail


def check_699(S):
    """Les capteurs disent-ils la vérité sur leur propre inactivité ?"""
    el, _ = S["elements"]
    sh, _ = S["sensor_health"]
    if el is None or sh is None:
        return BLIND, "capteurs elements / sensor_health absents"
    sev = g(el, "severity", default="?")
    r = sum(v for v in g(el, "reactions", default={}).values() if isinstance(v, int))
    hits = g(el, "hits", default={})
    distincts = sum(1 for v in hits.values() if isinstance(v, int) and v > 0)
    stalled = g(sh, "stalled", default=0)
    paths = g(sh, "stalled_paths", default=[])
    watched, live = g(sh, "watched"), g(sh, "live")

    attendu = "warn" if (distincts >= 2 and r == 0) else ("info" if distincts < 2 else "ok")
    detail = (f"elements.severity « {sev} » (attendu « {attendu} ») · "
              f"chien de garde : {watched} surveillés, {live} vivants, {stalled} arrêtés")
    if paths:
        detail += f" → {paths}"
    if sev != attendu:
        return FAIL, (f"{detail} — le capteur ne dit PAS ce que son contenu montre. "
                      "C'est le mensonge que story-699 corrige.")
    if watched == 0:
        return BLIND, detail + " — le chien de garde n'a rien balayé"
    return PASS, detail


# (id, titre, fonction, capteurs dont le contrôle DÉPEND)
#
# La 4ᵉ colonne n'est pas décorative : si l'un de ces fichiers est périmé, le
# contrôle ne peut pas rendre ECHEC — il rend AVEUGLE. Juger un jeu sur un fichier
# de la semaine dernière, c'est le défaut que cette journée entière a traqué.
CHECKS = [
    ("696", "Le hitstop se déclenche", check_696, ["gamefeel"]),
    ("697", "Une réaction élémentaire part", check_697, ["elements", "element_vfx"]),
    ("698", "Une mort produit burst + son", check_698,
     ["knockback", "audio", "weapon_vfx", "elements"]),
    ("699", "Les capteurs avouent leur inactivité", check_699,
     ["elements", "sensor_health"]),
]

FICHIERS = {
    "gamefeel": "forgia2_gamefeel.json",
    "elements": "forgia2_elements.json",
    "element_vfx": "forgia2_element_vfx.json",
    "weapon_vfx": "forgia2_weapon_vfx.json",
    "knockback": "forgia2_knockback.json",
    "audio": "forgia2_roguelite_audio.json",
    "sensor_health": "forgia2_sensor_health.json",
}


def main():
    S = {k: load(v) for k, v in FICHIERS.items()}

    ages = [a for _, a in S.values() if a is not None]
    ref = min(ages) if ages else 0.0  # le capteur le plus frais = l'heure de la run

    def perime(k):
        a = S[k][1]
        return a is not None and a > ref + STALE_MARGIN_SECS

    def duree(s):
        if s < 3600:
            return f"{s/60:.0f} min"
        if s < 86400:
            return f"{s/3600:.1f} h"
        return f"{s/86400:.0f} jours"

    # « L'artefact est la preuve, pas la source » (multi-terminal-coordination.md §5).
    # Un capteur tout frais produit par un binaire d'hier décrit le code d'hier —
    # et c'est invisible dans le JSON. On le dit ici, avant tout verdict.
    exes = sorted(ROOT.glob("target/*/forgia.exe"), key=lambda p: -p.stat().st_mtime)
    if exes:
        exe_age = time.time() - exes[0].stat().st_mtime
        plus_recent = max(
            (p.stat().st_mtime for p in (ROOT / "crates").rglob("*.rs")), default=0
        )
        if plus_recent > exes[0].stat().st_mtime:
            print(f"⚠️  BINAIRE PÉRIMÉ — {exes[0].name} date de {duree(exe_age)}, et des "
                  "sources .rs\n    sont plus récentes. Les capteurs ci-dessous "
                  "décrivent l'ANCIEN code.\n    Rebuild (`cargo run -p forgia`) avant "
                  "de conclure quoi que ce soit.\n")

    print(f"Capteurs lus : {len(ages)}/{len(FICHIERS)} · dernière run il y a {duree(ref)}")
    vieux = [f"{FICHIERS[k]} ({duree(S[k][1])})" for k in FICHIERS if perime(k)]
    if vieux:
        print("⚠️  Ces fichiers n'ont PAS été réécrits par la dernière run — le système\n"
              "    qui les produit ne tourne probablement plus :")
        for v in vieux:
            print(f"      · {v}")
    print()

    resume = {PASS: 0, FAIL: 0, BLIND: 0}
    for sid, titre, fn, depend in CHECKS:
        try:
            verdict, detail = fn(S)
        except Exception as e:  # un contrôle qui casse ne doit pas masquer les autres
            verdict, detail = BLIND, f"contrôle en erreur : {e}"

        # Un verdict négatif exige des données FRAÎCHES. Sinon on accuserait le jeu
        # pour un fichier que la run n'a pas réécrit.
        perimes = [f"{FICHIERS[k]} ({duree(S[k][1])})" for k in depend if perime(k)]
        absents = [FICHIERS[k] for k in depend if S[k][0] is None]
        if verdict == FAIL and (perimes or absents):
            verdict = BLIND
            manque = ", ".join(perimes + absents)
            detail = (f"{detail}\n              ⚠️  verdict RÉTROGRADÉ en AVEUGLE : "
                      f"{manque} — la run n'a pas réécrit ce fichier, "
                      "donc rien ici ne décrit ce que tu viens de jouer.")

        resume[verdict] += 1
        marque = {PASS: "✅", FAIL: "❌", BLIND: "🔍"}[verdict]
        print(f"{marque} story-{sid} — {titre}")
        print(f"     {verdict:8} {detail}\n")

    print(f"── {resume[PASS]} PASS · {resume[FAIL]} ECHEC · {resume[BLIND]} AVEUGLE ──")
    if resume[BLIND]:
        print("AVEUGLE n'est pas un échec : c'est « la run n'a pas produit la situation »."
              "\nVoir le protocole de run dans story-697 (alterner 2 armes sur un boss).")


if __name__ == "__main__":
    main()
