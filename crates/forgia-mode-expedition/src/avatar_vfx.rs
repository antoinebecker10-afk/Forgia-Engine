//! Les feux portés par l'avatar de l'Expédition : ourlet ardent et traînée de pas.
//!
//! # Ce module ne spawne PAS de personnage — et c'est la leçon
//!
//! Sa première version en spawnait un. Résultat en jeu : **deux avatars
//! superposés**, dont un figé bras écartés. Le corps de l'Expédition est déjà
//! posé par `forgia-mode-roguelite::avatar`, piloté par le contrôleur et animé
//! depuis `[expedition_body]` du génome d'équipement. Il n'y avait rien à
//! spawner, seulement des feux à accrocher.
//!
//! # L'accroche se fait sur les OS, pas sur mes sockets
//!
//! `26_cape_ardente.py` a posé 4 sockets nommés dans `stylized_male_complet.glb`.
//! Mais l'autre corps disponible (`stylized_male_animated.glb`) n'en a aucun,
//! alors que les **62 os sont identiques entre les deux** (vérifié : nommage
//! Mixamo, `LeftFoot` / `RightFoot` / `Spine2` présents des deux côtés).
//! S'accrocher aux os marche donc quel que soit le corps chargé ; s'accrocher
//! aux sockets aurait lié ce module à un seul fichier.
//!
//! # Deux pièges du dépôt, payés avant moi
//!
//! 1. **`SpawnerSettings::rate` ne rend rien ici** — `weapon_vfx/status.rs:4`
//!    le dit noir sur blanc. Tout le VFX maison passe par des **bursts**,
//!    redéclenchés en **retirant `EffectSpawner`** (cf. `element_vfx.rs:220`).
//!    D'où : une bouffée **par pas**, pas un débit.
//! 2. **Un matériau émissif n'éclaire pas.** Il brille, et c'est tout. Sans les
//!    `PointLight` posées ici, l'ourlet ardent se verrait sans illuminer quoi
//!    que ce soit — or la consigne était que la cape et les traînées éclairent.
//!
//! # Discrétion
//!
//! « Un bel effet brûlant mais discret ». D'où : l'ourlet ne couvre que 21 %
//! des faces de la cape, et la traînée n'existe **qu'en mouvement**.

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use serde::Deserialize;

use forgia_core::prelude::{AppMode, GameMode};

/// Désambiguïse : `bevy::prelude::Gradient` et celui de hanabi portent le même
/// nom. `weapon_vfx/impact.rs` a tranché de la même façon.
use bevy_hanabi::Gradient as HanabiGradient;

/// Les os d'accroche. Nommage Mixamo, présents dans les DEUX corps livrés.
/// Les sockets nommés existent aussi dans `stylized_male_complet.glb`, mais ils
/// sont enfants de ces mêmes os : viser l'os donne le même point d'ancrage et
/// ne dépend pas du fichier chargé.
const OS_PIED_GAUCHE: &str = "LeftFoot";
const OS_PIED_DROIT: &str = "RightFoot";
const OS_DOS: &str = "Spine2";

/// Les réglages de feu vivent en couche **definition** (`no-hardcode.md`) : ils
/// se jugent en jeu, de nuit, pas dans un tableur.
const GENOME_VFX: &str = "assets/genomes/expedition_vfx.toml";

/// Noms des sockets tels qu'ils sortent du glTF. Ils sont écrits ici ET dans
/// `26_cape_ardente.py` — c'est une grandeur écrite deux fois, donc le test
/// `les_sockets_couvrent_les_deux_pieds_et_le_dos` existe pour le rappeler.
pub const SOCKET_PIED_GAUCHE: &str = "socket_pied_gauche";
pub const SOCKET_PIED_DROIT: &str = "socket_pied_droit";
pub const SOCKET_MAIN_DROITE: &str = "socket_main_droite";
pub const SOCKET_DOS: &str = "socket_dos";

/// Combien de sockets ce module équipe : deux pieds et le dos. Sert de borne
/// d'arrêt au balayage — sans elle, `accrocher_trainees` relirait tous les
/// noms de la scène à chaque frame (`scalability.md`).
const SOCKETS_A_EQUIPER: u32 = 3;

pub struct ExpeditionAvatarPlugin;

