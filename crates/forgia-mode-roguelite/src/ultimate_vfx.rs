//! ultimate_vfx.rs — VFX des techniques d'Ultime (story-596 T4b, 1re passe).
//!
//! Rend les techniques d'Ultime VISIBLES sans asset externe : un **flash émissif**
//! (sphère colorée, fade par scale + lumière) par technique, aux positions
//! touchées. Réutilise le moteur de `element_vfx.rs` :
//! - **mesh sphère + 4 matériaux partagés** (1 par technique) → 0 alloc/hit,
//! - le composant [`ElementSpark`] (animé/despawné par
//!   `element_vfx::sys_tick_element_sparks`, déjà enregistré).
//!
//! Event-driven : `ultimate_apply::sys_apply_ultimate_technique` émet des
//! [`UltimateVfxEvent`] (gameplay découplé du rendu) ; [`sys_spawn_ultimate_vfx`]
//! les consomme. Couleurs/tailles data-driven (genome `[vfx]`, hot-reload).
//!
//! 1re passe = placeholder « qui pop » (sphères émissives, bloom). Polish futur :
//! beams d'arc/perforation, sprite-sheets, particules Hanabi.

use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use forgia_combat::weapons::WeaponType;
use forgia_core::prelude::*;

use crate::element_vfx::ElementSpark;
use crate::ultimate_config::UltimateConfig;

/// Garde-fou anti-spam (cadence auto Bourrasque pendant 10 s). Partagé avec les
/// sparks d'éléments via le même composant → on compte tous les `ElementSpark`.
const MAX_ACTIVE_SPARKS: usize = 96;
/// Boost émissif (bloom-friendly), aligné sur `element_vfx::EMISSIVE_BOOST`.
const EMISSIVE_BOOST: f32 = 3.0;

/// Index couleur d'une technique (aligné sur l'ordre du genome `[vfx]`).
pub const VFX_EXPLOSION: usize = 0;
pub const VFX_CHAIN: usize = 1;
pub const VFX_PIERCE: usize = 2;
pub const VFX_FREEZE: usize = 3;

/// Demande de flash VFX émise par une technique d'Ultime au moment où elle frappe.
#[derive(Message, Debug, Clone, Copy)]
pub struct UltimateVfxEvent {
    pub pos: Vec3,
    /// `VFX_EXPLOSION` / `VFX_CHAIN` / `VFX_PIERCE` / `VFX_FREEZE`.
    pub technique: usize,
    /// Rayon (m) de la sphère de flash.
    pub scale: f32,
    /// Attache une lumière ponctuelle (réservé aux gros impacts type explosion).
    pub light: bool,
}

/// Mesh + 4 matériaux partagés (1 par technique), construits une fois.
#[derive(Resource)]
pub struct UltimateVfxAssets {
    pub sphere: Handle<Mesh>,
    /// Indexé par `VFX_*` (explosion/chaîne/perfo/gel).
    pub mats: [Handle<StandardMaterial>; 4],
    /// Couleurs RGB (pour la teinte des lumières).
    pub rgb: [[f32; 3]; 4],
}

/// Configure un matériau (unlit + émissif coloré + blend). Partagé → appelé à
/// l'init ET au hot-reload (mêmes handles → les flashs vivants changent aussi).
fn apply_vfx_material(m: &mut StandardMaterial, rgb: [f32; 3]) {
    let [r, g, b] = rgb;
    m.base_color = Color::srgba(r, g, b, 0.85);
    m.emissive = LinearRgba::new(r * EMISSIVE_BOOST, g * EMISSIVE_BOOST, b * EMISSIVE_BOOST, 1.0);
    m.unlit = true;
    m.alpha_mode = AlphaMode::Blend;
}

fn colors(cfg: &UltimateConfig) -> [[f32; 3]; 4] {
    [
        cfg.vfx.explosion_rgb,
        cfg.vfx.chain_rgb,
        cfg.vfx.pierce_rgb,
        cfg.vfx.freeze_rgb,
    ]
}

/// Index de technique de l'arme équipée (None si non mappée). Sert au HUD et à
/// la lueur d'arme pour lier la couleur à l'effet produit.
pub fn weapon_technique_idx(w: WeaponType) -> Option<usize> {
    match w {
        WeaponType::ModernAR => Some(VFX_EXPLOSION),
        WeaponType::AssaultRifle => Some(VFX_CHAIN),
        WeaponType::Shotgun => Some(VFX_PIERCE),
        WeaponType::RocketLauncher => Some(VFX_FREEZE),
        _ => None,
    }
}

/// Couleur RGB de la technique d'Ultime de l'arme (blanc si non mappée).
pub fn weapon_vfx_color(cfg: &UltimateConfig, w: WeaponType) -> [f32; 3] {
    match weapon_technique_idx(w) {
        Some(i) => colors(cfg)[i],
        None => [1.0, 1.0, 1.0],
    }
}

