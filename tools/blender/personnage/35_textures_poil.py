"""Déplie la peau du chien et CUIT sa fourrure en deux cartes.

    python tools/blender/bmcp.py code tools/blender/personnage/35_textures_poil.py

À lancer APRÈS `34_consolider_chien.py`.

POURQUOI CUIRE, ET PAS GARDER LE PROCÉDURAL

Un bruit procédural ne franchit pas le glTF — même mur que le shader triplanaire
du terrain et que les particules de la cape. Ce qui traverse, c'est une IMAGE.
On construit donc la fourrure en nœuds, on la cuit dans une texture, et c'est
la texture qui part dans le jeu. Le shader n'est qu'un moule.

DEUX CARTES, ET CE QUE CHACUNE PORTE

    poil_bc.png   couleur de base — le brun, la crème, l'intérieur d'oreille,
                  plus une moucheture qui casse l'aplat
    poil_n.png    normales — LES POILS. C'est elle qui donne le duveteux, et
                  elle ne coûte pas un seul triangle. Un poil géométrique
                  ferait exploser le budget pour un gain qui ne se voit pas
                  à distance de jeu.

Le relief est bâti en deux fréquences, comme une vraie fourrure : des MÈCHES
(ondes distordues, basse fréquence) et un GRAIN (bruit fin) par-dessus. Une
seule fréquence donne soit du crépi, soit du velours — jamais du poil.
"""

import json
import math
import os

import bpy

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
DOSSIER = os.path.join(RACINE, "assets", "textures", "chien_1k")

SPEC = {
    "taille": 1024,          # même gabarit que `personnage_1k`, par cohérence
    "angle_deplie_deg": 66.0,
    "marge_ilot": 0.02,
    "marge_cuisson": 10,
    "echantillons": 8,
    # L'occlusion demande des rayons, pas une simple évaluation de matière :
    # 8 échantillons la rendraient piquée.
    "echantillons_ao": 48,
    # Force du mélange de l'AO dans la couleur, et PLANCHER en dessous duquel
    # aucun texel ne descend.
    #
    # 🚨 Le plancher n'est pas de la prudence, il ferme une classe de défaut.
    # Une occlusion cuite décrit UNE pose ; toute surface qu'une animation
    # révèle ensuite arrive avec l'ombre de sa pose de repos. Vu en jeu : la
    # calotte de paupière est enfouie aux trois quarts dans l'orbite, donc cuite
    # à AO ≈ 0 — et le chien clignait avec deux taches NOIRES sur les yeux.
    # Masquer les billes pendant la cuisson n'y change rien : l'occulteur est le
    # crâne. Tant qu'on fond l'AO dans la couleur, la seule parade honnête est
    # de borner sa profondeur.
    "force_ao": 0.58,
    "plancher_ao": 0.50,

    # 🎨 LE DÉCALAGE DE TEINTE — le levier le plus rentable de tout l'habillage.
    #
    # Une occlusion qui multiplie du GRIS produit une ombre morte : la couleur
    # descend en valeur sans changer de nature, et la tête a l'air sale plutôt
    # qu'éclairée. Tout le rendu stylisé moderne (Overwatch, Fortnite) fait
    # l'inverse : l'ombre part vers le FROID et se sature, la lumière part vers
    # le CHAUD. C'est ce écart de teinte, bien plus que le contraste, qui donne
    # l'impression de peinture.
    #
    # On multiplie donc par une TEINTE interpolée entre ces deux pôles selon
    # l'occlusion, au lieu d'un simple scalaire.
    "teinte_ombre": "#6E6AA8",     # violet froid : le ciel qui remplit l'ombre
    "force_teinte": 0.75,          # 0 = gris neutre (l'ancien comportement)

    # Les deux fréquences du poil. Les échelles sont en unités d'objet : la
    # peau tient dans ±0,13, donc « échelle 42 » ≈ 6 mm de période.
    #
    # 🚨 `etirement` est ce qui fait la différence entre du POIL et du CRÉPI.
    # Première version : une onde distordue, isotrope — elle a produit un
    # labyrinthe de corail cérébral, très net et parfaitement inanimal. Une
    # fourrure est DIRECTIONNELLE, elle tombe. On écrase donc la coordonnée sur
    # l'axe du crâne avant d'échantillonner : le motif s'allonge d'autant dans
    # ce sens, et les mèches coulent du sommet vers le cou.
    "meches": {"echelle": 42.0, "detail": 4.0, "rugosite": 0.55, "etirement": 0.18},
    "grain": {"echelle": 130.0, "detail": 6.0, "rugosite": 0.62, "etirement": 0.50},
    "part_grain": 0.30,      # poids du grain dans le relief
    "force_bump": 0.36,
    "distance_bump": 0.014,

    # Moucheture de la couleur : assez pour tuer l'aplat, pas assez pour salir.
    # Elle suit le MÊME relief, donc le creux entre deux mèches s'assombrit —
    # c'est ce qui fait lire les mèches même sans lumière rasante.
    "variation_couleur": 0.22,
    "assombrissement_meche": 0.18,
}