impl Plugin for ExpeditionAvatarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvatarExpedition>()
            .insert_resource(VfxConfig::charger())
            .add_systems(OnExit(GameMode::Expedition), retirer_avatar)
            .add_systems(
                Update,
                (accrocher_trainees, moduler_trainees)
                    .chain()
                    .run_if(in_state(GameMode::Expedition))
                    .run_if(in_state(AppMode::InGame)),
            );
    }
}

// ---------------------------------------------------------------------------
// Réglages
// ---------------------------------------------------------------------------

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct VfxConfig {
    pub vitesse_min_ms: f32,
    pub vitesse_pleine_ms: f32,
    pub pas_m: f32,
    pub braises_par_pas: f32,
    pub braise_duree_s: f32,
    pub braise_taille_m: f32,
    pub braise_rayon_m: f32,
    pub braise_vitesse_ms: f32,
    pub lueur_pied_intensite: f32,
    pub lueur_pied_portee_m: f32,
    pub lueur_cape_intensite: f32,
    pub lueur_cape_portee_m: f32,
    pub lueur_fondu_par_s: f32,
}

impl Default for VfxConfig {
    fn default() -> Self {
        Self {
            vitesse_min_ms: 1.5,
            vitesse_pleine_ms: 8.0,
            pas_m: 0.9,
            braises_par_pas: 14.0,
            braise_duree_s: 0.55,
            braise_taille_m: 0.085,
            braise_rayon_m: 0.06,
            braise_vitesse_ms: 0.35,
            lueur_pied_intensite: 4000.0,
            lueur_pied_portee_m: 4.0,
            lueur_cape_intensite: 9000.0,
            lueur_cape_portee_m: 5.5,
            lueur_fondu_par_s: 6.0,
        }
    }
}

#[derive(Deserialize)]
struct VfxToml {
    #[serde(default)]
    trainee: Option<TraineeToml>,
    #[serde(default)]
    lueur: Option<LueurToml>,
}

#[derive(Deserialize)]
struct TraineeToml {
    vitesse_min_ms: f32,
    vitesse_pleine_ms: f32,
    pas_m: f32,
    braises_par_pas: f32,
    braise_duree_s: f32,
    braise_taille_m: f32,
    braise_rayon_m: f32,
    braise_vitesse_ms: f32,
}

#[derive(Deserialize)]
struct LueurToml {
    pied_intensite: f32,
    pied_portee_m: f32,
    cape_intensite: f32,
    cape_portee_m: f32,
    fondu_par_s: f32,
}

impl VfxConfig {
    /// Lit le génome, ou retombe sur les valeurs par défaut **en le disant**.
    /// Un repli silencieux ferait passer un fichier absent pour un réglage
    /// volontaire — c'est exactement le défaut wasm déjà payé sur ce projet
    /// (`std::fs` direct → génome muet), d'où `def_io`.
    #[must_use]
    pub fn charger() -> Self {
        match forgia_core::def_io::read_def_str(GENOME_VFX) {
            Ok(s) => Self::depuis_toml(&s),
            Err(e) => {
                warn!("[avatar-exp] {GENOME_VFX} illisible ({e}) — réglages par défaut");
                Self::default()
            }
        }
    }

    fn depuis_toml(contenu: &str) -> Self {
        Self::lire_strict(contenu).unwrap_or_else(|| {
            warn!("[avatar-exp] {GENOME_VFX} mal formé — réglages par défaut");
            Self::default()
        })
    }

    /// Rend `None` quand le fichier ne se lit pas, au lieu de retomber en
    /// silence sur les défauts.
    ///
    /// **Pourquoi cette fonction existe.** Ma première version repliait
    /// directement sur `default()`. Comme le génome livré porte précisément les
    /// valeurs par défaut, un fichier illisible et un fichier bien lu rendaient
    /// le MÊME objet : le test censé prouver la lecture ne distinguait plus les
    /// deux cas. Un repli doit être observable, sinon il masque exactement la
    /// panne qu'on voulait voir.
    fn lire_strict(contenu: &str) -> Option<Self> {
        let mut cfg = Self::default();
        let t = toml::from_str::<VfxToml>(contenu).ok()?;
        if let Some(t) = t.trainee {
            cfg.vitesse_min_ms = t.vitesse_min_ms;
            cfg.vitesse_pleine_ms = t.vitesse_pleine_ms;
            cfg.pas_m = t.pas_m;
            cfg.braises_par_pas = t.braises_par_pas;
            cfg.braise_duree_s = t.braise_duree_s;
            cfg.braise_taille_m = t.braise_taille_m;
            cfg.braise_rayon_m = t.braise_rayon_m;
            cfg.braise_vitesse_ms = t.braise_vitesse_ms;
        }
        if let Some(l) = t.lueur {
            cfg.lueur_pied_intensite = l.pied_intensite;
            cfg.lueur_pied_portee_m = l.pied_portee_m;
            cfg.lueur_cape_intensite = l.cape_intensite;
            cfg.lueur_cape_portee_m = l.cape_portee_m;
            cfg.lueur_fondu_par_s = l.fondu_par_s;
        }
        Some(cfg)
    }

