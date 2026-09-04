//! matiere.rs — Habiller les 28 aplats du Vallon sans rouvrir Blender.
//!
//! # Le constat qui rend ce module court
//!
//! La carte porte 37 matériaux, dont **30 sans aucune texture** — l'héritage
//! direct du kit low-poly. Mais ce ne sont pas 30 problèmes : ce sont **6
//! familles** (pierre ×8, bois ×6, accents ×6, terre ×4, feuillage ×3, herbe),
//! et chaque matériau porte DÉJÀ sa teinte dans son `baseColorFactor` glTF.
//!
//! La palette existe, choisie sous Blender. Il ne manque que le grain. Six
//! cartes de gris de 256², 71 Ko au total, couvrent donc 28 matériaux — sans
//! refaire une seule UV et sans toucher aux couleurs de l'auteur.
//!
//! # Pourquoi `base_color_texture` suffit, et pas de triplanaire
//!
//! Bevy multiplie `base_color_texture` par `base_color`. Poser une carte de gris
//! dans la première **module** donc la seconde, sans la remplacer. Aucun shader,
//! aucun `ExtendedMaterial`.
//!
//! Ce raccourci n'était pas acquis : il fallait que les maillages aient des UV
//! exploitables. Mesuré avant d'écrire — les 553 primitives concernées en ont, et
//! leurs UV s'étendent de **−22 à +21 en U**, **−63 à +16 en V**. Elles sont
//! donc déjà tuilées à l'échelle du monde. Un atlas ponctuel aurait imposé le
//! triplanaire ; ici il serait du travail en pure perte.
//!
//! # 🚨 Les trois pièges, tous mesurés avant d'être contournés
//!
//! 1. **`ClampToEdge` par défaut.** Bevy 0.18 charge toute image ainsi
//!    (`bevy_image/src/image.rs:668`). Une texture parfaitement sans couture ne
//!    se répète PAS — on obtient un aplat, et on cherche le défaut dans l'art.
//! 2. **L'espace de couleur.** `base_color_texture` est décodée sRGB → linéaire.
//!    Une carte centrée sur l'octet 127 vaudrait **0,21 en linéaire** : le décor
//!    assombri de 79 % par ce qui devait le détailler. Les cartes sont donc
//!    pré-encodées, et leur sommet est à 1,0.
//! 3. **Aucun mipmap.** `mip_level_count: 1` est codé en dur en 0.18. À
//!    l'échelle 1, le motif se répéterait 30 à 75 fois par pièce et grouillerait.
//!    D'où `echelle_uv` au génome, et sa valeur basse.

use bevy::asset::RenderAssetUsages;
use bevy::gltf::GltfMaterialName;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use serde::Deserialize;

const GENOME: &str = "assets/genomes/expedition_matiere.toml";

#[derive(Debug, Clone, Deserialize)]
pub struct FamilleDef {
    pub id: String,
    pub prefixes: Vec<String>,
    pub texture: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MatiereConfig {
    #[serde(default = "vrai")]
    pub actif: bool,
    #[serde(default = "echelle_par_defaut")]
    pub echelle_uv: f32,
    #[serde(default)]
    pub familles: Vec<FamilleDef>,
}

fn vrai() -> bool {
    true
}

fn echelle_par_defaut() -> f32 {
    0.25
}

impl Default for MatiereConfig {
    fn default() -> Self {
        Self {
            actif: false,
            echelle_uv: echelle_par_defaut(),
            familles: Vec::new(),
        }
    }
}

impl MatiereConfig {
    /// La famille d'un matériau, par préfixe de nom.
    ///
    /// Le préfixe est ce qui fait tenir huit variantes de pierre
    /// (`stone_v0..v3`, `stoneDark_v0..v3`) sur une seule déclaration.
    #[must_use]
    pub fn famille_de(&self, nom: &str) -> Option<&FamilleDef> {
        self.familles
            .iter()
            .find(|f| f.prefixes.iter().any(|p| nom.starts_with(p.as_str())))
    }
}

/// Les cartes chargées, prêtes à poser. Absente = rien à faire.
#[derive(Resource)]
pub struct Matieres {
    cfg: MatiereConfig,
    /// (id de famille, carte). Chargées UNE fois, partagées par tous les
    /// matériaux de la famille — c'est ce qui garde le coût à six textures.
    cartes: Vec<(String, Handle<Image>)>,
    /// Matériaux déjà habillés, pour ne pas les recloner à chaque cellule qui
    /// arrive. Le streaming en ouvre 48 : sans ça, autant de clones par famille.
    faits: Vec<AssetId<StandardMaterial>>,
    pub habilles: u32,
    pub ignores: u32,
}

pub struct ExpeditionMatierePlugin;

impl Plugin for ExpeditionMatierePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(forgia_core::prelude::GameMode::Expedition),
            charger_les_matieres,
        )
        .add_systems(
            Update,
            (habiller_les_aplats, capteur_matiere)
                .chain()
                .run_if(in_state(forgia_core::prelude::GameMode::Expedition))
                .run_if(resource_exists::<Matieres>),
        );
    }
}

