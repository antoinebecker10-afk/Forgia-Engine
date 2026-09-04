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
//! simulation ⊇ combat ⊇ retour_de_combat ⊇ hud_generique ⊇ vagues
//! ```
//!
//! - **simulation** — la zone fait tourner un monde : des corps, des colliders,
//!   un sol. Le menu n'en a pas, et c'est normal. Ajoutée le 2026-08-18 après
//!   un run où **3 alertes sur 7 étaient des faux positifs de menu** :
//!   « Physics world empty », « toon 0 caméra », « lag events ». Aucune ne
//!   décrivait un défaut. C'est ainsi qu'on apprend à ignorer un capteur — et
//!   c'est ainsi que 19 capteurs se sont figés jusqu'à 436 h sans que personne
//!   réagisse.
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
    /// La zone fait tourner un monde physique : corps, colliders, sol.
    ///
    /// Un capteur de simulation qui ne trouve rien doit rendre `info` quand
    /// c'est `false` — « rien à mesurer ici » — et `warn` seulement quand c'est
    /// `true`. Sans cette distinction, il crie au menu et on cesse de le lire.
    pub simulation: bool,
    /// Le tir, les munitions et le changement d'arme tournent.
    pub combat: bool,
    /// Flash d'écran, arc de direction, killfeed, jauge de munitions.
    pub retour_de_combat: bool,
    /// La zone n'a pas de HUD à elle : barre de vie + bandeau d'armes partagés.
    pub hud_generique: bool,
    /// Compteur de vagues.
    pub vagues: bool,
    /// Un ennemi tué **réapparaît** après un délai.
    ///
    /// Hors de la chaîne d'inclusion : ce n'est pas une couche d'affichage, c'est
    /// une règle de jeu. Elle vit ici parce qu'elle était encodée **en négatif**
    /// dans `forgia-ai-arena-bot` — `if !is_roguelite { queue_respawn() }` — ce
    /// qui donnait la réapparition infinie à toute zone qui n'était pas l'Abîme,
    /// **l'Expédition comprise**. Un campement dont les morts reviennent ne se
    /// nettoie jamais, donc son verrou ne s'ouvre jamais.
    ///
    /// Sixième occurrence du même défaut dans la journée du 2026-08-18, et la
    /// plus vicieuse : ici le comportement par défaut était le MAUVAIS, et une
    /// seule zone s'en exemptait **par son nom**.
    pub ennemis_reapparaissent: bool,
}

impl Capacites {
    const RIEN: Self = Self {
        simulation: false,
        combat: false,
        retour_de_combat: false,
        hud_generique: false,
        vagues: false,
        ennemis_reapparaissent: false,
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
            simulation: true,
            combat: true,
            retour_de_combat: true,
            hud_generique: true,
            vagues: true,
            // L'arène sans fin : la réapparition EST son mode de jeu.
            ennemis_reapparaissent: true,
        },

        // L'Abîme. Combat complet, mais il dessine SON HUD (carte vitals,
        // slots d'armes aux noms lore) — d'où `hud_generique: false`.
        GameMode::Roguelite => Capacites {
            simulation: true,
            combat: true,
            retour_de_combat: true,
            hud_generique: false,
            vagues: false,
            // Chaque vague a un compte fixé : une réapparition ferait fuiter le
            // compteur de vivants et la vague ne se terminerait jamais.
            ennemis_reapparaissent: false,
        },

        // L'Expédition. Passée à la 3ᵉ personne le 2026-08-14 ; elle tire et
        // encaisse, et n'a pas de HUD à elle.
        GameMode::Expedition => Capacites {
            simulation: true,
            combat: true,
            retour_de_combat: true,
            hud_generique: true,
            vagues: false,
            // Un campement se NETTOIE, et son verrou s'ouvre. Des morts qui
            // reviennent le rendraient infranchissable.
            ennemis_reapparaissent: false,
        },

        // Banc de blockout : on tire pour éprouver la forme d'une salle. Aucun
        // affichage — c'est voulu, la géométrie doit se juger nue.
        GameMode::ArenaTest => Capacites {
            simulation: true,
            combat: true,
            ..Capacites::RIEN
        },

        // Zones sans combat — mais qui font TOURNER UN MONDE : le RPG a sa
        // propre pile, le Hall est neutre par conception (GDD), la démo Cyber
        // City n'a aucun gameplay. Toutes trois ont un sol et un joueur qui
        // marche dessus : `simulation` est vrai, tout le reste est faux.
        GameMode::Rpg | GameMode::CastleHub | GameMode::CyberCity => Capacites {
            simulation: true,
            ..Capacites::RIEN
        },
    }
}

// ─── Run-conditions ──────────────────────────────────────────────────────────
//
// Elles vérifient TOUTES `AppMode::InGame` : une capacité de gameplay n'a aucun
// sens au menu ou en pause, et chaque site d'appel le redemandait à la main.

/// La zone courante fait-elle tourner un monde physique ?
///
/// Sans garde `AppMode` : un capteur doit pouvoir répondre « aucune zone
/// chargée » depuis le menu, et c'est précisément l'information qui manquait.
pub fn simule_un_monde(mode: Res<State<GameMode>>) -> bool {
    capacites(mode.get()).simulation
}

/// Les ennemis de cette zone réapparaissent-ils après leur mort ?
///
/// Sans garde `AppMode` : la règle appartient à la zone, pas à l'état de l'app.
#[must_use]
pub fn ennemis_reapparaissent(mode: &GameMode) -> bool {
    capacites(mode).ennemis_reapparaissent
}

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
                c.simulation || !c.combat,
                "{m:?} : du combat sans monde a simuler"
            );
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

    /// Toute zone jouable simule un monde ; le menu, non. C'est ce qui permet à
    /// un capteur de physique de distinguer « rien à mesurer » de « ça devrait
    /// marcher et ça ne marche pas ».
    #[test]
    fn seul_le_menu_ne_simule_rien() {
        assert!(!capacites(&GameMode::None).simulation);
        for m in TOUTES {
            if m == GameMode::None {
                continue;
            }
            assert!(capacites(&m).simulation, "{m:?} devrait simuler un monde");
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

    /// Seule l'arène sans fin fait revenir ses morts.
    ///
    /// C'était encodé EN NÉGATIF dans le code des bots (« si ce n'est pas
    /// l'Abîme, remets-le en file ») : le défaut par défaut. L'Expédition en a
    /// hérité sans que rien ne le dise — un campement dont les morts reviennent
    /// ne se nettoie jamais.
    #[test]
    fn seule_l_arene_sans_fin_fait_revenir_ses_morts() {
        assert!(capacites(&GameMode::Fps).ennemis_reapparaissent);
        for m in TOUTES {
            if m == GameMode::Fps {
                continue;
            }
            assert!(
                !capacites(&m).ennemis_reapparaissent,
                "{m:?} : des morts qui reviennent empechent tout nettoyage"
            );
        }
    }

    #[test]
    fn le_banc_de_blockout_tire_sans_rien_afficher() {
        let c = capacites(&GameMode::ArenaTest);
        assert!(c.combat);
        assert!(!c.retour_de_combat);
    }
}
