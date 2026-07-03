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
}

/// Marker: muzzle VFX entity (for cleanup)
#[derive(Component)]
pub struct MuzzleVfxMarker;

/// Marker: impact VFX entity (for cleanup)
#[derive(Component)]
pub struct ImpactVfxMarker;

/// Construit tous les EffectAssets selon le tuning (Startup + rebuild hot-reload).
fn build_weapon_vfx(
    effects: &mut ResMut<Assets<EffectAsset>>,
    asset_server: &AssetServer,
    t: &VfxTuning,
) -> WeaponVfxEffects {
    WeaponVfxEffects {
        muzzle_core_flash: muzzle::create_muzzle_core_flash(effects, t),
        muzzle_sparks: muzzle::create_muzzle_sparks(effects, t),
        muzzle_smoke: muzzle::create_muzzle_smoke(effects, t),
        muzzle_heat_glow: muzzle::create_muzzle_heat_glow(effects, t),
        muzzle_forward_flash: muzzle::create_muzzle_forward_flash(effects, t),
        impact_sparks: impact::create_impact_sparks(effects, t),
        impact_dust: impact::create_impact_dust(effects, t),
        impact_flash: impact::create_impact_flash(effects, t),
        status_flame: status::create_status_flame(effects, t),
        status_poison_cloud: status::create_status_poison_cloud(effects, t),
        status_shock: status::create_status_shock(effects, t),
        impact_offset_m: t.impact_offset_m,
        lifetime_mult: t.lifetime_mult,
        kill_burst: impact::create_kill_burst(effects, t),
        tex_muzzle_flash: asset_server.load(tex_paths::MUZZLE_FLASH),
        tex_muzzle_tongue: asset_server.load(tex_paths::MUZZLE_TONGUE),
        tex_spark: asset_server.load(tex_paths::SPARK),
        tex_smoke: asset_server.load(tex_paths::SMOKE),
        tex_dust: asset_server.load(tex_paths::DUST),
        tex_glow: asset_server.load(tex_paths::GLOW),
        tex_flare: asset_server.load(tex_paths::FLARE),
        tex_flame: asset_server.load(tex_paths::FLAME),
        tex_poison: asset_server.load(tex_paths::POISON),
        tex_burst: asset_server.load(tex_paths::BURST),
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
    let vfx = build_weapon_vfx(&mut effects, &asset_server, &t);
    commands.insert_resource(vfx);
    commands.insert_resource(t);
    commands.insert_resource(VfxGenomeWatch {
        last_mtime: mtime,
        ..Default::default()
    });
    info!(
        "Weapon VFX initialises (5-layer muzzle + 3-layer impact + kill burst, tuning ×{:.1} taille ×{:.1} qté — story-652)",
        t.size_mult, t.count_mult
    );
}

/// Story-652 — hot-reload du tuning VFX : RECONSTRUIT les EffectAssets au
/// changement du TOML (1Hz). Les instances déjà spawnées gardent l'ancien
/// asset jusqu'à leur despawn (<1 s) ; petit hitch de compile shader possible —
/// outil de tuning en session, pas un toggle de combat.
pub fn sys_hot_reload_vfx_genome(
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
        *vfx = build_weapon_vfx(&mut effects, &asset_server, &new_tuning);
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!(
            "[weapon-vfx] HOT-RELOADED — taille ×{:.1} qté ×{:.1} durée ×{:.1} (assets reconstruits)",
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
    if let Err(e) = std::fs::write(VFX_SENSOR_PATH, &json) {
        warn!("[weapon-vfx] sensor write failed: {e}");
    }
}

/// Story-652 — burst de kill au point d'impact : volutes chaudes + PointLight
/// bref tinté par l'arme. Appelé par le fire path à l'edge vivant→mort.
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
    commands.spawn((
        ParticleEffect::new(effects.kill_burst.clone()),
        EffectMaterial {
            images: vec![effects.tex_burst.clone()],
        },
        Transform::from_translation(pos).with_scale(scale_v),
        ImpactVfxMarker,
        Lifetime(Timer::from_seconds(1.2 * effects.lifetime_mult, TimerMode::Once)),
    ));
    // Lumière du kill : plus forte que l'impact standard (l'événement), toujours
    // bornée (leçon anti-casse-Atmosphère, cf weapon_muzzle_light_intensity).
    let tint = weapon_impact_tint(weapon);
    commands.spawn((
        PointLight {
            color: Color::LinearRgba(tint),
            intensity: 6_000.0,
            range: 5.0,
            radius: 0.1,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(pos),
        ImpactVfxMarker,
        Lifetime(Timer::from_seconds(0.08, TimerMode::Once)),
    ));
}

/// Spawn all 5 muzzle flash layers at the given barrel tip position.
/// `shot_dir` orients the forward flash tongue.
/// `weapon` détermine le scale (Shotgun/Rocket = plus gros, mêlée = skip).
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

    // Transform oriented along shot direction
    let flash_tf = Transform::from_translation(barrel_tip)
        .looking_to(shot_dir, Vec3::Y)
        .with_scale(scale_v);

    // Layer 1: Core flash (world-space, at barrel tip)
    commands.spawn((
        ParticleEffect::new(effects.muzzle_core_flash.clone()),
        EffectMaterial {
            images: vec![effects.tex_muzzle_flash.clone()],
        },
        Transform::from_translation(barrel_tip).with_scale(scale_v),
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(0.15 * effects.lifetime_mult, TimerMode::Once)),
    ));

    // Layer 2: Spark spray (world-space)
    commands.spawn((
        ParticleEffect::new(effects.muzzle_sparks.clone()),
        EffectMaterial {
            images: vec![effects.tex_spark.clone()],
        },
        Transform::from_translation(barrel_tip).with_scale(scale_v),
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(0.5 * effects.lifetime_mult, TimerMode::Once)),
    ));

    // Layer 3: Smoke puff (world-space, lingers — per-arme : sniper = trail long)
    commands.spawn((
        ParticleEffect::new(effects.muzzle_smoke.clone()),
        EffectMaterial {
            images: vec![effects.tex_smoke.clone()],
        },
        Transform::from_translation(barrel_tip + shot_dir * 0.05).with_scale(scale_v),
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(
            weapon_muzzle_smoke_lifetime(weapon) * effects.lifetime_mult,
            TimerMode::Once,
        )),
    ));

    // Layer 4: Heat glow — story-450 (2026-05-18) : RE-ENABLED après shrink x3
    // (0.10-0.20 vs 0.30-0.60) + alpha /2. Halo bloom subtil qui ajoute la
    // qualité AAA "weight feel" sans bloquer la cible. Coût hanabi acceptable
    // (12 particles × 0.07s = ~0.84 particles/s steady-state en auto-fire).
    commands.spawn((
        ParticleEffect::new(effects.muzzle_heat_glow.clone()),
        EffectMaterial {
            images: vec![effects.tex_glow.clone()],
        },
        Transform::from_translation(barrel_tip).with_scale(scale_v),
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(0.12 * effects.lifetime_mult, TimerMode::Once)),
    ));

    // Layer 5: Forward flash tongue (oriented along barrel)
    commands.spawn((
        ParticleEffect::new(effects.muzzle_forward_flash.clone()),
        EffectMaterial {
            images: vec![effects.tex_muzzle_tongue.clone()],
        },
        flash_tf,
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(0.15 * effects.lifetime_mult, TimerMode::Once)),
    ));

    // Layer 6: PointLight bref tinté per-arme (Phase 5 dette tech 2026-05-18).
    // Historique : un PointLight plein-intensité cassait l'Atmosphere. Solution :
    // intensity faible (2k-8k lumens, vs 100k+ "réaliste"), lifetime ultra-court
    // (50-80ms), range borné. Donne signature couleur sans bloom run-away.
    let light_intensity = weapon_muzzle_light_intensity(weapon);
    if light_intensity > 0.0 {
        let tint = weapon_muzzle_tint(weapon);
        commands.spawn((
            PointLight {
                color: Color::LinearRgba(tint),
                intensity: light_intensity,
                range: 4.0,
                radius: 0.05,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(barrel_tip),
            MuzzleVfxMarker,
            Lifetime(Timer::from_seconds(0.07, TimerMode::Once)),
        ));
    }
}

