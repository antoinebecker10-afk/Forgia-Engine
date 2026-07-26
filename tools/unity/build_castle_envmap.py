"""Construit la cubemap d'ambiance du Hall à partir des sondes cuites du pack.

Pourquoi
--------
Forgia n'a **aucun éclairage par image** : `EnvironmentMapLight` n'est employé nulle
part dans le projet. Une surface PBR avec une carte de rugosité et rien à réfléchir
tombe sur un reflet plat et uniforme — c'est l'aspect « pierre mouillée » relevé en
comparant notre Hall à la capture du créateur.

Le pack, lui, contient ses **32 sondes de réflexion cuites** (`ReflectionProbe-N.exr`).
Ce sont des mesures de ce que sa pierre réfléchit réellement, pas une supposition :
des valeurs sombres et franchement orangées (rapport R/B jusqu'à 25).

Ce script en tire **une** cubemap d'ambiance pour tout l'intérieur. Les 31 sondes
localisées avec projection de boîte viendront après : l'association sonde ↔ fichier
EXR n'est pas donnée par la scène (elle vit dans `LightingData.asset`, binaire), et
poser une sonde sur une association devinée serait pire que pas de sonde du tout.

Format des sondes
-----------------
768 × 128 = **6 faces de 128 en bande horizontale**, ordre Unity standard
`+X, −X, +Y, −Y, +Z, −Z`. Cet ordre est vérifié et non supposé : sur 25 des 32
sondes, la face 2 est plus sombre que la face 3 — soit un plafond plus sombre que
le sol, ce qu'on attend d'un intérieur éclairé aux bougies. Les 7 exceptions sont
les sondes extérieures, où le ciel est au contraire la source lumineuse.

Le miroir
---------
Le château porte un miroir sur X. Une direction `d` de notre monde correspond à
`(−d.x, d.y, d.z)` chez lui, ce qui se traduit sur les faces par :

- **toutes** les faces sont retournées horizontalement ;
- les faces `+X` et `−X` s'échangent.

Oublier ce miroir mettrait les reflets à l'envers — même piège que sur les
rotations des bannières.

Usage :
    python tools/unity/build_castle_envmap.py <dossier de sondes> <sortie.hdr>
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

# Doit être posé avant l'import d'OpenCV, qui lit la variable au chargement.
os.environ.setdefault("OPENCV_IO_ENABLE_OPENEXR", "1")

import cv2  # noqa: E402
import numpy as np  # noqa: E402

FACE_COUNT = 6
# Une sonde d'intérieur est sombre et chaude ; une sonde de ciel est claire et
# bleue. Les seuils séparent nettement les deux populations mesurées (les
# intérieurs montent à R/B = 25, les extérieurs descendent à 0,23).
INTERIOR_WARMTH = 1.8
INTERIOR_MAX_LUMINANCE = 0.08
# Coefficients de luminance Rec. 709.
LUMINANCE = np.array([0.2126, 0.7152, 0.0722], dtype=np.float32)


def load_faces(path: Path) -> np.ndarray | None:
    """Retourne les 6 faces `(6, n, n, 3)` en RGB, ou None si illisible."""
    image = cv2.imread(str(path), cv2.IMREAD_UNCHANGED)
    if image is None or image.ndim != 3 or image.shape[2] < 3:
        return None
    height, width = image.shape[:2]
    if width != height * FACE_COUNT:
        return None
    # OpenCV rend du BGR ; on repasse en RGB une fois pour toutes.
    rgb = image[:, :, :3][:, :, ::-1].astype(np.float32)
    return np.stack([rgb[:, i * height : (i + 1) * height] for i in range(FACE_COUNT)])


def is_interior(faces: np.ndarray) -> bool:
    means = faces.mean(axis=(1, 2))
    warmth = means[:, 0].mean() / max(means[:, 2].mean(), 1e-6)
    luminance = float((means @ LUMINANCE).mean())
    return warmth > INTERIOR_WARMTH and luminance < INTERIOR_MAX_LUMINANCE


def mirror_x(faces: np.ndarray) -> np.ndarray:
    """Applique à la cubemap le miroir sur X que porte le château."""
    flipped = faces[:, :, ::-1, :]
    order = [1, 0, 2, 3, 4, 5]  # +X et −X s'échangent
    return flipped[order]


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 1
    source = Path(sys.argv[1])
    destination = Path(sys.argv[2])

    interiors, skipped = [], []
    paths = sorted(
        source.glob("ReflectionProbe-*.exr"),
        key=lambda p: int(re.search(r"ReflectionProbe-(\d+)", p.name).group(1)),
    )
    if not paths:
        print(f"Aucune sonde dans {source}", file=sys.stderr)
        return 1
    for path in paths:
        faces = load_faces(path)
        if faces is None:
            print(f"  ignorée (format inattendu) : {path.name}", file=sys.stderr)
            continue
        (interiors if is_interior(faces) else skipped).append((path.name, faces))

    if not interiors:
        print("Aucune sonde d'intérieur reconnue — seuils à revoir.", file=sys.stderr)
        return 1

    average = np.mean([faces for _, faces in interiors], axis=0)
    average = mirror_x(average)

    # Bevy lit une cubemap depuis une image empilée **verticalement**, qu'il
    # réinterprète en tableau de 6 couches.
    stacked = np.concatenate(list(average), axis=0)
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Retour en BGR pour l'écriture, et contigu — cv2 refuse les vues inversées.
    if not cv2.imwrite(str(destination), np.ascontiguousarray(stacked[:, :, ::-1])):
        print(f"Écriture impossible : {destination}", file=sys.stderr)
        return 1

    size = average.shape[1]
    luminance = float((average.mean(axis=(1, 2)) @ LUMINANCE).mean())
    print(f"{len(interiors)} sondes d'intérieur moyennées ({len(skipped)} écartées)")
    print(f"  ecartees : {', '.join(name for name, _ in skipped) or 'aucune'}")
    print(f"  luminance moyenne : {luminance:.5f}")
    print(f"  {size}x{size} par face, empilees en {stacked.shape[1]}x{stacked.shape[0]}")
    print(f"  -> {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
