//! sectors.rs — l'arène en parts : géométrie PURE, aucune dépendance moteur.
//!
//! Sert [story-703](../../../docs/stories/story-703-arene-en-trois-parts-dps-check.md)
//! incrément 1. Trois consommateurs prévus, dans trois crates différentes — d'où
//! la place ici, à côté de `layout` :
//!
//! | qui | ce qu'il en fait |
//! |---|---|
//! | `forgia-stage` | bâtir les cloisons et les portes |
//! | `forgia-mode-roguelite` | poster les packs, ouvrir les portes au chrono |
//! | l'IA | décider de l'aggro **angulaire** |
//!
//! # Ce que ce module fait, et surtout ce qu'il ne fait pas
//!
//! Il **calcule**. Il ne spawne rien, ne lit aucune ressource, ne dépend ni de
//! Bevy ni de Rapier : tout y est testable sans moteur. C'est délibéré — les
//! sept défauts du chantier navmesh (2026-08-13) vivaient tous dans du code
//! qu'aucun test ne pouvait interroger.
//!
//! # Les grandeurs se DÉRIVENT
//!
//! Aucun nombre n'est choisi ici. La largeur de porte vient du plus gros agent
//! qui doit la franchir, l'ouverture angulaire du nombre de parts, le débordement
//! d'aggro d'un gène. Si une valeur vous paraît arbitraire, c'est un défaut :
//! elle doit se lire comme une conséquence.

use bevy::math::Vec2;
use std::f32::consts::TAU;

/// Découpe angulaire de l'arène et ses ouvertures.
///
/// Les parts sont numérotées `0..count` dans le sens trigonométrique, la part 0
/// étant centrée sur l'axe **+X** (angle 0). Ce choix n'est pas neutre : il rend
/// `sector_of` et `axis_of` réciproques sans décalage à retenir, donc un test
/// peut vérifier l'aller-retour au lieu de recopier une convention.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectorLayout {
    /// Nombre de parts. 3 pour le camembert de story-703.
    pub count: u32,
    /// Rayon de l'atrium central — l'espace commun, où le joueur combat.
    pub atrium_radius_m: f32,
    /// Rayon jouable de l'arène (apothème de l'enceinte).
    pub outer_radius_m: f32,
    /// Passage UTILE d'une porte (m) — pas son emprise, ce qui reste libre.
    pub door_width_m: f32,
}

impl SectorLayout {
    /// Largeur de porte MINIMALE pour qu'un agent de rayon `r` passe.
    ///
    /// # Pourquoi ce n'est pas `2 × r`
    ///
    /// Un agent qui passe exactement dans sa propre largeur frotte les deux
    /// montants : la moindre erreur de suivi le bloque, et c'est précisément le
    /// « les mobs se bloquent dans les passages » du 2026-08-13. On lui laisse
    /// **son propre rayon de jeu de chaque côté**, soit le double de sa largeur.
    ///
    /// Ce n'est pas un coefficient de confort : le maillage rétrécit déjà les
    /// obstacles du rayon d'agent, donc une porte de `2r` a un couloir navigable
    /// de largeur NULLE — elle est infranchissable par construction, pas par
    /// malchance.
    #[must_use]
    pub fn door_width_for(agent_radius_m: f32) -> f32 {
        4.0 * agent_radius_m
    }

    /// Ouverture angulaire d'une part (rad).
    #[must_use]
    pub fn span_rad(&self) -> f32 {
        TAU / self.count.max(1) as f32
    }

    /// Angle de l'axe médian de la part `i` (rad).
    #[must_use]
    pub fn axis_rad(&self, i: u32) -> f32 {
        self.span_rad() * i as f32
    }

    /// Dans quelle part tombe ce point ? Indépendant du rayon — c'est un secteur
    /// angulaire, pas une zone.
    #[must_use]
    pub fn sector_of(&self, p: Vec2) -> u32 {
        if self.count == 0 {
            return 0;
        }
        let span = self.span_rad();
        // Le +span/2 recentre : la part 0 va de -span/2 à +span/2 autour de +X.
        let a = (p.y.atan2(p.x) + span * 0.5).rem_euclid(TAU);
        ((a / span) as u32) % self.count
    }

