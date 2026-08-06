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
pub fn poisson_disk_annulus(r_min: f32, r_max: f32, min_dist: f32, seed: u64) -> Vec<(f32, f32)> {
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

// ─── La géométrie POSÉE, et ce qu'on en mesure ───────────────────────────────
//
// Ce bloc existe parce que le capteur d'arène ne mesurait que les modules du
// solveur — lequel ne produit rien sur aucune des quatre cartes. Il se déclarait
// « ok » avec toutes ses métriques à zéro : le capteur aveugle que
// `map-design-patterns.md` §13 interdit explicitement.
//
// Les primitives sont ici, pures et testables : `forgia-stage` les alimente avec
// la géométrie réelle (pièces autorées, murs de pièces, modules) et le mode
// roguelite y ajoute son décor.

/// Hauteur de l'œil du joueur (m) — capsule 2,0 m, MESURÉ.
pub const EYE_HEIGHT_M: f32 = 1.70;

/// Hauteur à partir de laquelle un solide CASSE la ligne de vue (m).
///
/// L'œil est à [`EYE_HEIGHT_M`] et il n'y a **pas d'accroupissement** : en
/// dessous, un obstacle masque le corps sans masquer la vue — il ne sert à rien
/// (`map-design-patterns.md` §11). Ce n'est pas un réglage, c'est la géométrie
/// du personnage.
pub const SIGHT_BREAK_H_M: f32 = 1.80;

/// Un solide de l'arène réduit à ce qui décide du gameplay : son emprise au sol
/// et sa hauteur. Un prop, une pièce autorée, un module — tous se ramènent là.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidDisc {
    pub x: f32,
    pub z: f32,
    /// Rayon d'emprise au sol (m) — une EMPRISE mesurée, jamais un coefficient
    /// de tuning (`spawn-clearance.md` §4).
    pub r: f32,
    /// Hauteur en jeu (m). C'est elle qui décide du rôle, pas le nom de l'asset.
    pub h: f32,
}

impl SolidDisc {
    /// Casse-t-il la ligne de vue ? Dérivé de la hauteur, jamais déclaré.
    pub fn breaks_sight(&self) -> bool {
        self.h >= SIGHT_BREAK_H_M
    }
}

/// Un tronçon de mur — la seule chose de l'arène qui n'est pas un disque.
///
/// Le réduire à un disque le rendrait soit troué (rayon = demi-épaisseur), soit
/// énorme (rayon = demi-longueur). Le personnage est un disque
/// (`map-design-patterns.md` §1) ; un mur, lui, est un segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidSeg {
    pub x0: f32,
    pub z0: f32,
    pub x1: f32,
    pub z1: f32,
    pub half_thick_m: f32,
    pub h: f32,
}

impl SolidSeg {
    pub fn breaks_sight(&self) -> bool {
        self.h >= SIGHT_BREAK_H_M
    }
}

/// Distance origine → premier point du disque, le long de `dir` (unitaire).
///
/// `None` si le rayon ne touche pas, ou si le disque est derrière. Une origine
/// À L'INTÉRIEUR du disque rend `0.0` : on est déjà à couvert.
fn ray_hits_disc(ox: f32, oz: f32, dx: f32, dz: f32, d: &SolidDisc) -> Option<f32> {
    let cx = d.x - ox;
    let cz = d.z - oz;
    // Projection du centre sur le rayon.
    let t = cx * dx + cz * dz;
    let perp2 = (cx * cx + cz * cz) - t * t;
    let r2 = d.r * d.r;
    if perp2 > r2 {
        return None; // le rayon passe à côté
    }
    let back = (r2 - perp2).max(0.0).sqrt();
    let enter = t - back;
    if enter >= 0.0 {
        Some(enter)
    } else if t + back >= 0.0 {
        Some(0.0) // origine dedans
    } else {
        None // entièrement derrière
    }
}

/// Distance origine → intersection avec le segment épaissi, le long de `dir`.
///
/// Le segment est traité comme une capsule 2D d'épaisseur `half_thick_m` : c'est
/// la même forme que le collider cuboïde posé en jeu, à l'arrondi des coins près.
fn ray_hits_seg(ox: f32, oz: f32, dx: f32, dz: f32, s: &SolidSeg) -> Option<f32> {
    // Échantillonnage de la capsule par disques : exact au pas choisi, et
    // surtout HONNÊTE — un tronçon de 12 m testé au demi-rayon ne laisse pas
    // passer un rayon entre deux échantillons.
    let ex = s.x1 - s.x0;
    let ez = s.z1 - s.z0;
    let len = (ex * ex + ez * ez).sqrt();
    if len < 1e-4 {
        return ray_hits_disc(
            ox,
            oz,
            dx,
            dz,
            &SolidDisc {
                x: s.x0,
                z: s.z0,
                r: s.half_thick_m,
                h: s.h,
            },
        );
    }
    let step = s.half_thick_m.max(0.05);
    let n = (len / step).ceil() as u32;
    let mut best: Option<f32> = None;
    for i in 0..=n {
        let f = i as f32 / n as f32;
        let hit = ray_hits_disc(
            ox,
            oz,
            dx,
            dz,
            &SolidDisc {
                x: s.x0 + ex * f,
                z: s.z0 + ez * f,
                r: s.half_thick_m,
                h: s.h,
            },
        );
        if let Some(t) = hit {
            best = Some(best.map_or(t, |b: f32| b.min(t)));
        }
    }
    best
}

