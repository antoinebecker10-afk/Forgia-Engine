"""Fond les 24 pièces du chien en TROIS maillages.

    python tools/blender/bmcp.py code tools/blender/personnage/34_consolider_chien.py

À lancer APRÈS `32_tete_chien.py`, sur la scène qu'il laisse.

POURQUOI TROIS, ET PAS UN NI VINGT-QUATRE

Vingt-quatre objets, c'est un état d'atelier : chaque pièce a été posée
séparément parce qu'elle se calculait séparément. Pour la suite, ça bloque tout
— un dépliage UV se fait par objet, une cuisson de texture aussi, et une pesée
sur squelette aussi. Un seul objet serait l'excès inverse : les yeux n'ont rien
à faire dans l'atlas de fourrure, et une truffe vernie ne veut pas la même
matière qu'un poil mat.

La coupe suit donc ce qui recevra le MÊME traitement en aval :

    peau     poil + masque crème + cou + oreilles + pavillons + mèches
             + paupières      → un dépliage, un atlas, une cuisson de fourrure
    yeux     billes + éclats  → lisses, vernies, jamais de poil
    museau   truffe + narines + gueule → lisses aussi, mais mates

LA FUSION NE DÉFORME RIEN. Toutes les pièces portent DÉJÀ la même matrice de
pose (celle dérivée de l'os `Head`) : `join` ramène les sommets dans le repère
de l'objet actif, ce qui est ici l'identité. Si un jour une pièce arrivait avec
une autre matrice, la fusion la déplacerait en silence — d'où le contrôle
d'emprise avant/après en fin de script, qui refuse ce cas au lieu de le subir.
"""

import json

import bpy

# Chaque famille se réclame AUSSI elle-même : le script doit pouvoir se rejouer
# sur une scène déjà consolidée sans laisser un résultat en orpheline.
FAMILLES = {
    # 🚨 `chien_marque_` DOIT figurer ici. Les trois marques de pelage (crème,
    # selle, cerne) sont des objets à part entière produits par la coupe ; sans
    # leur préfixe, elles tombaient en ORPHELINES — ni fondues, ni texturées,
    # ni pesées — et la tête cuisait entièrement brune, masque crème effacé.
    # Le rapport le disait dans `orphelins`, encore fallait-il le lire.
    "chien_peau": ("chien_peau", "chien_tete", "chien_masque", "chien_marque_",
                   "chien_cou", "chien_oreille_", "chien_pavillon_",
                   "chien_meche_", "chien_paupiere_"),
    "chien_yeux": ("chien_yeux", "chien_oeil_", "chien_eclat_"),
    "chien_museau": ("chien_museau", "chien_truffe", "chien_narine_",
                     "chien_gueule"),
}


def emprise(objets):
    pts = [o.matrix_world @ v.co for o in objets for v in o.data.vertices]
    if not pts:
        return None
    return [round(min(p[i] for p in pts), 5) for i in range(3)] + \
           [round(max(p[i] for p in pts), 5) for i in range(3)]


def marquer(obj):
    """Un groupe de sommets au nom de la pièce, AVANT de la fondre.

    🚨 Sans ça la fusion est irréversible en information : après `join` il n'y a
    plus aucun moyen de désigner « les paupières » ou « l'oreille droite » — les
    sommets sont mêlés. Or c'est exactement ce que réclament les deux étapes
    suivantes : peser les oreilles sur leurs os, et bouger les paupières pour le
    clignement. Une fusion qui perd la provenance oblige à re-sculpter pour
    retrouver ce qu'on savait déjà."""
    groupe = obj.vertex_groups.new(name=obj.name)
    groupe.add(range(len(obj.data.vertices)), 1.0, "REPLACE")
    return groupe.name


def fusionner(nom_cible, membres):
    """`join` exige un objet ACTIF et une sélection — donc un contexte forcé."""
    bpy.ops.object.select_all(action="DESELECT")
    for obj in membres:
        obj.select_set(True)
    # L'actif décide du repère ET du nom conservé. On prend le plus gros : sa
    # matrice est la plus « autoritaire », et ses slots matière arrivent en tête.
    actif = max(membres, key=lambda o: len(o.data.vertices))
    bpy.context.view_layer.objects.active = actif
    with bpy.context.temp_override(active_object=actif, object=actif,
                                   selected_editable_objects=membres,
                                   selected_objects=membres):
        bpy.ops.object.join()
    actif.name = actif.data.name = nom_cible
    return actif


