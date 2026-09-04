//! # Static merge — fusion de géométrie statique par cellule × matériau (story-663)
//!
//! Perf : le sol tuilé (~1 600 entités + ~3 200 nœuds de scène) et les murs de
//! ramparts (~130 prefabs) payaient chacun transform-propagation + visibilité +
//! extraction CPU par frame (audit 2026-07-19 : la scène statique borne le CPU ;
//! cible ship = 60 fps GTX 1060). Ici : des **sondes cachées** instancient
//! chaque GLB une fois, on lit meshes/matériaux/transforms **réellement
//! produits** par le scene-spawner (zéro hypothèse sur la hiérarchie interne du
//! GLB — même philosophie que `NeedsAssetCalibrate`), puis on fusionne les
//! instances en 1 mesh par (cellule de [`MERGE_CELL_WORLD_M`] × matériau) via
//! `Mesh::transformed_by` + `Mesh::merge`. Le frustum culling reste efficace
//! (AABB par cellule, pas un mesh géant tout-ou-rien).
//!
//! Générique par `label` ("floor", "walls", …) : Inc.1 sol, Inc.2 murs — les
//! colliders ne changent JAMAIS (sol = 1 cuboid global, murs = 6 cuboids
//! segment, gérés par le caller).
//!
//! **Cache par clé** ([`MergedStaticCache`]) : re-entrer dans une salle de même
//! taille réutilise les `Handle<Mesh>` fusionnés — zéro rebuild, zéro hitch.
//!
//! **Fallback** : si les sondes n'ont rien produit sous [`MERGE_TIMEOUT_SECS`]
//! (GLB manquant/corrompu), spawn instance-par-instance (`SceneRoot` direct) —
//! la géométrie ne peut JAMAIS manquer.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::{StageArenaMarker, StageLoadResult};

/// Côté d'une cellule de fusion en mètres (8 tuiles de 4 m). Granularité
/// interne perf (compromis batch vs frustum culling) — pas un paramètre
/// créateur (creator-simplicity : seuils perf non exposés).
const MERGE_CELL_WORLD_M: f32 = 32.0;

/// Au-delà : GLB considérés KO → fallback spawn individuel + warn.
const MERGE_TIMEOUT_SECS: f32 = 5.0;

/// Cellules fusionnées spawnées PAR FRAME (mesure 2026-07-20 : tout spawner
/// d'un coup = burst 412 ms au 1er rendu, tout se spécialise/uploade sur une
/// frame). 8/frame → ~88 meshes étalés sur ~11 frames pendant le fade-in
/// d'arène. Interne perf — pas un paramètre créateur.
const SPAWN_CELLS_PER_FRAME: usize = 8;

/// Une tuile de sol planifiée. `kind` : 0=clean, 1=dirt, 2=rocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingTile {
    pub kind: u8,
    pub pos: Vec3,
    pub yaw: f32,
}

/// Lot de géométrie statique en attente de fusion — spawné par
/// `spawn_stage_arena_on_request` À LA PLACE des N instances. Porte
/// [`StageArenaMarker`] : un switch de stage nettoie aussi un merge en cours.
#[derive(Component)]
pub struct PendingStaticMerge {
    /// "floor" | "walls" — route les compteurs sensor + nomme les entités.
    pub label: &'static str,
    /// Instances : (index de scène dans `scenes`, pose monde).
    pub instances: Vec<(u8, Transform)>,
    /// Scènes GLB sources (sondes + fallback).
    pub scenes: Vec<Handle<Scene>>,
    /// Clé de cache **dérivée du contenu** — cf [`content_cache_key`].
    ///
    /// Elle valait `label:extent_dm`, sur l'hypothèse que le plan était
    /// déterministe par extent. Trois labels l'ont démentie ; le round 2
    /// affichait les murs du round 1 par-dessus les colliders du round 2.
    pub cache_key: String,
    /// Stage propriétaire (fix QA-663 #1) : si le `StageLoadRequest` courant a
    /// changé, ce plan est périmé → jeté sans spawner (sinon un cache-hit la
    /// même frame qu'une transition spawnerait de la géométrie orpheline, hors
    /// du snapshot `q_existing` déjà capturé par le despawn).
    pub stage_id: String,
    pub elapsed_secs: f32,
}

