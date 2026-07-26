//! enemy_anim.rs — story-636 : animation squelettique des ennemis Roguelite.
//!
//! Les GLB KayKit (`Skeleton_Warrior/Minion/Mage.glb`) sont **déjà rigués** (skin
//! `Rig`, 41 bones) **et contiennent 95 clips bakés** (`Idle`, `Walking_A`,
//! `Running_A`, attaques, `Death_A`…). On les joue via Bevy `AnimationPlayer` /
//! `AnimationGraph`, pilotés par l'état IA (`ArenaBot.state`) + vitesse mesurée.
//!
//! Concept-first étape 0 (data vs code) : le rig + les anims vivent dans la donnée
//! (couche `definition`). On ne **synthétise** rien (le toolbench procédural
//! `forgia-auto-rig`/`forgia-anim-locomotion` est pour des meshes NON animés et est
//! bloqué multi-humanoïde) ; on **branche** le player sur le rig existant.
//!
//! Pattern crate (`mushrooms.rs`/`decor.rs`) : 1 sous-plugin, genome TOML
//! (hot-reload mtime 1Hz), sensor `forgia2_enemy_anim.json` en `GameSet::Sensors`.
//!
//! Robustesse multi-ennemis : chaque `SceneRoot` reçoit SON `AnimationPlayer`
//! auto-spawné par le GltfLoader → pas de limite `.single()`. Le graphe (asset
//! read-only) est **partagé** entre tous les ennemis d'un même GLB.

use crate::enemies::{self, EnemyArchetype};
use bevy::gltf::Gltf;
use bevy::prelude::*;
use forgia_ai_arena_bot::{ArenaBot, BotState};
use forgia_core::prelude::*;
use forgia_player::Player;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::time::{Duration, SystemTime};

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_enemy_anim.toml";
const SENSOR_PATH: &str = "forgia2_enemy_anim.json";
const POLL_PERIOD_SEC: f32 = 1.0;
/// Garde-fou anti-spike : la vitesse mesurée est plafonnée (téléport / 1re frame).
const MAX_SANE_SPEED_MPS: f32 = 30.0;

// ─── TOML schema ──────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct EnemyAnimGenomeToml {
    enabled: Option<bool>,
    idle_clip: Option<String>,
    walk_clip: Option<String>,
    run_clip: Option<String>,
    attack_clip: Option<String>,
    death_clip: Option<String>,
    hit_clip: Option<String>,
    walk_speed_min: Option<f32>,
    run_speed_min: Option<f32>,
    crossfade_ms: Option<u64>,
    rig_visible: Option<bool>,
    mesh_alpha: Option<f32>,
    gizmo_enabled: Option<bool>,
    lod_near_distance_m: Option<f32>,
    lod_far_distance_m: Option<f32>,
    lod_mid_frame_divisor: Option<u32>,
    lod_far_frame_divisor: Option<u32>,
}

// ─── Config (Resource) ──────────────────────────────────────────────────────

/// Config animation ennemis — pilotée par `roguelite_enemy_anim.toml` (hot-reload).
///
/// Note : les **noms de clips** sont lus au BUILD du graphe (pas hot-reload live —
/// changer un nom de clip nécessite un restart). Les **seuils de vitesse**, le
/// **crossfade** et les **toggles debug** sont lus chaque frame → hot-reload live.
#[derive(Resource, Clone, PartialEq, Debug)]
pub struct EnemyAnimConfig {
    pub enabled: bool,
    pub idle_clip: String,
    pub walk_clip: String,
    pub run_clip: String,
    pub attack_clip: String,
    pub death_clip: String,
    pub hit_clip: String,
    pub walk_speed_min: f32,
    pub run_speed_min: f32,
    pub crossfade_ms: u64,
    pub rig_visible: bool,
    pub mesh_alpha: f32,
    pub gizmo_enabled: bool,
    /// Distance où une animation ennemie doit rester à fréquence native.
    pub lod_near_distance_m: f32,
    /// Au-delà, une animation est échantillonnée à cadence réduite.
    pub lod_far_distance_m: f32,
    /// Cadence des ennemis entre near/far (2 = 30 Hz à 60 FPS).
    pub lod_mid_frame_divisor: u32,
    /// Cadence des ennemis au-delà de far (4 = 15 Hz à 60 FPS).
    pub lod_far_frame_divisor: u32,
}

