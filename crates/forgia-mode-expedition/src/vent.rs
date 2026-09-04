//! vent.rs — faire onduler le feuillage et l'herbe du Vallon.
//!
//! # La contrainte, et ce qu'elle impose
//!
//! Le glTF ne transporte **aucun shader**. Tout ce que Blender pouvait faire
//! pour le vent, c'était cuire un MASQUE DE RAIDEUR — et il l'a fait, dans
//! l'alpha de `COLOR_0` (`14_ancrage.py`, étape 1 bis : 7 790 sommets souples
//! sur 70 261).
//!
//! # 🚨 Pourquoi l'alpha, alors que j'avais conclu l'inverse
//!
//! J'avais écrit qu'elle était inutilisable : « Bevy multiplie `base_color` par
//! `COLOR_0`, alpha comprise, donc y loger un masque rendrait le feuillage
//! translucide ». C'était faux, et la source le dit mot pour mot
//! (`bevy_pbr-0.18.1/src/render/pbr_functions.wgsl:100-103`) :
//!
//! ```text
//! if alpha_mode == ...ALPHA_MODE_OPAQUE {
//!     // NOTE: If rendering as opaque, alpha should be ignored so set to 1.0
//!     color.a = 1.0;
//! }
//! ```
//!
//! Les matériaux du kit sont opaques — un glTF sans `alphaMode` vaut OPAQUE.
//! Leur alpha n'est donc **jamais lue par le rendu**. Le canal était libre, il
//! était déjà exporté, et j'allais faire recuire un second jeu d'UV pour rien.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne trie pas les maillages. C'est le masque qui décide : un sommet à 0 ne
//! bouge jamais, donc troncs et rochers restent plantés même si leur matériau
//! porte l'extension. Trier côté moteur aurait voulu dire redécouvrir, à
//! l'exécution, ce que Blender savait déjà à la cuisson.

use bevy::asset::{Asset, Handle};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin, StandardMaterial};
use bevy::prelude::*;
use forgia_core::prelude::GameMode;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use serde::Deserialize;

const SHADER: &str = "shaders/expedition_vent.wgsl";
const GENOME: &str = "assets/genomes/expedition_vent.toml";

/// Le matériau du décor qui bouge : un `StandardMaterial` **augmenté**, pas
/// remplacé. Toute la lumière, les textures de `matiere.rs` et les couleurs de
/// sommet continuent de marcher — on n'ajoute qu'un déplacement de sommets.
pub type MateriauVent = ExtendedMaterial<StandardMaterial, VentExtension>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct VentExtension {
    /// `xyz` = direction du vent en plan (y inutilisé), `w` = débattement (m).
    #[uniform(100)]
    pub direction_force: Vec4,
    /// `x` rafale (rad/s), `y` frisson (rad/s), `z` longueur d'onde (m), `w` temps (s).
    #[uniform(100)]
    pub reglages: Vec4,
}

