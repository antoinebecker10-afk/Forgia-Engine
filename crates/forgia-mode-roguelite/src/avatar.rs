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

use bevy::camera::visibility::NoFrustumCulling;
use bevy::gltf::Gltf;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use std::time::Duration;

use crate::equipment::{AvatarAnimCfg, CorpsAvatar, EquipmentConfig, EquipmentSave};
use crate::locomotion_etat::{Clip, EtatLocomotion, choisir};

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
    /// (chemin, glTF entier, Scene(0)) par corps connu.
    ///
    /// 🚨 Une seule entrée ne suffit PAS depuis qu'il y a deux corps (Hall et
    /// Expédition) : passer de l'un à l'autre lâcherait le handle du premier,
    /// donc son glTF serait déchargé, et le retour au Hall le rechargerait —
    /// c'est exactement la ré-instanciation qui figeait l'armure sur des os
    /// morts. Le cache ne s'invalide jamais : deux corps, deux entrées.
    cached: Vec<(String, Handle<Gltf>, Handle<Scene>)>,
}

impl AvatarBodyHandles {
    /// Les handles du corps pour `path`, chargés UNE fois puis réutilisés.
    fn body(&mut self, assets: &AssetServer, path: &str) -> (Handle<Gltf>, Handle<Scene>) {
        match self.cached.iter().find(|(p, _, _)| p == path) {
            Some((_, g, s)) => (g.clone(), s.clone()),
            None => {
                let gltf: Handle<Gltf> = assets.load(path.to_string());
                let scene: Handle<Scene> =
                    assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()));
                self.cached
                    .push((path.to_string(), gltf.clone(), scene.clone()));
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
    corps: &CorpsAvatar,
    save: &EquipmentSave,
    parent: Entity,
    local: Transform,
) -> Vec<Entity> {
    let mut spawned = Vec::new();
    if !corps.model.is_empty() {
        // Handles issus du cache permanent — cf. [`AvatarBodyHandles`].
        let (gltf, scene) = handles.body(assets, &corps.model);
        spawned.push(
            commands
                .spawn((
                    AvatarPart,
                    SceneRoot(scene),
                    AvatarBody,
                    // Le glTF complet, pour lire ses clips PAR NOM une fois chargé.
                    AvatarClips::new(gltf, corps.anim.clone(), corps.model.clone()),
                    local,
                    Visibility::Inherited,
                    Name::new("avatar_body"),
                    ChildOf(parent),
                ))
                .id(),
        );
    }
    // Un corps à son propre squelette ne porte pas l'armure : les pièces sont
    // riggées sur les os du corps de base et suivraient les mauvais joints —
    // elles flotteraient dans le monde au lieu de disparaître, donc le défaut
    // serait visible sans être compris.
    if !corps.porte_armure {
        return spawned;
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
/// Retire le tri par frustum des maillages **skinnés** de l'avatar.
///
/// # Le défaut que ça corrige, mesuré le 2026-08-14
///
/// Bevy calcule l'`Aabb` d'un maillage depuis ses **sommets en pose de repos**,
/// puis la transforme par le `GlobalTransform` de l'entité. Or un maillage
/// skinné n'est PAS placé par son entité : il est déformé par ses **os**. Les
/// deux n'ont aucune raison de coïncider, et sur ce personnage elles divergent
/// d'un facteur cent :
///
/// | | boîte de tri | géométrie affichée |
/// |---|---|---|
/// | `Cloak_low` (la cape) | **4 × 8 × 1 mm** | ~0,4 × 0,8 m |
///
/// La cape porte une échelle de 0,01 sur son nœud parent (`root.001`) — sans
/// effet sur le rendu, qui passe par les os, mais qui **écrase sa boîte de tri**.
/// Résultat en jeu : elle disparaît dès que ce point de 4 mm sort du champ,
/// c'est-à-dire à beaucoup d'angles. Rapporté tel quel : « le personnage
/// n'apparaît pas sous tous les angles… cape et accessoire aussi ».
///
/// Le second défaut, plus général : **tous** les clips de ce personnage sont des
/// rotations pures (0 translation mesurée sur les 9). Les os font donc sortir la
/// géométrie de la boîte de repos sans jamais la mettre à jour — le cas est
/// structurel, pas propre à la cape.
///
/// # Pourquoi désactiver plutôt que corriger la boîte
///
/// Recalculer l'`Aabb` d'un maillage skinné chaque frame demanderait de lire les
/// matrices de ses joints — un coût par frame pour une dizaine de pièces qui
/// tiennent de toute façon toutes dans le même mètre cube. Les garder toujours
/// rendues coûte moins que de les mesurer, et supprime la classe entière.
pub fn sys_disable_frustum_culling_on_avatar(
    mut commands: Commands,
    q_parts: Query<Entity, With<AvatarPart>>,
    q_children: Query<&Children>,
    q_skinned: Query<Entity, (With<SkinnedMesh>, Without<NoFrustumCulling>)>,
) {
    for part in &q_parts {
        for descendant in q_children.iter_descendants(part) {
            if q_skinned.get(descendant).is_ok() {
                commands.entity(descendant).insert(NoFrustumCulling);
            }
        }
    }
}

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
    /// Les clips ATTENDUS de CE corps. Deux corps n'ont pas les mêmes noms :
    /// lire un réglage global ici ferait chercher « walk » du trooper dans un
    /// glTF qui ne l'a pas, et le personnage resterait bras écartés.
    anim: AvatarAnimCfg,
    /// Le chemin du corps, uniquement pour le diagnostic : savoir QUEL corps
    /// n'a pas trouvé ses clips est la moitié de la réponse.
    modele: String,
    /// Passes écoulées sans avoir trouvé de lecteur d'animation. Même raison que
    /// [`AvatarNeedsSkeletonShare`] : un corps qui n'est jamais câblé reste sur
    /// la pose de repos du glTF — bras écartés — et rien ne le signale.
    tries: u32,
}

impl AvatarClips {
    pub fn new(gltf: Handle<Gltf>, anim: AvatarAnimCfg, modele: String) -> Self {
        Self {
            gltf,
            anim,
            modele,
            tries: 0,
        }
    }
}

/// Nœuds du graphe, une fois le lecteur du corps câblé.
///
/// # Une table, plus trois champs
///
/// Elle portait `idle`/`walk`/`run` en dur. Avec quinze états, quinze champs
/// deviendraient quinze `Option` à tester à la main — et un état oublié serait
/// un `else` silencieux. La table dit exactement ce que CE corps sait jouer : le
/// trooper en aura deux entrées, celui de l'Expédition quinze, et le même code
/// sert les deux.
#[derive(Component)]
struct AvatarAnimNodes {
    /// Ce que ce corps sait jouer. Absent = il ne l'a pas.
    par_clip: HashMap<Clip, AnimationNodeIndex>,
    /// Nœud effectivement joué — évite de relancer le même clip chaque frame,
    /// ce qui le remettrait à zéro et figerait le personnage sur sa 1re pose.
    current: AnimationNodeIndex,
    /// Seuils et fondu de CE corps — même raison que sur [`AvatarClips`].
    anim: AvatarAnimCfg,
    /// Le corps concerné, pour ranger le compteur de relances dans SA fiche.
    modele: String,
}

impl AvatarAnimNodes {
    /// Le nœud à jouer pour cet état, en descendant les replis.
    ///
    /// C'est ici que le trooper — deux clips — survit à une demande de glissade.
    /// `None` seulement si même `Idle` manque, c'est-à-dire si le corps n'a
    /// aucune animation : il n'y a alors rien à jouer, et le dire vaut mieux que
    /// choisir au hasard.
    fn resoudre(&self, voulu: Clip) -> Option<AnimationNodeIndex> {
        if let Some(n) = self.par_clip.get(&voulu) {
            return Some(*n);
        }
        voulu
            .replis()
            .iter()
            .find_map(|c| self.par_clip.get(c).copied())
    }
}

/// Ce que l'avatar doit jouer.
///
/// # Ce que le scalaire d'avant ne pouvait pas dire
///
/// Il n'y avait qu'un `speed`. Une vitesse sans signe ni axe ne distingue pas
/// une marche arrière d'une marche avant, ni un pas de côté d'une course — et
/// elle ne dit rien du sol, donc rien d'un saut. Les onze clips ajoutés le
/// 2026-08-17 étaient inatteignables par construction, pas par oubli.
///
/// `speed` reste : deux lecteurs s'en servent (le capteur du Hall, et lui-même).
/// Les autres champs s'y ajoutent, et un mode qui les laisse à zéro obtient
/// exactement le comportement d'avant.
#[derive(Resource, Default)]
pub struct AvatarLocomotion {
    /// Norme horizontale, conservée pour ses lecteurs existants.
    pub speed: f32,
    /// Signée dans le repère du personnage : positive quand il avance.
    pub avant: f32,
    /// Signée : positive vers sa droite.
    pub lateral: f32,
    pub au_sol: bool,
    pub vitesse_verticale: f32,
    pub accroupi: bool,
    pub glisse: bool,
    pub vise: bool,
    pub tire: bool,
    pub lacet_rad_s: f32,
    pub depuis_atterrissage_s: f32,
    /// Depuis quand il a quitté le sol. Distingue une chute d'un décollement
    /// d'une frame sur une bosse — cf `locomotion_etat::Seuils::air_min_s`.
    pub depuis_decollage_s: f32,
    pub duree_vol_precedent_s: f32,
}

/// Pourquoi l'avatar joue — ou ne joue pas — ses clips.
///
/// # Pourquoi cette ressource existe
///
/// Le 2026-08-14, l'avatar de l'Expédition est resté en pose de repos, bras
/// écartés. Le moteur AVAIT la réponse : un `warn!` imprime les clips attendus
/// et ceux réellement présents. Mais le log n'était pas écrit — le jeu tournait
/// encore — alors que les 99 capteurs, eux, l'étaient. Le diagnostic existait et
/// restait hors de portée.
///
/// 🚨 La leçon dépasse ce cas : **un diagnostic qui ne vit que dans le log est
/// indisponible tant que le jeu tourne**, c'est-à-dire exactement quand on en a
/// besoin. Ce qui doit se lire pendant une partie appartient à un capteur.
/// 🚨 UN DIAGNOSTIC PAR CORPS, jamais un seul global.
///
/// La première version n'en gardait qu'un. Or DEUX corps se câblent en même
/// temps — l'aperçu du menu (trooper) et celui du mode en cours — et le dernier
/// à passer écrasait l'autre. Le 2026-08-16, en pleine enquête sur une T-pose en
/// Expédition, le capteur affichait `corps: trooper/body.gltf` et
/// `clips_attendus: [idle, walk, walk]` : la fiche de l'aperçu du menu, pendant
/// qu'on cherchait celle du personnage à l'écran. Un instrument qui décrit un
/// autre objet que celui qu'on examine coûte plus cher que pas d'instrument,
/// parce qu'on le croit.
#[derive(Resource, Default, Debug, Clone)]
pub struct AvatarClipDiag {
    /// Une fiche par corps rencontré, indexée par son chemin de modèle.
    pub par_corps: Vec<FicheClips>,
}

impl AvatarClipDiag {
    /// La fiche de ce corps, créée au premier passage.
    fn fiche(&mut self, modele: &str) -> &mut FicheClips {
        if let Some(i) = self.par_corps.iter().position(|f| f.corps == modele) {
            return &mut self.par_corps[i];
        }
        self.par_corps.push(FicheClips {
            corps: modele.to_string(),
            ..default()
        });
        self.par_corps.last_mut().expect("on vient de pousser")
    }
}

#[derive(Default, Debug, Clone)]
pub struct FicheClips {
    /// Le corps concerné, pour distinguer Hall, Expédition et aperçu du menu.
    pub corps: String,
    /// Les trois noms cherchés, dans l'ordre repos / marche / course.
    pub attendus: Vec<String>,
    /// Ce que le glTF expose réellement. Vide = il n'est pas encore chargé.
    pub presents: Vec<String>,
    /// Le graphe a-t-il été câblé sur un lecteur d'animation ?
    pub lie: bool,
    /// Combien de fois il a fallu RELANCER un lecteur devenu muet.
    ///
    /// 0 = le corps n'a jamais cessé de jouer. Un nombre qui monte lentement
    /// désigne un évènement précis (une reconstruction, un changement de mode).
    /// Un nombre qui monte à chaque frame désigne une boucle : quelque chose
    /// arrête le lecteur en continu, et le rattrapage ne fait que masquer.
    pub relances: u32,
    /// Passes écoulées à chercher. Grand et `lie == false` = ça ne viendra plus.
    pub passes: u32,
    /// 🚨 L'état que le sélecteur DEMANDE, et le clip qui est réellement joué.
    ///
    /// # La mesure qui manquait, et ce qu'elle sépare
    ///
    /// L'audit du 2026-08-16 l'a nommée : aucun capteur ne disait **quel clip
    /// joue**. Sans elle, « le personnage ne s'accroupit pas » a quatre causes
    /// indiscernables — l'entrée n'est pas lue, la posture n'est pas écrite, le
    /// sélecteur ne choisit pas cet état, ou le clip est absent du corps.
    ///
    /// Les publier TOUS LES DEUX est ce qui tranche : `demande` ≠ `joue` dit que
    /// le corps n'a pas ce clip et qu'un repli a servi ; `demande` faux dit que
    /// c'est en amont, dans l'entrée ou la posture. Une seule des deux valeurs
    /// n'aurait pas suffi.
    pub etat_demande: String,
    pub clip_joue: String,
}

/// Câble le graphe d'animation dès que la scène d'une pièce expose son lecteur.
///
/// Un `SceneRoot` n'a pas d'`AnimationPlayer` à la frame de son spawn : le
/// composant `AvatarClips` reste posé jusqu'à ce que le lecteur apparaisse.
fn sys_bind_avatar_animations(
    mut commands: Commands,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    gltfs: Res<Assets<Gltf>>,
    mut diag: ResMut<AvatarClipDiag>,
    mut q_parts: Query<(Entity, &mut AvatarClips)>,
    q_children: Query<&Children>,
    mut q_player: Query<&mut AnimationPlayer>,
) {
    for (part, mut clips) in &mut q_parts {
        clips.tries = clips.tries.saturating_add(1);
        let complain = clips.tries == SKELETON_SHARE_PATIENCE;
        // Le diagnostic se met à jour à CHAQUE passe, y compris celles où rien
        // n'aboutit : c'est justement l'état « rien n'aboutit » qu'on veut
        // pouvoir lire pendant que le jeu tourne. Et il est indexé par CORPS,
        // sinon l'aperçu du menu écrase la fiche du personnage joué.
        let fiche = diag.fiche(&clips.modele);
        fiche.attendus = vec![
            clips.anim.idle_clip.clone(),
            clips.anim.walk_clip.clone(),
            clips.anim.run_clip.clone(),
        ];
        fiche.passes = clips.tries;
        fiche.lie = false;
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
        let anim = clips.anim.clone();
        let fiche = diag.fiche(&clips.modele);
        fiche.presents = gltf
            .named_animations
            .keys()
            .map(|k| k.to_string())
            .collect();
        fiche.presents.sort();
        let pick = |name: &str| gltf.named_animations.get(name).cloned();
        // `Idle` est la seule exigence dure : c'est le fond de toutes les chaînes
        // de repli. Sans lui, aucun état n'a de solution et le corps resterait
        // en pose de bind, bras écartés — l'échec qu'on veut nommer.
        let Some(idle_handle) = pick(&anim.idle_clip) else {
            warn!(
                "[avatar] le clip de repos « {} » est absent du corps — présents {:?}. \
                 Rien ne peut être joué : tous les états retombent sur le repos.",
                anim.idle_clip,
                gltf.named_animations.keys().collect::<Vec<_>>()
            );
            commands.entity(part).remove::<AvatarClips>();
            continue;
        };
        // Ce que CE corps sait jouer, état par état. Un nom vide au génome ou
        // absent du glTF n'est pas une erreur : c'est un état que ce corps n'a
        // pas, et le sélecteur descendra ses replis. Le trooper en remplit deux,
        // celui de l'Expédition quinze, et le code est le même.
        let mut demandes: Vec<String> = Vec::new();
        let mut manquants: Vec<String> = Vec::new();
        let mut trouves: Vec<(Clip, bevy::prelude::Handle<AnimationClip>)> = Vec::new();
        for c in Clip::TOUS {
            let nom = anim.nom_du_clip(c);
            if nom.is_empty() {
                continue;
            }
            demandes.push(nom.to_string());
            match pick(nom) {
                Some(h) => trouves.push((c, h)),
                None => manquants.push(format!("{c:?}→{nom}")),
            }
        }
        if !manquants.is_empty() {
            // Un nom déclaré mais absent est une FAUTE DE FRAPPE au génome, pas
            // un corps pauvre — les deux se distinguent, donc ils se disent
            // différemment. Sans ce message, un `rifle_wlak` se lirait comme
            // « ce corps n'a pas de marche ».
            warn!(
                "[avatar] {} clip(s) déclaré(s) au génome mais absent(s) de {} : {} — \
                 vérifier l'orthographe ; ces états retomberont sur leurs replis",
                manquants.len(),
                clips.modele,
                manquants.join(", ")
            );
        }
        fiche.attendus = demandes;

        let mut bound = false;
        for descendant in q_children.iter_descendants(part) {
            let Ok(mut player) = q_player.get_mut(descendant) else {
                continue;
            };
            let mut graph = AnimationGraph::new();
            let root = graph.root;
            let mut par_clip: HashMap<Clip, AnimationNodeIndex> = HashMap::default();
            // Le repos d'abord : il est le fond de toutes les chaînes de repli,
            // donc il doit exister avant qu'on résolve quoi que ce soit.
            let noeud_idle = graph.add_clip(idle_handle.clone(), 1.0, root);
            par_clip.insert(Clip::Idle, noeud_idle);
            for (c, h) in &trouves {
                if *c == Clip::Idle {
                    continue;
                }
                par_clip.insert(*c, graph.add_clip(h.clone(), 1.0, root));
            }
            let nodes = AvatarAnimNodes {
                par_clip,
                current: noeud_idle,
                anim: anim.clone(),
                modele: clips.modele.clone(),
            };
            // `AnimationTransitions` est ce qui permet le FONDU. Sans lui, tout
            // changement de clip est une coupure sèche — c'est ce qui trahit le
            // plus une animation de jeu.
            let mut transitions = AnimationTransitions::new();
            transitions
                .play(&mut player, noeud_idle, Duration::ZERO)
                .repeat();
            let combien = nodes.par_clip.len();
            commands.entity(descendant).insert((
                AnimationGraphHandle(graphs.add(graph)),
                transitions,
                nodes,
            ));
            // Le repos joue dès le bind. Publier cette transition initiale ici
            // évite un diagnostic vide jusqu'au premier mouvement du joueur.
            let fiche = diag.fiche(&clips.modele);
            fiche.etat_demande = format!("{:?}", Clip::Idle);
            fiche.clip_joue = format!("{:?}", Clip::Idle);
            // Un corps non re-câblé après reconstruction reste sur la pose de
            // repos du glTF (bras écartés) — c'est ce qu'on voit à l'écran quand
            // l'avatar « se désynchronise ». Tracer le câblage permet de dire
            // s'il a eu lieu, au lieu de le supposer. Le NOMBRE d'états câblés
            // distingue en plus « ce corps est pauvre » de « le génome est faux ».
            info!("[avatar] animations câblées sur le corps {part:?} — {combien} état(s)");
            bound = true;
        }
        diag.fiche(&clips.modele).lie = bound;
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
    loco: Res<AvatarLocomotion>,
    mut diag: ResMut<AvatarClipDiag>,
    mut q: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut AvatarAnimNodes,
    )>,
) {
    for (mut player, mut transitions, mut nodes) in &mut q {
        // 🚨 LE LECTEUR PEUT SE TAIRE APRÈS UN CÂBLAGE RÉUSSI, et rien ne le
        // relançait.
        //
        // Mesuré le 2026-08-16 : `clips_lies: true`, les trois clips attendus
        // présents dans le glTF, `anim_players: 1` — et `anim_playing: 0`. Le
        // personnage restait bras écartés en jeu pendant que tous les
        // indicateurs de câblage étaient au vert.
        //
        // Le mécanisme existe dans Bevy : `expire_completed_transitions` appelle
        // `player.stop()` sur toute transition dont le poids atteint zéro. Rien
        // ne garantit qu'il reste ensuite une animation active — et la boucle
        // ci-dessous ne relançait QUE sur changement de clip, donc jamais.
        //
        // On ne devine pas le déclencheur : on tient l'INVARIANT — le corps joue
        // toujours quelque chose — et on COMPTE les fois où il a fallu le
        // rattraper. Un compteur qui monte nomme la cause ; un personnage figé,
        // non.
        let muet = player.playing_animations().count() == 0;
        let anim = nodes.anim.clone();
        let cross = Duration::from_millis(anim.crossfade_ms);
        // La DÉCISION vit dans `locomotion_etat` — pure, testée hors moteur.
        // Ici on ne fait plus que la traduire en nœud. C'est ce qui permet de
        // vérifier quinze états en quinze assertions plutôt qu'en playtest.
        let etat = EtatLocomotion {
            avant_ms: loco.avant,
            lateral_ms: loco.lateral,
            au_sol: loco.au_sol,
            vitesse_verticale_ms: loco.vitesse_verticale,
            accroupi: loco.accroupi,
            glisse: loco.glisse,
            vise: loco.vise,
            tire: loco.tire,
            lacet_rad_s: loco.lacet_rad_s,
            depuis_atterrissage_s: loco.depuis_atterrissage_s,
            // Sans ce report, le délai de grâce reste à zéro et la machine
            // retombe sur l'ancien comportement : la chute clignote pendant la
            // marche. Un producteur branché à moitié est un correctif absent.
            depuis_decollage_s: loco.depuis_decollage_s,
            duree_vol_precedent_s: loco.duree_vol_precedent_s,
        };
        let voulu = choisir(&etat, &anim.seuils());
        // Résolution par replis : c'est ici que le trooper — deux clips — survit
        // à une demande de glissade au lieu de se figer.
        let Some(next) = nodes.resoudre(voulu) else {
            // Même le repos manque : ce corps n'a aucune animation. Le câblage
            // l'a déjà dit, inutile de le répéter soixante fois par seconde.
            continue;
        };
        // Muet ⇒ on relance MÊME si le clip voulu n'a pas changé : c'est
        // précisément le cas que l'ancienne condition laissait passer.
        if nodes.current == next && !muet {
            continue;
        }
        // Sans fondu quand on rattrape : fondre depuis le néant laisserait le
        // personnage transparent au mouvement pendant 150 ms de plus.
        let fondu = if muet { Duration::ZERO } else { cross };
        // 🚨 Trois clips ne DOIVENT pas boucler. Un atterrissage répété est un
        // personnage qui rebondit sur place ; une glissade répétée est un tapis
        // roulant ; un tir répété vide le chargeur à l'écran alors qu'on a lâché
        // la gâchette. Ils s'éteignent d'eux-mêmes, et l'invariant « le corps
        // joue toujours quelque chose » ci-dessus les rattrape en repos.
        let boucle = !matches!(voulu, Clip::Atterrissage | Clip::Glissade | Clip::Tir);
        let active = transitions.play(&mut player, next, fondu);
        if boucle {
            active.repeat();
        }
        nodes.current = next;
        // Ce que le sélecteur voulait, et ce qui sort vraiment. Écrit ICI, au
        // moment du changement : c'est le seul endroit où les deux existent en
        // même temps. Le repli ayant servi se lit dans leur ÉCART.
        {
            let modele = nodes.modele.clone();
            let joue = nodes
                .par_clip
                .iter()
                .find(|(_, n)| **n == next)
                .map(|(c, _)| format!("{c:?}"))
                .unwrap_or_else(|| "?".to_string());
            let fiche = diag.fiche(&modele);
            fiche.etat_demande = format!("{voulu:?}");
            fiche.clip_joue = joue;
        }
        if muet {
            let fiche = diag.fiche(&nodes.modele);
            fiche.relances = fiche.relances.saturating_add(1);
        }
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
            .init_resource::<AvatarClipDiag>()
            .add_systems(
                Update,
                (
                    sys_tint_avatar_pieces,
                    // Avant tout le reste : une pièce triée hors du champ ne se voit
                    // pas, quelle que soit la justesse de sa teinte ou de son os.
                    sys_disable_frustum_culling_on_avatar,
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
