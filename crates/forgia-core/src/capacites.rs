//! `Capacites` — ce qu'une zone SAIT faire, déclaré une fois, lu partout.
//!
//! # Le défaut que ce module supprime
//!
//! Jusqu'ici, chaque crate partagée portait sa propre liste de modes :
//!
//! ```ignore
//! matches!(mode, GameMode::Fps | GameMode::Roguelite)   // × 12 endroits
//! ```
//!
//! Une liste tenue à la main ne casse jamais quand on l'oublie — elle rend une
//! capacité **invisible**, sans erreur, sans avertissement, sans capteur. Le
//! dépôt a payé ce défaut **deux fois pour la même ligne** :
//!
//! | Date | Zone ajoutée | Ce qui était muet |
//! |---|---|---|
//! | 2026-07-20 | Roguelite | flash de dégâts + vignette bas-PV (le mode shippé) |
//! | 2026-08-18 | Expédition | flash, arc de direction, killfeed — 4 mois plus tard |
//!
//! Mesuré le 2026-08-18 avant correction : **6 prédicats multi-mode sur 12** ne
//! nommaient pas l'Expédition, alors qu'elle tire et encaisse depuis le 14/08.
//!
//! # Ce que la forme garantit
//!
//! La table est un `match` **exhaustif sans joker**. Ajouter une variante à
//! `GameMode` ne compile plus tant que ses capacités ne sont pas déclarées :
//! l'oubli devient une **erreur de compilation** au lieu d'un trou silencieux.
//! C'est la seule garantie qui tienne sans discipline.
//!
//! # Les quatre capacités, et pourquoi elles s'emboîtent
//!
//! Elles forment une chaîne d'inclusion, vérifiée par un test :
//!
//! ```text
//! combat  ⊇  retour_de_combat  ⊇  hud_generique  ⊇  vagues
//! ```
//!
//! - **combat** — le tir, les munitions et le changement d'arme tournent.
//!   `ArenaTest` en fait partie : c'est un banc de blockout où l'on tire pour
//!   éprouver la forme, sans rien afficher.
//! - **retour_de_combat** — ce qu'un combat DOIT montrer : flash d'écran, arc de
//!   direction des dégâts, killfeed, jauge de munitions. Un mode qui tire sans
//!   ça n'est pas « épuré », il est illisible.
//! - **hud_generique** — la zone n'a **pas** de HUD à elle : elle prend la barre
//!   de vie et le bandeau d'armes partagés. Le Roguelite est exclu **parce
//!   qu'il a les siens** (`hud::draw_weapon_slots`, carte vitals unifiée) — pas
//!   parce qu'il en manque.
//! - **vagues** — le compteur de vagues de l'arène Fps.
//!
//! # Ce que ce module NE fait PAS
//!
//! Il ne gate ni la physique, ni les caméras, ni le rendu, ni l'observabilité :
//! ces axes ont leurs propres portes et leurs propres raisons. Il ne dit pas non
//! plus si une capacité est *bien* implémentée dans une zone — seulement si elle
//! y est **attendue**.

use crate::states::{AppMode, GameMode};
use bevy::prelude::*;

/// Ce qu'une zone sait faire. Voir l'en-tête du module pour chaque champ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capacites {
    /// Le tir, les munitions et le changement d'arme tournent.
    pub combat: bool,
    /// Flash d'écran, arc de direction, killfeed, jauge de munitions.
    pub retour_de_combat: bool,
    /// La zone n'a pas de HUD à elle : barre de vie + bandeau d'armes partagés.
    pub hud_generique: bool,
    /// Compteur de vagues.
    pub vagues: bool,
}

impl Capacites {
    const RIEN: Self = Self {
        combat: false,
        retour_de_combat: false,
        hud_generique: false,
        vagues: false,
    };
}

/// La table. **Exhaustive et sans joker** : une nouvelle zone ne compile pas
/// tant qu'on n'a pas dit ce qu'elle sait faire.
///
/// Les valeurs ci-dessous reproduisent **exactement** les prédicats qui étaient
/// dispersés dans les crates partagées au 2026-08-18, Expédition incluse. Ce
/// module ne change donc aucun comportement : il rassemble une vérité qui
/// existait déjà en douze exemplaires.
#[must_use]
pub const fn capacites(mode: &GameMode) -> Capacites {
    match mode {
        // Aucun mode choisi : on est au menu, rien de gameplay ne tourne.
        GameMode::None => Capacites::RIEN,

        // L'arène FPS d'origine — la seule zone à compteur de vagues, et la
        // référence dont les autres ont hérité leurs portes.
        GameMode::Fps => Capacites {
            combat: true,
            retour_de_combat: true,
            hud_generique: true,
            vagues: true,
        },

        // L'Abîme. Combat complet, mais il dessine SON HUD (carte vitals,
        // slots d'armes aux noms lore) — d'où `hud_generique: false`.
        GameMode::Roguelite => Capacites {
            combat: true,
            retour_de_combat: true,
            hud_generique: false,
            vagues: false,
        },

        // L'Expédition. Passée à la 3ᵉ personne le 2026-08-14 ; elle tire et
        // encaisse, et n'a pas de HUD à elle.
        GameMode::Expedition => Capacites {
            combat: true,
            retour_de_combat: true,
            hud_generique: true,
            vagues: false,
        },

        // Banc de blockout : on tire pour éprouver la forme d'une salle. Aucun
        // affichage — c'est voulu, la géométrie doit se juger nue.
        GameMode::ArenaTest => Capacites {
            combat: true,
            ..Capacites::RIEN
        },

        // Zones sans combat : le RPG a sa propre pile, le Hall est neutre par
        // conception (GDD), la démo Cyber City n'a aucun gameplay.
        GameMode::Rpg | GameMode::CastleHub | GameMode::CyberCity => Capacites::RIEN,
    }
}