impl Default for EnemyAnimConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            idle_clip: "Idle".to_string(),
            walk_clip: "Walking_A".to_string(),
            run_clip: "Running_A".to_string(),
            attack_clip: "Idle_Combat".to_string(),
            death_clip: "Death_A".to_string(),
            hit_clip: "Hit_A".to_string(),
            walk_speed_min: 0.3,
            run_speed_min: 4.0,
            crossfade_ms: 150,
            rig_visible: true,
            mesh_alpha: 0.45,
            gizmo_enabled: true,
            lod_near_distance_m: 18.0,
            lod_far_distance_m: 38.0,
            lod_mid_frame_divisor: 2,
            lod_far_frame_divisor: 4,
        }
    }
}

impl EnemyAnimConfig {
    /// Pur — testable. Merge le TOML sur les défauts.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: EnemyAnimGenomeToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let d = Self::default();
        Self {
            enabled: parsed.enabled.unwrap_or(d.enabled),
            idle_clip: parsed.idle_clip.unwrap_or(d.idle_clip),
            walk_clip: parsed.walk_clip.unwrap_or(d.walk_clip),
            run_clip: parsed.run_clip.unwrap_or(d.run_clip),
            attack_clip: parsed.attack_clip.unwrap_or(d.attack_clip),
            death_clip: parsed.death_clip.unwrap_or(d.death_clip),
            hit_clip: parsed.hit_clip.unwrap_or(d.hit_clip),
            walk_speed_min: parsed
                .walk_speed_min
                .unwrap_or(d.walk_speed_min)
                .clamp(0.0, 20.0),
            run_speed_min: parsed
                .run_speed_min
                .unwrap_or(d.run_speed_min)
                .clamp(0.1, 30.0),
            crossfade_ms: parsed.crossfade_ms.unwrap_or(d.crossfade_ms).clamp(0, 2000),
            rig_visible: parsed.rig_visible.unwrap_or(d.rig_visible),
            mesh_alpha: parsed.mesh_alpha.unwrap_or(d.mesh_alpha).clamp(0.05, 1.0),
            gizmo_enabled: parsed.gizmo_enabled.unwrap_or(d.gizmo_enabled),
            lod_near_distance_m: parsed
                .lod_near_distance_m
                .unwrap_or(d.lod_near_distance_m)
                .clamp(1.0, 200.0),
            lod_far_distance_m: parsed
                .lod_far_distance_m
                .unwrap_or(d.lod_far_distance_m)
                .clamp(2.0, 300.0),
            lod_mid_frame_divisor: parsed
                .lod_mid_frame_divisor
                .unwrap_or(d.lod_mid_frame_divisor)
                .clamp(1, 4),
            lod_far_frame_divisor: parsed
                .lod_far_frame_divisor
                .unwrap_or(d.lod_far_frame_divisor)
                .clamp(1, 8),
        }
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

/// Suivi mtime du genome + compteurs runtime (capteur).
#[derive(Resource, Default, Debug)]
pub struct EnemyAnimWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
    /// True dès la 1re tentative de build (tous les GLB chargés), même si vide.
    pub graphs_built: bool,
}

// ─── Graph cache (par GLB unique) ───────────────────────────────────────────

