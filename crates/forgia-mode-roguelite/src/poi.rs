//! poi.rs — Story-561 : Points d'Intérêt (POI) gameplay.
//!
//! Couche gameplay greffée **par-dessus** les anchors de layout produits par
//! `forgia-stage` (`AnchorKind::PoiSlot`, positions = solveur sight-line).
//! Ce module ne touche PAS `forgia-stage` : il réagit à `Added<AnchorPoint>`
//! et spawne ses propres entités gameplay AU POINT de l'anchor.
//!
//! ## 3 types de POI (data-driven `roguelite_poi_gameplay.toml`)
//!
//! - **Loot Vault** : coffre brillant, walk-over (rayon) → +Souls 1×/run, dim.
//! - **Lava Hazard** : disque orange, dégât périodique au contact (joueur ET
//!   ennemis). Pousser un ennemi dedans (boon knockback) = kill environnemental.
//! - **Forge** (slot 0) : présence physique emissive du Coffre du Forgeron.
//!
//! ## Lifecycle (zéro leak inter-stage)
//!
//! Chaque entité POI porte `forgia_stage::StageArenaMarker` → elle est nettoyée
//! par `forgia_stage::despawn_stage_entities` à CHAQUE transition de stage ET
//! `OnExit(GameMode::Roguelite)`. `Added<AnchorPoint>` re-tire pour les nouveaux
//! anchors du stage suivant → re-spawn propre. Pattern anti-leak
//! (cf `reference_roguelite_bot_leak_run_restart`).
//!
//! ## Piège des 2 `Health`
//!
//! - Ennemis (`ArenaBot`) portent `forgia_combat::Health` → mutation directe +
//!   `CombatHitEvent`, mort déléguée à `forgia-fps::despawn_dead_cubes`.
//! - Joueur porte `forgia_damage::Health` → `DamageEvent{kind:Fire}` (route
//!   `HealthGuard` → `DeathEvent` → `obs_roguelite_player_death` → Defeat).
//!   JAMAIS muter directement (sinon pas de DeathEvent → pas de Defeat).

use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_rapier3d::prelude::{Collider, RigidBody, Sensor};
use forgia_ai_arena_bot::ArenaBot;
use forgia_anchor::{AnchorKind, AnchorPoint};
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_combat::Health as CombatHealth;
use forgia_core::prelude::*;
use forgia_damage::{DamageEvent, DamageKind, HitZone};
use forgia_player::Player;
use forgia_rpg_data::loot_tables::Souls;
use forgia_stage::StageArenaMarker;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::run::{RunSeed, StartRunEvent};

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_poi_gameplay.toml";
const SENSOR_PATH: &str = "forgia2_stage_poi.json";
const POLL_PERIOD_SEC: f32 = 1.0;
/// Cap PointLight pour ne pas casser Atmosphere/bloom (dette 2026-05-18,
/// cf `reference_pointlight_intensity_8k_atmosphere_cap`).
const ATMOSPHERE_LUMEN_CAP: f32 = 8_000.0;

// ─── Genome / Config (definition layer) ──────────────────────────────────────

#[derive(Deserialize)]
struct GeneToml {
    id: String,
    #[serde(default)]
    default: f32,
}

#[derive(Deserialize)]
struct PoiGenomeToml {
    #[serde(default)]
    genes: Vec<GeneToml>,
}

/// Config gameplay POI, consommée par les systèmes spawn/loot/lava.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct RoguelitePoiConfig {
    pub vault_weight: f32,
    pub lava_weight: f32,
    pub vault_souls_reward: u32,
    pub loot_trigger_radius: f32,
    pub lava_radius: f32,
    pub lava_damage_per_tick: f32,
    pub lava_tick_period: f32,
    pub forge_enabled: bool,
    /// Hauteur du faisceau lumineux vertical (loot beam) au-dessus des coffres
    /// + forge. Lisibilité "vu de partout" anti-camouflage biome. 0 = aucun.
    pub beacon_height: f32,
}

