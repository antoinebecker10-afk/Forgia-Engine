//! defense.rs — Story-640 P0-2 : couche défensive tri-couche (Vie/Bouclier/Armure).
//!
//! `DefenseLayer` est un composant **générique** (agnostique du type `Health`) qui
//! s'empile **AU-DESSUS** de la Vie : les dégâts entrants sont absorbés par le
//! **Bouclier** (bleu, régénérant hors combat) puis l'**Armure** (jaune, permanente)
//! avant de « fuir » vers la `Health`. Vit dans `forgia-damage` (crate atomique
//! neutre) pour être partagé par les deux pipelines de dégâts :
//! - **Joueur** (`forgia_damage::Health`) : `apply_damage` absorbe via ce composant.
//! - **Ennemis** (`forgia_combat::Health`) : le hit de base (forgia-fps) appelle
//!   `absorb()` puis mute la Vie directement (PIÈGE dual-health : jamais de `DamageEvent`
//!   sur un ennemi).
//!
//! La logique (`absorb`/`note_hit`/`regen`) est **pure et testable headless** ; les
//! valeurs (shield_max/armor_max/regen) vivent dans un genome côté mode (roguelite).

use bevy::prelude::*;

/// Canal de dégâts — détermine quelles couches absorbent avant la Vie.
///
/// P0-2 n'utilise que `Physical` (hit de base) et `TrueHealth` (bypass, pour les
/// effets déjà couplés à la Vie : brûlure, poison DoT). Les canaux `Shield`
/// (Électrique, P0-3) et `Armor` (Perforant, P0-4) seront ajoutés avec leur élément.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DamageChannel {
    /// Hit de base / physique : Bouclier → Armure → Vie (ordre d'absorption complet).
    #[default]
    Physical,
    /// Bypass total des couches → frappe la Vie directement (brûlure, poison DoT « pur »).
    TrueHealth,
}

/// Affinité élémentaire par couche (story-642 P0-4). Multiplicateur d'efficacité
/// d'un élément contre chaque couche : `mult > 1` = fort (efficace), `mult < 1` =
/// faible. Modèle « effectiveness » : 1 point de dégât brut retire `mult` points de
/// la couche visée (donc casser une couche coûte `couche / mult` de brut).
/// Ex. Feu = (bouclier 0.75, armure 0.75, vie 1.5) → +50 % Vie, −25 % Bouclier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElementAffinity {
    pub shield_mult: f32,
    pub armor_mult: f32,
    pub life_mult: f32,
}

impl ElementAffinity {
    /// Neutre — aucun bonus/malus (fallback si l'élément n'a pas d'affinité).
    pub const NEUTRAL: Self = Self {
        shield_mult: 1.0,
        armor_mult: 1.0,
        life_mult: 1.0,
    };

    pub fn new(shield_mult: f32, armor_mult: f32, life_mult: f32) -> Self {
        Self { shield_mult, armor_mult, life_mult }
    }
}

/// Marque de **vulnérabilité** générique (story-642 P0-4 Inc.3) : les dégâts subis
/// par l'entité sont ×`mult` tant que la marque tient (`secs_left`). Vit dans
/// `forgia-damage` (crate neutre) pour être LUE à la fois par le **hit de base**
/// (forgia-fps) ET la **couche élémentaire** (forgia-mode-roguelite) — remplace
/// l'ex-`StatusShock` mode-only (invisible à forgia-fps). Posée par les hits
/// électriques (`Element::Shock`). Non-stackante : un nouveau hit rafraîchit.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Vulnerability {
    /// Multiplicateur de dégâts subis (1.1 = +10 %).
    pub mult: f32,
    /// Secondes restantes avant expiration.
    pub secs_left: f32,
}

impl Vulnerability {
    pub fn new(mult: f32, secs_left: f32) -> Self {
        Self { mult, secs_left }
    }

