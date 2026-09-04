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
    /// Rayon circonscrit de l'enceinte hexagonale (m) — l'`extent` du stage.
    ///
    /// # Il n'y a PAS d'atrium
    ///
    /// Une première version posait un anneau central de 20 m avec des cloisons
    /// radiales. Elle a été retirée le 2026-08-13 après une run : le tank et le
    /// boss apparaissent à 12 m, donc DANS l'anneau, enfermés avec le joueur —
    /// « Player died » 18 s après le spawn, deux fois.
    ///
    /// La seule cage, pour le joueur COMME pour les ennemis, est l'enceinte
    /// extérieure. C'est elle qu'on perce, et rien d'autre.
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

    /// La porte est-elle assez large pour cet agent ? **Un `false` ici est un
    /// défaut de conception, pas un réglage** : le maillage tracera un chemin que
    /// l'agent ne pourra pas suivre.
    #[must_use]
    pub fn door_admits(&self, agent_radius_m: f32) -> bool {
        self.door_width_m >= Self::door_width_for(agent_radius_m)
    }

    // ── Les portes percées dans l'ENCEINTE ──────────────────────────────
    //
    // L'enceinte est un hexagone (`ramparts_hex_positions`) : 6 faces dont les
    // milieux tombent à 0°, 60°, 120°, 180°, 240°, 300°. Trois portes « dans
    // trois directions opposées » sont donc exactement **une face sur deux** —
    // et elles s'alignent d'elles-mêmes sur les axes de parts, sans décalage à
    // retenir. La géométrie du kit donnait la réponse.

    /// Apothème de l'hexagone (m) : du centre au MILIEU d'une face, là où la
    /// porte se perce. Ce n'est PAS `outer_radius_m`, qui va jusqu'à un sommet.
    #[must_use]
    pub fn apothem_m(&self) -> f32 {
        self.outer_radius_m * (TAU / 12.0).cos()
    }

    /// Longueur d'une face. Dans un hexagone régulier, le côté vaut le rayon
    /// circonscrit — d'où l'égalité, qui n'est pas une coïncidence de code.
    #[must_use]
    pub fn hex_side_len_m(&self) -> f32 {
        self.outer_radius_m
    }

    /// La face percée pour la part `i` : celle dont le milieu est angulairement
    /// le plus proche de l'axe de la part.
    ///
    /// Choisir « la plus proche » plutôt qu'une formule fermée garde le calcul
    /// juste pour un `count` autre que 3 — et un test vérifie qu'aucune face
    /// n'est réclamée par deux parts.
    #[must_use]
    pub fn doored_hex_face(&self, i: u32) -> usize {
        let axe = self.axis_rad(i);
        (0..6)
            .min_by(|a, b| {
                let da = Self::ecart_angulaire(Self::hex_face_mid_rad(*a), axe).abs();
                let db = Self::ecart_angulaire(Self::hex_face_mid_rad(*b), axe).abs();
                da.partial_cmp(&db).unwrap()
            })
            .unwrap_or(0)
    }

    /// Angle du milieu de la face `f` de l'hexagone (rad). Les sommets sont à
    /// `30° + 60°·i`, donc les milieux à `60°·(i+1)`.
    #[must_use]
    pub fn hex_face_mid_rad(f: usize) -> f32 {
        (TAU / 6.0) * (f as f32 + 1.0)
    }

    /// Les faces percées, une par part.
    #[must_use]
    pub fn doored_hex_faces(&self) -> Vec<usize> {
        (0..self.count).map(|i| self.doored_hex_face(i)).collect()
    }

    /// Centre d'une porte : au milieu de sa face, sur l'apothème.
    #[must_use]
    pub fn door_center(&self, i: u32) -> Vec2 {
        let a = Self::hex_face_mid_rad(self.doored_hex_face(i));
        Vec2::new(a.cos(), a.sin()) * self.apothem_m()
    }

    /// Ce qui reste d'une face percée : deux demi-panneaux, en
    /// `(demi-longueur, décalage depuis le milieu de la face)`.
    ///
    /// Le décalage est SIGNÉ par le caller (±). Une porte plus large que la face
    /// rend deux panneaux de longueur nulle — la face entière est ouverte, ce qui
    /// est un défaut de dimensionnement mais pas une panne : autant que ça se
    /// voie plutôt que ça panique.
    #[must_use]
    pub fn pierced_panels(&self) -> (f32, f32) {
        let l = self.hex_side_len_m();
        let w = self.door_width_m.min(l);
        ((l - w) * 0.25, (l + w) * 0.25)
    }

    /// Le module visuel posé à la fraction `t` de la face tombe-t-il dans la
    /// porte ?
    ///
    /// `t` suit la convention de `ramparts_hex_tiled_positions` : `(j+0.5)/n`.
    /// Le demi-module entre dans le test — un module qui CHEVAUCHE l'ouverture
    /// la rétrécirait, et « le nom est un contrat » (`map-design-intention` §5.1).
    #[must_use]
    pub fn module_in_door(&self, t: f32, module_len_m: f32) -> bool {
        let depuis_le_milieu = (t - 0.5).abs() * self.hex_side_len_m();
        depuis_le_milieu < self.door_width_m * 0.5 + module_len_m * 0.5
    }

    /// Passage utile RÉELLEMENT laissé une fois les modules retirés (m).
    ///
    /// **Se mesure, ne se suppose pas.** On enlève des modules ENTIERS : le trou
    /// obtenu n'est donc pas l'ouverture nominale, il est plus large. C'est ce
    /// nombre-là que l'agent franchit.
    #[must_use]
    pub fn measured_door_width_m(&self, modules_per_face: u32, module_len_m: f32) -> f32 {
        let n = modules_per_face.max(1);
        let retires = (0..n)
            .filter(|j| {
                let t = (*j as f32 + 0.5) / n as f32;
                self.module_in_door(t, module_len_m)
            })
            .count();
        if retires == 0 {
            return 0.0;
        }
        // Les modules retirés sont contigus (le test est un intervalle centré) :
        // le trou vaut leur longueur cumulée.
        retires as f32 * (self.hex_side_len_m() / n as f32)
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

    /// La disposition de story-703 : trois parts, et une porte percee dans
    /// l'enceinte pour chacune, dimensionnee pour le plus GROS agent du jeu
    /// (le boss, rayon 1,40 m). `outer_radius_m` = l'`extent` de forge_sanctum.
    fn camembert() -> SectorLayout {
        SectorLayout {
            count: 3,
            outer_radius_m: 80.0,
            door_width_m: SectorLayout::door_width_for(1.40),
        }
    }

    // AUCUNE constante de longueur de module ici — deux versions s'y sont
    // trompées, et la seconde était pire que la première parce qu'elle avait
    // l'air sourcée. Les tests balaient la plage plausible : voir
    // `le_passage_mesure_n_est_jamais_plus_etroit_que_la_porte_nominale`.

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

    // ── Les portes percées dans l'enceinte ──────────────────────────────

    #[test]
    fn les_portes_tombent_sur_une_face_d_hexagone_sur_deux() {
        // « Faut qu'ils soient dans 3 directions opposees » : les milieux des 6
        // faces sont a 0/60/120/180/240/300°, donc trois portes a 120° sont
        // exactement une face sur deux. La geometrie du kit donne la reponse —
        // on ne la choisit pas.
        let l = camembert();
        let mut faces = l.doored_hex_faces();
        faces.sort_unstable();
        assert_eq!(faces.len(), 3);
        for w in faces.windows(2) {
            assert_eq!(w[1] - w[0], 2, "faces {faces:?} : ce n'est pas une sur deux");
        }
    }

    #[test]
    fn deux_parts_ne_reclament_jamais_la_meme_face() {
        // Sans ce garde, deux parts pourraient partager une porte et la
        // troisieme n'en aurait aucune — un pack enferme dehors, en silence.
        let l = camembert();
        let faces = l.doored_hex_faces();
        let mut vues = faces.clone();
        vues.sort_unstable();
        vues.dedup();
        assert_eq!(vues.len(), faces.len(), "faces partagees : {faces:?}");
    }

    #[test]
    fn les_portes_sont_bien_a_cent_vingt_degres_les_unes_des_autres() {
        // « Trois directions opposees », verifie en ANGLES et pas en indices.
        let l = camembert();
        let mut angles: Vec<f32> = l
            .doored_hex_faces()
            .iter()
            .map(|f| SectorLayout::hex_face_mid_rad(*f).to_degrees().rem_euclid(360.0))
            .collect();
        angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("PORTES a {angles:?} deg");
        for w in angles.windows(2) {
            assert!((w[1] - w[0] - 120.0).abs() < 1.0, "ecart {:.0} deg", w[1] - w[0]);
        }
    }

    #[test]
    fn une_porte_se_perce_au_milieu_d_une_face_pas_a_un_sommet() {
        // Le milieu d'une face est a l'APOTHEME (0,866 x rayon), pas au rayon
        // circonscrit. Confondre les deux poserait la porte 10 m trop loin, dans
        // le vide hors de l'enceinte.
        let l = camembert();
        for i in 0..l.count {
            let c = l.door_center(i);
            assert!(
                (c.length() - l.apothem_m()).abs() < 1.0e-3,
                "porte {i} a {:.2} m, apotheme {:.2} m",
                c.length(),
                l.apothem_m()
            );
            assert!(c.length() < l.outer_radius_m, "une porte ne se perce pas a un sommet");
        }
    }

    #[test]
    fn les_deux_demi_panneaux_couvrent_la_face_moins_la_porte() {
        // Conservation de la matiere : ce qui reste + l'ouverture = la face.
        let l = camembert();
        let (demi, decalage) = l.pierced_panels();
        let reste = 2.0 * (2.0 * demi);
        assert!(
            (reste + l.door_width_m - l.hex_side_len_m()).abs() < 1.0e-3,
            "reste {reste:.2} + porte {:.2} != face {:.2}",
            l.door_width_m,
            l.hex_side_len_m()
        );
        // Le panneau doit etre COLLE a l'ouverture, ni chevauchant ni ecarte.
        assert!(
            (decalage - demi - l.door_width_m * 0.5).abs() < 1.0e-3,
            "panneau mal place : decalage {decalage:.2}, demi {demi:.2}"
        );
    }

    #[test]
    fn une_porte_plus_large_que_la_face_ne_panique_pas() {
        // Defaut de dimensionnement possible (un agent enorme, une petite arene).
        // Il doit se VOIR — face entierement ouverte — pas paniquer.
        let l = SectorLayout {
            door_width_m: 999.0,
            ..camembert()
        };
        let (demi, _) = l.pierced_panels();
        assert!(demi.abs() < 1.0e-3, "il ne devrait rien rester du panneau");
    }

    #[test]
    fn le_passage_mesure_n_est_jamais_plus_etroit_que_la_porte_nominale() {
        // LE test qui compte. On retire des modules ENTIERS : le trou obtenu
        // n'est pas l'ouverture nominale, il est plus large. C'est ce trou-la que
        // l'agent franchit, donc c'est lui qu'on compare a son gabarit.
        //
        // # Le test BALAIE au lieu de fixer une longueur de module
        //
        // Deux versions se sont trompees sur cette constante. D'abord 8,0 m,
        // devine. Puis 4,0 m, lu dans `kaykit_dungeon` — exact, mais pour un
        // AUTRE kit que celui de forge_sanctum. Mesure en jeu : 66 modules sur
        // 6 faces, soit 11 par face, donc 80/11 = 7,27 m.
        //
        // Remplacer un nombre invente par un nombre exact pour le mauvais objet
        // est PIRE que le deviner : ca a l'air source. Le test ne fixe donc plus
        // aucune longueur — il verifie la PROPRIETE sur toute la plage plausible,
        // et reste juste quel que soit le kit.
        let l = camembert();
        for module_len in [2.0_f32, 4.0, 6.0, 7.27, 8.0, 12.0] {
            let n = ((l.hex_side_len_m() / module_len).ceil() as u32).max(1);
            let mesure = l.measured_door_width_m(n, module_len);
            println!(
                "PORTE : modules de {module_len:.2} m ({n}/face) -> passage {mesure:.2} m \
                 pour {:.2} nominal",
                l.door_width_m
            );
            assert!(
                mesure >= l.door_width_m,
                "modules de {module_len} m : {mesure:.2} m mesures pour {:.2} annonces",
                l.door_width_m
            );
            assert!(
                mesure > 0.0,
                "modules de {module_len} m : aucun module retire, il n'y a pas de porte"
            );
        }
    }

    #[test]
    fn le_passage_mesure_admet_le_boss_quel_que_soit_le_kit() {
        // Meme discipline : la garantie doit tenir pour toute taille de module,
        // pas pour celle que j'aurais devinee.
        let l = camembert();
        let requis = SectorLayout::door_width_for(1.40);
        for module_len in [2.0_f32, 4.0, 7.27, 12.0] {
            let n = ((l.hex_side_len_m() / module_len).ceil() as u32).max(1);
            assert!(
                l.measured_door_width_m(n, module_len) >= requis,
                "le boss ne passe pas avec des modules de {module_len} m"
            );
        }
    }

    #[test]
    fn la_mesure_reagit_a_la_taille_des_modules() {
        // Le test negatif : une fonction qui rendrait docilement le nominal
        // passerait tous les tests positifs sans rien mesurer.
        let l = camembert();
        let fin = l.measured_door_width_m(40, 2.0);
        let grossier = l.measured_door_width_m(4, 20.0);
        assert!(
            (fin - grossier).abs() > 1.0,
            "la mesure ne reagit pas au pas des modules ({fin:.2} vs {grossier:.2})"
        );
    }

    #[test]
    fn un_module_qui_chevauche_l_ouverture_est_retire() {
        // Garder un module a cheval retrecirait le passage sous sa valeur
        // annoncee — « le nom est un contrat ».
        let l = camembert();
        let t = 0.5 + (l.door_width_m * 0.5) / l.hex_side_len_m();
        assert!(l.module_in_door(t, 4.0), "module a cheval conserve");
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
}
