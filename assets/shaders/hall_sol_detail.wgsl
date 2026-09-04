// hall_sol_detail.wgsl — le grain que l'albédo cuit du sol ne peut pas porter.
//
// # Le problème, mesuré
//
// Le sol du Hall est UN albédo 2048² cuit sur 300 × 300 m : **5 texels par mètre**,
// contre 194 de médiane pour le reste de la carte. Agrandir cet albédo ne sert à
// rien — à 8192² il ferait encore 20 texels/m pour 268 Mo de VRAM. Ce qui manque
// n'est pas de la couleur, c'est de la HAUTE FRÉQUENCE.
//
// On multiplie donc l'albédo par une texture de détail tuilée tous les quelques
// mètres. Le détail a une moyenne de 0,5 exactement (garanti à la génération), d'où
// le `× 2` : la couleur d'ensemble de la carte reste rigoureusement inchangée, seul
// le grain apparaît.
//
// # Pourquoi ce shader ne touche QUE l'étage fragment
//
// 🚨 Le 2026-08-18, l'Expédition ne se lançait plus du tout : un shader qui
// surchargeait l'étage sommet déclarait sa propre structure de sortie et il y
// manquait des emplacements que le fragment PBR réclame dès que Bevy définit
// certains `#ifdef`. wgpu tue le processus, sans message exploitable.
//
// Ici on ne surcharge que le fragment, et on IMPORTE `VertexOutput` au lieu de le
// redéclarer — c'est le piège symétrique, et il est tout aussi silencieux.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
}

#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}

// 🚨 `#{MATERIAL_BIND_GROUP}` ET NON `2` EN DUR
//
// Le numéro du groupe du matériau est INJECTÉ par Bevy : l'écrire en dur pointe
// vers un groupe qui ne contient pas ces bindings, et wgpu refuse le pipeline avec
// « Shader global ResourceBinding { group: 2, binding: 100 } is not available in
// the pipeline layout ». Le processus meurt alors en `STATUS_STACK_BUFFER_OVERRUN`,
// sans que le message ne dise jamais « ton @group est faux ».
//
// Mesuré le 2026-08-21 : c'est exactement ce qui est arrivé à ce fichier.
// `expedition_vent.wgsl` faisait déjà correctement la chose depuis le début.

struct ReglagesSol {
    // `x` = tuiles par unité d'UV · `y` = force sur la couleur (0 = éteint)
    // `z` = force sur la rugosité · `w` = réservé.
    reglages: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> sol: ReglagesSol;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var detail_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var detail_sampler: sampler;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let uv = in.uv * sol.reglages.x;
    let grain = textureSample(detail_texture, detail_sampler, uv).r;

    // Moyenne du détail = 0,5, donc `grain * 2` a une moyenne de 1,0 : neutre.
    // `mix` permet d'éteindre le détail par la donnée sans recompiler le shader.
    let facteur = mix(1.0, grain * 2.0, sol.reglages.y);
    pbr_input.material.base_color = vec4<f32>(
        pbr_input.material.base_color.rgb * facteur,
        pbr_input.material.base_color.a,
    );

    // Le même grain module légèrement la rugosité : une surface parfaitement lisse
    // sur 300 m se lit comme du plastique, même bien texturée. On reste dans des
    // bornes sûres — une rugosité nulle ferait un miroir.
    let rugosite = pbr_input.material.perceptual_roughness
        * mix(1.0, grain * 2.0, sol.reglages.z);
    pbr_input.material.perceptual_roughness = clamp(rugosite, 0.08, 1.0);

    pbr_input.material.base_color = alpha_discard(
        pbr_input.material, pbr_input.material.base_color);

    // Pas de branche prepass : ce materiau ne declare PAS de
    // `prepass_fragment_shader()`, donc Bevy n'appelle jamais ce fragment la.
    // La branche d'origine appelait `deferred_output` sans l'importer — du code
    // mort qui ne pouvait que nuire.
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
