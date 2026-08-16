//! La rivière du Vallon : la charger, la faire ressembler à de l'eau, la faire
//! couler.
//!
//! # Pourquoi ce module existe
//!
//! Le maillage `vallon_eau.gltf` était sur le disque, complet et correct — 192
//! triangles, UV présentes, descendant de 7,69 m exactement comme le manifeste
//! le déclare — et **aucune ligne du moteur ne le chargeait**. Il ne fait pas
//! partie des 48 cellules de streaming (c'est un ruban unique de 190 m de long,
//! le découper n'aurait pas de sens), donc le système qui charge les cellules ne
//! le voyait pas. Le symptôme rapporté en jeu était « il manque l'eau » ; la
//! cause n'était ni un asset absent, ni un bug de rendu, mais un fichier que
//! personne ne cite.
//!
//! 🚨 La classe de défaut, plus large que ce cas : **un fichier produit par la
//! chaîne d'assets n'est pas chargé parce qu'il l'est**. Deux fichiers étaient
//! dans ce cas — l'eau et la porte du village. Le contrôle qui l'attrape
//! maintenant est le test `tout_gltf_produit_est_cite`.
//!
//! # Le courant
//!
//! `tuile_m` et `courant_tuiles_par_s` sont **déclarés au manifeste de la
//! carte** par l'auteur sous Blender, avec le sens (« V croissant = vers
//! l'aval »). Ce module les CONSOMME : il ne redéclare ni l'un ni l'autre. Une
//! valeur déclarée sans consommateur n'est pas une donnée, c'est un commentaire.
//!
//! # Pourquoi `bevy_water` et pas une carte de normales maison
//!
//! La première version fabriquait sa propre carte de normales et la faisait
//! défiler. Résultat en jeu : du velours côtelé. La cause n'était pas la
//! texture mais le modèle d'éclairage — une surface quasi miroir sous une
//! lumière rasante transforme chaque ondulation en arête spéculaire.
//!
//! `bevy_water` — **déjà dans le workspace**, déjà branché par
//! `forgia_water::ForgiaWaterPlugin` — apporte ce qu'une carte de normales ne
//! peut pas donner : une couleur qui varie avec la **profondeur** (`deep_color`
//! / `shallow_color`, via le tampon de profondeur), de l'**écume aux berges**
//! (`edge_color`), et des vagues calculées dans le shader plutôt qu'échantillonnées.
//!
//! 🚨 Ce qu'il faut savoir avant d'y toucher : son `WaterPlugin` sert d'ordinaire
//! une NAPPE HORIZONTALE (`WaterSettings.height`, un plan infini au niveau de la
//! mer). Notre rivière DESCEND de 7,69 m — aucun plan horizontal ne la suit. On
//! n'utilise donc pas ses tuiles : on pose son **matériau** sur le ruban exporté
//! par Blender. `StandardWaterMaterial` est un `ExtendedMaterial`, il va sur
//! n'importe quel maillage.

use bevy::prelude::*;
use bevy_water::material::{StandardWaterMaterial, WaterMaterial};
use serde::Deserialize;

use crate::plugin::{ActiveExpedition, ExpeditionMarker};

const MODELE_EAU: &str =
    "models/environment/expedition/vallon_stream_cells/vallon_eau.gltf#Scene0";
const GENOME_EAU: &str = "assets/genomes/expedition_eau.toml";

/// L'apparence de la surface, réglable en couche definition.
#[derive(Debug, Clone, Deserialize)]
pub struct SurfaceEau {
    /// Rugosité de la surface. Encadrée des deux côtés — cf. le test.
    pub rugosite: f32,
    pub reflectance: f32,
    /// Couleur au plus profond, et près des berges. C'est cette DIFFÉRENCE qui
    /// donne la lecture de profondeur ; deux couleurs identiques rendraient un
    /// aplat, comme n'importe quel plan coloré.
    pub couleur_profonde: [f32; 3],
    pub couleur_bord: [f32; 3],
    /// Transparence de l'eau. Bas = opaque, haut = on voit le lit.
    pub clarte: f32,
    /// Écume au contact des berges et des rochers : sa couleur et sa largeur.
    pub ecume_rgb: [f32; 3],
    pub ecume_echelle: f32,
    /// Amplitude des vagues.
    ///
    /// 🚨 `bevy_water` DÉPLACE les sommets. Notre ruban n'a que 276 sommets sur
    /// 190 m — environ 10 m par triangle : une amplitude franche ne ferait pas
    /// des vagues, elle plierait la nappe en accordéon et la décollerait du lit.
    /// C'est la contrainte du maillage, pas un goût.
    pub amplitude_vagues: f32,
    /// Densité du motif de vagues du shader. Petit = grandes ondulations.
    pub echelle_vagues: f32,
    /// 1 à 4. Le nombre d'octaves du shader — 4 = le plus détaillé.
    pub qualite: u32,
}

