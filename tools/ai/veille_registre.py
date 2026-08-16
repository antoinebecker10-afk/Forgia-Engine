#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Registre de veille Forgia — la source de verite, et le mecanisme anti-redondance.

Pourquoi ce fichier existe
--------------------------
La veille precedente ecrivait un rapport date par jour (`docs/veille/veille-AAAA-MM-JJ.md`).
Consequence mecanique : la meme release Bevy reapparait dans le rapport du lundi,
du mardi et du mercredi, parce qu'aucun fichier ne connait le contenu des autres.
Un registre append-only avec un identifiant stable par entree supprime la classe
entiere : une nouvelle deja consignee ne peut plus etre reconsignee.

L'identifiant se derive de la SOURCE (URL normalisee), pas du titre — deux
resumes differents du meme billet sont la meme nouvelle. Sans URL, on retombe
sur `axe + titre normalise`.

Le registre ne sait PAS ce qui a ete envoye sur Telegram : cet etat vit dans
`.claude/veille-pousse.json`, hors versionnement. Un marqueur d'envoi dans le
fichier versionne salirait l'arbre de travail a chaque ouverture de session,
ce que la regle multi-terminal interdit (le standup lit `git status`).

Commandes
---------
    python tools/ai/veille_registre.py ajouter          # lit un JSON sur stdin
    python tools/ai/veille_registre.py lister [--axe bevy] [--depuis 2026-08-01]
    python tools/ai/veille_registre.py nouveau          # ce qui n'a jamais ete pousse
    python tools/ai/veille_registre.py marquer ID [ID...]
    python tools/ai/veille_registre.py rendre           # regenere REGISTRE.md
    python tools/ai/veille_registre.py stats
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import sys
import unicodedata
from pathlib import Path

RACINE = Path(__file__).resolve().parents[2]
REGISTRE = RACINE / "docs" / "veille" / "registre.jsonl"
VUE = RACINE / "docs" / "veille" / "REGISTRE.md"
POUSSE = RACINE / ".claude" / "veille-pousse.json"

# Vocabulaire ferme. Un axe libre redevient vite un fourre-tout, et un registre
# qu'on ne peut plus filtrer ne se relit pas.
AXES = {
    "bevy": "Bevy",
    "moteurs-rust": "Moteurs Rust",
    "jeux-ia": "Jeux faits par IA",
    # Ajoute le 2026-08-13, a la demande d'Antoine. Defini SERRE pour ne pas
    # redevenir un fourre-tout : ce qu'on peut VOLER ET APPLIQUER (architecture,
    # perf moteur, pipeline artiste, game design qui retient et qui vend) —
    # jamais « ce qui est sorti », qui appartient aux trois autres axes.
    "patterns": "Patterns à voler",
}
IMPACTS = ("haut", "moyen", "bas")
ACTIONS = ("bloquant", "integrer", "surveiller", "ignorer")

_RANG_IMPACT = {v: i for i, v in enumerate(IMPACTS)}


# ── identite ────────────────────────────────────────────────────────────────
def _sans_accents(s: str) -> str:
    return "".join(c for c in unicodedata.normalize("NFKD", s) if not unicodedata.combining(c))


def normaliser_url(url: str) -> str:
    """Reduit une URL a ce qui l'identifie. Deux liens qui ne different que par
    leur tracking sont le meme lien."""
    u = url.strip().lower()
    u = re.sub(r"^https?://", "", u)
    u = re.sub(r"^www\.", "", u)
    u = re.split(r"[?#]", u)[0]
    return u.rstrip("/")


def normaliser_titre(t: str) -> str:
    t = _sans_accents(t).lower()
    return re.sub(r"[^a-z0-9]+", " ", t).strip()


def calculer_id(entree: dict) -> str:
    source = (entree.get("source") or "").strip()
    graine = normaliser_url(source) if source else f"{entree.get('axe','')}|{normaliser_titre(entree.get('titre',''))}"
    return hashlib.sha1(graine.encode("utf-8")).hexdigest()[:10]


# ── entrees/sorties ─────────────────────────────────────────────────────────
def charger() -> list[dict]:
    if not REGISTRE.exists():
        return []
    out = []
    for i, ligne in enumerate(REGISTRE.read_text(encoding="utf-8").splitlines(), 1):
        ligne = ligne.strip()
        if not ligne:
            continue
        try:
            out.append(json.loads(ligne))
        except json.JSONDecodeError as e:
            print(f"[registre] ligne {i} illisible, ignoree : {e}", file=sys.stderr)
    return out


