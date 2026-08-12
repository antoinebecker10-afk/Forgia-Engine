//! forgia-navmesh — la géométrie solide d'une zone devient un maillage de navigation.
//!
//! # Pourquoi cette crate existe
//!
//! `forgia-ai-arena-bot` avance en ligne droite vers sa cible et pousse dans les colliders.
//! Un mob né derrière un pilier ne se débloque jamais : ce n'est pas un ralentissement,
//! c'est un ennemi retiré du combat (`spawn-clearance.md` §5, qui documente le manque et
//! s'interdit explicitement de prétendre que « ça n'arrive jamais »).
//!
//! Le compagnon de l'épic **E1** rend le défaut bien plus visible : un mob coincé à 40 m
//! passe inaperçu, un compagnon dont le point ne bouge plus sur la minimap se voit toutes
//! les trois secondes.
//!
//! # Ce que fait cette crate, et rien d'autre
//!
//! Convertir des solides (`forgia_core::layout`) en maillage polyanya, et répondre
//! « donne-moi un chemin de A à B ». Pas de système Bevy, pas de composant de suivi :
//! l'appelant décide quand reconstruire et qui suit le chemin.
//!
//! # Ce qu'elle ne fait PAS
//!
//! - **Désenlisement** d'un agent déjà coincé — c'est un chien de garde séparé, et il doit
//!   être livré *avec* le compagnon, pas après.
//! - **Régénération par chunk** d'un terrain streamé — Phase 2 de la refonte. L'API prend
//!   un bord quelconque précisément pour que l'Expédition la réutilise sans la réécrire.
//! - **Évitement dynamique** entre agents (deux mobs qui se gênent) — le navmesh est statique.

use bevy::prelude::*;
use forgia_core::layout::{SolidDisc, SolidSeg};
use serde::Deserialize;
use std::f32::consts::PI;
use vleue_navigator::NavMesh;

/// Chemin vers la couche definition. Aucune valeur numérique ne vit dans ce fichier.
const GENOME_PATH: &str = "assets/genomes/navmesh.toml";

// ─────────────────────────────── Réglages ───────────────────────────────

/// Réglages de construction, lus depuis [`GENOME_PATH`].
///
/// Les bornes de `clamp` ne sont pas décoratives : un rayon d'agent nul produirait un
/// maillage où tout passe (donc des agents encastrés), et un rayon démesuré fermerait
/// l'arène entière — les deux échouent en silence, sans erreur ni panique.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct NavmeshBuild {
    /// Rayon d'emprise de l'agent (m). Dilate les obstacles, rétrécit le bord.
    pub agent_radius_m: f32,
    /// Ressaut franchissable (m). Au-dessus, le solide est un obstacle.
    pub step_height_m: f32,
    /// Côtés du polygone qui approxime un disque.
    pub disc_segments: u32,
}

impl Default for NavmeshBuild {
    /// Reflet exact de `navmesh.toml`. Sert de repli si le fichier manque — pas de
    /// source de vérité concurrente : quand les deux divergent, c'est le TOML qui gagne.
    fn default() -> Self {
        Self {
            agent_radius_m: 0.30,
            step_height_m: 0.45,
            disc_segments: 12,
        }
    }
}

#[derive(Deserialize)]
struct GenomeFile {
    #[serde(default)]
    agent: AgentSection,
    #[serde(default)]
    quality: QualitySection,
}

/// Chaque champ est optionnel : une section absente ou partielle retombe champ par champ
/// sur [`NavmeshBuild::default`], au lieu de perdre tout le reste du fichier.
#[derive(Deserialize, Default)]
struct AgentSection {
    radius_m: Option<f32>,
    step_height_m: Option<f32>,
}

#[derive(Deserialize, Default)]
struct QualitySection {
    disc_segments: Option<u32>,
}

impl NavmeshBuild {
    /// Lit le génome via `def_io` — **jamais `std::fs`** : sur wasm un `std::fs` échoue en
    /// silence et le défaut ne se voit qu'en jeu (mémoire `reference_wasm_web_port_forgia`).
    pub fn load_or_default() -> Self {
        let fallback = Self::default();
        let Ok(content) = forgia_core::def_io::read_def_str(GENOME_PATH) else {
            warn!("[navmesh] {GENOME_PATH} illisible — repli sur les valeurs par défaut");
            return fallback;
        };
        match toml::from_str::<GenomeFile>(&content) {
            Ok(file) => Self {
                agent_radius_m: file
                    .agent
                    .radius_m
                    .unwrap_or(fallback.agent_radius_m)
                    .clamp(0.05, 2.0),
                step_height_m: file
                    .agent
                    .step_height_m
                    .unwrap_or(fallback.step_height_m)
                    .clamp(0.0, 3.0),
                disc_segments: file
                    .quality
                    .disc_segments
                    .unwrap_or(fallback.disc_segments)
                    .clamp(3, 64),
            },
            Err(e) => {
                warn!("[navmesh] {GENOME_PATH} illisible ({e}) — repli sur les défauts");
                fallback
            }
        }
    }
}

