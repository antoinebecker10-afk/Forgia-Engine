//! castle_avatar.rs — Le Hall se joue à la 3ᵉ personne, avec l'armure qu'on porte.
//!
//! Le Hall est le lieu où l'on se regarde : c'est là qu'on voit ce qu'on a
//! débloqué. Le reste du jeu reste en vue subjective.
//!
//! Le montage reprend trait pour trait celui du mode RPG
//! (`forgia_rpg::character::spawn_rex_character`), qui est la référence 3P du
//! projet :
//!
//! 1. la `FpsCamera` est **désactivée**, pas détruite — on la réactive en sortant ;
//! 2. l'`OrbitCamera` est une entité **séparée**, jamais un enfant du joueur :
//!    enfant, elle hériterait de son lacet et la vue cesserait d'être de la 3ᵉ
//!    personne ;
//! 3. le personnage est un **enfant** du joueur, donc il suit position et lacet
//!    sans code de synchronisation.
//!
//! L'avatar lui-même (corps + pièces teintées) est monté par
//! `forgia_mode_roguelite::avatar`, partagé avec l'aperçu du menu.

use bevy::prelude::*;
use bevy_rapier3d::prelude::{
    CharacterLength, KinematicCharacterController, QueryFilter, ReadRapierContext,
};
use forgia_camera_orbit::OrbitCamera;
use forgia_core::prelude::*;
use forgia_mode_roguelite::avatar::{equipped_key, spawn_equipped_avatar, AvatarLocomotion};
use forgia_mode_roguelite::equipment::{EquipmentConfig, EquipmentSave};
use forgia_player::prelude::{FpsCamera, Player};
use forgia_player::ViewmodelCamera;

/// Pieds du personnage sous l'origine du joueur.
///
/// Dérivé du collider, pas choisi : `Collider::capsule_y(0.7, 0.3)` fait 2,0 m
/// centrés sur l'origine, donc le sol est à −1,0. Le modèle a son origine aux
/// pieds (AABB mesurée 0 → 1,82 m).
const AVATAR_FEET_OFFSET_M: f32 = -(0.7 + 0.3);

/// Le personnage regarde +Z (vérifié au rendu de contrôle) ; l'avant de Bevy est
/// −Z. Sans ce demi-tour on verrait l'avatar de face en avançant.
const AVATAR_FACING_YAW: f32 = std::f32::consts::PI;

/// Hauteur du modèle, mesurée (AABB du glTF, pieds à 0).
const AVATAR_HEIGHT_M: f32 = 1.82;

/// Point visé par la caméra, au-dessus de l'origine du JOUEUR.
///
/// 🚨 `OrbitCamera::new` pose `height_offset = 1.8`, documenté « hauteur
/// d'épaule » — ce qui suppose une cible dont l'origine est aux PIEDS. Notre
/// `Player` a la sienne au centre de sa capsule, 1 m plus haut : 1,8 viserait
/// 2,8 m au-dessus du sol, soit un mètre AU-DESSUS de la tête. Le rayon
/// anti-clip part alors du plafond, touche immédiatement, et le bras à ressort
/// se rétracte à 20 cm — caméra dans le crâne, personnage invisible.
///
/// On vise l'épaule : 82 % de la hauteur du modèle, ramenés dans le repère du
/// joueur.
const CAMERA_SHOULDER_OFFSET_M: f32 = AVATAR_HEIGHT_M * 0.82 + AVATAR_FEET_OFFSET_M;

/// Caméra orbitale du Hall — despawn en sortie.
#[derive(Component)]
struct HallOrbitCamera;

/// Conteneur du personnage, enfant du joueur. Reconstruit quand l'équipement
/// change ; c'est lui qui porte l'offset et le demi-tour.
#[derive(Component)]
struct HallAvatarRoot;

/// Équipement actuellement monté sur l'avatar du Hall.
#[derive(Resource, Default)]
struct HallAvatarShown(Option<String>);

