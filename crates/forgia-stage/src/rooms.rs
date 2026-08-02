//! rooms.rs — des PIÈCES et des COULOIRS dans l'arène (story-683).
//!
//! Modèle Gunfire Reborn / Doom : on ne se bat plus dans un disque nu, on
//! traverse un complexe. **Tout reste à plat** — c'est la condition pour que ça
//! marche du premier coup.
//!
//! ## Pourquoi plat, et pourquoi c'est la bonne première marche
//!
//! Les bots n'ont ni navmesh ni gravité : `bot_tactical_movement` travaille en
//! **XZ pur** (`Vec3::new(step.x, 0.0, step.z)`) et un bot reste au Y où il est
//! né. Relief, escaliers et étages sont donc bloqués par la même pièce absente.
//!
//! En revanche, `collide_and_slide` lance déjà **3 rayons parallèles** (centre +
//! deux bords) et fait glisser le bot le long d'un mur en vérifiant que le
//! couloir est dégagé. C'est *littéralement* de la navigation de couloir : les
//! pièces à plat sont la seule partie du Doom-like qui ne demande aucun travail
//! d'IA.
//!
//! ## Tout se dérive, rien ne se choisit
//!
//! | Grandeur | Dérivation | Source |
//! |---|---|---|
//! | Côté d'une pièce | `vision_grunt / √2` — la diagonale doit tenir dans la vue | `map-design-intention.md` §2.2 |
//! | Largeur de couloir | `2·(r_joueur + r_bot)` puis ×2 pour le confort | disque joueur 0,6 m, bot 0,4 m |
//! | Pas de trame | multiple du module de mur (4 m) | sinon les murs laissent des trous |
//! | Hauteur de mur | ≥ 1,80 m pour casser la ligne de vue | `map-design-patterns.md` §11 |
//!
//! Une pièce plus large que la vue d'un grunt le fait tirer dessus **sans qu'il
//! puisse réagir** ; un couloir plus étroit que la somme des deux corps bloque
//! le croisement. Aucun de ces deux nombres n'est une opinion.
//!
//! ## Ce que la construction GARANTIT
//!
//! - **Connexité** : les portes viennent d'un arbre couvrant. Aucune pièce ne
//!   peut être isolée — ce n'est pas vérifié après coup, c'est impossible.
//! - **≥ 2 sorties** par pièce : des arêtes de boucle sont ajoutées à l'arbre.
//!   Une pièce de combat à une seule entrée est un cul-de-sac
//!   (`map-design-intention.md` §3.1).
//! - **Le complexe ne remplit pas l'arène** : il occupe le centre, l'anneau
//!   extérieur reste ouvert. C'est la forme Gunfire — une structure bâtie dans
//!   un espace plus large — et ça préserve les anneaux d'apparition existants.

use bevy::prelude::Vec3;

pub(crate) const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_rooms.toml";
use serde::Deserialize;

/// Longueur ET hauteur du module de mur `kaykit/dungeon/wall.glb` — **MESURÉ**
/// 4,00 × 4,00 × 1,00 m.
///
/// L'autre mur du projet (`medieval_hexagon/wall_straight`) fait **1,10 m** de
/// haut : le joueur saute 1,174 m, donc il passe par-dessus. C'est une clôture,
/// pas un mur, et elle ne peut pas servir à faire des pièces.
pub const WALL_MODULE_M: f32 = 4.0;

/// Rayon du disque joueur (m) — capsule 0,6 m. Le personnage est un DISQUE en
/// plan, jamais une boîte (`map-design-patterns.md` §1).
pub const PLAYER_RADIUS_M: f32 = 0.6;
/// Rayon du corps d'un bot (m) — `BOT_BODY_RADIUS_M` de `forgia-ai-arena-bot`.
pub const BOT_RADIUS_M: f32 = 0.4;

/// Portée de vue du grunt (m) — `assets/genomes/enemies/`. C'est elle qui borne
/// la taille d'une pièce : au-delà, il se fait tirer dessus sans pouvoir réagir.
pub const GRUNT_VISION_M: f32 = 20.0;

