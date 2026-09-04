//! Le grain du sol du Hall — un `ExtendedMaterial` qui rend la haute fréquence.
//!
//! # Le problème, mesuré
//!
//! Le sol du Hall est **un** albédo 2048² cuit sur 300 × 300 m : **5 texels par
//! mètre**, contre 194 de médiane pour le reste de la carte (`resolutions_hall.json`).
//! C'est de très loin le point le plus faible du décor.
//!
//! Agrandir l'albédo ne sert à rien : à 8192² il ferait encore 20 texels/m pour
//! 268 Mo de VRAM. Ce qui manque n'est pas de la *couleur* — la splatmap Unity a
//! été correctement cuite — c'est de la **haute fréquence**. On multiplie donc
//! l'albédo par une texture de détail tuilée tous les quelques mètres.
//!
//! Le détail a une moyenne de 0,5 **exactement** (garanti par
//! `tools/assets/generer_detail_sol.py`, qui le vérifie sur le fichier écrit), d'où
//! le `× 2` côté shader : la couleur d'ensemble de la carte reste rigoureusement
//! inchangée, seul le grain apparaît.
//!
//! # Pourquoi un module et pas une crate
//!
//! Un seul consommateur — le Hall. `fine-grained-crates` demande une crate à partir
//! de deux ; en dessous, c'est un module dans la crate appelante.

use bevy::asset::Assets;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use forgia_core::prelude::GameMode;
use serde::Deserialize;

use crate::castle_ground::CastleGroundVisual;

const SHADER: &str = "shaders/hall_sol_detail.wgsl";
const TEXTURE: &str = "models/environment/castle/sources_sol/detail_sol_hall.png";
const GENOME: &str = "assets/genomes/castle_sol_detail.toml";
const SENSOR: &str = "forgia2_castle_sol.json";
/// Cadence de publication du capteur.
const CAPTEUR_PERIODE_S: f32 = 1.0;
/// Au-dela, un sol toujours sans grain n'est plus « la scene charge », c'est
/// un defaut. A 60 images/s cela fait cinq secondes — largement de quoi
/// laisser un `SceneRoot` arriver, meme sur un disque lent.
const TENTATIVES_AVANT_ALERTE: u32 = 300;

/// Le matériau du sol : un `StandardMaterial` **augmenté**, pas remplacé. L'albédo
/// cuit, la normale, la lumière et le brouillard continuent de fonctionner — on
/// n'ajoute qu'une multiplication par le grain.
pub type MateriauSolHall = ExtendedMaterial<StandardMaterial, SolDetailExtension>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct SolDetailExtension {
    /// `x` tuiles par unité d'UV · `y` force sur la couleur · `z` force sur la
    /// rugosité · `w` réservé.
    #[uniform(100)]
    pub reglages: Vec4,
    #[texture(101)]
    #[sampler(102)]
    pub detail: Option<Handle<Image>>,
}

impl MaterialExtension for SolDetailExtension {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// Les réglages, en couche definition — ils se jugent à l'œil, pas à la compilation.
#[derive(Debug, Clone, Deserialize)]
pub struct SolDetailConfig {
    /// Combien de fois le grain se répète sur l'UV du terrain.
    ///
    /// L'UV du sol Unity couvre 0..1 sur ses ~300 m. À 100, une tuile fait donc
    /// environ 3 m — assez fin pour rendre du grain, assez large pour qu'aucun
    /// motif ne se reconnaisse.
    pub tuiles_par_uv: f32,
    /// 0 = détail éteint, 1 = plein. Permet de couper sans recompiler.
    pub force: f32,
    /// Même grain sur la rugosité. Une surface parfaitement lisse sur 300 m se lit
    /// comme du plastique, même bien texturée.
    pub force_rugosite: f32,
}

impl Default for SolDetailConfig {
    fn default() -> Self {
        Self {
            tuiles_par_uv: 100.0,
            force: 0.85,
            force_rugosite: 0.35,
        }
    }
}

impl SolDetailConfig {
    fn charger() -> Self {
        match forgia_core::def_io::read_def_str(GENOME) {
            Ok(txt) => match toml::from_str::<Self>(&txt) {
                Ok(c) => c,
                Err(e) => {
                    warn!("[castle-sol] {GENOME} illisible ({e}) — réglages par défaut");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    fn en_vec4(&self) -> Vec4 {
        Vec4::new(self.tuiles_par_uv, self.force, self.force_rugosite, 0.0)
    }
}

/// Posé une fois sur la racine du sol : sans lui, le remplacement se rejouerait à
/// chaque frame et créerait un matériau par frame.
#[derive(Component)]
struct SolDetailPose;

/// Ce que le capteur publie. Il vit hors du système pour survivre à la sortie du
/// Hall : un capteur qui s'efface en sortant se lit comme « rien à signaler ».
#[derive(Resource, Default)]
struct SolDetailEtat {
    maillages_convertis: u32,
    tentatives: u32,
    reglages: Vec4,
    texture_prete: bool,
    publie_a: f32,
}

pub struct CastleSolDetailPlugin;

impl Plugin for CastleSolDetailPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<MateriauSolHall>::default())
            .init_resource::<SolDetailEtat>()
            .add_systems(
                Update,
                poser_le_grain.run_if(in_state(GameMode::CastleHub)),
            )
            // Le capteur, LUI, tourne toujours. Un capteur muet se lit comme
            // « rien a signaler » alors qu'il veut dire « je ne tourne pas » —
            // et il retient sa derniere valeur reelle au lieu de s'effacer en
            // sortant du Hall.
            .add_systems(Update, capteur_sol);
    }
}

/// Remplace le `StandardMaterial` du sol par sa version augmentée.
///
/// # Pourquoi chaque frame plutôt qu'un `OnEnter`
///
/// La scène du terrain arrive par `SceneRoot`, donc de façon **asynchrone** : au
/// moment de l'`OnEnter`, ses maillages n'existent pas encore. Un `OnEnter` ne
/// verrait rien et le sol garderait son matériau d'origine — sans une ligne de
/// journal pour le dire. Le marqueur `SolDetailPose` arrête le système dès que le
/// travail est fait.
fn poser_le_grain(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    q_sol: Query<Entity, (With<CastleGroundVisual>, Without<SolDetailPose>)>,
    q_enfants: Query<&Children>,
    q_materiau: Query<&MeshMaterial3d<StandardMaterial>>,
    standards: Res<Assets<StandardMaterial>>,
    mut etendus: ResMut<Assets<MateriauSolHall>>,
    mut etat: ResMut<SolDetailEtat>,
    mut detail: Local<Option<Handle<Image>>>,
    mut config: Local<Option<SolDetailConfig>>,
) {
    let Ok(racine) = q_sol.single() else {
        return;
    };
    etat.tentatives += 1;

    // La texture se charge une fois, en RÉPÉTITION : sans `Repeat`, le grain
    // s'étire au lieu de se tuiler et le sol prend un dégradé géant.
    let texture = detail
        .get_or_insert_with(|| {
            asset_server.load_with_settings(TEXTURE, |s: &mut ImageLoaderSettings| {
                s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                    address_mode_u: ImageAddressMode::Repeat,
                    address_mode_v: ImageAddressMode::Repeat,
                    ..ImageSamplerDescriptor::linear()
                });
            })
        })
        .clone();
    etat.texture_prete = asset_server.is_loaded_with_dependencies(&texture);