def srgb_lineaire(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def hex_lineaire(code):
    code = code.lstrip("#")
    return tuple(srgb_lineaire(int(code[i:i + 2], 16) / 255.0)
                 for i in (0, 2, 4)) + (1.0,)


def dossier():
    if not os.path.isdir(DOSSIER):
        os.makedirs(DOSSIER)
    return DOSSIER


def bsdf_de(mat):
    return next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)


def habiller_de_poil(mat):
    """Greffe la fourrure sur la matière, SANS toucher à sa couleur déclarée.

    La couleur de base est relue puis réinjectée : chaque matière garde son
    identité (brun, crème, intérieur d'oreille) et reçoit le même poil. Écrire
    la couleur ici la dupliquerait — elle vit dans `32_tete_chien.py`."""
    nt = mat.node_tree
    bsdf = bsdf_de(mat)
    if bsdf is None:
        return None
    base = tuple(bsdf.inputs["Base Color"].default_value)

    coord = nt.nodes.new("ShaderNodeTexCoord")

    def bruit_etire(reglage):
        """Un bruit échantillonné dans un espace ÉCRASÉ sur z : le motif s'y
        allonge du même facteur, et devient une mèche au lieu d'une tache."""
        carte = nt.nodes.new("ShaderNodeMapping")
        carte.inputs["Scale"].default_value = (1.0, 1.0, reglage["etirement"])
        nt.links.new(coord.outputs["Object"], carte.inputs["Vector"])
        bruit = nt.nodes.new("ShaderNodeTexNoise")
        bruit.inputs["Scale"].default_value = reglage["echelle"]
        bruit.inputs["Detail"].default_value = reglage["detail"]
        bruit.inputs["Roughness"].default_value = reglage["rugosite"]
        nt.links.new(carte.outputs["Vector"], bruit.inputs["Vector"])
        return bruit

    meches = bruit_etire(SPEC["meches"])
    grain = bruit_etire(SPEC["grain"])

    relief = nt.nodes.new("ShaderNodeMix")
    relief.data_type = "FLOAT"
    relief.inputs["Factor"].default_value = SPEC["part_grain"]
    nt.links.new(meches.outputs["Fac"], relief.inputs[2])   # A
    nt.links.new(grain.outputs["Fac"], relief.inputs[3])    # B

    bump = nt.nodes.new("ShaderNodeBump")
    bump.inputs["Strength"].default_value = SPEC["force_bump"]
    bump.inputs["Distance"].default_value = SPEC["distance_bump"]
    nt.links.new(relief.outputs[0], bump.inputs["Height"])
    nt.links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])

    # La couleur : le creux des mèches assombrit, le grain module.
    sombre = tuple(c * (1.0 - SPEC["assombrissement_meche"]) for c in base[:3]) + (1.0,)
    teinte = nt.nodes.new("ShaderNodeMix")
    teinte.data_type = "RGBA"
    teinte.inputs[6].default_value = base            # A
    teinte.inputs[7].default_value = sombre          # B
    facteur = nt.nodes.new("ShaderNodeMix")
    facteur.data_type = "FLOAT"
    facteur.inputs["Factor"].default_value = SPEC["variation_couleur"]
    facteur.inputs[2].default_value = 0.0
    nt.links.new(relief.outputs[0], facteur.inputs[3])
    nt.links.new(facteur.outputs[0], teinte.inputs["Factor"])
    nt.links.new(teinte.outputs[2], bsdf.inputs["Base Color"])
    return base