    /// Part de traînée `[0, 1]` à une vitesse donnée. **Pure**, donc testable
    /// sans monter une `App` — c'est l'invariant « discret » qui se vérifie ici.
    #[must_use]
    pub fn part_a(&self, vitesse: f32) -> f32 {
        ((vitesse - self.vitesse_min_ms) / (self.vitesse_pleine_ms - self.vitesse_min_ms))
            .clamp(0.0, 1.0)
    }
}

// ---------------------------------------------------------------------------
// État et marqueurs
// ---------------------------------------------------------------------------

/// Ce que la passe de préparation retient, pour que l'état soit lisible depuis
/// un capteur au lieu d'être deviné.
#[derive(Resource, Default)]
pub struct AvatarExpedition {
    /// Os équipés (2 pieds + 1 dos). Reste à 0 tant que le corps n'est pas là.
    pub sockets_trouves: u32,
    pub bouffees: u32,
    pub etat: &'static str,
}

/// Un socket déjà équipé — sert à ne pas le ré-équiper à la frame suivante.
#[derive(Component)]
struct SocketEquipe;

/// L'effet accroché sous un pied.
#[derive(Component)]
struct TraineeDePas;

/// La lumière d'un pied, montée et descendue avec la vitesse.
#[derive(Component)]
struct LueurDePas;

/// La lumière de la cape : douce et constante, l'ourlet fait le reste.
#[derive(Component)]
struct LueurDeCape;

/// Mémorise la position de la frame précédente et la distance depuis la
/// dernière bouffée. On ne lit pas la vélocité du contrôleur : ce module ne
/// doit rien savoir de lui, sinon il ne serait plus branchable ailleurs.
#[derive(Component)]
struct SuiviDuPied {
    position: Vec3,
    depuis_le_pas: f32,
    initialisee: bool,
}

// ---------------------------------------------------------------------------
// Cycle de vie
// ---------------------------------------------------------------------------

/// Retire NOS effets à la sortie — jamais le corps, qui ne nous appartient pas.
///
/// Despawner la racine reviendrait à supprimer l'avatar d'un autre système.
/// On ne retire que ce qu'on a posé : les entités marquées.
fn retirer_avatar(
    mut commands: Commands,
    q_effets: Query<Entity, Or<(With<TraineeDePas>, With<LueurDePas>, With<LueurDeCape>)>>,
    q_marques: Query<Entity, With<SocketEquipe>>,
    mut etat: ResMut<AvatarExpedition>,
) {
    let mut n = 0;
    for e in &q_effets {
        commands.entity(e).despawn();
        n += 1;
    }
    // La marque part aussi : sans ça, un retour dans la carte trouverait les os
    // « déjà équipés » et n'accrocherait plus rien.
    for e in &q_marques {
        commands.entity(e).remove::<SocketEquipe>();
    }
    *etat = AvatarExpedition {
        etat: "sorti",
        ..default()
    };
    info!("[avatar-exp] {n} effet(s) retiré(s)");
}

