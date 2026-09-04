#!/usr/bin/env python3
"""Pilote le jeu en marche depuis un terminal, via les commandes BRP du harnais.

Le scénario (`run_brp_scenario.py`) rejoue une preuve écrite d'avance ; CE
script sert au diagnostic à la main : appuyer une touche, tourner la tête,
regarder ce qui bouge pendant qu'on joue.

    python tools/ai/brp.py snapshot                 # photographie compacte
    python tools/ai/brp.py watch --seconds 5        # une ligne par échantillon
    python tools/ai/brp.py key KeyR                 # tape R (recharge)
    python tools/ai/brp.py key ShiftLeft --hold     # tient Shift
    python tools/ai/brp.py look --yaw 90            # tourne de 90 deg a la SOURIS
    python tools/ai/brp.py act sprint_forward --frames 60
    python tools/ai/brp.py release-all              # filet : tout relacher
    python tools/ai/brp.py call bevy/list           # n'importe quelle methode BRP

Prerequis : le jeu tourne avec `cargo forgia-dev` (ou `cargo run -p forgia
--features dev-brp`). Sans ca, tout echoue avec « connexion refusee ».
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request

BRP_URL = "http://127.0.0.1:15702/"
CONSEIL_HORS_LIGNE = (
    "BRP injoignable sur 127.0.0.1:15702 — le jeu n'est pas lance, "
    "ou il est lance SANS la feature dev-brp (`cargo forgia-dev`)."
)


def brp(method: str, params: dict | None = None) -> dict:
    payload: dict = {"jsonrpc": "2.0", "id": 1, "method": method}
    if params is not None:
        payload["params"] = params
    request = urllib.request.Request(
        BRP_URL,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=3) as response:
            body = json.load(response)
    except (urllib.error.URLError, OSError) as error:
        raise SystemExit(f"{CONSEIL_HORS_LIGNE}\n  ({error})") from error
    if "error" in body:
        raise SystemExit(f"BRP {method}: {body['error']}")
    return body["result"]


def chemin(valeur, route: str):
    """`locomotion.speed_mps` -> la valeur, ou None si la route casse."""
    for morceau in route.split("."):
        if isinstance(valeur, list):
            try:
                valeur = valeur[int(morceau)]
                continue
            except (ValueError, IndexError):
                return None
        if not isinstance(valeur, dict) or morceau not in valeur:
            return None
        valeur = valeur[morceau]
    return valeur


def ligne_compacte(snapshot: dict) -> str:
    """Une ligne lisible d'un coup d'oeil — le reste est dans `--json`."""
    position = chemin(snapshot, "player.position") or []
    locomotion = snapshot.get("locomotion") or {}
    animation = (snapshot.get("avatar_animation") or [{}])[0]
    entrees = snapshot.get("inputs") or {}
    suivi = snapshot.get("path_follower") or {}
    morceaux = [
        f"mode={snapshot.get('game_mode')}",
        "pos=({:.1f},{:.1f},{:.1f})".format(*position) if len(position) == 3 else "pos=?",
        "v={:.2f}".format(locomotion.get("speed_mps") or 0.0),
        "sol={}".format(1 if locomotion.get("grounded") else 0),
        "etat={}".format(animation.get("requested_state") or "?"),
        "clip={}".format(animation.get("playing_clip") or "?"),
        "touches={}".format(",".join(entrees.get("held_keys") or []) or "-"),
    ]
    if entrees.get("held_mouse"):
        morceaux.append("souris={}".format(entrees["held_mouse"]))
    if entrees.get("failure"):
        morceaux.append("PANNE={}".format(entrees["failure"]))
    if suivi.get("active"):
        morceaux.append(
            "chemin={}/{} cap={:.0f}deg".format(
                suivi.get("waypoint"),
                suivi.get("waypoints"),
                suivi.get("heading_error_deg") or 0.0,
            )
        )
    if suivi.get("failure"):
        morceaux.append("ECHEC_CHEMIN={}".format(suivi["failure"]))
    return " ".join(morceaux)


def commande_snapshot(args) -> int:
    snapshot = brp("forgia.scenario.snapshot")
    if args.field:
        print(json.dumps(chemin(snapshot, args.field), ensure_ascii=False, indent=2))
    elif args.json:
        print(json.dumps(snapshot, ensure_ascii=False, indent=2))
    else:
        print(ligne_compacte(snapshot))
    return 0


def commande_watch(args) -> int:
    """Boucle d'observation : c'est le mode « regarde ce qui se passe ».

    Elle imprime aussi combien d'echantillons elle a pris : un `watch` qui rend
    trois lignes sur cinq secondes dit quelque chose sur la frame, pas seulement
    sur le jeu.
    """
    fin = time.monotonic() + args.seconds
    pris = 0
    precedent = None
    try:
        while time.monotonic() < fin:
            snapshot = brp("forgia.scenario.snapshot")
            ligne = (
                json.dumps(chemin(snapshot, args.field), ensure_ascii=False)
                if args.field
                else ligne_compacte(snapshot)
            )
            if not args.changes_only or ligne != precedent:
                print(f"{time.strftime('%H:%M:%S')} {ligne}", flush=True)
                precedent = ligne
            pris += 1
            time.sleep(args.interval)
    except KeyboardInterrupt:
        pass
    print(f"— {pris} echantillons en {args.seconds:.0f}s (intervalle {args.interval}s)")
    return 0


def commande_key(args) -> int:
    if args.release:
        etat = "release"
    elif args.hold:
        etat = "press"
    else:
        etat = "tap"
    params: dict = {"key": args.key, "state": etat}
    if etat == "tap":
        params["frames"] = args.frames
    print(json.dumps(brp("forgia.scenario.key", params), ensure_ascii=False))
    return 0


def commande_look(args) -> int:
    params = {"yaw_deg": args.yaw, "pitch_deg": args.pitch, "frames": args.frames}
    print(json.dumps(brp("forgia.scenario.look", params), ensure_ascii=False))
    return 0


def commande_act(args) -> int:
    params = {"action": args.action, "frames": args.frames}
    print(json.dumps(brp("forgia.scenario.act", params), ensure_ascii=False))
    return 0


def commande_stop(_args) -> int:
    print(json.dumps(brp("forgia.scenario.stop"), ensure_ascii=False))
    return 0


def commande_release_all(_args) -> int:
    print(json.dumps(brp("forgia.scenario.release_all"), ensure_ascii=False))
    return 0


def commande_call(args) -> int:
    params = json.loads(args.params) if args.params else None
    print(json.dumps(brp(args.method, params), ensure_ascii=False, indent=2))
    return 0


def construire_parseur() -> argparse.ArgumentParser:
    parseur = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    sous = parseur.add_subparsers(dest="commande", required=True)

    p = sous.add_parser("snapshot", help="photographie du monde")
    p.add_argument("--json", action="store_true", help="tout, brut")
    p.add_argument("--field", help="une seule route, ex: locomotion.speed_mps")
    p.set_defaults(fonction=commande_snapshot)

    p = sous.add_parser("watch", help="boucle d'observation")
    p.add_argument("--seconds", type=float, default=5.0)
    p.add_argument("--interval", type=float, default=0.25)
    p.add_argument("--field", help="suivre une seule route")
    p.add_argument(
        "--changes-only",
        action="store_true",
        help="n'imprimer que les lignes qui changent",
    )
    p.set_defaults(fonction=commande_watch)

    p = sous.add_parser("key", help="appuyer une touche physique (KeyR, Space, F3…)")
    p.add_argument("key")
    p.add_argument("--frames", type=int, default=6, help="duree du tap (defaut 6)")
    p.add_argument("--hold", action="store_true", help="tenir jusqu'a --release")
    p.add_argument("--release", action="store_true", help="relacher une touche tenue")
    p.set_defaults(fonction=commande_key)

    p = sous.add_parser("look", help="tourner le regard par la souris")
    p.add_argument("--yaw", type=float, default=0.0, help="degres, + vers la gauche")
    p.add_argument("--pitch", type=float, default=0.0, help="degres, + vers le haut")
    p.add_argument("--frames", type=int, default=6)
    p.set_defaults(fonction=commande_look)

    p = sous.add_parser("act", help="une action du vocabulaire ferme")
    p.add_argument("action")
    p.add_argument("--frames", type=int, default=30)
    p.set_defaults(fonction=commande_act)

    p = sous.add_parser("stop", help="arreter l'action en cours")
    p.set_defaults(fonction=commande_stop)

    p = sous.add_parser("release-all", help="tout relacher (touches, action, chemin)")
    p.set_defaults(fonction=commande_release_all)

    p = sous.add_parser("call", help="n'importe quelle methode BRP")
    p.add_argument("method")
    p.add_argument("params", nargs="?", help="JSON")
    p.set_defaults(fonction=commande_call)

    return parseur


def main() -> int:
    args = construire_parseur().parse_args()
    return args.fonction(args)


if __name__ == "__main__":
    sys.exit(main())