impl Default for RoguelitePoiConfig {
    fn default() -> Self {
        Self {
            vault_weight: 3.0,
            lava_weight: 2.0,
            vault_souls_reward: 50,
            loot_trigger_radius: 3.0,
            lava_radius: 5.0,
            lava_damage_per_tick: 8.0,
            lava_tick_period: 0.5,
            forge_enabled: true,
            beacon_height: 7.0,
        }
    }
}

impl RoguelitePoiConfig {
    /// Pur — testable headless.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: PoiGenomeToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut c = Self::default();
        for gene in &parsed.genes {
            match gene.id.as_str() {
                "poi_vault_weight" => c.vault_weight = gene.default.clamp(0.0, 100.0),
                "poi_lava_weight" => c.lava_weight = gene.default.clamp(0.0, 100.0),
                "poi_vault_souls_reward" => {
                    c.vault_souls_reward = gene.default.clamp(0.0, 100_000.0) as u32
                }
                "poi_loot_trigger_radius" => c.loot_trigger_radius = gene.default.clamp(0.5, 20.0),
                "poi_lava_radius" => c.lava_radius = gene.default.clamp(0.5, 30.0),
                "poi_lava_damage_per_tick" => {
                    c.lava_damage_per_tick = gene.default.clamp(0.0, 1000.0)
                }
                "poi_lava_tick_period" => c.lava_tick_period = gene.default.clamp(0.05, 5.0),
                "poi_forge_enabled" => c.forge_enabled = gene.default >= 0.5,
                "poi_beacon_height" => c.beacon_height = gene.default.clamp(0.0, 30.0),
                _ => {}
            }
        }
        c
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(PathBuf::from(GENOME_PATH)) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

/// Watch mtime du TOML pour hot-reload.
#[derive(Resource, Default, Debug)]
pub struct PoiGenomeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

// ─── Stats cumulatives (sensor) ──────────────────────────────────────────────

/// Compteurs cumulatifs sur la run courante (reset `OnEnter(Roguelite)`).
/// Les COUNTS de POI vivants sont requêtés en direct par le sensor (reflètent
/// le stage courant), pas stockés ici.
#[derive(Resource, Default, Debug)]
pub struct PoiStats {
    pub vaults_looted: u32,
    pub souls_from_vaults: u32,
    pub lava_kills_total: u32,
    pub lava_player_ticks: u32,
}

// ─── Components gameplay ──────────────────────────────────────────────────────

#[derive(Component, Debug, Clone, Copy)]
pub struct LootVault {
    pub looted: bool,
    pub reward: u32,
    pub radius: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct LavaHazard {
    pub radius: f32,
    pub dmg_per_tick: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct PoiForge;

/// Marqueur posé sur un anchor (entité persistante de `forgia-stage`) une fois
/// son POI gameplay spawné. Pilote la **réconciliation** : tout anchor PoiSlot
/// `Without<PoiAssigned>` reçoit un POI. Robuste aux re-spawns d'arène (vs
/// `Added<AnchorPoint>` qui ratait les anchors recréés à chaque transition).
#[derive(Component, Debug, Clone, Copy)]
pub struct PoiAssigned;

// ─── Assignation type → anchor (pure, déterministe) ──────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoiKind {
    LootVault,
    LavaHazard,
    Forge,
}

/// Salt de mixage seed×slot (constante de Knuth, distribution uniforme des bits).
const SLOT_SALT: u64 = 0x9E37_79B9_7F4A_7C15;

/// Assigne déterministiquement un `PoiKind` au slot `slot_index`.
/// - Slot 0 = Forge garantie (si `forge_enabled`).
/// - Autres slots = tirage pondéré vault/lava, seed-stable (même seed → même map).
///
/// Pur — testable headless.
pub fn assign_poi_kind(slot_index: u32, run_seed: u64, cfg: &RoguelitePoiConfig) -> PoiKind {
    if cfg.forge_enabled && slot_index == 0 {
        return PoiKind::Forge;
    }
    let total = cfg.vault_weight + cfg.lava_weight;
    if total <= 0.0 {
        return PoiKind::LootVault;
    }
    let mix = run_seed ^ u64::from(slot_index).wrapping_mul(SLOT_SALT);
    let mut rng = Xoshiro256StarStar::seed_from_u64(mix);
    let r = (rng.next_u32() as f32 / u32::MAX as f32) * total;
    if r < cfg.vault_weight {
        PoiKind::LootVault
    } else {
        PoiKind::LavaHazard
    }
}

/// Distance horizontale (XZ) au carré ≤ rayon² — pur, sans sqrt ni alloc.
#[inline]
fn within_radius_xz(a: Vec3, b: Vec3, radius: f32) -> bool {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz <= radius * radius
}

// ─── Systems : genome load + hot-reload ──────────────────────────────────────

/// Startup : charge le TOML + initialise le watch mtime.
pub fn sys_init_poi_genome(mut commands: Commands) {
    let cfg = RoguelitePoiConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(cfg);
    commands.insert_resource(PoiGenomeWatch {
        last_mtime: mtime,
        ..default()
    });
    info!(
        "[poi] genome loaded — vault_w={:.1} lava_w={:.1} reward={} lava_r={:.1} dmg/tick={:.1} forge={}",
        cfg.vault_weight,
        cfg.lava_weight,
        cfg.vault_souls_reward,
        cfg.lava_radius,
        cfg.lava_damage_per_tick,
        cfg.forge_enabled
    );
}

/// Poll mtime 1Hz, re-parse si changé (hot-reload Shift+F12-like).
pub fn sys_hot_reload_poi_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<RoguelitePoiConfig>,
    mut watch: ResMut<PoiGenomeWatch>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;