impl Default for SurfaceEau {
    fn default() -> Self {
        Self {
            rugosite: 0.22,
            reflectance: 0.30,
            couleur_profonde: [0.02, 0.16, 0.28],
            couleur_bord: [0.22, 0.48, 0.55],
            clarte: 0.18,
            ecume_rgb: [0.92, 0.96, 1.0],
            ecume_echelle: 0.28,
            amplitude_vagues: 0.06,
            echelle_vagues: 0.35,
            qualite: 4,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct GenomeEau {
    #[serde(default)]
    surface: SurfaceEau,
}

/// Le matériau de la rivière, une fois pris en main.
///
/// Il porte l'avancement du courant plutôt qu'une horloge globale : la position
/// dans le cycle d'écoulement appartient à CETTE surface, et deux rivières
/// n'auraient aucune raison de couler en phase.
#[derive(Component)]
pub struct SurfaceRiviere {
    materiau: Handle<StandardWaterMaterial>,
    /// Décalage cumulé, en tuiles. Modulo 1 à chaque frame : sans ça le nombre
    /// grandit sans fin et perd sa précision flottante au bout de quelques
    /// heures, ce qui fait saccader le courant.
    avance_tuiles: f32,
}

/// Marque la scène de l'eau tant que son matériau n'a pas été repris.
#[derive(Component)]
pub struct EauAReprendre;

/// Lit le génome d'apparence. Absent = valeurs par défaut, et on le dit.
fn charger_genome() -> SurfaceEau {
    match forgia_core::def_io::read_def_str(GENOME_EAU) {
        Ok(s) => match toml::from_str::<GenomeEau>(&s) {
            Ok(g) => g.surface,
            Err(e) => {
                warn!("[expedition-eau] {GENOME_EAU} illisible ({e}) — apparence par defaut");
                SurfaceEau::default()
            }
        },
        Err(e) => {
            warn!("[expedition-eau] {GENOME_EAU} absent ({e}) — apparence par defaut");
            SurfaceEau::default()
        }
    }
}

/// Pose la rivière. Appelé une fois, à l'entrée en expédition.
pub fn setup_eau(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Name::new("ExpeditionRiviere"),
        ExpeditionMarker,
        EauAReprendre,
        SceneRoot(asset_server.load(MODELE_EAU)),
        // Le maillage est exporté en coordonnées monde Bevy — cf. le manifeste
        // de cellules, `coordinate_system = "bevy_y_up_meters"`. Aucun placement
        // à faire : le décaler serait le désaligner de son lit.
        Transform::IDENTITY,
        Visibility::Inherited,
    ));
}

/// Remplace le matériau exporté par celui de `bevy_water` dès que la scène est
/// peuplée.
///
/// Ici on REMPLACE au lieu d'ajuster : le matériau du kit est un
/// `StandardMaterial` opaque à une couleur, et ce qu'on veut — profondeur,
/// écume, vagues — n'existe pas dans ce type. On garde toutefois ce que
/// l'auteur de la carte a décidé qui reste pertinent : le **rendu translucide**
/// et la teinte de base viennent de son export, le reste du génome.
pub fn reprendre_materiau_eau(
    mut commands: Commands,
    materials: Res<Assets<StandardMaterial>>,
    mut eaux: ResMut<Assets<StandardWaterMaterial>>,
    q_attente: Query<Entity, With<EauAReprendre>>,
    q_enfants: Query<&Children>,
    q_mat: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    for racine in &q_attente {
        let mut repris = 0;
        let cfg = charger_genome();
        for descendant in q_enfants.iter_descendants(racine) {
            let Ok(mat_handle) = q_mat.get(descendant) else {
                continue;
            };
            // Attendre que le matériau d'origine soit chargé : sa transparence
            // est celle voulue par l'auteur de la carte, et un remplacement
            // fait trop tôt la perdrait sans qu'on sache pourquoi.
            let Some(base) = materials.get(&mat_handle.0) else {
                continue;
            };
            let handle = eaux.add(StandardWaterMaterial {
                base: StandardMaterial {
                    base_color: base.base_color,
                    perceptual_roughness: cfg.rugosite,
                    reflectance: cfg.reflectance,
                    alpha_mode: AlphaMode::Blend,
                    // Une rivière n'a pas d'envers : la voir depuis la berge
                    // opposée à travers sa propre nappe serait un défaut.
                    double_sided: false,
                    ..default()
                },
                extension: WaterMaterial {
                    amplitude: cfg.amplitude_vagues,
                    clarity: cfg.clarte,
                    deep_color: rgb(cfg.couleur_profonde),
                    shallow_color: rgb(cfg.couleur_bord),
                    edge_color: rgb(cfg.ecume_rgb),
                    edge_scale: cfg.ecume_echelle,
                    coord_scale: Vec2::splat(cfg.echelle_vagues),
                    quality: cfg.qualite.clamp(1, 4),
                    ..default()
                },
            });
            commands
                .entity(descendant)
                // Le `StandardMaterial` d'origine DOIT partir : deux matériaux
                // sur la même entité, ce sont deux passes de rendu et un
                // z-fighting sur la même surface.
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(handle.clone()));
            commands.entity(racine).insert(SurfaceRiviere {
                materiau: handle,
                avance_tuiles: 0.0,
            });
            repris += 1;
        }
        if repris > 0 {
            commands.entity(racine).remove::<EauAReprendre>();
            info!(
                "[expedition-eau] riviere posee — {repris} surface(s) sous bevy_water \
                 (amplitude {:.2}, clarte {:.2}, qualite {})",
                cfg.amplitude_vagues, cfg.clarte, cfg.qualite
            );
        }
    }
}

