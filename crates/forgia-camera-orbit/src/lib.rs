//! # forgia-camera-orbit
//!
//! Orbit camera 3P (RPG mode) qui suit une entité cible.
//!
//! - Yaw : récupéré depuis la rotation Y du target (réutilise `Player.yaw` de forgia-player,
//!   pas de double input handling).
//! - Pitch : géré par mouse Y motion (interne à OrbitCamera).
//! - Distance : mouse wheel zoom in/out, clampée.
//! - Position camera = target_pos + arrière(target.yaw, pitch) * distance + height_offset.
//!
//! ## Usage
//!
//! ```ignore
//! commands.spawn((
//!     Camera3d::default(),
//!     OrbitCamera::new(player_entity),
//!     Transform::default(),
//! ));
//! ```

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_rapier3d::prelude::{QueryFilter, ReadRapierContext};
use forgia_core::prelude::*;

pub mod prelude {
    pub use crate::{ForgiaCameraOrbitPlugin, OrbitCamera};
}

/// Composant à attacher à un `Camera3d` pour en faire une orbit cam 3P.
#[derive(Component)]
pub struct OrbitCamera {
    pub target: Entity,
    /// Distance camera ↔ target (m). Mouse wheel scrollable.
    pub distance: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    /// Pitch local en radians, mouse-driven.
    pub pitch: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
    /// Hauteur d'épaule (m) — caméra vise target + height_offset, pas target.center.
    pub height_offset: f32,
    /// Sensibilité mouse Y → pitch.
    pub pitch_sensitivity: f32,
    /// Sensibilité mouse wheel → distance.
    pub zoom_sensitivity: f32,
    /// Yaw additionnel manuel (rad), contrôlé par LMB drag (mouse X). S'ajoute
    /// au yaw du target → permet à l'utilisateur de tourner autour du perso
    /// sans tourner le perso lui-même. Wraps automatiquement.
    pub yaw_offset: f32,
    /// Sensibilité mouse X → yaw_offset quand LMB held.
    pub yaw_sensitivity: f32,
    /// Décalage LATÉRAL de la caméra (m), positif = à droite du personnage.
    ///
    /// # C'est le seul terme qui sépare une orbite d'une vue Fortnite
    ///
    /// Une orbite classique cadre le personnage **au centre** : le regard est
    /// centré, et ce qu'il vise est masqué par son propre corps. Les jeux
    /// « par-dessus l'épaule » (Fortnite, Gears of War, Resident Evil 4) le
    /// décalent latéralement pour **libérer l'axe de visée** — c'est fonctionnel
    /// avant d'être esthétique.
    ///
    /// 0 = orbite cinématique (le Hall, le RPG, CyberCity gardent ce défaut et
    /// ne changent donc pas de comportement).
    pub shoulder_offset: f32,
    /// La souris est-elle **capturée en permanence** (visée type FPS/Fortnite) ?
    ///
    /// # Le défaut que ce drapeau corrige — mesuré en jeu le 2026-08-14
    ///
    /// Deux systèmes se contredisaient dans l'Expédition, et le commentaire de
    /// `mouse_look` avait décrit le piège sans prévoir ce cas :
    ///
    /// - `mouse_look` (forgia-player) ne prend la branche « 3ᵉ personne, tourne
    ///   seulement si RMB tenu » que pour `Rpg | CastleHub`. L'Expédition tombait
    ///   donc dans la branche FPS : **la souris tournait le personnage en
    ///   permanence** ;
    /// - `orbit_cursor_grab` (ici) relâchait le curseur dès qu'aucun bouton
    ///   n'était tenu.
    ///
    /// Résultat rapporté : « la souris ne suit pas le réticule ». Un curseur
    /// libre se promenait à l'écran pendant que le personnage pivotait, et le
    /// réticule au centre ne désignait plus rien.
    ///
    /// `false` = commandes WoW (curseur libre, clic maintenu pour regarder) —
    /// ce que le Hall, le RPG et CyberCity gardent.
    pub mouselook_permanent: bool,
}