    let Ok(meta) = fs::metadata(GENOME_PATH) else {
        return;
    };
    let Ok(mtime) = meta.modified() else {
        return;
    };
    if watch.last_mtime == Some(mtime) {
        return;
    }
    let Ok(content) = fs::read_to_string(GENOME_PATH) else {
        return;
    };
    let new_cfg = RoguelitePoiConfig::parse_toml(&content);
    watch.last_mtime = Some(mtime);
    if new_cfg != *cfg {
        *cfg = new_cfg;
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!(
            "[poi] HOT-RELOADED — vault_w={:.1} lava_w={:.1} reward={} lava_r={:.1} dmg/tick={:.1}",
            new_cfg.vault_weight,
            new_cfg.lava_weight,
            new_cfg.vault_souls_reward,
            new_cfg.lava_radius,
            new_cfg.lava_damage_per_tick
        );
    }
}

/// Reset stats cumulatives au début de run.
pub fn sys_reset_poi_stats(mut stats: ResMut<PoiStats>) {
    *stats = PoiStats::default();
}

/// Régénère TOUS les POI au début de chaque run — y compris les coffres lootés
/// (despawnés par `sys_loot_vault_walkover`) qui sinon ne reviennent jamais.
///
/// **Pourquoi `StartRunEvent` et pas `OnEnter(Roguelite)`** : un retry *en place*
/// (bouton REFORGER sur Défaite `hud.rs`, Nouvelle Run sur Victoire) **ne quitte
/// pas** `GameMode::Roguelite` → `OnEnter` ne tire pas, `forgia-stage` ne
/// reconstruit pas l'arène (même `stage_id` → early-return), les anchors
/// persistent avec `PoiAssigned` collant → coffre looté jamais re-spawné.
/// `StartRunEvent` est LE signal commun à tous les départs de run (entrée +
/// retries in-place + Nouvelle Run). Bug observé 2026-06-03.
///
/// **Anti-double-spawn (même frame)** : on despawn TOUS les POI vivants AVANT de
/// retirer `PoiAssigned`. Les deux passent par `Commands` (deferred). Grâce à
/// l'edge d'ordre `sys_reconcile_poi_anchors.after(this)` et au
/// `AutoInsertApplyDeferredPass` de Bevy 0.18 (un `ApplyDeferred` est inséré sur
/// l'edge car ce système porte `Commands`), le despawn + `remove::<PoiAssigned>`
/// sont **flushés avant** que la réconciliation ne tourne, le frame même →
/// reconcile part d'ardoise vierge, exactement 1 POI/anchor, jamais 2.
/// L'ordre `.after(run::sys_start_run)` garantit que le `RunSeed` fraîchement
/// inséré (deferred) est visible avant le re-tirage des types POI.
pub fn sys_clear_poi_on_run_start(
    mut commands: Commands,
    mut ev: MessageReader<StartRunEvent>,
    q_pois: Query<Entity, Or<(With<LootVault>, With<LavaHazard>, With<PoiForge>)>>,
    q_assigned: Query<Entity, With<PoiAssigned>>,
) {
    if ev.is_empty() {
        return;
    }
    ev.clear();

    let mut despawned = 0u32;
    for e in &q_pois {
        commands.entity(e).despawn();
        despawned += 1;
    }
    let mut cleared = 0u32;
    for e in &q_assigned {
        commands.entity(e).remove::<PoiAssigned>();
        cleared += 1;
    }
    // Observabilité : seul moyen runtime-falsifiable que le chemin in-place a
    // tourné (le sensor `loot_vaults_unlooted` ne distingue pas in-place d'un
    // re-enter déjà fonctionnel sans le fix).
    if despawned > 0 || cleared > 0 {
        info!(
            "[poi] run-start reset — despawn {despawned} POI + clear {cleared} anchors → régénération"
        );
    }
}

// ─── System : assignation POI sur anchors (réaction à Added) ─────────────────

/// Réconcilie le gameplay POI sur les anchors de `forgia-stage`. Tout anchor
/// `Without<PoiAssigned>` est traité une fois (marqué), et si c'est un `PoiSlot`
/// il reçoit un POI gameplay. Tirage déterministe via `RunSeed`.
///
/// **Pourquoi pas `Added<AnchorPoint>`** : `forgia-stage` re-spawne l'arène à
/// chaque transition de stage (`despawn_stage_entities` + nouveaux anchors).
/// `Added` ne tire qu'une fois et rate les anchors recréés → POI=0 malgré
/// anchors vivants (bug observé 2026-06-03, sensor `poi_slot:6` / `poi_total:0`).
/// La réconciliation par marqueur garantit l'invariant à chaque frame.
/// Les POI sont tagués `StageArenaMarker` → cleanup auto par `forgia-stage`.
#[allow(clippy::too_many_arguments)]
pub fn sys_reconcile_poi_anchors(
    mut commands: Commands,
    q_unassigned: Query<(Entity, &Transform, &AnchorPoint), Without<PoiAssigned>>,
    cfg: Option<Res<RoguelitePoiConfig>>,
    run_seed: Option<Res<RunSeed>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(cfg) = cfg else {
        return;
    };
    let seed = run_seed.map(|s| s.seed).unwrap_or(0xF05A_C0DE);

    for (anchor_e, xf, anchor) in &q_unassigned {
        // Marque tout anchor traité (POI ou non) → plus jamais re-scanné.
        commands.entity(anchor_e).insert(PoiAssigned);
        if anchor.kind != AnchorKind::PoiSlot {
            continue;
        }
        let pos = xf.translation;
        let kind = assign_poi_kind(anchor.slot_index, seed, &cfg);
        match kind {
            PoiKind::LootVault => {
                spawn_loot_vault(&mut commands, &mut meshes, &mut materials, pos, &cfg)
            }
            PoiKind::LavaHazard => {
                spawn_lava_hazard(&mut commands, &mut meshes, &mut materials, pos, &cfg)
            }
            PoiKind::Forge => spawn_forge(&mut commands, &mut meshes, &mut materials, pos, &cfg),
        }
        // Observabilité spawn-timing (BUG-561-05) : confirme le placement in-game
        // sans relancer. Croiser avec forgia2_stage_poi.json::poi_total.
        info!(
            "[poi] {:?} spawned @ slot {} ({:.0}, {:.0})",
            kind, anchor.slot_index, pos.x, pos.z
        );
    }
}

/// Bundle commun : marqueurs de cleanup + nom. Lifecycle = stage (StageArenaMarker
/// → cleanup par forgia-stage à chaque transition + OnExit) ; PAS RogueliteRunMarker
/// (lifecycle = run, despawnerait les POI en cours de run si un cleanup run-marker
/// était ajouté). DespawnOnExit = filet de sécurité sortie Roguelite.
fn poi_markers(name: impl Into<String>) -> (Name, StageArenaMarker, DespawnOnExit<GameMode>) {
    (
        Name::new(name.into()),
        StageArenaMarker,
        DespawnOnExit(GameMode::Roguelite),
    )
}

/// Faisceau vertical translucide emissive (loot beam) — lisibilité "vu de
/// partout" anti-camouflage biome. Spawné en enfant d'un POI. `base_y` = sommet
/// du corps du POI (le faisceau part de là vers le haut).
fn beacon_bundle(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    height: f32,
    base_y: f32,
    base: Color,
    emissive: LinearRgba,
) -> impl Bundle {
    let mesh = meshes.add(Cylinder::new(0.22, height));
    let mat = materials.add(StandardMaterial {
        base_color: base,
        emissive,
        unlit: true, // ignore l'éclairage scène → reste vif malgré le toon
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    (
        Name::new("PoiBeacon"),
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_xyz(0.0, base_y + height * 0.5, 0.0),
    )
}

fn spawn_loot_vault(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    cfg: &RoguelitePoiConfig,
) {
    let mesh = meshes.add(Cuboid::new(1.4, 1.0, 1.0));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.72, 0.20), // FORGE_OR
        emissive: LinearRgba::new(2.2, 1.6, 0.40, 1.0),
        metallic: 0.6,
        perceptual_roughness: 0.4,
        ..default()
    });
    // Balise CYAN précalculée (None si désactivée) — cyan = contraste max contre
    // le biome chaud orange + lecture "loot" universelle (Apex/Fortnite ping).
    let beacon = (cfg.beacon_height > 0.0).then(|| {
        beacon_bundle(
            meshes,
            materials,
            cfg.beacon_height,
            1.05,
            Color::srgba(0.55, 0.95, 1.0, 0.55),
            LinearRgba::new(0.8, 4.5, 6.0, 1.0),
        )
    });
    commands
        .spawn((
            poi_markers("RoguelitePoi_LootVault"),
            LootVault {
                looted: false,
                reward: cfg.vault_souls_reward,
                radius: cfg.loot_trigger_radius,
            },
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(pos.with_y(0.5)),
            // Sensor : présence physique + détection AI, sans bloquer le joueur.
            RigidBody::Fixed,
            Collider::cuboid(0.7, 0.5, 0.5),
            Sensor,
        ))
        .with_children(|p| {
            if let Some(b) = beacon {
                p.spawn(b);
            }
        });
}