    /// Angles des cloisons (rad) — elles séparent deux parts, donc elles se
    /// posent aux FRONTIÈRES, à mi-chemin entre deux axes.
    pub fn partition_angles(&self) -> impl Iterator<Item = f32> + '_ {
        let span = self.span_rad();
        (0..self.count).map(move |i| span * i as f32 + span * 0.5)
    }

    /// Écart angulaire signé le plus court entre deux angles (rad).
    #[must_use]
    fn ecart_angulaire(a: f32, b: f32) -> f32 {
        let d = (a - b).rem_euclid(TAU);
        if d > TAU * 0.5 { d - TAU } else { d }
    }

    /// Demi-ouverture du cône d'aggro d'un pack (rad).
    ///
    /// `spill_frac` = débordement sur CHAQUE part voisine, en fraction de part.
    /// À 0,25 sur des parts de 120°, le cône couvre 120 + 2×30 = **180°**, donc
    /// une demi-ouverture de 90°.
    #[must_use]
    pub fn aggro_half_angle_rad(&self, spill_frac: f32) -> f32 {
        self.span_rad() * (0.5 + spill_frac.max(0.0))
    }

    /// Le pack de la part `sector` voit-il un point à cette position ?
    ///
    /// **Angulaire, pas radial.** Le rayon d'aggro actuel (`detect_range`, 50 m)
    /// est plus large qu'une part : entrer dans un secteur réveillerait tout le
    /// monde. C'est l'avertissement de story-703 §3, et c'est ce qui rend la
    /// poche sûre possible.
    ///
    /// L'atrium est exclu du test angulaire par sa nature : au centre exact,
    /// l'angle n'a pas de sens. On considère qu'un point sous le rayon
    /// `EPS_CENTRE` est vu par TOUS les packs éveillés — c'est le pire cas, et il
    /// vaut mieux qu'un angle tiré au sort.
    #[must_use]
    pub fn aggro_covers(&self, sector: u32, p: Vec2, spill_frac: f32) -> bool {
        const EPS_CENTRE: f32 = 0.01;
        if p.length() < EPS_CENTRE {
            return true;
        }
        let ecart = Self::ecart_angulaire(p.y.atan2(p.x), self.axis_rad(sector)).abs();
        ecart <= self.aggro_half_angle_rad(spill_frac)
    }

    /// Arc TOTAL (rad) couvert par l'aggro des parts ouvertes — union, pas somme.
    ///
    /// L'union est la seule mesure juste : deux cônes de 180° centrés à 120° l'un
    /// de l'autre se chevauchent de 60°, et les additionner annoncerait 360° de
    /// couverture quand il en reste 60 de libres.
    #[must_use]
    pub fn covered_arc_rad(&self, opened: &[u32], spill_frac: f32) -> f32 {
        if opened.is_empty() {
            return 0.0;
        }
        // Échantillonnage au degré : exact à 1° près, et surtout INDÉPENDANT de
        // la forme des cônes — une formule d'union fermée se casserait au premier
        // changement de `count` ou de `spill_frac`.
        let pas = TAU / 360.0;
        let couverts = (0..360)
            .filter(|i| {
                let a = pas * *i as f32;
                let p = Vec2::new(a.cos(), a.sin());
                opened.iter().any(|s| self.aggro_covers(*s, p, spill_frac))
            })
            .count();
        pas * couverts as f32
    }

    /// Arc LIBRE restant (rad) — la « poche sûre » de story-703 §3.
    #[must_use]
    pub fn safe_arc_rad(&self, opened: &[u32], spill_frac: f32) -> f32 {
        (TAU - self.covered_arc_rad(opened, spill_frac)).max(0.0)
    }

    /// Position du centre d'une porte : sur l'axe de sa part, au bord de l'atrium.
    #[must_use]
    pub fn door_center(&self, i: u32) -> Vec2 {
        let a = self.axis_rad(i);
        Vec2::new(a.cos(), a.sin()) * self.atrium_radius_m
    }

    /// Longueur d'une cloison (m) — de l'atrium à l'enceinte.
    #[must_use]
    pub fn partition_length_m(&self) -> f32 {
        (self.outer_radius_m - self.atrium_radius_m).max(0.0)
    }

    /// La porte est-elle assez large pour cet agent ? **Un `false` ici est un
    /// défaut de conception, pas un réglage** : le maillage tracera un chemin que
    /// l'agent ne pourra pas suivre.
    #[must_use]
    pub fn door_admits(&self, agent_radius_m: f32) -> bool {
        self.door_width_m >= Self::door_width_for(agent_radius_m)
    }

    /// Demi-ouverture angulaire d'une porte percée dans l'anneau de l'atrium (rad).
    ///
    /// Dérivée pour que la **corde** entre les deux montants vaille exactement
    /// `door_width_m` : `2·R·sin(θ) = largeur`. Prendre l'arc au lieu de la corde
    /// donnerait une porte plus étroite que voulu — et c'est la corde que l'agent
    /// franchit.
    #[must_use]
    pub fn door_half_angle_rad(&self) -> f32 {
        if self.atrium_radius_m <= 0.0 {
            return 0.0;
        }
        (self.door_width_m / (2.0 * self.atrium_radius_m)).clamp(-1.0, 1.0).asin()
    }

    /// Les cloisons radiales : de l'atrium à l'enceinte, aux frontières de parts.
    ///
    /// Elles séparent deux parts — donc elles se posent aux frontières, jamais sur
    /// un axe. Une cloison sur un axe couperait une part en deux au lieu d'en
    /// séparer deux.
    #[must_use]
    pub fn partition_walls(&self) -> Vec<WallSeg> {
        self.partition_angles()
            .map(|a| {
                let d = Vec2::new(a.cos(), a.sin());
                WallSeg {
                    a: d * self.atrium_radius_m,
                    b: d * self.outer_radius_m,
                }
            })
            .collect()
    }

    /// L'anneau de l'atrium, **percé d'une porte par part**.
    ///
    /// Approximé par des cordes de `chord_rad` d'ouverture. Les segments qui
    /// tomberaient dans une porte sont omis — c'est cette omission qui EST la
    /// porte, il n'y a pas d'entité « porte » à ce stade.
    ///
    /// `chord_rad` fin rend l'anneau plus rond mais multiplie les colliders ; le
    /// caller arbitre. Le contrat tenu ici : **aucun segment ne mord dans le
    /// passage utile**, vérifié par test.
    #[must_use]
    pub fn atrium_walls(&self, chord_rad: f32) -> Vec<WallSeg> {
        let pas = chord_rad.max(1.0e-3);
        let n = (TAU / pas).ceil() as u32;
        let demi_porte = self.door_half_angle_rad();
        let mut murs = Vec::new();
        for i in 0..n {
            let a0 = TAU * i as f32 / n as f32;
            let a1 = TAU * (i + 1) as f32 / n as f32;
            // Un segment est écarté dès qu'il TOUCHE une porte, même
            // partiellement : garder un bout de mur dans l'ouverture la
            // rétrécirait sous sa largeur nominale, et « le nom est un contrat »
            // (`map-design-intention.md` §5.1).
            let mord = (0..self.count).any(|s| {
                let axe = self.axis_rad(s);
                Self::ecart_angulaire(a0, axe).abs() < demi_porte
                    || Self::ecart_angulaire(a1, axe).abs() < demi_porte
                    // le segment enjambe entièrement la porte
                    || (Self::ecart_angulaire(a0, axe) < 0.0
                        && Self::ecart_angulaire(a1, axe) > 0.0)
            });
            if mord {
                continue;
            }
            let r = self.atrium_radius_m;
            murs.push(WallSeg {
                a: Vec2::new(a0.cos(), a0.sin()) * r,
                b: Vec2::new(a1.cos(), a1.sin()) * r,
            });
        }
        murs
    }

    /// Passage libre RÉELLEMENT laissé par l'anneau autour de l'axe de la part
    /// `sector` — la distance entre les deux montants les plus proches.
    ///
    /// **Se mesure, ne se suppose pas.** Un anneau approximé par des cordes ne
    /// laisse pas exactement l'ouverture nominale : c'est ce nombre-là que
    /// l'agent franchit, et c'est donc lui qu'il faut comparer à son gabarit.
    #[must_use]
    pub fn measured_door_width_m(&self, sector: u32, chord_rad: f32) -> f32 {
        let axe = self.axis_rad(sector);
        let murs = self.atrium_walls(chord_rad);
        // Le montant de gauche et celui de droite : les extrémités de mur les
        // plus proches de l'axe, de part et d'autre.
        let (mut gauche, mut droite) = (f32::INFINITY, f32::INFINITY);
        for m in &murs {
            for p in [m.a, m.b] {
                let d = Self::ecart_angulaire(p.y.atan2(p.x), axe);
                if d >= 0.0 {
                    gauche = gauche.min(d);
                } else {
                    droite = droite.min(-d);
                }
            }
        }
        if !gauche.is_finite() || !droite.is_finite() {
            // Aucun mur : l'anneau est entièrement ouvert.
            return f32::INFINITY;
        }
        // Corde entre les deux montants.
        2.0 * self.atrium_radius_m * ((gauche + droite) * 0.5).sin()
    }
}