impl MaterialExtension for VentExtension {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// Les réglages du vent, en couche definition.
#[derive(Debug, Clone, Deserialize)]
pub struct VentConfig {
    /// Débattement de la pointe la plus souple, en mètres.
    ///
    /// 0,18 m : une touffe d'herbe de 40 cm qui bat de 18 cm se lit comme du
    /// vent ; au-delà elle a l'air d'être frappée. C'est un choix d'auteur
    /// assumé — aucune mesure ne le dérive, et il se règle à l'œil.
    pub amplitude_m: f32,
    /// Cap du vent, en degrés.
    pub cap_deg: f32,
    /// Période de la rafale, en secondes.
    pub rafale_s: f32,
    /// Période du frisson, en secondes.
    pub frisson_s: f32,
    /// Longueur d'onde spatiale, en mètres. C'est ELLE qui empêche toute la
    /// prairie de battre à l'unisson : sans décalage spatial, mille touffes
    /// pulsent comme un seul organisme.
    pub houle_m: f32,
}

impl Default for VentConfig {
    fn default() -> Self {
        Self {
            amplitude_m: 0.18,
            cap_deg: 35.0,
            rafale_s: 4.5,
            frisson_s: 1.3,
            houle_m: 14.0,
        }
    }
}

impl VentConfig {
    /// Lit le génome, ou rend les valeurs par défaut en le disant.
    #[must_use]
    pub fn load_or_default() -> Self {
        match forgia_core::def_io::read_def_str(GENOME) {
            Ok(s) => match toml::from_str::<Self>(&s) {
                Ok(c) => c,
                Err(e) => {
                    warn!("[vent] genome illisible ({e}) — valeurs par defaut");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Les deux vecteurs que le shader consomme. **Fonction pure** : c'est ici
    /// que vit tout ce qui peut être faux, et ça se teste sans moteur.
    #[must_use]
    pub fn uniformes(&self, temps_s: f32) -> (Vec4, Vec4) {
        let r = self.cap_deg.to_radians();
        (
            Vec4::new(r.cos(), 0.0, r.sin(), self.amplitude_m),
            Vec4::new(
                // Période → pulsation. Une période de 0 ferait une division par
                // zéro et un vent infiniment rapide : on la borne.
                std::f32::consts::TAU / self.rafale_s.max(0.05),
                std::f32::consts::TAU / self.frisson_s.max(0.05),
                self.houle_m.max(0.1),
                temps_s,
            ),
        )
    }
}

#[derive(Resource, Debug, Clone)]
pub struct VentState {
    pub config: VentConfig,
    pub materiaux: Vec<Handle<MateriauVent>>,
}

/// Les matières que le vent anime, par préfixe de nom glTF.
///
/// **Exactement la même liste que `14_ancrage.py`** — et c'est une duplication
/// qu'il faut savoir : Blender décide quels SOMMETS sont souples, le moteur
/// décide quels MATÉRIAUX portent le shader. Si les deux divergent, l'effet est
/// silencieux dans les deux sens (un matériau animé dont tous les sommets sont
/// rigides ne bouge pas ; un masque cuit sur un matériau non animé est ignoré).
/// Le capteur publie donc le compte des deux côtés.
const MATIERES_SOUPLES: [&str; 4] = ["leafs", "grass", "plant", "flower"];

pub struct ExpeditionVentPlugin;

impl Plugin for ExpeditionVentPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<MateriauVent>::default())
            .init_resource::<VentState>()
            .add_systems(
                Update,
                (habiller_le_feuillage, avancer_le_temps)
                    .chain()
                    // 🚨 APRÈS `matiere`, ET CE N'EST PAS UN CONFORT.
                    //
                    // `matiere::habiller_les_aplats` pose la texture de grain en
                    // MODIFIANT le `StandardMaterial` **en place**
                    // (`mats.get_mut`). Nous, on le CLONE dans un matériau
                    // étendu, et on retire le `MeshMaterial3d<StandardMaterial>`
                    // que ce système interroge.
                    //
                    // Sans cet ordre, deux courses silencieuses :
                    //   1. on clone la version NUE, et le feuillage perd son
                    //      grain définitivement — le clone ne relit jamais
                    //      l'asset d'origine ;
                    //   2. l'entité perd son composant avant que `matiere` ne
                    //      la voie, donc elle n'est jamais habillée et son
                    //      compteur `ignores` grimpe sans qu'on sache pourquoi.
                    //
                    // Aucune des deux ne lève d'erreur. La seule trace serait
                    // « le feuillage est plat », qui n'évoque pas un ordre de
                    // systèmes — d'où le compte publié par le capteur.
                    .after(crate::matiere::habiller_les_aplats)
                    .run_if(in_state(GameMode::Expedition)),
            );
    }
}

impl Default for VentState {
    fn default() -> Self {
        Self { config: VentConfig::load_or_default(), materiaux: Vec::new() }
    }
}

/// Remplace le matériau du feuillage par sa version qui ondule.
///
/// # Pourquoi c'est un système d'`Update` et pas un `OnEnter`
///
/// Le décor arrive par le STREAMING, cellule par cellule, sur plusieurs
/// secondes. Un passage unique à l'entrée du mode ne verrait que les premières
/// — et les suivantes resteraient figées, ce qui se lit comme « le vent ne
/// marche que par endroits » et se cherche très loin de sa cause.
/// On réagit donc à l'apparition (`Added`), comme `matiere.rs` juste à côté.
fn habiller_le_feuillage(
    mut commands: Commands,
    mut etat: ResMut<VentState>,
    standards: Res<Assets<StandardMaterial>>,
    mut etendus: ResMut<Assets<MateriauVent>>,
    q: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>, &bevy::gltf::GltfMaterialName),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
) {
    for (e, mat, nom) in &q {
        let base = nom.0.split('.').next().unwrap_or("");
        if !MATIERES_SOUPLES.iter().any(|p| base.starts_with(p)) {
            continue;
        }
        let Some(src) = standards.get(&mat.0) else { continue };
        let (dir, reg) = etat.config.uniformes(0.0);
        let h = etendus.add(MateriauVent {
            // Le `StandardMaterial` est REPRIS tel quel : la texture de grain
            // posée par `matiere.rs`, la couleur de base et les couleurs de
            // sommet continuent de marcher. On n'ajoute qu'un déplacement.
            base: src.clone(),
            extension: VentExtension { direction_force: dir, reglages: reg },
        });
        etat.materiaux.push(h.clone());
        commands
            .entity(e)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(h));
    }
}

