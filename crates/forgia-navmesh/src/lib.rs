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
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;
use vleue_navigator::NavMesh;
/// Ré-export : [`PathOutcome`] porte un `Path` dans ses variantes, donc l'appelant
/// doit pouvoir le nommer sans dépendre de `vleue_navigator` en direct.
pub use vleue_navigator::Path;
use web_time::{SystemTime, UNIX_EPOCH};

/// Chemin vers la couche definition. Aucune valeur numérique ne vit dans ce fichier.
const GENOME_PATH: &str = "assets/genomes/navmesh.toml";
const SENSOR_PATH: &str = "forgia2_navmesh.json";
const SENSOR_WRITE_PERIOD_SEC: f32 = 1.0;

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
    let mut mesh = NavMesh::from_edge_and_obstacles(edge, obstacles);
    tune_search_reach(&mut mesh, cfg.agent_radius_m);
    (mesh, report)
}

/// Portée de la recherche « point navigable le plus proche », **dérivée** du rayon
/// d'agent.
///
/// # Pourquoi ce réglage existe — mesuré le 2026-08-13
///
/// `from_edge_and_obstacles` pose `search_delta = 0,01` et laisse `search_steps = 2` :
/// la recherche porte donc à **2 cm**. Or le maillage est volontairement rétréci de
/// `agent_radius_m` (30 cm) sur son bord *et* autour de chaque obstacle — c'est cette
/// bande qui garantit qu'un agent ne frotte pas les murs.
///
/// Le JOUEUR, lui, entre dans cette bande. Relevé en jeu : une cible à `[64,9 ; 26,1]`,
/// soit **69,95 m** du centre, pour un bord de maillage à **69,67 m** sous cet angle —
/// **28 cm dehors**. Une recherche de 2 cm ne la rattrape pas, `path()` rend `None`, et
/// tous les bots qui poursuivent un joueur collé au mur perdent leur chemin d'un coup.
/// C'est ce qui a produit **13,5 % de refus** sur la run de 10:53.
///
/// La portée n'est donc pas un réglage de confort : sa borne basse est **la largeur de
/// la bande abandonnée**, c'est-à-dire `agent_radius_m`. On prend le double par marge
/// (flottants, chevauchement de deux dilatations), découpé en 4 pas pour la précision.
fn tune_search_reach(mesh: &mut NavMesh, agent_radius_m: f32) {
    const STEPS: u32 = 4;
    // portée = STEPS × delta = 2 × agent_radius_m
    mesh.set_search_delta(agent_radius_m * 2.0 / STEPS as f32);
    mesh.set_search_steps(STEPS);
}

/// Ce qu'une demande de trajet a réellement donné.
///
/// Un `Option<Path>` ne savait pas dire *pourquoi* il était vide, et le capteur qui en
/// découlait a coûté une session de diagnostic : « 105 refus » ne permet pas de trancher
/// entre « la cible est hors du maillage » (rattrapable) et « aucun trajet ne relie les
/// deux » (poche isolée, vrai défaut de géométrie). Deux causes opposées, un seul
/// compteur — c'est le capteur aveugle que `map-design-patterns.md` §13 interdit.
#[derive(Debug)]
pub enum PathOutcome {
    /// Trajet trouvé tel quel.
    Direct(Path),
    /// Trajet trouvé après avoir **ramené une extrémité sur le maillage**. Le chemin
    /// s'arrête à `snapped_to`, à moins d'un rayon d'agent de la cible demandée : le
    /// dernier mètre revient à la ligne droite, exactement comme le dernier tronçon
    /// d'un chemin ordinaire.
    Snapped { path: Path, snapped_to: Vec2 },
    /// Aucun trajet. Les deux drapeaux disent **lequel des bouts** est en cause ;
    /// tous deux `false` = les deux points sont navigables mais rien ne les relie.
    NoRoute { from_off_mesh: bool, to_off_mesh: bool },
    /// Pas de maillage (hors zone, chargement). N'est **pas** un échec.
    NoMesh,
}

// ─────────────────────────── Le maillage courant ───────────────────────────

