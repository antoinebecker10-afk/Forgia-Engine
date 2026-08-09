//! avatar.rs — Le personnage du joueur, habillé de ce qu'il porte.
//!
//! Un seul endroit sait construire l'avatar : corps de base + pièces équipées,
//! chacune teintée à sa rareté. Deux consommateurs s'en servent, et c'est
//! précisément pour ça qu'il est ici plutôt que chez l'un d'eux :
//!
//! - l'**aperçu du menu** (`forgia_ui::weapon_preview`), rendu hors écran ;
//! - l'**avatar du Hall** (`forgia_game::castle_hub`), vu à la 3ᵉ personne.
//!
//! Dupliquer ce montage aurait garanti qu'ils divergent : une teinte corrigée
//! d'un côté, une pièce ajoutée de l'autre.
//!
//! Ce module ne décide NI où l'avatar apparaît, NI comment il est rendu — il
//! attache des enfants à une entité donnée. Le layer de rendu, la caméra et le
//! cycle de vie restent la responsabilité de l'appelant.

use bevy::gltf::Gltf;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use std::time::Duration;

use crate::equipment::{EquipmentConfig, EquipmentSave};

/// Teinte de rareté en attente d'application sur les matériaux d'une pièce.
///
/// Un `SceneRoot` n'a pas d'enfants à la frame de son spawn : le marqueur reste
/// posé jusqu'à ce qu'un maillage ait réellement été teinté.
#[derive(Component)]
pub struct AvatarPieceTint(pub Color);

/// Marque les entités montées par [`spawn_equipped_avatar`], pour les retirer
/// d'un bloc quand l'équipement change ou que la scène se ferme.
#[derive(Component)]
pub struct AvatarPart;

/// Teinte normalisée : garde la couleur, jamais la luminosité.
///
/// `base_color` **multiplie** la texture d'albédo. Appliquer la couleur brute
/// d'une rareté sombre (le gris Commun est à 0,62) assombrirait l'armure de 40 %
/// au lieu de la colorer.
pub fn rarity_tint(rgb: [f32; 3]) -> Color {
    let [r, g, b] = rgb;
    let max = r.max(g).max(b).max(1e-3);
    Color::srgb(r / max, g / max, b / max)
}

/// Décrit l'équipement porté sous forme de clé — deux états égaux ⇒ rien à
/// reconstruire. Sans elle, l'avatar se rebâtirait à chaque frame.
pub fn equipped_key(save: &EquipmentSave) -> String {
    let mut parts: Vec<String> = save
        .equipped
        .iter()
        .map(|(slot, rarity)| format!("{slot}={rarity}"))
        .collect();
    parts.sort();
    parts.join(",")
}

/// Handles PERMANENTS vers le glTF du corps (fichier entier + `Scene(0)`).
///
/// 🚨 Le remède de « l'armure ne tourne plus » (cause prouvée par sonde le
/// 2026-08-06, `reference_skinned_mesh_follows_bones_not_hierarchy`) : le corps
/// est le seul à porter DEUX handles vers le même fichier. Au respawn de
/// l'avatar (changement de pièce), l'ancien `AvatarClips` — dernier handle
/// fort vers le `Gltf` — tombait, l'asset était déchargé, et le rechargement
/// qui suivait ré-insérait la scène : Bevy RÉ-INSTANCIAIT le corps sous la
/// même entité, ses 68 os étaient remplacés APRÈS que les pièces (des meshes
/// skinnés, qui suivent leurs OS et pas leur parent) se soient liées — elles
/// restaient figées sur les os morts pendant que le corps tournait.
///
/// Garder les handles vivants ici rend chaque `load()` idempotent : plus de
/// déchargement, plus de rechargement, plus de ré-instanciation — les os
/// survivent au respawn.
#[derive(Resource, Default)]
pub struct AvatarBodyHandles {
    /// (chemin, glTF entier, Scene(0)) — invalidé si le chemin du génome change.
    cached: Option<(String, Handle<Gltf>, Handle<Scene>)>,
}

