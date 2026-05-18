//! # forgia-terrain::flatten
//!
//! Terrain leveling local pour villages procgen — applique un disc plat avec
//! falloff smoothstep en bordure. Pattern AAA Skyrim (Burgess GDC 2013) :
//! local flattening disc autour POI, gradient blend vers terrain naturel.
//!
//! ## Pureté
//!
//! [`FlattenZones::sample`] est pure — prend `raw_y` (sortie de `heightmap_at`)
//! et retourne `y_flattened`. Aucune mutation in-place du heightmap source.
//! Le caller (meshing, village spawn, gizmos) wrappe explicitement les appels
//! à `heightmap_at`.
//!
//! ## Smoothstep falloff
//!
//! Pour un point à distance `d` du centre :
//! - `d <= inner_radius` → `y = target_y` (plat parfait)
//! - `inner_radius < d < inner_radius + falloff_radius` → blend smoothstep
//! - `d >= inner_radius + falloff_radius` → `y = raw_y` (terrain naturel)
//!
//! Smoothstep classique `t² (3 - 2t)` pour `t = (d - inner) / falloff`.

use bevy::prelude::*;

/// Une zone plate centrée sur un village. Plusieurs zones peuvent coexister
/// (cas multi-villages, hors scope V1).
#[derive(Debug, Clone, Copy)]
pub struct VillageFlattenZone {
    /// Centre du disc en coordonnées monde XZ (mêmes coords que `heightmap_at`).
    pub center: Vec2,
    /// Altitude cible (m) du plateau plat. Typiquement `heightmap_at(center)` raw.
    pub target_y: f32,
    /// Rayon (m) du plateau parfaitement plat.
    pub inner_radius: f32,
    /// Largeur (m) de la transition smoothstep vers le terrain naturel.
    /// `inner_radius + falloff_radius` = limite extérieure d'influence.
    pub falloff_radius: f32,
}

impl VillageFlattenZone {
    /// Distance d'influence totale (au-delà : aucun effet).
    pub fn outer_radius(&self) -> f32 {
        self.inner_radius + self.falloff_radius
    }

    /// Applique le leveling sur un point monde, retourne le Y résultant.
    /// `raw_y` = sortie de `heightmap_at(x, z, &config)`.
    pub fn sample(&self, x: f32, z: f32, raw_y: f32) -> f32 {
        let d = Vec2::new(x, z).distance(self.center);
        if d <= self.inner_radius {
            self.target_y
        } else if d < self.outer_radius() && self.falloff_radius > f32::EPSILON {
            let t = ((d - self.inner_radius) / self.falloff_radius).clamp(0.0, 1.0);
            // smoothstep : ease-in-out
            let s = t * t * (3.0 - 2.0 * t);
            // s=0 → target_y (intérieur) ; s=1 → raw_y (extérieur)
            self.target_y * (1.0 - s) + raw_y * s
        } else {
            raw_y
        }
    }
}

/// Resource — collection de zones de leveling actives. Inséré par `forgia-rpg`
/// (ou tout caller mode RPG) au moment du spawn village ; cleanup OnExit avec
/// les autres ressources du monde.
#[derive(Resource, Default, Debug, Clone)]
pub struct FlattenZones {
    pub zones: Vec<VillageFlattenZone>,
}

impl FlattenZones {
    /// Resource fresh vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute une zone.
    pub fn push(&mut self, zone: VillageFlattenZone) {
        self.zones.push(zone);
    }

    /// Applique toutes les zones séquentiellement. Si un point est dans
    /// plusieurs discs (chevauchement), la zone trouvée la plus restrictive
    /// (target_y minimum local) gagne. Pour V1 mono-village, séquentiel OK.
    pub fn sample(&self, x: f32, z: f32, raw_y: f32) -> f32 {
        let mut y = raw_y;
        for z_def in &self.zones {
            y = z_def.sample(x, z, y);
        }
        y
    }

    /// True si aucune zone n'influence ce point (early-exit perf).
    pub fn is_outside_all(&self, x: f32, z: f32) -> bool {
        let p = Vec2::new(x, z);
        self.zones
            .iter()
            .all(|zone| p.distance(zone.center) >= zone.outer_radius())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_zone() -> VillageFlattenZone {
        VillageFlattenZone {
            center: Vec2::new(0.0, 0.0),
            target_y: 20.0,
            inner_radius: 10.0,
            falloff_radius: 5.0,
        }
    }

    #[test]
    fn inner_is_flat() {
        let z = sample_zone();
        assert!((z.sample(0.0, 0.0, 5.0) - 20.0).abs() < f32::EPSILON);
        assert!((z.sample(7.0, 0.0, 50.0) - 20.0).abs() < f32::EPSILON);
        assert!((z.sample(0.0, -10.0, 0.0) - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn outer_is_raw() {
        let z = sample_zone();
        // d = 15 = inner + falloff = limite. Au-delà : raw.
        assert!((z.sample(16.0, 0.0, 5.0) - 5.0).abs() < 0.001);
        assert!((z.sample(100.0, 0.0, 42.0) - 42.0).abs() < f32::EPSILON);
    }

    #[test]
    fn falloff_smoothstep_monotonic() {
        let z = sample_zone();
        let mut last = z.sample(10.5, 0.0, 5.0);
        // En sortant du disc inner (10.5 → 14.5), y doit décroître de 20 vers 5.
        for d in 110..=145 {
            let dist = d as f32 * 0.1;
            let y = z.sample(dist, 0.0, 5.0);
            assert!(y <= last + 0.01, "should be monotonic decreasing : d={} last={} y={}", dist, last, y);
            assert!(y >= 5.0 - 0.01 && y <= 20.0 + 0.01);
            last = y;
        }
    }

    #[test]
    fn zero_falloff_hard_cutoff() {
        let z = VillageFlattenZone {
            center: Vec2::ZERO,
            target_y: 20.0,
            inner_radius: 10.0,
            falloff_radius: 0.0,
        };
        assert_eq!(z.sample(9.0, 0.0, 5.0), 20.0);
        assert_eq!(z.sample(11.0, 0.0, 5.0), 5.0);
    }

    #[test]
    fn empty_zones_passthrough() {
        let zones = FlattenZones::new();
        assert_eq!(zones.sample(123.0, -45.0, 7.5), 7.5);
    }

    #[test]
    fn multiple_zones_chained() {
        let mut zones = FlattenZones::new();
        zones.push(VillageFlattenZone {
            center: Vec2::ZERO,
            target_y: 10.0,
            inner_radius: 5.0,
            falloff_radius: 2.0,
        });
        zones.push(VillageFlattenZone {
            center: Vec2::new(100.0, 0.0),
            target_y: 30.0,
            inner_radius: 5.0,
            falloff_radius: 2.0,
        });
        // Au centre de zone 0 : y = 10 (peu importe la 2e, elle est loin)
        assert_eq!(zones.sample(0.0, 0.0, 50.0), 10.0);
        // Au centre de zone 1 : y = 30
        assert_eq!(zones.sample(100.0, 0.0, 50.0), 30.0);
        // Loin des deux : raw
        assert_eq!(zones.sample(50.0, 0.0, 7.0), 7.0);
    }
}