/// `Handle<Gltf>` des GLB ennemis uniques (Warrior/Minion/Mage), chargés au Startup.
/// Sert à accéder à `Gltf.named_animations` pour construire les graphes.
#[derive(Resource)]
pub struct EnemyGltfHandles {
    pub handles: Vec<(&'static str, Handle<Gltf>)>,
}

/// Un graphe d'anim + index de nœuds pour un GLB. Cloné par player (asset read-only).
#[derive(Clone)]
pub struct ArchetypeGraph {
    pub graph: Handle<AnimationGraph>,
    pub idle: AnimationNodeIndex,
    pub walk: AnimationNodeIndex,
    pub run: AnimationNodeIndex,
    pub attack: AnimationNodeIndex,
    pub death: AnimationNodeIndex,
}

/// Graphes construits, keyés par chemin GLB. Inséré une fois les GLB chargés.
#[derive(Resource)]
pub struct EnemyAnimGraphs {
    pub by_path: HashMap<&'static str, ArchetypeGraph>,
}

// ─── Composants ─────────────────────────────────────────────────────────────

/// Échantillon de locomotion (vitesse horizontale mesurée) sur le PARENT ennemi.
/// Posé au spawn (`waves.rs`). Lu pour choisir marche vs course.
#[derive(Component, Default, Debug)]
pub struct EnemyLocoSample {
    pub prev_pos: Vec3,
    pub speed: f32,
    pub initialized: bool,
}

/// Marqueur sur l'entité `AnimationPlayer` déjà liée (idempotence du binding).
#[derive(Component)]
pub struct EnemyAnimBound;

/// Lien parent ennemi → son `AnimationPlayer` + le chemin GLB de son graphe.
#[derive(Component)]
pub struct EnemyAnimLink {
    pub player: Entity,
    pub graph_path: &'static str,
}

/// Mémo du nœud d'anim courant sur l'entité `AnimationPlayer` (évite de rejouer).
#[derive(Component)]
pub struct EnemyAnimCurrent {
    pub node: AnimationNodeIndex,
}

/// Compteurs de cadence d'animation pour rendre le gain vérifiable dans le
/// capteur, plutôt que de déclarer l'optimisation sans donnée runtime.
#[derive(Resource, Clone, Default, Debug)]
pub struct EnemyAnimLodMetrics {
    pub near: u32,
    pub mid: u32,
    pub far: u32,
    pub paused: u32,
}

// ─── Sélection de clip (pur, testable) ──────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClipKind {
    Idle,
    Walk,
    Run,
    Attack,
    Death,
}

/// État IA + vitesse → clip désiré. Pur.
/// - `Dead` → death (one-shot). `Attack` → idle de combat (sur place).
/// - `Idle` → idle. `Chase` → idle si bloqué (< walk_min), marche, puis course.
pub fn desired_clip(state: BotState, speed: f32, walk_min: f32, run_min: f32) -> ClipKind {
    match state {
        BotState::Dead => ClipKind::Death,
        BotState::Attack => ClipKind::Attack,
        BotState::Idle => ClipKind::Idle,
        BotState::Chase => {
            if speed < walk_min {
                ClipKind::Idle
            } else if speed < run_min {
                ClipKind::Walk
            } else {
                ClipKind::Run
            }
        }
    }
}

fn node_for(ag: &ArchetypeGraph, kind: ClipKind) -> AnimationNodeIndex {
    match kind {
        ClipKind::Idle => ag.idle,
        ClipKind::Walk => ag.walk,
        ClipKind::Run => ag.run,
        ClipKind::Attack => ag.attack,
        ClipKind::Death => ag.death,
    }
}

/// Politique d'animation LOD. Les actions/deaths restent natives : elles sont
/// des feedbacks de combat lisibles. Les ennemis éloignés conservent le même
/// temps d'animation mais leurs os ne sont évalués qu'à une cadence réduite,
/// déphasée par entité pour éviter un burst synchronisé de rigs.
pub fn animation_lod_divisor(distance_m: f32, state: BotState, cfg: &EnemyAnimConfig) -> u32 {
    if matches!(state, BotState::Attack | BotState::Dead) || distance_m <= cfg.lod_near_distance_m {
        1
    } else if distance_m <= cfg.lod_far_distance_m.max(cfg.lod_near_distance_m) {
        cfg.lod_mid_frame_divisor
    } else {
        cfg.lod_far_frame_divisor
    }
}

// ─── Systèmes ───────────────────────────────────────────────────────────────

pub fn sys_init_enemy_anim_genome(mut commands: Commands) {
    let cfg = EnemyAnimConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(cfg);
    commands.insert_resource(EnemyAnimWatch {
        last_mtime: mtime,
        ..Default::default()
    });
}