impl AvatarBodyHandles {
    /// Les handles du corps pour `path`, chargés UNE fois puis réutilisés.
    fn body(&mut self, assets: &AssetServer, path: &str) -> (Handle<Gltf>, Handle<Scene>) {
        match &self.cached {
            Some((p, g, s)) if p == path => (g.clone(), s.clone()),
            _ => {
                let gltf: Handle<Gltf> = assets.load(path.to_string());
                let scene: Handle<Scene> =
                    assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()));
                self.cached = Some((path.to_string(), gltf.clone(), scene.clone()));
                (gltf, scene)
            }
        }
    }
}

/// Attache le corps et les pièces portées à `parent`, et rend les entités
/// créées pour que l'appelant y ajoute ce qui le regarde (layer de rendu,
/// marqueurs de cycle de vie…).
///
/// `local` place l'avatar dans le repère du parent : les appelants n'ont pas le
/// même besoin (un aperçu centre le sujet, le Hall descend le perso sous la
/// caméra du joueur et le retourne).
pub fn spawn_equipped_avatar(
    commands: &mut Commands,
    assets: &AssetServer,
    handles: &mut AvatarBodyHandles,
    cfg: &EquipmentConfig,
    save: &EquipmentSave,
    parent: Entity,
    local: Transform,
) -> Vec<Entity> {
    let mut spawned = Vec::new();
    if !cfg.body_model.is_empty() {
        // Handles issus du cache permanent — cf. [`AvatarBodyHandles`].
        let (gltf, scene) = handles.body(assets, &cfg.body_model);
        spawned.push(
            commands
                .spawn((
                    AvatarPart,
                    SceneRoot(scene),
                    AvatarBody,
                    // Le glTF complet, pour lire ses clips PAR NOM une fois chargé.
                    AvatarClips::new(gltf),
                    local,
                    Visibility::Inherited,
                    Name::new("avatar_body"),
                    ChildOf(parent),
                ))
                .id(),
        );
    }
    for (slot_id, rarity_id) in &save.equipped {
        let (Some(slot), Some(rarity)) = (cfg.slot(slot_id), cfg.rarity(rarity_id)) else {
            continue;
        };
        spawned.push(
            commands
                .spawn((
                    AvatarPart,
                    SceneRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(slot.model.clone()))),
                    // Pas de clips : la pièce suivra le squelette du corps.
                    AvatarNeedsSkeletonShare::default(),
                    local,
                    Visibility::Inherited,
                    AvatarPieceTint(rarity_tint(rarity.rgb)),
                    Name::new(format!("avatar_{slot_id}")),
                    ChildOf(parent),
                ))
                .id(),
        );
    }
    spawned
}

/// Applique la teinte de rareté dès que la scène d'une pièce est peuplée.
///
/// Le matériau du glTF est **partagé** par les cinq pièces (elles sortent du
/// même jeu de textures) : le muter en place les teindrait toutes pareil. On en
/// clone donc un par pièce.
pub fn sys_tint_avatar_pieces(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_pending: Query<(Entity, &AvatarPieceTint)>,
    q_children: Query<&Children>,
    q_mat: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    for (entity, tint) in &q_pending {
        let mut touched = false;
        for descendant in q_children.iter_descendants(entity) {
            let Ok(handle) = q_mat.get(descendant) else {
                continue;
            };
            let Some(base) = materials.get(&handle.0) else {
                continue;
            };
            let mut cloned = base.clone();
            cloned.base_color = tint.0;
            commands
                .entity(descendant)
                .insert(MeshMaterial3d(materials.add(cloned)));
            touched = true;
        }
        // Retirer le marqueur avant qu'un maillage existe laisserait la pièce
        // non teintée pour de bon.
        if touched {
            commands.entity(entity).remove::<AvatarPieceTint>();
        }
    }
}

// ── Squelette partagé ────────────────────────────────────────────────────────
//
// 🚨 Chaque fichier glTF embarque l'armature complète — contrainte du format : un
// maillage skinné ne peut pas référencer un squelette externe. Si on laisse
// chaque pièce animer le SIEN, les six squelettes dérivent et l'armure se
// détache du corps (observé 2026-08-01 : brassards flottant à l'écart des bras).
//
// La correction n'est pas de synchroniser six squelettes, c'est de n'en garder
// qu'UN. Les pièces gardent leur maillage et **rebranchent leurs joints sur les
// os du corps** ; leurs propres os sont supprimés. Elles ne peuvent alors plus
// diverger, puisqu'elles n'ont plus rien à elles qui puisse diverger.
//
// Le partage n'est exact que parce que les six fichiers sortent de la MÊME
// armature au même repos : matrices de liaison inverses identiques (SHA1
// vérifié) et même ordre de joints. Le rebranchement se fait quand même **par
// nom** — le jour où un ré-export changerait l'ordre, il échouera bruyamment au
// lieu de tordre le personnage en silence.
//
// Seul le corps embarque les clips.