/// Fait avancer l'horloge du vent dans chaque matériau.
///
/// Le temps est un uniforme et non un `globals.time` du shader : le vent doit
/// pouvoir s'arrêter avec le jeu (menu, pause) sans que le décor continue de
/// battre derrière l'interface.
fn avancer_le_temps(
    time: Res<Time>,
    etat: Res<VentState>,
    mut mats: ResMut<Assets<MateriauVent>>,
) {
    let (dir, mut reg) = etat.config.uniformes(time.elapsed_secs());
    for h in &etat.materiaux {
        if let Some(m) = mats.get_mut(h) {
            reg.w = time.elapsed_secs();
            m.extension.direction_force = dir;
            m.extension.reglages = reg;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_cap_devient_une_direction_unitaire_dans_le_plan() {
        // Un vent qui aurait une composante verticale ferait respirer les
        // plantes au lieu de les pousser.
        let (dir, _) = VentConfig::default().uniformes(0.0);
        assert!((dir.y).abs() < 1.0e-6, "le vent pousse a la verticale");
        assert!(
            (Vec2::new(dir.x, dir.z).length() - 1.0).abs() < 1.0e-5,
            "la direction n'est pas unitaire"
        );
    }

    #[test]
    fn l_amplitude_voyage_dans_le_w_de_la_direction() {
        let c = VentConfig { amplitude_m: 0.42, ..VentConfig::default() };
        assert!((c.uniformes(0.0).0.w - 0.42).abs() < 1.0e-6);
    }

    #[test]
    fn une_periode_nulle_ne_produit_pas_un_vent_infini() {
        // Un genome mal rempli ne doit pas rendre une pulsation infinie : le
        // decor se mettrait a vibrer a la frequence de l'image, ce qui se lit
        // comme un bug de rendu et se cherche tres loin de sa cause.
        let c = VentConfig { rafale_s: 0.0, frisson_s: 0.0, ..VentConfig::default() };
        let (_, reg) = c.uniformes(0.0);
        assert!(reg.x.is_finite() && reg.y.is_finite());
        assert!(reg.x < 200.0 && reg.y < 200.0, "pulsation {reg:?} demesuree");
    }

    #[test]
    fn le_temps_arrive_bien_au_shader() {
        // Sans lui, le vent est fige — et un decor fige se lit comme « le
        // shader ne marche pas », pas comme « l'horloge n'avance pas ».
        assert!((VentConfig::default().uniformes(12.5).1.w - 12.5).abs() < 1.0e-6);
    }

    #[test]
    fn la_houle_ne_peut_pas_etre_nulle() {
        // Longueur d'onde nulle = division par zero dans le shader, donc une
        // phase NaN et un feuillage qui disparait.
        let c = VentConfig { houle_m: 0.0, ..VentConfig::default() };
        assert!(c.uniformes(0.0).1.z > 0.0);
    }
}
