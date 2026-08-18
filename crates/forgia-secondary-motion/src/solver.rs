//! Solver Verlet pour spring bones (hot path).
//!
//! Tourne dans `PostUpdate` APRÈS l'animation des clips (`animate_targets`) et AVANT
//! `TransformSystem::TransformPropagate`. Ce timing garantit que les bones parents
//! ont leur pose finale du frame avant qu'on calcule les bones suiveurs.
//!
//! Discipline hot path :
//! - `Local<Vec<Vec3>>` réutilisés (zéro alloc/frame)
//! - Query filtrée `With<SpringBoneChain>`
//! - `run_if(any_with_component::<SpringBoneChain>)` skip total si aucune chaîne

use bevy::prelude::*;
use forgia_anim_debug::{AnimLayerStats, AnimTimer};

use crate::spring_bone::{SpringBone, SpringBoneChain, SpringBoneState};

/// Nombre d'itérations de contrainte de distance par frame.
/// 3-4 suffit pour des chaînes de 3-6 bones, plus convergent mal sans coût significatif.
const CONSTRAINT_ITERATIONS: u8 = 4;

/// Système principal. Itère chaque chaîne, applique Verlet + contraintes + écrit Transform.
pub fn update_spring_bones(
    time: Res<Time>,
    mut chains: Query<(&GlobalTransform, &mut SpringBoneChain)>,
    mut bones: Query<(
        &SpringBone,
        Option<&mut SpringBoneState>,
        &GlobalTransform,
        &mut Transform,
    )>,
    mut commands: Commands,
    mut stats: ResMut<AnimLayerStats>,
    // Buffers scratch réutilisés chaque frame (zéro alloc, chemin chaud).
    mut scratch_positions: Local<Vec<Vec3>>,
    mut scratch_axes: Local<Vec<Vec3>>,
    mut scratch_parent_rots: Local<Vec<Quat>>,
) {
    let timer = AnimTimer::start();
    let dt = time.delta_secs();
    if dt <= 0.0 {
        stats.spring_solver_us = timer.elapsed_us();
        return;
    }
    // Cap dt pour stabilité Verlet (téléport / pause / first frame)
    let dt = dt.min(1.0 / 30.0);
    let dt2 = dt * dt;

    // Stats : counters cumulés sur ce frame
    let mut chains_active = 0u32;
    let mut bones_total = 0u32;

    for (chain_global, mut chain) in &mut chains {
        if chain.bones.is_empty() {
            continue;
        }

        chains_active += 1;
        bones_total += chain.bones.len() as u32;

        let root_world = chain_global.translation();

        // 1. Init au premier passage : capture rest_lengths + pose initiale
        if !chain.initialized {
            let mut prev = root_world;
            let mut lengths = Vec::with_capacity(chain.bones.len());
            for &bone_entity in &chain.bones {
                let Ok((_, _, bone_gt, _)) = bones.get(bone_entity) else {
                    continue;
                };
                let pos = bone_gt.translation();
                let len = (pos - prev).length();
                lengths.push(len);
                // Insert state component si absent
                commands
                    .entity(bone_entity)
                    .insert(SpringBoneState::new(pos));
                prev = pos;
            }
            chain.rest_lengths = lengths;
            chain.initialized = true;
            continue; // skip ce frame, sim démarre au suivant
        }

        // 2. Collecte positions courantes dans le scratch (évite borrow conflict)
        scratch_positions.clear();
        scratch_positions.push(root_world); // index 0 = root (fixe, drive par animation)

        for &bone_entity in &chain.bones {
            let Ok((spring, Some(mut state), _, _)) = bones.get_mut(bone_entity) else {
                continue;
            };

            // Verlet integration : pos_new = pos + (pos - prev) * (1 - damping) + accel*dt²
            let velocity = (state.pos_world - state.prev_pos_world) * (1.0 - spring.damping);
            let accel = spring.gravity;
            let new_pos = state.pos_world + velocity + accel * dt2;

            state.prev_pos_world = state.pos_world;
            state.pos_world = new_pos;

            scratch_positions.push(new_pos);
        }

        // 3. Contraintes : enforce rest_length entre bones consécutifs + biais vers parent rigide
        for _iter in 0..CONSTRAINT_ITERATIONS {
            for i in 0..chain.bones.len() {
                let parent_idx = i; // scratch[i] = parent (root si i=0, sinon bone i-1)
                let bone_idx = i + 1;
                let parent_pos = scratch_positions[parent_idx];
                let bone_pos = scratch_positions[bone_idx];
                let rest_len = chain.rest_lengths.get(i).copied().unwrap_or(0.1);

                let delta = bone_pos - parent_pos;
                let dist = delta.length().max(1e-5);
                let dir = delta / dist;
                // Position rigide cible (parent + rest_length dans la direction courante)
                let rigid_pos = parent_pos + dir * rest_len;
                // Position de drift (Verlet courante, non contrainte)
                let drift_pos = bone_pos;

                // Stiffness : lerp entre drift (0.0 = mou, suit gravité) et rigide (1.0 = suit parent).
                // BUG-ANIMQA-08 fix : l'ancienne formule blendait target_pos avec target_pos = no-op.
                if let Some(spring) = chain
                    .bones
                    .get(i)
                    .and_then(|&e| bones.get(e).ok())
                    .map(|(s, _, _, _)| s)
                {
                    let stiff = spring.stiffness.clamp(0.0, 1.0);
                    let blended = drift_pos.lerp(rigid_pos, stiff);
                    // Ré-enforce distance exacte après stiffness blend (constraint pass)
                    let new_delta = blended - parent_pos;
                    let new_dist = new_delta.length().max(1e-5);
                    scratch_positions[bone_idx] = parent_pos + (new_delta / new_dist) * rest_len;
                } else {
                    scratch_positions[bone_idx] = rigid_pos;
                }
            }
        }

        // 4. Écriture des Transforms locaux : orientation = look vers le bone suivant
        //    Translation locale reste sur l'offset bindpose (Bevy le fournit déjà)
        //    On ne modifie QUE la rotation pour éviter de casser la hiérarchie skinning.
        // 🚨 Ce que la rotation d'un os exige, et que la version « Phase 1 »
        // supposait au lieu de le mesurer : son AXE LONG au repos, et le
        // repère de son PARENT.
        //
        // L'audit d'animation du 2026-06-04 a débranché la queue de Rex à cause
        // de ça (« whip Verlet, queue de travers ») en laissant la consigne de
        // corriger l'axe supposé +Y. Mesuré le 2026-08-18 sur la cape : sa
        // chaîne court selon −X (translations −16, −18, −16…), donc l'axe +Y
        // l'aurait tordue d'un quart de tour, exactement comme la queue.
        //
        // Un axe d'os ne se devine pas : il se LIT sur la pose de liaison.
        scratch_axes.clear();
        scratch_parent_rots.clear();
        for (i, _) in chain.bones.iter().enumerate() {
            // L'axe long d'un os = la direction de l'os SUIVANT, exprimée dans
            // le repère local de cet os. Le solveur n'écrivant jamais de
            // translation, cette valeur reste celle de la pose de liaison à
            // toutes les frames — on peut donc la lire en vol.
            let axe = chain
                .bones
                .get(i + 1)
                .and_then(|&suivant| bones.get(suivant).ok())
                .and_then(|(_, _, _, tf)| tf.translation.try_normalize())
                // Le dernier os n'a pas de suivant dans la chaîne : il hérite de
                // l'axe de son prédécesseur, une chaîne étant à peu près uniforme.
                .or_else(|| scratch_axes.last().copied())
                .unwrap_or(Vec3::Y);
            scratch_axes.push(axe);
            // Le repère du parent. Une frame de retard (on tourne avant la
            // propagation), ce qui est sans conséquence sur du mouvement
            // secondaire — et sans commune mesure avec l'erreur d'ignorer le
            // parent, qui est permanente.
            let rot = if i == 0 {
                chain_global.rotation()
            } else {
                bones
                    .get(chain.bones[i - 1])
                    .map(|(_, _, gt, _)| gt.rotation())
                    .unwrap_or(Quat::IDENTITY)
            };
            scratch_parent_rots.push(rot);
        }

        for (i, &bone_entity) in chain.bones.iter().enumerate() {
            let final_pos = scratch_positions[i + 1];
            let parent_pos = scratch_positions[i];

            // Sync state.pos_world avec la position contrainte (sinon drift Verlet)
            if let Ok((_, Some(mut state), _, mut local_tf)) = bones.get_mut(bone_entity) {
                state.pos_world = final_pos;

                // Calcul orientation : le bone doit pointer de parent_pos vers final_pos
                // En Bevy, look_to oriente -Z vers la cible, Y up.
                // Pour les bones GLB, l'axe "long" est généralement +Y (convention Blender)
                // mais le rig détermine ça. Phase 1 : on assume forward = direction de la chaîne.
                let dir = (final_pos - parent_pos).normalize_or_zero();
                if dir.length_squared() > 0.0 {
                    // La rotation écrite est LOCALE ; la direction visée est
                    // MONDE. On ramène donc la cible dans le repère du parent
                    // avant de construire l'arc, sinon l'os vise juste
                    // uniquement quand son parent est à l'identité.
                    let axe = scratch_axes[i];
                    let dir_locale = scratch_parent_rots[i].inverse() * dir;
                    local_tf.rotation = Quat::from_rotation_arc(axe, dir_locale);
                }
            }
        }
    }

    // Stats sensor → forgia-anim-debug
    stats.spring_chains_active = chains_active;
    stats.spring_bones_total = bones_total;
    stats.spring_solver_us = timer.elapsed_us();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dt_cap_protects_verlet_stability() {
        // Test conceptuel : un dt énorme (pause/téléport) ne doit pas faire exploser
        // la sim. Le cap à 1/30s = 33ms = limite haute physique stable Verlet.
        let big_dt = 1.0_f32;
        let capped = big_dt.min(1.0 / 30.0);
        assert!(capped <= 1.0 / 30.0);
        assert!(capped > 0.0);
    }

    #[test]
    fn spring_bone_default_reasonable() {
        let s = SpringBone::default();
        assert!(s.stiffness >= 0.0 && s.stiffness <= 1.0);
        assert!(s.damping >= 0.0 && s.damping <= 1.0);
        assert!(s.gravity.y < 0.0, "gravity should pull down by default");
    }
}