/// Pièce dont les joints n'ont pas encore été rebranchés sur le corps.
///
/// Porte un compteur de tentatives parce que le rebranchement échoue
/// **silencieusement** dans trois cas légitimes-en-attente (corps pas encore
/// là, os du corps pas encore lisibles, scène de la pièce pas peuplée). Tant
/// qu'ils sont transitoires, tout va bien ; s'ils durent, la pièce reste plantée
/// à l'écran et RIEN ne le dit — le silence se lit comme un succès.
///
/// Observé le 2026-08-06 : l'avatar se désynchronise au changement de pièce, et
/// aucun log ne permettait de distinguer « pas encore » de « jamais ».
#[derive(Component, Default)]
pub struct AvatarNeedsSkeletonShare {
    /// Passes écoulées sans succès.
    tries: u32,
}

/// Au-delà, l'attente n'est plus transitoire : on crie une fois.
///
/// ~2 s à 60 Hz. Le chargement d'une scène glTF déjà en cache prend quelques
/// frames ; au-delà de deux secondes, c'est un blocage, pas une latence.
const SKELETON_SHARE_PATIENCE: u32 = 120;

/// Le corps : il porte l'armature et l'animation de tout l'avatar.
#[derive(Component)]
pub struct AvatarBody;

/// Rebranche les maillages des pièces sur le squelette du corps.
///
/// Réessaie tant que les scènes ne sont pas peuplées : elles n'apparaissent pas
/// à la frame de leur spawn, et rien ne garantit que le corps arrive en premier.
fn sys_share_body_skeleton(
    mut commands: Commands,
    q_body: Query<(Entity, &ChildOf), With<AvatarBody>>,
    mut q_pieces: Query<(Entity, &ChildOf, &mut AvatarNeedsSkeletonShare)>,
    q_children: Query<&Children>,
    q_name: Query<&Name>,
    mut q_skin: Query<&mut SkinnedMesh>,
) {
    if q_pieces.is_empty() {
        return;
    }
    // 🚨 Chaque pièce se branche sur le corps de SON avatar, apparié par parent
    // commun. Deux avatars peuvent vivre en même temps — l'aperçu du menu et
    // celui du Hall — et prendre « le premier corps venu » branchait les pièces
    // de l'un sur le squelette de l'autre : l'armure restait plantée pendant que
    // le personnage tournait. Un simple garde ne suffisait pas non plus, il
    // affamait le second avatar, dont les pièces n'auraient jamais trouvé leur
    // tour. C'est l'appariement qui doit être juste, pas le filtrage.
    let bodies: HashMap<Entity, Entity> = q_body.iter().map(|(e, p)| (p.parent(), e)).collect();
    if bodies.is_empty() {
        return;
    }
    // SONDE (2026-08-06) — deux corps sous le MÊME parent s'écrasent dans la
    // carte, et le survivant est arbitraire : les pièces se brancheraient sur
    // celui qui perd, donc sur un squelette condamné. Rien ne le dirait, puisque
    // le rebranchement « réussit ». Le compte est le seul témoin.
    let seen = q_body.iter().count();
    if seen != bodies.len() {
        warn!(
            "[avatar] SONDE : {seen} corps pour {} parent(s) distinct(s) — deux corps \
             partagent un parent, l'appariement en perd un",
            bodies.len()
        );
    }
    // Os indexés par nom, pris depuis le maillage skinné du corps : c'est la
    // seule liste dont on soit sûr qu'elle contienne les joints, et eux seuls.
    // Le cache évite de la reconstruire pour chaque pièce du même avatar.
    let mut bones_of: HashMap<Entity, HashMap<String, Entity>> = HashMap::default();

    for (piece, piece_parent, mut pending) in &mut q_pieces {
        // Une seule fois, au franchissement : chaque sortie ci-dessous dit
        // LAQUELLE des trois attentes ne se résout pas. Sans ça, les trois se
        // ressemblent — un silence — et on ne peut pas les distinguer.
        pending.tries = pending.tries.saturating_add(1);
        let complain = pending.tries == SKELETON_SHARE_PATIENCE;
        let name = q_name.get(piece).map(|n| n.to_string()).unwrap_or_default();

        let Some(&body) = bodies.get(&piece_parent.parent()) else {
            if complain {
                warn!(
                    "[avatar] {name} : aucun corps sous le même parent depuis \
                     {SKELETON_SHARE_PATIENCE} passes — la pièce restera plantée"
                );
            }
            continue; // son corps n'est pas encore là — on repassera
        };
        if !bones_of.contains_key(&body) {
            let mut map: HashMap<String, Entity> = HashMap::default();
            for d in q_children.iter_descendants(body) {
                let Ok(skin) = q_skin.get(d) else {
                    continue;
                };
                for joint in &skin.joints {
                    if let Ok(name) = q_name.get(*joint) {
                        map.insert(name.to_string(), *joint);
                    }
                }
                break;
            }
            bones_of.insert(body, map);
        }
        let body_bones = &bones_of[&body];
        if body_bones.is_empty() {
            if complain {
                warn!(
                    "[avatar] {name} : le corps {body:?} n'expose toujours aucun \
                     os après {SKELETON_SHARE_PATIENCE} passes — sa scène n'est \
                     pas peuplée, la pièce restera plantée"
                );
            }
            continue; // scène du corps pas encore peuplée
        }

        let mut found = None;
        for d in q_children.iter_descendants(piece) {
            if let Ok(skin) = q_skin.get(d) {
                found = Some((d, skin.joints.clone()));
                break;
            }
        }
        let Some((mesh_entity, own_joints)) = found else {
            if complain {
                warn!(
                    "[avatar] {name} : pas de maillage skinné après \
                     {SKELETON_SHARE_PATIENCE} passes — sa propre scène n'est \
                     pas peuplée, la pièce restera plantée"
                );
            }
            continue; // scène pas encore peuplée — on repassera
        };

        // Traduction par NOM. Un seul os introuvable et on renonce : mieux vaut
        // une pièce qui ne suit pas qu'un personnage tordu par un mauvais joint.
        let shared: Vec<Entity> = own_joints
            .iter()
            .map_while(|j| {
                q_name
                    .get(*j)
                    .ok()
                    .and_then(|n| body_bones.get(n.as_str()))
                    .copied()
            })
            .collect();
        if shared.len() != own_joints.len() {
            warn!(
                "[avatar] pièce non rebranchée : {}/{} os retrouvés sur le corps — squelettes divergents ?",
                shared.len(),
                own_joints.len()
            );
            commands.entity(piece).remove::<AvatarNeedsSkeletonShare>();
            continue;
        }
        // SONDE — l'identité du 1er os retenu. C'est elle qu'on comparera plus
        // tard à celle du corps VIVANT : si elles divergent, les os du corps ont
        // été remplacés sous lui et la pièce tient un jeu périmé.
        let bone0 = shared.first().copied();
        if let Ok(mut skin) = q_skin.get_mut(mesh_entity) {
            skin.joints = shared;
        }
        // Les os propres de la pièce ne servent plus. Le maillage n'est PAS sous
        // eux (il est frère de la racine d'os dans le glTF, vérifié), donc les
        // supprimer ne l'emporte pas — et ça évite de propager 68 transforms
        // inertes par pièce à chaque frame.
        //
        // 🚨 SAUF si ces os sont ceux du CORPS. Ce serait le cas d'une pièce
        // traitée deux fois : au second passage, `own_joints` a déjà été
        // remplacé par le squelette partagé, et cette ligne décapiterait
        // l'avatar ENTIER — toutes les pièces perdraient leurs os d'un coup,
        // resteraient figées dans le monde pendant que le corps tourne. C'est
        // le symptôme mesuré le 2026-08-06 (« 68/68 os MORTS » sur les cinq
        // pièces à la fois). Détruire les os du corps n'est jamais voulu :
        // le refus est inconditionnel, et il se signale.
        if let Some(root_bone) = own_joints.first() {
            if body_bones.values().any(|b| b == root_bone) {
                warn!(
                    "[avatar] {name} : refus de détruire {root_bone:?} — c'est un os \
                     du CORPS, pas de la pièce (pièce traitée deux fois)"
                );
            } else {
                commands.entity(*root_bone).despawn();
            }
        }
        commands.entity(piece).remove::<AvatarNeedsSkeletonShare>();
        // `info!` et non `debug!` : c'est l'événement qui dit si l'avatar est
        // cohérent. En `debug!` il était invisible dans le log par défaut, et
        // l'absence de rebranchement se lisait comme un silence — donc comme
        // « tout va bien », alors que c'est précisément le symptôme.
        info!(
            "[avatar] {name} rebranchée ({} os) sur le corps {body:?} — SONDE os[0]={bone0:?}",
            own_joints.len()
        );
    }
}

