//! arme_main.rs — L'arme équipée, tenue dans la main du personnage.
//!
//! # Ce module ne spawne PAS de personnage, et ne décide PAS du tir
//!
//! Même partage que `avatar_vfx` : le corps de l'Expédition est posé par
//! `forgia-mode-roguelite::avatar`, monté par `forgia-game::castle_avatar`. Ici
//! on accroche un objet à un os, et on publie **où sort la balle à l'écran**.
//! Le rayon, lui, part de la caméra (`forgia-fps`) — c'est ce qui fait que le
//! réticule dit vrai.
//!
//! # L'accroche se fait sur un socket, qui est un enfant d'OS
//!
//! `socket_main_droite` est un nœud du corps d'Expédition, enfant de
//! `RightHand`. Être **enfant** est tout le sujet : une entité posée à la
//! position de la main resterait sur place pendant que le personnage court.
//! Chaîne relevée dans le glTF :
//!
//! ```text
//! socket_main_droite < RightHand < RightForeArm < RightArm < RightShoulder
//!                    < Spine2 < Spine1 < Spine < Hips < perso_squelette
//! ```
//!
//! # Trois pièges du dépôt, payés avant ce fichier
//!
//! 1. **Jamais d'échelle en dur sur un GLB importé** (`no-hardcode.md`). On
//!    déclare la longueur voulue, on mesure l'AABB du maillage chargé, on en
//!    dérive le facteur. C'est le motif `auto_scale_viewmodel` de
//!    `forgia-viewmodel`, appliqué ici à une main plutôt qu'à une caméra.
//! 2. **L'avatar est reconstruit** dès que l'équipement porté change
//!    (`sys_sync_hall_avatar` despawn puis respawn la racine). Un socket
//!    mémorisé dans une ressource deviendrait une entité morte, en silence. On
//!    marque donc l'entité elle-même : le marqueur meurt avec elle, et son
//!    absence relance la recherche.
//! 3. **La dague du corps est dans la MÊME main** (`SM_Dagger`, enfant de
//!    `RightHand`). Sans la masquer, l'arme à feu et elle s'interpénètrent dans
//!    le poing.

use std::collections::HashMap;

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use serde::Deserialize;

use forgia_combat::weapons::{EquippedWeapons, MuzzleWorld, WeaponType};
use forgia_core::prelude::{AppMode, GameMode, GameSet};
use forgia_genome_core::{Genome, GenomeLoader};

/// Le socket de main droite. Posé par `tools/blender/personnage/26_cape_ardente.py`
/// dans `stylized_male_complet.glb` — le nom est donc écrit à deux endroits, et
/// le test `le_socket_vise_est_celui_du_corps_d_expedition` le rappelle.
const SOCKET_MAIN: &str = "socket_main_droite";

/// L'os de repli, quand le corps n'a pas de socket.
///
/// # La leçon que le module voisin avait écrite, et que je n'ai pas suivie
///
/// `avatar_vfx.rs` s'accroche aux OS et le dit dans son en-tête : « les sockets
/// existent dans `stylized_male_complet.glb`, mais l'autre corps disponible n'en
/// a aucun, alors que les 62 os sont identiques ». J'ai quand même visé le
/// socket. Le 2026-08-16, le corps d'Expédition est devenu
/// `chien_expedition.glb` — même rig, tête de chien — bâti par une autre chaîne
/// d'outils : **aucun de ses 83 nœuds n'est un socket**, alors que `RightHand`,
/// `RightArm` et `Spine2` y sont. Résultat : plus aucune arme en main.
///
/// Un os fait partie du squelette ; un socket est un ajout d'outillage, et il
/// disparaît dès qu'on change d'outil. On vise donc l'os, et on ne prend le
/// socket que s'il est là — il est plus précisément placé (4,5 cm au-dessus du
/// poignet), mais il n'est jamais nécessaire : la correction d'orientation se
/// dérive de l'ancrage RÉELLEMENT utilisé, quel qu'il soit.
const OS_MAIN_DROITE: &str = "RightHand";

/// La dague que le corps tient déjà dans cette main.
const NOEUD_DAGUE: &str = "SM_Dagger";

/// Couche **definition** : tout ce qui se juge à l'œil vit là, et s'y recharge
/// à chaud pendant que le jeu tourne.
const GENOME: &str = "genomes/expedition_arme_main.toml";

/// Où le GLB d'une arme se trouve. Le nom du fichier EST la clé de génome de
/// l'arme (`WeaponType::genome_key`) — une seule table, dans `forgia-combat`.
fn chemin_glb(arme: WeaponType) -> String {
    format!("models/weapons/forgia/{}.glb#Scene0", arme.genome_key())
}

/// Une AABB dont la plus grande arête dépasse ce seuil est corrompue, pas
/// grande : c'est la signature d'une quantification `i16` mal décodée, qui rend
/// des dizaines de milliers de mètres. Même garde que `auto_scale_viewmodel`.
const AABB_ABERRANTE_M: f32 = 100.0;

// ---------------------------------------------------------------------------
// Couche definition
// ---------------------------------------------------------------------------

#[derive(TypePath, Deserialize, Debug, Clone, Default)]
pub struct ArmeMainGenome {
    #[serde(default)]
    pub dague: DagueReglage,
    #[serde(default)]
    pub armes: HashMap<String, ArmeReglage>,
    /// Comment on tient l'arme au repos et en visée. Vit dans CE fichier parce
    /// que la tenue et l'arme tenue ne se règlent pas l'une sans l'autre : une
    /// pose se juge avec l'arme dans la main, pas dans l'abstrait.
    #[serde(default)]
    pub visee: crate::visee::ViseeGenome,
}

/// La dague que le corps porte déjà — où elle va, et de quoi elle est faite.
///
/// # Un rattrapage d'asset, assumé comme tel
///
/// Le nœud `SM_Dagger` est correct en taille (21 cm rendus) mais porte une
/// translation de **1,39 m** dans le repère de la main, et un matériau `Other`
/// sans aucune texture — alors que le même fichier contient déjà `armor_bc`.
/// La vraie correction est dans la chaîne Blender ; ceci la rattrape au
/// chargement, et se supprimera le jour où l'asset sera juste.
#[derive(Deserialize, Debug, Clone)]
pub struct DagueReglage {
    #[serde(default = "vrai")]
    pub visible: bool,
    /// À la ceinture elle ne gêne plus l'arme à feu : par défaut on ne la masque
    /// donc plus. Le réglage reste, pour le jour où on la remet en main.
    #[serde(default)]
    pub masquer_si_arme_en_main: bool,
    /// L'os qui la porte. Vide = on la laisse où le glTF l'a mise.
    #[serde(default = "os_dague_par_defaut")]
    pub os: String,
    /// Nom du matériau à lui donner, tel qu'écrit dans le GLB. Vide = on ne
    /// touche pas à sa matière.
    #[serde(default = "materiau_dague_par_defaut")]
    pub materiau: String,
    #[serde(default)]
    pub decalage_x: f32,
    #[serde(default)]
    pub decalage_y: f32,
    #[serde(default)]
    pub decalage_z: f32,
    #[serde(default)]
    pub rotation_x_deg: f32,
    #[serde(default)]
    pub rotation_y_deg: f32,
    #[serde(default)]
    pub rotation_z_deg: f32,
}

impl Default for DagueReglage {
    fn default() -> Self {
        Self {
            visible: true,
            masquer_si_arme_en_main: false,
            os: os_dague_par_defaut(),
            materiau: materiau_dague_par_defaut(),
            decalage_x: 0.0,
            decalage_y: 0.0,
            decalage_z: 0.0,
            rotation_x_deg: 0.0,
            rotation_y_deg: 0.0,
            rotation_z_deg: 0.0,
        }
    }
}

impl DagueReglage {
    pub fn decalage(&self) -> Vec3 {
        Vec3::new(self.decalage_x, self.decalage_y, self.decalage_z)
    }
    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_x(self.rotation_x_deg.to_radians())
            * Quat::from_rotation_y(self.rotation_y_deg.to_radians())
            * Quat::from_rotation_z(self.rotation_z_deg.to_radians())
    }
}

fn os_dague_par_defaut() -> String {
    "Hips".to_string()
}

fn materiau_dague_par_defaut() -> String {
    "Armor".to_string()
}