fn spawn_lava_hazard(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    cfg: &RoguelitePoiConfig,
) {
    // Disque dont le RAYON = exactement la hitbox (Perfect Information).
    // Cœur rouge molten TRÈS vif → tranche sur le sable orange (lisibilité).
    let mesh = meshes.add(Cylinder::new(cfg.lava_radius, 0.12));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.10, 0.02),
        emissive: LinearRgba::new(5.0, 0.85, 0.06, 1.0),
        metallic: 0.0,
        perceptual_roughness: 0.6,
        ..default()
    });
    // Bord charbon sombre, légèrement plus large → "ici ça brûle" visible AVANT
    // de marcher dedans (AC3 bord clairement marqué, anti-frustration).
    let rim_mesh = meshes.add(Cylinder::new(cfg.lava_radius + 0.6, 0.08));
    let rim_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.05, 0.04),
        metallic: 0.0,
        perceptual_roughness: 0.95,
        ..default()
    });
    commands
        .spawn((
            poi_markers("RoguelitePoi_LavaHazard"),
            LavaHazard {
                radius: cfg.lava_radius,
                dmg_per_tick: cfg.lava_damage_per_tick,
            },
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(pos.with_y(0.06)),
            // Sensor : awareness AI + cohérence physique. Le dégât est géré par
            // sys_lava_tick (timer-gated radius), pas par les collision events.
            RigidBody::Fixed,
            Collider::cylinder(0.06, cfg.lava_radius),
            Sensor,
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(rim_mesh),
                MeshMaterial3d(rim_mat),
                Transform::from_xyz(0.0, -0.03, 0.0),
            ));
        });
}

