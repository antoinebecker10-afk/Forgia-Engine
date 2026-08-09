#!/usr/bin/env python3
"""Banc de non-regression de la recherche grepai — hybride OFF vs ON.

Chaque cas porte une reponse ATTENDUE (le fichier qui repond vraiment).
Le verdict est binaire : le fichier attendu est-il dans le top-3 ?
On ne juge JAMAIS sur le score — mesure du 2026-08-09 : une question absurde
(recette de tarte aux pommes) scorait 0,586 quand une bonne reponse plafonne
a 0,670. Le score ne discrimine pas ; le fichier rendu, si.

[!] AVANT DE LANCER : arreter le watcher (`grepai watch --stop`), sinon ce banc
    MENT. Mesure le 2026-08-09 : le watcher garde la config EN MEMOIRE depuis
    son demarrage et reecrit le fichier ENTIER a chaque mise a jour de
    `last_index_time` — il ecrase donc le basculement OFF/ON en pleine mesure.
    Relancer le watcher apres (`grepai watch --background`).
"""
import json, os, shutil, subprocess, sys

PROJ = r"c:/Users/Antoi/Desktop/Forgia Rewrite"
CFG  = os.path.join(PROJ, ".grepai/config.yaml")

# (requete, fragment attendu dans le chemin d'un des 3 premiers resultats, famille)
CAS = [
    ("prevent player spawning inside prop collider clearance", "decor.rs",            "EN code"),
    ("arena backdrop render to texture diorama props",         "arena_backdrop.rs",   "EN code"),
    ("cosmetics ownership purchase and equip",                 "cosmetics.rs",        "EN code"),
    ("window mode borderless persisted user settings",         "pause_menu.rs",       "EN code"),
    ("foot inverse kinematics locomotion",                     "foot_ik",             "EN code"),
    ("weapon viewmodel sprite pixel art quad",                 "sprite",              "EN code"),
    ("enemy_grunt hp speed vision genome",                     "enemy_grunt.toml",    "identifiants"),
    ("roguelite cosmetics catalogue shards",                   "cosmetics",           "identifiants"),
    ("toon shader post process outline",                       "toon",                "shader"),
    ("no hardcode rule genome TOML definition layer",          "no-hardcode",         "regles"),
    ("story gate mechanical check before DONE",                "story-done-gate",     "regles"),
    ("la fenetre se decide sur disque, borderless par defaut", "pause_menu.rs",       "FR (piege)"),
    ("regle de degagement autour des points d apparition",     "spawn-clearance",     "FR"),
    ("comment le fond du menu choisit ses props",              "arena_backdrop.rs",   "FR"),
    ("recette de tarte aux pommes et caramel beurre sale",     None,                  "SANS reponse"),
]

def top3(q):
    r = subprocess.run(["grepai", "search", q, "-j", "-c", "-n", "3"],
                       cwd=PROJ, capture_output=True, text=True, timeout=90)
    try:
        return [x["file_path"] for x in json.loads(r.stdout or "[]")]
    except Exception:
        return []

def passe(nom, cfg_src):
    shutil.copyfile(cfg_src, CFG)
    res = {}
    for q, attendu, fam in CAS:
        hits = top3(q)
        if attendu is None:                 # cas sans reponse : on note ce qui sort
            res[q] = (None, hits)
        else:
            res[q] = (any(attendu.lower() in h.lower() for h in hits), hits)
    return res

import re, pathlib
BAK = os.path.join(PROJ, ".grepai/config.yaml.bak-2026-08-09")
# Fabrique une copie ON distincte, pour ne jamais copier un fichier sur lui-meme.
HYB = os.path.join(PROJ, ".grepai/config.yaml.on-tmp")
pathlib.Path(HYB).write_text(
    re.sub(r"(hybrid:\s*\n\s*enabled:\s*)false", r"\1true",
           pathlib.Path(BAK).read_text(encoding="utf-8"), count=1),
    encoding="utf-8")

try:
    off = passe("OFF", BAK)
    on = passe("ON", HYB)
finally:
    # QUOI QU'IL ARRIVE la config repart en hybride ON : un banc de test ne doit
    # jamais laisser l'outil dans l'etat degrade qu'il servait a mesurer.
    shutil.copyfile(HYB, CFG)
    os.remove(HYB)

print(f"{'famille':<14}{'requete':<50}{'OFF':>5}{'ON':>5}  verdict")
print("-" * 92)
gain = perte = 0
for q, attendu, fam in CAS:
    o, _ = off[q]; w, on_hits = on[q]
    if attendu is None:
        print(f"{fam:<14}{q[:48]:<50}{'—':>5}{'—':>5}  top1 ON: {os.path.basename(on_hits[0]) if on_hits else 'rien'}")
        continue
    so = "✓" if o else "✗"; sw = "✓" if w else "✗"
    v = ""
    if w and not o: v = "GAIN"; gain += 1
    elif o and not w: v = "PERTE !!"; perte += 1
    print(f"{fam:<14}{q[:48]:<50}{so:>5}{sw:>5}  {v}")

tot = sum(1 for _, a, _ in CAS if a is not None)
no = sum(1 for q, a, _ in CAS if a is not None and off[q][0])
nw = sum(1 for q, a, _ in CAS if a is not None and on[q][0])
print("-" * 92)
print(f"  reussite OFF : {no}/{tot}     reussite ON : {nw}/{tot}     gains {gain}  ·  PERTES {perte}")
sys.exit(1 if perte else 0)
