//! plugin.rs — charger « Le Vallon » et pouvoir y marcher.
//!
//! Premier incrément : **le décor, les collisions, le spawn**. Pas d'ennemis,
//! pas de vagues, pas de porte. C'est observable en une run — on marche les
//! 358,7 m du chemin, ou on ne les marche pas.
//!
//! # Ce que ce fichier ne calcule pas
//!
//! La décision de streaming vient de [`forgia_streaming::cells`], partagée avec
//! le Château. La conversion de repère vient de [`crate::manifest`]. Ici on ne
//! fait que spawner et despawner — c'est délibéré : ce qui se décide doit rester
//! testable sans moteur.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_core::prelude::GameMode;
use forgia_player::Player;
use forgia_streaming::cells::{self, CellAction, CellManifest, StreamRadii};

use crate::manifest::ExpeditionManifest;

const MANIFESTE_GAMEPLAY: &str = "assets/models/environment/expedition/expedition_vallon.json";
const MANIFESTE_CELLULES: &str =
    "assets/models/environment/expedition/vallon_stream_cells/vallon_stream_cells.toml";

/// Le SOL collidable — un maillage unique, décimé, réservé à la physique.
///
/// # Pourquoi il ne se déduit pas des cellules
///
/// Les 48 cellules du décor portent le champ `render` : elles sont **du rendu
/// pur**. Et les 943 cylindres du manifeste sont les *props* — troncs, rochers,
/// murs. Aucun des deux n'est le terrain.
///
/// Sans ce fichier, le joueur apparaît au bon endroit et **traverse le monde** :
/// relevé en jeu le 2026-08-14, spawn correct à (-124,0 ; 4,5 ; 10,0) et chute
/// immédiate. Le symptôme — « je suis sous la map » — n'évoque pas un asset
/// manquant, il évoque un mauvais spawn. C'est ce qui le rend coûteux.
///
/// Même montage que le Château (`castle_ground.rs`) : un `AsyncSceneCollider` en
/// TriMesh sur une version décimée, invisible. Un seul collider au lieu des
/// milliers que produirait le GLB complet.
const SOL_COLLISION: &str = "models/environment/expedition/expedition_vallon_collision.glb#Scene0";

/// Les deux battants de la porte du village.
///
/// Comme la rivière, ce fichier est **hors de la grille de streaming** et
/// n'était cité nulle part : il dormait sur le disque. Il est chargé statique
/// pour l'instant — l'ouverture au passage du joueur (`porte_village` du
/// manifeste donne son rayon et sa durée) reste à faire. Le montrer fermé est
/// déjà mieux que de ne pas le montrer : le village a une entrée lisible.
const MODELE_PORTES: &str =
    "models/environment/expedition/vallon_stream_cells/vallon_portes.gltf#Scene0";

/// Rayon de chargement des cellules (m).
///
/// **Dérivé de l'emprise**, pas choisi : la carte fait 280 × 200 m, sa
/// demi-diagonale vaut donc ~172 m. Un rayon de 140 m couvre largement ce qu'on
/// voit depuis n'importe quel point du chemin sans charger la carte entière —
/// et le déchargement s'en déduit (`StreamRadii::from_load`), il ne se choisit
/// pas séparément.
const RAYON_CHARGEMENT_M: f32 = 140.0;

/// Hauteur des cylindres de collision (m).
///
/// Le manifeste donne `[x, y, z, rayon]` : la base et l'emprise, pas la hauteur.
/// 6 m couvre un tronc d'arbre du kit (mesuré : `tree_default` 1,708 × 4 = 6,8 m)
/// et dépasse largement le saut du joueur (1,174 m), donc rien ne se franchit
/// par le dessus. Un cylindre trop court laisserait sauter par-dessus un arbre.
const HAUTEUR_COLLIDER_M: f32 = 6.0;

/// Marqueur : tout ce qui appartient à l'expédition, pour un démontage propre.
#[derive(Component)]
pub struct ExpeditionMarker;

/// Une cellule de décor présente à l'écran.
#[derive(Component)]
pub struct ExpeditionCell {
    pub id: String,
}