    /// Fait avancer le temps ; retourne `true` si la marque a expiré (à retirer).
    pub fn tick(&mut self, dt: f32) -> bool {
        self.secs_left -= dt;
        self.secs_left <= 0.0
    }
}

/// Couche défensive empilée au-dessus de la `Health`. Absorbe Bouclier → Armure → Vie.
///
/// Champs `f32` plats (replication-ready pour le netcode post-ship). `since_hit`
/// pilote la régénération du bouclier (remis à 0 par `note_hit`).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct DefenseLayer {
    /// Bouclier courant (bleu). Régénère à `regen_rate`/s après `regen_delay` s. sans coup.
    pub shield: f32,
    /// Bouclier max (plafond de régénération).
    pub shield_max: f32,
    /// Armure courante (jaune). Absorbe après le bouclier ; **ne régénère pas**.
    pub armor: f32,
    /// Armure max (référence pour le sensor / futurs rechargements).
    pub armor_max: f32,
    /// Points de bouclier régénérés par seconde (hors combat).
    pub regen_rate: f32,
    /// Délai (s) sans coup avant que le bouclier commence à régénérer.
    pub regen_delay: f32,
    /// Temps (s) écoulé depuis le dernier coup absorbé. Remis à 0 par `note_hit`.
    pub since_hit: f32,
}

impl DefenseLayer {
    /// Crée une couche pleine (bouclier + armure au max).
    pub fn new(shield_max: f32, armor_max: f32, regen_rate: f32, regen_delay: f32) -> Self {
        Self {
            shield: shield_max,
            shield_max,
            armor: armor_max,
            armor_max,
            regen_rate,
            regen_delay,
            since_hit: regen_delay, // prêt à régénérer si jamais entamé au spawn
        }
    }

    /// Bouclier + armure combinés (points d'absorption restants au-dessus de la Vie).
    pub fn buffer(&self) -> f32 {
        self.shield + self.armor
    }

    /// Absorbe `raw` dégâts selon le canal. Draine Bouclier puis Armure ; retourne
    /// les dégâts **résiduels** à soustraire de la `Health`. PUR (0 side-effect hors
    /// self). N'appelle PAS `note_hit` — le caller le fait (il connaît « un coup a eu lieu »
    /// même si `raw` a été entièrement absorbé, pour geler la régénération).
    pub fn absorb(&mut self, raw: f32, channel: DamageChannel) -> f32 {
        if raw <= 0.0 {
            return raw.max(0.0);
        }
        match channel {
            DamageChannel::TrueHealth => raw, // bypass : tout passe à la Vie
            DamageChannel::Physical => {
                let mut dmg = raw;
                // 1) Bouclier
                let s = dmg.min(self.shield.max(0.0));
                self.shield -= s;
                dmg -= s;
                // 2) Armure
                if dmg > 0.0 {
                    let a = dmg.min(self.armor.max(0.0));
                    self.armor -= a;
                    dmg -= a;
                }
                dmg // 3) résidu → Vie
            }
        }
    }

    /// Absorbe des dégâts **élémentaires** avec efficacité par couche (P0-4). Draine
    /// Bouclier puis Armure avec leur multiplicateur d'affinité (1 pt de `raw` retire
    /// `mult` pts de la couche → casser une couche coûte `couche / mult` de `raw`),
    /// puis renvoie le résidu appliqué à la Vie, **déjà ×`life_mult`**. PUR ; n'appelle
    /// PAS `note_hit` (le caller le fait). Un `mult ≤ 0` sur une couche = la couche est
    /// sautée (l'élément ne peut pas l'entamer, le budget passe à la suivante).
    pub fn absorb_elemental(&mut self, raw: f32, aff: &ElementAffinity) -> f32 {
        if raw <= 0.0 {
            return 0.0;
        }
        let mut budget = raw;
        // 1) Bouclier
        if self.shield > 0.0 && aff.shield_mult > 0.0 {
            let needed = self.shield / aff.shield_mult; // brut requis pour le vider
            if budget <= needed {
                self.shield -= budget * aff.shield_mult;
                return 0.0;
            }
            self.shield = 0.0;
            budget -= needed;
        }
        // 2) Armure
        if self.armor > 0.0 && aff.armor_mult > 0.0 {
            let needed = self.armor / aff.armor_mult;
            if budget <= needed {
                self.armor -= budget * aff.armor_mult;
                return 0.0;
            }
            self.armor = 0.0;
            budget -= needed;
        }
        // 3) Vie : résidu ×life_mult
        (budget * aff.life_mult).max(0.0)
    }