/// Le glTF du corps, gardé pour lire ses `named_animations`.
///
/// 🚨 Les clips se résolvent par **nom**, jamais par index. `Animation(0)` se
/// décale en silence au moindre ré-export ; un nom manquant se signale. C'est le
/// choix déjà fait pour les ennemis (`enemy_anim.rs`).
#[derive(Component)]
pub struct AvatarClips {
    gltf: Handle<Gltf>,
    /// Passes écoulées sans avoir trouvé de lecteur d'animation. Même raison que
    /// [`AvatarNeedsSkeletonShare`] : un corps qui n'est jamais câblé reste sur
    /// la pose de repos du glTF — bras écartés — et rien ne le signale.
    tries: u32,
}

impl AvatarClips {
    pub fn new(gltf: Handle<Gltf>) -> Self {
        Self { gltf, tries: 0 }
    }
}

/// Nœuds du graphe, une fois le lecteur du corps câblé.
#[derive(Component)]
struct AvatarAnimNodes {
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
    /// Nœud effectivement joué — évite de relancer le même clip chaque frame,
    /// ce qui le remettrait à zéro et figerait le personnage sur sa 1re pose.
    current: AnimationNodeIndex,
}

/// Ce que l'avatar doit jouer. Le Hall y écrit la vitesse mesurée ; l'aperçu du
/// menu la laisse à zéro et obtient l'idle.
#[derive(Resource, Default)]
pub struct AvatarLocomotion {
    pub speed: f32,
}