impl OrbitCamera {
    /// Defaults RPG cinématique style Forgia V1 / Witcher / GTA :
    /// 7m back, 1.8m hauteur épaule, pitch piqué pour cadrer le character entier.
    pub fn new(target: Entity) -> Self {
        Self {
            target,
            distance: 7.0,
            min_distance: 3.0,
            max_distance: 18.0,
            pitch: -0.30, // ~17° vers le bas
            min_pitch: -1.2,
            max_pitch: 0.6,
            height_offset: 1.8,
            pitch_sensitivity: 0.002,
            zoom_sensitivity: 0.5,
            yaw_offset: 0.0,
            yaw_sensitivity: 0.005,
            // 0 = orbite centrée. Le Hall, le RPG et CyberCity gardent ce
            // comportement : ajouter le champ ne change rien pour eux.
            shoulder_offset: 0.0,
            mouselook_permanent: false,
        }
    }

    /// Préréglage **par-dessus l'épaule**, façon Fortnite.
    ///
    /// # Ce qui fait la différence, et pourquoi ces valeurs-là
    ///
    /// Une orbite cinématique cadre le personnage entier à 7 m : c'est beau et
    /// c'est injouable pour viser, parce que le corps du joueur masque
    /// exactement ce qu'il regarde. Trois termes règlent ça, et chacun se dérive
    /// d'une contrainte, pas d'un goût :
    ///
    /// | terme | valeur | d'où elle vient |
    /// |---|---|---|
    /// | `distance` | **3,2 m** | le personnage fait 2,0 m ; en deçà de ~3 m il remplit l'écran, au-delà de ~4 m on perd la lisibilité de ses gestes |
    /// | `shoulder_offset` | **0,65 m** | la capsule fait 0,30 m de rayon : il faut plus du double pour dégager l'axe de visée, sinon l'épaule reste dedans |
    /// | `height_offset` | **1,55 m** | hauteur d'épaule d'un personnage de 2,0 m dont l'origine est au centre — donc pieds à −1,0, épaule à ~+0,55 |
    ///
    /// Le zoom reste possible mais **borné serré** (2,2 – 4,5 m) : laisser
    /// reculer jusqu'à 18 m redonnerait la caméra cinématique et annulerait le
    /// préréglage sans que personne ne comprenne pourquoi la visée a changé.
    pub fn over_shoulder(target: Entity) -> Self {
        Self {
            distance: 3.2,
            min_distance: 2.2,
            max_distance: 4.5,
            // Presque à l'horizontale : on vise devant soi, pas ses pieds.
            pitch: -0.12,
            min_pitch: -0.9,
            max_pitch: 0.7,
            height_offset: 1.55,
            shoulder_offset: 0.65,
            // Le réticule est au centre de l'écran : pour qu'il désigne quelque
            // chose, la souris doit VISER, donc être capturée. Sans ça le
            // décalage d'épaule ne sert à rien — on dégage l'axe de visée d'un
            // joueur qui ne peut pas viser.
            mouselook_permanent: true,
            ..Self::new(target)
        }
    }
}

pub struct ForgiaCameraOrbitPlugin;

impl Plugin for ForgiaCameraOrbitPlugin {
    fn build(&self, app: &mut App) {
        // BUG-ANIMQA-06 fix : in_set(GameSet::Camera) pour ordering canonique V2
        app.add_systems(
            Update,
            (
                orbit_input,
                orbit_auto_recenter_on_move,
                orbit_follow,
                orbit_cursor_grab,
            )
                .chain()
                .in_set(GameSet::Camera)
                .run_if(any_with_component::<OrbitCamera>),
        );
    }
}