/// Relève les clips du GLB et les range dans la ressource, par NOM.
///
/// Le moteur joue des clips nommés : c'est pour ça que les 3 clips exportés par
/// Mixamo sous `Armature|mixamo.com|Layer0` ont été renommés d'après leur
/// fichier avant l'export. Trois animations homonymes auraient été
/// indiscernables ici.
/// Construit l'effet de traînée. Écrit en Rust : les `.particle` du dépôt sont
/// des marqueurs vides (75 octets de commentaire), et `bevy_hanabi` définit ses
/// effets par expressions, pas par fichier.
fn effet_trainee(effets: &mut Assets<EffectAsset>, cfg: &VfxConfig) -> Handle<EffectAsset> {
    let mut couleur = HanabiGradient::new();
    // Braise vive au contact, puis refroidit et s'éteint. L'alpha tombe à 0
    // AVANT la fin de vie : une particule qui disparaît d'un coup se voit.
    couleur.add_key(0.0, Vec4::new(1.0, 0.55, 0.12, 0.85));
    couleur.add_key(0.35, Vec4::new(0.95, 0.25, 0.05, 0.55));
    couleur.add_key(1.0, Vec4::new(0.25, 0.06, 0.02, 0.0));

    let mut taille = HanabiGradient::new();
    taille.add_key(0.0, Vec3::splat(cfg.braise_taille_m));
    taille.add_key(1.0, Vec3::splat(cfg.braise_taille_m * 0.18));

    let auteur = ExprWriter::new();
    let duree = auteur
        .lit(cfg.braise_duree_s)
        .uniform(auteur.lit(cfg.braise_duree_s * 1.6))
        .expr();
    let init_age = SetAttributeModifier::new(Attribute::AGE, auteur.lit(0.0).expr());
    let init_vie = SetAttributeModifier::new(Attribute::LIFETIME, duree);
    let init_pos = SetPositionSphereModifier {
        center: auteur.lit(Vec3::ZERO).expr(),
        radius: auteur.lit(cfg.braise_rayon_m).expr(),
        dimension: ShapeDimension::Volume,
    };
    // Les braises MONTENT doucement : sans cette dérive elles restent collées
    // au sol et se lisent comme une tache, pas comme un feu.
    let init_vit = SetVelocitySphereModifier {
        center: auteur.lit(Vec3::new(0.0, -0.35, 0.0)).expr(),
        speed: auteur
            .lit(cfg.braise_vitesse_ms)
            .uniform(auteur.lit(cfg.braise_vitesse_ms * 2.0))
            .expr(),
    };

    let effet = EffectAsset::new(
        256,
        // Période énorme : le burst ne se répète JAMAIS tout seul. C'est le
        // retrait de `EffectSpawner` qui le redéclenche, à chaque pas.
        SpawnerSettings::burst(cfg.braises_par_pas.into(), 99_999.0.into()),
        auteur.finish(),
    )
    .with_name("trainee_de_pas")
    .init(init_pos)
    .init(init_vit)
    .init(init_age)
    .init(init_vie)
    .render(ColorOverLifetimeModifier {
        gradient: couleur,
        ..default()
    })
    .render(SizeOverLifetimeModifier {
        gradient: taille,
        screen_space_size: false,
    });
    effets.add(effet)
}

/// Accroche une traînée et sa lumière sous chaque socket, une seule fois.
fn accrocher_trainees(
    mut commands: Commands,
    mut effets: ResMut<Assets<EffectAsset>>,
    cfg: Res<VfxConfig>,
    q_noms: Query<(Entity, &Name), Without<SocketEquipe>>,
    mut etat: ResMut<AvatarExpedition>,
) {
    // Borne d'arrêt : sans elle ce système relit tous les noms de la scène à
    // chaque frame, pour toujours (`scalability.md` — pas de balayage inutile).
    if etat.sockets_trouves >= SOCKETS_A_EQUIPER {
        return;
    }
    let mut poses = 0;
    let mut effet = None;
    for (entite, nom) in &q_noms {
        let n = nom.as_str();
        let pied = n == OS_PIED_GAUCHE || n == OS_PIED_DROIT;
        let dos = n == OS_DOS;
        if !pied && !dos {
            continue;
        }
        commands.entity(entite).insert(SocketEquipe);

        if pied {
            let handle = effet
                .get_or_insert_with(|| effet_trainee(&mut effets, &cfg))
                .clone();
            // ENFANTS du socket : ils suivent l'os pendant les clips. Une
            // entité posée à la position du pied resterait sur place.
            commands.entity(entite).with_children(|parent| {
                parent.spawn((
                    ParticleEffect::new(handle),
                    Transform::default(),
                    TraineeDePas,
                    Name::new(format!("trainee_{n}")),
                ));
                // LA LUMIÈRE. Sans elle la braise brillerait sans rien éclairer.
                parent.spawn((
                    PointLight {
                        color: Color::srgb(1.0, 0.55, 0.18),
                        intensity: 0.0, // montée par `moduler_trainees`
                        range: cfg.lueur_pied_portee_m,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.15, 0.0),
                    LueurDePas,
                    Name::new(format!("lueur_{n}")),
                ));
            });
            commands.entity(entite).insert(SuiviDuPied {
                position: Vec3::ZERO,
                depuis_le_pas: 0.0,
                initialisee: false,
            });
        } else {
            // La cape : une lueur douce et constante. Son ourlet émissif fait
            // le reste du travail, et il est déjà cuit dans le GLB.
            commands.entity(entite).with_children(|parent| {
                parent.spawn((
                    PointLight {
                        color: Color::srgb(1.0, 0.42, 0.12),
                        intensity: cfg.lueur_cape_intensite,
                        range: cfg.lueur_cape_portee_m,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, -0.35, -0.12),
                    LueurDeCape,
                    Name::new("lueur_cape"),
                ));
            });
        }
        poses += 1;
    }
    if poses > 0 {
        etat.sockets_trouves += poses;
        etat.etat = "sockets_equipes";
        info!(
            "[avatar-exp] {poses} socket(s) équipé(s) — {}/{SOCKETS_A_EQUIPER}",
            etat.sockets_trouves
        );
    }
}