fn spawn_forge(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    cfg: &RoguelitePoiConfig,
) {
    let mesh = meshes.add(Cuboid::new(1.3, 1.1, 0.9));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.28, 0.30),     // fer sombre
        emissive: LinearRgba::new(2.0, 0.8, 0.2, 1.0), // braise de forge
        metallic: 0.7,
        perceptual_roughness: 0.4,
        ..default()
    });
    // Balise forge = braise chaude vive + tint blanc-chaud (distincte du cyan
    // des vaults ; la forge a aussi un PointLight donc lisible de près).
    let beacon = (cfg.beacon_height > 0.0).then(|| {
        beacon_bundle(
            meshes,
            materials,
            cfg.beacon_height,
            1.1,
            Color::srgba(1.0, 0.85, 0.55, 0.55),
            LinearRgba::new(6.0, 3.0, 1.0, 1.0),
        )
    });
    commands
        .spawn((
            poi_markers("RoguelitePoi_Forge"),
            PoiForge,
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(pos.with_y(0.55)),
            RigidBody::Fixed,
            Collider::cuboid(0.65, 0.55, 0.45),
            Sensor,
        ))
        .with_children(|p| {
            // Lueur chaude — clampée sous le plafond Atmosphere 8k.
            p.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.55, 0.2),
                    intensity: 6_000.0_f32.min(ATMOSPHERE_LUMEN_CAP),
                    range: 12.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 1.2, 0.0),
            ));
            if let Some(b) = beacon {
                p.spawn(b);
            }
        });
}