/// Libellé court de la technique (HUD).
pub fn weapon_technique_label(w: WeaponType) -> &'static str {
    match w {
        WeaponType::ModernAR => "EXPLOSION",
        WeaponType::AssaultRifle => "CHAÎNE ÉLEC",
        WeaponType::Shotgun => "PERFORATION",
        WeaponType::RocketLauncher => "GEL",
        _ => "ULTIME",
    }
}

/// Startup (après `ultimate_config::sys_init_ultimate_genome`) : construit le mesh
/// sphère et les 4 matériaux depuis les couleurs du genome. `Option<Res>` avec
/// fallback `default` → robuste à l'ordre d'init des Commands.
pub fn sys_init_ultimate_vfx_assets(
    mut commands: Commands,
    config: Option<Res<UltimateConfig>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cfg = config.map(|c| *c).unwrap_or_default();
    let sphere = meshes.add(Sphere::new(1.0));
    let rgb = colors(&cfg);
    let mats = rgb.map(|c| {
        let mut m = StandardMaterial::default();
        apply_vfx_material(&mut m, c);
        materials.add(m)
    });
    commands.insert_resource(UltimateVfxAssets { sphere, mats, rgb });
}

/// Ré-applique les couleurs en place au hot-reload du genome (mêmes handles).
pub fn sys_refresh_ultimate_vfx_materials(
    config: Res<UltimateConfig>,
    assets: Option<ResMut<UltimateVfxAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !config.is_changed() {
        return;
    }
    let Some(mut assets) = assets else {
        return;
    };
    let rgb = colors(&config);
    assets.rgb = rgb;
    for (i, c) in rgb.iter().enumerate() {
        if let Some(m) = materials.get_mut(&assets.mats[i]) {
            apply_vfx_material(m, *c);
        }
    }
}

/// Consomme les [`UltimateVfxEvent`] et spawn un flash émissif partagé (0 alloc
/// matériau/hit). Cap anti-spam via le compte global de `ElementSpark`. Le fade +
/// despawn sont gérés par `element_vfx::sys_tick_element_sparks` (déjà enregistré).
pub fn sys_spawn_ultimate_vfx(
    mut events: MessageReader<UltimateVfxEvent>,
    config: Res<UltimateConfig>,
    assets: Option<Res<UltimateVfxAssets>>,
    q_sparks: Query<(), With<ElementSpark>>,
    mut commands: Commands,
) {
    let Some(assets) = assets else {
        events.read().for_each(drop);
        return;
    };
    if !config.vfx.enabled {
        events.read().for_each(drop);
        return;
    }
    let mut live = q_sparks.iter().count();
    for ev in events.read() {
        if live >= MAX_ACTIVE_SPARKS {
            continue;
        }
        let idx = ev.technique.min(3);
        let light0 = if ev.light { config.vfx.light_intensity } else { 0.0 };
        let mut ent = commands.spawn((
            Mesh3d(assets.sphere.clone()),
            MeshMaterial3d(assets.mats[idx].clone()),
            Transform::from_translation(ev.pos).with_scale(Vec3::splat(ev.scale)),
            ElementSpark {
                age: 0.0,
                ttl: config.vfx.ttl,
                start_scale: ev.scale,
                light0,
            },
            DespawnOnExit(GameMode::Roguelite),
        ));
        if ev.light {
            let [r, g, b] = assets.rgb[idx];
            ent.insert(PointLight {
                color: Color::srgb(r, g, b),
                intensity: light0,
                range: config.vfx.light_range,
                shadows_enabled: false,
                ..default()
            });
        }
        live += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_apply_sets_emissive_from_rgb() {
        let mut m = StandardMaterial::default();
        apply_vfx_material(&mut m, [1.0, 0.5, 0.0]);
        assert!(m.unlit);
        assert_eq!(m.emissive.red, 1.0 * EMISSIVE_BOOST);
        assert_eq!(m.emissive.green, 0.5 * EMISSIVE_BOOST);
    }

    #[test]
    fn technique_indices_distinct_and_in_range() {
        let idx = [VFX_EXPLOSION, VFX_CHAIN, VFX_PIERCE, VFX_FREEZE];
        assert_eq!(idx, [0, 1, 2, 3]);
    }

    #[test]
    fn colors_map_from_config() {
        let c = UltimateConfig::default();
        let rgb = colors(&c);
        assert_eq!(rgb[VFX_EXPLOSION], c.vfx.explosion_rgb);
        assert_eq!(rgb[VFX_FREEZE], c.vfx.freeze_rgb);
    }

    #[test]
    fn weapon_maps_to_its_technique_color() {
        let c = UltimateConfig::default();
        assert_eq!(weapon_technique_idx(WeaponType::ModernAR), Some(VFX_EXPLOSION));
        assert_eq!(weapon_technique_idx(WeaponType::RocketLauncher), Some(VFX_FREEZE));
        assert_eq!(weapon_vfx_color(&c, WeaponType::AssaultRifle), c.vfx.chain_rgb);
        assert_eq!(weapon_technique_label(WeaponType::Shotgun), "PERFORATION");
    }
}
