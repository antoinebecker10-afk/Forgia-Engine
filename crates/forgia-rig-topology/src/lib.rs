//! # forgia-rig-topology — Procedural Rig Classification (story-440 Phase 1A)
//!
//! **Spécialité Forgia** : classifier les bones d'un squelette importé
//! (Meshy, AccuRig, Mixamo, Reallusion, Blender) sans dépendre des Names.
//!
//! Approche : heuristiques 3D positionnelles + bonus Name. Le mesh peut sortir
//! avec n'importe quelle convention de noms, on s'en fout — on regarde la
//! géométrie du rig.
//!
//! ## Algorithme
//! 1. Trouver le bone root (descendant de la SceneRoot avec le plus de descendants).
//! 2. BFS depuis root, calculer position locale de chaque bone (parent chain).
//! 3. Pour chaque bone, scorer son aptitude à être : leg, arm, spine, tail, head.
//!    Scoring combine : axe de pousse (vec parent→bone), position relative au root
//!    (Y altitude, |X| latéralité, Z avant/arrière), longueur du sous-arbre.
//! 4. Sélection greedy par rôle : meilleur score, contrainte symétrie L/R via X sign.
//!
//! ## Pourquoi pas BFS-par-Name
//! Cassé en pratique : V1 character.rs tablait sur `mixamorig:LeftUpLeg`, `LeftUpLeg`,
//! `L_Thigh`, `thigh.L`, `DEF-thigh.L`, `leg_l`. Rex.glb (Meshy v5) sort des noms
//! hors de cette table → cache vide → 0 anim. La géométrie elle, est universelle.
//!
//! ## API
//! - `analyze_rig_topology(root_entity, &world) -> RigTopology` : one-shot à
//!   l'arrivée de la Scene.
//! - `RigTopology` Component porté par l'entité du character — consommé par
//!   `forgia-rpg::character::procedural_locomotion`.

use bevy::prelude::*;

/// Résultat de classification du squelette. Les `Option<Entity>` sont les
/// candidats best-scorés par rôle, ou None si aucun candidat ne dépasse le seuil.
#[derive(Component, Default, Debug, Clone)]
pub struct RigTopology {
    pub root: Option<Entity>,
    pub left_leg: Option<Entity>,
    pub right_leg: Option<Entity>,
    pub left_arm: Option<Entity>,
    pub right_arm: Option<Entity>,
    pub spine: Option<Entity>,
    pub head: Option<Entity>,
    /// QW3 (story-637) — bones enfants dérivés par chaîne (le + de descendants).
    /// Fallback quand les noms ne matchent pas le template (rigs étrangers).
    /// forearm = enfant du bras ; hand = enfant du forearm ; shin/foot idem jambes ;
    /// neck = parent direct de head.
    pub left_forearm: Option<Entity>,
    pub left_hand: Option<Entity>,
    pub right_forearm: Option<Entity>,
    pub right_hand: Option<Entity>,
    pub left_shin: Option<Entity>,
    pub left_foot: Option<Entity>,
    pub right_shin: Option<Entity>,
    pub right_foot: Option<Entity>,
    pub neck: Option<Entity>,
    /// Tail = chaîne ordonnée du root du tail vers la pointe (Vec::first() = root).
    pub tail_chain: Vec<Entity>,
    /// Diagnostic : liste de tous les bones inspectés (name + local_pos) pour log.
    pub diagnostics: Vec<BoneDiag>,
}

#[derive(Debug, Clone)]
pub struct BoneDiag {
    pub entity: Entity,
    pub name: String,
    pub local_pos: Vec3,
    pub depth: u32,
    pub child_count: usize,
}

impl RigTopology {
    /// True si au moins un bone leg OU spine OU tail a été identifié.
    pub fn is_usable(&self) -> bool {
        self.spine.is_some()
            || self.left_leg.is_some()
            || self.right_leg.is_some()
            || !self.tail_chain.is_empty()
    }
}