def charger_pousse() -> dict:
    if POUSSE.exists():
        try:
            return json.loads(POUSSE.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            pass
    return {"ids": [], "dernier_envoi": None, "dernier_sha": None}


def ecrire_pousse(etat: dict) -> None:
    POUSSE.parent.mkdir(parents=True, exist_ok=True)
    POUSSE.write_text(json.dumps(etat, ensure_ascii=False, indent=1), encoding="utf-8")


# ── validation ──────────────────────────────────────────────────────────────
def valider(e: dict) -> str | None:
    for champ in ("axe", "titre", "quoi"):
        if not str(e.get(champ, "")).strip():
            return f"champ obligatoire vide : {champ}"
    if e["axe"] not in AXES:
        return f"axe inconnu '{e['axe']}' — attendus : {', '.join(AXES)}"
    if e.get("impact") not in IMPACTS:
        return f"impact invalide '{e.get('impact')}' — attendus : {', '.join(IMPACTS)}"
    if e.get("action") not in ACTIONS:
        return f"action invalide '{e.get('action')}' — attendues : {', '.join(ACTIONS)}"
    return None


# ── commandes ───────────────────────────────────────────────────────────────
def cmd_ajouter(args) -> int:
    brut = sys.stdin.read() if args.fichier is None else Path(args.fichier).read_text(encoding="utf-8")
    try:
        lot = json.loads(brut)
    except json.JSONDecodeError as e:
        print(f"JSON invalide : {e}", file=sys.stderr)
        return 2
    if isinstance(lot, dict):
        lot = [lot]

    existants = charger()
    connus = {e["id"] for e in existants}
    # Anti-doublon secondaire : le meme titre dans le meme axe, meme si la source
    # differe. C'est le cas « deux blogs racontent la meme release ».
    titres = {(e["axe"], normaliser_titre(e["titre"])) for e in existants}

    ajoutes, ignores, refuses = [], [], []
    aujourdhui = dt.date.today().isoformat()

    for e in lot:
        erreur = valider(e)
        if erreur:
            refuses.append((e.get("titre", "?"), erreur))
            continue
        e = {
            "id": calculer_id(e),
            "date": e.get("date") or aujourdhui,
            "axe": e["axe"],
            "titre": e["titre"].strip(),
            "quoi": e["quoi"].strip(),
            "impact": e["impact"],
            "action": e["action"],
            "source": (e.get("source") or "").strip(),
            "version": (e.get("version") or "").strip(),
        }
        cle_titre = (e["axe"], normaliser_titre(e["titre"]))
        if e["id"] in connus:
            ignores.append((e["titre"], "id deja au registre"))
            continue
        if cle_titre in titres:
            ignores.append((e["titre"], "titre deja au registre dans cet axe"))
            continue
        connus.add(e["id"])
        titres.add(cle_titre)
        ajoutes.append(e)

    if ajoutes:
        REGISTRE.parent.mkdir(parents=True, exist_ok=True)
        with REGISTRE.open("a", encoding="utf-8", newline="\n") as f:
            for e in ajoutes:
                f.write(json.dumps(e, ensure_ascii=False) + "\n")
        rendre()

    print(f"ajoutees {len(ajoutes)} · deja connues {len(ignores)} · refusees {len(refuses)}")
    for t, r in ignores:
        print(f"  = {t[:70]} ({r})")
    for t, r in refuses:
        print(f"  ! {t[:70]} — {r}", file=sys.stderr)
    return 1 if refuses else 0


def _trier(es: list[dict]) -> list[dict]:
    return sorted(es, key=lambda e: (e["date"], _RANG_IMPACT.get(e["impact"], 9)), reverse=True)


def cmd_corriger(args) -> int:
    """Reecrit une entree existante. Le registre est append-only pour les FAITS,
    pas pour les ERREURS : une entree fausse deja poussee doit pouvoir etre
    rectifiee, sinon le telephone garde la version fausse et le registre ment.
    L'entree corrigee est DEPOUSSEE, donc renvoyee au prochain recap."""
    entrees = charger()
    idx = next((i for i, e in enumerate(entrees) if e["id"] == args.id), None)
    if idx is None:
        print(f"id inconnu : {args.id}", file=sys.stderr)
        return 2

    brut = sys.stdin.read() if args.fichier is None else Path(args.fichier).read_text(encoding="utf-8")
    try:
        patch = json.loads(brut)
    except json.JSONDecodeError as e:
        print(f"JSON invalide : {e}", file=sys.stderr)
        return 2

    e = dict(entrees[idx])
    for k in ("titre", "quoi", "impact", "action", "source", "version", "axe", "date"):
        if k in patch:
            e[k] = patch[k]
    erreur = valider(e)
    if erreur:
        print(f"refuse : {erreur}", file=sys.stderr)
        return 1
    e["corrige_le"] = dt.date.today().isoformat()
    # L'identifiant NE bouge PAS meme si la source change : c'est la meme nouvelle.
    entrees[idx] = e

    with REGISTRE.open("w", encoding="utf-8", newline="\n") as f:
        for x in entrees:
            f.write(json.dumps(x, ensure_ascii=False) + "\n")
    rendre()

    etat = charger_pousse()
    if args.id in etat.get("ids", []):
        etat["ids"] = [i for i in etat["ids"] if i != args.id]
        ecrire_pousse(etat)
        print(f"corrigee {args.id} — depoussee, elle repartira au prochain recap")
    else:
        print(f"corrigee {args.id}")
    return 0


def cmd_lister(args) -> int:
    es = charger()
    if args.axe:
        es = [e for e in es if e["axe"] == args.axe]
    if args.depuis:
        es = [e for e in es if e["date"] >= args.depuis]
    if args.json:
        print(json.dumps(_trier(es), ensure_ascii=False, indent=1))
        return 0
    for e in _trier(es):
        print(f"{e['id']}  {e['date']}  [{e['axe']:<12}] {e['impact']:<5} {e['action']:<10} {e['titre']}")
    print(f"— {len(es)} entree(s)")
    return 0


def cmd_nouveau(args) -> int:
    deja = set(charger_pousse().get("ids", []))
    neuf = _trier([e for e in charger() if e["id"] not in deja])
    print(json.dumps(neuf, ensure_ascii=False, indent=1) if args.json
          else "\n".join(f"{e['id']}  [{e['axe']}] {e['titre']}" for e in neuf))
    return 0


def cmd_marquer(args) -> int:
    etat = charger_pousse()
    ids = set(etat.get("ids", []))
    connus = {e["id"] for e in charger()}
    inconnus = [i for i in args.ids if i not in connus]
    ids.update(i for i in args.ids if i in connus)
    etat["ids"] = sorted(ids)
    etat["dernier_envoi"] = dt.datetime.now().astimezone().isoformat(timespec="seconds")
    ecrire_pousse(etat)
    print(f"marquees {len(args.ids) - len(inconnus)} · inconnues {len(inconnus)}")
    return 0


def rendre() -> None:
    """Regenere la vue humaine. Elle est DERIVEE — ne jamais l'editer a la main."""
    es = charger()
    lignes = [
        "# Registre de veille — Forgia",
        "",
        "> ⚙️ **Vue generee par `python tools/ai/veille_registre.py rendre`. Ne pas editer a la main.**",
        "> La source de verite est [`registre.jsonl`](registre.jsonl) — append-only, un identifiant",
        "> stable par entree derive de son URL. C'est ce qui empeche la meme nouvelle d'etre",
        "> consignee deux fois, et le recap Telegram de la reannoncer.",
        "",
        f"**{len(es)} entrees** · derniere mise a jour {dt.date.today().isoformat()}",
        "",
    ]
    if not es:
        lignes += ["_Registre vide. Alimente-le avec `/ia-veille`._", ""]
    for axe, nom in AXES.items():
        lot = _trier([e for e in es if e["axe"] == axe])
        lignes += [f"## {nom} — {len(lot)}", ""]
        if not lot:
            lignes += ["_Rien encore._", ""]
            continue
        lignes += ["| Date | Sujet | Impact | Action |", "| --- | --- | --- | --- |"]
        for e in lot:
            titre = f"[{e['titre']}]({e['source']})" if e["source"] else e["titre"]
            v = f" `{e['version']}`" if e.get("version") else ""
            lignes.append(f"| {e['date']} | **{titre}**{v}<br>{e['quoi']} | {e['impact']} | {e['action']} |")
        lignes.append("")
    VUE.parent.mkdir(parents=True, exist_ok=True)
    VUE.write_text("\n".join(lignes), encoding="utf-8", newline="\n")


def cmd_rendre(args) -> int:
    rendre()
    print(f"vue regeneree : {VUE.relative_to(RACINE)}")
    return 0


def cmd_stats(args) -> int:
    es = charger()
    pousses = set(charger_pousse().get("ids", []))
    print(f"entrees        {len(es)}")
    print(f"jamais poussee {len([e for e in es if e['id'] not in pousses])}")
    for axe, nom in AXES.items():
        lot = [e for e in es if e["axe"] == axe]
        hauts = len([e for e in lot if e["impact"] == "haut"])
        print(f"  {nom:<20} {len(lot):>3}  (impact haut : {hauts})")
    etat = charger_pousse()
    print(f"dernier envoi  {etat.get('dernier_envoi') or 'jamais'}")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    s = p.add_subparsers(dest="cmd", required=True)

    a = s.add_parser("ajouter", help="ajoute un lot JSON (stdin par defaut), en dedupliquant")
    a.add_argument("--fichier")
    a.set_defaults(fn=cmd_ajouter)

    c = s.add_parser("corriger", help="rectifie une entree existante et la depousse")
    c.add_argument("--id", required=True)
    c.add_argument("--fichier")
    c.set_defaults(fn=cmd_corriger)

    l = s.add_parser("lister", help="liste le registre")
    l.add_argument("--axe", choices=list(AXES))
    l.add_argument("--depuis", help="AAAA-MM-JJ")
    l.add_argument("--json", action="store_true")
    l.set_defaults(fn=cmd_lister)

    n = s.add_parser("nouveau", help="entrees jamais envoyees sur Telegram")
    n.add_argument("--json", action="store_true")
    n.set_defaults(fn=cmd_nouveau)

    m = s.add_parser("marquer", help="marque des entrees comme envoyees")
    m.add_argument("ids", nargs="+")
    m.set_defaults(fn=cmd_marquer)

    s.add_parser("rendre", help="regenere REGISTRE.md").set_defaults(fn=cmd_rendre)
    s.add_parser("stats", help="compte par axe").set_defaults(fn=cmd_stats)

    args = p.parse_args()
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