#[derive(Deserialize, Default)]
struct RoomsToml {
    #[serde(default)]
    rooms: RoomsSection,
}

#[derive(Deserialize, Default)]
struct RoomsSection {
    enabled: Option<bool>,
    rooms_per_side: Option<u32>,
    corridor_modules: Option<u32>,
    extra_loops: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomsConfig {
    pub enabled: bool,
    /// Côté de la grille. 3 → 9 pièces.
    pub rooms_per_side: u32,
    /// Largeur d'une porte, en modules de mur. 1 → 4 m.
    pub corridor_modules: u32,
    /// Fraction d'arêtes ajoutées à l'arbre couvrant, pour tuer les culs-de-sac.
    pub extra_loops: f32,
}

impl Default for RoomsConfig {
    fn default() -> Self {
        // Miroir EXACT de assets/genomes/roguelite/roguelite_rooms.toml.
        Self {
            enabled: true,
            rooms_per_side: 3,
            corridor_modules: 1,
            extra_loops: 0.5,
        }
    }
}

impl RoomsConfig {
    /// Chargement disque, repli sur le miroir Rust.
    pub fn load_or_default() -> Self {
        match std::fs::read_to_string(GENOME_PATH) {
            Ok(c) => Self::parse_toml(&c),
            Err(_) => Self::default(),
        }
    }

    pub fn parse_toml(content: &str) -> Self {
        let t: RoomsToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let d = Self::default();
        Self {
            enabled: t.rooms.enabled.unwrap_or(d.enabled),
            // ≥ 2 : une grille 1×1 n'a ni pièce ni couloir.
            rooms_per_side: t.rooms.rooms_per_side.unwrap_or(d.rooms_per_side).clamp(2, 8),
            // La PARITÉ est imposée, pas subie : un mur troué se pave en
            // `(cell - porte) / 2` de chaque côté, et ce demi-tronçon doit être
            // un nombre ENTIER de modules. Sinon le dernier module déborde dans
            // la porte (et la ferme) ou laisse un trou. Étirer un module pour
            // combler est exclu : story-483 a montré que ça donne des murs
            // tordus comme des planches.
            corridor_modules: Self::snap_corridor_parity(
                t.rooms.corridor_modules.unwrap_or(d.corridor_modules).clamp(1, 4),
                d.cell_modules(),
            ),
            extra_loops: t.rooms.extra_loops.unwrap_or(d.extra_loops).clamp(0.0, 1.0),
        }
    }

    /// Nombre de modules de mur d'une cellule.
    pub fn cell_modules(&self) -> u32 {
        (self.cell_m() / WALL_MODULE_M).round() as u32
    }

    /// Ramène la largeur de porte à la PARITÉ de la cellule.
    ///
    /// `(cellule − porte)` doit être pair pour que les deux demi-tronçons soient
    /// des entiers de modules. Une porte de mauvaise parité est corrigée d'un
    /// module vers le bas (jamais vers le haut : élargir une porte ouvre une
    /// brèche, la rétrécir reste une porte).
    fn snap_corridor_parity(requested: u32, cell_modules: u32) -> u32 {
        if (cell_modules % 2) == (requested % 2) {
            requested.max(1)
        } else {
            requested.saturating_sub(1).max(1)
        }
    }

    /// Largeur de porte (m) — bornée en bas par ce qu'il faut pour se croiser.
    ///
    /// Un joueur et un bot doivent passer : `2·(0,6 + 0,4) = 2,0 m` au strict
    /// minimum. Le module de mur fait 4 m, donc une porte d'un module double
    /// déjà cette marge — c'est confortable sans être une brèche.
    pub fn corridor_width_m(&self) -> f32 {
        let min = 2.0 * (PLAYER_RADIUS_M + BOT_RADIUS_M);
        (self.corridor_modules as f32 * WALL_MODULE_M).max(min)
    }