// ─── System : loot vault walk-over ───────────────────────────────────────────

/// Walk-over (rayon) → +Souls 1×/run, puis despawn du coffre + faisceau.
///
/// Despawn (récursif → emporte le beam enfant) plutôt que dim : le faisceau cyan
/// ne doit plus pointer un coffre vidé (sinon "loot ici" trompeur). Le prefab
/// décoratif `forgia-stage` posé au même anchor reste visible = "coffre ouvert".
/// L'anchor garde `PoiAssigned` → pas de re-spawn du vault dans ce stage.
pub fn sys_loot_vault_walkover(
    mut commands: Commands,
    q_player: Query<&Transform, With<Player>>,
    mut q_vaults: Query<(Entity, &Transform, &mut LootVault)>,
    mut souls: Option<ResMut<Souls>>,
    mut stats: ResMut<PoiStats>,
) {
    let Some(player_xf) = q_player.iter().next() else {
        return;
    };
    let ppos = player_xf.translation;
    for (e, xf, mut vault) in &mut q_vaults {
        if vault.looted {
            continue;
        }
        if !within_radius_xz(xf.translation, ppos, vault.radius) {
            continue;
        }
        vault.looted = true;
        if let Some(souls) = souls.as_deref_mut() {
            souls.current = souls.current.saturating_add(vault.reward);
            souls.total_collected = souls.total_collected.saturating_add(vault.reward);
        }
        stats.vaults_looted = stats.vaults_looted.saturating_add(1);
        stats.souls_from_vaults = stats.souls_from_vaults.saturating_add(vault.reward);
        info!(
            "[poi] Loot vault looté : +{} Souls (vaults looted total={})",
            vault.reward, stats.vaults_looted
        );
        commands.entity(e).despawn();
    }
}

// ─── System : hazard de lave (timer-gated, hot path) ─────────────────────────