/// Déclenche une bouffée à chaque pas, et fait vivre la lueur avec la vitesse.
///
/// C'est ce qui rend l'effet « discret » : immobile, aucune bouffée et lumière
/// éteinte ; à la marche, une braise tous les `pas_m` ; à la course, davantage,
/// parce que la distance se parcourt plus vite. Un débit constant aurait donné
/// un brasier permanent, y compris planté dans le village.
fn moduler_trainees(
    mut commands: Commands,
    temps: Res<Time>,
    cfg: Res<VfxConfig>,
    mut q_socket: Query<(&GlobalTransform, &mut SuiviDuPied, &Children)>,
    q_effet: Query<Entity, With<TraineeDePas>>,
    mut q_lueur: Query<&mut PointLight, With<LueurDePas>>,
    mut etat: ResMut<AvatarExpedition>,
) {
    let dt = temps.delta_secs().max(1e-4);
    let mut bouffees = 0;
    for (transforme, mut suivi, enfants) in &mut q_socket {
        let ici = transforme.translation();
        if !suivi.initialisee {
            // Première frame : on n'a pas de précédente position, donc pas de
            // vitesse. La déduire de `Vec3::ZERO` donnerait un pic énorme et
            // une bouffée parasite au spawn.
            suivi.position = ici;
            suivi.initialisee = true;
            continue;
        }
        let pas = (ici - suivi.position).length();
        let vitesse = pas / dt;
        suivi.position = ici;

        if vitesse >= cfg.vitesse_min_ms {
            suivi.depuis_le_pas += pas;
        } else {
            // Sous le seuil, on REMET À ZÉRO : sinon un piétinement long finit
            // par cumuler `pas_m` et crache une braise à l'arrêt.
            suivi.depuis_le_pas = 0.0;
        }

        let declenche = suivi.depuis_le_pas >= cfg.pas_m;
        if declenche {
            suivi.depuis_le_pas = 0.0;
        }
        let part = cfg.part_a(vitesse);

        for enfant in enfants.iter() {
            if declenche && q_effet.get(enfant).is_ok() {
                // L'idiome du dépôt : retirer `EffectSpawner` relance le burst.
                commands.entity(enfant).remove::<EffectSpawner>();
                bouffees += 1;
            }
            if let Ok(mut lumiere) = q_lueur.get_mut(enfant) {
                let cible = part * cfg.lueur_pied_intensite;
                lumiere.intensity +=
                    (cible - lumiere.intensity) * (dt * cfg.lueur_fondu_par_s).min(1.0);
            }
        }
    }
    if bouffees > 0 {
        etat.bouffees = etat.bouffees.saturating_add(bouffees);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le génome livré doit se LIRE — pas seulement « produire un objet ».
    ///
    /// La nuance a coûté un test inutile : comme le fichier porte les valeurs
    /// par défaut, comparer le résultat à `default()` ne prouvait rien. C'est
    /// `lire_strict` qui tranche, parce qu'elle rend `None` sur échec.
    #[test]
    fn le_genome_livre_se_lit() {
        let toml = include_str!("../../../assets/genomes/expedition_vfx.toml");
        let cfg = VfxConfig::lire_strict(toml).expect("le génome livré ne se lit pas");
        assert!(cfg.pas_m > 0.0, "un pas nul déclencherait à chaque frame");
        assert!(cfg.vitesse_pleine_ms > cfg.vitesse_min_ms, "plage vide");
        assert!(cfg.braises_par_pas >= 1.0, "moins d'une braise = rien à voir");
    }

    /// Et chaque champ doit être RELIÉ. Un `serde` qui parse sans erreur mais
    /// dont j'aurais oublié une affectation rendrait la valeur par défaut sans
    /// rien signaler — c'est le même angle mort, un cran plus bas.
    #[test]
    fn chaque_champ_du_toml_atteint_la_config() {
        let cfg = VfxConfig::lire_strict(
            r#"
            [trainee]
            vitesse_min_ms = 1.0
            vitesse_pleine_ms = 2.0
            pas_m = 3.0
            braises_par_pas = 4.0
            braise_duree_s = 5.0
            braise_taille_m = 6.0
            braise_rayon_m = 7.0
            braise_vitesse_ms = 8.0
            [lueur]
            pied_intensite = 9.0
            pied_portee_m = 10.0
            cape_intensite = 11.0
            cape_portee_m = 12.0
            fondu_par_s = 13.0
            "#,
        )
        .expect("TOML de test illisible");
        assert_eq!(
            [
                cfg.vitesse_min_ms,
                cfg.vitesse_pleine_ms,
                cfg.pas_m,
                cfg.braises_par_pas,
                cfg.braise_duree_s,
                cfg.braise_taille_m,
                cfg.braise_rayon_m,
                cfg.braise_vitesse_ms,
                cfg.lueur_pied_intensite,
                cfg.lueur_pied_portee_m,
                cfg.lueur_cape_intensite,
                cfg.lueur_cape_portee_m,
                cfg.lueur_fondu_par_s,
            ],
            [
                1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0
            ]
        );
    }

    /// Un fichier illisible DOIT se voir. Sans ce test, le repli silencieux
    /// reviendrait à la première occasion.
    #[test]
    fn un_genome_casse_est_signale_pas_masque() {
        assert!(VfxConfig::lire_strict("ceci n'est pas du toml {{{").is_none());
    }

    /// La modulation DOIT s'éteindre à l'arrêt. C'est l'invariant qui tient la
    /// consigne « discret » : sans lui, le joueur laisse un brasier en restant
    /// planté dans le village.
    #[test]
    fn la_trainee_s_eteint_a_l_arret() {
        let cfg = VfxConfig::default();
        assert_eq!(cfg.part_a(0.0), 0.0, "immobile => aucune braise");
        assert_eq!(cfg.part_a(cfg.vitesse_min_ms), 0.0, "au seuil, rien encore");
        assert!(cfg.part_a(6.5) > 0.0, "à la marche (6,5 m/s), ça existe");
        assert_eq!(cfg.part_a(9.75), 1.0, "au sprint, pleine lueur");
    }

    /// Les points d'accroche sont des OS, pas mes sockets.
    ///
    /// Ce test garde la leçon qui a coûté un avatar en double : les sockets de
    /// `26_cape_ardente.py` n'existent que dans `stylized_male_complet.glb`,
    /// alors que ces trois os existent dans les DEUX corps livrés (62 os
    /// identiques, vérifié sur les deux GLB). Viser l'os rend ce module
    /// indépendant du fichier chargé.
    #[test]
    fn l_accroche_vise_des_os_mixamo_pas_des_sockets() {
        for os in [OS_PIED_GAUCHE, OS_PIED_DROIT, OS_DOS] {
            assert!(
                !os.starts_with("socket_"),
                "{os} est un socket : il n'existe que dans un des deux corps"
            );
        }
        // Nommage Mixamo, celui que les deux squelettes partagent.
        assert_eq!(OS_PIED_GAUCHE, "LeftFoot");
        assert_eq!(OS_PIED_DROIT, "RightFoot");
        assert_eq!(OS_DOS, "Spine2");
    }

    /// `SOCKETS_A_EQUIPER` sert de borne d'arrêt au balayage. Si elle dépasse
    /// le nombre de sockets réellement équipés, le système ne s'arrête jamais.
    #[test]
    fn la_borne_d_arret_correspond_aux_sockets_equipes() {
        let equipes = [SOCKET_PIED_GAUCHE, SOCKET_PIED_DROIT, SOCKET_DOS];
        assert_eq!(SOCKETS_A_EQUIPER as usize, equipes.len());
        assert!(
            !equipes.contains(&SOCKET_MAIN_DROITE),
            "la dague n'est pas un socket de feu"
        );
    }
}