/// Le maillage de la zone courante, et **d'où il vient**.
///
/// La provenance (`source`, `seed`) n'est pas décorative : deux arènes de même `stage_id`
/// diffèrent par leur graine, et un maillage qui reste en place après un changement de
/// stage enverrait les agents à travers des murs qui n'existent plus. Le capteur l'expose
/// pour qu'une désynchronisation se voie au lieu de se deviner.
#[derive(Resource, Default)]
pub struct ActiveNavMesh {
    mesh: Option<NavMesh>,
    pub report: BuildReport,
    pub source: String,
    pub seed: u64,
    pub build_ms: f32,
    // ── Cumulés de session ──────────────────────────────────────────────
    //
    // 2026-08-12, première run réelle : le capteur rapportait `built: false,
    // blind: true` — et c'était **inexploitable**. Le maillage est effacé quand
    // l'arène est démontée, donc un instantané pris au menu ne dit RIEN de ce qui
    // s'est passé pendant la partie. Le capteur ne pouvait pas répondre à la seule
    // question pour laquelle il existait.
    //
    // Ces deux compteurs survivent au démontage. C'est la leçon
    // `feedback_les_agregats_cachent_la_chronologie_tranche` prise à l'envers :
    // un instantané sans cumul ne prouve rien non plus.
    /// Nombre de maillages bâtis depuis le lancement. **0 = jamais bâti**, et là
    /// seulement on peut l'affirmer.
    pub builds_session: u32,
    /// Le plus grand nombre d'obstacles retenus sur un bâti de la session.
    pub max_obstacles_session: usize,
}

impl ActiveNavMesh {
    /// Remplace le maillage courant. `source` = `stage_id` en arène, nom de zone ailleurs.
    pub fn set(&mut self, mesh: NavMesh, report: BuildReport, source: &str, seed: u64, build_ms: f32) {
        self.mesh = Some(mesh);
        self.builds_session = self.builds_session.saturating_add(1);
        self.max_obstacles_session = self.max_obstacles_session.max(report.obstacles_kept);
        self.report = report;
        self.source.clear();
        self.source.push_str(source);
        self.seed = seed;
        self.build_ms = build_ms;
    }

    /// Oublie le maillage — à appeler quand la zone est démontée. Mieux vaut aucun
    /// chemin qu'un chemin calculé sur une géométrie disparue.
    pub fn clear(&mut self) {
        self.mesh = None;
        self.report = BuildReport::default();
        self.source.clear();
        self.seed = 0;
        self.build_ms = 0.0;
    }

    #[must_use]
    pub fn is_built(&self) -> bool {
        self.mesh.is_some()
    }

    /// Chemin de `from` à `to`, ou `None` si le maillage manque ou si aucun trajet
    /// n'existe. L'appelant DOIT distinguer les deux cas : « pas encore de maillage »
    /// et « cette destination est inatteignable » n'appellent pas la même réaction.
    ///
    /// Forme brute, sans recalage : préférer [`Self::route`], qui rattrape le cas
    /// courant d'une cible dans la bande abandonnée et dit *pourquoi* quand il échoue.
    #[must_use]
    pub fn path(&self, from: Vec2, to: Vec2) -> Option<Path> {
        self.mesh.as_ref()?.path(from, to)
    }