/// Bascule le Hall en 3ᵉ personne. Idempotent et réessayé : le joueur est spawné
/// par un autre plugin (`OnEnter(AppMode::InGame)`), donc il peut ne pas encore
/// exister à l'entrée du mode.
/// Les modes qui se jouent **à la 3ᵉ personne**.
///
/// Centralisé ici plutôt que répété sur chaque système : c'est exactement la
/// mécanique par laquelle un mode finit à moitié câblé — la leçon que
/// `fps_combat_mode` porte déjà côté FPS, et qui a coûté une session le
/// 2026-08-14 quand l'Expédition s'est retrouvée sans contrôleur.
fn mode_troisieme_personne(mode: Res<State<GameMode>>) -> bool {
    matches!(mode.get(), GameMode::CastleHub | GameMode::Expedition)
}

/// La caméra que ce mode mérite.
///
/// Le Hall est un **lieu** : on s'y promène, on regarde le château, la caméra
/// cinématique à 7 m cadre le personnage entier et c'est ce qu'on veut.
///
/// L'Expédition est un **parcours avec du combat** : on y vise. Une caméra
/// centrée place le corps du joueur exactement devant ce qu'il regarde, donc
/// elle est injouable — d'où le préréglage par-dessus l'épaule.
fn camera_du_mode(mode: &GameMode, joueur: Entity) -> OrbitCamera {
    match mode {
        GameMode::Expedition => OrbitCamera::over_shoulder(joueur),
        _ => {
            let mut c = OrbitCamera::new(joueur);
            c.height_offset = CAMERA_SHOULDER_OFFSET_M;
            c
        }
    }
}

fn sys_setup_third_person(
    mut commands: Commands,
    mode: Res<State<GameMode>>,
    q_player: Query<Entity, With<Player>>,
    q_existing: Query<(), With<HallOrbitCamera>>,
    mut q_fps: Query<&mut Camera, (With<FpsCamera>, Without<ViewmodelCamera>)>,
    mut q_viewmodel: Query<&mut Camera, With<ViewmodelCamera>>,
) {
    if !q_existing.is_empty() {
        return;
    }
    let Ok(player) = q_player.single() else {
        return;
    };

    for mut cam in &mut q_fps {
        cam.is_active = false;
    }
    // La caméra du viewmodel est une entité SÉPARÉE : couper la FpsCamera ne
    // l'éteint pas, et les bras resteraient collés à l'écran par-dessus la vue
    // 3P. C'est le piège que le mode RPG n'a pas eu à traiter (pas de viewmodel).
    for mut cam in &mut q_viewmodel {
        cam.is_active = false;
    }

    let orbit = camera_du_mode(mode.get(), player);
    let epaule = orbit.shoulder_offset;
    commands.spawn((
        HallOrbitCamera,
        Camera3d::default(),
        Camera {
            is_active: true,
            ..default()
        },
        Transform::default(),
        orbit,
        Name::new("HallOrbitCamera"),
    ));
    info!(
        "[avatar-3p] {:?} en 3e personne — FpsCamera et viewmodel coupes, \
         visee a {:.2} m, epaule {epaule:.2} m",
        mode.get(),
        CAMERA_SHOULDER_OFFSET_M
    );
}

/// (Re)construit l'avatar quand l'équipement porté change.
fn sys_sync_hall_avatar(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut body_handles: ResMut<forgia_mode_roguelite::avatar::AvatarBodyHandles>,
    cfg: Res<EquipmentConfig>,
    save: Res<EquipmentSave>,
    q_player: Query<(Entity, &KinematicCharacterController), With<Player>>,
    q_root: Query<Entity, With<HallAvatarRoot>>,
    mut shown: ResMut<HallAvatarShown>,
) {
    let Ok((player, kcc)) = q_player.single() else {
        return;
    };
    // 🚨 Le contrôleur cinématique ne laisse JAMAIS la capsule toucher le sol :
    // il garde `offset` de marge. Sans compensation l'avatar flotte d'autant
    // (4,5 cm mesurés). On LIT l'offset sur le contrôleur au lieu de recopier
    // sa constante — deux déclarations de la même grandeur finissent toujours
    // par diverger.
    let skin = match kcc.offset {
        CharacterLength::Absolute(v) => v,
        // Une marge relative dépend du rayon de la capsule : hors de portée ici,
        // on ne compense pas plutôt que de compenser faux.
        CharacterLength::Relative(_) => 0.0,
    };
    let key = equipped_key(&save);
    if shown.0.as_deref() == Some(key.as_str()) && !q_root.is_empty() {
        return;
    }
    for e in &q_root {
        commands.entity(e).despawn();
    }
    let root = commands
        .spawn((
            HallAvatarRoot,
            Transform::from_xyz(0.0, AVATAR_FEET_OFFSET_M - skin, 0.0)
                .with_rotation(Quat::from_rotation_y(AVATAR_FACING_YAW)),
            Visibility::Inherited,
            Name::new("HallAvatar"),
            ChildOf(player),
        ))
        .id();
    spawn_equipped_avatar(
        &mut commands,
        &assets,
        &mut body_handles,
        &cfg,
        &save,
        root,
        Transform::default(),
    );
    shown.0 = Some(key);
    info!(
        "[castle-avatar] avatar monté — {} pièce(s) portée(s)",
        save.equipped.len()
    );
}