    /// Signale qu'un coup vient d'être reçu : gèle la régénération (reset du compteur).
    pub fn note_hit(&mut self) {
        self.since_hit = 0.0;
    }

    /// Fait avancer le temps et régénère le bouclier hors combat. À appeler chaque
    /// frame/tick. Le bouclier ne dépasse jamais `shield_max` ; l'armure ne régénère pas.
    /// Retourne les points de bouclier régénérés ce tick (pour le sensor).
    pub fn regen(&mut self, dt: f32) -> f32 {
        self.since_hit += dt;
        if self.since_hit < self.regen_delay || self.shield >= self.shield_max {
            return 0.0;
        }
        let before = self.shield;
        self.shield = (self.shield + self.regen_rate * dt).min(self.shield_max);
        self.shield - before
    }

    /// True si le bouclier est en train de régénérer (hors combat et pas plein).
    pub fn is_regenerating(&self) -> bool {
        self.since_hit >= self.regen_delay && self.shield < self.shield_max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer() -> DefenseLayer {
        // shield 50, armor 30, regen 20/s, delay 3s.
        DefenseLayer::new(50.0, 30.0, 20.0, 3.0)
    }

    #[test]
    fn absorb_physical_drains_shield_first() {
        let mut d = layer();
        // 40 dmg < shield 50 → tout dans le bouclier, 0 fuite.
        let leak = d.absorb(40.0, DamageChannel::Physical);
        assert_eq!(leak, 0.0);
        assert_eq!(d.shield, 10.0);
        assert_eq!(d.armor, 30.0);
    }

    #[test]
    fn absorb_physical_overflows_shield_then_armor() {
        let mut d = layer();
        // 60 : 50 bouclier + 10 armure → 0 fuite, armure 20.
        let leak = d.absorb(60.0, DamageChannel::Physical);
        assert_eq!(leak, 0.0);
        assert_eq!(d.shield, 0.0);
        assert_eq!(d.armor, 20.0);
    }

    #[test]
    fn absorb_physical_leaks_to_health_when_buffer_empty() {
        let mut d = layer();
        // 100 : 50 bouclier + 30 armure = 80 absorbés → 20 fuite vers la Vie.
        let leak = d.absorb(100.0, DamageChannel::Physical);
        assert!((leak - 20.0).abs() < 1e-4);
        assert_eq!(d.shield, 0.0);
        assert_eq!(d.armor, 0.0);
    }

    #[test]
    fn absorb_true_health_bypasses_layers() {
        let mut d = layer();
        let leak = d.absorb(25.0, DamageChannel::TrueHealth);
        assert_eq!(leak, 25.0);
        assert_eq!(d.shield, 50.0, "TrueHealth ne touche pas le bouclier");
        assert_eq!(d.armor, 30.0);
    }

    #[test]
    fn absorb_non_positive_is_noop() {
        let mut d = layer();
        assert_eq!(d.absorb(0.0, DamageChannel::Physical), 0.0);
        assert_eq!(d.absorb(-5.0, DamageChannel::Physical), 0.0);
        assert_eq!(d.shield, 50.0);
    }

    // ── absorb_elemental (affinité par couche, story-642 P0-4) ──

    /// Feu (faible vs bouclier 0.75) : 40 brut retire 40×0.75 = 30 de bouclier, 0 fuite.
    #[test]
    fn elemental_weak_vs_shield_drains_less() {
        let mut d = layer(); // shield 50, armor 30
        let fire = ElementAffinity::new(0.75, 0.75, 1.5);
        let leak = d.absorb_elemental(40.0, &fire);
        assert_eq!(leak, 0.0);
        assert!((d.shield - 20.0).abs() < 1e-4, "40×0.75=30 retirés → 20");
    }

    /// Électrique (fort vs bouclier 1.5) : casse le bouclier plus vite qu'un neutre.
    #[test]
    fn elemental_strong_vs_shield_breaks_faster() {
        let mut d = layer();
        let shock = ElementAffinity::new(1.5, 0.75, 0.75);
        // bouclier 50 : cassé en 50/1.5 = 33.33 brut. 40 brut → bouclier 0, reste 6.67
        // → armure ×0.75 : 6.67×0.75 = 5 retirés → armure 25. 0 fuite.
        let leak = d.absorb_elemental(40.0, &shock);
        assert_eq!(leak, 0.0);
        assert_eq!(d.shield, 0.0);
        assert!((d.armor - 25.0).abs() < 1e-3, "reste 6.67 brut ×0.75 = 5 sur l'armure");
    }

    /// Feu +50 % Vie : après avoir percé bouclier+armure, le résidu frappe la Vie ×1.5.
    #[test]
    fn elemental_life_mult_amplifies_leak() {
        let mut d = layer(); // shield 50, armor 30
        let fire = ElementAffinity::new(0.75, 0.75, 1.5);
        // vider bouclier = 50/0.75 = 66.67 ; armure = 30/0.75 = 40 ; total 106.67 brut.
        // 120 brut → reste 13.33 → Vie ×1.5 = 20.
        let leak = d.absorb_elemental(120.0, &fire);
        assert!((leak - 20.0).abs() < 1e-2, "résidu 13.33 ×1.5 = 20");
        assert_eq!(d.shield, 0.0);
        assert_eq!(d.armor, 0.0);
    }

    /// NEUTRAL se comporte comme l'absorption physique (sanity).
    #[test]
    fn elemental_neutral_matches_physical() {
        let mut d = layer();
        let leak = d.absorb_elemental(100.0, &ElementAffinity::NEUTRAL);
        assert!((leak - 20.0).abs() < 1e-4, "50 bouclier + 30 armure = 80, fuite 20");
        assert_eq!(d.shield, 0.0);
        assert_eq!(d.armor, 0.0);
    }

    #[test]
    fn vulnerability_ticks_and_expires() {
        let mut v = Vulnerability::new(1.1, 1.0);
        assert!(!v.tick(0.4), "0.4s < 1.0 → pas expiré");
        assert!((v.secs_left - 0.6).abs() < 1e-4);
        assert!(!v.tick(0.5), "0.9s < 1.0 → pas expiré");
        assert!(v.tick(0.2), "1.1s ≥ 1.0 → expiré");
        assert_eq!(v.mult, 1.1, "le mult ne change pas au tick");
    }

    #[test]
    fn elemental_non_positive_is_noop() {
        let mut d = layer();
        assert_eq!(d.absorb_elemental(0.0, &ElementAffinity::NEUTRAL), 0.0);
        assert_eq!(d.absorb_elemental(-3.0, &ElementAffinity::NEUTRAL), 0.0);
        assert_eq!(d.shield, 50.0);
    }

    /// Documente le comportement P0-4 Inc.2 (story-642 §3, décision balance) : un DoT
    /// qui appelle `note_hit()` à chaque tick (ex. Miasma, 2 Hz) gèle la régén tant
    /// qu'il tient (`since_hit` remis à 0 avant d'atteindre `regen_delay`). Garde-fou :
    /// si on décide plus tard de laisser régénérer sous DoT léger, ce test change AVEC
    /// l'intention (pas une régression silencieuse).
    #[test]
    fn repeated_note_hit_freezes_regen_like_active_miasma() {
        let mut d = layer(); // shield 50, regen 20/s, delay 3s
        d.absorb_elemental(20.0, &ElementAffinity::new(0.75, 1.5, 0.75)); // entame le bouclier
        let before = d.shield;
        for _ in 0..20 {
            // 10 s de DoT à 2 Hz : chaque tick = note_hit + regen(0.5).
            d.note_hit();
            d.regen(0.5);
        }
        assert!(d.shield <= before, "régén gelée tant que note_hit est répété (Miasma actif)");
        assert!(!d.is_regenerating(), "since_hit remis à 0 chaque tick → jamais en régén");
    }

    /// Un `mult` minuscule (genome mal réglé) dégrade proprement : la couche devient
    /// quasi-incassable mais AUCUN NaN/Inf/panic (comportement dégénéré documenté).
    #[test]
    fn elemental_tiny_mult_degrades_gracefully() {
        let mut d = layer(); // shield 50
        let aff = ElementAffinity::new(0.001, 0.001, 1.0);
        let leak = d.absorb_elemental(40.0, &aff);
        assert!(leak.is_finite() && leak == 0.0, "budget 40 << needed → tout absorbé");
        assert!(d.shield.is_finite() && d.shield >= 0.0);
        assert!((d.shield - 49.96).abs() < 1e-3, "40×0.001 = 0.04 retiré du bouclier");
    }

    #[test]
    fn regen_frozen_before_delay() {
        let mut d = layer();
        d.absorb(20.0, DamageChannel::Physical);
        d.note_hit(); // since_hit = 0
        let regenned = d.regen(1.0); // 1s < 3s delay
        assert_eq!(regenned, 0.0);
        assert_eq!(d.shield, 30.0, "pas de régen avant le délai");
        assert!(!d.is_regenerating());
    }

    #[test]
    fn regen_after_delay_refills_shield() {
        let mut d = layer();
        d.absorb(30.0, DamageChannel::Physical); // shield 20
        d.note_hit(); // since_hit = 0
        // Sous le délai (3 s) : gelé.
        assert_eq!(d.regen(2.0), 0.0); // since_hit 2.0 < 3.0
        assert_eq!(d.regen(0.5), 0.0); // since_hit 2.5 < 3.0
        assert_eq!(d.shield, 20.0, "aucune régén avant le délai");
        // Franchit le délai : since_hit 2.5 → 3.5 ≥ 3.0 → régénère 20×1.0 = 20 → 40.
        let regenned = d.regen(1.0);
        assert!((regenned - 20.0).abs() < 1e-4);
        assert!((d.shield - 40.0).abs() < 1e-4);
        assert!(d.is_regenerating());
    }

    #[test]
    fn regen_caps_at_shield_max() {
        let mut d = layer();
        d.absorb(5.0, DamageChannel::Physical); // shield 45
        d.note_hit();
        d.regen(3.0); // franchit le délai
        let _ = d.regen(10.0); // largement de quoi dépasser
        assert_eq!(d.shield, d.shield_max);
        assert!(!d.is_regenerating(), "plein → plus en régén");
    }

    #[test]
    fn armor_never_regenerates() {
        let mut d = layer();
        d.absorb(60.0, DamageChannel::Physical); // shield 0, armor 20
        d.note_hit();
        d.regen(3.0);
        d.regen(100.0); // le bouclier remonte, l'armure reste
        assert_eq!(d.armor, 20.0, "l'armure ne régénère jamais");
        assert_eq!(d.shield, d.shield_max);
    }

    #[test]
    fn buffer_sums_shield_and_armor() {
        let d = layer();
        assert!((d.buffer() - 80.0).abs() < 1e-4);
    }
}
