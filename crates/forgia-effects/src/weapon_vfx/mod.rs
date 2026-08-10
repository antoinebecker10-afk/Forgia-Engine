#![allow(dead_code, unused_imports)]
//! Port verbatim de `forgia-game/src/effects/weapon_vfx/mod.rs` (V1).
//! Weapon VFX module — AAA-quality particle effects for hitscan weapons.
//!
//! Architecture mirrors fireball_vfx: resource cache of EffectAsset handles,
//! setup at Startup, spawned at fire time via event or direct spawn.

pub mod impact;
pub mod muzzle;
pub mod status;
pub mod tracer;

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use forgia_combat::weapons::WeaponType;
use std::sync::atomic::{AtomicUsize, Ordering};

// TODO: port from V1 — components::Lifetime
// use forgia_core::components::Lifetime;

/// Multiplier scale appliqué au Transform des particules muzzle, par arme.
/// Snipers / shotguns / rockets ont un muzzle visiblement plus gros — feedback gameplay
/// distinct (pattern V1 confirmé : Boucherie shotgun = flash large, Lenoir sniper = long
/// flash, Pépin / Bourrasque = standard SMG).
pub fn weapon_muzzle_scale(w: &WeaponType) -> f32 {
    // Story-450 (2026-05-18) — softening user request : VFX trop gros bloquaient la
    // visée. Réduction ~35% sur tous les scales, baseline ModernAR passe 1.0 → 0.65.
    // Signature gameplay préservée (shotgun/rocket toujours plus gros).
    match w {
        WeaponType::Shotgun => 1.05, // Madame Lenoir (sniper-shotgun hybride V2)
        WeaponType::RocketLauncher => 1.20, // Boucherie (heavy mais réduit)
        WeaponType::AK47 => 0.80,
        WeaponType::AssaultRifle => 0.75, // Bourrasque
        WeaponType::ModernAR => 0.65,     // Pépin (baseline shrink)
        WeaponType::PlasmaRifle => 0.85,
        WeaponType::Chainsaw => 0.0, // pas de muzzle (mêlée)
    }
}

/// Multiplier scale impact VFX. Rocket = gros plume, sniper = impact net, SMG = standard.
pub fn weapon_impact_scale(w: &WeaponType) -> f32 {
    match w {
        WeaponType::RocketLauncher => 2.0,
        WeaponType::Shotgun => 0.8, // par pellet plus petit (8 pellets cumulés)
        WeaponType::PlasmaRifle => 1.3,
        _ => 1.0,
    }
}

/// Tint LinearRgba du muzzle flash per-arme (Phase 5 dette tech 2026-05-18).
///
/// Mapping Arena V1 (WeaponType legacy → arme V2 réelle) :
/// - `ModernAR` (Pépin pistolet)        → blanc-jaune cordite standard
/// - `AssaultRifle` (Bourrasque SMG)    → blanc-jaune chaud
/// - `Shotgun` (Madame Lenoir sniper)   → blanc froid, signature flash long
/// - `RocketLauncher` (Boucherie pump)  → rouge-orange poudre brute
/// - Autres : fallback blanc neutre
///
/// Valeurs HDR (>1.0 sur composantes pour bloom catch). Source : refs Valorant /
/// Apex tuning sheets — muzzle = warm white avec twist per-weapon.
pub fn weapon_muzzle_tint(w: &WeaponType) -> LinearRgba {
    // story-659 — teintes alignées sur les ÉLÉMENTS des armes (roguelite_elements
    // .toml : Pépin=électrique, Bourrasque=feu, Lenoir=perce, Boucherie=poison).
    // Chaque tir peint le décor à la couleur de son élément (light + flash).
    match w {
        WeaponType::ModernAR => LinearRgba::new(1.0, 2.0, 3.4, 1.0), // électrique bleu
        WeaponType::AssaultRifle => LinearRgba::new(3.4, 1.6, 0.5, 1.0), // FEU orange
        WeaponType::Shotgun => LinearRgba::new(0.9, 2.8, 3.0, 1.0),  // perce cyan froid
        WeaponType::RocketLauncher => LinearRgba::new(0.9, 3.2, 0.6, 1.0), // POISON vert
        WeaponType::PlasmaRifle => LinearRgba::new(0.8, 1.8, 3.5, 1.0), // cyan plasma
        _ => LinearRgba::new(3.0, 2.6, 1.4, 1.0),
    }
}

/// Tint impact VFX per-arme. Plus sobre que muzzle (impact = poussière/spark, pas flash).
pub fn weapon_impact_tint(w: &WeaponType) -> LinearRgba {
    // story-659 — impacts alignés sur les éléments (cf weapon_muzzle_tint).
    match w {
        WeaponType::ModernAR => LinearRgba::new(0.8, 1.6, 2.6, 1.0), // électrique
        WeaponType::AssaultRifle => LinearRgba::new(2.6, 1.2, 0.4, 1.0), // feu
        WeaponType::RocketLauncher => LinearRgba::new(0.7, 2.4, 0.5, 1.0), // poison
        WeaponType::Shotgun => LinearRgba::new(0.7, 2.2, 2.4, 1.0),  // perce cyan
        WeaponType::PlasmaRifle => LinearRgba::new(0.6, 1.6, 3.0, 1.0), // cyan plasma
        _ => LinearRgba::new(2.4, 2.0, 1.2, 1.0),                    // étincelles standard
    }
}