/// Le PROFIL DE PORTÉES d'un point de l'arène.
///
/// `map-design-intention.md` §1 : « une carte n'a pas une taille, elle a un
/// profil de portées qui doit correspondre à l'arsenal ». Une seule ligne
/// mesurée (l'axe joueur↔boss) ne pouvait pas répondre à ça.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SightlineProfile {
    /// Combien de rayons ont été lancés. **Zéro = aveugle, jamais vert.**
    pub rays: u32,
    /// Portée médiane (m) — la distance de combat typique depuis ce point.
    pub median_m: f32,
    /// La plus longue ligne non brisée (m).
    pub max_m: f32,
    /// Part des rayons qui dépassent le seuil de falloff de l'arsenal.
    pub frac_over_threshold: f32,
}

/// Lance `rays` rayons répartis et mesure jusqu'au premier solide qui casse la
/// vue, borné par `max_m` (le rempart).
///
/// Les solides plus bas que [`SIGHT_BREAK_H_M`] sont ignorés : ils masquent le
/// corps sans masquer la vue, les compter gonflerait le profil d'obstacles qui
/// n'arrêtent aucun tir.
pub fn sightline_profile(
    ox: f32,
    oz: f32,
    discs: &[SolidDisc],
    segs: &[SolidSeg],
    max_m: f32,
    rays: u32,
    over_threshold_m: f32,
) -> SightlineProfile {
    if rays == 0 || max_m <= 0.0 {
        return SightlineProfile {
            rays: 0,
            median_m: 0.0,
            max_m: 0.0,
            frac_over_threshold: 0.0,
        };
    }
    let mut hits: Vec<f32> = Vec::with_capacity(rays as usize);
    for i in 0..rays {
        let a = std::f32::consts::TAU * (i as f32) / (rays as f32);
        let (dz, dx) = a.sin_cos();
        let mut best = max_m;
        for d in discs.iter().filter(|d| d.breaks_sight()) {
            if let Some(t) = ray_hits_disc(ox, oz, dx, dz, d) {
                if t < best {
                    best = t;
                }
            }
        }
        for s in segs.iter().filter(|s| s.breaks_sight()) {
            if let Some(t) = ray_hits_seg(ox, oz, dx, dz, s) {
                if t < best {
                    best = t;
                }
            }
        }
        hits.push(best);
    }
    let over = hits.iter().filter(|d| **d > over_threshold_m).count() as f32;
    let longest = hits.iter().copied().fold(0.0_f32, f32::max);
    hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = hits[hits.len() / 2];
    SightlineProfile {
        rays,
        median_m: median,
        max_m: longest,
        frac_over_threshold: over / rays as f32,
    }
}

/// Rayon du disque autour de `(ox, oz)` qui ne contient AUCUN abri (m).
///
/// C'est la mesure du « stand de tir » : la zone où l'on se bat sans qu'aucun
/// repli existe. Bornée par `max_m` — au-delà, c'est l'arène qui s'arrête, pas
/// le couvert qui manque.
pub fn open_radius_m(ox: f32, oz: f32, discs: &[SolidDisc], segs: &[SolidSeg], max_m: f32) -> f32 {
    let mut best = max_m;
    for d in discs.iter().filter(|d| d.breaks_sight()) {
        let dx = d.x - ox;
        let dz = d.z - oz;
        let surface = ((dx * dx + dz * dz).sqrt() - d.r).max(0.0);
        if surface < best {
            best = surface;
        }
    }
    for s in segs.iter().filter(|s| s.breaks_sight()) {
        // Distance point↔segment, puis retrait de la demi-épaisseur.
        let ex = s.x1 - s.x0;
        let ez = s.z1 - s.z0;
        let l2 = ex * ex + ez * ez;
        let t = if l2 < 1e-6 {
            0.0
        } else {
            (((ox - s.x0) * ex + (oz - s.z0) * ez) / l2).clamp(0.0, 1.0)
        };
        let px = s.x0 + ex * t;
        let pz = s.z0 + ez * t;
        let dx = px - ox;
        let dz = pz - oz;
        let surface = ((dx * dx + dz * dz).sqrt() - s.half_thick_m).max(0.0);
        if surface < best {
            best = surface;
        }
    }
    best
}

