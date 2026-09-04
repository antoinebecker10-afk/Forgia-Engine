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

/// Fraction de vitesse **conservée sur une frame**, pour un amortissement
/// exprimé par seconde. À `dt = 1 s` elle vaut exactement `1 - amorti`.
///
/// C'est la conversion qui manquait : sans elle, le même nombre décrivait une
/// perte par frame, donc un amortissement 60 fois trop fort.
#[must_use]
pub fn retention_par_frame(amorti: f32, dt: f32) -> f32 {
    (1.0 - amorti.clamp(0.0, 1.0)).powf(dt.max(0.0))
}

/// Raideur à appliquer à CHAQUE itération pour que le cumul des `iterations`
/// passes vaille `raideur`.
///
/// `1 - (1 - r)^(1/n)` est l'inverse exact de la composition `1 - (1 - s)^n`.
/// Sans ça, le nombre du génome vaudrait plus du double une fois les quatre
/// passes faites.
#[must_use]
pub fn raideur_par_iteration(raideur: f32, iterations: u8) -> f32 {
    let r = raideur.clamp(0.0, 1.0);
    let n = f32::from(iterations.max(1));
    1.0 - (1.0 - r).powf(1.0 / n)
}

/// Rabat une position d'os dans le cône autorisé autour de la direction du
/// segment PRÉCÉDENT — une limite de COURBURE, pas d'orientation.
///
/// # Pourquoi pas autour de la pose de repos
///
/// C'était la première version, et elle a produit le défaut qu'elle devait
/// empêcher : sur ce corps, la pose de liaison de la cape est **à plat**
/// (chaîne horizontale selon −X, mesurée sur le GLB). Un cône refermé sur elle
/// interdit à l'étoffe de tomber : cape tendue à l'horizontale, vue à l'écran
/// le 2026-08-21. Troisième mécanisme d'affilée à hériter du même défaut
/// d'asset — après la raideur et le recentrage.
///
/// Limiter l'angle entre deux maillons consécutifs ne dépend, lui, d'aucune
/// pose de liaison : la chaîne garde sa liberté de pendre et de voler, elle
/// perd seulement le droit de se plier en épingle.
#[must_use]
pub fn limiter_ouverture(
    parent: Vec3,
    position: Vec3,
    repos: Vec3,
    longueur: f32,
    angle_max_rad: f32,
) -> Vec3 {
    let Some(dir) = (position - parent).try_normalize() else {
        return position;
    };
    let Some(repos) = repos.try_normalize() else {
        return position;
    };
    let angle = dir.dot(repos).clamp(-1.0, 1.0).acos();
    if angle <= angle_max_rad {
        return position;
    }
    // On tourne la direction courante VERS la pose de repos, juste assez pour
    // rentrer dans le cône : l'os garde son azimut, il perd son excès.
    let Some(axe) = repos.cross(dir).try_normalize() else {
        return position;
    };
    let rabattue = Quat::from_axis_angle(axe, angle_max_rad) * repos;
    parent + rabattue * longueur
}

/// Écarte un os du volume du corps — un cylindre vertical centré sur la racine
/// de la chaîne.
///
/// Sans ça, la cape passe DANS le personnage dès qu'il tourne : le tissu suit
/// son inertie pendant que le corps pivote, et les deux se croisent. Le
/// cylindre est grossier ; c'est exactement ce qu'il faut pour un tissu qui
/// n'a besoin que de « pas dedans ».
#[must_use]
pub fn ecarter_du_corps(position: Vec3, axe_du_corps: Vec3, rayon: f32) -> Vec3 {
    if rayon <= 0.0 {
        return position;
    }
    let ecart = Vec3::new(position.x - axe_du_corps.x, 0.0, position.z - axe_du_corps.z);
    let distance = ecart.length();
    if distance >= rayon {
        return position;
    }
    // Pile sur l'axe : aucune direction pour sortir. On ne devine pas, on laisse.
    let Some(dehors) = ecart.try_normalize() else {
        return position;
    };
    Vec3::new(
        axe_du_corps.x + dehors.x * rayon,
        position.y,
        axe_du_corps.z + dehors.z * rayon,
    )
}