/// Sonde cachée : 1 par (label, scène utilisée), instancie le GLB une fois
/// pour lire les meshes réels. `Visibility::Hidden` → jamais rendue.
#[derive(Component)]
pub struct MergeProbe {
    pub label: &'static str,
    pub scene_idx: u8,
}

/// Cache des fusions par clé. Les `Handle<Mesh>` restent vivants tant que la
/// Resource existe (quelques Mo pour 1-2 tailles d'arène × 2 labels).
#[derive(Resource, Default)]
pub struct MergedStaticCache {
    pub by_key: HashMap<String, Vec<(Handle<Mesh>, Handle<StandardMaterial>, Name)>>,
}

/// File de spawn étalé des meshes fusionnés (mesure 2026-07-20 : spawn
/// one-shot = burst 412 ms de spécialisation/upload au 1er rendu). Drainée
/// [`SPAWN_CELLS_PER_FRAME`] par frame par `sys_drain_merge_spawn_queue`.
/// Porte [`StageArenaMarker`] → nettoyée par les transitions de stage.
#[derive(Component)]
pub struct MergedSpawnQueue {
    pub label: &'static str,
    pub stage_id: String,
    pub items: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Name)>,
    pub cursor: usize,
}

/// Hachage entier stable d'une case de trame — déterministe, sans RNG d'état.
///
/// Story-689. Il remplace `(tx + tz) % 3`, qui dessinait des rayures diagonales
/// régulières, parfaitement visibles sur un sol tuilé.
fn tile_hash(tx: i32, tz: i32) -> u32 {
    let mut h = (tx as u32).wrapping_mul(0x9E37_79B1) ^ (tz as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 13;
    h
}

/// Plan de tuiles du sol — culling circulaire, mélange radial bruité, yaw
/// déterministe 0/90/180/270° anti-répétition. Testable headless.
///
/// Story-689 — le mélange était déterministe et CARRÉ : le centre formait un
/// carré de 48 × 48 m d'une seule et même tuile, et au-delà `(tx+tz) % 3`
/// dessinait des rayures. Sur une arène ronde, la transition elle-même était
/// carrée. Voir le corps pour la dérivation.
pub(crate) fn plan_floor_tiles(extent: f32, tile_size: f32) -> Vec<PendingTile> {
    let tiles_radius = (extent / tile_size).ceil() as i32;
    // `clean_center` n'est plus utilisé : la transition est désormais continue
    // et radiale (story-689), plus un seuil carré.
    let mut tiles = Vec::new();

    for tx in -tiles_radius..=tiles_radius {
        for tz in -tiles_radius..=tiles_radius {
            let pos = Vec3::new(tx as f32 * tile_size, 0.0, tz as f32 * tile_size);
            if pos.length() > extent {
                continue; // cull hors arène (cercle inscrivant l'hex)
            }
            // Story-689 — le mélange était DÉTERMINISTE et CARRÉ.
            //
            // `tx.abs().max(tz.abs())` est une distance de Tchebychev : le
            // centre était un CARRÉ de `clean_center` tuiles entièrement pavé
            // d'une seule et même tuile — 48 × 48 m d'uniformité sur une arène
            // de 80 m. Au-delà, `(tx+tz) % 3` dessinait des rayures diagonales
            // régulières. Et la transition était carrée dans une arène RONDE.
            //
            // Maintenant : distance EUCLIDIENNE (l'arène est un disque, sa
            // transition aussi) et bruit de hachage au lieu d'un motif. La
            // proportion de variantes croît avec le rayon — le centre reste
            // plus propre, mais jamais uniforme.
            let dist = pos.length();
            let t = (dist / extent.max(1.0e-3)).clamp(0.0, 1.0);
            let h = tile_hash(tx, tz);
            // 12 % de variantes au centre, 85 % au bord. Le centre garde sa
            // lisibilité de combat sans être une dalle vide.
            let variant_chance = 0.12 + 0.73 * t;
            let kind = if (h & 0xFFFF) as f32 / 65535.0 >= variant_chance {
                0 // clean
            } else if (h >> 16) & 1 == 0 {
                1 // dirt
            } else {
                2 // rocks
            };
            let yaw_q = (tx.wrapping_mul(7) ^ tz.wrapping_mul(13)).rem_euclid(4);
            let yaw = yaw_q as f32 * std::f32::consts::FRAC_PI_2;
            tiles.push(PendingTile { kind, pos, yaw });
        }
    }
    tiles
}

// ─── La clé de cache se DÉRIVE du contenu (story-690b) ──────────────────────

/// FNV-1a 64 bits — stable entre exécutions, sans dépendance.
///
/// Surtout pas `DefaultHasher` : `RandomState` change de graine à chaque
/// processus, et une clé de cache qui change au lancement n'est pas une clé.
fn fnv1a(bytes: &[u8], mut h: u64) -> u64 {
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Clé de cache **dérivée de ce qu'on s'apprête à fusionner** : les poses et les
/// scènes sources.
///
/// ## Pourquoi, et ce que ça corrige
///
/// La clé était `label:extent_dm`, sur l'hypothèse — écrite noir sur blanc — que
/// « le plan est déterministe par extent ». Elle était vraie pour les deux seuls
/// labels d'origine (`floor`, remparts). Trois choses l'ont périmée sans que
/// personne ne le voie :
///
/// - **`rooms`** (story-683) vient de `plan_rooms(extent, **graine**, cfg)`, et
///   la graine change à chaque round. Les colliders suivaient le nouveau
///   labyrinthe pendant que le cache redessinait celui du round précédent :
///   **des murs invisibles, et des murs traversables** — rapporté en jeu au
///   round 2.
/// - **`floor`** reçoit ses tuiles de l'ambiance du round (story-676). Le ciel
///   changeait d'univers, le sol rejouait le précédent.
/// - **deux salles de même extent** (`forge_sanctum` et `donjon_oublie`, 80 m)
///   partageaient `walls:800` : la première chargée imposait son kit à l'autre.
///
/// ## Pourquoi le CONTENU et pas « extent + graine »
///
/// Ajouter la graine aurait remis la clé à jour de ce qu'on sait aujourd'hui, et
/// le prochain label qui dépendra d'autre chose la repérimerait en silence — la
/// classe resterait ouverte. Dérivée du contenu, la clé est juste **par
/// construction** : même géométrie → même mesh, géométrie différente → rebuild,
/// sans liste de dépendances à tenir.
///
/// Les `f32` sont hachés sur leurs BITS : deux plans issus du même code
/// déterministe sont bit-identiques. `-0.0` et `0.0` ont des bits différents, ce
/// qui peut coûter un rebuild inutile — jamais un cache-hit erroné. L'erreur ne
/// peut pencher que du côté sûr.
fn content_cache_key(
    label: &str,
    instances: &[(u8, Transform)],
    scenes: &[Handle<Scene>],
) -> String {
    let mut h = fnv1a(label.as_bytes(), 0xCBF2_9CE4_8422_2325);
    for s in scenes {
        // Le CHEMIN, pas l'id de handle : deux chargements du même GLB doivent
        // donner la même clé, sinon le cache ne servirait jamais.
        match s.path() {
            Some(p) => h = fnv1a(p.to_string().as_bytes(), h),
            // Handle sans chemin (scène générée) : rien ne permet de dire que
            // deux d'entre eux portent le même contenu. On hache l'identité du
            // handle — la clé cesse alors d'être partageable, donc ce lot se
            // reconstruit. Le seul risque est de ne pas réutiliser, jamais de
            // réutiliser à tort.
            None => h = fnv1a(format!("{:?}", s.id()).as_bytes(), h),
        }
    }
    for (kind, tf) in instances {
        h = fnv1a(&[*kind], h);
        for v in [
            tf.translation.x,
            tf.translation.y,
            tf.translation.z,
            tf.rotation.x,
            tf.rotation.y,
            tf.rotation.z,
            tf.rotation.w,
            tf.scale.x,
            tf.scale.y,
            tf.scale.z,
        ] {
            h = fnv1a(&v.to_bits().to_le_bytes(), h);
        }
    }
    format!("{label}:{h:016x}")
}

/// Spawne le plan + les sondes d'un lot statique à fusionner. Appelé par
/// `spawn_stage_arena_on_request` (sol Inc.1, murs Inc.2).
pub(crate) fn spawn_static_merge(
    commands: &mut Commands,
    label: &'static str,
    instances: Vec<(u8, Transform)>,
    scenes: Vec<Handle<Scene>>,
    stage_id: &str,
) {
    for idx in 0..scenes.len() as u8 {
        if instances.iter().any(|(k, _)| *k == idx) {
            commands.spawn((
                Name::new(format!("MergeProbe_{label}_{idx}")),
                StageArenaMarker,
                MergeProbe {
                    label,
                    scene_idx: idx,
                },
                SceneRoot(scenes[idx as usize].clone()),
                Transform::IDENTITY,
                Visibility::Hidden,
            ));
        }
    }
    let cache_key = content_cache_key(label, &instances, &scenes);
    commands.spawn((
        Name::new(format!("PendingStaticMerge_{label}_{stage_id}")),
        StageArenaMarker,
        PendingStaticMerge {
            label,
            instances,
            scenes,
            cache_key,
            stage_id: stage_id.to_string(),
            elapsed_secs: 0.0,
        },
    ));
}

/// Fallback : spawn instance-par-instance (`SceneRoot` direct, équivalent
/// visuel de l'ancien spawn tuile/prefab).
fn spawn_instances_individually(commands: &mut Commands, pending: &PendingStaticMerge) -> u32 {
    for (kind, tf) in &pending.instances {
        commands.spawn((
            Name::new(format!("StaticFallback_{}", pending.label)),
            StageArenaMarker,
            *tf,
            Visibility::default(),
            SceneRoot(pending.scenes[*kind as usize].clone()),
        ));
    }
    pending.instances.len() as u32
}

fn despawn_probes_of(
    commands: &mut Commands,
    label: &str,
    q_probes: &Query<(Entity, &MergeProbe)>,
) {
    for (e, p) in q_probes.iter() {
        if p.label == label {
            commands.entity(e).despawn();
        }
    }
}

fn note_merged(result: &mut StageLoadResult, label: &str, cells: u32) {
    match label {
        "floor" => result.floor_merged_cells = cells,
        "walls" => result.walls_merged_cells = cells,
        _ => {}
    }
}

/// Construit la géométrie fusionnée dès que les sondes ont instancié leurs
/// meshes. Retry par frame (pattern `sys_collide_authored_pieces` : GLB async).
#[allow(clippy::too_many_arguments)]
pub fn sys_build_merged_static(
    mut commands: Commands,
    time: Res<Time>,
    request: Option<Res<crate::StageLoadRequest>>,
    mut cache: ResMut<MergedStaticCache>,
    mut q_pending: Query<(Entity, &mut PendingStaticMerge)>,
    q_probes: Query<(Entity, &MergeProbe)>,
    q_children: Query<&Children>,
    q_mesh_nodes: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>, &GlobalTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut result: ResMut<StageLoadResult>,
) {
    if q_pending.is_empty() {
        return;
    }
    let current_id = request.as_ref().map(|r| r.stage_id.as_str()).unwrap_or("");

    for (pending_e, mut pending) in &mut q_pending {
        // Fix QA-663 #1 — plan périmé (transition de stage cette frame) : jeter
        // sans rien spawner. Le nouveau stage re-spawnera son propre plan.
        if pending.stage_id != current_id {
            despawn_probes_of(&mut commands, pending.label, &q_probes);
            commands.entity(pending_e).despawn();
            continue;
        }

        // ── Cache hit : re-spawn (étalé) des meshes déjà fusionnés ───────────
        if let Some(cached) = cache.by_key.get(&pending.cache_key) {
            // Story-690b — le cache devient OBSERVABLE. Un hit sur un plan qui
            // a changé était précisément le bug (murs du round précédent
            // redessinés sur les colliders du round courant) : si ça recommence,
            // la ligne le dit au lieu de le taire.
            info!(
                "[stage-arena] fusion '{}' : cache HIT ({} meshes, clé {})",
                pending.label,
                cached.len(),
                pending.cache_key
            );
            commands.spawn((
                Name::new(format!("MergedSpawnQueue_{}", pending.label)),
                StageArenaMarker,
                MergedSpawnQueue {
                    label: pending.label,
                    stage_id: pending.stage_id.clone(),
                    items: cached.clone(),
                    cursor: 0,
                },
            ));
            note_merged(&mut result, pending.label, cached.len() as u32);
            info!(
                "[stage-arena] Static merge '{}': cache hit key={} → {} meshes (spawn étalé)",
                pending.label,
                pending.cache_key,
                cached.len()
            );
            despawn_probes_of(&mut commands, pending.label, &q_probes);
            commands.entity(pending_e).despawn();
            continue;
        }

        pending.elapsed_secs += time.delta_secs();

        // ── Harvest des sondes du label : produit réel du scene-spawner ──────
        let scene_count = pending.scenes.len();
        let mut per_scene: Vec<Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)>> =
            vec![Vec::new(); scene_count];
        for (probe_e, probe) in &q_probes {
            if probe.label != pending.label {
                continue;
            }
            let mut stack: Vec<Entity> = q_children
                .get(probe_e)
                .map(|c| c.iter().collect())
                .unwrap_or_default();
            while let Some(e) = stack.pop() {
                if let Ok((m, mat, gt)) = q_mesh_nodes.get(e) {
                    // Sonde à l'identité → GlobalTransform = transform relatif au GLB.
                    per_scene[probe.scene_idx as usize].push((
                        m.0.clone(),
                        mat.0.clone(),
                        gt.compute_transform(),
                    ));
                }
                if let Ok(ch) = q_children.get(e) {
                    stack.extend(ch.iter());
                }
            }
        }

        let mut used = vec![false; scene_count];
        for (k, _) in &pending.instances {
            used[*k as usize] = true;
        }
        let probes_ready = (0..scene_count).all(|k| !used[k] || !per_scene[k].is_empty());
        let assets_ready = probes_ready
            && per_scene
                .iter()
                .flatten()
                .all(|(h, _, _)| meshes.get(h).is_some());

        if !assets_ready {
            if pending.elapsed_secs > MERGE_TIMEOUT_SECS {
                warn!(
                    "[stage-arena] Static merge '{}' TIMEOUT ({:.1}s, GLB KO ?) — fallback spawn individuel",
                    pending.label, pending.elapsed_secs
                );
                spawn_instances_individually(&mut commands, &pending);
                note_merged(&mut result, pending.label, 0);
                despawn_probes_of(&mut commands, pending.label, &q_probes);
                commands.entity(pending_e).despawn();
            }
            continue;
        }

        // ── Fusion par (cellule, matériau) — vertices bakés en espace monde ──
        let mut acc: HashMap<
            (i32, i32, AssetId<StandardMaterial>),
            (Mesh, Handle<StandardMaterial>),
        > = HashMap::new();
        let mut merge_failed = false;
        'instances: for (kind, inst_tf) in &pending.instances {
            let cx = (inst_tf.translation.x / MERGE_CELL_WORLD_M).floor() as i32;
            let cz = (inst_tf.translation.z / MERGE_CELL_WORLD_M).floor() as i32;
            for (mesh_h, mat_h, rel) in &per_scene[*kind as usize] {
                let Some(src) = meshes.get(mesh_h) else {
                    continue;
                };
                let piece = src.clone().transformed_by(inst_tf.mul_transform(*rel));
                match acc.entry((cx, cz, mat_h.id())) {
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        if e.get_mut().0.merge(&piece).is_err() {
                            merge_failed = true;
                            break 'instances;
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(v) => {
                        v.insert((piece, mat_h.clone()));
                    }
                }
            }
        }
        if merge_failed {
            // Attributs incompatibles (ne devrait pas arriver : même mesh
            // source) — on ne livre jamais une géométrie partielle.
            warn!(
                "[stage-arena] Static merge '{}': Mesh::merge a échoué — fallback individuel",
                pending.label
            );
            spawn_instances_individually(&mut commands, &pending);
            note_merged(&mut result, pending.label, 0);
            despawn_probes_of(&mut commands, pending.label, &q_probes);
            commands.entity(pending_e).despawn();
            continue;
        }

        let instance_count = pending.instances.len();
        let mut cached: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Name)> =
            Vec::with_capacity(acc.len());
        for ((cx, cz, _), (mesh, mat)) in acc {
            let name = Name::new(format!("StaticMerged_{}_{cx}_{cz}", pending.label));
            let handle = meshes.add(mesh);
            cached.push((handle, mat, name));
        }
        info!(
            "[stage-arena] Static merge '{}': {} instances → {} meshes fusionnés (cellules {:.0}m, key={}, spawn étalé {}/frame)",
            pending.label,
            instance_count,
            cached.len(),
            MERGE_CELL_WORLD_M,
            pending.cache_key,
            SPAWN_CELLS_PER_FRAME
        );
        note_merged(&mut result, pending.label, cached.len() as u32);
        commands.spawn((
            Name::new(format!("MergedSpawnQueue_{}", pending.label)),
            StageArenaMarker,
            MergedSpawnQueue {
                label: pending.label,
                stage_id: pending.stage_id.clone(),
                items: cached.clone(),
                cursor: 0,
            },
        ));
        cache.by_key.insert(pending.cache_key.clone(), cached);
        despawn_probes_of(&mut commands, pending.label, &q_probes);
        commands.entity(pending_e).despawn();
    }

    // Compte de pendings restants (les despawns ci-dessus sont différés → la
    // valeur se stabilise à la frame suivante ; suffisant pour le sensor 1Hz).
    result.static_merge_pending = q_pending.iter().count() as u32;
}