/// Charge les `Handle<Gltf>` des GLB ennemis uniques (dédup par chemin).
pub fn sys_load_enemy_gltf(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut handles: Vec<(&'static str, Handle<Gltf>)> = Vec::new();
    for arch in [
        EnemyArchetype::Tank,
        EnemyArchetype::Runner,
        EnemyArchetype::Sniper,
        EnemyArchetype::Boss,
    ] {
        let path = enemies::skeleton_asset_path(arch);
        if handles.iter().all(|(p, _)| *p != path) {
            handles.push((path, asset_server.load(path)));
        }
    }
    commands.insert_resource(EnemyGltfHandles { handles });
}

/// Construit les graphes d'anim une fois tous les GLB chargés (one-shot).
pub fn sys_build_enemy_anim_graphs(
    mut commands: Commands,
    existing: Option<Res<EnemyAnimGraphs>>,
    handles: Option<Res<EnemyGltfHandles>>,
    cfg: Res<EnemyAnimConfig>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut watch: ResMut<EnemyAnimWatch>,
) {
    if existing.is_some() {
        return;
    }
    let Some(handles) = handles else {
        return;
    };
    // Attendre que TOUS les GLB soient chargés (sinon retry next frame).
    if !handles.handles.iter().all(|(_, h)| gltfs.get(h).is_some()) {
        return;
    }
    let mut by_path: HashMap<&'static str, ArchetypeGraph> = HashMap::new();
    for (path, h) in &handles.handles {
        let Some(gltf) = gltfs.get(h) else { continue };
        let pick = |name: &str| gltf.named_animations.get(name).cloned();
        let (Some(idle), Some(walk), Some(run), Some(attack), Some(death)) = (
            pick(&cfg.idle_clip),
            pick(&cfg.walk_clip),
            pick(&cfg.run_clip),
            pick(&cfg.attack_clip),
            pick(&cfg.death_clip),
        ) else {
            warn!("[enemy_anim] clips manquants dans {path} — vérifier roguelite_enemy_anim.toml");
            continue;
        };
        let mut g = AnimationGraph::new();
        let root = g.root;
        let ag = ArchetypeGraph {
            idle: g.add_clip(idle, 1.0, root),
            walk: g.add_clip(walk, 1.0, root),
            run: g.add_clip(run, 1.0, root),
            attack: g.add_clip(attack, 1.0, root),
            death: g.add_clip(death, 1.0, root),
            graph: graphs.add(g),
        };
        by_path.insert(*path, ag);
    }
    let n = by_path.len();
    watch.graphs_built = true;
    commands.insert_resource(EnemyAnimGraphs { by_path });
    info!("[enemy_anim] graphes construits — {n} GLB");
}

pub fn sys_hot_reload_enemy_anim_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<EnemyAnimConfig>,
    mut watch: ResMut<EnemyAnimWatch>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let Ok(mtime) = fs::metadata(GENOME_PATH).and_then(|m| m.modified()) else {
        return;
    };
    if watch.last_mtime == Some(mtime) {
        return;
    }
    watch.last_mtime = Some(mtime);
    let Ok(content) = fs::read_to_string(GENOME_PATH) else {
        return;
    };
    let new_cfg = EnemyAnimConfig::parse_toml(&content);
    if new_cfg != *cfg {
        *cfg = new_cfg;
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!("[enemy_anim] HOT-RELOADED");
    }
}

/// Mesure la vitesse horizontale par delta de Transform (lag 1 frame toléré).
pub fn sys_sample_enemy_speed(time: Res<Time>, mut q: Query<(&Transform, &mut EnemyLocoSample)>) {
    let dt = time.delta_secs().max(1e-5);
    for (xf, mut s) in &mut q {
        let p = xf.translation;
        if !s.initialized {
            s.prev_pos = p;
            s.speed = 0.0;
            s.initialized = true;
            continue;
        }
        let d = p - s.prev_pos;
        let horiz = Vec2::new(d.x, d.z).length();
        s.speed = (horiz / dt).min(MAX_SANE_SPEED_MPS);
        s.prev_pos = p;
    }
}