def image(nom, couleur_lineaire):
    img = bpy.data.images.get(nom)
    if img is not None:
        bpy.data.images.remove(img)
    img = bpy.data.images.new(nom, SPEC["taille"], SPEC["taille"], alpha=False,
                              float_buffer=False)
    # 🚨 Une carte de normales en sRGB est une carte FAUSSE : le moteur lirait
    # des vecteurs corrigés en gamma. Non-Color, toujours.
    img.colorspace_settings.name = "Non-Color" if couleur_lineaire else "sRGB"
    return img


def cible_de_cuisson(obj, img):
    """Chaque matière doit porter le nœud image ACTIF, sinon la cuisson refuse."""
    noeuds = []
    for slot in obj.material_slots:
        nt = slot.material.node_tree
        node = nt.nodes.new("ShaderNodeTexImage")
        node.image = img
        node.label = "cible_cuisson"
        for autre in nt.nodes:
            autre.select = False
        node.select = True
        nt.nodes.active = node
        noeuds.append((nt, node))
    return noeuds


def retirer(noeuds):
    for nt, node in noeuds:
        nt.nodes.remove(node)


def deplier(obj):
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    with bpy.context.temp_override(active_object=obj, object=obj,
                                   selected_editable_objects=[obj],
                                   selected_objects=[obj]):
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.uv.smart_project(
            angle_limit=math.radians(SPEC["angle_deplie_deg"]),
            island_margin=SPEC["marge_ilot"])
        bpy.ops.object.mode_set(mode="OBJECT")
    return len(obj.data.uv_layers)


def cuire(obj, img, genre, filtre):
    scene = bpy.context.scene
    noeuds = cible_de_cuisson(obj, img)
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    with bpy.context.temp_override(active_object=obj, object=obj,
                                   selected_editable_objects=[obj],
                                   selected_objects=[obj]):
        bpy.ops.object.bake(type=genre, pass_filter=filtre,
                            margin=SPEC["marge_cuisson"], use_clear=True)
    retirer(noeuds)
    chemin = os.path.join(dossier(), f"{img.name}.png")
    img.filepath_raw = chemin
    img.file_format = "PNG"
    img.save()
    return chemin