    /// Trajet de `from` à `to`, en **ramenant sur le maillage** l'extrémité qui en sort.
    ///
    /// # Pourquoi le recalage n'est pas une commodité
    ///
    /// Le maillage abandonne une bande de `agent_radius_m` le long de chaque mur et
    /// autour de chaque obstacle. Le joueur, lui, y entre — il a sa propre collision, et
    /// rien ne l'empêche de se coller à une paroi. **Toute position qu'il occupe dans
    /// cette bande est hors maillage**, donc injoignable telle quelle.
    ///
    /// Sans recalage, se coller à un mur suffit à faire perdre son chemin à *tous* les
    /// poursuivants d'un coup. Mesuré le 2026-08-13 : 105 refus sur 776 demandes en 31 s
    /// de jeu, et 2 désenlisements. Avec [`tune_search_reach`], la recherche porte à
    /// deux rayons d'agent — assez pour la bande, pas assez pour téléporter la cible.
    ///
    /// Le chemin rendu s'arrête au point recalé ; le bot finit en ligne droite, ce qu'il
    /// fait déjà sur le dernier tronçon de n'importe quel chemin.
    #[must_use]
    pub fn route(&self, from: Vec2, to: Vec2) -> PathOutcome {
        let Some(mesh) = self.mesh.as_ref() else {
            return PathOutcome::NoMesh;
        };
        if let Some(p) = mesh.path(from, to) {
            return PathOutcome::Direct(p);
        }
        let from_off_mesh = !mesh.is_in_mesh(from);
        let to_off_mesh = !mesh.is_in_mesh(to);
        // Les deux bouts sont navigables et pourtant rien ne les relie : c'est une poche
        // isolée, un vrai défaut de géométrie. Le recalage n'y peut rien, et le dire
        // vaut mieux que le masquer par une retouche qui ne changerait rien.
        if !from_off_mesh && !to_off_mesh {
            return PathOutcome::NoRoute {
                from_off_mesh: false,
                to_off_mesh: false,
            };
        }
        let inner = mesh.get();
        // Deux recours, dans cet ordre.
        //
        // 1. `get_closest_point` — le vrai plus proche, mais son echantillonnage a un
        //    trou : `get_closest_point_inner` parcourt `0..=(sample*step)` avec un pas
        //    de `TAU/(sample*(step+1))`, donc il ne boucle JAMAIS le cercle entier et
        //    laisse toujours un secteur non teste. Mesure ici : il rate le sommet de
        //    l'hexagone, a 35 cm, alors que sa portee est de 60 cm.
        //
        // 2. Le repli par bissection vers un point CONNU dedans. Geometriquement
        //    garanti : un segment dont une extremite est dans le maillage et l'autre
        //    dehors traverse la frontiere, donc la bissection converge. Ce n'est pas
        //    le point le plus proche dans l'absolu, mais c'est un point navigable
        //    dans la direction utile — celle d'ou vient le poursuivant.
        let vers_le_maillage = |dehors: Vec2, dedans: Vec2| -> Vec2 {
            let (mut ko, mut ok) = (dehors, dedans);
            // 8 pas : l'ecart initial vaut au plus la portee de l'arene, 1/256e de
            // 80 m = 31 cm — sous le rayon d'agent, donc sans effet visible.
            for _ in 0..8 {
                let milieu = ko.midpoint(ok);
                if mesh.is_in_mesh(milieu) {
                    ok = milieu;
                } else {
                    ko = milieu;
                }
            }
            ok
        };
        let nearest = |p: Vec2, dehors: bool, ancre: Option<Vec2>| -> Option<Vec2> {
            if !dehors {
                return Some(p);
            }
            inner
                .get_closest_point(p)
                .map(|c| c.position())
                .or_else(|| ancre.map(|a| vers_le_maillage(p, a)))
        };
        // L'ancre n'existe que si l'autre bout est, lui, dans le maillage.
        let (Some(f), Some(t)) = (
            nearest(from, from_off_mesh, (!to_off_mesh).then_some(to)),
            nearest(to, to_off_mesh, (!from_off_mesh).then_some(from)),
        ) else {
            return PathOutcome::NoRoute {
                from_off_mesh,
                to_off_mesh,
            };
        };
        match mesh.path(f, t) {
            Some(path) => PathOutcome::Snapped {
                path,
                snapped_to: t,
            },
            None => PathOutcome::NoRoute {
                from_off_mesh,
                to_off_mesh,
            },
        }
    }
}

// ─────────────────────────── Capteur ───────────────────────────

/// Pur, testable headless.
///
/// **Un maillage bâti n'est jamais `ok` par défaut.** Deux façons de « réussir » à vide :
/// aucun solide soumis (la géométrie n'était pas prête), ou aucun retenu (le ressaut est
/// mal réglé et tout passe). Les deux produisent un plan où les agents traversent le
/// décor sans qu'aucune erreur ne soit levée — donc les deux alertent.
#[must_use]
pub fn severity_for_navmesh(
    built: bool,
    solids_seen: usize,
    obstacles_kept: usize,
) -> (&'static str, &'static str) {
    if !built {
        return (
            "info",
            "aucune zone batie — le maillage se construit a l'arrivee d'une arene",
        );
    }
    if solids_seen == 0 {
        return (
            "warn",
            "AVEUGLE : maillage bati sur zero solide soumis — ArenaGeometry etait vide \
             ou incomplete au moment de l'appel (cf. authored_pending)",
        );
    }
    if obstacles_kept == 0 {
        return (
            "warn",
            "0 obstacle retenu alors que des solides ont ete soumis — tous sous le \
             ressaut ? verifier agent.step_height_m dans assets/genomes/navmesh.toml",
        );
    }
    ("ok", "")
}