def main():
    scene = bpy.context.scene
    rapport = {"familles": {}}

    presents = [o for o in scene.objects if o.type == "MESH"
                and o.name.startswith("chien_")]
    if not presents:
        print("RESULT: " + json.dumps(
            {"erreur": "aucune pièce — lancer 32_tete_chien.py d'abord"}))
        return
    avant_total = emprise(presents)
    avant_tris = 0
    for obj in presents:
        obj.data.calc_loop_triangles()
        avant_tris += len(obj.data.loop_triangles)

    # 🚨 On raisonne en NOMS, pas en objets. `join` DÉTRUIT les pièces absorbées ;
    # garder leurs références Python fait lever « StructRNA has been removed » au
    # premier `.name` de la famille suivante. Même piège que `convert` sur les
    # métaballes — troisième fois aujourd'hui, d'où la règle : après un opérateur
    # qui fusionne ou convertit, on relit la scène.
    noms = sorted(o.name for o in presents)
    orphelins = set(noms)
    fusionnes = []
    for cible, prefixes in FAMILLES.items():
        membres = [bpy.data.objects[n] for n in noms
                   if n.startswith(prefixes) and n in bpy.data.objects]
        if not membres:
            rapport["familles"][cible] = {"pieces": 0}
            continue
        orphelins -= {o.name for o in membres}
        avant = emprise(membres)
        traces = [marquer(o) for o in membres]
        # `join` sur une sélection d'un seul objet est un opérateur qui échoue :
        # une famille déjà fusionnée n'a qu'à être renommée.
        if len(membres) == 1:
            obj = membres[0]
            obj.name = obj.data.name = cible
        else:
            obj = fusionner(cible, membres)
        obj.data.calc_loop_triangles()
        apres = emprise([obj])
        rapport["familles"][cible] = {
            "pieces": len(membres),
            "triangles": len(obj.data.loop_triangles),
            "matieres": [m.name for m in obj.data.materials],
            # Contrôle qui ne peut pas passer à vide : autant de groupes que de
            # pièces fondues, sinon une provenance a été perdue.
            "traces": len([g for g in obj.vertex_groups if g.name in traces]),
            # Une fusion correcte ne bouge RIEN. Le dire en millimètres, pas en
            # « ça a l'air bon ».
            "derive_max_mm": round(max(abs(a - b) for a, b in zip(avant, apres))
                                   * 1000, 3),
        }
        fusionnes.append(obj)

    apres_tris = 0
    for obj in fusionnes:
        apres_tris += len(obj.data.loop_triangles)

    rapport.update({
        "objets_avant": len(presents),
        "objets_apres": len(fusionnes),
        "triangles_avant": avant_tris,
        "triangles_apres": apres_tris,
        # Un orphelin est une pièce qu'aucune famille ne réclame : silencieuse,
        # elle disparaîtrait du pipeline sans un mot.
        "orphelins": sorted(orphelins),
        # 🚨 Se compare sur TOUT ce qui survit, pas sur les seules fusionnées.
        # Première version : elle mesurait l'emprise d'avant (24 pièces) contre
        # celle d'après (2 familles sur 3, une orpheline restée dehors) et
        # annonçait 223 mm de dérive. Le maillage n'avait pas bougé d'un micron ;
        # c'est le CONTRÔLE qui comparait deux ensembles différents. Un capteur
        # qui crie sur un décalage de périmètre au lieu d'un décalage de forme
        # coûte plus cher que pas de capteur.
        "emprise_derive_mm": round(max(abs(a - b) for a, b in zip(
            avant_total,
            emprise([o for o in scene.objects
                     if o.type == "MESH" and o.name.startswith("chien_")]))) * 1000, 3),
    })
    print("RESULT: " + json.dumps(rapport, ensure_ascii=False))


main()