/// La carte chargée. Absente = aucune expédition en cours.
#[derive(Resource)]
pub struct ActiveExpedition {
    pub gameplay: ExpeditionManifest,
    pub cells: CellManifest,
    pub radii: StreamRadii,
    /// Cumulés de session — ils SURVIVENT au démontage, sinon une lecture faite
    /// au menu ne dirait rien de ce qui s'est passé pendant la partie (leçon du
    /// capteur navmesh, 2026-08-12).
    pub cells_loaded_session: u32,
    pub colliders_spawned: u32,
    /// Le joueur reste-t-il à placer ?
    ///
    /// # Le piège de séquencement que ce drapeau évite
    ///
    /// Le joueur est spawné sur `OnEnter(AppMode::InGame)` par `forgia-player`,
    /// **via `Commands`** — donc il n'existe pas encore quand
    /// `OnEnter(GameMode::Expedition)` tourne dans la même frame. Placer le
    /// joueur là serait un `q_player.single()` qui échoue, un avertissement dans
    /// le log, et un joueur laissé à l'origine : il tomberait, à 124 m de la
    /// carte. Le symptôme — « je tombe à l'infini » — n'évoque PAS un problème
    /// d'ordre de systèmes.
    ///
    /// On arme donc, et un système d'`Update` place dès que le joueur paraît.
    /// Même motif que `NeedsSpawnSnap` côté Roguelite.
    pub spawn_pending: bool,
}

pub struct ForgiaExpeditionPlugin;

impl Plugin for ForgiaExpeditionPlugin {
    fn build(&self, app: &mut App) {
        // L'avatar et ses feux vivent dans leur propre plugin : il apporte sa
        // ressource de réglages, et rien ici n'a besoin de le savoir.
        // L'arme tenue en main a le sien, pour la même raison — et parce qu'elle
        // publie la bouche du canon, que le chemin de tir lit sans rien savoir
        // de l'Expédition.
        app.add_plugins((
            crate::avatar_vfx::ExpeditionAvatarPlugin,
            crate::arme_main::ExpeditionArmeMainPlugin,
            // La visée lit le génome de l'arme en main : elle vient après, mais
            // l'ordre d'ajout des plugins ne décide de rien ici — chacun
            // n'expose que sa ressource.
            crate::visee::ExpeditionViseePlugin,
        ))
            .add_systems(
                OnEnter(GameMode::Expedition),
                (
                    reprendre_la_main_sur_le_reticule,
                    setup_expedition,
                    crate::eau::setup_eau,
                    crate::lighting::setup_expedition_lighting,
                    // APRÈS `setup_expedition` : les braseros se lisent dans le
                    // manifeste, que celui-ci vient de charger.
                    crate::lampes::setup_lampes,
                )
                    .chain(),
            )
            .add_systems(
                OnExit(GameMode::Expedition),
                (
                    teardown_expedition,
                    crate::lighting::teardown_expedition_lighting,
                    // Sans ça, l'Expédition laisse seize points lumineux
                    // allumés dans l'arène et le Hall.
                    crate::lampes::teardown_lampes,
                ),
            )
            .add_systems(
                Update,
                (
                    place_player_when_ready,
                    stream_expedition_cells,
                    // La rivière : reprendre son matériau dès que sa scène est
                    // peuplée, puis la faire couler à la vitesse de la carte.
                    crate::eau::reprendre_materiau_eau,
                    crate::eau::faire_couler,
                    // Le cycle APRES le placement : la progression se lit sur la
                    // position REELLE du joueur, pas sur celle d'avant son spawn.
                    crate::lighting::update_expedition_cycle,
                    crate::lighting::update_expedition_ambiance,
                    // Les braseros APRES le cycle : ils lisent l'élévation du
                    // soleil et la progression que celui-ci vient d'écrire.
                    // Avant lui, ils travailleraient sur la frame précédente.
                    crate::lampes::update_lampes,
                )
                    .chain()
                    .run_if(in_state(GameMode::Expedition)),
            );
    }
}