/// Câble le graphe d'animation dès que la scène d'une pièce expose son lecteur.
///
/// Un `SceneRoot` n'a pas d'`AnimationPlayer` à la frame de son spawn : le
/// composant `AvatarClips` reste posé jusqu'à ce que le lecteur apparaisse.
fn sys_bind_avatar_animations(
    mut commands: Commands,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    gltfs: Res<Assets<Gltf>>,
    cfg: Res<EquipmentConfig>,
    mut q_parts: Query<(Entity, &mut AvatarClips)>,
    q_children: Query<&Children>,
    mut q_player: Query<&mut AnimationPlayer>,
) {
    for (part, mut clips) in &mut q_parts {
        clips.tries = clips.tries.saturating_add(1);
        let complain = clips.tries == SKELETON_SHARE_PATIENCE;
        // Le glTF doit être chargé pour qu'on puisse lire ses noms de clips.
        let Some(gltf) = gltfs.get(&clips.gltf) else {
            if complain {
                warn!(
                    "[avatar] le glTF du corps {part:?} n'est toujours pas chargé \
                     après {SKELETON_SHARE_PATIENCE} passes — corps figé sur sa \
                     pose de repos"
                );
            }
            continue;
        };
        let anim = &cfg.animation;
        let pick = |name: &String| gltf.named_animations.get(name.as_str()).cloned();
        let (Some(idle), Some(walk), Some(run)) = (
            pick(&anim.idle_clip),
            pick(&anim.walk_clip),
            pick(&anim.run_clip),
        ) else {
            warn!(
                "[avatar] clips absents du corps — attendus {:?}/{:?}/{:?}, présents {:?}",
                anim.idle_clip,
                anim.walk_clip,
                anim.run_clip,
                gltf.named_animations.keys().collect::<Vec<_>>()
            );
            commands.entity(part).remove::<AvatarClips>();
            continue;
        };

        let mut bound = false;
        for descendant in q_children.iter_descendants(part) {
            let Ok(mut player) = q_player.get_mut(descendant) else {
                continue;
            };
            let mut graph = AnimationGraph::new();
            let root = graph.root;
            let nodes = AvatarAnimNodes {
                idle: graph.add_clip(idle.clone(), 1.0, root),
                walk: graph.add_clip(walk.clone(), 1.0, root),
                run: graph.add_clip(run.clone(), 1.0, root),
                current: AnimationNodeIndex::default(),
            };
            // `AnimationTransitions` est ce qui permet le FONDU. Sans lui, tout
            // changement de clip est une coupure sèche — c'est ce qui trahit le
            // plus une animation de jeu.
            let mut transitions = AnimationTransitions::new();
            transitions
                .play(&mut player, nodes.idle, Duration::ZERO)
                .repeat();
            commands.entity(descendant).insert((
                AnimationGraphHandle(graphs.add(graph)),
                transitions,
                AvatarAnimNodes {
                    current: nodes.idle,
                    ..nodes
                },
            ));
            // Un corps non re-câblé après reconstruction reste sur la pose de
            // repos du glTF (bras écartés) — c'est ce qu'on voit à l'écran quand
            // l'avatar « se désynchronise ». Tracer le câblage permet de dire
            // s'il a eu lieu, au lieu de le supposer.
            info!("[avatar] animations câblées sur le corps {part:?}");
            bound = true;
        }
        if bound {
            commands.entity(part).remove::<AvatarClips>();
        } else if complain {
            warn!(
                "[avatar] aucun AnimationPlayer sous le corps {part:?} après \
                 {SKELETON_SHARE_PATIENCE} passes — le corps reste sur sa pose \
                 de repos (bras écartés) et les pièces suivront un squelette figé"
            );
        }
    }
}