/// Distance minimale entre deux abris (m). `f32::INFINITY` si moins de deux.
///
/// Le `INFINITY` n'est pas un résultat, c'est un **aveu d'échantillon vide** :
/// le capteur doit l'exposer comme tel, pas le sérialiser en `-1`.
pub fn min_spacing_m(points: &[(f32, f32)]) -> f32 {
    if points.len() < 2 {
        return f32::INFINITY;
    }
    let mut min_d = f32::INFINITY;
    for i in 0..points.len() {
        for j in (i + 1)..points.len() {
            let dx = points[i].0 - points[j].0;
            let dz = points[i].1 - points[j].1;
            let d = (dx * dx + dz * dz).sqrt();
            if d < min_d {
                min_d = d;
            }
        }
    }
    min_d
}

#[cfg(test)]
mod geometry_tests {
    use super::*;

    fn tall(x: f32, z: f32, r: f32) -> SolidDisc {
        SolidDisc {
            x,
            z,
            r,
            h: SIGHT_BREAK_H_M + 1.0,
        }
    }

    #[test]
    fn a_low_prop_never_breaks_the_sight_line() {
        // La bande 1,2-1,7 m masque le corps sans masquer la vue.
        let low = SolidDisc {
            x: 5.0,
            z: 0.0,
            r: 2.0,
            h: 1.5,
        };
        assert!(!low.breaks_sight());
        let p = sightline_profile(0.0, 0.0, &[low], &[], 60.0, 8, 30.0);
        // Rien n'arrête le regard : tous les rayons vont au bout.
        assert_eq!(p.max_m, 60.0);
        assert_eq!(p.median_m, 60.0);
    }

    #[test]
    fn a_tall_prop_shortens_the_ray_that_meets_it() {
        // Un mur à 10 m, rayon 2 m → le rayon vers +X s'arrête à 8 m.
        let p = sightline_profile(0.0, 0.0, &[tall(10.0, 0.0, 2.0)], &[], 60.0, 4, 30.0);
        // 4 rayons : +X touche à 8 m, les 3 autres vont au bout.
        assert!((p.max_m - 60.0).abs() < 1e-3);
        assert!(p.frac_over_threshold < 1.0, "un rayon au moins est court");
    }

    #[test]
    fn an_empty_arena_reports_its_full_extent_on_every_ray() {
        let p = sightline_profile(0.0, 0.0, &[], &[], 70.0, 16, 30.0);
        assert_eq!(p.rays, 16);
        assert!((p.median_m - 70.0).abs() < 1e-3);
        assert!((p.frac_over_threshold - 1.0).abs() < 1e-3);
    }

    #[test]
    fn zero_rays_is_blind_not_perfect() {
        // Un profil sans rayon ne doit PAS ressembler à une arène parfaite.
        let p = sightline_profile(0.0, 0.0, &[tall(5.0, 0.0, 1.0)], &[], 70.0, 0, 30.0);
        assert_eq!(p.rays, 0);
        assert_eq!(p.max_m, 0.0);
    }

    #[test]
    fn a_wall_segment_blocks_along_its_whole_length() {
        // Mur de 24 m le long de X, à z = 6 m. Un rayon lancé vers +Z le touche
        // à 6 m — et un échantillonnage trop grossier le raterait.
        let w = SolidSeg {
            x0: -12.0,
            z0: 6.0,
            x1: 12.0,
            z1: 6.0,
            half_thick_m: 0.2,
            h: 4.0,
        };
        let p = sightline_profile(0.0, 0.0, &[], &[w], 60.0, 4, 30.0);
        // Le rayon +Z (indice 1 sur 4) bute à ~6 m.
        assert!(p.median_m <= 60.0);
        let open = open_radius_m(0.0, 0.0, &[], &[w], 60.0);
        assert!(
            (open - 5.8).abs() < 0.1,
            "distance à la SURFACE du mur, pas à son axe : {open}"
        );
    }

    #[test]
    fn open_radius_measures_to_the_surface_not_the_center() {
        // Le personnage est un disque : on mesure une distance à la surface.
        let d = tall(10.0, 0.0, 3.0);
        let open = open_radius_m(0.0, 0.0, &[d], &[], 70.0);
        assert!((open - 7.0).abs() < 1e-3, "{open}");
    }

    #[test]
    fn open_radius_ignores_props_too_low_to_hide_behind() {
        let low = SolidDisc {
            x: 4.0,
            z: 0.0,
            r: 1.0,
            h: 1.2,
        };
        assert!((open_radius_m(0.0, 0.0, &[low], &[], 70.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn min_spacing_of_a_single_cover_is_infinite_not_zero() {
        assert!(min_spacing_m(&[(0.0, 0.0)]).is_infinite());
        assert!(min_spacing_m(&[]).is_infinite());
        assert!((min_spacing_m(&[(0.0, 0.0), (3.0, 4.0)]) - 5.0).abs() < 1e-3);
    }
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
        assert!(
            !pts.is_empty(),
            "un anneau de 20-60 m doit accueillir des points"
        );
        for (x, z) in &pts {
            let d = (x * x + z * z).sqrt();
            assert!(
                d >= 20.0 - 1e-3 && d <= 60.0 + 1e-3,
                "point hors anneau : {d}"
            );
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