// ─────────────────────────── Qui bloque un agent ───────────────────────────

/// Un solide de hauteur `h` arrête-t-il un agent ?
///
/// **Ce n'est PAS `SolidDisc::breaks_sight()`.** Le couvert commence à 1,80 m parce qu'il
/// s'agit de casser une ligne de vue ; la navigation se joue à hauteur de pied. Un muret
/// d'un mètre ne masque personne et arrête pourtant un agent qui ne saute pas. Réutiliser
/// le prédicat existant parce qu'il est sous la main aurait rendu la moitié des obstacles
/// invisibles au maillage.
#[inline]
#[must_use]
pub fn blocks_agent(h: f32, step_height_m: f32) -> bool {
    h > step_height_m
}

// ─────────────────────────── Géométrie → polygones ───────────────────────────

/// Le bord d'une arène hexagonale, **rétréci** du rayon d'agent.
///
/// `ArenaGeometry::playable_radius_m` est l'**apothème** (distance perpendiculaire au
/// côté), pas le rayon circonscrit — c'est lui qu'il faut réduire, puisque c'est lui qui
/// mesure la distance au mur.
#[must_use]
pub fn hexagon_edge(apothem_m: f32, agent_radius_m: f32) -> Vec<Vec2> {
    let apothem = (apothem_m - agent_radius_m).max(0.0);
    // Apothème → rayon circonscrit : a = R·cos(π/6).
    let circumradius = apothem / (PI / 6.0).cos();
    (0..6)
        .map(|i| {
            let a = PI / 3.0 * i as f32;
            Vec2::new(circumradius * a.cos(), circumradius * a.sin())
        })
        .collect()
}

/// Un disque solide → polygone **circonscrit** au disque dilaté.
///
/// Circonscrit, jamais inscrit : un polygone inscrit passerait *à l'intérieur* du disque
/// et laisserait des chemins mordre dans l'obstacle. On sur-approxime toujours — un agent
/// qui contourne un peu large coûte un détour, un agent qui frotte coûte un blocage.
#[must_use]
pub fn disc_to_obstacle(d: &SolidDisc, agent_radius_m: f32, segments: u32) -> Vec<Vec2> {
    let n = segments.max(3);
    let inflated = d.r + agent_radius_m;
    // Rayon des sommets d'un n-gone qui CONTIENT le cercle de rayon `inflated`.
    let vertex_radius = inflated / (PI / n as f32).cos();
    (0..n)
        .map(|i| {
            let a = 2.0 * PI * i as f32 / n as f32;
            Vec2::new(
                d.x + vertex_radius * a.cos(),
                d.z + vertex_radius * a.sin(),
            )
        })
        .collect()
}

/// Un tronçon solide → rectangle dilaté, capuchons compris.
///
/// Les extrémités arrondies sont approximées par une extension droite de la demi-largeur :
/// encore une sur-approximation volontaire.
#[must_use]
pub fn seg_to_obstacle(s: &SolidSeg, agent_radius_m: f32) -> Vec<Vec2> {
    let p0 = Vec2::new(s.x0, s.z0);
    let p1 = Vec2::new(s.x1, s.z1);
    let half_w = s.half_thick_m + agent_radius_m;
    let delta = p1 - p0;
    // Tronçon dégénéré (les deux bouts confondus) : c'est un disque, pas un rectangle.
    let Some(dir) = delta.try_normalize() else {
        let disc = SolidDisc {
            x: s.x0,
            z: s.z0,
            r: s.half_thick_m,
            h: s.h,
        };
        return disc_to_obstacle(&disc, agent_radius_m, 8);
    };
    let normal = Vec2::new(-dir.y, dir.x);
    let a = p0 - dir * half_w;
    let b = p1 + dir * half_w;
    vec![
        a + normal * half_w,
        b + normal * half_w,
        b - normal * half_w,
        a - normal * half_w,
    ]
}

// ─────────────────────────── Construction ───────────────────────────