/// Os témoin pour mesurer si les pièces suivent le corps. La main est le point
/// le plus éloigné de la racine : c'est là que la moindre désynchronisation se
/// voit le plus.
const DESYNC_PROBE_BONE: &str = "hand_l";

/// Publie la vitesse horizontale RÉELLE du joueur ; les seuils qui la traduisent
/// en clip vivent dans le genome (`[animation]`), comme pour les ennemis.
///
/// On dérive la vitesse de la position plutôt que de lire un état du contrôleur :
/// c'est ce que le personnage fait à l'écran qui doit commander l'animation, pas
/// ce que l'entrée demande — sinon il marche en poussant contre un mur.
fn sys_drive_avatar_walk(
    time: Res<Time>,
    cfg: Res<EquipmentConfig>,
    q_player: Query<&GlobalTransform, With<Player>>,
    mut loco: ResMut<AvatarLocomotion>,
    mut last: Local<Option<Vec3>>,
) {
    let Ok(gt) = q_player.single() else {
        return;
    };
    let pos = gt.translation();
    let dt = time.delta_secs().max(1e-4);
    let raw = last.map_or(0.0, |p| (pos - p).with_y(0.0).length() / dt);
    *last = Some(pos);
    // 🚨 Plafond anti-téléport : un replacement au spawn produit un pic de
    // plusieurs centaines de m/s, et le personnage se mettrait à courir à
    // l'arrêt. Même garde-fou que les ennemis (`MAX_SANE_SPEED_MPS`).
    loco.speed = if raw > cfg.animation.max_sane_speed {
        0.0
    } else {
        raw
    };
}

/// Sortie du Hall : on rend la vue subjective telle qu'on l'a trouvée.
fn sys_teardown_third_person(
    mut commands: Commands,
    q_orbit: Query<Entity, With<HallOrbitCamera>>,
    q_root: Query<Entity, With<HallAvatarRoot>>,
    mut q_fps: Query<&mut Camera, (With<FpsCamera>, Without<ViewmodelCamera>)>,
    mut q_viewmodel: Query<&mut Camera, With<ViewmodelCamera>>,
    mut shown: ResMut<HallAvatarShown>,
) {
    for e in &q_orbit {
        commands.entity(e).despawn();
    }
    for e in &q_root {
        commands.entity(e).despawn();
    }
    for mut cam in &mut q_fps {
        cam.is_active = true;
    }
    for mut cam in &mut q_viewmodel {
        cam.is_active = true;
    }
    shown.0 = None;
    info!("[castle-avatar] retour vue subjective");
}

