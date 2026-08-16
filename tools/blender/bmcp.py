"""Client minimal du socket de l'addon BlenderMCP (port 9876).

Le serveur `blender-mcp` lancé par Claude Code n'est qu'un proxy JSON au-dessus
de ce même socket. Passer par ce client garde le trajet REPRODUCTIBLE : chaque
commande envoyée à Blender est un fichier versionné, pas un appel volatil.

    python tools/blender/bmcp.py scene
    python tools/blender/bmcp.py code tools/blender/expedition/00_probe.py
    python tools/blender/bmcp.py shot out.png

Convention : un script envoyé par `code` communique son résultat en imprimant
une ligne `RESULT: <json>`. Blender renvoie le stdout capturé, donc c'est le
seul canal de retour fiable.
"""

from __future__ import annotations

import json
import socket
import sys

HOST = "localhost"
PORT = 9876
# L'addon répond par un flux non délimité : on relit jusqu'à ce que le JSON
# soit complet. Un timeout court sur un import de 300 GLB donnerait un faux
# « Blender ne répond pas » alors qu'il travaille.
TIMEOUT_S = 900.0
CHUNK = 65536


def send(command: str, params: dict | None = None) -> dict:
    """Envoie une commande à l'addon et rend sa réponse décodée."""
    payload = json.dumps({"type": command, "params": params or {}}).encode()
    with socket.create_connection((HOST, PORT), timeout=TIMEOUT_S) as sock:
        sock.settimeout(TIMEOUT_S)
        sock.sendall(payload)
        buffer = b""
        while True:
            chunk = sock.recv(CHUNK)
            if not chunk:
                break
            buffer += chunk
            try:
                return json.loads(buffer.decode("utf-8"))
            except (json.JSONDecodeError, UnicodeDecodeError):
                continue  # réponse encore partielle
    raise RuntimeError(f"réponse incomplète ({len(buffer)} octets)")


def run_code(path: str) -> dict:
    """Exécute un fichier Python dans Blender."""
    with open(path, encoding="utf-8") as handle:
        return send("execute_code", {"code": handle.read()})


def _fail(response: dict) -> bool:
    """Vrai si l'addon a signalé une erreur. Une trace Blender ne doit jamais
    se lire comme un succès : c'est ce qui fait perdre le plus de temps."""
    return response.get("status") != "success"


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2

    verb = sys.argv[1]
    if verb == "scene":
        response = send("get_scene_info")
    elif verb == "object":
        response = send("get_object_info", {"name": sys.argv[2]})
    elif verb == "code":
        response = run_code(sys.argv[2])
    elif verb == "shot":
        response = send("get_viewport_screenshot", {"filepath": sys.argv[2], "max_size": 1200})
    else:
        print(f"verbe inconnu : {verb}")
        return 2

    if _fail(response):
        print("ECHEC BLENDER")
        print(json.dumps(response, indent=2, ensure_ascii=False)[:4000])
        return 1

    result = response.get("result", response)
    text = result.get("result") if isinstance(result, dict) and "result" in result else result
    print(text if isinstance(text, str) else json.dumps(text, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