/// WoW pattern : cursor caché tant qu'un bouton souris (LMB ou RMB) est tenu,
/// restauré au release. Hold-to-grab, pas toggle (cf research §1.4).
///
/// `CursorOptions` est un Component séparé de Window depuis Bevy 0.16
/// (PR #19668) — query directe sur PrimaryWindow.
fn orbit_cursor_grab(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    q_cam: Query<&OrbitCamera>,
    mut q: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Ok(mut cursor) = q.single_mut() else {
        return;
    };
    let any_held =
        mouse_buttons.pressed(MouseButton::Left) || mouse_buttons.pressed(MouseButton::Right);
    // En visée permanente le curseur ne se relâche JAMAIS : c'est ce qui fait que
    // le réticule central désigne enfin quelque chose.
    let permanent = q_cam.iter().any(|c| c.mouselook_permanent);
    let desired_grab = if any_held || permanent {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    let desired_visible = !(any_held || permanent);
    if cursor.grab_mode != desired_grab {
        cursor.grab_mode = desired_grab;
    }
    if cursor.visible != desired_visible {
        cursor.visible = desired_visible;
    }
}

/// **WoW camera pattern** (sources Blizzard/wowpedia §research) :
///
/// - **LMB held + mouse X** : `yaw_offset` orbite la caméra autour du player.
///   Player yaw inchangé. "Look mode".
/// - **RMB held + mouse X** : `yaw_offset` reset à 0 (la caméra suit le player
///   directement). Mouse X consumé par `forgia-player` qui tourne le player
///   yaw. "Mouselook" — caméra et perso steer ensemble.
/// - **Pitch** : actif si LMB OU RMB tenu (gate hold-to-look WoW).
/// - **Mouse wheel** : zoom in/out toujours actif (même sans bouton tenu).
fn orbit_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut q: Query<&mut OrbitCamera>,
) {
    let lmb_held = mouse_buttons.pressed(MouseButton::Left);
    let rmb_held = mouse_buttons.pressed(MouseButton::Right);
    let any_held = lmb_held || rmb_held;

    let mut delta_x = 0.0;
    let mut delta_y = 0.0;
    for ev in motion.read() {
        delta_x += ev.delta.x;
        delta_y += ev.delta.y;
    }
    let mut delta_wheel = 0.0;
    for ev in wheel.read() {
        delta_wheel += ev.y;
    }
    if delta_x.abs() < f32::EPSILON
        && delta_y.abs() < f32::EPSILON
        && delta_wheel.abs() < f32::EPSILON
    {
        return;
    }
    for mut cam in &mut q {
        // Pitch : sur bouton tenu (WoW), ou en permanence en visée FPS —
        // sinon on ne pourrait pas viser haut ou bas.
        if (any_held || cam.mouselook_permanent) && delta_y.abs() > f32::EPSILON {
            cam.pitch =
                (cam.pitch - delta_y * cam.pitch_sensitivity).clamp(cam.min_pitch, cam.max_pitch);
        }
        // RMB tenu : la cam suit le yaw du player → reset yaw_offset à 0
        // smooth-lerp pour éviter snap brutal au passage LMB → RMB.
        // En visée permanente, c'est `mouse_look` qui tourne le PERSONNAGE et
        // la caméra le suit : un `yaw_offset` non nul ferait diverger les deux et
        // le réticule cesserait de désigner ce que vise le personnage.
        if cam.mouselook_permanent {
            cam.yaw_offset = 0.0;
        } else if rmb_held {
            cam.yaw_offset *= 0.85;
            if cam.yaw_offset.abs() < 0.001 {
                cam.yaw_offset = 0.0;
            }
        } else if lmb_held && delta_x.abs() > f32::EPSILON {
            // LMB only : orbit cam-only (player yaw inchangé).
            // ⚠️ Pas de rem_euclid (range [0, TAU]) — sinon le shortest-path
            // decay de l'auto-recenter ne peut pas raccourcir un offset > π en
            // partant via -π. Laisser flotter, cos/sin wrap naturellement.
            cam.yaw_offset -= delta_x * cam.yaw_sensitivity;
        }
        if delta_wheel.abs() > f32::EPSILON {
            cam.distance = (cam.distance - delta_wheel * cam.zoom_sensitivity)
                .clamp(cam.min_distance, cam.max_distance);
        }
    }
}