/// Remet le réticule dans un état que ce mode assume.
///
/// # Pourquoi un mode doit déclarer ça lui-même
///
/// Deux ressources décident si un réticule s'affiche, et **aucun système de
/// l'Expédition ne les écrit** :
///
/// - `CrosshairHidden` n'a que deux écrivains, tous deux accrochés au Lobby du
///   Roguelite (`OnEnter/OnExit(RunState::Lobby)`). Rien ne garantit qu'ils
///   aient rendu la main avant qu'on arrive ici.
/// - `CrosshairMode` (progression de visée, lunette plein écran) n'est écrite
///   que par le plugin de pose du viewmodel, gaté `Fps | Roguelite`. En
///   Expédition elle **garde donc sa dernière valeur** : une partie de Roguelite
///   terminée l'œil dans la lunette de Madame Lenoir suffit à supprimer le
///   réticule ici, sans qu'aucun code de ce mode n'ait rien fait.
///
/// Un état hérité d'un autre mode est un état que personne ne possède. On le
/// remet donc à neuf en entrant — même geste que la bouche du canon, remise à
/// `None` en sortant (`arme_main::retirer_arme`).
fn reprendre_la_main_sur_le_reticule(
    cache: Option<ResMut<forgia_crosshair::CrosshairHidden>>,
    mode: Option<ResMut<forgia_crosshair::CrosshairMode>>,
) {
    if let Some(mut c) = cache {
        c.0 = false;
    }
    if let Some(mut m) = mode {
        // Pas de visée en Expédition : le plugin qui l'anime ne tourne pas ici.
        // Laisser une progression non nulle ferait disparaître la croix au
        // profit d'un point rouge, ou d'une lunette, que rien ne ferait revenir.
        m.ads_progress = 0.0;
        m.sniper_fullscreen = false;
    }
}