impl ArmeMainGenome {
    /// Réglage d'une arme, ou le réglage neutre. Un défaut neutre est
    /// **visible** : l'arme paraît à la taille brute de son GLB, ce qui se
    /// remarque immédiatement — contrairement à un repli sur des valeurs
    /// plausibles, qui ferait passer un génome absent pour un réglage voulu.
    pub fn reglage(&self, arme: WeaponType) -> ArmeReglage {
        self.armes
            .get(arme.genome_key())
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct ArmeReglage {
    /// Longueur réelle voulue de l'arme à l'écran (m). L'échelle s'en dérive.
    #[serde(default = "taille_neutre")]
    pub taille_m: f32,
    /// Où la main tient l'arme : 0 = l'arrière (la crosse), 0,5 = le milieu,
    /// 1 = l'avant. Voir `prise_par_defaut` pour pourquoi ce n'est pas 0,5.
    #[serde(default = "prise_par_defaut")]
    pub prise: f32,
    /// De combien la vue se resserre au clic droit. 1 = aucun zoom. C'est un
    /// **diviseur du champ de vision** : 2 montre deux fois moins large.
    #[serde(default = "zoom_par_defaut")]
    pub zoom: f32,
    #[serde(default)]
    pub decalage_x: f32,
    #[serde(default)]
    pub decalage_y: f32,
    #[serde(default)]
    pub decalage_z: f32,
    #[serde(default)]
    pub rotation_x_deg: f32,
    #[serde(default)]
    pub rotation_y_deg: f32,
    #[serde(default)]
    pub rotation_z_deg: f32,
}

impl Default for ArmeReglage {
    fn default() -> Self {
        Self {
            taille_m: taille_neutre(),
            prise: prise_par_defaut(),
            zoom: zoom_par_defaut(),
            decalage_x: 0.0,
            decalage_y: 0.0,
            decalage_z: 0.0,
            rotation_x_deg: 0.0,
            rotation_y_deg: 0.0,
            rotation_z_deg: 0.0,
        }
    }
}

impl ArmeReglage {
    pub fn decalage(&self) -> Vec3 {
        Vec3::new(self.decalage_x, self.decalage_y, self.decalage_z)
    }

    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_x(self.rotation_x_deg.to_radians())
            * Quat::from_rotation_y(self.rotation_y_deg.to_radians())
            * Quat::from_rotation_z(self.rotation_z_deg.to_radians())
    }
}

fn vrai() -> bool {
    true
}

/// 0 = « non déclaré », et non « minuscule » : le calibrage saute alors
/// l'échelle au lieu de réduire l'arme à un point invisible.
fn taille_neutre() -> f32 {
    0.0
}

/// Un tiers depuis l'arrière — là où se trouve la poignée d'une arme à feu.
///
/// # Pourquoi ce n'est pas 0,5, et pourquoi ça ne pouvait pas se deviner
///
/// Les quatre GLB d'armes sont **centrés sur leur origine** (mesuré : centre à
/// 0,001 m près, emprise 1,911 m pour les quatre). Accrocher l'origine au socket
/// met donc le MILIEU de l'arme dans le poing : la moitié du canon dépasse
/// derrière la main, et l'arme paraît flotter autour d'elle au lieu d'être
/// tenue. C'est exactement le retour de jeu du 2026-08-15 — « les armes ne sont
/// pas directement dans sa main ».
///
/// Aucun de ces GLB ne porte de nœud de poignée (1 nœud, 1 maillage, 0 os) : la
/// prise ne peut donc pas se lire dans le fichier. Elle se déclare, arme par
/// arme, et se règle à chaud.
fn prise_par_defaut() -> f32 {
    0.30
}

/// Aucun zoom. C'est le défaut sûr : une arme dont on a oublié de déclarer le
/// zoom garde la vue normale, elle ne l'écrase pas silencieusement.
fn zoom_par_defaut() -> f32 {
    1.0
}

#[derive(Resource)]
pub struct ArmeMainGenomeHandle(pub Handle<Genome<ArmeMainGenome>>);

// ---------------------------------------------------------------------------
// Marqueurs et état
// ---------------------------------------------------------------------------

/// L'entité du socket, une fois trouvée. Porté par l'entité elle-même : elle
/// meurt avec l'avatar quand celui-ci est reconstruit, et son absence relance
/// la recherche sans qu'aucun code n'ait à savoir qu'une reconstruction a eu
/// lieu.
#[derive(Component)]
struct SocketMain;

/// La dague du corps, une fois trouvée.
#[derive(Component)]
struct DagueDuCorps;

/// Dague déjà replacée et rhabillée : le rattrapage ne se rejoue pas.
#[derive(Component)]
struct DaguePosee;

/// L'arme réellement accrochée.
#[derive(Component)]
struct ArmeEnMain {
    arme: WeaponType,
}

/// En attente de mesure : la scène glTF n'a pas encore produit ses AABB.
///
/// Porte les deux valeurs du génome dont le calibrage a besoin, plutôt que de
/// relire celui-ci : entre l'accrochage et la mesure, le fichier peut avoir été
/// rechargé à chaud. On mesure l'arme telle qu'elle a été posée.
/// Tout ce dont la pose de l'arme dépend, mesuré une fois au calibrage.
///
/// 🚨 Les TROIS ensemble, et c'est le sujet.
///
/// La pose se compose de deux grandeurs qui dérivent de la MÊME correction de
/// socket : la rotation (où pointe le canon) et l'avance de prise (de combien
/// l'arme glisse dans le poing pour être tenue par la poignée et non par le
/// milieu). Le 2026-08-18 j'ai rendu la rotation dynamique en laissant l'avance
/// figée sur la correction du calibrage : les deux ont cessé de parler du même
/// repère, et l'arme est partie sur le côté d'exactement l'angle que la visée
/// venait d'introduire — « elle est sur le côté quand on vise ».
///
/// Deux grandeurs tirées d'une seule source se recalculent ENSEMBLE, ou pas du
/// tout. C'est la même faute que la longueur d'os corrigée sans la dérive de
/// racine, et que la fraîcheur vérifiée sur les seuls capteurs verts.
#[derive(Component, Clone, Copy)]
struct PoseDeRepos {
    /// « Quel axe du MODÈLE est le canon » — propriété du fichier, immuable.
    genome: Quat,
    /// La translation locale déclarée au génome, avant l'avance de prise.
    base: Vec3,
    /// De combien avancer l'arme dans son propre axe pour la tenir par la
    /// poignée. Une DISTANCE : elle ne porte aucune direction, c'est la
    /// correction courante qui la lui donne.
    recul: f32,
}

#[derive(Component)]
struct ACalibrer {
    taille_m: f32,
    prise: f32,
}

/// La bouche du canon, dans le repère LOCAL de l'arme. Dérivée de l'AABB
/// mesurée, jamais déclarée — cf. l'en-tête du génome.
#[derive(Component)]
struct BoucheLocale(Vec3);

/// Ce que le capteur donne à lire. Sans lui, « je ne vois pas mon arme » a
/// quatre causes qu'aucune capture d'écran ne distingue : pas de socket (mauvais
/// corps), GLB non chargé, échelle aberrante, ou génome absent.
#[derive(Resource, Default)]
pub struct EtatArmeMain {
    pub socket_trouve: bool,
    /// Ce à quoi l'arme est réellement accrochée : le socket, l'os, ou rien.
    /// Publié : « pas d'arme » et « arme accrochée à l'os de repli » sont deux
    /// états très différents qu'aucune capture d'écran ne distingue.
    pub ancrage: &'static str,
    pub arme_posee: Option<WeaponType>,
    pub taille_mesuree_m: f32,
    pub taille_cible_m: f32,
    pub echelle: f32,
    pub calibree: bool,
    pub dague_masquee: bool,
    /// L'os sur lequel la dague a été reposée, et si sa matière a été rendue.
    /// Publiés : « la dague est invisible » et « la dague est blanche à un
    /// mètre du corps » ne se corrigent pas au même endroit.
    pub dague_os: String,
    pub dague_materiau: bool,
    /// Secondes passées dans le mode — sépare « pas encore chargé » (normal les
    /// deux premières secondes) de « ne chargera jamais » (un défaut).
    pub depuis_entree_s: f32,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ExpeditionArmeMainPlugin;

impl Plugin for ExpeditionArmeMainPlugin {
    fn build(&self, app: &mut App) {
        // `init_asset` est idempotent, mais le loader ne l'est pas : deux
        // enregistrements pour la même extension se disputeraient les `.toml`.
        // Ce génome a un type à lui, donc aucun conflit avec les autres.
        app.init_asset::<Genome<ArmeMainGenome>>()
            .register_asset_loader(GenomeLoader::<ArmeMainGenome>::default())
            .init_resource::<EtatArmeMain>()
            .init_resource::<MuzzleWorld>()
            .add_systems(Startup, charger_genome)
            .add_systems(OnExit(GameMode::Expedition), retirer_arme)
            // Hors gate de mode, DÉLIBÉRÉMENT : les messages d'asset sont
            // abandonnés au bout de deux frames. Un système gaté sur le mode
            // manquerait tout rechargement survenu pendant qu'on ne joue pas —
            // et « le fichier ne fait rien » est le pire retour possible pour un
            // réglage censé se voir à chaud. `.before` garde l'ordre utile.
            .add_systems(Update, reagir_au_genome.before(accrocher_arme))
            .add_systems(
                Update,
                (
                    trouver_socket_et_dague,
                    accrocher_arme,
                    calibrer_arme,
                    // APRÈS la recherche : la dague doit être marquée pour être
                    // reposée. AVANT le masquage : on masque ce qui est en place.
                    poser_la_dague,
                    masquer_dague,
                )
                    .chain()
                    .run_if(in_state(GameMode::Expedition))
                    .run_if(in_state(AppMode::InGame)),
            )
            // APRÈS la propagation : la bouche se lit sur la position RÉELLE de
            // l'arme, os animés compris. La lire en `Update` donnerait celle de
            // la frame précédente — soit, à 9,75 m/s en sprint, 16 cm d'écart.
            .add_systems(
                PostUpdate,
                suivre_la_visee_avec_l_arme
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameMode::Expedition)),
            )
            .add_systems(
                PostUpdate,
                publier_bouche
                    .after(TransformSystems::Propagate)
                    .run_if(in_state(GameMode::Expedition)),
            )
            .add_systems(Update, capteur_arme_main.in_set(GameSet::Sensors));
    }
}