/// Lifetime smoke per-arme. Sniper Lenoir = trail long (signature one-shot).
/// Shotgun = court (déjà compensé par spawn larger).
pub fn weapon_muzzle_smoke_lifetime(w: &WeaponType) -> f32 {
    match w {
        WeaponType::Shotgun => 2.5,        // sniper Lenoir : long trail
        WeaponType::RocketLauncher => 1.6, // shotgun Boucherie : smoke lourd
        WeaponType::AssaultRifle => 0.9,   // SMG full-auto : court pour pas accumuler
        _ => 1.2,
    }
}

/// PointLight intensité brève muzzle (lumens). Reste sous threshold "too bright with
/// Atmosphere" (cf comment historique L167) : 0 = skip (mêlée), sniper plus brillant.
pub fn weapon_muzzle_light_intensity(w: &WeaponType) -> f32 {
    match w {
        WeaponType::Chainsaw => 0.0,
        WeaponType::RocketLauncher => 8_000.0, // shotgun boom
        WeaponType::Shotgun => 6_000.0,        // sniper one-shot
        WeaponType::AssaultRifle => 2_500.0,   // SMG sustained = faible (anti eye-strain)
        WeaponType::ModernAR => 3_500.0,       // pistolet
        _ => 3_000.0,
    }
}

/// Lifetime component — ported inline until forgia-core exports it.
#[derive(Component)]
pub struct Lifetime(pub Timer);

// ─── Story-652 : visibilité des VFX pilotée en DATA ──────────────────────────
// La story-450 avait nerfé les effets (-35 % taille, HDR ÷3) car « trop gros,
// bloquaient la visée » — résultat : flash de 1,5-3,5 cm sur 25-50 ms =
// subliminal (feedback user 2026-07-03 : « je ne vois aucun effet »). La
// visibilité devient un choix DATA (genome, hot-reload avec rebuild des
// assets), plus un hardcode historique.

const VFX_GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_vfx.toml";
const VFX_SENSOR_PATH: &str = "forgia2_weapon_vfx.json";

/// Multiplicateurs globaux appliqués À LA CONSTRUCTION des EffectAssets.
/// Hot-reload : les assets sont RECONSTRUITS au changement du TOML (petit
/// hitch shader possible — outil de tuning, pas un toggle de combat).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct VfxTuning {
    /// Taille des particules (init SIZE + gradients SizeOverLifetime).
    pub size_mult: f32,
    /// Quantité par burst.
    pub count_mult: f32,
    /// Durée de vie des particules.
    pub lifetime_mult: f32,
    /// Décalage des effets d'impact HORS de la surface, vers le tireur (m) —
    /// architecture standard (offset le long de la normale, cf VFXDoc/RealTimeVFX) :
    /// sans lui, la moitié des particules naît DANS le mesh du mob et est occluse.
    pub impact_offset_m: f32,
    /// Largeur de l'enveloppe des AURAS de statut autour du mob (× le rayon de
    /// spawn ~0.5 m) — demande user 2026-07-03 « plus large autour du mob ».
    pub aura_width_mult: f32,
}

impl Default for VfxTuning {
    fn default() -> Self {
        // Défauts story-652 Inc.2 (feedback user : « encore trop caché dans les
        // mobs ») : ×3.0 taille / ×1.8 quantité + offset hors-surface 0.35 m.
        Self {
            size_mult: 3.0,
            count_mult: 1.8,
            lifetime_mult: 1.3,
            impact_offset_m: 0.35,
            aura_width_mult: 1.5,
        }
    }
}

#[derive(serde::Deserialize)]
struct VfxGeneToml {
    id: String,
    #[serde(default)]
    default: f32,
}

#[derive(serde::Deserialize)]
struct VfxGenomeToml {
    #[serde(default)]
    genes: Vec<VfxGeneToml>,
}