// ─── Run-conditions ──────────────────────────────────────────────────────────
//
// Elles vérifient TOUTES `AppMode::InGame` : une capacité de gameplay n'a aucun
// sens au menu ou en pause, et chaque site d'appel le redemandait à la main.

/// La zone courante tire-t-elle ?
pub fn a_du_combat(app: Res<State<AppMode>>, mode: Res<State<GameMode>>) -> bool {
    *app.get() == AppMode::InGame && capacites(mode.get()).combat
}

/// Faut-il afficher le retour de combat (flash, direction, killfeed, munitions) ?
pub fn affiche_le_retour_de_combat(app: Res<State<AppMode>>, mode: Res<State<GameMode>>) -> bool {
    *app.get() == AppMode::InGame && capacites(mode.get()).retour_de_combat
}

/// La zone utilise-t-elle le HUD générique (barre de vie, bandeau d'armes) ?
pub fn utilise_le_hud_generique(app: Res<State<AppMode>>, mode: Res<State<GameMode>>) -> bool {
    *app.get() == AppMode::InGame && capacites(mode.get()).hud_generique
}

/// La zone affiche-t-elle un compteur de vagues ?
pub fn affiche_les_vagues(app: Res<State<AppMode>>, mode: Res<State<GameMode>>) -> bool {
    *app.get() == AppMode::InGame && capacites(mode.get()).vagues
}

#[cfg(test)]
mod tests {
    use super::{capacites, Capacites};
    use crate::states::GameMode;

    /// Toutes les zones du jeu. Si `GameMode` grandit, cette liste doit grandir
    /// aussi — et le `match` de `capacites` refusera de compiler entre-temps.
    const TOUTES: [GameMode; 8] = [
        GameMode::None,
        GameMode::Fps,
        GameMode::Rpg,
        GameMode::Roguelite,
        GameMode::CyberCity,
        GameMode::CastleHub,
        GameMode::ArenaTest,
        GameMode::Expedition,
    ];

    #[test]
    fn les_capacites_s_emboitent() {
        // combat ⊇ retour_de_combat ⊇ hud_generique ⊇ vagues.
        //
        // Ce n'est pas de l'élégance : afficher un killfeed dans une zone qui ne
        // tire pas, ou un compteur de vagues sans HUD, est incohérent par
        // construction. Une nouvelle zone mal déclarée casse ICI, pas en jeu.
        for m in TOUTES {
            let c = capacites(&m);
            assert!(
                c.combat || !c.retour_de_combat,
                "{m:?} : retour de combat sans combat"
            );
            assert!(
                c.retour_de_combat || !c.hud_generique,
                "{m:?} : HUD générique sans retour de combat"
            );
            assert!(
                c.hud_generique || !c.vagues,
                "{m:?} : compteur de vagues sans HUD générique"
            );
        }
    }

    #[test]
    fn le_menu_ne_gate_rien() {
        assert_eq!(capacites(&GameMode::None), Capacites::RIEN);
    }

    #[test]
    fn les_trois_zones_de_combat_sont_declarees() {
        // Le GDD « The Spared » ship trois zones jouables : le lobby (menu),
        // l'Abîme et l'Expédition. Les deux dernières tirent, et elles doivent
        // TOUTES DEUX montrer ce qu'elles font — c'est précisément ce qui
        // manquait à l'Expédition entre le 2026-08-14 et le 2026-08-18.
        for m in [GameMode::Roguelite, GameMode::Expedition] {
            let c = capacites(&m);
            assert!(c.combat, "{m:?} doit tirer");
            assert!(c.retour_de_combat, "{m:?} doit montrer ses dégâts");
        }
    }

    #[test]
    fn le_roguelite_garde_son_propre_hud() {
        // Régression à protéger : le Roguelite dessine sa carte vitals et ses
        // slots d'armes. Lui donner le HUD générique en superposerait deux.
        assert!(!capacites(&GameMode::Roguelite).hud_generique);
        assert!(capacites(&GameMode::Expedition).hud_generique);
    }

    #[test]
    fn le_banc_de_blockout_tire_sans_rien_afficher() {
        let c = capacites(&GameMode::ArenaTest);
        assert!(c.combat);
        assert!(!c.retour_de_combat);
    }
}
