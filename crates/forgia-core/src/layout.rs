//! layout.rs — les primitives d'AMÉNAGEMENT partagées (story-674).
//!
//! Deux fonctions, deux consommateurs chacune, et surtout : deux façons de ne plus
//! deviner.
//!
//! - `poisson_disk_sample` — semis à bruit bleu (Bridson). Vivait dans
//!   `forgia-terrain::sampling`, utilisé par la végétation. L'aménagement des
//!   arènes roguelite en avait besoin aussi, et une crate gameplay n'a pas à
//!   dépendre du terrain pour des maths pures. Le chemin d'origine est conservé
//!   par re-export : rien n'a bougé pour la végétation.
//! - `covers_expected` — le NOMBRE d'abris n'est pas une opinion, il se dérive de
//!   la surface et de l'espacement visé. Vivait dans le banc Arena Test, où il
//!   MESURAIT sans jamais s'appliquer au générateur.

/// Generate Poisson disk sample points within a 2D rectangle.
/// Returns Vec<(x, z)> positions in local space [0, width] × [0, depth].
///
/// - `min_dist`: minimum distance between any two points
/// - `seed`: deterministic RNG seed (e.g., derived from chunk coord)
/// - `k`: max attempts per active point (30 is standard)
pub fn poisson_disk_sample(
    width: f32,
    depth: f32,
    min_dist: f32,
    seed: u64,
    k: u32,
) -> Vec<(f32, f32)> {
    let cell_size = min_dist / std::f32::consts::SQRT_2;
    let grid_w = (width / cell_size).ceil() as usize + 1;
    let grid_h = (depth / cell_size).ceil() as usize + 1;
    let mut grid: Vec<Option<usize>> = vec![None; grid_w * grid_h];
    let mut points: Vec<(f32, f32)> = Vec::new();
    let mut active: Vec<usize> = Vec::new();

    let mut rng_state: u64 = seed ^ 0xCAFE_BABE_1337;
    let mut next_f32 = || -> f32 {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        (rng_state & 0xFFFFFF) as f32 / 0xFFFFFF as f32
    };

    // Initial point at center
    let p0 = (width * 0.5, depth * 0.5);
    let gi = (p0.0 / cell_size) as usize;
    let gj = (p0.1 / cell_size) as usize;
    if gi < grid_w && gj < grid_h {
        grid[gi + gj * grid_w] = Some(0);
    }
    points.push(p0);
    active.push(0);

    while !active.is_empty() {
        let ai = (next_f32() * active.len() as f32) as usize % active.len();
        let pi = active[ai];
        let (px, pz) = points[pi];

        let mut found = false;
        for _ in 0..k {
            let angle = next_f32() * std::f32::consts::TAU;
            let r = min_dist + next_f32() * min_dist;
            let nx = px + r * angle.cos();
            let nz = pz + r * angle.sin();

            if nx < 0.0 || nx >= width || nz < 0.0 || nz >= depth {
                continue;
            }

            let ngi = (nx / cell_size) as usize;
            let ngj = (nz / cell_size) as usize;

            // Check neighbors in 5x5 grid
            let mut too_close = false;
            let imin = ngi.saturating_sub(2);
            let imax = (ngi + 3).min(grid_w);
            let jmin = ngj.saturating_sub(2);
            let jmax = (ngj + 3).min(grid_h);

            for jj in jmin..jmax {
                for ii in imin..imax {
                    if let Some(idx) = grid[ii + jj * grid_w] {
                        let (ox, oz) = points[idx];
                        let dx = nx - ox;
                        let dz = nz - oz;
                        if dx * dx + dz * dz < min_dist * min_dist {
                            too_close = true;
                            break;
                        }
                    }
                }
                if too_close {
                    break;
                }
            }

            if !too_close {
                let new_idx = points.len();
                grid[ngi + ngj * grid_w] = Some(new_idx);
                points.push((nx, nz));
                active.push(new_idx);
                found = true;
            }
        }

        if !found {
            active.swap_remove(ai);
        }
    }

    points
}

// ─── Semis dans un disque / un anneau ────────────────────────────────────────