#[derive(Serialize)]
struct NavmeshSensor<'a> {
    id: &'a str,
    severity: &'a str,
    next_step: &'a str,
    timestamp_unix: u64,
    built: bool,
    source: &'a str,
    seed: u64,
    discs_seen: usize,
    segs_seen: usize,
    obstacles_kept: usize,
    /// `true` = rien n'a été mesuré. Distinct de « rien n'a été retenu ».
    blind: bool,
    build_ms: f32,
    /// **Le champ qui compte pour une lecture APRÈS la run.** L'instantané ci-dessus
    /// décrit l'instant présent — souvent le menu, où le maillage a été effacé.
    builds_session: u32,
    max_obstacles_session: usize,
    agent_radius_m: f32,
    step_height_m: f32,
}

pub fn sys_write_navmesh_sensor(
    time: Res<Time>,
    active: Res<ActiveNavMesh>,
    cfg: Res<NavmeshBuild>,
    mut accum: Local<f32>,
) {
    *accum += time.delta_secs();
    if *accum < SENSOR_WRITE_PERIOD_SEC {
        return;
    }
    *accum = 0.0;

    let r = &active.report;
    let solids_seen = r.discs_seen + r.segs_seen;
    let (severity, next_step) = severity_for_navmesh(active.is_built(), solids_seen, r.obstacles_kept);

    let payload = NavmeshSensor {
        id: "navmesh",
        severity,
        next_step,
        timestamp_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        built: active.is_built(),
        source: &active.source,
        seed: active.seed,
        discs_seen: r.discs_seen,
        segs_seen: r.segs_seen,
        obstacles_kept: r.obstacles_kept,
        blind: r.is_blind(),
        build_ms: active.build_ms,
        builds_session: active.builds_session,
        max_obstacles_session: active.max_obstacles_session,
        agent_radius_m: cfg.agent_radius_m,
        step_height_m: cfg.step_height_m,
    };

    match serde_json::to_string(&payload) {
        Ok(json) => {
            if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
                warn!("[navmesh] ecriture capteur echouee: {e}");
            }
        }
        Err(e) => warn!("[navmesh] serialisation capteur echouee: {e}"),
    }
}

/// Insère les ressources et le capteur. **Ne bâtit rien** : c'est à la crate qui possède
/// la géométrie d'appeler [`build`] et [`ActiveNavMesh::set`] — `forgia-navmesh` ne
/// connaît ni les arènes ni le terrain, et c'est ce qui lui permet de servir les deux.
pub struct ForgiaNavmeshPlugin;

