//! Visée de l'éditeur — **exactement deux tests par frame**, tous les outils
//! lisent la ressource (même contrat que le LOCK L4 « EditorRaycast 1/frame »).
//!
//! Deux visées distinctes parce qu'elles répondent à deux questions différentes :
//!
//! - **`surface_*`** : rayon physique Rapier → « quel point du monde je vise ? ».
//!   Sert au placement d'un nouvel asset et au magnétisme au sol.
//! - **`picked`** : test rayon/AABB sur les meshes visibles → « quelle pièce je
//!   vise ? ». Le décor du Hall n'a **aucun collider par pièce** (un seul TriMesh
//!   fusionné + boîtes de la Grande Salle, cf `castle_hub.rs`) : un rayon physique
//!   ne peut donc PAS désigner un tonneau ou une colonne. Le test AABB, lui, voit
//!   la géométrie réellement affichée.

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy_rapier3d::prelude::{QueryFilter, ReadRapierContext};
use forgia_player::prelude::{FpsCamera, Player};

/// Portée de visée de l'éditeur, en mètres. Outil de dev/création : ne touche
/// aucune valeur de gameplay, donc `const` nommée plutôt que gène TOML.
/// 400 m couvre l'emprise du château (~193 m) vue depuis un bord opposé.
const EDIT_RANGE_M: f32 = 400.0;

/// Résultat de visée partagé par tous les outils de l'éditeur.
#[derive(Resource, Default)]
pub struct EditorRay {
    pub origin: Vec3,
    pub dir: Vec3,
    /// Vrai si le rayon physique a touché quelque chose (sol, collision château).
    pub surface_hit: bool,
    pub surface_point: Vec3,
    pub surface_normal: Vec3,
    /// Distance caméra → impact physique (ou `EDIT_RANGE_M` si aucun).
    pub surface_distance: f32,
    /// Entité de mesh visée par le test AABB (feuille de la hiérarchie glTF).
    pub picked: Option<Entity>,
    /// Distance caméra → AABB visée.
    pub picked_distance: f32,
}

impl EditorRay {
    /// Point de placement d'un nouvel asset : impact sol si visé, sinon un point
    /// devant la caméra à distance de confort.
    pub fn placement_point(&self) -> Vec3 {
        if self.surface_hit {
            self.surface_point
        } else {
            self.origin + self.dir * PLACEMENT_FALLBACK_M
        }
    }
}

/// Distance de dépôt quand le joueur vise le vide (ciel) — assez proche pour
/// rester visible, assez loin pour ne pas apparaître dans la caméra.
const PLACEMENT_FALLBACK_M: f32 = 8.0;