/// Dégât périodique au contact de la lave (joueur ET ennemis). Timer-gated par
/// `cfg.lava_tick_period` (pas de scan per-frame), N bornés, 0 alloc.
///
/// - Joueur : `DamageEvent{kind:Fire}` → route HealthGuard → DeathEvent → Defeat.
/// - Bots : `forgia_combat::Health` direct + `CombatHitEvent`, mort déléguée à
///   `despawn_dead_cubes`. Un ennemi poussé dedans (knockback boon) y meurt.
#[allow(clippy::too_many_arguments)]
pub fn sys_lava_tick(
    time: Res<Time>,
    cfg: Option<Res<RoguelitePoiConfig>>,
    mut accum: Local<f32>,
    q_lava: Query<(&Transform, &LavaHazard)>,
    q_player: Query<(Entity, &Transform), With<Player>>,
    q_bots: Query<(Entity, &Transform), With<ArenaBot>>,
    mut q_health: Query<&mut CombatHealth>,
    mut damage_w: MessageWriter<DamageEvent>,
    mut hits_w: MessageWriter<CombatHitEvent>,
    mut stats: ResMut<PoiStats>,
) {
    let Some(cfg) = cfg else {
        return;
    };
    *accum += time.delta_secs();
    if *accum < cfg.lava_tick_period {
        return;
    }
    *accum = 0.0;

    if q_lava.is_empty() {
        return;
    }

    // Joueur(s) dans la lave → DamageEvent Fire.
    for (p_e, p_xf) in &q_player {
        let in_lava = q_lava
            .iter()
            .any(|(lxf, lava)| within_radius_xz(lxf.translation, p_xf.translation, lava.radius));
        if in_lava {
            damage_w.write(DamageEvent {
                target: p_e,
                source: None,
                amount: cfg.lava_damage_per_tick,
                kind: DamageKind::Fire,
            });
            stats.lava_player_ticks = stats.lava_player_ticks.saturating_add(1);
        }
    }

    // Ennemis dans la lave → dégât combat direct + CombatHitEvent.
    for (b_e, b_xf) in &q_bots {
        let in_lava = q_lava
            .iter()
            .any(|(lxf, lava)| within_radius_xz(lxf.translation, b_xf.translation, lava.radius));
        if !in_lava {
            continue;
        }
        if let Ok(mut hp) = q_health.get_mut(b_e) {
            if hp.is_dead() {
                continue;
            }
            hp.current = (hp.current - cfg.lava_damage_per_tick).max(0.0);
            let dead = hp.is_dead();
            if dead {
                stats.lava_kills_total = stats.lava_kills_total.saturating_add(1);
            }
            hits_w.write(CombatHitEvent {
                target: b_e,
                attacker: None, // kill environnemental — pas de crédit heal-on-kill joueur
                damage: cfg.lava_damage_per_tick,
                is_kill: dead,
                is_headshot: false,
                hit_world_pos: b_xf.translation + Vec3::Y * 1.0,
                weapon: None,
                body_zone: HitZone::Body,
            });
        }
    }
}

// ─── Sensor `forgia2_stage_poi.json` (counts vivants + cumuls) ───────────────

#[allow(clippy::too_many_arguments)]
pub fn sys_write_poi_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    q_vaults: Query<&LootVault>,
    q_lava: Query<&LavaHazard>,
    q_forge: Query<&PoiForge>,
    stats: Res<PoiStats>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let vaults = q_vaults.iter().count() as u32;
    let vaults_unlooted = q_vaults.iter().filter(|v| !v.looted).count() as u32;
    let lava = q_lava.iter().count() as u32;
    let forges = q_forge.iter().count() as u32;
    let total = vaults + lava + forges;
    let (severity, next_step) = severity_for_poi(total);

    let json = format!(
        r#"{{"id":"stage_poi","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"poi_total":{total},"loot_vaults":{vaults},"loot_vaults_unlooted":{vaults_unlooted},"lava_hazards":{lava},"forges":{forges},"vaults_looted":{},"souls_from_vaults":{},"lava_kills_total":{},"lava_player_ticks":{}}}"#,
        time.elapsed_secs(),
        stats.vaults_looted,
        stats.souls_from_vaults,
        stats.lava_kills_total,
        stats.lava_player_ticks,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[poi] sensor write failed: {e}");
    }
}