    // Lue UNE fois. Ce systeme tourne a chaque image tant que la scene n'est
    // pas arrivee : y laisser une lecture de fichier, c'est un acces disque par
    // image pendant tout le chargement.
    let cfg = config.get_or_insert_with(SolDetailConfig::charger).clone();
    etat.reglages = cfg.en_vec4();

    // Parcours explicite plutôt qu'un helper de hiérarchie : le contrat des
    // extensions de requête a bougé d'une version de Bevy à l'autre, une pile ne
    // bouge pas.
    let mut pile = vec![racine];
    let mut convertis = 0u32;
    while let Some(e) = pile.pop() {
        if let Ok(enfants) = q_enfants.get(e) {
            pile.extend(enfants.iter());
        }
        let Ok(actuel) = q_materiau.get(e) else {
            continue;
        };
        let Some(base) = standards.get(&actuel.0) else {
            continue;
        };
        let handle = etendus.add(MateriauSolHall {
            base: base.clone(),
            extension: SolDetailExtension {
                reglages: cfg.en_vec4(),
                detail: Some(texture.clone()),
            },
        });
        commands
            .entity(e)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(handle));
        convertis += 1;
    }

    if convertis == 0 {
        // La scène n'est pas encore là. On repassera — et si on ne repasse jamais,
        // le capteur le dira avec `maillages_convertis = 0`.
        return;
    }

    commands.entity(racine).insert(SolDetailPose);
    etat.maillages_convertis = convertis;
    info!(
        "[castle-sol] grain posé sur {convertis} maillage(s) — {:.0} tuiles/UV, \
         force {:.2}, rugosité {:.2} (après {} tentative(s))",
        cfg.tuiles_par_uv, cfg.force, cfg.force_rugosite, etat.tentatives
    );
}

/// Un capteur, parce qu'un sol qui reste sans grain ne se voit pas dans un journal.
fn capteur_sol(
    time: Res<Time>,
    mode: Res<State<GameMode>>,
    mut etat: ResMut<SolDetailEtat>,
) {
    if time.elapsed_secs() - etat.publie_a < CAPTEUR_PERIODE_S {
        return;
    }
    etat.publie_a = time.elapsed_secs();

    let au_hall = *mode.get() == GameMode::CastleHub;
    let (severity, next_step) = if etat.maillages_convertis > 0 {
        ("ok", String::new())
    } else if !au_hall {
        // Hors du Hall il n'y a rien a poser : le dire, ne pas alerter.
        ("info", String::new())
    } else if etat.tentatives > TENTATIVES_AVANT_ALERTE {
        (
            "warn",
            format!(
                "le sol n'a reçu AUCUN matériau augmenté après {} tentatives — \
                 il rend donc à 5 texels/m. Vérifier que la scène du terrain porte \
                 bien un MeshMaterial3d<StandardMaterial> sous CastleGroundVisual",
                etat.tentatives
            ),
        )
    } else {
        ("info", String::new())
    };

    let json = format!(
        r#"{{"id":"castle_sol","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"au_hall":{},"maillages_convertis":{},"tentatives":{},"texture_prete":{},"tuiles_par_uv":{:.1},"force":{:.2},"force_rugosite":{:.2}}}"#,
        etat.publie_a,
        au_hall,
        etat.maillages_convertis,
        etat.tentatives,
        etat.texture_prete,
        etat.reglages.x,
        etat.reglages.y,
        etat.reglages.z,
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR, json);
}