/// Lie l'`AnimationPlayer` auto-spawné de chaque ennemi à son graphe.
/// `Without<EnemyAnimBound>` (pas `Added<>`) → robuste à l'ordre de chargement.
pub fn sys_wire_enemy_anim(
    mut commands: Commands,
    cfg: Res<EnemyAnimConfig>,
    graphs: Option<Res<EnemyAnimGraphs>>,
    mut new_players: Query<(Entity, &mut AnimationPlayer), Without<EnemyAnimBound>>,
    child_of_q: Query<&ChildOf>,
    arch_q: Query<&EnemyArchetype>,
) {
    if !cfg.enabled {
        return;
    }
    let Some(graphs) = graphs else {
        return;
    };
    for (player_e, mut player) in &mut new_players {
        // Remonter la hiérarchie jusqu'au parent portant EnemyArchetype.
        let mut cur = player_e;
        let mut found: Option<(Entity, EnemyArchetype)> = None;
        for _ in 0..8 {
            if let Ok(arch) = arch_q.get(cur) {
                found = Some((cur, *arch));
                break;
            }
            match child_of_q.get(cur) {
                Ok(co) => cur = co.parent(),
                Err(_) => break,
            }
        }
        let Some((enemy_root, arch)) = found else {
            continue; // pas un AnimationPlayer d'ennemi (re-vérifié next frame)
        };
        let path = enemies::skeleton_asset_path(arch);
        let Some(ag) = graphs.by_path.get(path) else {
            continue; // graphe absent pour ce GLB (clips manquants) — sensor le dira
        };
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut player, ag.idle, Duration::ZERO)
            .repeat();
        commands.entity(player_e).insert((
            AnimationGraphHandle(ag.graph.clone()),
            transitions,
            EnemyAnimBound,
            EnemyAnimCurrent { node: ag.idle },
        ));
        commands.entity(enemy_root).insert(EnemyAnimLink {
            player: player_e,
            graph_path: path,
        });
    }
}

/// Choisit le clip selon l'état IA + vitesse, crossfade si le nœud change.
pub fn sys_update_enemy_anim(
    cfg: Res<EnemyAnimConfig>,
    graphs: Option<Res<EnemyAnimGraphs>>,
    enemies_q: Query<(&ArenaBot, &EnemyLocoSample, &EnemyAnimLink)>,
    mut players: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut EnemyAnimCurrent,
    )>,
) {
    if !cfg.enabled {
        return;
    }
    let Some(graphs) = graphs else {
        return;
    };
    let cross = Duration::from_millis(cfg.crossfade_ms);
    for (bot, loco, link) in &enemies_q {
        let Some(ag) = graphs.by_path.get(link.graph_path) else {
            continue;
        };
        let kind = desired_clip(bot.state, loco.speed, cfg.walk_speed_min, cfg.run_speed_min);
        let node = node_for(ag, kind);
        let Ok((mut player, mut transitions, mut current)) = players.get_mut(link.player) else {
            continue;
        };
        if current.node == node {
            continue;
        }
        let active = transitions.play(&mut player, node, cross);
        if kind != ClipKind::Death {
            active.repeat();
        }
        current.node = node;
    }
}

/// Réduit la pression de `propagate_parent_transforms` confirmée par Tracy :
/// chaque rig KayKit modifie ~41 os, donc tous les mettre à jour dans la même
/// frame crée des pics. Cette passe s'exécute avant l'animation PostUpdate de
/// Bevy ; les joueurs éloignés sont pausés la plupart des frames puis rattrapent
/// le temps écoulé lors de leur frame attribuée.
pub fn sys_apply_enemy_animation_lod(
    time: Res<Time>,
    cfg: Res<EnemyAnimConfig>,
    player: Query<&GlobalTransform, With<Player>>,
    enemies: Query<(Entity, &GlobalTransform, &ArenaBot, &EnemyAnimLink)>,
    mut players: Query<&mut AnimationPlayer>,
    mut metrics: ResMut<EnemyAnimLodMetrics>,
    mut frame: Local<u64>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    *frame = frame.wrapping_add(1);
    *metrics = EnemyAnimLodMetrics::default();
    let player_pos = player_transform.translation();
    for (entity, enemy_transform, bot, link) in &enemies {
        let distance = enemy_transform.translation().distance(player_pos);
        let divisor = animation_lod_divisor(distance, bot.state, &cfg);
        match divisor {
            1 => metrics.near = metrics.near.saturating_add(1),
            2 => metrics.mid = metrics.mid.saturating_add(1),
            _ => metrics.far = metrics.far.saturating_add(1),
        }
        let Ok(mut animation_player) = players.get_mut(link.player) else {
            continue;
        };
        // Déphasage par Entity index : pour divisor=4, environ un quart des
        // rigs éloignés réévaluent leurs bones à chaque frame au lieu de tous.
        let update_this_frame = divisor == 1
            || (*frame + u64::from(entity.index().index())).is_multiple_of(u64::from(divisor));
        if update_this_frame {
            if divisor > 1 {
                animation_player.seek_all_by(time.delta_secs() * (divisor - 1) as f32);
            }
            animation_player.resume_all();
        } else {
            animation_player.pause_all();
            metrics.paused = metrics.paused.saturating_add(1);
        }
    }
}