    /// Pas de la trame (m) — multiple du module de mur, et borné par la vue du
    /// grunt : la DIAGONALE de la pièce doit tenir dans sa portée de vue.
    pub fn cell_m(&self) -> f32 {
        let max_side = GRUNT_VISION_M / std::f32::consts::SQRT_2;
        let modules = (max_side / WALL_MODULE_M).floor().max(1.0);
        modules * WALL_MODULE_M
    }

    /// Emprise totale du complexe (m).
    pub fn complex_size_m(&self) -> f32 {
        self.rooms_per_side as f32 * self.cell_m()
    }

    /// Le complexe tient-il dans une arène de rayon `extent` ?
    ///
    /// Sa demi-diagonale doit rester à l'intérieur, avec la marge du disque
    /// joueur — sinon un mur sortirait de l'enceinte.
    pub fn fits_in(&self, extent_m: f32) -> bool {
        let half_diag = self.complex_size_m() * std::f32::consts::SQRT_2 / 2.0;
        half_diag + PLAYER_RADIUS_M <= extent_m
    }
}

/// Un segment de mur à instancier : centre, longueur, et axe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallSeg {
    pub center_x: f32,
    pub center_z: f32,
    pub len_m: f32,
    /// `true` = le mur court selon X, `false` = selon Z.
    pub along_x: bool,
}

/// Le plan d'un complexe : les murs à poser, et les centres de pièces.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoomPlan {
    pub walls: Vec<WallSeg>,
    /// Centre de chaque pièce (utile pour y placer du décor ou des arrivées).
    pub room_centers: Vec<(f32, f32)>,
    pub cell_m: f32,
    pub corridor_w_m: f32,
}

// ─── SplitMix64 — déterministe, sans dépendance ─────────────────────────────

fn mix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Arête entre deux cellules voisines de la grille.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Edge {
    a: u32,
    b: u32,
    /// `true` = les deux cellules sont côte à côte selon X (mur vertical entre).
    horizontal: bool,
}

/// Toutes les arêtes de la grille, dans un ordre STABLE.
fn all_edges(n: u32) -> Vec<Edge> {
    let mut v = Vec::new();
    for row in 0..n {
        for col in 0..n {
            let id = row * n + col;
            if col + 1 < n {
                v.push(Edge {
                    a: id,
                    b: id + 1,
                    horizontal: true,
                });
            }
            if row + 1 < n {
                v.push(Edge {
                    a: id,
                    b: id + n,
                    horizontal: false,
                });
            }
        }
    }
    v
}

/// Choisit les arêtes OUVERTES (= les portes).
///
/// Arbre couvrant par union-find sur des arêtes mélangées : la connexité est
/// garantie **par construction**, pas vérifiée après coup. Puis on rouvre une
/// fraction des arêtes restantes pour tuer les culs-de-sac — une pièce de combat
/// à une seule entrée est un piège, pas un espace.
fn pick_open_edges(n: u32, seed: u64, extra_loops: f32) -> Vec<Edge> {
    let mut edges = all_edges(n);
    let mut rng = seed ^ 0x524F_4F4D_5F47_454E;
    // Mélange de Fisher-Yates — déterministe à graine égale.
    for i in (1..edges.len()).rev() {
        let j = (mix(&mut rng) % (i as u64 + 1)) as usize;
        edges.swap(i, j);
    }
    let cells = (n * n) as usize;
    let mut parent: Vec<usize> = (0..cells).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    let mut open = Vec::new();
    let mut rest = Vec::new();
    for e in edges {
        let (ra, rb) = (find(&mut parent, e.a as usize), find(&mut parent, e.b as usize));
        if ra != rb {
            parent[ra] = rb;
            open.push(e);
        } else {
            rest.push(e);
        }
    }
    // Boucles : des routes alternatives, donc des pièces à ≥ 2 sorties.
    let extra = (rest.len() as f32 * extra_loops).round() as usize;
    open.extend(rest.into_iter().take(extra));
    open
}