def melanger_occlusion(bc, ao, force):
    """Multiplie l'occlusion DANS la couleur de base, en linéaire.

    Pourquoi la mélanger au lieu de l'exporter à part : le glTF sait porter une
    `occlusionTexture`, mais le corps du personnage n'en utilise pas — son AO
    est déjà fondu dans sa couleur (sa `Base Color` sort d'un nœud `MIX`).
    Exporter l'AO séparément donnerait au chien un traitement que rien d'autre
    ne partage, et une carte de plus à charger.

    🚨 Le mélange se fait sur `image.pixels`, qui est TOUJOURS linéaire chez
    Blender — jamais sur les octets sRGB du PNG. Multiplier une valeur encodée
    en gamma par une occlusion linéaire assombrit deux fois les tons moyens.
    """
    import numpy as np

    couleur = np.empty(len(bc.pixels), dtype=np.float32)
    bc.pixels.foreach_get(couleur)
    occlusion = np.empty(len(ao.pixels), dtype=np.float32)
    ao.pixels.foreach_get(occlusion)

    couleur = couleur.reshape(-1, 4)
    brut = occlusion.reshape(-1, 4)[:, 0]

    # 🚨 L'occlusion BRUTE ne s'applique pas telle quelle. Une bonne part de
    # l'atlas couvre des surfaces ENFOUIES — bases d'oreille dans le crâne, haut
    # du cou, intérieur du masque — légitimement noires et jamais vues. Mesuré :
    # facteur moyen 0,35, soit une tête assombrie aux deux tiers par des zones
    # invisibles. On normalise donc sur le 90ᵉ centile : ce qui est franchement
    # exposé vaut 1, et le reste s'ordonne en dessous. L'AO devient un terme
    # d'ombrage RELATIF — ce qu'elle est de toute façon sur un asset stylisé.
    haut = float(np.percentile(brut, 90.0))
    normalise = np.clip(brut / max(1e-4, haut), SPEC["plancher_ao"], 1.0)
    facteur = 1.0 - force * (1.0 - normalise)

    # La teinte : NEUTRE en pleine lumière, froide dans l'ombre.
    #
    # 🚨 Première version : elle interpolait entre une teinte « lumière chaude »
    # et une teinte froide, puis normalisait sur le max du canal. Résultat, la
    # teinte chaude s'appliquait AUSSI en pleine lumière — elle coupait 31 % du
    # bleu partout, et la tête est sortie brune et sale, le masque crème
    # quasiment effacé. Une teinte de lumière n'a rien à faire dans une texture
    # de couleur de base : c'est l'éclairage de la scène qui la donne.
    #
    # On ne garde donc que le DÉCALAGE d'ombre, et on le normalise en luminance
    # pour qu'il colore sans assombrir — l'assombrissement est le travail de
    # `facteur`, et le faire deux fois donne un trou noir.
    ombre = np.array(hex_lineaire(SPEC["teinte_ombre"])[:3], dtype=np.float32)
    luminance = float(0.2126 * ombre[0] + 0.7152 * ombre[1] + 0.0722 * ombre[2])
    ombre = ombre / max(1e-4, luminance)

    k = (1.0 - normalise)[:, None] * SPEC["force_teinte"]
    teinte = (1.0 - k) + ombre[None, :] * k

    couleur[:, 0:3] *= facteur[:, None] * teinte

    bc.pixels.foreach_set(couleur.ravel())
    bc.update()
    bc.save()
    return {"teinte_ombre": SPEC["teinte_ombre"],
            "ao_brut_moyen": round(float(brut.mean()), 3),
            "ao_centile90": round(haut, 3),
            "plancher": SPEC["plancher_ao"],
            "facteur_moyen": round(float(facteur.mean()), 3),
            "facteur_min": round(float(facteur.min()), 3)}


def matiere_finale(obj, bc, nm):
    """Une seule matière, qui ne porte plus que les deux images cuites."""
    mat = bpy.data.materials.new("Chien_poil_cuit")
    mat.use_nodes = True
    nt = mat.node_tree
    bsdf = bsdf_de(mat)
    bsdf.inputs["Roughness"].default_value = 0.72
    bsdf.inputs["Metallic"].default_value = 0.0

    t_bc = nt.nodes.new("ShaderNodeTexImage")
    t_bc.image = bc
    t_bc.location = (-560, 220)
    nt.links.new(t_bc.outputs["Color"], bsdf.inputs["Base Color"])

    t_n = nt.nodes.new("ShaderNodeTexImage")
    t_n.image = nm
    t_n.location = (-560, -160)
    carte = nt.nodes.new("ShaderNodeNormalMap")
    carte.location = (-260, -160)
    nt.links.new(t_n.outputs["Color"], carte.inputs["Color"])
    nt.links.new(carte.outputs["Normal"], bsdf.inputs["Normal"])

    obj.data.materials.clear()
    obj.data.materials.append(mat)
    # Les indices de slot des faces pointaient vers 3 matières ; il n'en reste
    # qu'une. Les remettre à 0 explicitement plutôt que de compter sur un
    # écrêtage silencieux.
    for poly in obj.data.polygons:
        poly.material_index = 0
    return mat.name