/// Un tronçon de mur en plan, d'extrémité à extrémité. Orientation quelconque —
/// les cloisons sont radiales, elles ne sont donc jamais alignées sur un axe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallSeg {
    pub a: Vec2,
    pub b: Vec2,
}

impl WallSeg {
    #[must_use]
    pub fn length_m(&self) -> f32 {
        self.a.distance(self.b)
    }

    #[must_use]
    pub fn center(&self) -> Vec2 {
        self.a.midpoint(self.b)
    }

    /// Lacet (rad) pour orienter un pavé le long du tronçon.
    #[must_use]
    pub fn yaw_rad(&self) -> f32 {
        let d = self.b - self.a;
        d.y.atan2(d.x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La disposition de story-703 : trois parts, atrium au centre, et une porte
    /// dimensionnée pour le plus GROS agent du jeu (le boss, rayon 1,40 m).
    fn camembert() -> SectorLayout {
        SectorLayout {
            count: 3,
            atrium_radius_m: 20.0,
            outer_radius_m: 69.28,
            door_width_m: SectorLayout::door_width_for(1.40),
        }
    }

    // ── Le découpage ────────────────────────────────────────────────────

    #[test]
    fn trois_parts_de_cent_vingt_degres() {
        assert!((camembert().span_rad().to_degrees() - 120.0).abs() < 1.0e-4);
    }

    #[test]
    fn l_axe_et_le_secteur_sont_reciproques() {
        // L'aller-retour, plutot que de recopier la convention dans le test :
        // un test qui reenonce la convention ne la verifie pas, il la repete.
        let l = camembert();
        for i in 0..l.count {
            let a = l.axis_rad(i);
            let p = Vec2::new(a.cos(), a.sin()) * 30.0;
            assert_eq!(l.sector_of(p), i, "part {i} : axe et secteur divergent");
        }
    }

    #[test]
    fn les_frontieres_de_parts_sont_bien_a_mi_chemin_des_axes() {
        // Une cloison posee sur un AXE couperait une part en deux au lieu d'en
        // separer deux — l'erreur de placement la plus facile a commettre.
        let l = camembert();
        let cloisons: Vec<f32> = l.partition_angles().collect();
        assert_eq!(cloisons.len(), 3);
        for c in &cloisons {
            for i in 0..l.count {
                let d = (c - l.axis_rad(i)).rem_euclid(TAU);
                let d = if d > TAU * 0.5 { TAU - d } else { d };
                assert!(
                    d > 1.0e-3,
                    "cloison a {:.1}° confondue avec l'axe de la part {i}",
                    c.to_degrees()
                );
            }
        }
    }

    #[test]
    fn tout_point_du_plan_appartient_a_exactement_une_part() {
        // Un trou ou un recouvrement dans le decoupage donnerait un pack qui ne
        // s'eveille jamais, ou deux qui s'eveillent ensemble.
        let l = camembert();
        for i in 0..720 {
            let a = TAU * i as f32 / 720.0;
            let p = Vec2::new(a.cos(), a.sin()) * 42.0;
            let s = l.sector_of(p);
            assert!(s < l.count, "angle {:.1}° -> part {s} hors bornes", a.to_degrees());
        }
    }

    // ── L'aggro angulaire et la poche sûre ──────────────────────────────

    #[test]
    fn le_cone_d_aggro_vaut_bien_la_part_plus_un_quart_de_chaque_voisine() {
        // `aggro_sector_spill_frac = 0.25` (story-703 §3) : 120 + 2x30 = 180°.
        let l = camembert();
        let demi = l.aggro_half_angle_rad(0.25).to_degrees();
        assert!((demi * 2.0 - 180.0).abs() < 1.0e-3, "cone de {:.1}°", demi * 2.0);
    }

    #[test]
    fn la_poche_sure_retrecit_a_chaque_porte_ouverte() {
        // LE test de la mecanique — et il a corrige la story au lieu de la
        // repeter. story-703 §3 annoncait « 120° / 60° / 0, divise par deux a
        // chaque porte ». MESURE : **180° / 61° / 0**, donc divise par TROIS.
        //
        // Les deux erreurs de la story venaient de la meme approximation : elle
        // raisonnait en « une part entiere » (120°) la ou la geometrie en laisse
        // une et demie, puis en deduisait un rapport de 2 qui n'existe pas.
        //
        // La vraie escalade est plus BRUTALE que celle qui etait ecrite. C'est
        // sans doute meilleur en jeu — mais ca se decide manette en main, pas
        // ici ; ce test ne fait que graver ce que la geometrie produit.
        let l = camembert();
        let f = 0.25;
        let un = l.safe_arc_rad(&[0], f).to_degrees();
        let deux = l.safe_arc_rad(&[0, 1], f).to_degrees();
        let trois = l.safe_arc_rad(&[0, 1, 2], f).to_degrees();
        println!("POCHE SURE : 1 porte {un:.0}° · 2 portes {deux:.0}° · 3 portes {trois:.0}°");

        // Ce qui est structurel, et que le jeu ne doit jamais perdre :
        assert!(un > deux, "la poche doit retrecir de 1 a 2 portes");
        assert!(deux > trois, "la poche doit retrecir de 2 a 3 portes");
        assert!(
            trois < 1.0,
            "trois packs eveilles doivent ne laisser AUCUNE poche, il en reste {trois:.0}°"
        );
        // Et les valeurs mesurees, pour qu'un changement de `count` ou de
        // `spill_frac` se voie au lieu de deriver en silence.
        assert!((un - 180.0).abs() < 3.0, "1 porte : {un:.0}° (attendu ~180)");
        assert!((deux - 61.0).abs() < 3.0, "2 portes : {deux:.0}° (attendu ~61)");
    }

    #[test]
    fn l_union_des_cones_n_est_pas_leur_somme() {
        // Deux cones de 180° centres a 120° l'un de l'autre se recouvrent de 60°.
        // Les additionner annoncerait 360° de couverture alors qu'il reste 60°
        // libres — et la poche sure disparaitrait sur le papier avant de
        // disparaitre en jeu.
        let l = camembert();
        let couvert = l.covered_arc_rad(&[0, 1], 0.25).to_degrees();
        assert!(
            couvert < 359.0,
            "l'union de deux cones de 180° ne peut pas couvrir tout le plan : {couvert:.0}°"
        );
        assert!((couvert - 300.0).abs() < 3.0, "attendu ~300°, mesure {couvert:.0}°");
    }

    #[test]
    fn le_centre_exact_est_vu_par_tous_les_packs() {
        // Au centre l'angle n'a pas de sens. Le pire cas vaut mieux qu'un angle
        // tire au sort : un joueur au centre exact ne doit pas etre invisible par
        // accident de flottant.
        let l = camembert();
        for s in 0..l.count {
            assert!(l.aggro_covers(s, Vec2::ZERO, 0.25));
        }
    }

    // ── Les portes ──────────────────────────────────────────────────────

    #[test]
    fn une_porte_de_deux_rayons_est_infranchissable_par_construction() {
        // LE piege que P1 de story-703 documente. Le maillage retrecit les
        // obstacles du rayon d'agent : une porte de `2r` a donc un couloir
        // navigable de largeur NULLE. Elle n'est pas « juste » — elle est
        // impossible, et aucun reglage de suivi n'y changera rien.
        let r = 1.40; // boss
        let juste = 2.0 * r;
        let requis = SectorLayout::door_width_for(r);
        assert!(
            requis > juste,
            "la largeur requise doit depasser 2r, sinon le couloir navigable est nul"
        );
        assert!((requis - 5.6).abs() < 1.0e-4, "porte boss = {requis} m");
    }

    #[test]
    fn la_porte_du_camembert_admet_les_quatre_archetypes() {
        // Les rayons reels de `roguelite_enemies.toml`. Le boss (1,40 m) est le
        // dimensionnant : c'est LUI qui fixe la porte, pas le confort visuel.
        let l = camembert();
        for (nom, r) in [("sniper", 0.30), ("runner", 0.32), ("tank", 0.55), ("boss", 1.40)] {
            assert!(
                l.door_admits(r),
                "{nom} (rayon {r} m) ne passe pas une porte de {} m",
                l.door_width_m
            );
        }
    }

    #[test]
    fn une_porte_dimensionnee_pour_un_petit_refuse_le_boss() {
        // Le test negatif, sans lequel le precedent ne prouve rien : si
        // `door_admits` rendait toujours vrai, les deux passeraient.
        let etroite = SectorLayout {
            door_width_m: SectorLayout::door_width_for(0.30),
            ..camembert()
        };
        assert!(etroite.door_admits(0.30));
        assert!(!etroite.door_admits(1.40), "une porte de sniper laisse passer un boss ?");
    }

    // ── Les murs bâtis ──────────────────────────────────────────────────

    /// Finesse de l'anneau : 5° par corde. Le caller arbitre, mais les tests
    /// doivent tourner sur une valeur realiste — un anneau a 90° serait un
    /// triangle et masquerait tous les defauts d'arrondi.
    const CORDE: f32 = TAU / 72.0;

    #[test]
    fn trois_cloisons_radiales_aux_frontieres() {
        let l = camembert();
        let murs = l.partition_walls();
        assert_eq!(murs.len(), 3);
        for m in &murs {
            assert!((m.a.length() - l.atrium_radius_m).abs() < 1.0e-3);
            assert!((m.b.length() - l.outer_radius_m).abs() < 1.0e-3);
            // Radiale : les deux extremites sur le meme rayon.
            let da = Vec2::new(m.a.x, m.a.y).normalize();
            let db = Vec2::new(m.b.x, m.b.y).normalize();
            assert!(da.dot(db) > 0.999, "cloison non radiale");
        }
    }

    #[test]
    fn l_anneau_laisse_bien_trois_ouvertures() {
        // Une porte par part. Zero ouverture = l'atrium est une prison ; quatre =
        // une cloison a ete mangee.
        let l = camembert();
        let murs = l.atrium_walls(CORDE);
        // Compte les ruptures de continuite angulaire.
        let mut angles: Vec<f32> = murs
            .iter()
            .map(|m| m.center().y.atan2(m.center().x).rem_euclid(TAU))
            .collect();
        angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let trous = angles
            .windows(2)
            .filter(|w| w[1] - w[0] > CORDE * 1.5)
            .count()
            // le trou qui enjambe 0 rad
            + usize::from(angles[0] + TAU - angles[angles.len() - 1] > CORDE * 1.5);
        assert_eq!(trous, 3, "attendu 3 portes, mesure {trous}");
    }

    #[test]
    fn le_passage_mesure_n_est_jamais_plus_etroit_que_la_porte_nominale() {
        // LE test qui compte, et celui que « le nom est un contrat »
        // (map-design-intention §5.1) exige : on MESURE l'ouverture reellement
        // laissee entre les deux montants, on ne suppose pas qu'elle vaut la
        // valeur nominale. Un anneau approxime par des cordes ne la donne pas
        // exactement — et c'est cette corde-la que l'agent franchit.
        let l = camembert();
        for s in 0..l.count {
            let mesure = l.measured_door_width_m(s, CORDE);
            println!("PORTE {s} : nominale {:.2} m · mesuree {mesure:.2} m", l.door_width_m);
            assert!(
                mesure >= l.door_width_m - 1.0e-3,
                "porte {s} : {mesure:.2} m mesures pour {:.2} m annonces — \
                 l'anneau mord dans le passage",
                l.door_width_m
            );
        }
    }

    #[test]
    fn le_passage_mesure_admet_le_boss() {
        // La consequence qui interesse le jeu : le plus gros agent passe VRAIMENT,
        // pas seulement sur le papier.
        let l = camembert();
        let requis = SectorLayout::door_width_for(1.40);
        for s in 0..l.count {
            assert!(
                l.measured_door_width_m(s, CORDE) >= requis,
                "le boss ne passe pas la porte {s}"
            );
        }
    }

    #[test]
    fn un_anneau_grossier_ne_ment_pas_sur_son_passage() {
        // Le test negatif de la mesure : avec des cordes ENORMES, l'ouverture
        // reelle s'ecarte du nominal. `measured_door_width_m` doit le REFLETER
        // (valeur differente), pas rendre docilement la valeur nominale — sinon
        // il ne mesure rien et le test precedent ne prouve rien.
        let l = camembert();
        let fin = l.measured_door_width_m(0, TAU / 72.0);
        let grossier = l.measured_door_width_m(0, TAU / 8.0);
        assert!(
            (fin - grossier).abs() > 0.5,
            "la mesure ne reagit pas a la finesse de l'anneau ({fin:.2} vs {grossier:.2})"
        );
    }

    #[test]
    fn un_troncon_donne_son_centre_sa_longueur_et_son_lacet() {
        let s = WallSeg {
            a: Vec2::new(0.0, 0.0),
            b: Vec2::new(10.0, 0.0),
        };
        assert!((s.length_m() - 10.0).abs() < 1.0e-4);
        assert!((s.center() - Vec2::new(5.0, 0.0)).length() < 1.0e-4);
        assert!(s.yaw_rad().abs() < 1.0e-4);
    }

    #[test]
    fn une_cloison_va_de_l_atrium_a_l_enceinte() {
        let l = camembert();
        assert!((l.partition_length_m() - 49.28).abs() < 1.0e-3);
    }

    #[test]
    fn les_portes_se_posent_sur_les_axes_au_bord_de_l_atrium() {
        // Une porte sur une CLOISON ouvrirait entre deux parts au lieu d'ouvrir
        // une part sur l'atrium — l'inverse de la mecanique.
        let l = camembert();
        for i in 0..l.count {
            let c = l.door_center(i);
            assert!((c.length() - l.atrium_radius_m).abs() < 1.0e-3);
            assert_eq!(l.sector_of(c), i, "la porte {i} n'ouvre pas sur sa part");
        }
    }
}