// ─── Sensor ─────────────────────────────────────────────────────────────────

/// Pur — severity/next_step du capteur.
pub fn severity_for_enemy_anim(
    enabled: bool,
    graphs_built: bool,
    enemies: u32,
    bound: u32,
) -> (&'static str, &'static str) {
    if !enabled {
        return ("info", "Animations ennemies coupées (enabled=false).");
    }
    if !graphs_built {
        return (
            "info",
            "Graphes d'anim en cours de build (GLB en chargement).",
        );
    }
    if enemies > 0 && bound == 0 {
        return (
            "warn",
            "Ennemis présents mais 0 AnimationPlayer lié — vérifier binding (ChildOf→EnemyArchetype / clips).",
        );
    }
    ("ok", "Animations ennemies actives.")
}

pub fn sys_write_enemy_anim_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    cfg: Option<Res<EnemyAnimConfig>>,
    watch: Option<Res<EnemyAnimWatch>>,
    q_enemies: Query<(), With<EnemyArchetype>>,
    q_bound: Query<(), With<EnemyAnimBound>>,
    lod: Option<Res<EnemyAnimLodMetrics>>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let (Some(cfg), Some(watch)) = (cfg, watch) else {
        return;
    };
    let enemies = q_enemies.iter().count() as u32;
    let bound = q_bound.iter().count() as u32;
    let (severity, next_step) =
        severity_for_enemy_anim(cfg.enabled, watch.graphs_built, enemies, bound);
    let lod = lod.as_deref().cloned().unwrap_or_default();
    let json = format!(
        r#"{{"id":"enemy_anim","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"enabled":{},"graphs_built":{},"enemies_total":{},"players_bound":{},"rig_visible":{},"gizmo_enabled":{},"crossfade_ms":{},"lod_near":{},"lod_mid":{},"lod_far":{},"lod_paused":{},"reload_count":{}}}"#,
        time.elapsed_secs(),
        cfg.enabled,
        watch.graphs_built,
        enemies,
        bound,
        cfg.rig_visible,
        cfg.gizmo_enabled,
        cfg.crossfade_ms,
        lod.near,
        lod.mid,
        lod.far,
        lod.paused,
        watch.reload_count,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[enemy_anim] sensor write failed: {e}");
    }
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

pub struct ForgiaRogueliteEnemyAnimPlugin;

impl Plugin for ForgiaRogueliteEnemyAnimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::enemy_rig_debug::EnemyMatRegistry>();
        app.init_resource::<EnemyAnimLodMetrics>();
        app.add_systems(Startup, (sys_init_enemy_anim_genome, sys_load_enemy_gltf));
        // Driver d'anim : hot-reload → build graphes → bind → select clip.
        app.add_systems(
            Update,
            (
                sys_hot_reload_enemy_anim_genome,
                sys_build_enemy_anim_graphs,
                sys_wire_enemy_anim,
                sys_update_enemy_anim,
                sys_apply_enemy_animation_lod,
            )
                .chain()
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Roguelite)),
        );
        // BUG-01 (qa story-636) — vitesse mesurée en PostUpdate : lit le Transform
        // APRÈS que l'IA bot (plain Update, forgia-ai-arena-bot) ait commité son
        // déplacement → vitesse déterministe et exacte (l'ordre cross-crate en
        // Update est indéfini). Rapier est en FixedUpdate (story-634) donc PostUpdate
        // ne re-bouge pas le Transform. `sys_update_enemy_anim` (Update) lit la
        // vitesse du PostUpdate précédent (lag 1 frame imperceptible pour l'anim).
        app.add_systems(
            PostUpdate,
            sys_sample_enemy_speed.run_if(in_state(GameMode::Roguelite)),
        );
        // BUG-04 (qa story-636) — purge les matériaux clonés entre runs.
        app.add_systems(
            OnExit(GameMode::Roguelite),
            crate::enemy_rig_debug::sys_clear_enemy_mat_registry,
        );
        // Viz de contrôle du rig (transparence + gizmos) — module enemy_rig_debug.
        app.add_systems(
            Update,
            (
                crate::enemy_rig_debug::sys_apply_rig_debug,
                crate::enemy_rig_debug::sys_draw_enemy_bones,
            )
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            sys_write_enemy_anim_sensor
                .in_set(GameSet::Sensors)
                .run_if(in_state(GameMode::Roguelite)),
        );
    }
}