/// `forgia2_castle_avatar.json` 1 Hz.
///
/// Sans ce capteur, « je ne vois pas le personnage » a trois causes possibles
/// qu'aucune capture d'écran ne distingue : le modèle n'a pas chargé, la caméra
/// est collée dedans, ou l'avatar est ailleurs. On mesure donc les trois.
///
/// - `AVATAR_NO_MESH` : l'avatar est monté mais n'a produit aucun maillage —
///   le glTF n'a pas chargé (racine d'assets, chemin, export).
/// - `CAMERA_COLLAPSED` : le bras à ressort est rétracté sous 1 m, la caméra est
///   dans le personnage. C'est la géométrie autour du joueur qui l'écrase.
fn sys_write_castle_avatar_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    q_root: Query<(Entity, &GlobalTransform), With<HallAvatarRoot>>,
    q_cam: Query<&GlobalTransform, With<HallOrbitCamera>>,
    q_children: Query<&Children>,
    q_mesh: Query<&ViewVisibility, With<Mesh3d>>,
    q_anim: Query<&AnimationPlayer>,
    q_named: Query<(&Name, &GlobalTransform)>,
    save: Res<EquipmentSave>,
    rapier: ReadRapierContext,
    q_player: Query<Entity, With<Player>>,
    loco: Res<AvatarLocomotion>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let root = q_root.iter().next();
    // `ViewVisibility` est ce que Bevy a RÉELLEMENT rendu cette frame. Compter
    // les `Mesh3d` ne dit que « l'entité existe » — c'est la distinction qui
    // sépare « le modèle n'a pas chargé » de « le modèle est culled ou masqué ».
    let (mesh_count, visible_meshes) = root.map_or((0, 0), |(e, _)| {
        let mut total = 0;
        let mut seen = 0;
        for d in q_children.iter_descendants(e) {
            if let Ok(vv) = q_mesh.get(d) {
                total += 1;
                if vv.get() {
                    seen += 1;
                }
            }
        }
        (total, seen)
    });
    // Lecteurs d'animation câblés et effectivement en train de jouer. Six
    // attendus (corps + 5 pièces) : un lecteur muet, c'est une pièce qui reste
    // en pose de repos pendant que le reste bouge — le personnage se disloque.
    let (anim_players, anim_playing) = root.map_or((0, 0), |(e, _)| {
        let mut total = 0;
        let mut active = 0;
        for d in q_children.iter_descendants(e) {
            if let Ok(p) = q_anim.get(d) {
                total += 1;
                if p.playing_animations().count() > 0 {
                    active += 1;
                }
            }
        }
        (total, active)
    });

    // Écart entre les SIX copies d'un même os. Chaque pièce embarque l'armature
    // complète : si toutes jouent le clip, leurs `hand_l` coïncident. Un écart
    // signifie qu'une pièce est restée en pose de repos pendant que le corps
    // bouge — c'est la dislocation visible à l'écran, et aucune autre mesure ne
    // la distingue d'un lecteur qui « joue » sans piloter ses os.
    let mut hands: Vec<Vec3> = Vec::new();
    if let Some((e, _)) = root {
        for d in q_children.iter_descendants(e) {
            if let Ok((name, gt)) = q_named.get(d) {
                if name.as_str() == DESYNC_PROBE_BONE {
                    hands.push(gt.translation());
                }
            }
        }
    }
    let bone_copies = hands.len();
    let mut desync: f32 = 0.0;
    for i in 0..hands.len() {
        for j in (i + 1)..hands.len() {
            desync = desync.max(hands[i].distance(hands[j]));
        }
    }

    let avatar_pos = root.map_or(Vec3::ZERO, |(_, gt)| gt.translation());
    let camera_pos = q_cam
        .iter()
        .next()
        .map_or(Vec3::ZERO, |gt| gt.translation());
    let cam_dist = match (root, q_cam.iter().next()) {
        (Some(_), Some(_)) => camera_pos.distance(avatar_pos),
        _ => 0.0,
    };
    let mounted = root.is_some();

    // Écart pieds ↔ sol, MESURÉ. Le contrôleur cinématique garde la capsule à
    // `offset` du sol : l'avatar flotte donc de cet écart si on ne le compense
    // pas. On le mesure plutôt que de recopier la constante du contrôleur.
    let ground_gap = match (root, q_player.single().ok(), rapier.single().ok()) {
        (Some(_), Some(player), Some(ctx)) => {
            let from = avatar_pos + Vec3::Y * 0.5;
            let filter = QueryFilter::default().exclude_rigid_body(player);
            ctx.cast_ray(from, Vec3::NEG_Y, 3.0, true, filter)
                .map(|(_, toi)| toi - 0.5)
                .unwrap_or(f32::NAN)
        }
        _ => f32::NAN,
    };

    let (severity, next_step) = if !mounted {
        (
            "info",
            "avatar non monte — hors Hall ou joueur pas encore spawne",
        )
    } else if mesh_count == 0 {
        (
            "warn",
            "AVATAR_NO_MESH : aucun maillage sous l'avatar — glTF non charge (verifier la racine d'assets : lancer par cargo run)",
        )
    } else if visible_meshes == 0 {
        (
            "warn",
            "AVATAR_CULLED : les maillages existent mais aucun n'est rendu — visibilite heritee, RenderLayers, ou AABB de skinning",
        )
    } else if cam_dist < 1.0 {
        (
            "warn",
            "CAMERA_COLLAPSED : bras a ressort retracte sous 1 m — la camera est dans le personnage, verifier height_offset et la geometrie autour du joueur",
        )
    } else if bone_copies > 1 && desync > 0.05 {
        (
            "warn",
            "AVATAR_DESYNC : les pieces ne suivent pas le corps — une armature reste en pose de repos, l'armure se detache des bras",
        )
    } else if anim_players > 0 && anim_playing < anim_players {
        (
            "warn",
            "AVATAR_ANIM_PARTIAL : des pieces ne jouent pas leur clip — elles resteront en pose de repos pendant que le corps bouge",
        )
    } else if ground_gap.is_finite() && ground_gap.abs() > 0.08 {
        (
            "warn",
            "AVATAR_OFF_GROUND : les pieds ne touchent pas le sol — ajuster AVATAR_FEET_OFFSET_M de l'ecart mesure",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"castle_avatar","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"mounted":{},"mesh_count":{},"visible_meshes":{},"camera_distance_m":{:.2},"ground_gap_m":{:.3},"speed_mps":{:.2},"anim_players":{},"anim_playing":{},"bone_copies":{},"desync_m":{:.3},"avatar_pos":[{:.1},{:.1},{:.1}],"camera_pos":[{:.1},{:.1},{:.1}],"equipped_total":{}}}"#,
        time.elapsed_secs(),
        mounted,
        mesh_count,
        visible_meshes,
        cam_dist,
        if ground_gap.is_finite() {
            ground_gap
        } else {
            -99.0
        },
        loco.speed,
        anim_players,
        anim_playing,
        bone_copies,
        desync,
        avatar_pos.x,
        avatar_pos.y,
        avatar_pos.z,
        camera_pos.x,
        camera_pos.y,
        camera_pos.z,
        save.equipped.len(),
    );
    let _ = forgia_core::sensor_io::enqueue("forgia2_castle_avatar.json", json);
}