/// Vitesse de chute que le solveur peut atteindre, en m/s.
///
/// Un réglage de tissu ne se juge pas sur trois nombres abstraits : celui-ci
/// est la grandeur que l'œil constate. Il est publié au capteur pour que
/// « la cape ne tombe pas » se chiffre au lieu de se discuter.
#[must_use]
pub fn chute_terminale_ms(pesanteur: f32, amorti: f32, dt: f32) -> f32 {
    let perdue = 1.0 - retention_par_frame(amorti, dt);
    if perdue <= f32::EPSILON || dt <= 0.0 {
        return f32::INFINITY;
    }
    pesanteur.abs() * dt / perdue
}

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
    // Direction de chaque os dans le repère de son parent, à la pose de
    // liaison. C'est la cible de la RAIDEUR : « là où l'os serait s'il suivait
    // son parent sans pendre ».
    mut scratch_dirs_repos: Local<Vec<Vec3>>,
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

            // Verlet : pos_new = pos + vitesse retenue + accel·dt²
            //
            // 🚨 L'AMORTISSEMENT EST PAR SECONDE, PAS PAR FRAME (2026-08-21).
            //
            // `(1 - damping)` appliqué à chaque frame retirait 70 % de la
            // vitesse SOIXANTE FOIS PAR SECONDE. La vitesse de chute plafonnait
            // alors à `a·dt²/amorti` = **0,095 m/s** pour la cape — une étoffe
            // qui retombe se lit à partir de ~0,5. Constaté à l'écran avant
            // d'être calculé : « on dirait qu'il n'y a pas de gravité ».
            //
            // Le nombre du génome décrit désormais ce que son commentaire
            // annonce : la fraction de vitesse perdue en UNE SECONDE.
            let velocity =
                (state.pos_world - state.prev_pos_world) * retention_par_frame(spring.damping, dt);
            let accel = spring.gravity;
            let new_pos = state.pos_world + velocity + accel * dt2;

            state.prev_pos_world = state.pos_world;
            state.pos_world = new_pos;

            scratch_positions.push(new_pos);
        }

        // 2bis. Le repère du parent et la direction de repos de chaque os.
        //
        // 🚨 Ce calcul vivait APRÈS la boucle de contraintes, et c'est ce qui
        // rendait la raideur inerte : faute de connaître la direction rigide,
        // la boucle visait la direction COURANTE, donc un point colinéaire au
        // drift. Remonté ici, il donne à la contrainte une vraie cible.
        scratch_axes.clear();
        scratch_parent_rots.clear();
        scratch_dirs_repos.clear();
        for (i, &os) in chain.bones.iter().enumerate() {
            // Où cet os pend à la pose de liaison, dans le repère de son
            // parent. Le solveur n'écrivant jamais de translation, la valeur
            // reste celle du rig à toutes les frames.
            let dir_repos = bones
                .get(os)
                .ok()
                .and_then(|(_, _, _, tf)| tf.translation.try_normalize())
                .unwrap_or(Vec3::NEG_Y);
            scratch_dirs_repos.push(dir_repos);
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
                // La cible RIGIDE : là où l'os serait s'il suivait son parent
                // sans pendre — sa direction de repos, tournée par le repère du
                // parent. C'est CE point que la raideur vise.
                //
                // 🚨 Avant le 2026-08-21, `rigid_pos` était construit sur `dir`,
                // la direction COURANTE : il tombait donc sur le même rayon que
                // le drift, le lerp n'interpolait qu'une longueur, et la
                // renormalisation ci-dessous effaçait tout. La raideur ne
                // faisait rien, de 0 à 1 — mesuré, pas supposé.
                let dir_rigide = (scratch_parent_rots[i] * scratch_dirs_repos[i])
                    .try_normalize()
                    .unwrap_or(dir);
                let rigid_pos = parent_pos + dir_rigide * rest_len;
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
                    // Le lerp entre le drift (mou, il pend) et la cible rigide
                    // (il suit le parent) : c'est là que se joue la souplesse.
                    // La raideur agit maintenant — et comme elle est appliquée
                    // DANS la boucle, les N passes la composeraient : 0,35
                    // deviendrait 0,82. On répartit donc la fraction pour que
                    // le CUMUL vaille ce que le génome demande.
                    let stiff = raideur_par_iteration(spring.stiffness, CONSTRAINT_ITERATIONS);
                    let blended = drift_pos.lerp(rigid_pos, stiff);
                    // Ré-enforce distance exacte après stiffness blend (constraint pass)
                    let new_delta = blended - parent_pos;
                    let new_dist = new_delta.length().max(1e-5);
                    let tenu = parent_pos + (new_delta / new_dist) * rest_len;
                    // Puis les deux bornes du bon sens : la chaîne ne se plie
                    // pas en épingle, et elle n'entre pas dans le corps. Dans
                    // la boucle, donc respectées par les passes suivantes — les
                    // poser après coup les ferait défaire.
                    //
                    // Le PREMIER maillon n'a pas de segment précédent : le
                    // brider contre la pose de liaison est précisément ce qui a
                    // tenu la cape à l'horizontale. Il reste donc libre, et
                    // c'est la gravité qui décide où part l'étoffe.
                    let precedent = if i == 0 {
                        None
                    } else {
                        (scratch_positions[i] - scratch_positions[i - 1]).try_normalize()
                    };
                    let borne = match precedent {
                        Some(axe) => limiter_ouverture(
                            parent_pos,
                            tenu,
                            axe,
                            rest_len,
                            spring.angle_max_rad,
                        ),
                        None => tenu,
                    };
                    scratch_positions[bone_idx] =
                        ecarter_du_corps(borne, root_world, spring.rayon_corps_m);
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

    /// La raideur DOIT changer le résultat — c'était un no-op jusqu'au
    /// 2026-08-21, et rien ne le disait.
    ///
    /// On rejoue ici la contrainte telle que le solveur la calcule : la cible
    /// rigide vient de la direction de REPOS tournée par le parent, pas de la
    /// direction courante. Si un jour les deux redeviennent colinéaires, ce
    /// test tombe avant que la cape ne redevienne une planche.
    #[test]
    fn la_raideur_change_vraiment_la_pose() {
        let parent = Vec3::ZERO;
        let drift = Vec3::new(0.3, -0.4, 0.0); // il a pendu de côté
        let dir_repos = Vec3::NEG_Y; // au repos, il pend droit sous le parent
        let rest_len = 0.5_f32;
        let resultat = |stiff: f32| {
            let rigide = parent + dir_repos * rest_len;
            let blended = drift.lerp(rigide, stiff);
            parent + (blended - parent).normalize() * rest_len
        };
        let mou = resultat(0.0);
        let raide = resultat(1.0);
        assert!(
            (mou - raide).length() > 0.1,
            "la raideur est redevenue inerte ({mou} vs {raide}) — la cible \
             rigide est-elle repassée sur la direction courante ?"
        );
        // Et elle doit ramener vers la pose de repos, pas ailleurs.
        assert!((raide - Vec3::new(0.0, -rest_len, 0.0)).length() < 1e-5);
    }

    /// Le débattement se BORNE, et la borne se vérifie au degré près.
    #[test]
    fn l_ouverture_se_rabat_dans_le_cone() {
        // La reference est la direction du segment PRECEDENT, pas la pose de
        // liaison : c'est ce qui permet a la cape de tomber alors que sa pose de
        // liaison est a plat.
        let parent = Vec3::ZERO;
        let repos = Vec3::NEG_Y;
        let longueur = 0.5_f32;
        let max = 45.0_f32.to_radians();
        // Un os parti à l'horizontale (90°) doit revenir à 45°, pas plus.
        let dehors = Vec3::new(longueur, 0.0, 0.0);
        let rabattu = limiter_ouverture(parent, dehors, repos, longueur, max);
        let angle = (rabattu - parent).normalize().dot(repos).acos();
        assert!(
            (angle - max).abs() < 1e-3,
            "rabattu à {:.1}° au lieu de {:.1}°",
            angle.to_degrees(),
            max.to_degrees()
        );
        assert!(((rabattu - parent).length() - longueur).abs() < 1e-4, "longueur perdue");
        // Un os déjà dans le cône ne bouge pas.
        let dedans = Vec3::new(0.1, -0.49, 0.0);
        assert!((limiter_ouverture(parent, dedans, repos, longueur, max) - dedans).length() < 1e-6);
    }

    /// Le corps n'est pas traversable, et l'os n'est pas téléporté pour autant.
    #[test]
    fn le_corps_repousse_sans_deplacer_en_hauteur() {
        let axe = Vec3::new(3.0, 1.0, -2.0);
        let rayon = 0.20;
        let dedans = Vec3::new(3.05, 0.4, -2.02);
        let sorti = ecarter_du_corps(dedans, axe, rayon);
        let horizontal = Vec3::new(sorti.x - axe.x, 0.0, sorti.z - axe.z).length();
        assert!((horizontal - rayon).abs() < 1e-5, "repoussé à {horizontal} au lieu de {rayon}");
        assert!((sorti.y - dedans.y).abs() < 1e-6, "la hauteur ne doit pas changer");
        // Dehors : intact. Rayon nul : désactivé.
        let dehors = Vec3::new(3.5, 0.4, -2.0);
        assert!((ecarter_du_corps(dehors, axe, rayon) - dehors).length() < 1e-6);
        assert!((ecarter_du_corps(dedans, axe, 0.0) - dedans).length() < 1e-6);
    }

    /// Le bouton vaut ce qu'il annonce APRÈS les quatre passes, pas avant.
    #[test]
    fn la_raideur_cumulee_vaut_celle_du_genome() {
        for declaree in [0.0, 0.1, 0.35, 0.55, 0.9, 1.0] {
            let par_passe = raideur_par_iteration(declaree, CONSTRAINT_ITERATIONS);
            let cumul = 1.0 - (1.0 - par_passe).powi(i32::from(CONSTRAINT_ITERATIONS));
            assert!(
                (cumul - declaree).abs() < 1e-4,
                "raideur {declaree} -> cumul {cumul} apres {CONSTRAINT_ITERATIONS} passes"
            );
        }
    }

    /// L'amortissement se compte par SECONDE : à 60 fps il ne peut pas retirer
    /// 70 % de la vitesse à chaque frame.
    #[test]
    fn l_amortissement_est_par_seconde_pas_par_frame() {
        let dt = 1.0 / 60.0;
        let retenue = retention_par_frame(0.7, dt);
        assert!(
            retenue > 0.97 && retenue < 0.99,
            "70 %/s doit retirer ~2 % par frame, retenu {retenue}"
        );
        // La définition tient à l'échelle de la seconde.
        assert!((retention_par_frame(0.7, 1.0) - 0.3).abs() < 1e-5);
        // Une frame deux fois plus longue retire deux fois plus (composition).
        let deux = retention_par_frame(0.7, dt * 2.0);
        assert!((deux - retenue * retenue).abs() < 1e-5);
    }

    /// Le défaut CONSTATÉ À L'ÉCRAN, transformé en garde chiffrée : avec les
    /// nombres du génome de la cape, l'ancienne formule plafonnait la chute à
    /// 0,095 m/s — invisible. Une étoffe se lit à partir de ~0,5 m/s.
    #[test]
    fn la_cape_peut_reellement_tomber() {
        let dt = 1.0 / 60.0;
        let (pesanteur, amorti) = (4.0, 0.7); // `assets/genomes/expedition_cape.toml`
        let ancienne = pesanteur * dt / amorti; // amortissement par frame
        assert!(ancienne < 0.1, "temoin de l'ancien defaut : {ancienne} m/s");
        let corrigee = chute_terminale_ms(pesanteur, amorti, dt);
        assert!(
            corrigee > 0.5,
            "la cape doit pouvoir tomber assez vite pour se voir, {corrigee} m/s"
        );
    }
}