fn charger_genome(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(ArmeMainGenomeHandle(asset_server.load(GENOME)));
    info!("[arme-main] génome en chargement : {GENOME}");
}

// ---------------------------------------------------------------------------
// Systèmes
// ---------------------------------------------------------------------------

/// Repère le socket et la dague dans la scène du corps.
///
/// Ne balaye les noms que tant qu'il manque quelque chose : dès que les deux
/// sont marqués, ce système coûte deux tests de requête vide (`scalability.md`).
fn trouver_socket_et_dague(
    mut commands: Commands,
    q_socket: Query<(), With<SocketMain>>,
    q_dague: Query<(), With<DagueDuCorps>>,
    q_noms: Query<(Entity, &Name), (Without<SocketMain>, Without<DagueDuCorps>)>,
    mut etat: ResMut<EtatArmeMain>,
) {
    let manque_socket = q_socket.is_empty();
    let manque_dague = q_dague.is_empty();
    // Se relit à chaque passe au lieu de se verrouiller à `true` : l'avatar est
    // reconstruit dès que l'équipement change, et un capteur qui affirmerait
    // « socket trouvé » devant une entité morte enverrait chercher le défaut
    // partout sauf là où il est.
    if etat.socket_trouve == manque_socket {
        etat.socket_trouve = !manque_socket;
    }
    if !manque_socket && !manque_dague {
        return;
    }
    // On collecte les deux candidats avant de trancher : le socket est plus
    // précis, l'os est toujours là. Choisir dans la boucle donnerait l'os quand
    // il passe en premier, alors même qu'un socket existe.
    let mut socket = None;
    let mut os_main = None;
    for (entite, nom) in &q_noms {
        match nom.as_str() {
            SOCKET_MAIN => socket = Some(entite),
            OS_MAIN_DROITE => os_main = Some(entite),
            NOEUD_DAGUE if manque_dague => {
                commands.entity(entite).insert(DagueDuCorps);
            }
            _ => {}
        }
    }
    if manque_socket {
        if let Some((entite, quoi)) = socket
            .map(|e| (e, SOCKET_MAIN))
            .or(os_main.map(|e| (e, OS_MAIN_DROITE)))
        {
            commands.entity(entite).insert(SocketMain);
            etat.ancrage = quoi;
            info!("[arme-main] ancrage « {quoi} » trouvé");
        } else if !etat.ancrage.is_empty() {
            // Le corps a disparu : laisser le nom du dernier ancrage ferait
            // lire « accroché à RightHand » sur un capteur qui dit par ailleurs
            // qu'il n'y a pas d'ancrage.
            etat.ancrage = "";
        }
    }
}

/// Redéclenche l'accrochage quand le génome arrive ou change.
///
/// # Le défaut que ce système corrige — trouvé en relecture, pas en jeu
///
/// Le génome est un asset : il arrive **après** le premier accrochage possible.
/// Sans ce système, une arme accrochée pendant ce délai gardait la taille et
/// l'orientation brutes de son GLB **pour toujours** — et le rechargement à
/// chaud, seule raison d'en faire un asset, n'aurait rien rechargé du tout. Le
/// symptôme aurait été « l'arme est énorme et de travers », qui n'évoque pas une
/// course entre deux chargements.
///
/// On retire l'arme plutôt que de la corriger sur place : `accrocher_arme` la
/// repose à la frame suivante avec les valeurs fraîches, et il n'existe donc
/// qu'un seul chemin qui pose une arme.
fn reagir_au_genome(
    mut commands: Commands,
    mut evs: MessageReader<AssetEvent<Genome<ArmeMainGenome>>>,
    q_arme: Query<Entity, With<ArmeEnMain>>,
    mut etat: ResMut<EtatArmeMain>,
) {
    let pertinent = evs.read().any(|e| {
        matches!(
            e,
            AssetEvent::Added { .. }
                | AssetEvent::Modified { .. }
                | AssetEvent::LoadedWithDependencies { .. }
        )
    });
    if !pertinent {
        return;
    }
    let mut n = 0;
    for e in &q_arme {
        commands.entity(e).despawn();
        n += 1;
    }
    etat.arme_posee = None;
    etat.calibree = false;
    info!("[arme-main] génome (re)chargé — {n} arme(s) à reposer");
}

/// Accroche l'arme équipée sous le socket, et la remplace quand elle change.
fn accrocher_arme(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ArmeMainGenomeHandle>>,
    genomes: Res<Assets<Genome<ArmeMainGenome>>>,
    q_socket: Query<Entity, With<SocketMain>>,
    q_arme: Query<(Entity, &ArmeEnMain)>,
    mut etat: ResMut<EtatArmeMain>,
) {
    let Ok(socket) = q_socket.single() else {
        // Pas de socket : soit le corps n'est pas encore là, soit ce n'est pas
        // le bon corps. Le capteur tranche — pas ce système.
        return;
    };
    let voulue = equipped.current;
    let deja = q_arme.iter().next();
    if let Some((_, en_main)) = deja {
        if en_main.arme == voulue {
            return;
        }
    }
    // Changement d'arme (ou avatar reconstruit) : on retire avant de poser.
    for (e, _) in &q_arme {
        commands.entity(e).despawn();
    }

    let genome = genome_handle
        .as_deref()
        .and_then(|h| genomes.get(&h.0))
        .map(|g| &g.data);
    let reglage = genome.map(|g| g.reglage(voulue)).unwrap_or_default();

    commands.entity(socket).with_children(|parent| {
        parent.spawn((
            ArmeEnMain { arme: voulue },
            SceneRoot(asset_server.load(chemin_glb(voulue))),
            Transform {
                translation: reglage.decalage(),
                rotation: reglage.rotation(),
                scale: Vec3::ONE,
            },
            // Cachée jusqu'à la mesure : sans ça, l'arme paraît une à deux
            // frames à la taille brute de son GLB. Sur un modèle de 7,7 Mo
            // exporté sans échelle, ce clignotement est énorme.
            Visibility::Hidden,
            ACalibrer {
                taille_m: reglage.taille_m,
                prise: reglage.prise,
            },
            Name::new(format!("ArmeEnMain_{}", voulue.genome_key())),
        ));
    });
    etat.arme_posee = Some(voulue);
    etat.taille_cible_m = reglage.taille_m;
    etat.calibree = false;
    info!(
        "[arme-main] {} accrochée à la main droite (taille visée {:.2} m)",
        voulue.genome_key(),
        reglage.taille_m
    );
}

/// Mesure l'arme chargée, en dérive son échelle et la bouche de son canon.
///
/// # Pourquoi la bouche se dérive ici plutôt que de se déclarer
///
/// Déclarer une « longueur de canon » par arme, ce serait la même grandeur
/// écrite deux fois — une fois dans le GLB, une fois dans un TOML — et les deux
/// finiraient par diverger au premier ré-export. On prend le point du modèle le
/// plus avancé **sur son axe de tir**, l'axe de tir étant −Z une fois la
/// rotation du génome appliquée (c'est ce à quoi cette rotation sert).
///
/// Conséquence utile : une rotation fausse se voit **deux fois** — l'arme est de
/// travers, ET le traceur sort d'ailleurs que du canon. Un seul critère à l'œil
/// valide les deux.
fn calibrer_arme(
    mut commands: Commands,
    q_a_calibrer: Query<(Entity, &ACalibrer, &Transform)>,
    q_children: Query<&Children>,
    q_aabb: Query<&Aabb>,
    q_socket: Query<&GlobalTransform, With<SocketMain>>,
    q_player: Query<&GlobalTransform, With<forgia_player::Player>>,
    mut etat: ResMut<EtatArmeMain>,
) {
    // L'orientation du socket et le regard du personnage : deux mesures, prises
    // au moment où l'arme est posée. Absentes, on retente — poser une arme sans
    // savoir où regarde le personnage, c'est reproduire l'hypothèse qu'on vient
    // de corriger.
    let (Ok(socket_gt), Ok(joueur_gt)) = (q_socket.single(), q_player.single()) else {
        return;
    };
    let avant_monde = joueur_gt.forward().as_vec3();
    let correction = correction_de_socket(socket_gt.rotation(), avant_monde);

    for (entite, cible, tf) in &q_a_calibrer {
        let Some((min, max)) = aabb_des_descendants(entite, &q_children, &q_aabb) else {
            continue; // scène encore en chargement — on retentera
        };
        let taille = max - min;
        let arete = taille.x.max(taille.y).max(taille.z);
        if arete < 0.001 {
            continue;
        }
        if arete > AABB_ABERRANTE_M {
            warn!(
                "[arme-main] AABB aberrante ({arete:.0} m) — arme laissée à l'échelle brute, \
                 signe d'une quantification i16 mal décodée dans le GLB"
            );
            commands
                .entity(entite)
                .remove::<ACalibrer>()
                .insert(Visibility::Inherited);
            continue;
        }
        // `taille_m = 0` = non déclarée : on montre l'arme telle quelle plutôt
        // que de la réduire à un point.
        let echelle = if cible.taille_m > 0.001 {
            cible.taille_m / arete
        } else {
            1.0
        };
        // La rotation FINALE = correction mesurée du socket × orientation propre
        // du GLB (génome). L'ordre compte : le génome amène le canon du modèle
        // sur −Z, la correction amène ce −Z sur le regard réel du personnage.
        let rot_finale = correction * tf.rotation;

        // 🚨 Mais la bouche et la prise se lisent sur la rotation du GÉNOME, pas
        // sur la finale. Le génome répond à « quel axe du MODÈLE est le canon »
        // — la correction ne change pas cette réponse, elle change seulement où
        // ce canon pointe une fois posé. Les calculer sur la finale ferait
        // ressortir la balle par la crosse, exactement du même angle que celui
        // qu'on vient de corriger.
        let bouche = bouche_locale(min, max, tf.rotation);
        // Ce que la main tient : la POIGNÉE, pas le milieu. Sans ce décalage,
        // l'origine du modèle — son centre, mesuré — se retrouve dans le poing.
        let recul = recul_de_prise(min, max, tf.rotation, cible.prise) * echelle;
        // Avancer l'arme se fait dans l'avant RÉEL du socket, pas dans son −Z.
        // C'est la même correction, appliquée à la translation : sans elle,
        // l'arme reculait DANS le personnage pendant qu'elle pointait derrière.
        let avance = (correction * Vec3::NEG_Z) * recul;

        commands
            .entity(entite)
            .remove::<ACalibrer>()
            .insert(PoseDeRepos {
                genome: tf.rotation,
                base: tf.translation,
                recul,
            })
            .insert(Transform {
                scale: Vec3::splat(echelle),
                rotation: rot_finale,
                translation: tf.translation + avance,
            })
            .insert(BoucheLocale(bouche))
            .insert(Visibility::Inherited);

        etat.taille_mesuree_m = arete;
        etat.echelle = echelle;
        etat.calibree = true;
        info!(
            "[arme-main] mesurée {arete:.3} m → échelle {echelle:.4} ; bouche locale \
             ({:.3}, {:.3}, {:.3})",
            bouche.x, bouche.y, bouche.z
        );
    }
}