fn rgb([r, g, b]: [f32; 3]) -> Color {
    Color::srgb(r, g, b)
}

/// Fait couler l'eau, à la vitesse et dans le sens déclarés par la carte.
pub fn faire_couler(
    time: Res<Time>,
    expe: Option<Res<ActiveExpedition>>,
    mut eaux: ResMut<Assets<StandardWaterMaterial>>,
    mut q: Query<&mut SurfaceRiviere>,
) {
    let Some(expe) = expe else {
        return;
    };
    let eau = &expe.gameplay.eau;
    for mut surface in &mut q {
        // `V croissant = vers l'aval` : le manifeste le dit, on avance donc en
        // +V. Inverser le signe ferait remonter la rivière — un défaut qu'on ne
        // voit qu'en jeu, et seulement si on sait dans quel sens elle coule.
        surface.avance_tuiles =
            (surface.avance_tuiles + eau.courant_tuiles_par_s * time.delta_secs()).fract();
        let Some(mat) = eaux.get_mut(&surface.materiau) else {
            continue;
        };
        // `coord_offset` fait glisser le motif du shader ; c'est l'équivalent
        // de l'ancien `uv_transform`, mais côté extension — le `StandardMaterial`
        // de base n'a aucune texture à décaler ici.
        mat.extension.coord_offset = Vec2::new(0.0, surface.avance_tuiles * eau.tuile_m);
        // La direction des vagues suit le courant, donc l'axe V du maillage.
        mat.extension.wave_dir_a = Vec2::new(0.0, 1.0);
        mat.extension.wave_dir_b = Vec2::new(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genome() -> SurfaceEau {
        toml::from_str::<GenomeEau>(
            &std::fs::read_to_string("../../assets/genomes/expedition_eau.toml")
                .expect("le genome d'eau doit etre lisible depuis la crate"),
        )
        .expect("le genome d'eau doit parser")
        .surface
    }

    /// Le défaut qui a coûté cette session : un fichier produit par la chaîne
    /// d'assets que **personne ne charge**. L'eau et la porte du village étaient
    /// tous deux dans ce cas, sur le disque depuis le début, invisibles en jeu.
    ///
    /// Ce test compare les deux listes : ce que Blender a produit, et ce que le
    /// moteur cite. Il n'a pas de seuil à régler — toute divergence est un
    /// défaut.
    #[test]
    fn tout_gltf_produit_est_cite_quelque_part() {
        let dossier =
            std::path::Path::new("../../assets/models/environment/expedition/vallon_stream_cells");
        let manifeste = std::fs::read_to_string(dossier.join("vallon_stream_cells.toml"))
            .expect("le manifeste de cellules doit exister");
        // Les fichiers hors-grille sont cités dans le CODE, pas au manifeste :
        // ce ne sont pas des cellules. On lit donc les deux sources.
        let code_eau = include_str!("eau.rs");
        let code_plugin = include_str!("plugin.rs");

        let mut orphelins = Vec::new();
        for f in std::fs::read_dir(dossier).expect("le dossier de cellules doit exister") {
            let f = f.expect("entree de dossier lisible");
            let nom = f.file_name().to_string_lossy().to_string();
            if !nom.ends_with(".gltf") {
                continue;
            }
            let cite = manifeste.contains(&nom)
                || code_eau.contains(&nom)
                || code_plugin.contains(&nom);
            if !cite {
                orphelins.push(nom);
            }
        }
        orphelins.sort();
        assert!(
            orphelins.is_empty(),
            "{} fichier(s) produits par Blender que RIEN ne charge — ils sont sur \
             le disque et invisibles en jeu : {orphelins:?}",
            orphelins.len()
        );
    }

    /// La rugosité de l'eau est encadrée **des deux côtés**, et les deux bornes
    /// ont été payées en jeu :
    ///
    /// - au-dessus, la valeur du kit (0,92) donne du plastique mat ;
    /// - **en dessous, 0,08 donne un quasi-miroir** — chaque ondulation devient
    ///   une arête spéculaire dure sous la lumière rasante, et la rivière lit
    ///   comme du velours côtelé. C'est le défaut livré au 1ᵉʳ essai.
    ///
    /// Une borne unique aurait laissé passer le second. Un intervalle dit qu'il
    /// y a deux façons de se tromper, pas une.
    #[test]
    fn la_rugosite_de_l_eau_evite_le_plastique_et_le_miroir() {
        let g = genome();
        assert!(
            (0.15..0.35).contains(&g.rugosite),
            "rugosite {} hors de [0,15 ; 0,35] : au-dessus la riviere lit comme du \
             plastique mat (valeur exportee : 0,92) ; en dessous elle devient miroir \
             et ses vaguelettes se figent en filets durs",
            g.rugosite
        );
    }

    /// 🚨 La borne la plus dure de ce fichier, et elle vient du MAILLAGE.
    ///
    /// `bevy_water` déplace les sommets pour faire ses vagues. La rivière du
    /// Vallon n'a que **276 sommets pour 190 m de long** — de l'ordre de 10 m
    /// par triangle. Une amplitude franche ne produirait donc pas des vagues
    /// mais un pliage en accordéon, qui décollerait la nappe de son lit et
    /// laisserait voir le sol à travers.
    ///
    /// Cette borne ne se relève PAS au génome : elle se relève en subdivisant
    /// la rivière sous Blender. Le test le dit, pour que personne ne monte la
    /// valeur en croyant que c'est un réglage de goût.
    #[test]
    fn l_amplitude_des_vagues_tient_dans_ce_que_le_maillage_supporte() {
        let g = genome();
        // Mesuré sur `vallon_eau.gltf` : 276 sommets, emprise 42 × 190 m.
        const SOMMETS: f32 = 276.0;
        const LONGUEUR_M: f32 = 190.0;
        const LARGEUR_M: f32 = 42.0;
        // Côté moyen d'un quad, en supposant une grille régulière.
        let pas_m = (LONGUEUR_M * LARGEUR_M / SOMMETS).sqrt();
        // Une vague ne se lit que si elle est PETITE devant le pas du maillage
        // qui la porte — au-delà, on déplace des sommets isolés et la surface
        // se déchire au lieu d'onduler. Un dixième du pas est déjà généreux.
        let plafond = pas_m * 0.1;
        assert!(
            g.amplitude_vagues <= plafond,
            "amplitude_vagues {:.3} m alors que le maillage ne supporte que \
             {plafond:.3} m (pas moyen {pas_m:.1} m sur {SOMMETS:.0} sommets) — \
             au-dela la nappe se plie en accordeon et decolle de son lit. \
             Pour monter cette valeur il faut SUBDIVISER la riviere sous Blender, \
             pas editer le genome.",
            g.amplitude_vagues
        );
    }

    /// La profondeur ne se lit que par la DIFFÉRENCE entre les deux couleurs.
    /// Les rapprocher rendrait un aplat — précisément ce qu'était la version
    /// précédente, et ce qu'on cherche à quitter.
    #[test]
    fn les_deux_couleurs_de_l_eau_sont_distinctes() {
        let g = genome();
        let ecart: f32 = (0..3)
            .map(|i| (g.couleur_profonde[i] - g.couleur_bord[i]).abs())
            .sum();
        assert!(
            ecart > 0.2,
            "couleur_profonde {:?} et couleur_bord {:?} n'ecartent que de {ecart:.3} : \
             sans contraste la riviere rend un APLAT, et toute la lecture de \
             profondeur qu'apporte bevy_water est perdue",
            g.couleur_profonde,
            g.couleur_bord
        );
        assert!(
            (1..=4).contains(&g.qualite),
            "qualite {} hors de 1..4 — le shader n'a que 4 niveaux d'octaves",
            g.qualite
        );
        assert!(
            (0.0..=1.0).contains(&g.clarte),
            "clarte {} hors de [0 ; 1]",
            g.clarte
        );
    }
}