/// Combien de solides ont réellement été retenus comme obstacles.
///
/// **Zéro n'est pas un succès, c'est un aveugle** (`map-design-patterns.md` §13). Un
/// maillage bâti sur zéro obstacle est un plan vide où tout passe : le rapport le dit au
/// lieu de laisser croire à une arène dégagée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BuildReport {
    pub discs_seen: usize,
    pub segs_seen: usize,
    pub obstacles_kept: usize,
}

impl BuildReport {
    /// Aucun solide n'a été soumis : on n'a rien mesuré, on ne conclut rien.
    #[must_use]
    pub fn is_blind(&self) -> bool {
        self.discs_seen + self.segs_seen == 0
    }
}

/// Bâtit le maillage à partir d'un bord et des solides d'une zone.
///
/// `edge` est déjà rétréci du rayon d'agent (cf. [`hexagon_edge`]). Prendre un bord
/// quelconque plutôt qu'un `ArenaGeometry` est délibéré : l'Expédition (Phase 2) n'a pas
/// d'arène hexagonale et doit pouvoir réutiliser cette fonction telle quelle.
#[must_use]
pub fn build(
    edge: Vec<Vec2>,
    discs: &[SolidDisc],
    segs: &[SolidSeg],
    cfg: &NavmeshBuild,
) -> (NavMesh, BuildReport) {
    let mut obstacles: Vec<Vec<Vec2>> = Vec::with_capacity(discs.len() + segs.len());
    for d in discs {
        if blocks_agent(d.h, cfg.step_height_m) {
            obstacles.push(disc_to_obstacle(d, cfg.agent_radius_m, cfg.disc_segments));
        }
    }
    for s in segs {
        if blocks_agent(s.h, cfg.step_height_m) {
            obstacles.push(seg_to_obstacle(s, cfg.agent_radius_m));
        }
    }
    let report = BuildReport {
        discs_seen: discs.len(),
        segs_seen: segs.len(),
        obstacles_kept: obstacles.len(),
    };
    (NavMesh::from_edge_and_obstacles(edge, obstacles), report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> NavmeshBuild {
        NavmeshBuild::default()
    }

    fn disc(x: f32, z: f32, r: f32, h: f32) -> SolidDisc {
        SolidDisc { x, z, r, h }
    }

    // ── Qui bloque ──────────────────────────────────────────────────────

    #[test]
    fn un_solide_plus_bas_que_le_ressaut_ne_bloque_pas() {
        assert!(!blocks_agent(0.30, cfg().step_height_m));
    }

    #[test]
    fn un_solide_plus_haut_que_le_ressaut_bloque() {
        assert!(blocks_agent(0.50, cfg().step_height_m));
    }

    #[test]
    fn le_seuil_de_navigation_n_est_pas_celui_du_couvert() {
        // Un muret d'1 m ne casse aucune ligne de vue (couvert = 1,80 m) mais arrête
        // un agent qui ne saute pas. Confondre les deux rendrait la moitié des
        // obstacles invisibles au maillage.
        let muret = disc(0.0, 0.0, 1.0, 1.0);
        assert!(!muret.breaks_sight());
        assert!(blocks_agent(muret.h, cfg().step_height_m));
    }

    // ── Géométrie ───────────────────────────────────────────────────────

    #[test]
    fn le_polygone_circonscrit_contient_bien_le_disque_dilate() {
        let c = cfg();
        let d = disc(0.0, 0.0, 2.0, 3.0);
        let poly = disc_to_obstacle(&d, c.agent_radius_m, c.disc_segments);
        let inflated = d.r + c.agent_radius_m;
        // Le point le plus proche du centre est le milieu d'une arête. S'il est au
        // moins à `inflated`, le polygone contient le disque dilaté partout.
        for i in 0..poly.len() {
            let mid = (poly[i] + poly[(i + 1) % poly.len()]) / 2.0;
            assert!(
                mid.length() >= inflated - 1e-4,
                "arête {i} passe à {} du centre, sous le disque dilaté {inflated}",
                mid.length()
            );
        }
    }

    #[test]
    fn le_bord_hexagonal_se_retrecit_du_rayon_d_agent() {
        let apothem = 10.0;
        let r = 0.30;
        let edge = hexagon_edge(apothem, r);
        assert_eq!(edge.len(), 6);
        // Le milieu d'un côté est à l'apothème du centre — c'est sa définition.
        let mid = (edge[0] + edge[1]) / 2.0;
        assert!((mid.length() - (apothem - r)).abs() < 1e-3);
    }

    #[test]
    fn un_troncon_degenere_devient_un_disque_au_lieu_de_paniquer() {
        let s = SolidSeg {
            x0: 1.0,
            z0: 2.0,
            x1: 1.0,
            z1: 2.0,
            half_thick_m: 0.5,
            h: 3.0,
        };
        let poly = seg_to_obstacle(&s, cfg().agent_radius_m);
        assert!(poly.len() >= 3, "un tronçon nul doit rendre un polygone valide");
    }

    // ── Le maillage répond ──────────────────────────────────────────────

    #[test]
    fn une_arene_vide_donne_un_chemin_droit() {
        let c = cfg();
        let (mesh, report) = build(hexagon_edge(20.0, c.agent_radius_m), &[], &[], &c);
        assert!(report.is_blind(), "aucun solide soumis = aveugle, pas « dégagé »");
        let from = Vec2::new(-10.0, 0.0);
        let to = Vec2::new(10.0, 0.0);
        let path = mesh.path(from, to).expect("un chemin doit exister");
        assert!(
            (path.length - from.distance(to)).abs() < 0.5,
            "sans obstacle le chemin doit être quasi droit, longueur {}",
            path.length
        );
    }

    #[test]
    fn un_pilier_force_le_detour() {
        let c = cfg();
        let discs = [disc(0.0, 0.0, 3.0, 4.0)];
        let (mesh, report) = build(hexagon_edge(20.0, c.agent_radius_m), &discs, &[], &c);
        assert_eq!(report.obstacles_kept, 1);
        let from = Vec2::new(-10.0, 0.0);
        let to = Vec2::new(10.0, 0.0);
        let path = mesh.path(from, to).expect("le pilier se contourne");
        assert!(
            path.length > from.distance(to) + 0.5,
            "le chemin devrait contourner, longueur {} vs droite {}",
            path.length,
            from.distance(to)
        );
    }

    #[test]
    fn un_solide_bas_ne_deforme_pas_le_chemin() {
        // Même pilier, mais franchissable : il ne doit pas entrer dans le maillage.
        let c = cfg();
        let discs = [disc(0.0, 0.0, 3.0, 0.20)];
        let (_mesh, report) = build(hexagon_edge(20.0, c.agent_radius_m), &discs, &[], &c);
        assert_eq!(report.discs_seen, 1);
        assert_eq!(
            report.obstacles_kept, 0,
            "un solide sous le ressaut n'est pas un obstacle"
        );
    }

    #[test]
    fn la_dilatation_ferme_un_passage_trop_etroit_pour_l_agent() {
        // LA preuve que le rayon d'agent est réellement appliqué : deux piliers laissant
        // 0,40 m entre eux. L'agent en exige 0,60 (2 × 0,30) → le passage doit être fermé,
        // donc le chemin passe par l'extérieur, bien plus long qu'à travers.
        let c = cfg();
        let gap = 0.40;
        let r = 3.0;
        let offset = r + gap / 2.0;
        let discs = [disc(0.0, offset, r, 4.0), disc(0.0, -offset, r, 4.0)];
        let (mesh, report) = build(hexagon_edge(20.0, c.agent_radius_m), &discs, &[], &c);
        assert_eq!(report.obstacles_kept, 2);
        let from = Vec2::new(-12.0, 0.0);
        let to = Vec2::new(12.0, 0.0);
        let path = mesh.path(from, to).expect("le contour reste possible");
        let through = from.distance(to);
        assert!(
            path.length > through + 2.0,
            "un passage de {gap} m doit être fermé à un agent de rayon {} ; \
             longueur obtenue {} vs {} en ligne droite",
            c.agent_radius_m,
            path.length,
            through
        );
    }

    // ── Réglages ────────────────────────────────────────────────────────

    #[test]
    fn les_valeurs_par_defaut_refletent_le_toml() {
        // Si ce test casse, c'est que le TOML et le repli ont divergé — exactement la
        // classe de défaut « une grandeur écrite deux fois » du projet.
        let toml_src = include_str!("../../../assets/genomes/navmesh.toml");
        let file: GenomeFile = toml::from_str(toml_src).expect("le génome doit parser");
        let d = NavmeshBuild::default();
        assert_eq!(file.agent.radius_m, Some(d.agent_radius_m));
        assert_eq!(file.agent.step_height_m, Some(d.step_height_m));
        assert_eq!(file.quality.disc_segments, Some(d.disc_segments));
    }
}
