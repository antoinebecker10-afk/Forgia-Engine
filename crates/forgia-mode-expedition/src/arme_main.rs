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

#[derive(TypePath, Deserialize, Debug, Clone)]
pub struct ArmeMainGenome {
    #[serde(default = "vrai")]
    pub cacher_dague: bool,
    #[serde(default)]
    pub armes: HashMap<String, ArmeReglage>,
    /// Comment on tient l'arme au repos et en visée. Vit dans CE fichier parce
    /// que la tenue et l'arme tenue ne se règlent pas l'une sans l'autre : une
    /// pose se juge avec l'arme dans la main, pas dans l'abstrait.
    #[serde(default)]
    pub visee: crate::visee::ViseeGenome,
}

impl Default for ArmeMainGenome {
    fn default() -> Self {
        Self {
            cacher_dague: true,
            armes: HashMap::new(),
            visee: crate::visee::ViseeGenome::default(),
        }
    }
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
    pub arme_posee: Option<WeaponType>,
    pub taille_mesuree_m: f32,
    pub taille_cible_m: f32,
    pub echelle: f32,
    pub calibree: bool,
    pub dague_masquee: bool,
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
    for (entite, nom) in &q_noms {
        match nom.as_str() {
            SOCKET_MAIN if manque_socket => {
                commands.entity(entite).insert(SocketMain);
                info!("[arme-main] socket « {SOCKET_MAIN} » trouvé");
            }
            NOEUD_DAGUE if manque_dague => {
                commands.entity(entite).insert(DagueDuCorps);
            }
            _ => {}
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
    mut etat: ResMut<EtatArmeMain>,
) {
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
        let bouche = bouche_locale(min, max, tf.rotation);
        // Ce que la main tient : la POIGNÉE, pas le milieu. Sans ce décalage,
        // l'origine du modèle — son centre, mesuré — se retrouve dans le poing.
        let recul = recul_de_prise(min, max, tf.rotation, cible.prise) * echelle;

        commands
            .entity(entite)
            .remove::<ACalibrer>()
            .insert(Transform {
                scale: Vec3::splat(echelle),
                // L'avant est −Z APRÈS rotation : reculer la prise revient donc
                // à avancer l'arme, dans le repère du socket, le long de −Z.
                translation: tf.translation + Vec3::NEG_Z * recul,
                ..*tf
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

/// Masque la dague du corps tant qu'une arme est tenue, et la rend au retrait.
fn masquer_dague(
    genome_handle: Option<Res<ArmeMainGenomeHandle>>,
    genomes: Res<Assets<Genome<ArmeMainGenome>>>,
    q_arme: Query<(), With<ArmeEnMain>>,
    mut q_dague: Query<&mut Visibility, With<DagueDuCorps>>,
    mut etat: ResMut<EtatArmeMain>,
) {
    let cacher = genome_handle
        .as_deref()
        .and_then(|h| genomes.get(&h.0))
        .map(|g| g.data.cacher_dague)
        .unwrap_or(true)
        && !q_arme.is_empty();
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

/// `forgia2_expedition_arme.json`, 1 Hz.
fn capteur_arme_main(
    time: Res<Time>,
    mut accum: Local<f32>,
    mode: Res<State<GameMode>>,
    genome_handle: Option<Res<ArmeMainGenomeHandle>>,
    genomes: Res<Assets<Genome<ArmeMainGenome>>>,
    muzzle: Res<MuzzleWorld>,
    q_arme: Query<&ArmeEnMain>,
    mut etat: ResMut<EtatArmeMain>,
) {
    let en_expedition = *mode.get() == GameMode::Expedition;
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
    let armes_declarees = genome_handle
        .as_deref()
        .and_then(|h| genomes.get(&h.0))
        .map(|g| g.data.armes.len())
        .unwrap_or(0);
    let arme = q_arme
        .iter()
        .next()
        .map(|a| a.arme.genome_key())
        .unwrap_or("");
    let bouche = muzzle.0.unwrap_or(Vec3::ZERO);

    // Deux secondes : le corps est un glTF de 81 nœuds chargé en tâche de fond,
    // il n'est jamais là à la première frame. En deçà, une alerte serait du
    // bruit à chaque entrée dans la carte.
    const DELAI_DE_GRACE_S: f32 = 2.0;
    let installe = etat.depuis_entree_s > DELAI_DE_GRACE_S;

    let (severity, next_step) = if !en_expedition {
        ("info", "hors Expedition — aucune arme en main attendue")
    } else if !genome_charge && installe {
        (
            "warn",
            "GENOME_ABSENT : genomes/expedition_arme_main.toml non chargé — l'arme prend taille et orientation brutes du GLB",
        )
    } else if armes_declarees > 0 && etat.taille_cible_m <= 0.0 && installe {
        (
            "warn",
            "ARME_NON_DECLAREE : le génome est chargé mais l'arme équipée n'y a pas d'entrée — ajouter [armes.<clé>] (clé = nom du GLB)",
        )
    } else if !etat.socket_trouve && installe {
        (
            "critical",
            "SOCKET_ABSENT : aucun nœud « socket_main_droite » dans la scène — le corps chargé n'est pas celui de l'Expédition (stylized_male_complet.glb), ou l'avatar n'est pas monté",
        )
    } else if arme.is_empty() && etat.socket_trouve && installe {
        (
            "warn",
            "ARME_NON_ACCROCHEE : socket trouvé mais aucune arme posée dessous — vérifier EquippedWeapons",
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
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"expedition_arme","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"en_expedition":{en_expedition},"socket_trouve":{},"arme":"{arme}","calibree":{},"taille_mesuree_m":{:.3},"taille_cible_m":{:.2},"echelle":{:.4},"dague_masquee":{},"bouche":[{:.2},{:.2},{:.2}],"bouche_publiee":{},"genome_charge":{genome_charge},"armes_declarees":{armes_declarees}}}"#,
        time.elapsed_secs(),
        etat.socket_trouve,
        etat.calibree,
        etat.taille_mesuree_m,
        etat.taille_cible_m,
        etat.echelle,
        etat.dague_masquee,
        bouche.x,
        bouche.y,
        bouche.z,
        muzzle.0.is_some(),
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