/// Pur — testable. `info` hors run (0 POI), `ok` dès qu'un POI est posé.
pub fn severity_for_poi(total: u32) -> (&'static str, &'static str) {
    if total == 0 {
        (
            "info",
            "0 POI (hors run ou stage sans anchor PoiSlot). Read forgia2_anchor.json::counts.poi_slot.",
        )
    } else {
        (
            "ok",
            "POI actifs. forgia2_anchor.json::counts.poi_slot doit etre > 0.",
        )
    }
}

// ─── Tests purs (headless) ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_default() {
        assert_eq!(
            RoguelitePoiConfig::parse_toml(""),
            RoguelitePoiConfig::default()
        );
    }

    #[test]
    fn parse_overrides_and_clamps() {
        let toml = r#"
[[genes]]
id = "poi_lava_radius"
default = 999.0
[[genes]]
id = "poi_vault_souls_reward"
default = 120.0
[[genes]]
id = "poi_forge_enabled"
default = 0.0
"#;
        let c = RoguelitePoiConfig::parse_toml(toml);
        assert_eq!(c.lava_radius, 30.0); // clamp haut
        assert_eq!(c.vault_souls_reward, 120);
        assert!(!c.forge_enabled);
    }

    #[test]
    fn forge_guaranteed_on_slot_zero() {
        let cfg = RoguelitePoiConfig::default();
        assert_eq!(assign_poi_kind(0, 42, &cfg), PoiKind::Forge);
    }

    #[test]
    fn slot_zero_not_forge_when_disabled() {
        let mut cfg = RoguelitePoiConfig::default();
        cfg.forge_enabled = false;
        // Slot 0 retombe dans le tirage vault/lava (jamais Forge).
        assert_ne!(assign_poi_kind(0, 42, &cfg), PoiKind::Forge);
    }

    #[test]
    fn assignment_is_seed_stable() {
        let cfg = RoguelitePoiConfig::default();
        for slot in 1..8u32 {
            let a = assign_poi_kind(slot, 0xCAFE, &cfg);
            let b = assign_poi_kind(slot, 0xCAFE, &cfg);
            assert_eq!(a, b, "slot {slot} non déterministe");
        }
    }

    #[test]
    fn different_seeds_can_diverge() {
        let cfg = RoguelitePoiConfig::default();
        // Au moins un slot diffère entre deux seeds (sinon RNG dégénéré).
        let any_diff =
            (1..16u32).any(|s| assign_poi_kind(s, 1, &cfg) != assign_poi_kind(s, 999_999, &cfg));
        assert!(any_diff, "aucune divergence inter-seed");
    }

    #[test]
    fn zero_weights_fallback_to_vault() {
        let mut cfg = RoguelitePoiConfig::default();
        cfg.forge_enabled = false;
        cfg.vault_weight = 0.0;
        cfg.lava_weight = 0.0;
        assert_eq!(assign_poi_kind(1, 7, &cfg), PoiKind::LootVault);
    }

    #[test]
    fn all_lava_weight_yields_lava() {
        let mut cfg = RoguelitePoiConfig::default();
        cfg.forge_enabled = false;
        cfg.vault_weight = 0.0;
        cfg.lava_weight = 10.0;
        for slot in 1..12u32 {
            assert_eq!(
                assign_poi_kind(slot, u64::from(slot), &cfg),
                PoiKind::LavaHazard
            );
        }
    }

    #[test]
    fn within_radius_xz_ignores_y() {
        let a = Vec3::new(0.0, 100.0, 0.0);
        let b = Vec3::new(3.0, -50.0, 4.0); // distance XZ = 5
        assert!(within_radius_xz(a, b, 5.01));
        assert!(!within_radius_xz(a, b, 4.99));
    }

    #[test]
    fn severity_info_when_empty_ok_when_present() {
        assert_eq!(severity_for_poi(0).0, "info");
        assert_eq!(severity_for_poi(3).0, "ok");
    }
}