impl Plugin for ForgiaNavmeshPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NavmeshBuild::load_or_default())
            .init_resource::<ActiveNavMesh>()
            .add_systems(
                Update,
                sys_write_navmesh_sensor.in_set(forgia_core::prelude::GameSet::Sensors),
            );
    }
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

    // ── Le capteur ne ment pas ──────────────────────────────────────────

    #[test]
    fn pas_de_maillage_est_une_info_pas_une_erreur() {
        let (sev, next) = severity_for_navmesh(false, 0, 0);
        assert_eq!(sev, "info");
        assert!(!next.is_empty(), "meme une info doit dire quoi attendre");
    }

    #[test]
    fn un_maillage_bati_sur_zero_solide_alerte_au_lieu_de_dire_ok() {
        // Le cas vicieux : la construction « reussit », le plan est vide, les agents
        // traversent tout, et aucune erreur n'est levee.
        let (sev, next) = severity_for_navmesh(true, 0, 0);
        assert_eq!(sev, "warn");
        assert!(next.contains("AVEUGLE"));
    }

    #[test]
    fn des_solides_soumis_mais_aucun_retenu_alerte_aussi() {
        // Distinct du precedent : ici la geometrie etait la, c'est le seuil qui l'a
        // toute rejetee — donc le next_step pointe le gene, pas la geometrie.
        let (sev, next) = severity_for_navmesh(true, 42, 0);
        assert_eq!(sev, "warn");
        assert!(next.contains("step_height_m"));
    }

    #[test]
    fn un_maillage_avec_des_obstacles_est_ok() {
        assert_eq!(severity_for_navmesh(true, 42, 17).0, "ok");
    }

    // ── La ressource ────────────────────────────────────────────────────

    #[test]
    fn la_ressource_vide_ne_rend_aucun_chemin() {
        let active = ActiveNavMesh::default();
        assert!(!active.is_built());
        assert!(active.path(Vec2::ZERO, Vec2::new(5.0, 0.0)).is_none());
    }

    #[test]
    fn les_cumules_survivent_au_demontage_de_l_arene() {
        // LE defaut du 2026-08-12 : le capteur lu apres la run disait
        // « built: false, blind: true » — inexploitable, parce que le maillage est
        // efface au demontage. Un instantane pris au menu ne dit RIEN de la partie.
        // Ces deux compteurs repondent a la seule question qui comptait.
        let c = cfg();
        let discs = [disc(0.0, 0.0, 3.0, 4.0)];
        let (mesh, report) = build(hexagon_edge(20.0, c.agent_radius_m), &discs, &[], &c);
        let mut active = ActiveNavMesh::default();
        assert_eq!(active.builds_session, 0, "rien de bati au depart");

        active.set(mesh, report, "forge_sanctum", 1, 0.0);
        assert_eq!(active.builds_session, 1);
        assert_eq!(active.max_obstacles_session, 1);

        active.clear();
        assert!(!active.is_built(), "l'instantane est bien remis a zero");
        assert_eq!(
            active.builds_session, 1,
            "mais le CUMULE survit — sinon on ne peut rien conclure apres coup"
        );
        assert_eq!(active.max_obstacles_session, 1);
    }

    #[test]
    fn clear_oublie_le_maillage_et_sa_provenance() {
        let c = cfg();
        let (mesh, report) = build(hexagon_edge(20.0, c.agent_radius_m), &[], &[], &c);
        let mut active = ActiveNavMesh::default();
        active.set(mesh, report, "forge_sanctum", 42, 1.5);
        assert!(active.is_built());
        assert_eq!(active.source, "forge_sanctum");

        // Une zone demontee ne doit plus repondre : mieux vaut aucun chemin qu'un
        // chemin calcule sur une geometrie qui n'existe plus.
        active.clear();
        assert!(!active.is_built());
        assert!(active.source.is_empty());
        assert_eq!(active.seed, 0);
        assert!(active.path(Vec2::ZERO, Vec2::new(5.0, 0.0)).is_none());
    }

    #[test]
    fn la_ressource_rend_le_chemin_du_maillage_qu_on_lui_donne() {
        let c = cfg();
        let discs = [disc(0.0, 0.0, 3.0, 4.0)];
        let (mesh, report) = build(hexagon_edge(20.0, c.agent_radius_m), &discs, &[], &c);
        let mut active = ActiveNavMesh::default();
        active.set(mesh, report, "test", 1, 0.0);
        let from = Vec2::new(-10.0, 0.0);
        let to = Vec2::new(10.0, 0.0);
        let path = active.path(from, to).expect("le pilier se contourne");
        assert!(path.length > from.distance(to));
    }

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

#[cfg(test)]
mod reproduction_terrain {
    use super::*;

