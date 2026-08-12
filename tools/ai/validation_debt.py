"""validation_debt.py — combien de travail livre n'a JAMAIS tourne ?

    python tools/ai/validation_debt.py

Le defaut que cet outil existe pour empecher a une date : le 2026-08-12, quatre
increments ont ete livres, testes et pousses sans qu'aucun ne tourne une seule fois
en jeu. Les tests headless etaient verts, clippy propre, les gates au vert — et
personne ne savait si un bot suivait reellement un chemin.

Le probleme n'est pas d'avoir code. C'est que RIEN NE MESURAIT cette dette : elle ne
levait aucune erreur, exactement comme un capteur a zero qui rapporte « ok »
(story-699). Un outil qui la compte la rend refusable.

DEUX NATURES D'INCREMENT, et c'est toute la nuance :

  FONDATION   — rien ne le consomme encore (une crate neuve, un type partage).
                Il peut s'empiler : le jeu ne peut pas en montrer l'effet.
  OBSERVABLE  — il change ce qui se passe a l'ecran ou dans un capteur.
                Celui-la DOIT tourner avant qu'on en empile un autre dessus.

L'outil ne devine pas la nature d'un increment. Il mesure ce qui est mesurable —
la fraicheur du binaire, celle des capteurs, les stories livrees non validees — et
laisse le jugement au lecteur, avec les chiffres sous les yeux.
"""

import json
import subprocess
import sys
import time
from pathlib import Path

RACINE = Path(__file__).resolve().parents[2]
EXE_CANDIDATS = [
    RACINE / "target" / "release-fast" / "forgia.exe",
    RACINE / "target" / "debug" / "forgia.exe",
    RACINE / "target" / "release" / "forgia.exe",
]
# Le seuil au-dela duquel une pile d'increments non valides devient un risque plutot
# qu'un retard. Trois, parce qu'au quatrieme on ne sait plus lequel a casse quoi.
SEUIL_STORIES_REVIEW = 3


def exe_le_plus_recent():
    """Le binaire reellement joue. `forgia`, jamais `forgia-game` : ce dernier
    produit un exe perime EN SILENCE (memoire du projet)."""
    presents = [(p.stat().st_mtime, p) for p in EXE_CANDIDATS if p.exists()]
    return max(presents) if presents else (None, None)


def sources_plus_recentes(seuil_mtime):
    """Les crates dont une source depasse le binaire. Par CRATE, pas par fichier :
    savoir que 40 fichiers ont bouge n'aide pas, savoir lesquels des 66 crates si."""
    crates = {}
    for f in (RACINE / "crates").rglob("*.rs"):
        if "target" in f.parts:
            continue
        try:
            m = f.stat().st_mtime
        except OSError:
            continue
        if m > seuil_mtime:
            try:
                nom = f.relative_to(RACINE / "crates").parts[0]
            except ValueError:
                continue
            if m > crates.get(nom, 0):
                crates[nom] = m
    return crates


def capteur_le_plus_recent():
    caps = list(RACINE.glob("forgia2_*.json")) + list(RACINE.glob("forgia_*.json"))
    presents = [(p.stat().st_mtime, p) for p in caps if p.exists()]
    return max(presents) if presents else (None, None)


def stories_en_review():
    """Les stories livrees dont la validation runtime n'a pas eu lieu.

    Lit les fichiers plutot que l'index : l'index est genere, et un index perime
    dirait exactement ce qu'on cherche a ne pas croire.
    """
    out = []
    for f in sorted((RACINE / "docs" / "stories").glob("story-*.md")):
        try:
            tete = f.read_text(encoding="utf-8", errors="replace")[:900]
        except OSError:
            continue
        for ligne in tete.splitlines():
            if "statut" in ligne.lower() and "REVIEW" in ligne:
                out.append((f.name, ligne.strip()[:110]))
                break
    return out


def age(mtime):
    if mtime is None:
        return "jamais"
    m = (time.time() - mtime) / 60.0
    return f"{m:.0f} min" if m < 120 else f"{m / 60.0:.1f} h"


def horodatage(mtime):
    return "—" if mtime is None else time.strftime("%d/%m %H:%M", time.localtime(mtime))


def branche():
    try:
        r = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            cwd=RACINE, capture_output=True, text=True, timeout=10,
        )
        return r.stdout.strip() or "?"
    except Exception:
        return "?"


def main():
    exe_mtime, exe_path = exe_le_plus_recent()
    cap_mtime, _ = capteur_le_plus_recent()
    review = stories_en_review()

    print(f"\nDETTE DE VALIDATION — branche {branche()}\n")

    if exe_mtime is None:
        print("  BINAIRE   aucun exe construit — rien n'a jamais pu tourner.")
        crates = {}
    else:
        crates = sources_plus_recentes(exe_mtime)
        print(f"  BINAIRE   {horodatage(exe_mtime)}  ({exe_path.parent.name})")

    print(f"  CAPTEURS  {horodatage(cap_mtime)}  (derniere run, il y a {age(cap_mtime)})")
    print()

    problemes = []

    # 1. Du code livre que le binaire ne contient pas.
    if crates:
        problemes.append(("binaire", len(crates)))
        print(f"  [{len(crates)}] crate(s) plus recentes que le binaire :")
        for nom, m in sorted(crates.items(), key=lambda kv: -kv[1]):
            print(f"        {nom:<32} {horodatage(m)}")
        print("        -> ce qui tourne NE CONTIENT PAS ces changements.")
        print("        -> `cargo build -p forgia` (jamais -p forgia-game : exe perime en silence)")
        print()

    # 2. Une run plus vieille que le code : les capteurs decrivent l'ancien monde.
    if exe_mtime and cap_mtime and cap_mtime < exe_mtime:
        problemes.append(("capteurs", 1))
        print("  [!] Les capteurs sont ANTERIEURS au binaire.")
        print("        -> aucun capteur ne decrit le code actuel ; ne rien conclure d'eux.")
        print()

    # 3. Du livre qui attend sa preuve.
    if len(review) >= SEUIL_STORIES_REVIEW:
        problemes.append(("review", len(review)))
        print(f"  [{len(review)}] stories en REVIEW — livrees, pas validees en jeu :")
        for nom, ligne in review:
            print(f"        {nom}")
        print(f"        -> au-dela de {SEUIL_STORIES_REVIEW}, on ne sait plus laquelle a casse quoi.")
        print()
    elif review:
        print(f"  [{len(review)}] story(s) en REVIEW — sous le seuil de {SEUIL_STORIES_REVIEW}, OK.")
        print()

    if not problemes:
        print("  OK — le binaire contient le code livre, et une run l'a vu tourner.\n")
        return 0

    print("  VERDICT : dette de validation. Une run d'arene la solde :")
    print("     1. cargo build -p forgia --profile release-fast")
    print("     2. jouer une arene — laisser les bots poursuivre AUTOUR des obstacles")
    print("     3. python tools/ai/forgia_digest.py all")
    print("     4. python tools/ai/phase0_check.py")
    print()
    print("  Ceci n'est pas un echec de build. C'est du travail qui n'a pas encore")
    print("  rencontre la realite — et qui coute d'autant plus cher a corriger qu'on")
    print("  aura empile dessus.\n")
    return 1


if __name__ == "__main__":
    sys.exit(main())