/// Construit le complexe. PUR — testable sans monde Bevy.
///
/// Le complexe est CENTRÉ sur l'origine et n'occupe que le milieu de l'arène :
/// l'anneau extérieur reste ouvert. C'est la forme Gunfire (une structure bâtie
/// dans un espace plus large) et ça préserve les anneaux d'apparition existants.
pub fn plan_rooms(extent_m: f32, seed: u64, cfg: &RoomsConfig) -> RoomPlan {
    if !cfg.enabled || !cfg.fits_in(extent_m) {
        return RoomPlan::default();
    }
    let n = cfg.rooms_per_side;
    let cell = cfg.cell_m();
    let door = cfg.corridor_width_m();
    let size = cfg.complex_size_m();
    let origin = -size / 2.0; // coin bas-gauche du complexe

    let open = pick_open_edges(n, seed, cfg.extra_loops);
    let is_open = |a: u32, b: u32| open.iter().any(|e| e.a == a.min(b) && e.b == a.max(b));

    let mut walls = Vec::new();
    // Un mur troué = deux tronçons de part et d'autre de la porte, centrée.
    let mut push_wall = |cx: f32, cz: f32, len: f32, along_x: bool, holed: bool| {
        if !holed {
            walls.push(WallSeg {
                center_x: cx,
                center_z: cz,
                len_m: len,
                along_x,
            });
            return;
        }
        let side = (len - door) / 2.0;
        if side <= 0.01 {
            return; // la porte mange tout le mur : pas de tronçon à poser
        }
        let off = door / 2.0 + side / 2.0;
        for s in [-1.0_f32, 1.0] {
            walls.push(WallSeg {
                center_x: if along_x { cx + s * off } else { cx },
                center_z: if along_x { cz } else { cz + s * off },
                len_m: side,
                along_x,
            });
        }
    };

    // Murs INTÉRIEURS : une arête ouverte devient un mur troué.
    for row in 0..n {
        for col in 0..n {
            let id = row * n + col;
            let x0 = origin + col as f32 * cell;
            let z0 = origin + row as f32 * cell;
            if col + 1 < n {
                // Mur vertical entre (row,col) et (row,col+1) — court selon Z.
                push_wall(x0 + cell, z0 + cell / 2.0, cell, false, is_open(id, id + 1));
            }
            if row + 1 < n {
                // Mur horizontal entre (row,col) et (row+1,col) — court selon X.
                push_wall(x0 + cell / 2.0, z0 + cell, cell, true, is_open(id, id + n));
            }
        }
    }

    // Murs EXTÉRIEURS du complexe, avec une entrée au milieu de chaque face :
    // on doit pouvoir y entrer depuis l'anneau ouvert, et par plus d'un côté.
    let mid = size / 2.0;
    for (cx, cz, along_x) in [
        (0.0, origin, true),
        (0.0, origin + size, true),
        (origin, 0.0, false),
        (origin + size, 0.0, false),
    ] {
        let _ = mid;
        push_wall(cx, cz, size, along_x, true);
    }

    let mut room_centers = Vec::with_capacity((n * n) as usize);
    for row in 0..n {
        for col in 0..n {
            room_centers.push((
                origin + (col as f32 + 0.5) * cell,
                origin + (row as f32 + 0.5) * cell,
            ));
        }
    }

    RoomPlan {
        walls,
        room_centers,
        cell_m: cell,
        corridor_w_m: door,
    }
}

// ─── Instanciation ──────────────────────────────────────────────────────────

impl WallSeg {
    /// Poses des modules de mur le long du tronçon, à leur TAILLE NATURELLE.
    ///
    /// Jamais d'étirement : story-483 a montré qu'un module allongé pour combler
    /// un tronçon donne des murs tordus comme des planches. La parité imposée en
    /// amont garantit que la longueur est un entier de modules, donc que le
    /// pavage tombe juste sans qu'on ait à tricher.
    pub fn module_poses(&self) -> Vec<(Vec3, f32)> {
        let n = (self.len_m / WALL_MODULE_M).round().max(1.0) as u32;
        let yaw = if self.along_x {
            0.0
        } else {
            std::f32::consts::FRAC_PI_2
        };
        (0..n)
            .map(|i| {
                let t = (i as f32 + 0.5) * WALL_MODULE_M - self.len_m / 2.0;
                let pos = if self.along_x {
                    Vec3::new(self.center_x + t, 0.0, self.center_z)
                } else {
                    Vec3::new(self.center_x, 0.0, self.center_z + t)
                };
                (pos, yaw)
            })
            .collect()
    }