/// WoW pattern — auto-recenter caméra derrière le perso quand il se déplace
/// ET qu'aucun bouton souris n'est tenu. Decay smooth de `yaw_offset` vers 0
/// (~0.5s à 60fps). Si l'utilisateur tient LMB/RMB, son intent override (pas
/// d'auto-recenter). Pattern Blizzard wowpedia `cameraDistanceMoveSpeed`.
/// Durée totale d'une transition de recenter (ease-out quad).
/// 1.2s = WoW cam follow snappy. Ease-out = vitesse rapide visible au début
/// puis ralentissement doux à l'arrivée (ressort sous-amorti, pattern AAA).
/// Tuning user 2026-05-18 : 2.0s trop lent → 1.2s.
const RECENTER_DURATION_SEC: f32 = 1.2;

/// État interne du recenter (tracked entre frames). Reset au touch souris ou
/// quand la transition est terminée. `initial_yaw` est l'offset au moment où
/// le user a commencé à bouger (snapshot).
#[derive(Default)]
struct RecenterState {
    initial_yaw: f32,
    elapsed: f32,
    active: bool,
}

fn orbit_auto_recenter_on_move(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut q_cam: Query<&mut OrbitCamera>,
    targets: Query<&GlobalTransform, Without<OrbitCamera>>,
    mut last_pos: Local<Option<Vec3>>,
    mut state: Local<RecenterState>,
) {
    // Si user tient un bouton, il steer la cam — annule la transition courante
    // et save la position pour calculer le delta au release suivant.
    let any_held =
        mouse_buttons.pressed(MouseButton::Left) || mouse_buttons.pressed(MouseButton::Right);
    if any_held {
        state.active = false;
        if let Some(orbit) = q_cam.iter().next() {
            if let Ok(t) = targets.get(orbit.target) {
                *last_pos = Some(t.translation());
            }
        }
        return;
    }
    for mut orbit in &mut q_cam {
        if orbit.yaw_offset.abs() < 0.001 {
            orbit.yaw_offset = 0.0;
            state.active = false;
            continue;
        }
        let Ok(target_gt) = targets.get(orbit.target) else {
            continue;
        };
        let pos = target_gt.translation();
        let moved = match *last_pos {
            // Seuil 1e-4 m² (~10mm) : ignore les micro-jitters de la physique.
            Some(prev) => (pos - prev).length_squared() > 1.0e-4,
            None => false,
        };
        *last_pos = Some(pos);

        // Shortest-path normalize sur le yaw_offset COURANT pour qu'à chaque
        // (re)trigger on parte du chemin le plus court vers 0.
        use std::f32::consts::{PI, TAU};
        let mut a = orbit.yaw_offset.rem_euclid(TAU);
        if a > PI {
            a -= TAU;
        }

        // Trigger : la transition démarre au premier mouvement. Une fois
        // active, elle continue à recentrer MÊME SI le player s'arrête (pas
        // de pause-on-stop : sinon le cam reste figée à mi-chemin, frustrant).
        if !state.active {
            if !moved {
                continue;
            }
            state.initial_yaw = a;
            state.elapsed = 0.0;
            state.active = true;
        }
        state.elapsed += time.delta_secs();
        let t = (state.elapsed / RECENTER_DURATION_SEC).clamp(0.0, 1.0);
        // Ease-out quad : t * (2 - t) → visible début + doux fin (WoW-like).
        let smooth_t = t * (2.0 - t);
        orbit.yaw_offset = state.initial_yaw * (1.0 - smooth_t);
        if t >= 1.0 {
            orbit.yaw_offset = 0.0;
            state.active = false;
        }
    }
}