/// Charge les deux manifestes, pose les colliders, place le joueur.
fn setup_expedition(mut commands: Commands, asset_server: Res<AssetServer>) {
    // `def_io` et non `std::fs` : sur wasm un `std::fs` échoue EN SILENCE et le
    // jeu tournerait sur une carte vide sans qu'aucune erreur ne le dise.
    let gameplay = match forgia_core::def_io::read_def_str(MANIFESTE_GAMEPLAY)
        .map_err(|e| e.to_string())
        .and_then(|s| ExpeditionManifest::parse(&s).map_err(|e| e.to_string()))
    {
        Ok(m) => m,
        Err(e) => {
            error!("[expedition] manifeste de gameplay illisible : {e}");
            return;
        }
    };
    let cells = match forgia_core::def_io::read_def_str(MANIFESTE_CELLULES)
        .map_err(|e| e.to_string())
        .and_then(|s| cells::parse_manifest(&s).map_err(|e| e.to_string()))
    {
        Ok(m) => m,
        Err(e) => {
            error!("[expedition] manifeste de cellules illisible : {e}");
            return;
        }
    };

    // LE SOL. Posé en premier : sans lui tout le reste flotte, et le joueur
    // traverse le monde (mesuré le 2026-08-14). Invisible — le décor visible
    // vient des cellules ; celui-ci n'existe que pour la physique.
    commands.spawn((
        Name::new("ExpeditionGroundCollision"),
        ExpeditionMarker,
        SceneRoot(asset_server.load(SOL_COLLISION)),
        Transform::IDENTITY,
        RigidBody::Fixed,
        AsyncSceneCollider {
            shape: Some(ComputedColliderShape::TriMesh(default())),
            ..default()
        },
        Visibility::Hidden,
    ));

    // La porte du village. Coordonnées monde Bevy comme les cellules : aucun
    // placement, elle est déjà à sa place dans le fichier.
    commands.spawn((
        Name::new("ExpeditionPortesVillage"),
        ExpeditionMarker,
        SceneRoot(asset_server.load(MODELE_PORTES)),
        Transform::IDENTITY,
        Visibility::Inherited,
    ));

    // Les colliders : 943 cylindres, posés une fois. Ils ne streament PAS —
    // retirer une collision sous les pieds du joueur parce qu'il s'est éloigné
    // du décor visuel serait le pire échange possible (même choix que le
    // Château, cf. `castle_hub`).
    let mut poses = 0u32;
    for (centre, rayon) in gameplay.colliders_bevy() {
        commands.spawn((
            Name::new("ExpeditionProp"),
            ExpeditionMarker,
            Transform::from_translation(centre + Vec3::Y * (HAUTEUR_COLLIDER_M * 0.5)),
            GlobalTransform::default(),
            RigidBody::Fixed,
            Collider::cylinder(HAUTEUR_COLLIDER_M * 0.5, rayon),
        ));
        poses += 1;
    }

    let depart = gameplay.spawn_player_origin();

    // Le contrôle de conception qui ne peut pas vivre dans un test : un
    // campement dont la plus longue ligne dépasse la vision de ses ennemis leur
    // fait subir du tir SANS pouvoir répondre (`map-design-intention.md` §2.2).
    // C'est un défaut de CARTE — il se corrige sous Blender ou au génome, pas
    // ici. Il doit donc se voir à chaque chargement plutôt que bloquer un build
    // qui ne peut rien pour lui.
    for c in &gameplay.campements {
        if !c.vision_couvre_la_ligne() {
            warn!(
                "[expedition] {} : ligne de vue max {:.0} m pour une vision de grunt \
                 de {:.0} m — {:.0} m de TIR GRATUIT. Remede : casser la ligne sous \
                 Blender, ou elargir la vision au genome. Ce n'est pas un bug moteur.",
                c.id,
                c.ligne_max_m,
                c.grunt_vision_m,
                c.ligne_max_m - c.grunt_vision_m
            );
        }
    }

    let radii = StreamRadii::from_load(RAYON_CHARGEMENT_M, cells.cell_size_m);
    info!(
        "[expedition] « {} » chargee — sol {SOL_COLLISION}, {} cellules de {:.0} m, \
         {poses} props, chemin {:.1} m, spawn ({:.1}, {:.1}, {:.1}), rayons {:.0}/{:.0} m",
        gameplay.carte,
        cells.cells.len(),
        cells.cell_size_m,
        gameplay.longueur_chemin_m(),
        depart.x,
        depart.y,
        depart.z,
        radii.load_m,
        radii.unload_m,
    );
    commands.insert_resource(ActiveExpedition {
        gameplay,
        cells,
        radii,
        cells_loaded_session: 0,
        colliders_spawned: poses,
        spawn_pending: true,
    });
}

/// Place le joueur au départ de la carte, **dès qu'il existe**.
///
/// Il naît sur `OnEnter(AppMode::InGame)` via `Commands`, donc après le setup de
/// ce mode. Attendre est la seule façon correcte : le placer trop tôt, c'est ne
/// pas le placer du tout — et un joueur laissé à l'origine tombe, à 124 m de la
/// carte. Le drapeau se désarme au premier succès, donc ce système ne coûte
/// qu'un test de booléen le reste de la partie.
fn place_player_when_ready(
    active: Option<ResMut<ActiveExpedition>>,
    mut q_player: Query<&mut Transform, With<Player>>,
) {
    let Some(mut active) = active else { return };
    if !active.spawn_pending {
        return;
    }
    let Ok(mut xf) = q_player.single_mut() else {
        return; // pas encore né — on retentera la frame suivante
    };
    let depart = active.gameplay.spawn_player_origin();
    let regard = active.gameplay.regard_bevy();
    *xf = Transform::from_translation(depart)
        .looking_at(Vec3::new(regard.x, depart.y, regard.y), Vec3::Y);
    active.spawn_pending = false;
    info!(
        "[expedition] joueur place a ({:.1}, {:.1}, {:.1}), regard vers ({:.1}, {:.1})",
        depart.x, depart.y, depart.z, regard.x, regard.y
    );
}