/// Draine les files de spawn étalé : [`SPAWN_CELLS_PER_FRAME`] meshes par
/// frame et par file. Étale la spécialisation pipeline + l'upload GPU sur
/// ~11 frames (fix du burst 412 ms mesuré au 1er rendu, 2026-07-20).
pub fn sys_drain_merge_spawn_queue(
    mut commands: Commands,
    request: Option<Res<crate::StageLoadRequest>>,
    mut q_queues: Query<(Entity, &mut MergedSpawnQueue)>,
) {
    if q_queues.is_empty() {
        return;
    }
    let current_id = request.as_ref().map(|r| r.stage_id.as_str()).unwrap_or("");
    for (queue_e, mut queue) in &mut q_queues {
        // Même garde anti-péremption que le build (QA-663 #1).
        if queue.stage_id != current_id {
            commands.entity(queue_e).despawn();
            continue;
        }
        let end = (queue.cursor + SPAWN_CELLS_PER_FRAME).min(queue.items.len());
        for i in queue.cursor..end {
            let (mesh_h, mat_h, name) = &queue.items[i];
            commands.spawn((
                name.clone(),
                StageArenaMarker,
                Mesh3d(mesh_h.clone()),
                MeshMaterial3d(mat_h.clone()),
            ));
        }
        queue.cursor = end;
        if queue.cursor >= queue.items.len() {
            commands.entity(queue_e).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE: f32 = 4.0;

    #[test]
    fn plan_matches_legacy_counts_and_bounds() {
        // Extent 90 m (crypts_of_anvil) : ~1 600 tuiles, toutes dans le cercle.
        let tiles = plan_floor_tiles(90.0, TILE);
        assert!(
            (1400..=1800).contains(&tiles.len()),
            "compte plausible pour r=23, got {}",
            tiles.len()
        );
        for t in &tiles {
            assert!(t.pos.length() <= 90.0 + f32::EPSILON, "tuile hors arène");
            assert!(t.kind < 3);
        }
    }

    #[test]
    fn plan_is_deterministic() {
        assert_eq!(plan_floor_tiles(60.0, TILE), plan_floor_tiles(60.0, TILE));
    }

    // ── La clé de cache dérive du contenu (story-690b) ───────────────────────
    //
    // Le défaut d'origine, rapporté en jeu au round 2 : la clé valait
    // `label:extent`, donc deux plans DIFFÉRENTS de même extent la partageaient.
    // Le cache redessinait les murs du round précédent par-dessus les colliders
    // du round courant — murs invisibles d'un côté, traversables de l'autre.

    fn pose(x: f32, z: f32) -> (u8, Transform) {
        (0, Transform::from_xyz(x, 0.0, z))
    }

    #[test]
    fn two_different_layouts_never_share_a_key() {
        // C'EST le test qui aurait attrapé le bug : même label, même extent,
        // murs ailleurs.
        let a = content_cache_key("rooms", &[pose(0.0, 0.0), pose(4.0, 0.0)], &[]);
        let b = content_cache_key("rooms", &[pose(0.0, 0.0), pose(8.0, 0.0)], &[]);
        assert_ne!(a, b);
    }

    #[test]
    fn the_same_layout_still_hits_the_cache() {
        // Le cache doit continuer de servir : c'est sa raison d'être (zéro
        // rebuild à la ré-entrée dans une salle identique).
        let plan = [pose(0.0, 0.0), pose(4.0, 0.0), pose(8.0, 0.0)];
        assert_eq!(
            content_cache_key("floor", &plan, &[]),
            content_cache_key("floor", &plan, &[])
        );
    }

    #[test]
    fn a_different_count_of_walls_changes_the_key() {
        let short = [pose(0.0, 0.0), pose(4.0, 0.0)];
        let long = [pose(0.0, 0.0), pose(4.0, 0.0), pose(8.0, 0.0)];
        assert_ne!(
            content_cache_key("rooms", &short, &[]),
            content_cache_key("rooms", &long, &[])
        );
    }

    #[test]
    fn rotation_alone_changes_the_key() {
        // Deux murs au même point mais orientés différemment ne donnent pas le
        // même mesh — l'un barre le nord, l'autre l'est.
        let flat = [(0u8, Transform::from_xyz(0.0, 0.0, 0.0))];
        let turned = [(
            0u8,
            Transform::from_xyz(0.0, 0.0, 0.0)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        )];
        assert_ne!(
            content_cache_key("rooms", &flat, &[]),
            content_cache_key("rooms", &turned, &[])
        );
    }

    #[test]
    fn the_tile_kind_changes_the_key() {
        // Le sol reçoit ses tuiles de l'ambiance du round : deux univers ne
        // doivent pas partager un mesh (le ciel changeait, le sol non).
        let clean = [(0u8, Transform::IDENTITY)];
        let rocks = [(2u8, Transform::IDENTITY)];
        assert_ne!(
            content_cache_key("floor", &clean, &[]),
            content_cache_key("floor", &rocks, &[])
        );
    }

    #[test]
    fn two_labels_with_identical_poses_stay_separate() {
        let plan = [pose(1.0, 2.0)];
        assert_ne!(
            content_cache_key("floor", &plan, &[]),
            content_cache_key("rooms", &plan, &[])
        );
    }

    #[test]
    fn an_empty_plan_has_a_key_of_its_own() {
        // Ne doit pas collisionner avec un plan non vide du même label.
        assert_ne!(
            content_cache_key("rooms", &[], &[]),
            content_cache_key("rooms", &[pose(0.0, 0.0)], &[])
        );
    }

    #[test]
    fn the_key_is_stable_across_calls_within_a_process() {
        // Garde-fou anti-`DefaultHasher` : une clé re-tirée doit être la MÊME.
        // Une clé qui change de graine au lancement n'est pas une clé.
        let plan = [pose(3.0, 7.0)];
        let first = content_cache_key("walls", &plan, &[]);
        for _ in 0..5 {
            assert_eq!(content_cache_key("walls", &plan, &[]), first);
        }
        assert!(first.starts_with("walls:"), "label lisible en log : {first}");
    }

    /// Story-689 — ce test asserait `center.kind == 0`, c'est-à-dire EXACTEMENT
    /// le défaut : le centre était un carré de 48 × 48 m entièrement pavé de la
    /// même tuile. Il vérifie maintenant le vrai contrat — le centre reste
    /// MAJORITAIREMENT propre (lisibilité de combat) sans être uniforme.
    #[test]
    fn the_centre_stays_mostly_clean_without_being_uniform() {
        let tiles = plan_floor_tiles(90.0, TILE);
        let inner: Vec<&PendingTile> = tiles.iter().filter(|t| t.pos.length() <= 20.0).collect();
        assert!(inner.len() > 20, "échantillon central trop petit");
        let clean = inner.iter().filter(|t| t.kind == 0).count();
        let share = clean as f32 / inner.len() as f32;
        assert!(
            share >= 0.70,
            "le centre doit rester majoritairement propre ({:.0} % seulement)",
            share * 100.0
        );
        assert!(
            share < 1.0,
            "le centre est UNIFORME — c'est la dalle de 48 m d'avant"
        );
    }

    #[test]
    fn cell_grouping_upper_bound() {
        // Cellules de 32 m → pour r=23 tuiles (~46×46 = 92 m), ≤ 7×7 cellules ;
        // × ≤3 matériaux = borne large mais significative vs 1 600 entités.
        let tiles = plan_floor_tiles(90.0, TILE);
        let mut cells = std::collections::HashSet::new();
        for t in &tiles {
            cells.insert((
                (t.pos.x / MERGE_CELL_WORLD_M).floor() as i32,
                (t.pos.z / MERGE_CELL_WORLD_M).floor() as i32,
            ));
        }
        assert!(
            cells.len() <= 49,
            "{} cellules pour r=23 — grouping cassé ?",
            cells.len()
        );
    }

    #[test]
    fn yaw_is_quarter_turns() {
        for t in plan_floor_tiles(40.0, TILE) {
            let q = t.yaw / std::f32::consts::FRAC_PI_2;
            assert!((q - q.round()).abs() < 1e-5, "yaw non quantifié: {}", t.yaw);
        }
    }
}

#[cfg(test)]
mod floor_mix_tests {
    use super::*;
    use std::collections::HashMap;

    /// Story-689 — **le centre ne doit plus être une dalle uniforme.**
    ///
    /// L'ancien mélange utilisait `tx.abs().max(tz.abs())` — une distance de
    /// Tchebychev — avec un seuil à `tiles_radius / 3` : sur une arène de 80 m,
    /// le centre était un CARRÉ de 48 × 48 m entièrement pavé de la même tuile.
    #[test]
    fn the_centre_is_not_a_single_uniform_slab() {
        let tiles = plan_floor_tiles(80.0, 4.0);
        let inner: Vec<&PendingTile> = tiles.iter().filter(|t| t.pos.length() <= 20.0).collect();
        assert!(inner.len() > 20, "échantillon central trop petit");
        let kinds: std::collections::HashSet<u8> = inner.iter().map(|t| t.kind).collect();
        assert!(
            kinds.len() >= 2,
            "le centre n'utilise qu'une seule tuile — c'est la dalle uniforme d'avant"
        );
    }

    /// La transition doit être RADIALE : l'arène est un disque, pas un carré.
    /// Deux tuiles à la même distance du centre doivent avoir la même chance
    /// d'être une variante, quelle que soit leur direction.
    #[test]
    fn the_transition_follows_the_disc_not_a_square() {
        let tiles = plan_floor_tiles(80.0, 4.0);
        // Deux couronnes fines, l'une dans l'axe, l'autre en diagonale.
        let ratio_in = |lo: f32, hi: f32, diagonal: bool| {
            let sel: Vec<&PendingTile> = tiles
                .iter()
                .filter(|t| {
                    let d = t.pos.length();
                    if d < lo || d > hi {
                        return false;
                    }
                    let axis = t.pos.x.abs().min(t.pos.z.abs()) < 6.0;
                    if diagonal {
                        !axis
                    } else {
                        axis
                    }
                })
                .collect();
            if sel.is_empty() {
                return -1.0;
            }
            sel.iter().filter(|t| t.kind != 0).count() as f32 / sel.len() as f32
        };
        let axis = ratio_in(40.0, 50.0, false);
        let diag = ratio_in(40.0, 50.0, true);
        assert!(axis >= 0.0 && diag >= 0.0, "échantillons vides");
        assert!(
            (axis - diag).abs() < 0.30,
            "à distance égale, l'axe ({axis:.2}) et la diagonale ({diag:.2}) diffèrent — \
             la transition est encore carrée"
        );
    }

    /// La proportion de variantes doit CROÎTRE avec le rayon : le centre plus
    /// propre (lisibilité de combat), les bords plus sales.
    #[test]
    fn variants_grow_with_the_radius() {
        let tiles = plan_floor_tiles(80.0, 4.0);
        let share = |lo: f32, hi: f32| {
            let sel: Vec<&PendingTile> = tiles
                .iter()
                .filter(|t| t.pos.length() >= lo && t.pos.length() < hi)
                .collect();
            sel.iter().filter(|t| t.kind != 0).count() as f32 / sel.len().max(1) as f32
        };
        let c = share(0.0, 20.0);
        let e = share(55.0, 80.0);
        assert!(
            e > c,
            "les bords ({e:.2}) doivent être plus variés que le centre ({c:.2})"
        );
        assert!(c > 0.0, "le centre doit tout de même varier un peu");
    }

    /// Aucune tuile ne doit sortir du disque, et le plan reste déterministe.
    #[test]
    fn the_plan_stays_inside_the_disc_and_is_deterministic() {
        let a = plan_floor_tiles(80.0, 4.0);
        let b = plan_floor_tiles(80.0, 4.0);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.kind, y.kind);
        }
        for t in &a {
            assert!(t.pos.length() <= 80.0 + 1e-3);
        }
    }

    /// Les trois tuiles doivent TOUTES servir — une variante jamais tirée est un
    /// asset chargé pour rien.
    #[test]
    fn all_three_tile_slots_are_actually_used() {
        let tiles = plan_floor_tiles(80.0, 4.0);
        let mut count: HashMap<u8, usize> = HashMap::new();
        for t in &tiles {
            *count.entry(t.kind).or_default() += 1;
        }
        for k in 0..3u8 {
            assert!(
                count.get(&k).copied().unwrap_or(0) > 0,
                "la tuile {k} n'est jamais posée — chargée pour rien"
            );
        }
    }
}