// ─── Tests purs ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_default() {
        assert_eq!(EnemyAnimConfig::parse_toml(""), EnemyAnimConfig::default());
    }

    #[test]
    fn parse_overrides_clips_and_thresholds() {
        let toml = r#"
enabled = false
walk_clip = "Walking_C"
run_speed_min = 6.0
mesh_alpha = 0.3
"#;
        let c = EnemyAnimConfig::parse_toml(toml);
        assert!(!c.enabled);
        assert_eq!(c.walk_clip, "Walking_C");
        assert!((c.run_speed_min - 6.0).abs() < 1e-6);
        assert!((c.mesh_alpha - 0.3).abs() < 1e-6);
        // non précisés → défauts
        assert_eq!(c.idle_clip, "Idle");
    }

    #[test]
    fn parse_clamps_alpha_and_crossfade() {
        let c = EnemyAnimConfig::parse_toml("mesh_alpha = 9.0\ncrossfade_ms = 99999");
        assert!(c.mesh_alpha <= 1.0);
        assert!(c.crossfade_ms <= 2000);
    }

    #[test]
    fn desired_clip_states() {
        // Idle / Attack / Dead indépendants de la vitesse.
        assert_eq!(desired_clip(BotState::Idle, 5.0, 0.3, 4.0), ClipKind::Idle);
        assert_eq!(
            desired_clip(BotState::Attack, 5.0, 0.3, 4.0),
            ClipKind::Attack
        );
        assert_eq!(desired_clip(BotState::Dead, 5.0, 0.3, 4.0), ClipKind::Death);
    }

    #[test]
    fn desired_clip_chase_speed_bands() {
        // Chase bloqué → idle ; marche ; course.
        assert_eq!(desired_clip(BotState::Chase, 0.1, 0.3, 4.0), ClipKind::Idle);
        assert_eq!(desired_clip(BotState::Chase, 2.0, 0.3, 4.0), ClipKind::Walk);
        assert_eq!(desired_clip(BotState::Chase, 7.0, 0.3, 4.0), ClipKind::Run);
    }

    #[test]
    fn animation_lod_keeps_combat_native_and_staggers_distance_bands() {
        let cfg = EnemyAnimConfig::default();
        assert_eq!(animation_lod_divisor(5.0, BotState::Chase, &cfg), 1);
        assert_eq!(animation_lod_divisor(25.0, BotState::Chase, &cfg), 2);
        assert_eq!(animation_lod_divisor(60.0, BotState::Chase, &cfg), 4);
        assert_eq!(animation_lod_divisor(60.0, BotState::Attack, &cfg), 1);
        assert_eq!(animation_lod_divisor(60.0, BotState::Dead, &cfg), 1);
    }

    #[test]
    fn severity_paths() {
        assert_eq!(severity_for_enemy_anim(false, true, 5, 0).0, "info");
        assert_eq!(severity_for_enemy_anim(true, false, 5, 0).0, "info");
        assert_eq!(severity_for_enemy_anim(true, true, 5, 0).0, "warn");
        assert_eq!(severity_for_enemy_anim(true, true, 5, 5).0, "ok");
        // pas d'ennemi → ok (transitoire entre vagues)
        assert_eq!(severity_for_enemy_anim(true, true, 0, 0).0, "ok");
    }
}