/// Analyse one-shot du squelette descendant de `scene_root`.
///
/// `name_of` : function-like accessor du composant `Name` (caller passe une closure
/// qui appelle un `Query<&Name>` ou équivalent — découplage du système Bevy).
/// `children_of` : idem pour `Children`.
/// `transform_of` : idem pour `Transform`.
pub fn analyze_rig_topology(
    scene_root: Entity,
    children_of: &dyn Fn(Entity) -> Vec<Entity>,
    transform_of: &dyn Fn(Entity) -> Option<Transform>,
    name_of: &dyn Fn(Entity) -> Option<String>,
) -> RigTopology {
    let mut topo = RigTopology::default();

    // Étape 1 : trouve le bone root = enfant direct de scene_root avec le plus
    // de descendants (souvent "Armature" ou "mixamorig:Hips").
    let direct_children = children_of(scene_root);
    let bone_root = direct_children
        .into_iter()
        .map(|e| (e, count_descendants(e, children_of)))
        .max_by_key(|(_, n)| *n)
        .filter(|(_, n)| *n > 0)
        .map(|(e, _)| e);

    let Some(bone_root) = bone_root else {
        return topo;
    };
    topo.root = Some(bone_root);

    // Étape 2 : BFS, accumule local_pos relative à bone_root.
    let mut bones: Vec<BoneInfo> = Vec::new();
    let mut stack = vec![(bone_root, Vec3::ZERO, 0u32)];
    while let Some((entity, parent_world, depth)) = stack.pop() {
        let local_tf = transform_of(entity).unwrap_or_default();
        let world_pos = parent_world + local_tf.translation;
        let name = name_of(entity).unwrap_or_default();
        let children = children_of(entity);
        bones.push(BoneInfo {
            entity,
            name: name.clone(),
            world_pos,
            depth,
            child_count: children.len(),
        });
        for c in children {
            stack.push((c, world_pos, depth + 1));
        }
    }

    // root_pos = origine du squelette dans son repère.
    let root_pos = bones
        .iter()
        .find(|b| b.entity == bone_root)
        .map(|b| b.world_pos)
        .unwrap_or(Vec3::ZERO);

    // Échelle de référence : Y max (= hauteur du rig). Sert à normaliser les seuils.
    let height = bones
        .iter()
        .map(|b| (b.world_pos.y - root_pos.y).abs())
        .fold(0.0_f32, f32::max)
        .max(0.01);

    // Étape 3 : scoring par rôle.
    let mut left_leg_best: Option<(Entity, f32)> = None;
    let mut right_leg_best: Option<(Entity, f32)> = None;
    let mut left_arm_best: Option<(Entity, f32)> = None;
    let mut right_arm_best: Option<(Entity, f32)> = None;
    let mut spine_best: Option<(Entity, f32)> = None;
    let mut head_best: Option<(Entity, f32)> = None;

    for b in &bones {
        if b.entity == bone_root {
            continue;
        }
        let rel = b.world_pos - root_pos;
        let y_norm = rel.y / height; // négatif = sous root, positif = au-dessus
        let x_norm = rel.x / height; // signe = côté
        let z_norm = rel.z / height; // signe = avant/arrière

        let lower = name_lower(&b.name);

        // Leg : en dessous du root (-Y), latéralisé (|X|>0.05), peu profond dans
        // la hiérarchie (depth 1..4 typique pour la cuisse).
        if y_norm < -0.05 && x_norm.abs() > 0.03 && b.depth <= 5 {
            let score = (-y_norm) * 2.0 + x_norm.abs() * 3.0 - (b.depth as f32) * 0.05
                + name_boost(&lower, &["thigh", "upleg", "leg", "_l_", "_r_", "hip"]);
            if x_norm > 0.0 {
                update_best(&mut right_leg_best, b.entity, score);
            } else {
                update_best(&mut left_leg_best, b.entity, score);
            }
        }

        // Arm : au-dessus root (+Y), latéralisé fort, depth modérée.
        // On veut l'**upper arm (shoulder)** — pas le forearm. forearm partage les
        // mêmes propriétés Y/X mais est plus latéral (gagne sur x_norm). Penalty
        // explicite sur les noms forearm/lower_arm/hand pour faire gagner l'upper.
        // Bonus enfants (upper arm a forearm + (chain), forearm a juste hand).
        if y_norm > 0.1 && x_norm.abs() > 0.1 && b.depth <= 6 {
            let is_forearm_name = lower.contains("forearm")
                || lower.contains("lower_arm")
                || lower.contains("lowerarm")
                || lower.contains("hand")
                || lower.contains("wrist");
            let forearm_penalty = if is_forearm_name { -4.0 } else { 0.0 };
            let upper_boost =
                name_boost(&lower, &["shoulder", "clavicle", "upper_arm", "upperarm"]);
            let arm_match = name_boost(&lower, &["arm"]);
            let score = y_norm + x_norm.abs() * 4.0 - (b.depth as f32) * 0.1
                + arm_match
                + upper_boost
                + forearm_penalty;
            if x_norm > 0.0 {
                update_best(&mut right_arm_best, b.entity, score);
            } else {
                update_best(&mut left_arm_best, b.entity, score);
            }
        }

        // Spine : sur l'axe vertical central (|X|<0.05), au-dessus root, depth 1..3.
        if y_norm > 0.0 && x_norm.abs() < 0.08 && b.depth <= 4 {
            let score = y_norm * 2.0 - x_norm.abs() * 10.0 - (b.depth as f32) * 0.1
                + name_boost(&lower, &["spine", "torso", "chest", "back"]);
            update_best(&mut spine_best, b.entity, score);
        }

        // Head : haut du squelette (y_norm fort), central, profondeur élevée.
        if y_norm > 0.4 && x_norm.abs() < 0.1 {
            let score =
                y_norm * 3.0 - x_norm.abs() * 5.0 + name_boost(&lower, &["head", "neck", "skull"]);
            update_best(&mut head_best, b.entity, score);
        }

        topo.diagnostics.push(BoneDiag {
            entity: b.entity,
            name: b.name.clone(),
            local_pos: rel,
            depth: b.depth,
            child_count: b.child_count,
        });

        // Tail collecté hors loop principal (séquence ordonnée).
        let _ = z_norm;
    }

    topo.left_leg = left_leg_best.map(|(e, _)| e);
    topo.right_leg = right_leg_best.map(|(e, _)| e);
    topo.left_arm = left_arm_best.map(|(e, _)| e);
    topo.right_arm = right_arm_best.map(|(e, _)| e);
    topo.spine = spine_best.map(|(e, _)| e);
    topo.head = head_best.map(|(e, _)| e);

    // QW3 (story-637) — dérive les bones enfants par chaîne anatomique : l'enfant
    // qui a le PLUS de descendants continue la chaîne (arm→forearm→hand,
    // thigh→shin→foot). Fallback pour rigs aux noms non-template ; côté locomotion
    // le NOM garde la priorité (`lookup(..).or(topo.child)`). One-shot (attach).
    let linear_child = |e: Entity| -> Option<Entity> {
        children_of(e)
            .into_iter()
            .map(|c| (c, count_descendants(c, children_of)))
            .max_by_key(|(_, n)| *n)
            .map(|(c, _)| c)
    };
    if let Some(arm) = topo.left_arm {
        topo.left_forearm = linear_child(arm);
        if let Some(fa) = topo.left_forearm {
            topo.left_hand = linear_child(fa);
        }
    }
    if let Some(arm) = topo.right_arm {
        topo.right_forearm = linear_child(arm);
        if let Some(fa) = topo.right_forearm {
            topo.right_hand = linear_child(fa);
        }
    }
    if let Some(leg) = topo.left_leg {
        topo.left_shin = linear_child(leg);
        if let Some(sh) = topo.left_shin {
            topo.left_foot = linear_child(sh);
        }
    }
    if let Some(leg) = topo.right_leg {
        topo.right_shin = linear_child(leg);
        if let Some(sh) = topo.right_shin {
            topo.right_foot = linear_child(sh);
        }
    }
    // neck = parent direct de head (le bone dont head est enfant), hors root.
    if let Some(head) = topo.head {
        topo.neck = bones
            .iter()
            .find(|b| b.entity != bone_root && children_of(b.entity).contains(&head))
            .map(|b| b.entity);
    }

    // Étape 4 : tail. Heuristique = chaîne descendante depuis un bone bas-arrière
    // (y_norm ≤ 0.1) qui pointe arrière (|z_norm|>x_norm.abs()) et a une chaîne
    // de descendants linéaire (child_count == 1 répétitivement).
    //
    // Approche : trouver le bone le plus arrière par |z_norm|, low Y, qui démarre
    // une chaîne linéaire de longueur >= 2.
    let tail_root = bones
        .iter()
        .filter(|b| {
            let rel = b.world_pos - root_pos;
            let y_norm = rel.y / height;
            let z_norm = rel.z / height;
            let x_norm = rel.x / height;
            // Tail démarre sous/à hauteur du root, arrière (|z| fort), central (faible |x|).
            y_norm < 0.3
                && z_norm.abs() > 0.1
                && x_norm.abs() < 0.15
                && b.child_count <= 2
        })
        .map(|b| {
            let chain_len = trace_linear_chain_len(b.entity, children_of, transform_of);
            (b.entity, chain_len, b.depth)
        })
        .filter(|(_, len, _)| *len >= 2)
        // Le tail root est en général en haut de chaîne (depth faible parmi candidates).
        .min_by_key(|(_, len, depth)| (*depth, std::cmp::Reverse(*len)))
        .map(|(e, _, _)| e);

    if let Some(root_e) = tail_root {
        let mut chain = vec![root_e];
        let mut current = root_e;
        loop {
            let kids = children_of(current);
            // Suit le child le plus aligné (single-child linéarité)
            if kids.len() != 1 {
                break;
            }
            chain.push(kids[0]);
            current = kids[0];
            if chain.len() > 16 {
                break; // safety
            }
        }
        topo.tail_chain = chain;
    }

    topo
}