fn charger_les_matieres(mut commands: Commands, assets: Res<AssetServer>) {
    let cfg: MatiereConfig = match forgia_core::def_io::read_def_str(GENOME) {
        Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
            warn!("[matiere] {GENOME} illisible ({e}) — decor laisse en aplats");
            MatiereConfig::default()
        }),
        Err(e) => {
            warn!("[matiere] {GENOME} absent ({e}) — decor laisse en aplats");
            MatiereConfig::default()
        }
    };
    if !cfg.actif || cfg.familles.is_empty() {
        info!("[matiere] desactivee au genome — le decor garde ses aplats");
        return;
    }

    // 🚨 LE SAMPLER. Sans `Repeat`, une texture tuilable rend le bord de
    // l'image partout : un aplat uniforme. C'est le défaut le plus traître du
    // lot, parce qu'il ressemble à « la texture est ratée » alors que la
    // texture est parfaite. Bevy 0.18 met `ClampToEdge` par défaut.
    let repete = |s: &mut ImageLoaderSettings| {
        let mut d = ImageSamplerDescriptor::linear();
        d.address_mode_u = ImageAddressMode::Repeat;
        d.address_mode_v = ImageAddressMode::Repeat;
        d.address_mode_w = ImageAddressMode::Repeat;
        s.sampler = ImageSampler::Descriptor(d);
        // Ces cartes ne servent qu'au GPU : pas de copie côté processeur.
        s.asset_usage = RenderAssetUsages::RENDER_WORLD;
    };

    let cartes = cfg
        .familles
        .iter()
        .map(|f| (f.id.clone(), assets.load_with_settings(f.texture.clone(), repete)))
        .collect::<Vec<_>>();

    info!(
        "[matiere] {} famille(s) chargee(s), echelle UV {:.2}",
        cartes.len(),
        cfg.echelle_uv
    );
    commands.insert_resource(Matieres {
        cfg,
        cartes,
        faits: Vec::new(),
        habilles: 0,
        ignores: 0,
    });
}