/// Choisit le clip depuis la vitesse et enchaîne en fondu.
fn sys_drive_avatar_locomotion(
    cfg: Res<EquipmentConfig>,
    loco: Res<AvatarLocomotion>,
    mut q: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut AvatarAnimNodes,
    )>,
) {
    let anim = &cfg.animation;
    let cross = Duration::from_millis(anim.crossfade_ms);
    for (mut player, mut transitions, mut nodes) in &mut q {
        let next = if loco.speed >= anim.run_speed_min {
            nodes.run
        } else if loco.speed >= anim.walk_speed_min {
            nodes.walk
        } else {
            nodes.idle
        };
        if nodes.current == next {
            continue;
        }
        transitions.play(&mut player, next, cross).repeat();
        nodes.current = next;
    }
}

/// Crie quand une pièce d'armure suit des os **morts**.
///
/// Un maillage skinné n'est pas placé par sa hiérarchie mais déformé par ses
/// joints. Une pièce dont les joints ont été despawnés perd donc tout repère et
/// se fige dans le monde : elle cesse de suivre le corps alors que celui-ci
/// continue de tourner.
///
/// C'est mot pour mot le symptôme rapporté en jeu le 2026-08-06, et seulement
/// **après un remplacement** de pièce : « quand je regarde juste le personnage
/// il tourne parfaitement avec son armure ; lorsque je remplace une pièce,
/// l'armure ne tourne plus alors que le personnage tourne toujours ».
///
/// Ce système ne corrige rien — il transforme un symptôme visuel en fait
/// mesuré. Deviner cette cause a déjà coûté trois hypothèses fausses ; le
/// squelette d'une pièce est invisible à l'écran, il faut donc l'interroger.
/// Le premier maillage skinné sous `root` — la seule liste de joints qui compte.
///
/// Extrait en fonction pour être interrogé DEUX fois : sur la pièce qui crie, et
/// sur le corps vivant du même avatar. C'est leur comparaison qui distingue « les
/// os sont morts » de « les os ne sont plus ceux du corps ».
fn first_skin<'a>(
    root: Entity,
    q_children: &Query<'_, '_, &Children>,
    q_skin: &'a Query<'_, '_, &SkinnedMesh>,
) -> Option<&'a SkinnedMesh> {
    q_children
        .iter_descendants(root)
        .find_map(|d| q_skin.get(d).ok())
}