/// Met à jour `EditorRay` : 1 rayon physique + 1 balayage AABB.
///
/// Chemin chaud borné : la query est filtrée `With<Mesh3d>`, ne tourne que
/// l'éditeur ouvert, n'alloue rien (meilleur candidat suivi dans deux scalaires)
/// et rejette d'abord les entités invisibles — dont les proxies de collision du
/// château, `Visibility::Hidden`, qui ne doivent jamais être sélectionnables.
pub fn sys_editor_ray(
    rapier: ReadRapierContext,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    q_player: Query<Entity, With<Player>>,
    q_meshes: Query<(Entity, &GlobalTransform, &Aabb, &InheritedVisibility), With<Mesh3d>>,
    mut ray: ResMut<EditorRay>,
) {
    let Ok(cam) = q_cam.single() else {
        *ray = EditorRay::default();
        return;
    };
    let origin = cam.translation();
    let dir = cam.forward().as_vec3();
    ray.origin = origin;
    ray.dir = dir;

    // ── 1. Rayon physique : le point du monde visé ────────────────────────
    ray.surface_hit = false;
    ray.surface_point = Vec3::ZERO;
    ray.surface_normal = Vec3::Y;
    ray.surface_distance = EDIT_RANGE_M;
    if let Ok(ctx) = rapier.single() {
        let mut filter = QueryFilter::default().exclude_sensors();
        if let Ok(player) = q_player.single() {
            filter = filter.exclude_collider(player);
        }
        if let Some((_, hit)) = ctx.cast_ray_and_get_normal(origin, dir, EDIT_RANGE_M, true, filter)
        {
            ray.surface_hit = true;
            ray.surface_point = origin + dir * hit.time_of_impact;
            ray.surface_normal = hit.normal;
            ray.surface_distance = hit.time_of_impact;
        }
    }

    // ── 2. Balayage AABB : la pièce visée ─────────────────────────────────
    let mut best: Option<(Entity, f32)> = None;
    for (entity, tf, aabb, visible) in &q_meshes {
        if !visible.get() {
            continue;
        }
        let inv = tf.affine().inverse();
        let local_origin = inv.transform_point3(origin);
        let local_dir = inv.transform_vector3(dir);
        let Some(t) = ray_aabb(
            local_origin,
            local_dir,
            Vec3::from(aabb.min()),
            Vec3::from(aabb.max()),
        ) else {
            continue;
        };
        if t > EDIT_RANGE_M {
            continue;
        }
        if best.is_none_or(|(_, best_t)| t < best_t) {
            best = Some((entity, t));
        }
    }
    match best {
        Some((entity, t)) => {
            ray.picked = Some(entity);
            ray.picked_distance = t;
        }
        None => {
            ray.picked = None;
            ray.picked_distance = EDIT_RANGE_M;
        }
    }
}

/// Intersection rayon / boîte alignée (méthode des tranches). `origin` et `dir`
/// sont dans le repère local de la boîte ; comme les deux subissent la même
/// transformation affine, le `t` retourné reste valable dans le repère monde.
///
/// Retourne le `t` d'entrée (0 si l'origine est dans la boîte), `None` si raté.
fn ray_aabb(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut t_enter = 0.0f32;
    let mut t_exit = f32::INFINITY;
    for axis in 0..3 {
        let d = dir[axis];
        if d.abs() < 1e-8 {
            // Rayon parallèle à cette paire de faces : hors tranche = pas d'impact.
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d;
        let mut t1 = (min[axis] - origin[axis]) * inv;
        let mut t2 = (max[axis] - origin[axis]) * inv;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_enter = t_enter.max(t1);
        t_exit = t_exit.min(t2);
        if t_enter > t_exit {
            return None;
        }
    }
    Some(t_enter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_box_in_front() {
        let t = ray_aabb(
            Vec3::new(0.0, 0.0, -5.0),
            Vec3::Z,
            Vec3::splat(-1.0),
            Vec3::splat(1.0),
        );
        assert_eq!(t, Some(4.0));
    }

    #[test]
    fn misses_box_beside() {
        assert!(ray_aabb(
            Vec3::new(10.0, 0.0, -5.0),
            Vec3::Z,
            Vec3::splat(-1.0),
            Vec3::splat(1.0)
        )
        .is_none());
    }

    #[test]
    fn origin_inside_box_returns_zero() {
        assert_eq!(
            ray_aabb(Vec3::ZERO, Vec3::Z, Vec3::splat(-1.0), Vec3::splat(1.0)),
            Some(0.0)
        );
    }

    #[test]
    fn parallel_ray_outside_slab_misses() {
        // Rayon strictement horizontal, boîte au-dessus : la tranche Y rejette.
        assert!(ray_aabb(
            Vec3::new(0.0, 10.0, -5.0),
            Vec3::Z,
            Vec3::splat(-1.0),
            Vec3::splat(1.0)
        )
        .is_none());
    }

    #[test]
    fn box_behind_ray_is_rejected_by_range_check() {
        // La boîte est derrière : t d'entrée négatif clampé à 0 par t_enter,
        // mais t_exit devient négatif → t_enter > t_exit → pas d'impact.
        assert!(ray_aabb(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::Z,
            Vec3::splat(-1.0),
            Vec3::splat(1.0)
        )
        .is_none());
    }
}