/// Pose la carte de sa famille sur chaque matériau nu qui arrive.
///
/// Les cellules arrivent par le streaming, donc ce système tourne en continu et
/// traite ce qui vient. Il ne touche QU'AUX matériaux sans texture : un
/// matériau déjà habillé sous Blender (`sol_terrain`, `sol_falaise`…) garde le
/// sien — écraser le travail de l'auteur serait le pire service à lui rendre.
pub fn habiller_les_aplats(
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut etat: ResMut<Matieres>,
    q_neufs: Query<(&GltfMaterialName, &MeshMaterial3d<StandardMaterial>), Added<GltfMaterialName>>,
) {
    let echelle = etat.cfg.echelle_uv.max(0.01);
    for (nom, handle) in &q_neufs {
        let id = handle.0.id();
        if etat.faits.contains(&id) {
            continue;
        }
        let Some(famille) = etat.cfg.famille_de(nom) else {
            etat.ignores = etat.ignores.saturating_add(1);
            continue;
        };
        let Some(carte) = etat
            .cartes
            .iter()
            .find(|(fid, _)| fid == &famille.id)
            .map(|(_, h)| h.clone())
        else {
            continue;
        };
        let Some(mat) = mats.get_mut(&handle.0) else {
            continue;
        };
        // Un matériau qui a DÉJÀ sa texture est laissé tel quel.
        if mat.base_color_texture.is_some() {
            etat.faits.push(id);
            etat.ignores = etat.ignores.saturating_add(1);
            continue;
        }
        mat.base_color_texture = Some(carte);
        // L'échelle rend le grain lisible : les UV du Vallon portent déjà des
        // dizaines de répétitions, les laisser telles quelles ferait grouiller.
        mat.uv_transform = bevy::math::Affine2::from_scale(Vec2::splat(echelle));
        etat.faits.push(id);
        etat.habilles = etat.habilles.saturating_add(1);
    }
}

