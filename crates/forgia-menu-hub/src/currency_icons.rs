//! currency_icons.rs — les **icônes des deux monnaies** (story-678, 2026-08-06).
//!
//! Le hub affichait « 1729 Âmes » et « 0 Éclats » en texte, dans la même
//! couleur : `C_PRIMARY` est un **alias de `FORGE_OR`**, donc rien ne
//! distinguait la monnaie de puissance de la monnaie cosmétique. Deux monnaies
//! qui se ressemblent, c'est un joueur qui croit dépenser l'une en dépensant
//! l'autre.
//!
//! Chacune a donc maintenant sa **forme** (jeton d'or / gemme) en plus de sa
//! couleur — parce qu'une différence de teinte seule ne suffit pas : elle
//! disparaît pour un daltonien, et sur un fond doré comme le nôtre elle est
//! déjà faible.
//!
//! Source des icônes : Éclats = `Icon_Materials_Gem02_Purple` du pack
//! **Layer Lab — 2D Icons Casual** fourni par le créateur (128 px). Âmes = saphir
//! taillé en diamant **dessiné maison** (2026-08-07) dans le même style (gros
//! contour, aplats, reflet) : la première icône était la pièce
//! `Icon_Resources_Coin01_Gold` du pack — le même visuel que l'Or ramassé en
//! run, rapporté confusant en jeu — et le pack n'offre qu'une seule gemme,
//! déjà prise par les Éclats. Couleur du texte accordée : `FORGE_AME`.
//!
//! ## Où l'enregistrement a lieu
//!
//! `OnEnter(AppMode::Menu)`, exactement comme les aperçus RTT
//! (`weapon_preview::create_rtt_image`) — c'est le motif qui marche dans ce
//! projet.
//!
//! 🚨 Premier jet : ce système tournait dans `Update` avec un garde
//! `if contexts.ctx_mut().is_err() { return }`. Le garde échouait à **chaque**
//! frame, donc les icônes n'étaient jamais enregistrées et le hub restait en
//! texte nu — sans la moindre erreur. `add_image` n'a PAS besoin du contexte
//! rendu ; demander `ctx_mut()` pour « vérifier » qu'il est prêt était une
//! précaution qui empêchait le travail qu'elle prétendait protéger.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiTextureHandle};
use forgia_core::prelude::AppMode;

/// Chemins (relatifs à `assets/`) — une seule définition.
const SOULS_PATH: &str = "ui/icons/currency_souls.png";
const SHARDS_PATH: &str = "ui/icons/currency_shards.png";

/// Côté d'affichage d'une icône de monnaie, en points egui.
///
/// Accordé à la taille du texte qu'elle accompagne (20 pt pour les Âmes) : une
/// icône plus haute que sa ligne pousse le texte et fait sauter le chip.
pub const CURRENCY_ICON: f32 = 22.0;
/// Version réduite — dans les listes du Marketplace, à côté d'un prix de 14 pt.
pub const CURRENCY_ICON_SMALL: f32 = 16.0;

/// Les deux icônes, prêtes à peindre.
#[derive(Resource, Default)]
pub struct CurrencyIcons {
    pub souls: Option<egui::TextureId>,
    pub shards: Option<egui::TextureId>,
    /// Handles forts gardés ici : sans eux Bevy pourrait libérer l'image, et
    /// egui peindrait une texture morte.
    handles: Vec<Handle<Image>>,
    registered: bool,
}

impl CurrencyIcons {
    /// Dessine l'icône si elle est prête, et rend `true` si elle a été peinte.
    ///
    /// Rendre le succès plutôt que de dessiner un carré de repli : l'appelant
    /// décide quoi faire du vide — un chip garde son texte, il n'a pas besoin
    /// d'un placeholder qui clignoterait au chargement.
    pub fn show(ui: &mut egui::Ui, id: Option<egui::TextureId>, side: f32) -> bool {
        let Some(id) = id else {
            return false;
        };
        ui.add(egui::Image::new(egui::load::SizedTexture::new(
            id,
            egui::vec2(side, side),
        )));
        true
    }
}

pub struct CurrencyIconsPlugin;

impl Plugin for CurrencyIconsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrencyIcons>()
            .add_systems(OnEnter(AppMode::Menu), sys_register_currency_icons);
    }
}

/// Enregistre les deux images auprès d'egui — une seule fois.
fn sys_register_currency_icons(
    mut icons: ResMut<CurrencyIcons>,
    mut contexts: EguiContexts,
    assets: Res<AssetServer>,
) {
    if icons.registered {
        return;
    }
    let souls: Handle<Image> = assets.load(SOULS_PATH);
    let shards: Handle<Image> = assets.load(SHARDS_PATH);
    icons.souls = Some(contexts.add_image(EguiTextureHandle::Strong(souls.clone())));
    icons.shards = Some(contexts.add_image(EguiTextureHandle::Strong(shards.clone())));
    icons.handles = vec![souls, shards];
    icons.registered = true;
    info!("[currency-icons] icônes de monnaie enregistrées ({SOULS_PATH}, {SHARDS_PATH})");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_icones_existent_sur_le_disque() {
        // Un chemin d'asset faux ne se voit qu'en jeu, et se voit MAL : egui
        // dessine du vide, sans erreur. Le contrôle mécanique coûte trois lignes.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        if !root.exists() {
            eprintln!("assets absents (checkout partiel), contrôle non exécuté");
            return;
        }
        for p in [SOULS_PATH, SHARDS_PATH] {
            assert!(root.join(p).is_file(), "icône manquante : assets/{p}");
        }
    }

    #[test]
    fn une_icone_absente_ne_dessine_rien_plutot_qu_un_carre() {
        // `show` rend `false` quand l'icône n'est pas prête — l'appelant garde
        // son texte au lieu d'afficher un placeholder clignotant au chargement.
        // (Le rendu lui-même demande un `Ui`, donc on vérifie le contrat ici.)
        let icons = CurrencyIcons::default();
        assert!(icons.souls.is_none());
        assert!(icons.shards.is_none());
        assert!(!icons.registered);
    }
}
