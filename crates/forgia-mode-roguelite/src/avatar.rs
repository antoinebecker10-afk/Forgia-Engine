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
use bevy::platform::collections::HashMap;
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
    cfg: &EquipmentConfig,
    save: &EquipmentSave,
    parent: Entity,
    local: Transform,
) -> Vec<Entity> {
    let mut spawned = Vec::new();
    if !cfg.body_model.is_empty() {
        spawned.push(
            commands
                .spawn((
                    AvatarPart,
                    SceneRoot(
                        assets.load(GltfAssetLabel::Scene(0).from_asset(cfg.body_model.clone())),
                    ),
                    AvatarBody,
                    // Le glTF complet, pour lire ses clips PAR NOM une fois chargé.
                    AvatarClips(assets.load(cfg.body_model.clone())),
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
                    AvatarNeedsSkeletonShare,
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
#[derive(Component)]
pub struct AvatarNeedsSkeletonShare;

/// Le corps : il porte l'armature et l'animation de tout l'avatar.
#[derive(Component)]
pub struct AvatarBody;

/// Rebranche les maillages des pièces sur le squelette du corps.
///
/// Réessaie tant que les scènes ne sont pas peuplées : elles n'apparaissent pas
/// à la frame de leur spawn, et rien ne garantit que le corps arrive en premier.
fn sys_share_body_skeleton(
    mut commands: Commands,
    q_body: Query<Entity, With<AvatarBody>>,
    q_pieces: Query<Entity, With<AvatarNeedsSkeletonShare>>,
    q_children: Query<&Children>,
    q_name: Query<&Name>,
    mut q_skin: Query<&mut SkinnedMesh>,
) {
    if q_pieces.is_empty() {
        return;
    }
    // Os du corps indexés par nom, pris depuis SON maillage skinné : c'est la
    // seule liste dont on soit sûr qu'elle contienne les joints, et eux seuls.
    let Some(body) = q_body.iter().next() else {
        return;
    };
    let mut body_bones: HashMap<String, Entity> = HashMap::default();
    for d in q_children.iter_descendants(body) {
        let Ok(skin) = q_skin.get(d) else {
            continue;
        };
        for joint in &skin.joints {
            if let Ok(name) = q_name.get(*joint) {
                body_bones.insert(name.to_string(), *joint);
            }
        }
        break;
    }
    if body_bones.is_empty() {
        return;
    }

    for piece in &q_pieces {
        let mut found = None;
        for d in q_children.iter_descendants(piece) {
            if let Ok(skin) = q_skin.get(d) {
                found = Some((d, skin.joints.clone()));
                break;
            }
        }
        let Some((mesh_entity, own_joints)) = found else {
            continue; // scène pas encore peuplée — on repassera
        };

        // Traduction par NOM. Un seul os introuvable et on renonce : mieux vaut
        // une pièce qui ne suit pas qu'un personnage tordu par un mauvais joint.
        let shared: Vec<Entity> = own_joints
            .iter()
            .map_while(|j| q_name.get(*j).ok().and_then(|n| body_bones.get(n.as_str())).copied())
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
        if let Ok(mut skin) = q_skin.get_mut(mesh_entity) {
            skin.joints = shared;
        }
        // Les os propres de la pièce ne servent plus. Le maillage n'est PAS sous
        // eux (il est frère de la racine d'os dans le glTF, vérifié), donc les
        // supprimer ne l'emporte pas — et ça évite de propager 68 transforms
        // inertes par pièce à chaque frame.
        if let Some(root_bone) = own_joints.first() {
            commands.entity(*root_bone).despawn();
        }
        commands.entity(piece).remove::<AvatarNeedsSkeletonShare>();
        debug!("[avatar] pièce rebranchée sur le squelette du corps");
    }
}

/// Le glTF du corps, gardé pour lire ses `named_animations`.
///
/// 🚨 Les clips se résolvent par **nom**, jamais par index. `Animation(0)` se
/// décale en silence au moindre ré-export ; un nom manquant se signale. C'est le
/// choix déjà fait pour les ennemis (`enemy_anim.rs`).
#[derive(Component)]
pub struct AvatarClips(Handle<Gltf>);

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
    q_parts: Query<(Entity, &AvatarClips)>,
    q_children: Query<&Children>,
    mut q_player: Query<&mut AnimationPlayer>,
) {
    for (part, clips) in &q_parts {
        // Le glTF doit être chargé pour qu'on puisse lire ses noms de clips.
        let Some(gltf) = gltfs.get(&clips.0) else {
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
            bound = true;
        }
        if bound {
            commands.entity(part).remove::<AvatarClips>();
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

/// Enregistre teinte et animation, une fois pour tous les consommateurs.
pub struct AvatarPlugin;

impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvatarLocomotion>().add_systems(
            Update,
            (
                sys_tint_avatar_pieces,
                // Le partage précède le câblage : une pièce encore sur son propre
                // squelette n'a rien à faire jouer.
                sys_share_body_skeleton,
                sys_bind_avatar_animations,
                sys_drive_avatar_locomotion,
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