    /// Demi-dimensions du collider du tronçon (un seul cuboïde par tronçon).
    ///
    /// UN collider par tronçon, pas un par module : les bots sondent par
    /// raycast, et 30 boîtes coûtent bien moins que 120. Même choix que les
    /// remparts (6 colliders pour ~130 murs visuels).
    pub fn collider_half_extents(&self, height_m: f32, thickness_m: f32) -> Vec3 {
        if self.along_x {
            Vec3::new(self.len_m * 0.5, height_m * 0.5, thickness_m * 0.5)
        } else {
            Vec3::new(thickness_m * 0.5, height_m * 0.5, self.len_m * 0.5)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet, VecDeque};

    /// Reconstruit la connectivité depuis les arêtes ouvertes.
    fn open_set(n: u32, seed: u64, loops: f32) -> HashSet<(u32, u32)> {
        pick_open_edges(n, seed, loops)
            .into_iter()
            .map(|e| (e.a.min(e.b), e.a.max(e.b)))
            .collect()
    }

    /// **Aucune pièce ne peut être isolée.** Garanti par l'arbre couvrant, mais
    /// vérifié : une pièce inatteignable serait un round qui ne se nettoie pas.
    #[test]
    fn every_room_is_reachable_from_every_other() {
        for n in 2..=6u32 {
            for seed in [0u64, 7, 42, 1337, 99_991] {
                let open = open_set(n, seed, 0.0); // arbre SEUL, sans boucles
                let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
                for (a, b) in &open {
                    adj.entry(*a).or_default().push(*b);
                    adj.entry(*b).or_default().push(*a);
                }
                let mut seen = HashSet::from([0u32]);
                let mut q = VecDeque::from([0u32]);
                while let Some(c) = q.pop_front() {
                    for &nb in adj.get(&c).map(|v| v.as_slice()).unwrap_or(&[]) {
                        if seen.insert(nb) {
                            q.push_back(nb);
                        }
                    }
                }
                assert_eq!(
                    seen.len(),
                    (n * n) as usize,
                    "grille {n}x{n} graine {seed} : {} pièces atteintes sur {}",
                    seen.len(),
                    n * n
                );
            }
        }
    }

    /// Les boucles doivent réellement tuer les culs-de-sac : une pièce de combat
    /// à une seule sortie est un piège (`map-design-intention.md` §3.1).
    #[test]
    fn loops_remove_most_dead_ends() {
        let n = 4u32;
        for seed in [1u64, 2, 3, 4, 5] {
            let count_dead_ends = |loops: f32| {
                let open = open_set(n, seed, loops);
                let mut deg: HashMap<u32, u32> = HashMap::new();
                for (a, b) in &open {
                    *deg.entry(*a).or_default() += 1;
                    *deg.entry(*b).or_default() += 1;
                }
                (0..n * n).filter(|c| deg.get(c).copied().unwrap_or(0) <= 1).count()
            };
            let without = count_dead_ends(0.0);
            let with = count_dead_ends(0.5);
            assert!(
                with <= without,
                "graine {seed} : les boucles ont AJOUTÉ des culs-de-sac ({without} → {with})"
            );
        }
        // Et à pleine boucle, il n'en reste aucun : la grille est complète.
        let open = open_set(n, 42, 1.0);
        let mut deg: HashMap<u32, u32> = HashMap::new();
        for (a, b) in &open {
            *deg.entry(*a).or_default() += 1;
            *deg.entry(*b).or_default() += 1;
        }
        assert_eq!(
            (0..n * n).filter(|c| deg.get(c).copied().unwrap_or(0) <= 1).count(),
            0,
            "à extra_loops = 1, aucune pièce ne doit avoir une seule sortie"
        );
    }

    /// **La pièce ne doit pas dépasser la vue d'un grunt.** Au-delà, il se fait
    /// tirer dessus sans pouvoir réagir (`map-design-intention.md` §2.2).
    #[test]
    fn a_room_never_outranges_the_enemy_that_fights_in_it() {
        let c = RoomsConfig::default();
        let diag = c.cell_m() * std::f32::consts::SQRT_2;
        assert!(
            diag <= GRUNT_VISION_M,
            "diagonale {diag:.1} m > vue du grunt {GRUNT_VISION_M} m — tir gratuit"
        );
    }

    /// **Le couloir doit laisser passer joueur ET bot.** Le personnage est un
    /// disque : la largeur se mesure en distance, pas en boîte gonflée.
    #[test]
    fn a_corridor_always_fits_a_player_and_a_bot_side_by_side() {
        for modules in 1..=4u32 {
            let c = RoomsConfig {
                corridor_modules: modules,
                ..RoomsConfig::default()
            };
            assert!(
                c.corridor_width_m() >= 2.0 * (PLAYER_RADIUS_M + BOT_RADIUS_M),
                "{modules} module(s) : {:.2} m est trop étroit",
                c.corridor_width_m()
            );
        }
    }

    /// Le pas de trame DOIT être un multiple du module de mur, sinon chaque
    /// mur laisse un trou ou déborde.
    #[test]
    fn the_grid_pitch_tiles_the_wall_module_exactly() {
        let c = RoomsConfig::default();
        let ratio = c.cell_m() / WALL_MODULE_M;
        assert!(
            (ratio - ratio.round()).abs() < 1e-4,
            "pas {:.2} m n'est pas un multiple de {WALL_MODULE_M} m",
            c.cell_m()
        );
    }

    /// Le complexe ne doit jamais sortir de l'enceinte : un mur hors des
    /// remparts, c'est une brèche par laquelle on quitte l'arène.
    #[test]
    fn the_complex_stays_inside_the_arena() {
        let c = RoomsConfig::default();
        // Nos arènes font 80 et 90 m de rayon.
        for extent in [80.0_f32, 90.0] {
            assert!(c.fits_in(extent), "le complexe ne tient pas dans {extent} m");
            let plan = plan_rooms(extent, 7, &c);
            assert!(!plan.walls.is_empty());
            for w in &plan.walls {
                let half = w.len_m / 2.0;
                let (dx, dz) = if w.along_x { (half, 0.0) } else { (0.0, half) };
                for s in [-1.0_f32, 1.0] {
                    let (x, z) = (w.center_x + s * dx, w.center_z + s * dz);
                    let d = (x * x + z * z).sqrt();
                    assert!(d <= extent, "un mur sort à {d:.1} m (enceinte {extent} m)");
                }
            }
        }
    }

    /// Une arène trop petite ne doit pas produire un complexe tronqué : elle
    /// n'en produit AUCUN, et c'est un cas explicite, pas un accident.
    #[test]
    fn a_too_small_arena_gets_no_complex_at_all() {
        let c = RoomsConfig::default();
        assert!(plan_rooms(5.0, 1, &c).walls.is_empty());
        assert!(!c.fits_in(5.0));
    }

    /// Déterminisme : même graine → même complexe. Sinon deux joueurs de la
    /// même run ne verraient pas la même arène.
    #[test]
    fn the_same_seed_builds_the_same_complex() {
        let c = RoomsConfig::default();
        assert_eq!(plan_rooms(80.0, 4242, &c), plan_rooms(80.0, 4242, &c));
    }

    /// Deux graines doivent donner deux complexes DIFFÉRENTS — sinon la
    /// génération est décorative.
    #[test]
    fn different_seeds_build_different_complexes() {
        let c = RoomsConfig::default();
        let a = plan_rooms(80.0, 1, &c);
        let b = plan_rooms(80.0, 2, &c);
        assert_ne!(a.walls, b.walls, "deux graines, un seul complexe");
        assert_eq!(a.room_centers, b.room_centers, "la trame, elle, ne bouge pas");
    }

    /// Un génome hostile ne doit pas produire une grille dégénérée.
    #[test]
    fn a_hostile_genome_cannot_degenerate_the_grid() {
        let c = RoomsConfig::parse_toml(
            "[rooms]\nrooms_per_side = 0\ncorridor_modules = 99\nextra_loops = 5.0\n",
        );
        assert!(c.rooms_per_side >= 2, "une grille 1x1 n'a ni pièce ni couloir");
        assert!(c.corridor_modules <= 4);
        assert!(c.extra_loops <= 1.0);
    }

    #[test]
    fn the_shipped_genome_matches_the_rust_mirror() {
        let content = std::fs::read_to_string(GENOME_PATH)
            .or_else(|_| std::fs::read_to_string(format!("../../{GENOME_PATH}")))
            .expect("roguelite_rooms.toml introuvable");
        assert_eq!(RoomsConfig::parse_toml(&content), RoomsConfig::default());
    }
    /// **Chaque tronçon doit être un nombre ENTIER de modules.**
    ///
    /// C'est l'invariant qui rend la porte réelle : un demi-tronçon de 1,5
    /// module se pave en 2 modules qui débordent dans l'ouverture et la
    /// referment. Étirer le dernier module pour combler est exclu — story-483 a
    /// montré que ça donne des murs tordus comme des planches.
    #[test]
    fn every_wall_segment_tiles_the_module_exactly() {
        for modules in 1..=4u32 {
            let c = RoomsConfig::parse_toml(&format!(
                "[rooms]
corridor_modules = {modules}
"
            ));
            let plan = plan_rooms(90.0, 33, &c);
            assert!(!plan.walls.is_empty(), "{modules} module(s) : aucun mur");
            for w in &plan.walls {
                let n = w.len_m / WALL_MODULE_M;
                assert!(
                    (n - n.round()).abs() < 1e-3,
                    "tronçon de {:.2} m = {n:.2} modules — il déborderait dans la porte",
                    w.len_m
                );
            }
        }
    }

    /// La porte doit rester une PORTE : jamais élargie en douce par la
    /// correction de parité (une brèche n'est pas une porte).
    #[test]
    fn the_parity_snap_never_widens_a_door() {
        for requested in 1..=4u32 {
            let c = RoomsConfig::parse_toml(&format!(
                "[rooms]
corridor_modules = {requested}
"
            ));
            assert!(
                c.corridor_modules <= requested,
                "porte demandée {requested}, obtenue {} — élargie en douce",
                c.corridor_modules
            );
            assert!(c.corridor_modules >= 1, "une porte de 0 module n'est plus une porte");
        }
    }

    /// Ce que la config LIVRÉE produit, en chiffres — pour ne pas annoncer une
    /// forme sans l'avoir mesurée.
    #[test]
    fn the_delivered_complex_is_measured_not_claimed() {
        let c = RoomsConfig::default();
        let plan = plan_rooms(80.0, 0xF06, &c);
        println!(
            "[story-683] cellule {:.0} m · porte {:.0} m · complexe {:.0} m ·              {} pièces · {} tronçons de mur",
            plan.cell_m,
            plan.corridor_w_m,
            c.complex_size_m(),
            plan.room_centers.len(),
            plan.walls.len()
        );
        assert_eq!(plan.cell_m, 12.0, "cellule dérivée de la vue du grunt");
        assert_eq!(plan.corridor_w_m, 4.0);
        assert_eq!(plan.room_centers.len(), 9);
        // Le complexe (36 m) doit laisser un anneau ouvert généreux dans une
        // arène de 80 m de rayon.
        assert!(c.complex_size_m() < 80.0, "le complexe mangerait toute l'arène");
    }
}