// ── Internals ─────────────────────────────────────────────────────────────────

struct BoneInfo {
    entity: Entity,
    name: String,
    world_pos: Vec3,
    depth: u32,
    child_count: usize,
}

fn count_descendants(e: Entity, children_of: &dyn Fn(Entity) -> Vec<Entity>) -> usize {
    let mut n = 0;
    let mut stack = vec![e];
    while let Some(x) = stack.pop() {
        let kids = children_of(x);
        n += kids.len();
        for k in kids {
            stack.push(k);
        }
    }
    n
}

fn trace_linear_chain_len(
    start: Entity,
    children_of: &dyn Fn(Entity) -> Vec<Entity>,
    _transform_of: &dyn Fn(Entity) -> Option<Transform>,
) -> usize {
    let mut len = 1;
    let mut current = start;
    loop {
        let kids = children_of(current);
        if kids.len() != 1 {
            break;
        }
        len += 1;
        current = kids[0];
        if len > 16 {
            break;
        }
    }
    len
}

fn update_best(slot: &mut Option<(Entity, f32)>, e: Entity, score: f32) {
    match slot {
        Some((_, s)) if *s >= score => {}
        _ => *slot = Some((e, score)),
    }
}

fn name_lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

fn name_boost(lower: &str, needles: &[&str]) -> f32 {
    if needles.iter().any(|n| lower.contains(n)) {
        1.5
    } else {
        0.0
    }
}

