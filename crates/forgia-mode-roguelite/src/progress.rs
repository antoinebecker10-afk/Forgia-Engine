//! progress.rs — le NIVEAU du joueur, dérivé de ses choix (story-680, cran 1).
//!
//! ## Ce qui était cassé
//!
//! Ce module tenait un niveau, de l'XP et des points de talent. Aucun des trois
//! ne servait :
//!
//! - l'XP valait `40 + durée de la run en secondes` — **aucun terme de
//!   performance**. Mourir au round 1 ou nettoyer 10 rounds ne changeait que le
//!   chrono, et la Défaite payait exactement comme la Victoire ;
//! - pire, ça récompensait la **lenteur**, en contradiction directe avec la
//!   boucle de rounds (story-677) qui demande de nettoyer dans un budget ;
//! - les points de talent s'accumulaient et **rien ne les dépensait** — leur
//!   unique consommateur du workspace les *affichait* ;
//! - et l'écran promettait « Choisis ton style — Feu · Givre · Éclair · Poison »
//!   alors qu'il n'existait ni arbre, ni style, ni combo.
//!
//! Un système vide serait neutre. Un système qui **annonce** ce qu'il ne fait
//! pas est une déception programmée.
//!
//! ## Le modèle retenu : Gunfire Reborn
//!
//! *« Le système de niveau tourne autour de la dépense d'une monnaie en talents
//! plutôt que de la collecte d'expérience ; un talent 2/5 donne 2 niveaux. »*
//!
//! Le niveau **est** la somme des rangs achetés à l'Enclume. Il ne peut donc
//! plus être creux, par construction : chaque niveau correspond à une décision
//! prise et à un effet réel en jeu. Il n'y a plus d'XP, plus de courbe, plus de
//! fichier de sauvegarde à part — la méta-boutique persiste déjà tout.

use bevy::prelude::*;

use crate::meta_shop::{MetaShopCatalogue, MetaShopSave};

/// Niveau du joueur — **dérivé**, jamais accumulé.
///
/// Ressource de lecture pour l'UI : elle évite que chaque écran recalcule la
/// somme des rangs de son côté (et finisse par en donner une version différente,
/// comme le « SALLE 5 / 4 » de story-678).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PlayerProgress {
    /// Niveau = 1 + rangs achetés + déblocages. Toujours ≥ 1.
    pub level: u32,
    /// Rangs encore achetables. « Il te reste N choses à débloquer » est une
    /// information utile, contrairement à « N points en attente » quand rien ne
    /// les dépense.
    pub ranks_remaining: u32,
}

impl PlayerProgress {
    /// Recalcule depuis la source de vérité. PUR — testable.
    pub fn derive(save: &MetaShopSave, cat: &MetaShopCatalogue) -> Self {
        Self {
            level: save.player_level(cat),
            ranks_remaining: save.ranks_remaining(cat),
        }
    }

    /// Fraction \[0,1\] de la progression méta totale — remplace l'ex-barre d'XP.
    ///
    /// Elle mesure quelque chose de VRAI : la part de l'Enclume déjà achetée.
    /// L'ancienne barre mesurait le temps passé en jeu.
    pub fn completion(&self) -> f32 {
        let bought = self.level.saturating_sub(1);
        let total = bought + self.ranks_remaining;
        if total == 0 {
            return 1.0;
        }
        (bought as f32 / total as f32).clamp(0.0, 1.0)
    }
}

/// Synchronise le niveau quand la sauvegarde méta change. Pas de système par
/// frame : `Changed` suffit, et c'est le seul moment où le niveau peut bouger.
fn sys_sync_progress(
    save: Res<MetaShopSave>,
    cat: Res<MetaShopCatalogue>,
    mut progress: ResMut<PlayerProgress>,
) {
    if !save.is_changed() && !cat.is_changed() && progress.level != 0 {
        return;
    }
    let next = PlayerProgress::derive(&save, &cat);
    if next != *progress {
        info!(
            "[progress] niveau {} ({} rang(s) restant(s), {:.0} % de l'Enclume)",
            next.level,
            next.ranks_remaining,
            next.completion() * 100.0
        );
        *progress = next;
    }
}

pub struct PlayerProgressPlugin;

impl Plugin for PlayerProgressPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerProgress>()
            .add_systems(Update, sys_sync_progress);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat() -> MetaShopCatalogue {
        MetaShopCatalogue::default()
    }

    #[test]
    fn a_fresh_player_is_level_one_not_zero() {
        let p = PlayerProgress::derive(&MetaShopSave::default(), &cat());
        assert_eq!(p.level, 1, "personne ne commence « niveau 0 »");
        assert!(p.ranks_remaining > 0, "il doit rester des choses à acheter");
        assert_eq!(p.completion(), 0.0);
    }

    /// LE point du cran 1 : le niveau est la somme des choix, pas du temps passé.
    #[test]
    fn the_level_is_exactly_the_number_of_ranks_bought() {
        let c = cat();
        let mut save = MetaShopSave::default();
        let base = PlayerProgress::derive(&save, &c).level;
        save.ranks.insert("max_hp".into(), 3);
        save.ranks.insert("damage".into(), 2);
        let p = PlayerProgress::derive(&save, &c);
        assert_eq!(p.level, base + 5, "3 + 2 rangs = 5 niveaux");
    }

    /// Un niveau ne s'obtient plus en laissant tourner le jeu : sans achat, il
    /// ne bouge pas, quelle que soit la durée de jeu.
    #[test]
    fn time_played_grants_nothing_anymore() {
        let c = cat();
        let mut save = MetaShopSave::default();
        save.runs_played = 500;
        save.souls_total = 99_999;
        assert_eq!(
            PlayerProgress::derive(&save, &c).level,
            PlayerProgress::derive(&MetaShopSave::default(), &c).level,
            "500 runs sans rien acheter = même niveau"
        );
    }

    /// La barre mesure la part de l'Enclume achetée — quelque chose de vrai.
    #[test]
    fn completion_reaches_one_only_when_everything_is_bought() {
        let c = cat();
        let mut save = MetaShopSave::default();
        for u in &c.upgrades {
            save.ranks.insert(u.id.clone(), u.max_rank());
        }
        let p = PlayerProgress::derive(&save, &c);
        assert_eq!(p.ranks_remaining, 0);
        assert!((p.completion() - 1.0).abs() < 1e-5);
    }
}