fn sys_warn_orphan_avatar_joints(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cried: Local<HashSet<Entity>>,
    q_parts: Query<(Entity, &Name, &ChildOf), With<AvatarPart>>,
    q_body: Query<(Entity, &ChildOf), With<AvatarBody>>,
    q_children: Query<&Children>,
    q_skin: Query<&SkinnedMesh>,
    q_exists: Query<Entity>,
) {
    // 1 Hz : un squelette orphelin le reste, inutile de le constater 60×/s.
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    for (part, name, parent) in &q_parts {
        let Some(skin) = first_skin(part, &q_children, &q_skin) else {
            continue;
        };
        // `q_exists.get` échoue uniquement si l'entité n'existe plus : c'est
        // le test de vie le plus direct qu'offre l'ECS.
        let dead = skin
            .joints
            .iter()
            .filter(|j| q_exists.get(**j).is_err())
            .count();
        // Une seule plainte par pièce : le défaut persiste, pas le besoin de
        // le dire. `cried` ne grandit pas — au plus une entrée par pièce.
        if dead == 0 || !cried.insert(part) {
            continue;
        }
        // SONDE — l'identité des os des DEUX côtés, prise au même instant.
        //
        // Le corps ne crie jamais alors que les pièces tiennent SES os : c'est
        // impossible si ce sont les mêmes entités. Soit elles ne l'ont jamais
        // été (mauvais corps apparié), soit elles ont cessé de l'être (scène du
        // corps ré-instanciée sous lui, l'entité `SceneRoot` gardant son ID —
        // d'où l'illusion de continuité dans les logs).
        //
        // `os[0] pièce` vs `os[0] corps` tranche : égaux ⇒ un tiers détruit les
        // os ; différents ⇒ le corps a changé de squelette sans le dire.
        let live = q_body
            .iter()
            .find(|(_, p)| p.parent() == parent.parent())
            .and_then(|(b, _)| first_skin(b, &q_children, &q_skin));
        warn!(
            "[avatar] {name} suit {dead}/{} os MORTS — elle est figée dans \
             le monde et ne suivra plus le corps (armure qui ne tourne pas) ; \
             SONDE os[0] pièce={:?} vs corps vivant={:?} ({} os vivants)",
            skin.joints.len(),
            skin.joints.first(),
            live.and_then(|s| s.joints.first()),
            live.map(|s| s
                .joints
                .iter()
                .filter(|j| q_exists.get(**j).is_ok())
                .count())
                .unwrap_or(0),
        );
    }
}

/// Enregistre teinte et animation, une fois pour tous les consommateurs.
pub struct AvatarPlugin;

impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvatarLocomotion>()
            .init_resource::<AvatarBodyHandles>()
            .add_systems(
            Update,
            (
                sys_tint_avatar_pieces,
                // Le partage précède le câblage : une pièce encore sur son propre
                // squelette n'a rien à faire jouer.
                sys_share_body_skeleton,
                sys_bind_avatar_animations,
                sys_drive_avatar_locomotion,
                // En queue : il constate l'état, il ne le produit pas.
                sys_warn_orphan_avatar_joints,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La teinte doit garder la couleur et rendre sa luminosité — sinon une
    /// rareté sombre assombrit l'armure au lieu de la colorer.
    #[test]
    fn tint_preserves_brightness() {
        let c = rarity_tint([0.62, 0.64, 0.68]).to_srgba();
        let max = c.red.max(c.green).max(c.blue);
        assert!((max - 1.0).abs() < 1e-4, "la composante max doit valoir 1");
        // La teinte relative est conservée (ce gris tire vers le bleu).
        assert!(c.blue > c.red);
    }

    #[test]
    fn tint_of_gold_stays_gold() {
        let c = rarity_tint([0.95, 0.72, 0.22]).to_srgba();
        assert!(c.red > c.green && c.green > c.blue);
    }

    #[test]
    fn equipped_key_is_order_independent() {
        let mut a = EquipmentSave::default();
        a.equipped.insert("helmet".into(), "rare".into());
        a.equipped.insert("boots".into(), "commun".into());
        let mut b = EquipmentSave::default();
        b.equipped.insert("boots".into(), "commun".into());
        b.equipped.insert("helmet".into(), "rare".into());
        assert_eq!(equipped_key(&a), equipped_key(&b));
    }

    #[test]
    fn equipped_key_changes_when_a_piece_changes() {
        let mut a = EquipmentSave::default();
        a.equipped.insert("helmet".into(), "rare".into());
        let mut b = a.clone();
        b.equipped.insert("helmet".into(), "epique".into());
        assert_ne!(equipped_key(&a), equipped_key(&b));
    }
}