/// Place chaque OrbitCamera derrière son target, à `distance` mètres, vise target+height.
/// Le yaw vient du target lui-même (réutilise Player.yaw managé par forgia-player).
fn orbit_follow(
    targets: Query<&GlobalTransform, Without<OrbitCamera>>,
    mut cams: Query<(&OrbitCamera, &mut Transform), With<Camera3d>>,
    rapier: ReadRapierContext,
) {
    // Contexte physique pour l'anti-clip (None en test / si Rapier absent →
    // distance pleine, comportement historique).
    let ctx = rapier.single().ok();
    for (orbit, mut cam_tf) in &mut cams {
        let Ok(target_gt) = targets.get(orbit.target) else {
            continue;
        };
        let target_pos = target_gt.translation();
        // Extrait le yaw du target depuis sa rotation (en Y).
        let (yaw, _, _) = target_gt
            .compute_transform()
            .rotation
            .to_euler(EulerRot::YXZ);

        // Vecteur "derrière" : opposé au forward du target, modulé par pitch.
        // Forward V2 = -Z après rotation Y(yaw) → forward = (-sin(yaw), 0, -cos(yaw))
        // On veut camera derrière, donc on inverse + on ajoute la composante pitch (Y).
        // `yaw_offset` ajoute la rotation manuelle (LMB drag) au yaw du target.
        let cos_pitch = orbit.pitch.cos();
        let sin_pitch = orbit.pitch.sin();
        let total_yaw = yaw + orbit.yaw_offset;
        let back = Vec3::new(
            total_yaw.sin() * cos_pitch,
            -sin_pitch,
            total_yaw.cos() * cos_pitch,
        );

        // Le décalage d'épaule est LATÉRAL par rapport au regard, pas aligné sur
        // un axe du monde : sans ça, tourner sur soi-même ferait passer la
        // caméra de l'épaule droite à la gauche, puis devant.
        //
        // `back` est horizontalement dirigé selon `total_yaw` ; sa perpendiculaire
        // dans le plan XZ donne la droite de la caméra. On la calcule directement
        // depuis l'angle plutôt qu'avec un produit vectoriel, qui dégénérerait
        // quand le pitch approche la verticale.
        let droite = Vec3::new(total_yaw.cos(), 0.0, -total_yaw.sin());
        let look_target =
            target_pos + Vec3::Y * orbit.height_offset + droite * orbit.shoulder_offset;

        // Anti-clip (2026-06-16) : raycast depuis le point visé vers la position
        // caméra souhaitée. Si un collider est touché avant `distance`, on
        // rapproche la caméra pour qu'elle ne traverse plus sol/murs. Exclut le
        // rigidbody du target (le joueur) pour ne pas se heurter à sa capsule.
        let mut dist = orbit.distance;
        if let Some(ctx) = &ctx {
            let filter = QueryFilter::default().exclude_rigid_body(orbit.target);
            if let Some((_e, toi)) = ctx.cast_ray(look_target, back, orbit.distance, true, filter) {
                const CAM_SKIN_M: f32 = 0.3;
                dist = (toi - CAM_SKIN_M).max(0.2);
            }
        }
        cam_tf.translation = look_target + back * dist;
        cam_tf.look_at(look_target, Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Le préréglage par-dessus l'épaule ───────────────────────────────

    #[test]
    fn l_orbite_par_defaut_reste_centree() {
        // Le champ `shoulder_offset` est ajoute POUR l'Expedition. Le Hall, le
        // RPG et CyberCity ne doivent rien voir changer — un decalage non nul
        // par defaut deplacerait trois cameras existantes sans que personne ne
        // l'ait demande.
        let mut w = World::new();
        let t = w.spawn_empty().id();
        assert_eq!(OrbitCamera::new(t).shoulder_offset, 0.0);
    }

    #[test]
    fn la_visee_permanente_va_de_pair_avec_le_decalage_d_epaule() {
        // Les deux ne se separent pas. Rapporte en jeu le 2026-08-14 : « la
        // souris ne suit pas le reticule ». Le curseur restait libre pendant que
        // `mouse_look` tournait deja le personnage — donc un curseur qui se
        // promene, un perso qui pivote, et un reticule central qui ne designe
        // rien.
        //
        // Sans visee permanente, le decalage d'epaule ne sert a RIEN : on degage
        // l'axe de visee d'un joueur qui ne peut pas viser.
        let mut w = World::new();
        let t = w.spawn_empty().id();
        let epaule = OrbitCamera::over_shoulder(t);
        assert!(epaule.mouselook_permanent, "l'epaule sans la visee ne sert a rien");
        assert!(epaule.shoulder_offset > 0.0);

        // Et l'inverse : l'orbite cinematique garde les commandes WoW, sinon on
        // changerait celles du Hall, du RPG et de CyberCity sans l'avoir demande.
        let cine = OrbitCamera::new(t);
        assert!(!cine.mouselook_permanent);
        assert_eq!(cine.shoulder_offset, 0.0);
    }

    #[test]
    fn le_decalage_d_epaule_degage_vraiment_l_axe_de_visee() {
        // La capsule du joueur fait 0,30 m de rayon. Un decalage inferieur
        // laisserait la camera DANS l'epaule : le corps masquerait encore ce
        // qu'on vise, et le prereglage ne servirait a rien.
        const RAYON_CAPSULE_M: f32 = 0.30;
        let mut w = World::new();
        let t = w.spawn_empty().id();
        let c = OrbitCamera::over_shoulder(t);
        assert!(
            c.shoulder_offset > RAYON_CAPSULE_M * 2.0,
            "decalage {} m : l'epaule reste dans le champ",
            c.shoulder_offset
        );
    }

    #[test]
    fn la_camera_d_epaule_est_proche_et_le_reste() {
        // Laisser reculer jusqu'a 18 m redonnerait la camera cinematique et
        // annulerait le prereglage — la visee changerait sans que personne ne
        // comprenne pourquoi.
        let mut w = World::new();
        let t = w.spawn_empty().id();
        let c = OrbitCamera::over_shoulder(t);
        let cine = OrbitCamera::new(t);
        assert!(c.distance < cine.distance * 0.5, "pas assez proche");
        assert!(
            c.max_distance < cine.max_distance * 0.35,
            "le zoom arriere redonne la camera cinematique ({} m)",
            c.max_distance
        );
        assert!(c.min_distance < c.distance && c.distance < c.max_distance);
    }

    #[test]
    fn elle_regarde_devant_pas_les_pieds() {
        // Un pitch pique cadre le personnage entier — c'est cinematique et ca
        // empeche de viser. Presque a l'horizontale, on voit ou on va.
        let mut w = World::new();
        let t = w.spawn_empty().id();
        let c = OrbitCamera::over_shoulder(t);
        assert!(
            c.pitch.abs() < 0.25,
            "pitch {} rad : la camera regarde le sol",
            c.pitch
        );
    }

    #[test]
    fn la_hauteur_visee_tombe_bien_sur_l_epaule() {
        // Le personnage fait 2,0 m, origine au CENTRE (donc pieds a -1,0). Son
        // epaule est donc vers +0,55 depuis l'origine, soit 1,55 depuis les
        // pieds. Viser le centre cadrerait le ventre.
        let mut w = World::new();
        let t = w.spawn_empty().id();
        let c = OrbitCamera::over_shoulder(t);
        assert!(
            (1.3..=1.8).contains(&c.height_offset),
            "hauteur de visee {} m hors de la bande epaule",
            c.height_offset
        );
    }

    #[test]
    fn orbit_camera_defaults_reasonable() {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let cam = OrbitCamera::new(target);
        assert!(cam.distance >= cam.min_distance);
        assert!(cam.distance <= cam.max_distance);
        assert!(cam.pitch >= cam.min_pitch);
        assert!(cam.pitch <= cam.max_pitch);
    }

    #[test]
    fn plugin_builds_without_panic() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ForgiaCameraOrbitPlugin);
    }
}