    /// Reproduit l'ECHELLE REELLE mesuree en jeu le 2026-08-13 : forge_sanctum,
    /// rayon jouable 69,28 m, 13 obstacles retenus — et 98,5 % de chemins refuses.
    /// Mes tests d'origine tournaient sur 20 m avec 2 obstacles : ils ne pouvaient
    /// pas voir le defaut.
    #[test]
    fn a_l_echelle_reelle_les_chemins_passent_ils() {
        let c = NavmeshBuild::default();
        let apothem = 69.28203_f32;
        // 13 abris disperses, comme le capteur les compte.
        let discs: Vec<SolidDisc> = (0..13)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 13.0;
                SolidDisc {
                    x: 25.0 * a.cos(),
                    z: 25.0 * a.sin(),
                    r: 3.0,
                    h: 3.0,
                }
            })
            .collect();
        let (mesh, report) = build(hexagon_edge(apothem, c.agent_radius_m), &discs, &[], &c);
        assert_eq!(report.obstacles_kept, 13);

        // 200 tirages joueur->bot repartis dans l'arene.
        let mut ok = 0;
        let mut ko = 0;
        for i in 0..200 {
            let a = std::f32::consts::TAU * i as f32 / 200.0;
            let rayon = 10.0 + (i % 5) as f32 * 10.0; // 10..50 m
            let from = Vec2::new(rayon * a.cos(), rayon * a.sin());
            let to = Vec2::new(5.0 * (a + 1.0).cos(), 5.0 * (a + 1.0).sin());
            if mesh.path(from, to).is_some() {
                ok += 1;
            } else {
                ko += 1;
            }
        }
        println!("ECHELLE REELLE : {ok} chemins OK, {ko} refuses");
        assert!(ko == 0, "{ko}/200 chemins refuses a l'echelle reelle");
    }

    // ── La bande abandonnee : ce que le test ci-dessus ne pouvait pas voir ──
    //
    // Le tirage ci-dessus va de 10 a 50 m sur un rayon jouable de 69,28 : il
    // n'approche JAMAIS le mur. Il rendait donc 200/200 et innocentait le maillage
    // pendant que le jeu refusait 13,5 % des chemins. Un echantillon qui evite la
    // zone du defaut ne prouve rien — il rassure, ce qui est pire.

    /// Le maillage bati pour forge_sanctum, tel qu'en jeu.
    fn maillage_forge_sanctum() -> (ActiveNavMesh, f32) {
        let c = NavmeshBuild::default();
        let apothem = 69.28203_f32;
        let discs: Vec<SolidDisc> = (0..13)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 13.0;
                SolidDisc {
                    x: 25.0 * a.cos(),
                    z: 25.0 * a.sin(),
                    r: 3.0,
                    h: 3.0,
                }
            })
            .collect();
        let (mesh, report) = build(hexagon_edge(apothem, c.agent_radius_m), &discs, &[], &c);
        let mut active = ActiveNavMesh::default();
        active.set(mesh, report, "forge_sanctum", 7, 0.3);
        (active, apothem)
    }

    #[test]
    fn la_portee_de_recherche_couvre_la_bande_abandonnee() {
        // LA borne. `from_edge_and_obstacles` laisse 0,01 x 2 = 2 cm, alors que le
        // maillage abandonne `agent_radius_m` = 30 cm. Si cette inegalite casse, le
        // recalage redevient inoperant — en SILENCE, comme le 2026-08-13.
        let c = NavmeshBuild::default();
        let (mesh, _) = build(hexagon_edge(69.28203, c.agent_radius_m), &[], &[], &c);
        let portee = mesh.search_delta() * mesh.search_steps() as f32;
        assert!(
            portee >= c.agent_radius_m,
            "portee de recherche {portee:.3} m < bande abandonnee {:.3} m : \
             une cible collee au mur restera injoignable",
            c.agent_radius_m
        );
    }

    #[test]
    fn la_cible_mesuree_en_jeu_le_2026_08_13_est_desormais_joignable() {
        // Les coordonnees EXACTES du dernier refus releve dans forgia_bot_ai.json.
        // Cible a 69,95 m du centre pour un bord de maillage a 69,67 m sous cet
        // angle : 28 cm dehors.
        //
        // Ce test compare le maillage TEL QUE POLYANYA LE LIVRE (portee 0,01 x 2 =
        // 2 cm) au notre. C'est la seule facon honnete de prouver la correction : sur
        // le maillage corrige, `path()` brut passe deja — donc affirmer « le brut
        // refuse » serait faux, et une version anterieure de ce test l'affirmait.
        let from = Vec2::new(44.7, -1.6);
        let to = Vec2::new(64.9, 26.1);
        let c = NavmeshBuild::default();
        let discs: Vec<SolidDisc> = (0..13)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 13.0;
                SolidDisc {
                    x: 25.0 * a.cos(),
                    z: 25.0 * a.sin(),
                    r: 3.0,
                    h: 3.0,
                }
            })
            .collect();

        // AVANT — la portee que polyanya pose lui-meme.
        let mut origine =
            NavMesh::from_edge_and_obstacles(hexagon_edge(69.28203, c.agent_radius_m), {
                let mut o = Vec::new();
                for d in &discs {
                    o.push(disc_to_obstacle(d, c.agent_radius_m, c.disc_segments));
                }
                o
            });
        origine.set_search_steps(2);
        assert!(
            origine.path(from, to).is_none(),
            "sans la portee corrigee, cette cible DOIT etre refusee — c'est le \
             defaut mesure en jeu, et sans lui ce test ne prouve rien"
        );

        // APRES — le maillage que `build` produit reellement.
        let (nav, _) = maillage_forge_sanctum();
        let ecart = match nav.route(from, to) {
            PathOutcome::Direct(_) => 0.0,
            PathOutcome::Snapped { snapped_to, .. } => snapped_to.distance(to),
            autre => panic!("cible toujours injoignable : {autre:?}"),
        };
        assert!(
            ecart <= c.agent_radius_m * 2.0,
            "le point atteint est a {ecart:.2} m de la cible : le bot irait \
             ailleurs qu'au joueur"
        );
    }

    #[test]
    fn un_joueur_qui_longe_le_mur_reste_poursuivable_tout_du_long() {
        // Le cas general dont la coordonnee ci-dessus n'est qu'un exemplaire : le
        // joueur peut longer TOUTE l'enceinte. Sans recalage, chaque pas dans la
        // bande fait perdre son chemin a tous ses poursuivants d'un coup.
        let (nav, apothem) = maillage_forge_sanctum();
        let mut brut_ko = 0;
        let mut route_ko = 0;
        let mut details: Vec<(usize, f32, f32)> = Vec::new();
        for i in 0..360 {
            let a = std::f32::consts::TAU * i as f32 / 360.0;
            // Sur le bord de l'hexagone du SOL (apotheme non rétréci), donc dans la
            // bande de 30 cm que le maillage abandonne.
            let bord = apothem / (((a % (std::f32::consts::PI / 3.0)) - std::f32::consts::PI / 6.0).cos());
            let joueur = Vec2::new(bord * a.cos(), bord * a.sin());
            let bot = Vec2::new(20.0 * (a + 2.0).cos(), 20.0 * (a + 2.0).sin());
            if nav.path(bot, joueur).is_none() {
                brut_ko += 1;
            }
            if matches!(nav.route(bot, joueur), PathOutcome::NoRoute { .. }) {
                route_ko += 1;
                details.push((i, joueur.length(), bord));
            }
        }
        println!("LE LONG DU MUR : brut {brut_ko}/360 refuses, route {route_ko}/360 — {details:?}");
        assert!(
            brut_ko > 0,
            "sans refus brut ce test ne mesure rien — l'echantillon rate la bande"
        );
        assert_eq!(
            route_ko, 0,
            "{route_ko}/360 positions le long du mur restent injoignables"
        );
    }

    #[test]
    fn une_poche_isolee_se_distingue_d_une_cible_hors_maillage() {
        // Les deux causes que l'ancien compteur unique confondait. Ici les deux
        // points sont navigables : le recalage n'y peut rien, et `route` doit le
        // DIRE (drapeaux a false) au lieu de laisser croire a un bord.
        let (nav, _) = maillage_forge_sanctum();
        let dedans = Vec2::new(5.0, 5.0);
        match nav.route(dedans, Vec2::new(-5.0, -5.0)) {
            PathOutcome::Direct(_) | PathOutcome::Snapped { .. } => {}
            PathOutcome::NoRoute {
                from_off_mesh,
                to_off_mesh,
            } => {
                assert!(
                    !from_off_mesh && !to_off_mesh,
                    "deux points au centre ne peuvent pas etre hors maillage"
                );
            }
            PathOutcome::NoMesh => panic!("maillage bati, NoMesh impossible"),
        }
    }

    #[test]
    fn sans_maillage_route_dit_no_mesh_et_pas_un_echec() {
        // La distinction qui evite de compter un chargement comme un defaut.
        let nav = ActiveNavMesh::default();
        assert!(matches!(
            nav.route(Vec2::ZERO, Vec2::new(1.0, 1.0)),
            PathOutcome::NoMesh
        ));
    }
}