/// Spawn impact VFX at hit point: 3 particle layers + point light.
/// `weapon` détermine le scale (Rocket = gros plume, sniper = précis, SMG = standard).
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

    // Layer 1: Sparks (hemisphere burst)
    commands.spawn((
        ParticleEffect::new(effects.impact_sparks.clone()),
        EffectMaterial {
            images: vec![effects.tex_spark.clone()],
        },
        Transform::from_translation(impact_pos).with_scale(scale_v),
        ImpactVfxMarker,
        Lifetime(Timer::from_seconds(0.6 * effects.lifetime_mult, TimerMode::Once)),
    ));

    // Layer 2: Dust cloud
    commands.spawn((
        ParticleEffect::new(effects.impact_dust.clone()),
        EffectMaterial {
            images: vec![effects.tex_dust.clone()],
        },
        Transform::from_translation(impact_pos).with_scale(scale_v),
        ImpactVfxMarker,
        Lifetime(Timer::from_seconds(1.0 * effects.lifetime_mult, TimerMode::Once)),
    ));

    // Layer 3: Flash burst — story-432 V5-A (2026-05-13) : DISABLED pour
    // réduire le coût spawn hanabi sur burst hits (combat_hit×3 dans 1 frame
    // → 9 spawn). Lifetime 0.1s ultra court = marginal visuel, drop = -33%
    // cost impact. Sparks + dust suffisent au feedback.
    // commands.spawn((
    //     ParticleEffect::new(effects.impact_flash.clone()),
    //     Transform::from_translation(impact_pos),
    //     ImpactVfxMarker,
    //     Lifetime(Timer::from_seconds(0.1, TimerMode::Once)),
    // ));

    // Layer 4: PointLight bref tinté per-arme (Phase 5 dette tech 2026-05-18).
    // Intensity ~50% du muzzle (impact = secondaire feedback), lifetime 40ms.
    let tint = weapon_impact_tint(weapon);
    let intensity = match weapon {
        WeaponType::RocketLauncher => 5_000.0,
        WeaponType::Shotgun => 3_000.0,
        _ => 1_500.0,
    };
    commands.spawn((
        PointLight {
            color: Color::LinearRgba(tint),
            intensity,
            range: 3.0,
            radius: 0.05,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(impact_pos),
        ImpactVfxMarker,
        Lifetime(Timer::from_seconds(0.04, TimerMode::Once)),
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