pub struct ForgiaRigTopologyPlugin;

impl Plugin for ForgiaRigTopologyPlugin {
    fn build(&self, _app: &mut App) {
        // Pas de système enregistré : crate library-only, l'orchestrateur
        // (forgia-rpg::character) appelle `analyze_rig_topology` à la demande.
    }
}

// ── Tests headless ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Construit dans un Bevy World un squelette mock et retourne :
    /// - l'Entity du scene root
    /// - une closure factory pour accéder enfants/transform/name
    /// - un `HashMap<&str, Entity>` pour récupérer les bones par leur Name de test.
    ///
    /// Schema entrée : Vec<(name, parent_name_or_None, local_translation)>
    struct Built {
        world: World,
        scene_root: Entity,
        by_name: HashMap<String, Entity>,
    }

    fn build(bones: Vec<(&str, Option<&str>, Vec3)>) -> Built {
        let mut world = World::new();
        let mut by_name: HashMap<String, Entity> = HashMap::new();
        // Premier passage : spawn toutes les entités avec Transform + Name.
        for (name, _parent, pos) in &bones {
            let e = world
                .spawn((
                    Transform::from_translation(*pos),
                    Name::new(name.to_string()),
                ))
                .id();
            by_name.insert(name.to_string(), e);
        }
        // Deuxième passage : pose Children via add_child.
        for (name, parent, _pos) in &bones {
            if let Some(parent_name) = parent {
                let p = by_name[*parent_name];
                let c = by_name[*name];
                world.entity_mut(p).add_child(c);
            }
        }
        let scene_root = by_name[bones[0].0];
        Built {
            world,
            scene_root,
            by_name,
        }
    }

    fn analyze(b: &Built) -> RigTopology {
        let children_of = |e: Entity| {
            b.world
                .get::<Children>(e)
                .map(|c| c.iter().collect())
                .unwrap_or_default()
        };
        let transform_of = |e: Entity| b.world.get::<Transform>(e).copied();
        let name_of = |e: Entity| b.world.get::<Name>(e).map(|n| n.to_string());
        analyze_rig_topology(b.scene_root, &children_of, &transform_of, &name_of)
    }

    #[test]
    fn classifies_humanoid_rig_with_mixamo_names() {
        let b = build(vec![
            ("SceneRoot", None, Vec3::ZERO),
            ("Armature", Some("SceneRoot"), Vec3::ZERO),
            ("Hips", Some("Armature"), Vec3::ZERO),
            ("mixamorig:Spine", Some("Hips"), Vec3::new(0.0, 0.5, 0.0)),
            (
                "mixamorig:LeftUpLeg",
                Some("Hips"),
                Vec3::new(-0.15, -0.4, 0.0),
            ),
            (
                "mixamorig:RightUpLeg",
                Some("Hips"),
                Vec3::new(0.15, -0.4, 0.0),
            ),
            (
                "mixamorig:LeftArm",
                Some("mixamorig:Spine"),
                Vec3::new(-0.25, 0.3, 0.0),
            ),
            (
                "mixamorig:RightArm",
                Some("mixamorig:Spine"),
                Vec3::new(0.25, 0.3, 0.0),
            ),
            (
                "mixamorig:Head",
                Some("mixamorig:Spine"),
                Vec3::new(0.0, 0.4, 0.0),
            ),
        ]);
        let topo = analyze(&b);
        assert!(topo.is_usable());
        assert_eq!(topo.left_leg, Some(b.by_name["mixamorig:LeftUpLeg"]));
        assert_eq!(topo.right_leg, Some(b.by_name["mixamorig:RightUpLeg"]));
        assert_eq!(topo.left_arm, Some(b.by_name["mixamorig:LeftArm"]));
        assert_eq!(topo.right_arm, Some(b.by_name["mixamorig:RightArm"]));
        assert_eq!(topo.spine, Some(b.by_name["mixamorig:Spine"]));
        assert_eq!(topo.head, Some(b.by_name["mixamorig:Head"]));
    }

    #[test]
    fn derives_arm_chain_and_neck_children() {
        // QW3 (story-637) : forearm/hand dérivés par `linear_child` (enfant au + de
        // descendants) depuis l'upper arm ; neck = parent direct de head.
        let b = build(vec![
            ("scene", None, Vec3::ZERO),
            ("armature", Some("scene"), Vec3::ZERO),
            ("bone_root", Some("armature"), Vec3::ZERO),
            ("spine", Some("bone_root"), Vec3::new(0.0, 0.5, 0.0)),
            ("neck", Some("spine"), Vec3::new(0.0, 0.7, 0.0)),
            ("head", Some("neck"), Vec3::new(0.0, 0.9, 0.0)),
            ("arm_l", Some("spine"), Vec3::new(-0.25, 0.3, 0.0)),
            ("forearm_l", Some("arm_l"), Vec3::new(-0.45, 0.2, 0.0)),
            ("hand_l", Some("forearm_l"), Vec3::new(-0.6, 0.1, 0.0)),
            ("arm_r", Some("spine"), Vec3::new(0.25, 0.3, 0.0)),
            ("forearm_r", Some("arm_r"), Vec3::new(0.45, 0.2, 0.0)),
            ("hand_r", Some("forearm_r"), Vec3::new(0.6, 0.1, 0.0)),
            // jambes présentes pour un humanoïde complet (assertions = bras/neck,
            // robustes ; la dérivation jambe dépend du scoring du thigh, couvert
            // par la priorité au NOM côté locomotion).
            ("thigh_l", Some("bone_root"), Vec3::new(-0.15, -0.4, 0.0)),
            ("thigh_r", Some("bone_root"), Vec3::new(0.15, -0.4, 0.0)),
        ]);
        let topo = analyze(&b);
        assert_eq!(
            topo.left_arm,
            Some(b.by_name["arm_l"]),
            "upper arm identifié"
        );
        assert_eq!(topo.left_forearm, Some(b.by_name["forearm_l"]));
        assert_eq!(topo.left_hand, Some(b.by_name["hand_l"]));
        assert_eq!(topo.right_forearm, Some(b.by_name["forearm_r"]));
        assert_eq!(topo.right_hand, Some(b.by_name["hand_r"]));
        assert_eq!(topo.neck, Some(b.by_name["neck"]), "neck = parent de head");
    }

    #[test]
    fn classifies_unnamed_rig_by_geometry_only() {
        // Même topologie 3D, mais TOUS les noms sont génériques "bone_N".
        // C'est le test crucial : si la géométrie suffit, on classifie correctement.
        let b = build(vec![
            ("scene", None, Vec3::ZERO),
            ("armature", Some("scene"), Vec3::ZERO),
            ("bone_root", Some("armature"), Vec3::ZERO),
            ("bone_spine", Some("bone_root"), Vec3::new(0.0, 0.5, 0.0)),
            ("bone_leg_l", Some("bone_root"), Vec3::new(-0.15, -0.4, 0.0)),
            ("bone_leg_r", Some("bone_root"), Vec3::new(0.15, -0.4, 0.0)),
            ("bone_arm_l", Some("bone_spine"), Vec3::new(-0.25, 0.3, 0.0)),
            ("bone_arm_r", Some("bone_spine"), Vec3::new(0.25, 0.3, 0.0)),
            ("bone_head", Some("bone_spine"), Vec3::new(0.0, 0.4, 0.0)),
        ]);
        let topo = analyze(&b);
        assert!(topo.is_usable());
        assert_eq!(topo.left_leg, Some(b.by_name["bone_leg_l"]));
        assert_eq!(topo.right_leg, Some(b.by_name["bone_leg_r"]));
        assert_eq!(topo.left_arm, Some(b.by_name["bone_arm_l"]));
        assert_eq!(topo.right_arm, Some(b.by_name["bone_arm_r"]));
        assert!(topo.spine.is_some());
    }

    #[test]
    fn detects_tail_chain_for_quadruped() {
        let b = build(vec![
            ("scene", None, Vec3::ZERO),
            ("rig", Some("scene"), Vec3::ZERO),
            ("spine_base", Some("rig"), Vec3::ZERO),
            ("spine_top", Some("spine_base"), Vec3::new(0.0, 0.3, 0.0)),
            ("leg_l", Some("spine_base"), Vec3::new(-0.2, -0.4, 0.0)),
            ("leg_r", Some("spine_base"), Vec3::new(0.2, -0.4, 0.0)),
            ("tail_01", Some("spine_base"), Vec3::new(0.0, -0.1, -0.3)),
            ("tail_02", Some("tail_01"), Vec3::new(0.0, 0.0, -0.2)),
            ("tail_03", Some("tail_02"), Vec3::new(0.0, 0.0, -0.2)),
            ("tail_04", Some("tail_03"), Vec3::new(0.0, 0.0, -0.2)),
        ]);
        let topo = analyze(&b);
        assert!(!topo.tail_chain.is_empty(), "tail chain must be detected");
        assert_eq!(topo.tail_chain[0], b.by_name["tail_01"]);
        assert!(topo.tail_chain.len() >= 3);
    }

    #[test]
    fn distinguishes_upper_arm_from_forearm() {
        // Régression bug Rex 2026-05-18 : topology retournait `forearm_L`
        // au lieu de `arm_L` parce que forearm est plus latéral et match "arm".
        // Le fix doit pénaliser les noms contenant "forearm" / "hand".
        let b = build(vec![
            ("scene", None, Vec3::ZERO),
            ("armature", Some("scene"), Vec3::ZERO),
            ("hip", Some("armature"), Vec3::ZERO),
            ("spine_lower", Some("hip"), Vec3::new(0.0, 0.15, 0.0)),
            ("chest", Some("spine_lower"), Vec3::new(0.0, 0.3, 0.0)),
            ("clavicle_L", Some("chest"), Vec3::new(-0.05, 0.0, 0.0)),
            ("arm_L", Some("clavicle_L"), Vec3::new(-0.15, 0.0, 0.0)),
            ("forearm_L", Some("arm_L"), Vec3::new(-0.3, 0.0, 0.0)),
            ("hand_L", Some("forearm_L"), Vec3::new(-0.2, 0.0, 0.0)),
            ("clavicle_R", Some("chest"), Vec3::new(0.05, 0.0, 0.0)),
            ("arm_R", Some("clavicle_R"), Vec3::new(0.15, 0.0, 0.0)),
            ("forearm_R", Some("arm_R"), Vec3::new(0.3, 0.0, 0.0)),
            ("hand_R", Some("forearm_R"), Vec3::new(0.2, 0.0, 0.0)),
            ("thigh_L", Some("hip"), Vec3::new(-0.1, -0.4, 0.0)),
            ("thigh_R", Some("hip"), Vec3::new(0.1, -0.4, 0.0)),
        ]);
        let topo = analyze(&b);
        assert_eq!(
            topo.left_arm,
            Some(b.by_name["arm_L"]),
            "left_arm must point to upper arm (shoulder), not forearm"
        );
        assert_eq!(
            topo.right_arm,
            Some(b.by_name["arm_R"]),
            "right_arm must point to upper arm (shoulder), not forearm"
        );
    }

    #[test]
    fn returns_empty_when_no_skeleton() {
        let b = build(vec![("scene", None, Vec3::ZERO)]);
        let topo = analyze(&b);
        assert!(!topo.is_usable());
        assert_eq!(topo.root, None);
    }
}