def main():
    obj = bpy.data.objects.get("chien_peau")
    if obj is None:
        print("RESULT: " + json.dumps(
            {"erreur": "chien_peau absent — lancer 34_consolider_chien.py d'abord"}))
        return

    # 🚨 CE SCRIPT N'EST PAS REJOUABLE, et il doit le dire au lieu de produire
    # une bouillie. Il CONSOMME les matières d'auteur (poil / crème / pavillon)
    # et les remplace par la matière cuite, dont la couleur de base est une
    # IMAGE. Relancé tel quel, il relit `default_value` sur une entrée déjà
    # branchée — qui vaut blanc — et recuit un chien entièrement BLANC. Vu.
    deja = [s.material.name for s in obj.material_slots
            if s.material and s.material.name.endswith("_cuit")]
    if deja:
        print("RESULT: " + json.dumps({
            "erreur": "matières déjà cuites — relancer 32 puis 34 d'abord",
            "trouvees": deja}))
        return

    couleurs = {slot.material.name: habiller_de_poil(slot.material)
                for slot in obj.material_slots}
    uv = deplier(obj)

    # Les deux autres pièces n'ont AUCUNE UV — mesuré dans le GLB livré. Sans
    # conséquence tant qu'elles sont en aplats, mais bloquant le jour où on
    # voudra un vrai iris ou une truffe mouillée. Un dépliage coûte une seconde
    # maintenant, et une re-livraison plus tard.
    autres = {}
    for nom in ("chien_yeux", "chien_museau"):
        piece = bpy.data.objects.get(nom)
        if piece is not None:
            autres[nom] = deplier(piece)

    scene = bpy.context.scene
    moteur_avant = scene.render.engine
    scene.render.engine = "CYCLES"
    scene.cycles.use_denoising = False

    bc = image("poil_bc", couleur_lineaire=False)
    nm = image("poil_n", couleur_lineaire=True)
    ao = image("poil_ao", couleur_lineaire=True)

    scene.cycles.samples = SPEC["echantillons"]
    chemins = {
        "poil_bc": cuire(obj, bc, "DIFFUSE", {"COLOR"}),
        "poil_n": cuire(obj, nm, "NORMAL", set()),
    }
    # 🚨 Les billes d'yeux sortent de la scène LE TEMPS DE LA CUISSON D'AO.
    #
    # La paupière est plaquée contre la bille : l'occlusion l'y voit enterrée et
    # rend ~0, donc la couleur cuite la rendait presque NOIRE. Invisible au
    # repos, où la paupière est relevée — mais dès que le chien cligne, deux
    # taches sombres se posent sur ses yeux.
    #
    # La règle générale : une occlusion se calcule contre ce qui NE BOUGE PAS
    # par rapport à la surface. Tout ce qui s'anime relativement à elle doit
    # sortir du calcul, sinon on cuit l'ombre d'une pose particulière.
    caches = [bpy.data.objects[n] for n in ("chien_yeux",)
              if n in bpy.data.objects]
    avant_visible = [(o, o.hide_render) for o in caches]
    for o in caches:
        o.hide_render = True

    scene.cycles.samples = SPEC["echantillons_ao"]
    chemins["poil_ao"] = cuire(obj, ao, "AO", set())

    for o, etat in avant_visible:
        o.hide_render = etat
    scene.render.engine = moteur_avant

    occlusion = melanger_occlusion(bc, ao, SPEC["force_ao"])
    nom_mat = matiere_finale(obj, bc, nm)

    print("RESULT: " + json.dumps({
        "uv_maps": uv,
        "uv_autres_pieces": autres,
        "occlusion": occlusion,
        "matieres_avant": sorted(k for k in couleurs),
        "matiere_apres": nom_mat,
        "images": {k: {"chemin": v, "octets": os.path.getsize(v)}
                   for k, v in chemins.items()},
        "taille": SPEC["taille"],
        "triangles": len(obj.data.loop_triangles),
    }, ensure_ascii=False))


main()