/// Replace la dague et lui rend une matière — une seule fois par avatar.
///
/// # Ce que ce système rattrape, et pourquoi il ne devrait pas exister
///
/// Deux défauts de l'asset, mesurés : une translation de 1,39 m qui l'éloigne de
/// la main, et un matériau `Other` sans texture. Leur vraie correction est dans
/// la chaîne Blender qui produit le corps. Ceci la rattrape au chargement, et se
/// supprimera le jour où l'asset sera juste — la marque `DaguePosee` rend
/// l'opération idempotente, donc inoffensive si l'asset se corrige.
///
/// Le matériau se retrouve **par son nom**, via l'asset `Gltf` du corps — dont
/// le chemin est LU sur la scène déjà chargée, jamais redéclaré. Redéclarer le
/// chemin du corps est exactement ce qui vient de coûter cette session : il a
/// changé sous nos pieds.
#[allow(clippy::too_many_arguments)]
fn poser_la_dague(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    genome_handle: Option<Res<ArmeMainGenomeHandle>>,
    genomes: Res<Assets<Genome<ArmeMainGenome>>>,
    q_dague: Query<(Entity, &Transform), (With<DagueDuCorps>, Without<DaguePosee>)>,
    q_noms: Query<(Entity, &Name)>,
    q_scene: Query<&SceneRoot>,
    q_parent: Query<&ChildOf>,
    q_children: Query<&Children>,
    q_mat: Query<(), With<MeshMaterial3d<StandardMaterial>>>,
    mut etat: ResMut<EtatArmeMain>,
) {
    let Some(cfg) = genome_handle
        .as_deref()
        .and_then(|h| genomes.get(&h.0))
        .map(|g| &g.data.dague)
    else {
        return;
    };
    let Ok((dague, tf)) = q_dague.single() else {
        return;
    };

    // ── Sa place ────────────────────────────────────────────────────────────
    // L'échelle du nœud est CONSERVÉE : elle compense la taille brute du
    // maillage (2139 unités pour 21 cm rendus). La réécrire ferait disparaître
    // la dague ou l'agrandirait d'un facteur 10 000.
    if !cfg.os.is_empty() {
        let Some(cible) = q_noms
            .iter()
            .find(|(_, n)| n.as_str() == cfg.os)
            .map(|(e, _)| e)
        else {
            return; // l'os n'est pas encore là — on retentera
        };
        commands.entity(dague).insert((
            ChildOf(cible),
            Transform {
                translation: cfg.decalage(),
                rotation: cfg.rotation(),
                scale: tf.scale,
            },
        ));
        etat.dague_os = cfg.os.clone();
    }

    // ── Sa matière ──────────────────────────────────────────────────────────
    if !cfg.materiau.is_empty() {
        // Remonter jusqu'à la scène qui a chargé ce corps, et lire SON chemin :
        // c'est ce qui rend le rattrapage indépendant du corps employé.
        let mut cur = dague;
        let mut scene = None;
        for _ in 0..16 {
            if let Ok(s) = q_scene.get(cur) {
                scene = Some(s.0.clone());
                break;
            }
            match q_parent.get(cur) {
                Ok(p) => cur = p.parent(),
                Err(_) => break,
            }
        }
        if let Some(chemin) = scene
            .and_then(|h| asset_server.get_path(h.id()))
            // `without_label` emprunte : on possède d'abord, on retire ensuite.
            .map(|p| p.without_label().into_owned())
        {
            let gltf: Handle<Gltf> = asset_server.load(chemin);
            if let Some(mat) = gltfs
                .get(&gltf)
                .and_then(|g| g.named_materials.get(cfg.materiau.as_str()))
            {
                // Le maillage est porté soit par le nœud lui-même, soit par un
                // enfant (une entité par primitive). On sert les deux.
                let mut poses = 0;
                for e in std::iter::once(dague).chain(q_children.iter_descendants(dague)) {
                    if q_mat.get(e).is_ok() {
                        commands.entity(e).insert(MeshMaterial3d(mat.clone()));
                        poses += 1;
                    }
                }
                etat.dague_materiau = poses > 0;
                if poses > 0 {
                    info!(
                        "[arme-main] dague reposée sur « {} », matière « {} » ({poses} maillage(s))",
                        cfg.os, cfg.materiau
                    );
                }
            } else {
                // L'asset Gltf n'est pas encore résolu : on retentera. Marquer
                // la dague maintenant la laisserait sans matière pour toujours.
                return;
            }
        }
    }
    commands.entity(dague).insert(DaguePosee);
}