/// Semis à bruit bleu dans un ANNEAU centré sur l'origine.
///
/// Les arènes de Forgia sont des disques, pas des rectangles : on échantillonne
/// le carré englobant puis on filtre par rayon. C'est la façon standard, et elle
/// évite de réécrire Bridson pour une géométrie différente.
///
/// `r_min == 0` donne un disque plein. Les points sortent centrés sur (0,0).
pub fn poisson_disk_annulus(
    r_min: f32,
    r_max: f32,
    min_dist: f32,
    seed: u64,
) -> Vec<(f32, f32)> {
    if r_max <= 0.0 || min_dist <= 0.0 || r_min >= r_max {
        return Vec::new();
    }
    let side = r_max * 2.0;
    let lo = r_min.max(0.0);
    poisson_disk_sample(side, side, min_dist, seed, 30)
        .into_iter()
        .map(|(x, z)| (x - r_max, z - r_max))
        .filter(|(x, z)| {
            let d2 = x * x + z * z;
            d2 >= lo * lo && d2 <= r_max * r_max
        })
        .collect()
}

// ─── Combien d'abris ? ───────────────────────────────────────────────────────

/// Bande d'espacement d'abris sourcée : **3 à 10 m**, 10 m au maximum
/// (Watch Dogs, Gears of War — cf `.claude/rules/map-design-patterns.md` §11).
pub const COVER_SPACING_MIN_M: f32 = 3.0;
pub const COVER_SPACING_MAX_M: f32 = 10.0;

/// Nombre d'abris attendu pour une aire, à l'espacement visé.
///
/// Le compte n'est pas une opinion : il se dérive de la surface jouable. Une
/// arène qui déclare « 34 props » sans regarder son aire ne peut pas être
/// sous-couverte ou sur-couverte « un peu » — elle l'est d'un facteur.
pub fn covers_expected(area_m2: f32, spacing_m: f32) -> f32 {
    if spacing_m <= 0.0 || area_m2 <= 0.0 {
        return 0.0;
    }
    area_m2 / (spacing_m * spacing_m)
}

/// Aire jouable d'un disque de rayon `r` (m²).
pub fn disc_area(r: f32) -> f32 {
    if r <= 0.0 {
        return 0.0;
    }
    std::f32::consts::PI * r * r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_expected_matches_the_sourced_formula() {
        // Valeur de référence du banc Arena Test : 484 m² à 6 m → 13,44.
        assert!((covers_expected(484.0, 6.0) - 13.44).abs() < 0.01);
        // Plus on espace, moins il en faut.
        assert!(covers_expected(484.0, 10.0) < covers_expected(484.0, 6.0));
        // Dégénérés : jamais de NaN ni de négatif.
        assert_eq!(covers_expected(0.0, 6.0), 0.0);
        assert_eq!(covers_expected(484.0, 0.0), 0.0);
    }

    #[test]
    fn the_annulus_respects_its_radii_and_spacing() {
        let pts = poisson_disk_annulus(20.0, 60.0, 8.0, 0xABCD);
        assert!(!pts.is_empty(), "un anneau de 20-60 m doit accueillir des points");
        for (x, z) in &pts {
            let d = (x * x + z * z).sqrt();
            assert!(d >= 20.0 - 1e-3 && d <= 60.0 + 1e-3, "point hors anneau : {d}");
        }
        // Espacement minimal respecté (propriété du bruit bleu).
        for (i, a) in pts.iter().enumerate() {
            for b in pts.iter().skip(i + 1) {
                let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                assert!(d >= 8.0 - 1e-2, "deux points à {d:.2} m, min 8");
            }
        }
    }

    #[test]
    fn a_full_disc_is_an_annulus_with_no_hole() {
        let pts = poisson_disk_annulus(0.0, 30.0, 6.0, 7);
        assert!(!pts.is_empty());
        assert!(pts.iter().any(|(x, z)| (x * x + z * z).sqrt() < 10.0));
    }

    #[test]
    fn degenerate_inputs_yield_nothing_instead_of_panicking() {
        assert!(poisson_disk_annulus(0.0, 0.0, 5.0, 1).is_empty());
        assert!(poisson_disk_annulus(50.0, 10.0, 5.0, 1).is_empty());
        assert!(poisson_disk_annulus(0.0, 30.0, 0.0, 1).is_empty());
    }

    #[test]
    fn the_annulus_is_deterministic_per_seed() {
        assert_eq!(
            poisson_disk_annulus(10.0, 40.0, 7.0, 42),
            poisson_disk_annulus(10.0, 40.0, 7.0, 42)
        );
    }
}