/// Ce que la matière a réellement fait, lisible pendant que le jeu tourne.
///
/// # Pourquoi ce système existe
///
/// La première version de ce module n'écrivait qu'une ligne de log. Or
/// `forgia2_run.log` ne se vide jamais tant que le jeu tourne — le processus
/// panique à la fermeture — donc ce diagnostic était indisponible **exactement
/// quand on en avait besoin**. C'est une leçon déjà payée sur l'animation, et je
/// l'ai répétée le lendemain.
///
/// # Le seuil, et pourquoi il est temporel
///
/// « 0 matériau habillé » est normal pendant les premières secondes : les
/// cellules arrivent par le streaming. Ce n'est un défaut qu'une fois passé le
/// délai de chargement. Sans cette patience, l'alerte crierait à chaque entrée
/// en Expédition et on cesserait de la lire — le pire sort d'un capteur.
fn capteur_matiere(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut depuis_entree: Local<f32>,
    etat: Res<Matieres>,
) {
    const PERIODE_S: f32 = 1.0;
    /// Au-delà, une carte encore vierge n'est plus « en cours de chargement ».
    /// 48 cellules de 40 m à 140 m de rayon : tout ce qui doit arriver est
    /// arrivé bien avant.
    const PATIENCE_S: f32 = 12.0;

    *depuis_entree += time.delta_secs();
    *accum += time.delta_secs();
    if *accum < PERIODE_S {
        return;
    }
    *accum = 0.0;

    let (severity, next_step) = if !etat.cfg.actif {
        (
            "info",
            "matiere desactivee au genome — le decor garde ses aplats. Ce n'est pas \
             un feu vert, c'est un choix."
                .to_string(),
        )
    } else if etat.habilles == 0 && *depuis_entree > PATIENCE_S {
        // 🚨 « Zéro mesuré n'est pas vert, c'est aveugle. » Trois causes, et le
        // message les nomme pour qu'on ne les cherche pas dans l'ordre inverse.
        (
            "warn",
            format!(
                "MATIERE_INERTE : 0 materiau habille apres {:.0} s, alors que {} \
                 materiau(x) ont ete vus et ecartes. Trois causes, dans cet ordre : \
                 (1) les noms de materiaux du glTF ne commencent par aucun prefixe du \
                 genome ; (2) les cellules n'arrivent pas ; (3) les six cartes ne se \
                 chargent pas — verifier assets/textures/detail/.",
                depuis_entree.min(999.0),
                etat.ignores
            ),
        )
    } else {
        ("ok", String::new())
    };

    let json = format!(
        r#"{{"id":"matiere","severity":"{severity}","next_step":"{}","timestamp_secs":{:.1},"actif":{},"familles":{},"echelle_uv":{:.3},"materiaux_habilles":{},"materiaux_ecartes":{}}}"#,
        next_step.replace('"', "'"),
        time.elapsed_secs(),
        etat.cfg.actif,
        etat.cartes.len(),
        etat.cfg.echelle_uv,
        etat.habilles,
        etat.ignores,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia2_matiere.json", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> MatiereConfig {
        toml::from_str(
            &std::fs::read_to_string("../../assets/genomes/expedition_matiere.toml")
                .expect("le genome de matiere doit etre lisible"),
        )
        .expect("le genome de matiere doit parser")
    }

    /// Les 28 matériaux nus mesurés dans les cellules du Vallon. Si le génome
    /// n'en couvre pas un, il reste en plastique et **rien ne le dit** — c'est
    /// exactement le genre d'oubli qu'on ne voit qu'en jeu, sur un rocher.
    #[test]
    fn les_six_familles_couvrent_les_28_aplats() {
        const NUS: [&str; 28] = [
            "stone_v0", "stone_v1", "stone_v2", "stone_v3", "stoneDark_v0", "stoneDark_v1",
            "stoneDark_v2", "stoneDark_v3", "wood", "woodBark", "woodBarkDark", "woodBirch",
            "woodDark", "woodInner", "dirt_v0", "dirt_v1", "dirt_v2", "dirt_v3", "leafsDark",
            "leafsFall", "leafsGreen", "grass", "colorPurple", "colorRed", "colorRedDark",
            "colorTan", "colorWhite", "colorYellow",
        ];
        let c = cfg();
        let orphelins: Vec<&str> = NUS
            .iter()
            .copied()
            .filter(|m| c.famille_de(m).is_none())
            .collect();
        assert!(
            orphelins.is_empty(),
            "{} materiau(x) du Vallon sans famille — ils resteront en aplat : {orphelins:?}",
            orphelins.len()
        );
    }

    /// 🚨 Le contrôle inverse, et il compte autant : un préfixe trop court
    /// happerait des matériaux qui ne lui appartiennent pas. `color` ne doit pas
    /// prendre `colorPurple` ET une future `colorimetrie_sol`.
    #[test]
    fn aucune_famille_ne_happe_les_materiaux_deja_textures() {
        // Ceux que l'auteur a texturés sous Blender : ils doivent rester intacts.
        const TEXTURES: [&str; 6] = [
            "sol_terrain",
            "sol_berge",
            "sol_chemin",
            "sol_falaise",
            "sol_falaise_haute",
            "hexagons_medieval",
        ];
        let c = cfg();
        for m in TEXTURES {
            assert!(
                c.famille_de(m).is_none(),
                "« {m} » est deja texture sous Blender et serait happe par la famille \
                 « {} » — on ecraserait le travail de l'auteur",
                c.famille_de(m).map(|f| f.id.as_str()).unwrap_or("?")
            );
        }
        // Et l'eau, qui a son propre materiau depuis bevy_water.
        assert!(c.famille_de("eau_riviere").is_none(), "l'eau ne doit pas etre happee");
    }

    /// Les fichiers cités doivent exister. Un chemin faux se lit comme « la
    /// texture ne s'affiche pas », pas comme « le fichier n'est pas là ».
    #[test]
    fn les_six_cartes_existent_sur_le_disque() {
        for f in &cfg().familles {
            let p = std::path::Path::new("../../assets").join(&f.texture);
            assert!(p.exists(), "famille « {} » : {} introuvable", f.id, f.texture);
        }
    }

    /// L'échelle doit rester basse : les UV du Vallon portent déjà des dizaines
    /// de répétitions, et Bevy 0.18 ne génère AUCUN mipmap. À l'échelle 1 le
    /// grain grouillerait.
    #[test]
    fn l_echelle_uv_tient_compte_des_uv_deja_tuilees() {
        let c = cfg();
        assert!(
            c.echelle_uv > 0.0 && c.echelle_uv <= 0.5,
            "echelle_uv {} : au-dela de 0,5 le motif se repete des dizaines de fois \
             par piece et scintille — Bevy 0.18 n'a pas de mipmap pour l'amortir",
            c.echelle_uv
        );
    }
}