/// Masque la dague du corps tant qu'une arme est tenue, et la rend au retrait.
fn masquer_dague(
    genome_handle: Option<Res<ArmeMainGenomeHandle>>,
    genomes: Res<Assets<Genome<ArmeMainGenome>>>,
    q_arme: Query<(), With<ArmeEnMain>>,
    mut q_dague: Query<&mut Visibility, With<DagueDuCorps>>,
    mut etat: ResMut<EtatArmeMain>,
) {
    let reglage = genome_handle
        .as_deref()
        .and_then(|h| genomes.get(&h.0))
        .map(|g| g.data.dague.clone())
        .unwrap_or_default();
    let cacher =
        !reglage.visible || (reglage.masquer_si_arme_en_main && !q_arme.is_empty());
    for mut vis in &mut q_dague {
        let voulue = if cacher {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        // Écriture conditionnelle : `Visibility` est un composant à
        // change-detection, l'écrire chaque frame invaliderait sa propagation
        // pour rien.
        if *vis != voulue {
            *vis = voulue;
        }
    }
    if etat.dague_masquee != cacher {
        etat.dague_masquee = cacher;
    }
}

/// Publie la position monde de la bouche, pour le chemin de tir.
fn publier_bouche(
    q_arme: Query<(&GlobalTransform, &BoucheLocale)>,
    mut muzzle: ResMut<MuzzleWorld>,
) {
    let nouvelle = q_arme
        .iter()
        .next()
        .map(|(gt, b)| gt.transform_point(b.0));
    if muzzle.0 != nouvelle {
        muzzle.0 = nouvelle;
    }
}

/// Sortie du mode : on retire ce qu'on a posé, et RIEN d'autre.
///
/// La bouche est remise à `None` : la laisser publiée ferait sortir les balles
/// de l'arène au dernier endroit connu de l'Expédition — un défaut d'autant plus
/// coûteux qu'il paraîtrait n'avoir aucun rapport avec le mode qu'on vient de
/// quitter.
fn retirer_arme(
    mut commands: Commands,
    q_arme: Query<Entity, With<ArmeEnMain>>,
    q_socket: Query<Entity, With<SocketMain>>,
    q_dague: Query<Entity, With<DagueDuCorps>>,
    mut muzzle: ResMut<MuzzleWorld>,
    mut etat: ResMut<EtatArmeMain>,
) {
    let mut n = 0;
    for e in &q_arme {
        commands.entity(e).despawn();
        n += 1;
    }
    // Les marques partent avec : au retour dans la carte, le corps est
    // reconstruit et ces entités-là n'existeront plus. Les retirer explicitement
    // évite qu'une entité survivante passe pour « déjà équipée ».
    for e in &q_socket {
        commands.entity(e).remove::<SocketMain>();
    }
    for e in &q_dague {
        commands.entity(e).remove::<DagueDuCorps>();
    }
    muzzle.0 = None;
    *etat = EtatArmeMain::default();
    info!("[arme-main] {n} arme(s) retirée(s) de la main");
}

/// Replis si le génome n'est pas chargé — mêmes valeurs que ses `serde(default)`.
/// Ils ne servent qu'à la fenêtre de démarrage : dès que le TOML arrive, ce sont
/// les siennes qui décident.
const SEUIL_ALERTE_DEFAUT_DEG: f32 = 90.0;
const SEUIL_AVERTIT_DEFAUT_DEG: f32 = 35.0;

/// Réoriente l'arme vers ce que le joueur VISE, chaque frame.
///
/// # Le défaut que ça corrige
///
/// La correction de socket était calculée **une fois**, au calibrage, contre
/// `Player.forward()` — la capsule. Deux conséquences rapportées en jeu le
/// 2026-08-18 : « quand on vise, l'arme ne pointe pas droit devant mais à
/// droite ».
///
/// 1. **Mauvaise référence.** Le tir part de la caméra active (`AimCamera`) ;
///    l'arme s'alignait sur la capsule. À la troisième personne, le corps
///    tourne vers son déplacement quand la caméra tourne vers la visée : les
///    deux ne coïncident qu'à l'arrêt sans viser. Quand elles divergent, le
///    réticule ment — on tire où on regarde, l'arme montre ailleurs.
/// 2. **Calculée une fois.** L'orientation de la main change avec la pose. Un
///    décalage réglé à l'œil sur la pose de repos est faux sur la pose de
///    visée, et aucune valeur de génome ne peut être juste dans les deux.
///
/// D'où : on recalcule à chaque frame, et on interpole entre les deux
/// références selon l'avancement de la visée. Au repos l'arme suit le corps —
/// c'est ce qui se lit naturellement de dos ; en visée elle suit exactement le
/// rayon de tir.
///
/// Tourne avant la propagation : la rotation écrite doit atteindre le rendu de
/// cette frame. Les `GlobalTransform` lus ont donc une frame de retard, ce qui
/// est sans effet visible sur une arme tenue à bout de bras.
fn suivre_la_visee_avec_l_arme(
    visee: Res<crate::visee::Visee>,
    q_socket: Query<&GlobalTransform, With<SocketMain>>,
    q_player: Query<&GlobalTransform, With<forgia_player::Player>>,
    q_cam: Query<(&GlobalTransform, &Camera), With<forgia_player::AimCamera>>,
    mut q_arme: Query<(&mut Transform, &PoseDeRepos)>,
) {
    let (Ok(socket_gt), Ok(joueur_gt)) = (q_socket.single(), q_player.single()) else {
        return;
    };
    let rot_socket = socket_gt.rotation();
    let vers_le_corps = correction_de_socket(rot_socket, joueur_gt.forward().as_vec3());
    // Sans caméra active, on garde le corps pour référence : mieux vaut une
    // arme qui suit le personnage qu'une arme qui pointe n'importe où.
    let vers_la_visee = q_cam
        .iter()
        .find(|(_, c)| c.is_active)
        .map(|(gt, _)| correction_de_socket(rot_socket, gt.forward().as_vec3()))
        .unwrap_or(vers_le_corps);

    let f = visee.facteur.clamp(0.0, 1.0);
    // `slerp` et non `lerp` : entre deux orientations, le chemin court sur la
    // sphère est le seul qui ne passe pas par une pose écrasée.
    let correction = vers_le_corps.slerp(vers_la_visee, f);
    for (mut tf, repos) in &mut q_arme {
        tf.rotation = correction * repos.genome;
        // 🚨 La MÊME correction porte l'avance de prise. La laisser sur celle du
        // calibrage décalait l'arme latéralement de l'angle de visée : elle
        // pointait juste et se tenait à côté de la main.
        tf.translation = repos.base + (correction * Vec3::NEG_Z) * repos.recul;
    }
}

/// `forgia2_expedition_arme.json`, 1 Hz.
fn capteur_arme_main(
    time: Res<Time>,
    mut accum: Local<f32>,
    // Deux mémos, un par extrémité de la visée. Un seul ne pourrait pas
    // distinguer « l'arme suit le corps, c'est normal » de « elle ne suit pas
    // la caméra alors qu'on vise ».
    mut ecart_en_visee: Local<f32>,
    mut ecart_au_repos: Local<f32>,
    mode: Res<State<GameMode>>,
    visee: Res<crate::visee::Visee>,
    genome_handle: Option<Res<ArmeMainGenomeHandle>>,
    genomes: Res<Assets<Genome<ArmeMainGenome>>>,
    muzzle: Res<MuzzleWorld>,
    q_arme: Query<(&ArmeEnMain, &GlobalTransform, Option<&BoucheLocale>)>,
    q_cam: Query<(&GlobalTransform, &Camera), With<forgia_player::AimCamera>>,
    mut etat: ResMut<EtatArmeMain>,
) {
    let en_expedition = *mode.get() == GameMode::Expedition;
    let facteur = visee.facteur;
    if en_expedition {
        etat.depuis_entree_s += time.delta_secs();
    }
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let genome_charge = genome_handle
        .as_deref()
        .map(|h| genomes.get(&h.0).is_some())
        .unwrap_or(false);
    let cfg = genome_handle.as_deref().and_then(|h| genomes.get(&h.0));
    let armes_declarees = cfg.map(|g| g.data.armes.len()).unwrap_or(0);
    // Les seuils d'alerte viennent de la couche definition, rechargée à chaud :
    // un seuil qu'il faut recompiler pour corriger reste faux.
    let seuil_alerte = cfg
        .map(|g| g.data.visee.ecart_alerte_deg)
        .unwrap_or(SEUIL_ALERTE_DEFAUT_DEG);
    let seuil_avertit = cfg
        .map(|g| g.data.visee.ecart_avertit_deg)
        .unwrap_or(SEUIL_AVERTIT_DEFAUT_DEG);
    // Le repli : l'arme pend à l'os `RightHand` faute de socket posé exprès.
    let ancrage_est_repli = etat.ancrage == OS_MAIN_DROITE;
    let tenue = q_arme.iter().next();
    let arme = tenue.map(|(a, _, _)| a.arme.genome_key()).unwrap_or("");
    let bouche = muzzle.0.unwrap_or(Vec3::ZERO);

    // 🚨 LE nombre qui aurait évité deux allers-retours : l'écart, en degrés,
    // entre la direction du CANON et le regard. 0° = l'arme pointe où on
    // regarde ; 180° = elle pointe derrière, ce qui a été rapporté en jeu le
    // 2026-08-16 et n'était mesuré nulle part.
    //
    // On le calcule de bout en bout — de l'origine de l'arme à sa bouche, dans
    // le monde — donc il vérifie TOUTE la chaîne d'un coup : rotation du
    // génome, correction du socket, et point de bouche. Un maillon faux se voit
    // ici, même si les trois se compensaient deux à deux.
    //
    // 🚨 LE RÉFÉRENTIEL EST LA CAMÉRA QUI REND, PAS LA CAPSULE.
    //
    // Il comparait à `Player.forward()`. C'est un référentiel DIFFÉRENT de
    // celui du tir : `fire_weapon_minimal` part de la caméra active marquée
    // `AimCamera`, parce qu'à la troisième personne le corps tourne vers son
    // déplacement quand la caméra, elle, tourne vers la visée. Les deux ne
    // coïncident qu'à l'arrêt.
    //
    // Conséquence mesurée le 2026-08-17 : le capteur restait épinglé entre 145
    // et 148° — donc en `critical` permanent — joueur immobile. Un canal saturé
    // ne dit plus rien : il ne peut ni confirmer un défaut, ni en signaler un
    // nouveau. Le capteur qui devait nommer « l'arme est à l'envers » était
    // devenu incapable de distinguer les deux cas.
    //
    // La règle générale : un capteur qui juge une chaîne doit se mesurer contre
    // la MÊME référence que le code qu'il juge. Sinon il mesure l'écart entre
    // deux conventions, pas un défaut.
    let regard = q_cam
        .iter()
        .find(|(_, c)| c.is_active)
        .map(|(gt, _)| gt.forward().as_vec3());
    let ecart_deg = match (tenue, regard) {
        (Some((_, gt, Some(b))), Some(regard)) => {
            let canon = gt.transform_point(b.0) - gt.translation();
            canon
                .try_normalize()
                .map(|c| c.angle_between(regard).to_degrees())
                .unwrap_or(-1.0)
        }
        _ => -1.0,
    };
    // `-1.0` n'est PAS une mesure : c'est l'aveu qu'on n'a pas pu mesurer. Le
    // publier à côté d'un `severity: "ok"` faisait lire « écart nul » là où il
    // fallait lire « aucune caméra active, ou aucune bouche ».
    let ecart_mesure = ecart_deg >= 0.0;

    // Deux secondes : le corps est un glTF de 81 nœuds chargé en tâche de fond,
    // il n'est jamais là à la première frame. En deçà, une alerte serait du
    // bruit à chaque entrée dans la carte.
    const DELAI_DE_GRACE_S: f32 = 2.0;
    let installe = etat.depuis_entree_s > DELAI_DE_GRACE_S;

    // 🚨 RETENIR NE SUFFIT PAS : IL FAUT DIRE DANS QUELLE CONDITION.
    //
    // Première version, 2026-08-18 : on retenait le dernier écart mesurable,
    // quel qu'il soit. Le premier relevé a rendu 30,9° — un nombre qui ne
    // conclut RIEN, parce qu'au repos l'arme suit le CORPS et qu'un écart de
    // 30° avec la caméra d'épaule est alors parfaitement normal, tandis qu'en
    // pleine visée le même 30° est un défaut franc.
    //
    // Une mesure sans sa condition n'est pas une mesure, c'est un nombre. On en
    // retient donc DEUX, prises aux deux extrémités de la visée : leur écart
    // est ce qui répond à « est-ce que l'arme pointe où je vise ».
    const VISEE_PLEINE: f32 = 0.9;
    const VISEE_NULLE: f32 = 0.1;
    if en_expedition && ecart_mesure {
        if facteur >= VISEE_PLEINE {
            *ecart_en_visee = ecart_deg;
        } else if facteur <= VISEE_NULLE {
            *ecart_au_repos = ecart_deg;
        }
    }
    let ecart_publie = if en_expedition && ecart_mesure {
        ecart_deg
    } else {
        *ecart_en_visee
    };

    let (severity, next_step) = if !en_expedition {
        ("info", "hors Expedition — ecart_visee_deg retenu de la derniere visee dans le mode")
    } else if !genome_charge && installe {
        (
            "warn",
            "GENOME_ABSENT : genomes/expedition_arme_main.toml non chargé — l'arme prend taille et orientation brutes du GLB",
        )
    // 🚨 ORDRE DES CAUSES — de la plus amont à la plus aval. Il n'est pas
    // cosmétique : « ARME_NON_DECLAREE » était testé AVANT l'ancrage, or sa
    // condition (`taille_cible_m == 0`) est vraie quand aucune arme n'a pu être
    // posée. Le 2026-08-16, un corps sans socket a donc fait dire au capteur
    // « ajoute une entrée au génome » — un fichier parfaitement correct — au
    // lieu de « il n'y a pas d'ancrage ». Un capteur qui envoie au mauvais
    // endroit coûte plus cher qu'un capteur muet : on le croit.
    } else if !etat.socket_trouve && installe {
        (
            "critical",
            "ANCRAGE_ABSENT : ni le socket « socket_main_droite » ni l'os « RightHand » dans la scene — l'avatar n'est pas monte, ou le corps charge n'a pas de squelette Mixamo. Verifier expedition_body.model au genome d'equipement",
        )
    // 🚨 Le repli DOIT se voir. Il ne se voyait pas.
    //
    // `ANCRAGE_ABSENT` ci-dessus était devenu inatteignable : quand le socket
    // manque, l'accrochage retombe sur l'os `RightHand` et pose `SocketMain`,
    // donc `socket_trouve` vaut `true`. Le capteur publiait bien
    // `ancrage: "RightHand"` — mais cette valeur se lisait exactement comme
    // `"socket_main_droite"` du point de vue de la santé : les deux passaient
    // au vert. Le 2026-08-16, un corps sans aucun socket est ainsi resté « ok »
    // pendant que l'arme était mal tenue.
    //
    // Un repli silencieux est un mensonge par omission : il marche, donc plus
    // rien ne rappelle que le corps chargé n'a pas ce qu'il devrait avoir.
    } else if ancrage_est_repli && installe {
        (
            "warn",
            "ANCRAGE_DE_REPLI : le corps n'a pas de socket « socket_main_droite », l'arme est accrochee a l'os « RightHand ». Ca tient, mais l'orientation vient alors du repere de l'os et non d'un point pose expres — l'arme peut etre de travers. Verifier que expedition_body.model exporte bien ses EMPTY socket_*",
        )
    } else if arme.is_empty() && installe {
        (
            "warn",
            "ARME_NON_ACCROCHEE : ancrage trouve mais aucune arme posee dessous — verifier EquippedWeapons",
        )
    } else if armes_declarees > 0 && etat.taille_cible_m <= 0.0 && installe {
        (
            "warn",
            "ARME_NON_DECLAREE : l'arme est accrochee mais n'a pas d'entree dans le genome — ajouter [armes.<cle>] (cle = nom du GLB). Elle garde la taille brute du fichier",
        )
    } else if !etat.calibree && !arme.is_empty() && installe {
        (
            "warn",
            "ARME_NON_CALIBREE : arme accrochée mais aucune AABB mesurée — le GLB n'a pas fini de charger, ou il ne contient aucun maillage",
        )
    } else if etat.calibree && muzzle.0.is_none() {
        (
            "warn",
            "BOUCHE_ABSENTE : arme calibrée mais aucune bouche publiée — la lueur et le traceur sortiront de la caméra, 3,2 m derrière le personnage",
        )
    } else if facteur >= VISEE_PLEINE && ecart_deg > seuil_alerte {
        (
            "critical",
            "ARME_A_L_ENVERS : le canon pointe a plus d'un quart de tour du regard de la CAMERA ACTIVE — celle qui sert aussi d'origine au tir. 180° = l'arme part vers l'arriere. Cause probable : rotation_*_deg du genome fausse pour ce GLB, ou correction de socket calculee sur une pose degeneree. Seuil reglable : [visee].ecart_alerte_deg",
        )
    } else if facteur >= VISEE_PLEINE && ecart_deg > seuil_avertit {
        (
            "warn",
            "ARME_DE_TRAVERS : le canon s'ecarte nettement du regard — l'arme se tient mal, et la lueur de bouche sort a cote. Ajuster rotation_*_deg de cette arme au genome (rechargement a chaud). Seuil : [visee].ecart_avertit_deg",
        )
    // 🚨 En DERNIER, et en `info` : « je n'ai pas pu mesurer » n'est pas un
    // défaut de l'arme, c'est un trou dans le capteur lui-même. Le mettre plus
    // haut masquerait les vraies causes ; l'omettre ferait tomber le cas dans
    // le `ok` final, où `ecart_visee_deg: -1.0` se lit comme un écart nul.
    } else if !ecart_mesure && installe {
        (
            "info",
            "ECART_NON_MESURABLE : aucune camera active marquee AimCamera, ou l'arme n'a pas de BoucheLocale — l'ecart canon/regard vaut -1.0 et ne veut RIEN dire. Ne pas le lire comme « bien oriente »",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"expedition_arme","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"en_expedition":{en_expedition},"socket_trouve":{},"ancrage":"{}","arme":"{arme}","calibree":{},"taille_mesuree_m":{:.3},"taille_cible_m":{:.2},"echelle":{:.4},"dague_masquee":{},"bouche":[{:.2},{:.2},{:.2}],"bouche_publiee":{},"ecart_visee_deg":{:.1},"ecart_en_visee_deg":{:.1},"ecart_au_repos_deg":{:.1},"visee_facteur":{:.2},"ecart_visee_mesure":{ecart_mesure},"ancrage_est_repli":{ancrage_est_repli},"depuis_entree_s":{:.1},"genome_charge":{genome_charge},"armes_declarees":{armes_declarees}}}"#,
        time.elapsed_secs(),
        etat.socket_trouve,
        etat.ancrage,
        etat.calibree,
        etat.taille_mesuree_m,
        etat.taille_cible_m,
        etat.echelle,
        etat.dague_masquee,
        bouche.x,
        bouche.y,
        bouche.z,
        muzzle.0.is_some(),
        ecart_deg,
        ecart_publie,
        *ecart_au_repos,
        facteur,
        etat.depuis_entree_s,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia2_expedition_arme.json", json);
}

// ---------------------------------------------------------------------------
// Fonctions pures — testables sans moteur
// ---------------------------------------------------------------------------

/// Union des AABB de tous les descendants porteurs de maillage.
fn aabb_des_descendants(
    racine: Entity,
    q_children: &Query<&Children>,
    q_aabb: &Query<&Aabb>,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut trouve = false;
    let mut pile = vec![racine];
    while let Some(e) = pile.pop() {
        if let Ok(aabb) = q_aabb.get(e) {
            let c: Vec3 = aabb.center.into();
            let h: Vec3 = aabb.half_extents.into();
            min = min.min(c - h);
            max = max.max(c + h);
            trouve = true;
        }
        if let Ok(enfants) = q_children.get(e) {
            pile.extend(enfants.iter());
        }
    }
    trouve.then_some((min, max))
}

/// La rotation qui amène l'arme à pointer là où le personnage REGARDE.
///
/// # L'hypothèse fausse que cette fonction remplace
///
/// La première version tenait pour acquis que « l'avant, c'est le −Z local du
/// socket ». C'est vrai dans le repère d'une caméra — d'où viennent les
/// rotations du viewmodel — et c'est faux ici : le socket est tourné de +90°
/// autour de X, sous un os de main lui-même orienté par le rig Mixamo. Résultat
/// rapporté en jeu le 2026-08-16 : **les armes partent vers l'arrière**. Et
/// comme la bouche et la prise se dérivent du même axe, elles étaient fausses
/// aussi, du même coup et dans le même sens — donc invisibles l'une par l'autre.
///
/// On ne devine donc plus : on LIT l'orientation réelle du socket, et on en
/// déduit la correction. Le génome garde son rôle — dire l'orientation propre à
/// chaque GLB, qui diffère d'un fichier à l'autre — mais il n'a plus à encoder
/// aussi celle du squelette, qu'aucun réglage à l'œil ne pouvait deviner juste.
///
/// Après cette rotation, le −Z de l'arme pointe où regarde le personnage, et son
/// +Y vers le haut du monde. Le roulis est donc fixé lui aussi : sans ça l'arme
/// pointerait juste, mais couchée sur le flanc.
///
/// Pure : `rot_socket` est l'orientation MONDE du socket, `avant_monde` la
/// direction du regard. Testable sans moteur, et c'est tout l'intérêt.
#[must_use]
pub fn correction_de_socket(rot_socket: Quat, avant_monde: Vec3) -> Quat {
    let inv = rot_socket.inverse();
    let Some(avant) = (inv * avant_monde).try_normalize() else {
        return Quat::IDENTITY;
    };
    let haut_ref = inv * Vec3::Y;
    // Regard exactement vertical : le produit vectoriel dégénère. N'importe
    // quelle perpendiculaire fait alors l'affaire — le roulis d'une arme
    // pointée à la verticale n'a pas de bonne réponse, il ne doit juste pas
    // produire de NaN.
    let droite = avant
        .cross(haut_ref)
        .try_normalize()
        .unwrap_or_else(|| avant.any_orthonormal_vector());
    let haut = droite.cross(avant);
    Quat::from_mat3(&Mat3::from_cols(droite, haut, -avant))
}

/// De combien avancer l'arme pour que sa POIGNÉE tombe dans la main.
///
/// Le modèle est centré sur son origine ; sa poignée est à la fraction `prise`
/// de sa longueur en partant de l'arrière, donc à `L·(prise − 0,5)` du centre le
/// long de l'axe de tir. Pour amener ce point sur le socket, on décale l'arme de
/// l'opposé — soit `L·(0,5 − prise)` **vers l'avant**.
///
/// La longueur retenue est celle mesurée **le long de l'axe de tir**, pas la plus
/// grande arête : sur Boucherie, le tube est sur Z et la plus grande arête sur Z
/// aussi, mais rien ne le garantit pour une arme dont l'ornement dépasserait la
/// longueur. Mesurer sur l'axe qu'on utilise est la seule façon que le décalage
/// reste juste quand le modèle change.
///
/// Pure : c'est la fonction dont une erreur de signe accrocherait l'arme par le
/// canon.
pub fn recul_de_prise(min: Vec3, max: Vec3, rotation: Quat, prise: f32) -> f32 {
    let axe = rotation.inverse() * Vec3::NEG_Z;
    let demi = (max - min) * 0.5;
    // Demi-longueur de la boîte projetée sur l'axe (support d'un pavé).
    let demi_longueur = demi.x * axe.x.abs() + demi.y * axe.y.abs() + demi.z * axe.z.abs();
    demi_longueur * 2.0 * (0.5 - prise.clamp(0.0, 1.0))
}

/// Le point de l'AABB le plus avancé sur l'axe de tir.
///
/// L'axe de tir est −Z **après** la rotation du génome ; dans le repère du
/// modèle, c'est donc `rotation⁻¹ · (−Z)`. On projette les huit coins et on
/// garde le plus avancé : aucune hypothèse sur l'orientation native du GLB, qui
/// diffère d'une arme à l'autre (−90°, +90°, 180° selon le fichier).
///
/// Pure et sans moteur — c'est la fonction dont une erreur ferait sortir les
/// balles de la crosse.
pub fn bouche_locale(min: Vec3, max: Vec3, rotation: Quat) -> Vec3 {
    let axe = rotation.inverse() * Vec3::NEG_Z;
    let mut meilleur = min;
    let mut score = f32::MIN;
    for i in 0..8 {
        let coin = Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        );
        let p = coin.dot(axe);
        if p > score {
            score = p;
            meilleur = coin;
        }
    }
    meilleur
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_socket_vise_est_celui_du_corps_d_expedition() {
        // Le nom est écrit ici ET dans `26_cape_ardente.py` (le script Blender
        // qui pose les sockets) ET dans `avatar_vfx.rs`. Deux déclarations d'une
        // même grandeur finissent toujours par diverger : ce test est le rappel.
        assert_eq!(SOCKET_MAIN, crate::avatar_vfx::SOCKET_MAIN_DROITE);
    }

    #[test]
    fn la_bouche_sort_du_canon_pas_de_la_crosse() {
        // Une arme longue sur X, tournée de −90° autour de Y : son axe de tir
        // dans le repère du modèle devient +X (ou −X). Le point retenu doit être
        // à l'extrémité de la longueur, jamais au milieu ni du mauvais côté.
        let min = Vec3::new(-0.5, -0.05, -0.05);
        let max = Vec3::new(0.5, 0.05, 0.05);
        let rot = Quat::from_rotation_y((-90.0_f32).to_radians());
        let b = bouche_locale(min, max, rot);
        assert!(
            b.x.abs() > 0.4,
            "bouche à x={:.2} : ce n'est pas une extrémité de l'arme",
            b.x
        );
        // Et l'axe opposé doit rendre l'extrémité opposée — sinon la fonction
        // rendrait toujours le même coin, quelle que soit la rotation.
        let inverse = bouche_locale(min, max, Quat::from_rotation_y(90.0_f32.to_radians()));
        assert!(
            (b.x - inverse.x).abs() > 0.9,
            "les deux orientations rendent le même bout ({:.2} vs {:.2})",
            b.x,
            inverse.x
        );
    }

    #[test]
    fn sans_rotation_la_bouche_est_devant() {
        // Convention Bevy : l'avant est −Z. Sans rotation, la bouche doit donc
        // être au z minimal.
        let min = Vec3::new(-0.05, -0.05, -0.4);
        let max = Vec3::new(0.05, 0.05, 0.4);
        let b = bouche_locale(min, max, Quat::IDENTITY);
        assert!((b.z - (-0.4)).abs() < 1e-5, "bouche à z={:.2}", b.z);
    }

    #[test]
    fn la_prise_avance_l_arme_au_lieu_de_la_tenir_par_le_milieu() {
        // Une arme d'1 m de long sur l'axe de tir, tenue au tiers arrière : son
        // centre doit avancer de 0,20 m pour que la poignée tombe dans la main.
        // Signe compris : un recul NÉGATIF accrocherait l'arme par le canon.
        let min = Vec3::new(-0.05, -0.05, -0.5);
        let max = Vec3::new(0.05, 0.05, 0.5);
        let r = recul_de_prise(min, max, Quat::IDENTITY, 0.30);
        assert!((r - 0.20).abs() < 1e-5, "recul {r:.3} m au lieu de 0,200");
        // Tenue au milieu : aucun décalage — c'est le comportement d'avant, et
        // il doit rester atteignable.
        assert!(recul_de_prise(min, max, Quat::IDENTITY, 0.5).abs() < 1e-6);
        // Tenue par l'avant : l'arme RECULE (recul négatif).
        assert!(recul_de_prise(min, max, Quat::IDENTITY, 1.0) < -0.4);
    }

    #[test]
    fn la_longueur_de_prise_se_mesure_sur_l_axe_de_tir() {
        // Une arme longue sur X mais fine sur Z, tournée pour tirer selon X : le
        // recul doit se calculer sur la LONGUEUR (1,0), pas sur l'épaisseur
        // (0,1). Prendre la plus grande arête marcherait ici par chance ; prendre
        // l'axe Z du modèle donnerait 0,02 m et l'arme resterait dans le poing.
        let min = Vec3::new(-0.5, -0.05, -0.05);
        let max = Vec3::new(0.5, 0.05, 0.05);
        let rot = Quat::from_rotation_y((-90.0_f32).to_radians());
        let r = recul_de_prise(min, max, rot, 0.30);
        assert!((r - 0.20).abs() < 1e-5, "recul {r:.3} m — mauvais axe mesuré");
    }

    /// Le test qui manquait : un socket dont le −Z pointe DERRIÈRE le
    /// personnage. C'est le cas réel du rig — et le code d'avant, qui prenait
    /// −Z pour l'avant, envoyait donc l'arme vers l'arrière.
    #[test]
    fn une_arme_ne_part_jamais_en_arriere_quel_que_soit_l_orientation_du_socket() {
        let avant = Vec3::NEG_Z; // le personnage regarde vers −Z (avant de Bevy)
        // Quatre socket-frames tordus, dont le demi-tour qui produisait le
        // défaut, et le +90° autour de X qu'a réellement le socket du corps.
        for (nom, rot_socket) in [
            ("identite", Quat::IDENTITY),
            ("demi-tour Y", Quat::from_rotation_y(std::f32::consts::PI)),
            ("+90 X (le socket reel)", Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            (
                "compose",
                Quat::from_rotation_y(1.1) * Quat::from_rotation_x(0.7),
            ),
        ] {
            let q = correction_de_socket(rot_socket, avant);
            // Ce que la correction promet : le −Z de l'arme, ramené en monde,
            // pointe là où regarde le personnage.
            let canon_monde = rot_socket * (q * Vec3::NEG_Z);
            assert!(
                canon_monde.dot(avant) > 0.999,
                "{nom} : le canon pointe à {:.0}° du regard",
                canon_monde.angle_between(avant).to_degrees()
            );
            // Et il ne part JAMAIS vers l'arrière — l'énoncé du défaut, testé
            // tel quel plutôt que par un angle abstrait.
            assert!(canon_monde.dot(avant) > 0.0, "{nom} : l'arme part en arrière");
        }
    }

    #[test]
    fn la_correction_garde_l_arme_droite_pas_couchee_sur_le_flanc() {
        // Pointer juste ne suffit pas : sans contrôle du roulis, l'arme vise
        // bien et se retrouve à plat. Le +Y de l'arme doit rester vers le haut
        // du monde.
        let rot_socket = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let q = correction_de_socket(rot_socket, Vec3::NEG_Z);
        let haut_monde = rot_socket * (q * Vec3::Y);
        assert!(
            haut_monde.dot(Vec3::Y) > 0.9,
            "l'arme est couchée : son haut fait {:.0}° avec le haut du monde",
            haut_monde.angle_between(Vec3::Y).to_degrees()
        );
    }

    #[test]
    fn un_regard_vertical_ne_produit_pas_de_nan() {
        // Viser à la verticale fait dégénérer le produit vectoriel. Le roulis
        // n'a alors pas de bonne réponse — mais il ne doit pas rendre NaN, ce
        // qui ferait disparaître l'arme du rendu sans un mot.
        let q = correction_de_socket(Quat::IDENTITY, Vec3::Y);
        assert!(q.is_finite(), "quaternion non fini pour un regard vertical");
        let q2 = correction_de_socket(Quat::IDENTITY, Vec3::ZERO);
        assert_eq!(q2, Quat::IDENTITY, "une direction nulle doit rendre l'identité");
    }

    #[test]
    fn l_echelle_se_derive_de_la_mesure_jamais_d_un_litteral() {
        // Le contrat du calibrage : une arme mesurée 2 m qu'on veut à 0,60 m
        // doit être réduite d'un facteur 0,30. C'est la seule façon dont une
        // taille peut être juste sans qu'aucun facteur ne soit écrit à la main.
        let mesuree = 2.0_f32;
        let voulue = 0.60_f32;
        assert!((voulue / mesuree - 0.30).abs() < 1e-6);
    }

    #[test]
    fn une_arme_non_declaree_reste_visible_a_taille_brute() {
        // `taille_m = 0` doit valoir « non déclarée », pas « invisible ». Une
        // arme réduite à un point serait indiscernable d'une arme absente, et
        // enverrait chercher le défaut du mauvais côté.
        let r = ArmeReglage::default();
        assert_eq!(r.taille_m, 0.0);
        assert_eq!(r.rotation(), Quat::IDENTITY);
        assert_eq!(r.decalage(), Vec3::ZERO);
    }

    /// Le garde mécanique qui manquait, et qui aurait épargné cette session.
    ///
    /// Il lit le corps d'Expédition **déclaré au génome d'équipement**, ouvre le
    /// VRAI fichier, et vérifie qu'au moins un ancrage y existe. Le 2026-08-16,
    /// ce corps est passé à `chien_expedition.glb` — bâti par une autre chaîne
    /// d'outils, sans aucun des quatre sockets — et plus une arme n'était tenue.
    /// Rien ne l'a signalé : le code compile, les tests de structure passent, et
    /// le défaut ne se voit qu'en jouant.
    #[test]
    fn le_corps_d_expedition_livre_porte_un_ancrage_de_main() {
        let equip = std::fs::read_to_string("../../assets/genomes/roguelite/roguelite_equipment.toml")
            .expect("génome d'équipement introuvable");
        let v: toml::Value = toml::from_str(&equip).expect("génome d'équipement mal formé");
        let modele = v
            .get("expedition_body")
            .and_then(|b| b.get("model"))
            .and_then(|m| m.as_str())
            .expect("[expedition_body].model non déclaré");

        let chemin = std::path::Path::new("../../assets").join(modele);
        assert!(chemin.exists(), "{modele} déclaré mais absent du disque");
        // On ne sait lire que le GLB binaire (JSON en clair au même endroit).
        // Un `.gltf` externe se lirait tel quel — on ne l'affirme donc pas.
        let Some(noms) = noeuds_du_glb(&chemin) else {
            return;
        };
        assert!(
            noms.iter().any(|n| n == SOCKET_MAIN) || noms.iter().any(|n| n == OS_MAIN_DROITE),
            "{modele} ne porte NI « {SOCKET_MAIN} » NI « {OS_MAIN_DROITE} » : aucune arme \
             ne pourra être tenue. Vérifier la chaîne Blender qui a produit ce corps."
        );
        // La dague vit dans la même main : sans elle, rien à masquer — et le
        // masquage qui échoue silencieusement laisserait deux objets dans le poing.
        assert!(
            noms.iter().any(|n| n == NOEUD_DAGUE),
            "{modele} n'a plus de « {NOEUD_DAGUE} » — le masquage de dague ne mesure plus rien"
        );
    }

    /// L'os et le matériau déclarés pour la dague doivent EXISTER dans le corps.
    ///
    /// Les deux se désignent par leur nom : un nom absent ne plante pas, il ne
    /// fait rien — la dague reste où elle est, blanche, et le seul signal serait
    /// de la voir en jeu. C'est précisément comme ça qu'elle a passé des
    /// semaines à un mètre quarante de la main.
    #[test]
    fn l_os_et_le_materiau_de_la_dague_existent_dans_le_corps() {
        let src = std::fs::read_to_string("../../assets/genomes/expedition_arme_main.toml")
            .expect("génome introuvable");
        let g: ArmeMainGenome = toml::from_str(&src).expect("génome mal formé");

        let equip =
            std::fs::read_to_string("../../assets/genomes/roguelite/roguelite_equipment.toml")
                .expect("génome d'équipement introuvable");
        let v: toml::Value = toml::from_str(&equip).expect("génome d'équipement mal formé");
        let modele = v["expedition_body"]["model"].as_str().unwrap();
        let chemin = std::path::Path::new("../../assets").join(modele);
        let (Some(noeuds), Some(materiaux)) =
            (noeuds_du_glb(&chemin), materiaux_du_glb(&chemin))
        else {
            return; // pas un GLB binaire : on n'affirme rien
        };

        if !g.dague.os.is_empty() {
            assert!(
                noeuds.contains(&g.dague.os),
                "l'os « {} » déclaré pour la dague n'existe pas dans {modele} — \
                 elle resterait à 1,39 m de la main",
                g.dague.os
            );
        }
        if !g.dague.materiau.is_empty() {
            assert!(
                materiaux.contains(&g.dague.materiau),
                "le matériau « {} » n'existe pas dans {modele} — la dague resterait \
                 blanche. Matériaux du fichier : {materiaux:?}",
                g.dague.materiau
            );
        }
    }

    /// Noms des matériaux d'un GLB. Même contrat que `noeuds_du_glb`.
    fn materiaux_du_glb(chemin: &std::path::Path) -> Option<Vec<String>> {
        let d = std::fs::read(chemin).ok()?;
        if d.len() < 20 || &d[0..4] != b"glTF" {
            return None;
        }
        let taille = u32::from_le_bytes(d[12..16].try_into().ok()?) as usize;
        let json: serde_json::Value = serde_json::from_slice(d.get(20..20 + taille)?).ok()?;
        Some(
            json.get("materials")?
                .as_array()?
                .iter()
                .filter_map(|m| m.get("name")?.as_str().map(str::to_owned))
                .collect(),
        )
    }

    /// Noms des nœuds d'un GLB, lus dans son chunk JSON. `None` si ce n'est pas
    /// un GLB binaire : on préfère ne rien affirmer plutôt que de rendre une
    /// liste vide, qui se lirait comme « aucun nœud ».
    fn noeuds_du_glb(chemin: &std::path::Path) -> Option<Vec<String>> {
        let d = std::fs::read(chemin).ok()?;
        if d.len() < 20 || &d[0..4] != b"glTF" {
            return None;
        }
        let taille = u32::from_le_bytes(d[12..16].try_into().ok()?) as usize;
        let json: serde_json::Value = serde_json::from_slice(d.get(20..20 + taille)?).ok()?;
        Some(
            json.get("nodes")?
                .as_array()?
                .iter()
                .filter_map(|n| n.get("name")?.as_str().map(str::to_owned))
                .collect(),
        )
    }

    #[test]
    fn le_genome_livre_declare_les_quatre_armes() {
        // Le fichier est lu depuis le disque : c'est ce qui distingue ce test
        // d'une simple vérification de structure. Une arme oubliée là paraîtrait
        // à la taille brute de son GLB, sans que rien ne le dise.
        let src = std::fs::read_to_string("../../assets/genomes/expedition_arme_main.toml")
            .expect("génome introuvable");
        let g: ArmeMainGenome = toml::from_str(&src).expect("génome mal formé");
        for arme in [
            WeaponType::ModernAR,
            WeaponType::AssaultRifle,
            WeaponType::Shotgun,
            WeaponType::RocketLauncher,
        ] {
            let r = g.reglage(arme);
            assert!(
                r.taille_m > 0.1,
                "{} n'a pas de taille déclarée",
                arme.genome_key()
            );
            // Une arme plus longue qu'un homme est une erreur de saisie, pas un
            // choix : le personnage mesure 1,82 m.
            assert!(
                r.taille_m < 1.82,
                "{} déclarée à {:.2} m — plus longue que le personnage",
                arme.genome_key(),
                r.taille_m
            );
            // Une prise à 0,5 remettrait le milieu de l'arme dans le poing —
            // le défaut rapporté en jeu le 2026-08-15. Les armes se tiennent
            // dans leur moitié arrière.
            assert!(
                (0.0..0.5).contains(&r.prise),
                "{} tenue à {:.2} — hors de la moitié arrière",
                arme.genome_key(),
                r.prise
            );
        }
        assert_eq!(g.armes.len(), 4, "le génome doit couvrir les 4 armes livrées");
    }
}
