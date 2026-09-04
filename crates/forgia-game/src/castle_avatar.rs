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
use forgia_mode_roguelite::avatar::{AvatarLocomotion, equipped_key, spawn_equipped_avatar};
use forgia_mode_roguelite::equipment::{CorpsAvatar, EquipmentConfig, EquipmentSave};
use forgia_player::ViewmodelCamera;
use forgia_player::prelude::{AimCamera, FpsCamera, Player};

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

/// Les fiches d'animation, UNE PAR CORPS.
///
/// 🚨 Il y en a toujours au moins deux en vol : l'aperçu du menu (trooper) et le
/// corps du mode joué. Un capteur qui n'en publiait qu'une affichait la fiche du
/// trooper pendant qu'on enquêtait sur le personnage de l'Expédition — et on la
/// croyait, parce qu'elle avait l'air d'une réponse. Les publier toutes coûte
/// trois lignes et supprime l'ambiguïté.
fn fiches_json(diag: &forgia_mode_roguelite::avatar::AvatarClipDiag) -> String {
    diag.par_corps
        .iter()
        .map(|f| {
            format!(
                // `etat_demande` ET `clip_joue` : c'est leur ÉCART qui informe.
                // Égaux = le corps a le clip demandé. Différents = un repli a
                // servi, donc ce corps ne l'a pas. Une seule des deux valeurs
                // aurait laissé « ne s'accroupit pas » avec quatre causes
                // indiscernables.
                r#"{{"modele":"{}","lies":{},"passes":{},"relances":{},"etat_demande":"{}","clip_joue":"{}","attendus":[{}],"presents":[{}]}}"#,
                f.corps,
                f.lie,
                f.passes,
                f.relances,
                f.etat_demande,
                f.clip_joue,
                liste_json(&f.attendus),
                liste_json(&f.presents)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Une liste de noms en tableau JSON. Les noms de clips viennent d'un glTF,
/// donc d'un fichier qu'on ne contrôle pas : les guillemets et antislashes
/// s'échappent, sinon un clip mal nommé casse le capteur ENTIER — et un capteur
/// illisible se lit comme une absence de problème.
fn liste_json(v: &[String]) -> String {
    v.iter()
        .map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(",")
}

/// Le corps que ce mode porte.
///
/// L'Expédition a son propre personnage, avec son squelette et ses clips — il ne
/// porte donc pas l'armure, et c'est déclaré dans le génome, pas ici.
///
/// Hors Expédition, le corps N'EST PLUS FIXÉ ICI : il se choisit dans le génome
/// (`corps_troisieme_personne`). Le Hall a été conçu pour montrer l'équipement
/// looté — riggé sur le seul squelette du Trooper — donc y mettre le corps de
/// l'Expédition fait disparaître les cinq pièces. C'est un arbitrage d'auteur,
/// et il appartient à la donnée.
fn corps_du_mode(mode: &GameMode, cfg: &EquipmentConfig) -> CorpsAvatar {
    match mode {
        GameMode::Expedition => cfg.corps_expedition(),
        // Le choix vit dans le génome (`corps_troisieme_personne`) : changer de
        // personnage au Hall ne doit pas demander une recompilation.
        _ => cfg.corps_hors_expedition(),
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
    // On LIT la visée sur la caméra choisie. La tracer depuis la constante du
    // Hall affichait « 0,49 m » en Expédition alors que la caméra y est réglée à
    // 1,55 : un log qui décrit un autre objet que celui qui tourne coûte plus
    // cher qu'un log absent, parce qu'on le croit.
    let visee = orbit.height_offset;
    commands.spawn((
        HallOrbitCamera,
        // 🚨 Sans ce marqueur, le tir continuerait de suivre la `FpsCamera` —
        // celle qu'on vient de DÉSACTIVER deux lignes plus haut. Le réticule au
        // centre de l'écran désignerait alors autre chose que ce que la balle
        // touche, et l'écart grandirait avec le tangage (les deux caméras n'ont
        // ni la même sensibilité ni les mêmes bornes). On tire depuis la caméra
        // qui rend : c'est celle-ci tant que la 3ᵉ personne est montée.
        AimCamera,
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
         visee a {visee:.2} m, epaule {epaule:.2} m",
        mode.get()
    );
}

/// (Re)construit l'avatar quand l'équipement porté change.
fn sys_sync_hall_avatar(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut body_handles: ResMut<forgia_mode_roguelite::avatar::AvatarBodyHandles>,
    cfg: Res<EquipmentConfig>,
    mode: Res<State<GameMode>>,
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
    let corps = corps_du_mode(mode.get(), &cfg);
    // Le corps entre dans la clé : sans lui, passer du Hall à l'Expédition ne
    // ferait rien bouger — l'équipement porté n'a pas changé — et le personnage
    // du Hall partirait en expédition.
    let key = format!("{}|{}", corps.model, equipped_key(&save));
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
        &corps,
        &save,
        root,
        Transform::default(),
    );
    shown.0 = Some(key);
    info!(
        "[castle-avatar] avatar monté — corps {}, {} pièce(s) portée(s)",
        corps.model,
        if corps.porte_armure {
            save.equipped.len()
        } else {
            0
        }
    );
}

/// Os témoin pour mesurer si les pièces suivent le corps. La main est le point
/// le plus éloigné de la racine : c'est là que la moindre désynchronisation se
/// voit le plus.
const DESYNC_PROBE_BONE: &str = "hand_l";

/// Publie la vitesse horizontale RÉELLE du joueur ; les seuils qui la traduisent
/// en clip vivent dans le genome (`[animation]`), comme pour les ennemis.
///
/// On dérive la vitesse du DÉPLACEMENT RÉEL plutôt que de l'intention d'entrée :
/// c'est ce que le personnage fait à l'écran qui doit commander l'animation —
/// sinon il marche en poussant contre un mur.
///
/// # 🚨 Mais on ne la mesure PAS ici, et c'est tout le sujet
///
/// Cette fonction calculait elle-même `(position − position d'avant) / dt`, en
/// `Update`. Le joueur avance en `FixedUpdate` à 64 Hz : sur une frame de rendu
/// sans pas fixe le delta vaut **0**, et sur une frame avec pas fixe il vaut un
/// pas entier divisé par un `dt` de rendu bien plus court. La « vitesse » n'était
/// donc pas 6,5 m/s mais un **repliement** oscillant entre 0 et ~15 — à 117 fps,
/// mesuré : pointes à **14,72 m/s** pour une marche à 6,5.
///
/// Et le garde-fou anti-téléport mettait ces pointes à **ZÉRO** au lieu de les
/// borner. Résultat en jeu, le 2026-08-16 : *« je ne vois que l'animation
/// idle »* — le personnage marchait, et son animation le croyait immobile.
///
/// `forgia_player::PlayerLocomotion` publie déjà cette vitesse, mesurée en
/// `FixedUpdate` **après le déplacement**. Sa documentation nomme exactement ce
/// piège : « évite le signal 0/élevé qu'aurait une mesure en Update ». La
/// recalculer ici était la même grandeur écrite deux fois — et c'est la mauvaise
/// des deux qui pilotait l'animation.
fn sys_drive_avatar_walk(
    cfg: Res<EquipmentConfig>,
    joueur: Res<forgia_player::PlayerLocomotion>,
    posture: Res<forgia_player::Posture>,
    souris: Res<ButtonInput<MouseButton>>,
    mut loco: ResMut<AvatarLocomotion>,
) {
    let mesuree = joueur.horizontal_speed;
    // Le plafond garde son rôle — un replacement au spawn produit un pic de
    // plusieurs centaines de m/s — mais il BORNE au lieu d'annuler. Annuler
    // faisait passer le personnage pour immobile à chaque pic ; borner le laisse
    // au pire courir une frame, ce qui ne se voit pas.
    let plafond = cfg.animation.max_sane_speed;
    loco.speed = mesuree.min(plafond);

    // Le reste de l'état vient du contrôleur, qui le MESURE là où le delta et le
    // transform sont sous la main. Ce système ne fait que transporter : refaire
    // la projection ici en aurait fait une grandeur écrite deux fois.
    loco.avant = joueur.avant.clamp(-plafond, plafond);
    loco.lateral = joueur.lateral.clamp(-plafond, plafond);
    loco.vitesse_verticale = joueur.vertical;
    loco.au_sol = joueur.au_sol;
    loco.lacet_rad_s = joueur.lacet_rad_s;
    loco.depuis_atterrissage_s = joueur.depuis_atterrissage_s;
    loco.duree_vol_precedent_s = joueur.duree_vol_precedent_s;

    // La posture est produite par le mode (l'Expédition), et vaut « debout »
    // partout ailleurs : le Hall ne change pas de comportement.
    loco.accroupi = posture.accroupi;
    loco.glisse = posture.glisse;

    // 🚨 Viser et tirer se lisent sur l'INTENTION, pas sur l'état de l'arme.
    // Un tir refusé — chargeur vide, cadence non écoulée — doit quand même
    // épauler le personnage : sinon appuyer sur la gâchette à vide le laisse au
    // repos, et le joueur croit que son entrée n'est pas passée.
    loco.vise = souris.pressed(MouseButton::Right);
    loco.tire = souris.pressed(MouseButton::Left);
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
    diag: Res<forgia_mode_roguelite::avatar::AvatarClipDiag>,
    mut vitesse_max: Local<f32>,
) {
    // 🚨 AVANT le pas de 1 Hz. La vitesse instantanée échantillonnée une fois
    // par seconde ne prouve rien : elle vaut 0 dès qu'on lâche la touche, et
    // « le personnage ne marche jamais » devient indistinguable de « je ne
    // bougeais pas au moment de la mesure ». Rapporté en jeu le 2026-08-15 :
    // « j'ai pas d'autre animation que idle ».
    //
    // Le maximum vu, lui, tranche : les seuils de clip valent 0,6 m/s (marche)
    // et 6,0 m/s (course). Un maximum resté sous 0,6 accuse le producteur de
    // vitesse ; un maximum au-dessus alors que rien ne bouge à l'écran accuse
    // le choix de clip. Deux causes, une mesure.
    *vitesse_max = vitesse_max.max(loco.speed);
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
            "avatar non monte — hors mode 3e personne (Hall/Expedition) ou joueur pas encore spawne",
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
    } else if anim_players > 0 && anim_playing == 0 && !diag.par_corps.iter().any(|f| f.lie) {
        // Le cas TOTAL se distingue du partiel : aucune pièce ne joue, donc ce
        // n'est pas une armure qui décroche — c'est le câblage lui-même qui n'a
        // pas eu lieu. Et le champ `clips_*` du capteur dit pourquoi, sans avoir
        // à attendre la fermeture du jeu pour lire le log.
        (
            "critical",
            "AVATAR_ANIM_ABSENT : le corps est en pose de repos, bras ecartes — comparer clips_attendus et clips_presents ci-dessous : si un nom manque, corriger le genome ; si presents est vide, le glTF du corps n'expose aucune animation",
        )
    } else if anim_players > 0 && anim_playing == 0 {
        // 🚨 Câblé ET muet. Ce cas tombait jusqu'ici dans « ANIM_PARTIAL », qui
        // accuse « des pièces » — alors qu'avec UN lecteur et zéro qui joue, ce
        // n'est pas une pièce qui décroche : c'est le corps entier qui s'est tu
        // APRÈS avoir été câblé. Relevé le 2026-08-16 (anim_players 1,
        // anim_playing 0) pendant que le log montrait le même corps recâblé
        // trois fois et ses pièces rebranchées quinze fois.
        //
        // Ce n'est pas cosmétique : un os que plus rien ne réécrit laisse toute
        // couche additive (la tenue de visée) s'accumuler sur elle-même.
        (
            "critical",
            "AVATAR_ANIM_MUET : le corps a bien ete cable (clips_lies=true) puis son lecteur s'est TU — plus aucun clip actif. Croiser avec le log : un corps recable plusieurs fois, ou des pieces rebranchees en boucle, signent une reconstruction repetee de l'avatar. Un os que rien ne reecrit fige le personnage ET fait deriver toute pose additive",
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
        r#"{{"id":"castle_avatar","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"mounted":{},"mesh_count":{},"visible_meshes":{},"camera_distance_m":{:.2},"ground_gap_m":{:.3},"speed_mps":{:.2},"vitesse_max_vue_mps":{:.2},"anim_players":{},"anim_playing":{},"bone_copies":{},"desync_m":{:.3},"avatar_pos":[{:.1},{:.1},{:.1}],"camera_pos":[{:.1},{:.1},{:.1}],"equipped_total":{},"corps":[{}]}}"#,
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
        *vitesse_max,
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
        fiches_json(&diag),
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
