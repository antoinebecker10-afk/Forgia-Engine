// expedition_vent.wgsl — le vent du Vallon, en déplacement de sommets.
//
// # Pourquoi un shader, et pourquoi celui-ci est court
//
// Le glTF ne transporte AUCUN shader : tout ce que Blender pouvait faire pour
// le vent, c'était cuire un MASQUE. Il l'a fait — dans l'alpha de `COLOR_0`,
// un canal que le rendu opaque de Bevy ignore (`pbr_functions.wgsl` : « If
// rendering as opaque, alpha should be ignored so set to 1.0 »).
//
// Il ne reste donc ici qu'une chose à faire : lire ce masque et pousser le
// sommet. 0 = rigide (le pied, le tronc), 1 = libre (la pointe des feuilles).
//
// # Ce que le déplacement respecte
//
// - Il est HORIZONTAL. Une plante qui monte et descend a l'air de respirer,
//   pas d'être poussée.
// - Sa phase dépend de la POSITION MONDE de la pièce, pas de l'index du
//   sommet : sans ça, mille touffes battent à l'unisson et la prairie pulse
//   comme un seul organisme. Le décalage spatial est ce qui fait une houle.
// - Deux fréquences : une lente qui porte la rafale, une rapide qui frissonne.
//   Une seule donne un balancier de métronome.

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    forward_io::{Vertex, VertexOutput},
}

// 🚨 POURQUOI ON IMPORTE `Vertex`/`VertexOutput` AU LIEU DE LES DECLARER
//
// Ce shader ne surcharge QUE l'etage sommet : le fragment reste celui de Bevy.
// Les deux etages doivent donc s'accorder EXACTEMENT sur les emplacements, et
// c'est Bevy qui decide lesquels existent, via ses `#ifdef`.
//
// La premiere version declarait sa propre structure de sortie avec les
// emplacements 0, 1, 2 et 5. Il manquait `@location(3) uv_b` et
// `@location(6) instance_index` — que le fragment PBR reclame des que Bevy
// definit `VERTEX_UVS_B` / `VERTEX_OUTPUT_INSTANCE_INDEX`. Resultat, le
// 2026-08-18 : l'Expedition ne se lancait plus du tout.
//
//   In Device::create_render_pipeline, label = 'pbr_opaque_mesh_pipeline'
//     Location[6] Uint32 ... is not provided by the previous stage outputs
//
// wgpu traite ca comme fatal : le processus meurt a la compilation du pipeline,
// 150 ms apres l'entree dans la carte.
//
// > Une structure d'interface RECOPIEE est une grandeur ecrite deux fois. Elle
// > diverge au premier `#ifdef` que l'on ne connaissait pas.
//
// En important celles de Bevy, l'accord est garanti par construction : toute
// evolution du contrat (0.19, une nouvelle option de rendu) suit toute seule.

struct VentMateriau {
    // Direction du vent en plan (x, z) et sa force en mètres de débattement.
    direction_force: vec4<f32>,
    // x = fréquence de la rafale, y = fréquence du frisson,
    // z = longueur d'onde spatiale (m), w = temps (s).
    reglages: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> vent: VentMateriau;

@vertex
fn vertex(entree: Vertex) -> VertexOutput {
    var sortie: VertexOutput;

    var world_from_local = mesh_functions::get_world_from_local(entree.instance_index);
    var monde = mesh_functions::mesh_position_local_to_world(
        world_from_local, vec4<f32>(entree.position, 1.0));

    // LA RAIDEUR — cuite par Blender dans l'alpha. Un sommet à 0 ne bouge
    // jamais : c'est ce qui garde les troncs plantés et les rochers immobiles
    // sans avoir à trier les maillages côté moteur.
    // Sans couleurs de sommet, il n'y a pas de raideur cuite : le maillage
    // reste immobile plutot que de battre au hasard.
#ifdef VERTEX_COLORS
    let souplesse = entree.color.a;
#else
    let souplesse = 0.0;
#endif

    if (souplesse > 0.001) {
        let t = vent.reglages.w;
        // La phase vient de la position MONDE : deux touffes voisines battent
        // presque ensemble, deux touffes éloignées non. C'est la houle.
        let phase = (monde.x + monde.z) / max(0.001, vent.reglages.z);
        let rafale = sin(t * vent.reglages.x + phase);
        let frisson = sin(t * vent.reglages.y + phase * 2.7) * 0.35;
        let ampleur = (rafale + frisson) * souplesse * vent.direction_force.w;
        monde = vec4<f32>(
            monde.x + vent.direction_force.x * ampleur,
            monde.y,
            monde.z + vent.direction_force.z * ampleur,
            monde.w);
    }

    sortie.world_position = monde;
    sortie.position = position_world_to_clip(monde.xyz);
    sortie.world_normal = mesh_functions::mesh_normal_local_to_world(
        entree.normal, entree.instance_index);

    // Tous les champs que `VertexOutput` DECLARE doivent etre ecrits, chacun
    // sous le meme `#ifdef` que dans `forward_io.wgsl`. Un champ declare et
    // jamais ecrit disparait de l'interface et casse le pipeline.
#ifdef VERTEX_UVS_A
    sortie.uv = entree.uv;
#endif
#ifdef VERTEX_UVS_B
    sortie.uv_b = entree.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    sortie.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local, entree.tangent, entree.instance_index);
#endif
#ifdef VERTEX_COLORS
    sortie.color = entree.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    sortie.instance_index = entree.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    sortie.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        entree.instance_index, world_from_local[3]);
#endif
    return sortie;
}