/// Charge et décharge les cellules visuelles selon la distance au joueur.
fn stream_expedition_cells(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    q_player: Query<&Transform, With<Player>>,
    active: Option<ResMut<ActiveExpedition>>,
    q_loaded: Query<(Entity, &ExpeditionCell)>,
) {
    let Some(mut active) = active else {
        return;
    };
    let Ok(joueur) = q_player.single() else {
        return;
    };
    let pos = joueur.translation;

    // Décharger d'abord : libérer avant de demander garde le pic mémoire borné.
    for (entity, cell) in &q_loaded {
        let Some(spec) = active.cells.cells.iter().find(|s| s.id == cell.id) else {
            // Cellule runtime absente du manifeste : le manifeste a changé sous
            // nos pieds (hot-reload d'asset). On retire plutôt que de garder un
            // décor qui n'est plus décrit nulle part.
            commands.entity(entity).despawn();
            continue;
        };
        if cells::decide(
            cells::horizontal_distance(pos, spec),
            true,
            active.radii,
        ) == CellAction::Decharger
        {
            commands.entity(entity).despawn();
        }
    }

    let presentes: Vec<&str> = q_loaded.iter().map(|(_, c)| c.id.as_str()).collect();
    let mut a_charger: Vec<(String, String)> = Vec::new();
    for spec in &active.cells.cells {
        if presentes.contains(&spec.id.as_str()) {
            continue;
        }
        if cells::decide(
            cells::horizontal_distance(pos, spec),
            false,
            active.radii,
        ) == CellAction::Charger
        {
            a_charger.push((spec.id.clone(), spec.render.clone()));
        }
    }
    for (id, render) in a_charger {
        commands.spawn((
            Name::new(format!("ExpeditionCell_{id}")),
            ExpeditionCell { id },
            ExpeditionMarker,
            SceneRoot(asset_server.load(render)),
            Transform::IDENTITY,
            GlobalTransform::default(),
        ));
        active.cells_loaded_session = active.cells_loaded_session.saturating_add(1);
    }
}

/// Démonte tout. La ressource part avec : une carte qui survit à sa sortie
/// enverrait le streaming travailler sur une géométrie disparue.
fn teardown_expedition(mut commands: Commands, q: Query<Entity, With<ExpeditionMarker>>) {
    let mut n = 0u32;
    for e in &q {
        commands.entity(e).despawn();
        n += 1;
    }
    commands.remove_resource::<ActiveExpedition>();
    info!("[expedition] demontee — {n} entites retirees");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_rayon_de_chargement_couvre_ce_qu_on_voit_sans_charger_toute_la_carte() {
        // Derive de l'emprise : 280 x 200 m, demi-diagonale ~172 m. Le rayon doit
        // depasser la demi-largeur (100 m) — sinon on verrait le decor apparaitre
        // devant soi — sans atteindre la demi-diagonale, sinon la carte entiere
        // est toujours chargee et le streaming ne sert a rien.
        let demi_diagonale = (280.0_f32.powi(2) + 200.0_f32.powi(2)).sqrt() * 0.5;
        assert!(RAYON_CHARGEMENT_M > 100.0, "le decor apparaitrait devant le joueur");
        assert!(
            RAYON_CHARGEMENT_M < demi_diagonale,
            "rayon {RAYON_CHARGEMENT_M} >= demi-diagonale {demi_diagonale:.0} : \
             la carte entiere serait toujours chargee"
        );
    }

    #[test]
    fn un_collider_de_prop_ne_se_franchit_pas_au_saut() {
        // Le joueur saute 1,174 m (metrique mesuree du projet). Un cylindre plus
        // court se franchirait par le dessus — on passerait a travers un arbre en
        // sautant, et le symptome serait « les collisions ne marchent pas ».
        const SAUT_JOUEUR_M: f32 = 1.174;
        assert!(
            HAUTEUR_COLLIDER_M > SAUT_JOUEUR_M,
            "un prop de {HAUTEUR_COLLIDER_M} m se franchit au saut ({SAUT_JOUEUR_M} m)"
        );
    }

    #[test]
    fn les_rayons_derives_restent_coherents() {
        let r = StreamRadii::from_load(RAYON_CHARGEMENT_M, 40.0);
        assert!(r.is_coherent());
    }
}