impl VfxTuning {
    /// Pur — genes plats, pattern render_quality/gamefeel.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: VfxGenomeToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut t = Self::default();
        for gene in &parsed.genes {
            match gene.id.as_str() {
                "vfx_size_mult" => t.size_mult = gene.default.clamp(0.2, 6.0),
                "vfx_count_mult" => t.count_mult = gene.default.clamp(0.2, 4.0),
                "vfx_lifetime_mult" => t.lifetime_mult = gene.default.clamp(0.3, 3.0),
                "vfx_impact_offset_m" => {
                    t.impact_offset_m = gene.default.clamp(0.0, 1.5);
                }
                "vfx_aura_width_mult" => {
                    t.aura_width_mult = gene.default.clamp(0.5, 4.0);
                }
                _ => {}
            }
        }
        t
    }

    fn load_or_default() -> Self {
        match std::fs::read_to_string(VFX_GENOME_PATH) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

/// Suivi mtime + compteurs capteur.
#[derive(Resource, Default)]
pub struct VfxGenomeWatch {
    pub last_mtime: Option<std::time::SystemTime>,
    pub reload_count: u32,
    pub kill_bursts: u64,
}

/// Textures particules Kenney Particle Pack (CC0) — story-647.
/// Licence : `assets/textures/vfx/kenney/LICENSE-CC0-Kenney.txt`.
/// Textures blanches/grayscale à alpha : teintées par les gradients HDR des
/// effets (`ImageSampleMapping::Modulate`) — une texture sert tous les éléments.
mod tex_paths {
    pub const MUZZLE_FLASH: &str = "textures/vfx/kenney/muzzle_01.png";
    pub const MUZZLE_TONGUE: &str = "textures/vfx/kenney/muzzle_03.png";
    pub const SPARK: &str = "textures/vfx/kenney/spark_04.png";
    pub const SMOKE: &str = "textures/vfx/kenney/smoke_04.png";
    pub const DUST: &str = "textures/vfx/kenney/smoke_01.png";
    pub const GLOW: &str = "textures/vfx/kenney/light_01.png";
    pub const FLARE: &str = "textures/vfx/kenney/flare_01.png";
    pub const FLAME: &str = "textures/vfx/kenney/flame_03.png";
    pub const POISON: &str = "textures/vfx/kenney/smoke_08.png";
    /// Story-652 — volute du burst de kill.
    pub const BURST: &str = "textures/vfx/kenney/twirl_01.png";
}

/// Cached handles for all weapon VFX particle effects.
/// Inserted at Startup, consumed by weapon_fire_system.
#[derive(Resource)]
pub struct WeaponVfxEffects {
    // Muzzle flash (5 layers)
    pub muzzle_core_flash: Handle<EffectAsset>,
    pub muzzle_sparks: Handle<EffectAsset>,
    pub muzzle_smoke: Handle<EffectAsset>,
    pub muzzle_heat_glow: Handle<EffectAsset>,
    pub muzzle_forward_flash: Handle<EffectAsset>,
    // Impact (3 layers)
    pub impact_sparks: Handle<EffectAsset>,
    pub impact_dust: Handle<EffectAsset>,
    pub impact_flash: Handle<EffectAsset>,
    // Status DoT continus (flamme sur brûlure, nuage sur poison) — story-611 VFX.
    pub status_flame: Handle<EffectAsset>,
    pub status_poison_cloud: Handle<EffectAsset>,
    // Story-653 — arcs crépitants sur StatusShock (identité Pépin électrique).
    pub status_shock: Handle<EffectAsset>,
    /// Copie du gene `vfx_impact_offset_m` (rebuild au hot-reload) — porté ici
    /// pour que les spawn fns l'aient sans param supplémentaire (limite 16 params
    /// du fire system).
    pub impact_offset_m: f32,
    // Story-652 — burst de kill (volutes chaudes qui s'ouvrent, le « moment WoW »).
    pub kill_burst: Handle<EffectAsset>,
    // Story-647 : textures particules (slot "color" de chaque EffectAsset).
    // Bindées via `EffectMaterial { images }` sur CHAQUE entité ParticleEffect
    // spawnée — un effet à slot texture sans EffectMaterial ne rend pas.
    pub tex_muzzle_flash: Handle<Image>,
    pub tex_muzzle_tongue: Handle<Image>,
    pub tex_spark: Handle<Image>,
    pub tex_smoke: Handle<Image>,
    pub tex_dust: Handle<Image>,
    pub tex_glow: Handle<Image>,
    pub tex_flare: Handle<Image>,
    pub tex_flame: Handle<Image>,
    pub tex_poison: Handle<Image>,
    pub tex_burst: Handle<Image>,
    /// Audit 2026-07-03 — les TTL des ENTITÉS d'effet doivent suivre le curseur
    /// durée : sinon monter `vfx_lifetime_mult` coupe les particules en vol
    /// (despawn entité hanabi = buffer particules détruit).
    pub lifetime_mult: f32,
    // ── LOT B (audit fire-path 2026-07-20) — pools persistants ──────────────
    // Plus AUCUN spawn/despawn par tir : chaque slot est repositionné et son
    // spawner est retrigger (anti-pattern hanabi #255 : churn EffectCache/
    // bind-group/dispatch-init GPU, ~90 instances/s en auto-fire Bourrasque).
    /// Pool des 5 couches muzzle + 1 lumière (1 set, repositionné à chaque tir).
    pub muzzle_pool: MuzzlePool,
    /// Pool round-robin d'impact (sparks+dust+light par slot) — 8 slots
    /// couvrent le cas extrême shotgun (jusqu'à 8 pellets/frame).
    pub impact_pool: [ImpactPoolSlot; IMPACT_POOL_SIZE],
    /// Curseur round-robin — `AtomicUsize` car `effects` est passé `&WeaponVfxEffects`
    /// (immuable) au fire path (forgia-fps), jamais `&mut`.
    pub impact_cursor: AtomicUsize,
    /// Pool round-robin de kill-burst (particule+light par slot) — 4 slots
    /// couvrent des kills simultanés (AoE/chaîne) qui se chevauchent dans la
    /// fenêtre de vie du burst (~0.35-0.6 s × lifetime_mult).
    pub kill_pool: [KillBurstPoolSlot; KILL_BURST_POOL_SIZE],
    pub kill_cursor: AtomicUsize,
}

/// Marker: muzzle VFX entity (for cleanup)
#[derive(Component)]
pub struct MuzzleVfxMarker;

/// Marker: impact VFX entity (for cleanup)
#[derive(Component)]
pub struct ImpactVfxMarker;

// ─── LOT B (audit fire-path 2026-07-20) — pools persistants Hanabi ──────────
// Best practice bevy_hanabi documentée (issue #255) : réutiliser un spawner
// existant (repositionner + retrigger) plutôt que spawn/despawn une entité par
// événement. Tailles de pool = perf interne, PAS exposées créateur (no-hardcode
// : ce sont des bornes de dimensionnement mémoire/GPU, pas des valeurs gameplay).

/// Nombre de slots round-robin du pool d'impact : couvre le cas extrême shotgun
/// (jusqu'à 8 pellets touchant dans la même frame) sans réutiliser un slot
/// encore visuellement actif.
const IMPACT_POOL_SIZE: usize = 8;
/// Nombre de slots du pool de kill-burst : kills simultanés (AoE / chaîne) qui
/// peuvent se chevaucher dans la fenêtre de vie du burst.
const KILL_BURST_POOL_SIZE: usize = 4;

/// Pool persistant des 5 couches muzzle + 1 lumière (LOT B) : repositionné à
/// chaque tir, jamais respawné.
pub struct MuzzlePool {
    pub core_flash: Entity,
    pub sparks: Entity,
    pub smoke: Entity,
    pub heat_glow: Entity,
    pub forward_flash: Entity,
    pub light: Entity,
}

/// 1 slot du pool d'impact : sparks + dust + light partageant toujours la
/// même position à chaque réutilisation.
pub struct ImpactPoolSlot {
    pub sparks: Entity,
    pub dust: Entity,
    pub light: Entity,
}

/// 1 slot du pool de kill-burst : particule + light.
pub struct KillBurstPoolSlot {
    pub particle: Entity,
    pub light: Entity,
}

/// Position initiale des entités de pool : cachée, loin de la scène — même
/// pattern que l'ancien warmup dummy (leur toute première émission hanabi,
/// invisible, paie AUSSI la compile shader avant le 1er tir réel, cf story-594).
fn pool_hidden_transform() -> Transform {
    Transform::from_xyz(0.0, -10_000.0, 0.0)
}

/// Spawn les 6 entités persistantes du pool muzzle (LOT B).
#[allow(clippy::too_many_arguments)]
fn spawn_muzzle_pool(
    commands: &mut Commands,
    core_flash: &Handle<EffectAsset>,
    sparks: &Handle<EffectAsset>,
    smoke: &Handle<EffectAsset>,
    heat_glow: &Handle<EffectAsset>,
    forward_flash: &Handle<EffectAsset>,
    tex_muzzle_flash: &Handle<Image>,
    tex_spark: &Handle<Image>,
    tex_smoke: &Handle<Image>,
    tex_glow: &Handle<Image>,
    tex_muzzle_tongue: &Handle<Image>,
) -> MuzzlePool {
    let hidden = pool_hidden_transform();
    let core_flash = commands
        .spawn((
            ParticleEffect::new(core_flash.clone()),
            EffectMaterial {
                images: vec![tex_muzzle_flash.clone()],
            },
            hidden,
            Visibility::Hidden,
            MuzzleVfxMarker,
        ))
        .id();
    let sparks = commands
        .spawn((
            ParticleEffect::new(sparks.clone()),
            EffectMaterial {
                images: vec![tex_spark.clone()],
            },
            hidden,
            Visibility::Hidden,
            MuzzleVfxMarker,
        ))
        .id();
    let smoke = commands
        .spawn((
            ParticleEffect::new(smoke.clone()),
            EffectMaterial {
                images: vec![tex_smoke.clone()],
            },
            hidden,
            Visibility::Hidden,
            MuzzleVfxMarker,
        ))
        .id();
    let heat_glow = commands
        .spawn((
            ParticleEffect::new(heat_glow.clone()),
            EffectMaterial {
                images: vec![tex_glow.clone()],
            },
            hidden,
            Visibility::Hidden,
            MuzzleVfxMarker,
        ))
        .id();
    let forward_flash = commands
        .spawn((
            ParticleEffect::new(forward_flash.clone()),
            EffectMaterial {
                images: vec![tex_muzzle_tongue.clone()],
            },
            hidden,
            Visibility::Hidden,
            MuzzleVfxMarker,
        ))
        .id();
    let light = commands
        .spawn((
            PointLight {
                intensity: 0.0,
                shadows_enabled: false,
                ..default()
            },
            hidden,
            Visibility::Hidden,
            MuzzleVfxMarker,
        ))
        .id();
    MuzzlePool {
        core_flash,
        sparks,
        smoke,
        heat_glow,
        forward_flash,
        light,
    }
}

/// Spawn les `IMPACT_POOL_SIZE` slots persistants du pool d'impact (LOT B).
fn spawn_impact_pool(
    commands: &mut Commands,
    impact_sparks: &Handle<EffectAsset>,
    impact_dust: &Handle<EffectAsset>,
    tex_spark: &Handle<Image>,
    tex_dust: &Handle<Image>,
) -> [ImpactPoolSlot; IMPACT_POOL_SIZE] {
    std::array::from_fn(|_| {
        let hidden = pool_hidden_transform();
        let sparks = commands
            .spawn((
                ParticleEffect::new(impact_sparks.clone()),
                EffectMaterial {
                    images: vec![tex_spark.clone()],
                },
                hidden,
                Visibility::Hidden,
                ImpactVfxMarker,
            ))
            .id();
        let dust = commands
            .spawn((
                ParticleEffect::new(impact_dust.clone()),
                EffectMaterial {
                    images: vec![tex_dust.clone()],
                },
                hidden,
                Visibility::Hidden,
                ImpactVfxMarker,
            ))
            .id();
        let light = commands
            .spawn((
                PointLight {
                    intensity: 0.0,
                    shadows_enabled: false,
                    ..default()
                },
                hidden,
                Visibility::Hidden,
                ImpactVfxMarker,
            ))
            .id();
        ImpactPoolSlot {
            sparks,
            dust,
            light,
        }
    })
}

/// Spawn les `KILL_BURST_POOL_SIZE` slots persistants du pool de kill-burst (LOT B).
fn spawn_kill_pool(
    commands: &mut Commands,
    kill_burst: &Handle<EffectAsset>,
    tex_burst: &Handle<Image>,
) -> [KillBurstPoolSlot; KILL_BURST_POOL_SIZE] {
    std::array::from_fn(|_| {
        let hidden = pool_hidden_transform();
        let particle = commands
            .spawn((
                ParticleEffect::new(kill_burst.clone()),
                EffectMaterial {
                    images: vec![tex_burst.clone()],
                },
                hidden,
                Visibility::Hidden,
                ImpactVfxMarker,
            ))
            .id();
        let light = commands
            .spawn((
                PointLight {
                    intensity: 0.0,
                    shadows_enabled: false,
                    ..default()
                },
                hidden,
                Visibility::Hidden,
                ImpactVfxMarker,
            ))
            .id();
        KillBurstPoolSlot { particle, light }
    })
}

/// Despawn l'ANCIEN pool avant reconstruction — hot-reload uniquement (rare,
/// outil de tuning en session). Le chemin de tir normal ne despawn JAMAIS ces
/// entités (c'est tout l'objet du LOT B).
fn despawn_weapon_vfx_pools(commands: &mut Commands, vfx: &WeaponVfxEffects) {
    let mp = &vfx.muzzle_pool;
    for e in [
        mp.core_flash,
        mp.sparks,
        mp.smoke,
        mp.heat_glow,
        mp.forward_flash,
        mp.light,
    ] {
        commands.entity(e).try_despawn();
    }
    for slot in &vfx.impact_pool {
        for e in [slot.sparks, slot.dust, slot.light] {
            commands.entity(e).try_despawn();
        }
    }
    for slot in &vfx.kill_pool {
        for e in [slot.particle, slot.light] {
            commands.entity(e).try_despawn();
        }
    }
}

/// Repositionne une entité de pool persistante et force une NOUVELLE émission :
/// retire `EffectSpawner`, que `tick_spawners()` de hanabi ré-ajoute frais dès
/// la frame suivante (clonage de `SpawnerSettings` depuis l'asset, comme pour
/// une entité neuve) — comportement identique à un spawn frais, sans jamais
/// recréer l'entité ni son slot GPU (EffectCache/bind-group). LOT B, audit
/// fire-path 2026-07-20.
fn retrigger_pool_particle(commands: &mut Commands, entity: Entity, transform: Transform) {
    commands
        .entity(entity)
        .insert((transform, Visibility::Visible))
        .remove::<EffectSpawner>();
}

/// Construit tous les EffectAssets selon le tuning (Startup + rebuild hot-reload)
/// ET spawn les pools persistants LOT B (muzzle/impact/kill — cf plus haut).
fn build_weapon_vfx(
    commands: &mut Commands,
    effects: &mut ResMut<Assets<EffectAsset>>,
    asset_server: &AssetServer,
    t: &VfxTuning,
) -> WeaponVfxEffects {
    let muzzle_core_flash = muzzle::create_muzzle_core_flash(effects, t);
    let muzzle_sparks = muzzle::create_muzzle_sparks(effects, t);
    let muzzle_smoke = muzzle::create_muzzle_smoke(effects, t);
    let muzzle_heat_glow = muzzle::create_muzzle_heat_glow(effects, t);
    let muzzle_forward_flash = muzzle::create_muzzle_forward_flash(effects, t);
    let impact_sparks = impact::create_impact_sparks(effects, t);
    let impact_dust = impact::create_impact_dust(effects, t);
    let impact_flash = impact::create_impact_flash(effects, t);
    let status_flame = status::create_status_flame(effects, t);
    let status_poison_cloud = status::create_status_poison_cloud(effects, t);
    let status_shock = status::create_status_shock(effects, t);
    let kill_burst = impact::create_kill_burst(effects, t);

    let tex_muzzle_flash: Handle<Image> = asset_server.load(tex_paths::MUZZLE_FLASH);
    let tex_muzzle_tongue: Handle<Image> = asset_server.load(tex_paths::MUZZLE_TONGUE);
    let tex_spark: Handle<Image> = asset_server.load(tex_paths::SPARK);
    let tex_smoke: Handle<Image> = asset_server.load(tex_paths::SMOKE);
    let tex_dust: Handle<Image> = asset_server.load(tex_paths::DUST);
    let tex_glow: Handle<Image> = asset_server.load(tex_paths::GLOW);
    let tex_flare: Handle<Image> = asset_server.load(tex_paths::FLARE);
    let tex_flame: Handle<Image> = asset_server.load(tex_paths::FLAME);
    let tex_poison: Handle<Image> = asset_server.load(tex_paths::POISON);
    let tex_burst: Handle<Image> = asset_server.load(tex_paths::BURST);

    let muzzle_pool = spawn_muzzle_pool(
        commands,
        &muzzle_core_flash,
        &muzzle_sparks,
        &muzzle_smoke,
        &muzzle_heat_glow,
        &muzzle_forward_flash,
        &tex_muzzle_flash,
        &tex_spark,
        &tex_smoke,
        &tex_glow,
        &tex_muzzle_tongue,
    );
    let impact_pool = spawn_impact_pool(
        commands,
        &impact_sparks,
        &impact_dust,
        &tex_spark,
        &tex_dust,
    );
    let kill_pool = spawn_kill_pool(commands, &kill_burst, &tex_burst);

    WeaponVfxEffects {
        muzzle_core_flash,
        muzzle_sparks,
        muzzle_smoke,
        muzzle_heat_glow,
        muzzle_forward_flash,
        impact_sparks,
        impact_dust,
        impact_flash,
        status_flame,
        status_poison_cloud,
        status_shock,
        impact_offset_m: t.impact_offset_m,
        lifetime_mult: t.lifetime_mult,
        kill_burst,
        tex_muzzle_flash,
        tex_muzzle_tongue,
        tex_spark,
        tex_smoke,
        tex_dust,
        tex_glow,
        tex_flare,
        tex_flame,
        tex_poison,
        tex_burst,
        muzzle_pool,
        impact_pool,
        impact_cursor: AtomicUsize::new(0),
        kill_pool,
        kill_cursor: AtomicUsize::new(0),
    }
}

pub fn setup_weapon_vfx(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    asset_server: Res<AssetServer>,
) {
    let t = VfxTuning::load_or_default();
    let mtime = std::fs::metadata(VFX_GENOME_PATH)
        .and_then(|m| m.modified())
        .ok();
    let vfx = build_weapon_vfx(&mut commands, &mut effects, &asset_server, &t);
    commands.insert_resource(vfx);
    commands.insert_resource(t);
    commands.insert_resource(VfxGenomeWatch {
        last_mtime: mtime,
        ..Default::default()
    });
    info!(
        "Weapon VFX initialises (5-layer muzzle + 3-layer impact + kill burst, pools persistants LOT B, tuning ×{:.1} taille ×{:.1} qté — story-652)",
        t.size_mult, t.count_mult
    );
}

/// Story-652 — hot-reload du tuning VFX : RECONSTRUIT les EffectAssets au
/// changement du TOML (1Hz). LOT B — l'ancien pool persistant est despawné
/// AVANT reconstruction (un seul hitch au hot-reload, rare et dev-only ; le
/// chemin de tir normal ne despawn jamais rien).
pub fn sys_hot_reload_vfx_genome(
    mut commands: Commands,
    time: Res<Time>,
    mut accum: Local<f32>,
    mut effects: ResMut<Assets<EffectAsset>>,
    asset_server: Res<AssetServer>,
    tuning: Option<ResMut<VfxTuning>>,
    watch: Option<ResMut<VfxGenomeWatch>>,
    vfx: Option<ResMut<WeaponVfxEffects>>,
) {
    let (Some(mut tuning), Some(mut watch), Some(mut vfx)) = (tuning, watch, vfx) else {
        return;
    };
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;
    let Ok(meta) = std::fs::metadata(VFX_GENOME_PATH) else {
        return;
    };
    let Ok(mtime) = meta.modified() else {
        return;
    };
    if watch.last_mtime == Some(mtime) {
        return;
    }
    watch.last_mtime = Some(mtime);
    let Ok(content) = std::fs::read_to_string(VFX_GENOME_PATH) else {
        return;
    };
    let new_tuning = VfxTuning::parse_toml(&content);
    if new_tuning != *tuning {
        *tuning = new_tuning;
        despawn_weapon_vfx_pools(&mut commands, &vfx);
        *vfx = build_weapon_vfx(&mut commands, &mut effects, &asset_server, &new_tuning);
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!(
            "[weapon-vfx] HOT-RELOADED — taille ×{:.1} qté ×{:.1} durée ×{:.1} (assets + pools reconstruits)",
            new_tuning.size_mult, new_tuning.count_mult, new_tuning.lifetime_mult
        );
    }
}

/// Capteur 1Hz `forgia2_weapon_vfx.json` — tuning actif + compteurs.
pub fn sys_write_weapon_vfx_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    tuning: Option<Res<VfxTuning>>,
    watch: Option<Res<VfxGenomeWatch>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;
    let (Some(t), Some(w)) = (tuning, watch) else {
        return;
    };
    let json = format!(
        r#"{{"id":"weapon_vfx","severity":"ok","next_step":"Visibilité tunable LIVE dans roguelite_vfx.toml (hot-reload 1Hz, assets reconstruits).","timestamp_secs":{:.1},"size_mult":{:.2},"count_mult":{:.2},"lifetime_mult":{:.2},"kill_bursts":{},"reload_count":{}}}"#,
        time.elapsed_secs(),
        t.size_mult,
        t.count_mult,
        t.lifetime_mult,
        w.kill_bursts,
        w.reload_count,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(VFX_SENSOR_PATH, json) {
        warn!("[weapon-vfx] sensor write failed: {e}");
    }
}

/// Story-652 — burst de kill au point d'impact : volutes chaudes + PointLight
/// bref tinté par l'arme. Appelé par le fire path à l'edge vivant→mort.
/// LOT B (2026-07-20) : round-robin sur `KILL_BURST_POOL_SIZE` slots persistants
/// — repositionne + retrigger, ne spawn/despawn plus jamais d'entité.
pub fn spawn_kill_burst(
    commands: &mut Commands,
    effects: &WeaponVfxEffects,
    pos: Vec3,
    shot_dir: Vec3,
    weapon: &WeaponType,
) {
    // Offset hors-surface vers le tireur (cf spawn_impact_vfx) — le burst de
    // kill doit s'ouvrir DEVANT le corps, pas dedans.
    let pos = pos - shot_dir.normalize_or_zero() * effects.impact_offset_m;
    let scale_v = Vec3::splat(weapon_impact_scale(weapon).max(1.0));

    let idx = effects.kill_cursor.fetch_add(1, Ordering::Relaxed) % KILL_BURST_POOL_SIZE;
    let slot = &effects.kill_pool[idx];
    retrigger_pool_particle(
        commands,
        slot.particle,
        Transform::from_translation(pos).with_scale(scale_v),
    );

    // Lumière du kill : plus forte que l'impact standard (l'événement), toujours
    // bornée (leçon anti-casse-Atmosphère, cf weapon_muzzle_light_intensity).
    // Decay géré par `PooledLightFade` (pas de despawn — pool persistant LOT B).
    let tint = weapon_impact_tint(weapon);
    commands.entity(slot.light).insert((
        PointLight {
            color: Color::LinearRgba(tint),
            intensity: 6_000.0,
            range: 5.0,
            radius: 0.1,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(pos),
        Visibility::Visible,
        crate::PooledLightFade {
            timer: Timer::from_seconds(0.08, TimerMode::Once),
            base_intensity: 6_000.0,
        },
    ));
}

/// Spawn all 5 muzzle flash layers at the given barrel tip position.
/// `shot_dir` orients the forward flash tongue.
/// `weapon` détermine le scale (Shotgun/Rocket = plus gros, mêlée = skip).
/// LOT B (2026-07-20) : pool persistant `effects.muzzle_pool` — repositionne
/// les 5 couches + retrigger leur spawner, ne spawn/despawn plus jamais.
pub fn spawn_muzzle_flash(
    commands: &mut Commands,
    effects: &WeaponVfxEffects,
    barrel_tip: Vec3,
    shot_dir: Vec3,
    weapon: &WeaponType,
) {
    let scale = weapon_muzzle_scale(weapon);
    if scale <= 0.0 {
        return; // mêlée — pas de muzzle
    }
    let scale_v = Vec3::splat(scale);
    let pool = &effects.muzzle_pool;

    // Transform oriented along shot direction
    let flash_tf = Transform::from_translation(barrel_tip)
        .looking_to(shot_dir, Vec3::Y)
        .with_scale(scale_v);

    // Layer 1: Core flash (world-space, at barrel tip)
    retrigger_pool_particle(
        commands,
        pool.core_flash,
        Transform::from_translation(barrel_tip).with_scale(scale_v),
    );

    // Layer 2: Spark spray (world-space)
    retrigger_pool_particle(
        commands,
        pool.sparks,
        Transform::from_translation(barrel_tip).with_scale(scale_v),
    );

    // Layer 3: Smoke puff (world-space, lingers — per-arme : sniper = trail long)
    retrigger_pool_particle(
        commands,
        pool.smoke,
        Transform::from_translation(barrel_tip + shot_dir * 0.05).with_scale(scale_v),
    );

    // Layer 4: Heat glow — story-450 (2026-05-18) : RE-ENABLED après shrink x3
    // (0.10-0.20 vs 0.30-0.60) + alpha /2. Halo bloom subtil qui ajoute la
    // qualité AAA "weight feel" sans bloquer la cible.
    retrigger_pool_particle(
        commands,
        pool.heat_glow,
        Transform::from_translation(barrel_tip).with_scale(scale_v),
    );

    // Layer 5: Forward flash tongue (oriented along barrel)
    retrigger_pool_particle(commands, pool.forward_flash, flash_tf);

    // Layer 6: PointLight bref tinté per-arme (Phase 5 dette tech 2026-05-18).
    // Historique : un PointLight plein-intensité cassait l'Atmosphere. Solution :
    // intensity faible (2k-8k lumens, vs 100k+ "réaliste"), decay ultra-court
    // (70ms), range borné. Donne signature couleur sans bloom run-away.
    // Decay géré par `PooledLightFade` (pas de despawn — pool persistant LOT B).
    let light_intensity = weapon_muzzle_light_intensity(weapon);
    if light_intensity > 0.0 {
        let tint = weapon_muzzle_tint(weapon);
        commands.entity(pool.light).insert((
            PointLight {
                color: Color::LinearRgba(tint),
                intensity: light_intensity,
                range: 4.0,
                radius: 0.05,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(barrel_tip),
            Visibility::Visible,
            crate::PooledLightFade {
                timer: Timer::from_seconds(0.07, TimerMode::Once),
                base_intensity: light_intensity,
            },
        ));
    }
}

/// Spawn impact VFX at hit point: 2 particle layers + point light.
/// `weapon` détermine le scale (Rocket = gros plume, sniper = précis, SMG = standard).
/// LOT B (2026-07-20) : round-robin sur `IMPACT_POOL_SIZE` slots persistants —
/// repositionne + retrigger, ne spawn/despawn plus jamais d'entité.
/// Bullet hole decal will be added when textures are available (Phase 3b).
pub fn spawn_impact_vfx(
    commands: &mut Commands,
    effects: &WeaponVfxEffects,
    impact_pos: Vec3,
    shot_dir: Vec3,
    weapon: &WeaponType,
) {
    // Story-652 Inc.2 — offset HORS de la surface, vers le tireur (architecture
    // standard : sans lui, la moitié des particules naît dans le mesh, occluse).
    let impact_pos = impact_pos - shot_dir.normalize_or_zero() * effects.impact_offset_m;
    let scale_v = Vec3::splat(weapon_impact_scale(weapon));
    let tf = Transform::from_translation(impact_pos).with_scale(scale_v);

    let idx = effects.impact_cursor.fetch_add(1, Ordering::Relaxed) % IMPACT_POOL_SIZE;
    let slot = &effects.impact_pool[idx];

    // Layer 1: Sparks (hemisphere burst)
    retrigger_pool_particle(commands, slot.sparks, tf);

    // Layer 2: Dust cloud
    retrigger_pool_particle(commands, slot.dust, tf);

    // Layer 3: Flash burst — story-432 V5-A (2026-05-13) : reste DISABLED
    // (marginal visuel, cf historique) — `effects.impact_flash` toujours
    // construit/warmé mais aucun slot de pool ne l'utilise.

    // Layer 4: PointLight bref tinté per-arme (Phase 5 dette tech 2026-05-18).
    // Intensity ~50% du muzzle (impact = secondaire feedback), decay 40ms.
    // Decay géré par `PooledLightFade` (pas de despawn — pool persistant LOT B).
    let tint = weapon_impact_tint(weapon);
    let intensity = match weapon {
        WeaponType::RocketLauncher => 5_000.0,
        WeaponType::Shotgun => 3_000.0,
        _ => 1_500.0,
    };
    commands.entity(slot.light).insert((
        PointLight {
            color: Color::LinearRgba(tint),
            intensity,
            range: 3.0,
            radius: 0.05,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(impact_pos),
        Visibility::Visible,
        crate::PooledLightFade {
            timer: Timer::from_seconds(0.04, TimerMode::Once),
            base_intensity: intensity,
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Phase 5 (dette tech 2026-05-18) — VFX colors per-arme tests purs ──

    #[test]
    fn muzzle_tint_matches_weapon_element() {
        // story-659 — chaque arme flash à la couleur de SON élément
        // (roguelite_elements.toml : Pépin=électrique, Bourrasque=feu,
        // Lenoir=perce cyan, Boucherie=poison).
        let pepin = weapon_muzzle_tint(&WeaponType::ModernAR);
        let bourrasque = weapon_muzzle_tint(&WeaponType::AssaultRifle);
        let lenoir = weapon_muzzle_tint(&WeaponType::Shotgun);
        let boucherie = weapon_muzzle_tint(&WeaponType::RocketLauncher);
        assert!(pepin.blue > pepin.red, "Pépin = électrique → bleu dominant");
        assert!(
            bourrasque.red > bourrasque.blue,
            "Bourrasque = feu → rouge-orange dominant"
        );
        assert!(lenoir.blue > lenoir.red, "Lenoir = perce → cyan froid");
        assert!(
            boucherie.green > boucherie.red && boucherie.green > boucherie.blue,
            "Boucherie = poison → VERT dominant"
        );
    }

    #[test]
    fn impact_tint_matches_weapon_element() {
        // story-659 — impacts alignés éléments (miroir muzzle, plus sobres).
        let pepin = weapon_impact_tint(&WeaponType::ModernAR);
        let boucherie = weapon_impact_tint(&WeaponType::RocketLauncher);
        let lenoir = weapon_impact_tint(&WeaponType::Shotgun);
        assert!(pepin.blue > pepin.red, "Pépin impact électrique");
        assert!(
            boucherie.green > boucherie.red,
            "Boucherie impact poison vert"
        );
        assert!(lenoir.blue >= lenoir.red, "Lenoir impact cyan froid");
    }

    #[test]
    fn smoke_lifetime_sniper_longest() {
        // Lenoir (Shotgun enum) doit avoir le trail le plus long — signature one-shot.
        let lenoir = weapon_muzzle_smoke_lifetime(&WeaponType::Shotgun);
        let pepin = weapon_muzzle_smoke_lifetime(&WeaponType::ModernAR);
        let bourrasque = weapon_muzzle_smoke_lifetime(&WeaponType::AssaultRifle);
        assert!(lenoir > pepin, "sniper trail > pistolet");
        assert!(lenoir > bourrasque, "sniper trail > SMG");
        assert!(
            bourrasque < pepin,
            "SMG plus court que pistolet (anti accumulation)"
        );
    }

    #[test]
    fn muzzle_light_intensity_melee_zero() {
        assert_eq!(
            weapon_muzzle_light_intensity(&WeaponType::Chainsaw),
            0.0,
            "mêlée : pas de PointLight"
        );
    }

    #[test]
    fn muzzle_light_intensity_smg_lowest_of_firearms() {
        // Anti eye-strain : SMG full-auto 16Hz × intensity élevée = strobe désagréable.
        let smg = weapon_muzzle_light_intensity(&WeaponType::AssaultRifle);
        let pepin = weapon_muzzle_light_intensity(&WeaponType::ModernAR);
        let lenoir = weapon_muzzle_light_intensity(&WeaponType::Shotgun);
        let boucherie = weapon_muzzle_light_intensity(&WeaponType::RocketLauncher);
        assert!(smg < pepin, "SMG < pistolet (anti strobe 16Hz)");
        assert!(smg < lenoir);
        assert!(smg < boucherie);
        // Et tous restent sous le threshold "casse Atmosphere" (~10k lumens empirique).
        for v in [smg, pepin, lenoir, boucherie] {
            assert!(v < 10_000.0, "intensité doit rester subtle, eut {v}");
        }
    }

    #[test]
    fn muzzle_scale_melee_zero() {
        assert_eq!(weapon_muzzle_scale(&WeaponType::Chainsaw), 0.0);
    }
}