pub struct CastleAvatarPlugin;

impl Plugin for CastleAvatarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HallAvatarShown>()
            .add_systems(
                Update,
                (
                    sys_setup_third_person,
                    sys_sync_hall_avatar,
                    sys_drive_avatar_walk,
                )
                    .chain()
                    .run_if(mode_troisieme_personne)
                    .run_if(in_state(AppMode::InGame)),
            )
            .add_systems(OnExit(GameMode::CastleHub), sys_teardown_third_person)
            .add_systems(OnExit(GameMode::Expedition), sys_teardown_third_person)
            .add_systems(
                Update,
                sys_write_castle_avatar_sensor.in_set(GameSet::Sensors),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les pieds doivent tomber exactement au bas de la capsule : un avatar qui
    /// flotte ou s'enfonce se voit immédiatement en 3ᵉ personne.
    #[test]
    fn feet_offset_matches_the_player_capsule() {
        // `Collider::capsule_y(half_height, radius)` → hauteur totale
        // 2*(0.7+0.3) = 2,0 m, centrée sur l'origine.
        assert!((AVATAR_FEET_OFFSET_M - (-1.0)).abs() < 1e-6);
    }

    /// Le demi-tour doit amener le +Z du modèle sur le −Z de Bevy.
    #[test]
    fn facing_turns_the_model_to_bevy_forward() {
        let rotated = Quat::from_rotation_y(AVATAR_FACING_YAW) * Vec3::Z;
        assert!((rotated - Vec3::NEG_Z).length() < 1e-5);
    }

    /// La caméra doit viser l'épaule, donc rester DANS la hauteur du personnage.
    /// Viser au-dessus de sa tête fait partir le rayon anti-clip dans le
    /// plafond, et le bras à ressort s'écrase — c'est le défaut observé.
    #[test]
    fn camera_aims_inside_the_character_height() {
        let aim_above_feet = CAMERA_SHOULDER_OFFSET_M - AVATAR_FEET_OFFSET_M;
        assert!(
            aim_above_feet > 1.0 && aim_above_feet < AVATAR_HEIGHT_M,
            "visée à {aim_above_feet:.2} m du sol, hors de la silhouette (0..{AVATAR_HEIGHT_M})"
        );
    }
}
