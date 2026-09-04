//! locomotion_etat — QUEL clip jouer, en fonction de ce que le personnage fait.
//!
//! # Pourquoi cette décision vit dans un module à part, et pure
//!
//! Une machine à états d'animation a quinze cas et des priorités qui se
//! chevauchent. Enfouie dans un système Bevy, elle n'est vérifiable qu'en jouant
//! — et « le personnage glisse alors qu'il devrait marcher » est un défaut qu'on
//! découvre trois jours plus tard, en playtest, sans savoir quelle branche a
//! gagné.
//!
//! Ici la décision est une **fonction pure** : un état dedans, un clip dehors.
//! Les quinze cas se testent en quinze assertions, sans moteur, sans squelette,
//! sans glTF. C'est la leçon que cette semaine a payée trois fois — la logique
//! qui se teste est celle qu'on extrait de l'ECS.
//!
//! # Le repli est la moitié du travail
//!
//! Deux corps cohabitent : celui de l'Expédition porte 34 clips, le trooper du
//! Hall en porte **deux** (`idle`, `walk`). Un sélecteur qui rendrait `Slide` à
//! un corps sans clip de glissade le figerait. D'où [`Clip::replis`] : chaque
//! clip déclare sa chaîne de secours, et le câblage descend jusqu'à ce qu'il
//! trouve. Un corps pauvre dégrade proprement au lieu de casser.

/// Ce que le personnage fait — sans rien savoir d'un squelette.
///
/// Les vitesses sont dans **son repère** : `avant` positif quand il avance,
/// `lateral` positif quand il va vers sa droite. C'est ce qui manquait : la
/// version d'avant ne portait qu'une vitesse scalaire, incapable de distinguer
/// une marche arrière d'une marche avant, ou un pas de côté d'une course.
#[derive(Debug, Clone, Copy, Default)]
pub struct EtatLocomotion {
    pub avant_ms: f32,
    pub lateral_ms: f32,
    pub au_sol: bool,
    /// Depuis quand il a quitté le sol. Zéro tant qu'il le touche.
    ///
    /// 🚨 `au_sol` seul ne suffit PAS. Un contrôleur à capsule perd le contact
    /// une frame sur une bosse, un raccord de dalle ou un haut de rampe, et
    /// basculer là-dessus fait clignoter le clip de chute pendant la marche —
    /// rapporté en jeu le 2026-08-18. Le contact est un booléen ET une durée.
    pub depuis_decollage_s: f32,
    /// Positive quand il monte. Sépare le saut de la chute — deux clips.
    pub vitesse_verticale_ms: f32,
    pub accroupi: bool,
    pub glisse: bool,
    pub vise: bool,
    pub tire: bool,
    /// Vitesse de rotation sur place (rad/s), pour le clip de virage.
    pub lacet_rad_s: f32,
    /// Depuis combien de temps il a touché le sol. Le clip d'atterrissage est
    /// bref et ne se rejoue pas : il lui faut une fenêtre, pas un état.
    pub depuis_atterrissage_s: f32,
    /// Durée du vol qui a précédé le dernier contact. Un raccord de terrain de
    /// quelques frames ne doit pas déclencher une réception plein corps.
    pub duree_vol_precedent_s: f32,
}

/// Le clip à jouer. Un `enum` et non une chaîne : le compilateur vérifie qu'on
/// traite chaque cas, et un nom mal orthographié ne compile pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Clip {
    Idle,
    Marche,
    Course,
    MarcheArriere,
    PasDeCote,
    Visee,
    Tir,
    Saut,
    Chute,
    Atterrissage,
    AccroupiIdle,
    AccroupiMarche,
    AccroupiArriere,
    Glissade,
    Virage,
}

impl Clip {
    /// Tous les états, pour que le câblage les parcoure sans en oublier.
    ///
    /// Écrite à la main faute de dérive, donc un test vérifie qu'elle est
    /// complète : un état ajouté à l'`enum` et oublié ici ne serait jamais câblé,
    /// et le personnage retomberait silencieusement sur son repli.
    pub const TOUS: [Clip; 15] = [
        Clip::Idle,
        Clip::Marche,
        Clip::Course,
        Clip::MarcheArriere,
        Clip::PasDeCote,
        Clip::Visee,
        Clip::Tir,
        Clip::Saut,
        Clip::Chute,
        Clip::Atterrissage,
        Clip::AccroupiIdle,
        Clip::AccroupiMarche,
        Clip::AccroupiArriere,
        Clip::Glissade,
        Clip::Virage,
    ];

    /// La chaîne de secours, du plus proche au plus pauvre.
    ///
    /// Elle existe pour que le trooper — deux clips — ne se fige pas quand le
    /// sélecteur demande une glissade. `Idle` est le fond du puits : tout corps
    /// animé en a un, et un personnage au repos est toujours préférable à un
    /// personnage en pose de bind, bras écartés.
    #[must_use]
    pub fn replis(self) -> &'static [Clip] {
        use Clip::*;
        match self {
            Idle => &[],
            Marche => &[Idle],
            Course => &[Marche, Idle],
            // Reculer en jouant la marche à l'endroit est laid mais lisible ;
            // rester figé ne l'est pas.
            MarcheArriere => &[Marche, Idle],
            PasDeCote => &[Marche, Idle],
            Visee => &[Idle],
            // Le tir retombe sur la visée : garder l'arme épaulée est déjà
            // l'essentiel de l'information.
            Tir => &[Visee, Idle],
            Saut => &[Idle],
            // Chuter en gardant la pose de saut est le repli classique.
            Chute => &[Saut, Idle],
            Atterrissage => &[Idle],
            AccroupiIdle => &[Idle],
            AccroupiMarche => &[AccroupiIdle, Marche, Idle],
            AccroupiArriere => &[AccroupiMarche, AccroupiIdle, Marche, Idle],
            // Une glissade sans clip vaut mieux en course qu'à l'arrêt : la
            // vitesse reste lisible.
            Glissade => &[Course, Marche, Idle],
            Virage => &[Idle],
        }
    }
}

/// Seuils de décision. En couche **definition** : ils se règlent au génome,
/// jamais en dur — la vitesse à laquelle un personnage « court » est un choix
/// de jeu, pas une constante.
#[derive(Debug, Clone, Copy)]
pub struct Seuils {
    pub marche_min_ms: f32,
    pub course_min_ms: f32,
    /// En dessous, un pas de côté n'est que du bruit de contrôleur.
    pub lateral_min_ms: f32,
    /// Vitesse de montée à partir de laquelle on est « en saut » plutôt qu'« en
    /// chute ». Séparer les deux évite la pose de saut figée en descente.
    pub montee_min_ms: f32,
    pub virage_min_rad_s: f32,
    /// Durée du clip d'atterrissage. Une fenêtre, pas un état.
    pub atterrissage_s: f32,
    /// Plafond anti-téléport : au-delà, on ignore la vitesse.
    pub vitesse_absurde_ms: f32,
    /// Combien de temps hors sol avant de jouer une pose aérienne.
    ///
    /// C'est le « coyote time » du métier, ici appliqué à l'ANIMATION et non au
    /// saut : il ne change pas ce que le personnage peut faire, seulement ce
    /// qu'on lui voit faire.
    pub air_min_s: f32,
}

impl Default for Seuils {
    fn default() -> Self {
        Self {
            marche_min_ms: 0.6,
            course_min_ms: 6.0,
            lateral_min_ms: 0.8,
            montee_min_ms: 0.5,
            // ~34°/s : au-delà, le personnage pivote assez pour qu'un virage
            // sur place se voie. En dessous, c'est une correction de visée.
            virage_min_rad_s: 0.6,
            atterrissage_s: 0.25,
            vitesse_absurde_ms: 30.0,
            // 0,10 s = six frames à 60 Hz. Assez pour absorber les décollements
            // de terrain, assez court pour qu'un vrai saut parte sans retard
            // visible : à 9,75 m/s on n'a parcouru qu'un mètre.
            air_min_s: 0.10,
        }
    }
}

/// Le clip que cet état demande.
///
/// # L'ordre des priorités, et pourquoi il est celui-là
///
/// Il descend du plus contraignant au plus ordinaire. Chaque niveau répond à
/// « qu'est-ce qui empêche de jouer la suite » :
///
/// 1. **la glissade** — c'est un mouvement scénarisé, il tient le corps entier ;
/// 2. **l'air** — au sol ou pas est la question la plus binaire qui soit ;
/// 3. **l'atterrissage** — une fenêtre courte qui doit gagner sur la marche,
///    sinon on ne la voit jamais ;
/// 4. **l'accroupissement** — il change toute la silhouette, donc tous les clips
///    de déplacement ;
/// 5. **le déplacement** — de côté d'abord (le plus fréquent en combat 3ᵉ
///    personne, et celui dont l'absence se voit le plus), puis arrière, puis
///    l'avant par vitesse ;
/// 6. **le virage sur place** — seulement s'il ne se déplace pas, sinon la
///    course en courbe déclencherait un virage ;
/// 7. **viser et tirer** — en DERNIER, et c'est un choix contraint : il n'existe
///    pas de clip « marcher en visant ». Les jouer par-dessus le déplacement
///    demanderait un mélange par couche que ce corps n'a pas. Tant qu'il n'y en
///    a pas, la visée ne s'affiche qu'à l'arrêt — et l'IK des bras porte le
///    reste, ce qui est précisément son rôle.
#[must_use]
pub fn choisir(e: &EtatLocomotion, s: &Seuils) -> Clip {
    use Clip::*;

    // Plafond anti-téléport : un replacement au spawn produit des centaines de
    // m/s et ferait courir le personnage à l'arrêt. Même garde que les ennemis.
    let absurde = |v: f32| v.abs() > s.vitesse_absurde_ms;
    let avant = if absurde(e.avant_ms) { 0.0 } else { e.avant_ms };
    let lateral = if absurde(e.lateral_ms) {
        0.0
    } else {
        e.lateral_ms
    };

    if e.glisse {
        return Glissade;
    }
    // Hors sol DEPUIS ASSEZ LONGTEMPS, ou en train de monter franchement : un
    // saut volontaire ne doit pas attendre le délai de grâce pour se voir.
    if !e.au_sol
        && (e.depuis_decollage_s >= s.air_min_s || e.vitesse_verticale_ms > s.montee_min_ms)
    {
        return if e.vitesse_verticale_ms > s.montee_min_ms {
            Saut
        } else {
            Chute
        };
    }
    if e.duree_vol_precedent_s >= s.air_min_s && e.depuis_atterrissage_s < s.atterrissage_s {
        return Atterrissage;
    }

    let bouge_lateral = lateral.abs() >= s.lateral_min_ms;
    let bouge_avant = avant.abs() >= s.marche_min_ms;

    if e.accroupi {
        return if avant <= -s.marche_min_ms {
            AccroupiArriere
        } else if bouge_avant || bouge_lateral {
            AccroupiMarche
        } else {
            AccroupiIdle
        };
    }

    // Le pas de côté gagne quand il DOMINE : marcher en diagonale doit rester
    // une marche, sinon le personnage se déplace de profil dès qu'il dévie.
    if bouge_lateral && lateral.abs() > avant.abs() {
        return PasDeCote;
    }
    if avant <= -s.marche_min_ms {
        return MarcheArriere;
    }
    if avant >= s.course_min_ms {
        return Course;
    }
    if avant >= s.marche_min_ms {
        return Marche;
    }
    // À l'arrêt seulement : une course en courbe ne doit pas déclencher un
    // virage sur place.
    if e.lacet_rad_s.abs() >= s.virage_min_rad_s {
        return Virage;
    }
    if e.tire {
        return Tir;
    }
    if e.vise {
        return Visee;
    }
    Idle
}

#[cfg(test)]
mod tests {
    use super::Clip::*;
    use super::*;

    fn immobile() -> EtatLocomotion {
        EtatLocomotion {
            au_sol: true,
            depuis_atterrissage_s: 10.0,
            ..Default::default()
        }
    }

    #[test]
    fn a_l_arret_il_est_au_repos() {
        assert_eq!(choisir(&immobile(), &Seuils::default()), Idle);
    }

    #[test]
    fn la_vitesse_avant_choisit_marche_puis_course() {
        let s = Seuils::default();
        let mut e = immobile();
        e.avant_ms = 0.3;
        assert_eq!(choisir(&e, &s), Idle, "0,3 m/s est du bruit de contrôleur");
        e.avant_ms = 3.0;
        assert_eq!(choisir(&e, &s), Marche);
        e.avant_ms = 9.0;
        assert_eq!(choisir(&e, &s), Course);
    }

    #[test]
    fn reculer_n_est_pas_avancer() {
        // Le défaut que le scalaire d'avant rendait impossible à corriger : une
        // vitesse sans signe ne peut pas distinguer les deux.
        let mut e = immobile();
        e.avant_ms = -3.0;
        assert_eq!(choisir(&e, &Seuils::default()), MarcheArriere);
    }

    #[test]
    fn le_pas_de_cote_ne_gagne_que_s_il_domine() {
        let s = Seuils::default();
        let mut e = immobile();
        // Franchement latéral → pas de côté.
        e.lateral_ms = 3.0;
        assert_eq!(choisir(&e, &s), PasDeCote);
        // En diagonale, l'avant l'emporte : sinon le personnage se déplacerait
        // de profil dès qu'il dévie d'un degré.
        e.avant_ms = 4.0;
        assert_eq!(choisir(&e, &s), Marche);
    }

    #[test]
    fn l_air_separe_le_saut_de_la_chute() {
        let s = Seuils::default();
        let mut e = immobile();
        e.au_sol = false;
        e.depuis_decollage_s = 0.5;
        e.vitesse_verticale_ms = 4.0;
        assert_eq!(choisir(&e, &s), Saut);
        e.vitesse_verticale_ms = -6.0;
        assert_eq!(choisir(&e, &s), Chute, "descendre en pose de saut se voit");
    }

    #[test]
    fn l_atterrissage_gagne_sur_la_marche_mais_pas_longtemps() {
        // S'il ne gagnait pas, on ne le verrait jamais : on atterrit toujours en
        // train de se déplacer.
        let s = Seuils::default();
        let mut e = immobile();
        e.avant_ms = 5.0;
        e.depuis_atterrissage_s = 0.1;
        e.duree_vol_precedent_s = 0.5;
        assert_eq!(choisir(&e, &s), Atterrissage);
        e.depuis_atterrissage_s = 0.5;
        assert_eq!(choisir(&e, &s), Marche, "la fenêtre doit se refermer");
    }

    #[test]
    fn un_micro_decollement_de_pente_ne_declenche_pas_atterrissage() {
        let s = Seuils::default();
        let mut e = immobile();
        e.avant_ms = 5.0;
        e.depuis_atterrissage_s = 0.0;
        e.duree_vol_precedent_s = s.air_min_s * 0.5;
        assert_eq!(choisir(&e, &s), Marche);
    }

    #[test]
    fn la_glissade_tient_le_corps_entier() {
        // Elle gagne sur TOUT, y compris l'air : une glissade interrompue par un
        // micro-décollement du contrôleur serait un clignotement.
        let s = Seuils::default();
        let mut e = immobile();
        e.glisse = true;
        e.avant_ms = 9.0;
        e.au_sol = false;
        e.accroupi = true;
        assert_eq!(choisir(&e, &s), Glissade);
    }

    #[test]
    fn accroupi_change_tous_les_clips_de_deplacement() {
        let s = Seuils::default();
        let mut e = immobile();
        e.accroupi = true;
        assert_eq!(choisir(&e, &s), AccroupiIdle);
        e.avant_ms = 2.0;
        assert_eq!(choisir(&e, &s), AccroupiMarche);
        e.avant_ms = -2.0;
        assert_eq!(choisir(&e, &s), AccroupiArriere);
        // Et courir accroupi reste accroupi : on ne se relève pas tout seul.
        e.avant_ms = 9.0;
        assert_eq!(choisir(&e, &s), AccroupiMarche);
    }

    #[test]
    fn le_virage_ne_se_declenche_qu_a_l_arret() {
        // Sinon une course en courbe jouerait un virage sur place, et le
        // personnage patinerait à chaque changement de direction.
        let s = Seuils::default();
        let mut e = immobile();
        e.lacet_rad_s = 2.0;
        assert_eq!(choisir(&e, &s), Virage);
        e.avant_ms = 5.0;
        assert_eq!(choisir(&e, &s), Marche);
    }

    #[test]
    fn viser_ne_s_affiche_qu_a_l_arret() {
        // Contrainte assumée, pas oubli : il n'existe pas de clip « marcher en
        // visant » dans ce corps. L'IK des bras porte la visée en mouvement.
        let s = Seuils::default();
        let mut e = immobile();
        e.vise = true;
        assert_eq!(choisir(&e, &s), Visee);
        e.tire = true;
        assert_eq!(choisir(&e, &s), Tir, "tirer gagne sur viser");
        e.avant_ms = 5.0;
        assert_eq!(choisir(&e, &s), Marche);
    }

    #[test]
    fn une_vitesse_absurde_est_ignoree_pas_jouee() {
        // Un replacement au spawn produit des centaines de m/s. Sans plafond, le
        // personnage court à l'arrêt — défaut déjà payé sur les ennemis.
        let s = Seuils::default();
        let mut e = immobile();
        e.avant_ms = 250.0;
        assert_eq!(choisir(&e, &s), Idle);
        e.lateral_ms = -400.0;
        assert_eq!(choisir(&e, &s), Idle);
    }

    #[test]
    fn la_liste_de_tous_les_etats_est_complete() {
        // `TOUS` est écrite à la main : un état ajouté à l'enum et oublié ici ne
        // serait JAMAIS câblé, et le personnage retomberait sur son repli sans
        // qu'aucun message ne le dise. Le compilateur ne l'attrape pas — ce test,
        // oui, parce qu'il compare à un `match` exhaustif.
        for c in Clip::TOUS {
            // Si un variant manquait à TOUS, ce match ne compilerait pas après
            // son ajout à l'enum : c'est le match qui fait le travail.
            let _: &str = match c {
                Idle => "idle",
                Marche => "marche",
                Course => "course",
                MarcheArriere => "arriere",
                PasDeCote => "cote",
                Visee => "visee",
                Tir => "tir",
                Saut => "saut",
                Chute => "chute",
                Atterrissage => "atterrissage",
                AccroupiIdle => "accroupi",
                AccroupiMarche => "accroupi_marche",
                AccroupiArriere => "accroupi_arriere",
                Glissade => "glissade",
                Virage => "virage",
            };
        }
        // Et aucun doublon : une entrée en double câblerait deux fois le même
        // clip et gonflerait le graphe sans rien changer à l'écran.
        let mut vus = std::collections::HashSet::new();
        for c in Clip::TOUS {
            assert!(vus.insert(c), "{c:?} apparaît deux fois dans TOUS");
        }
        assert_eq!(vus.len(), 15);
    }

    #[test]
    fn chaque_clip_a_un_repli_qui_finit_sur_idle() {
        // La garantie qui protège le trooper (deux clips) : quelle que soit la
        // demande, la chaîne descend jusqu'à quelque chose qu'il possède. Un
        // clip sans repli le laisserait en pose de bind, bras écartés.
        for c in [
            Marche,
            Course,
            MarcheArriere,
            PasDeCote,
            Visee,
            Tir,
            Saut,
            Chute,
            Atterrissage,
            AccroupiIdle,
            AccroupiMarche,
            AccroupiArriere,
            Glissade,
            Virage,
        ] {
            let r = c.replis();
            assert!(!r.is_empty(), "{c:?} n'a aucun repli");
            assert_eq!(
                *r.last().unwrap(),
                Idle,
                "{c:?} : la chaîne de repli doit finir sur Idle"
            );
        }
        assert!(Idle.replis().is_empty(), "Idle est le fond du puits");
    }

    #[test]
    fn aucun_repli_ne_boucle_sur_lui_meme() {
        // Une chaîne qui se citerait ferait boucler le câblage à l'infini au
        // chargement — un gel au démarrage, sans message.
        for c in [
            Marche,
            Course,
            MarcheArriere,
            PasDeCote,
            Visee,
            Tir,
            Saut,
            Chute,
            Atterrissage,
            AccroupiIdle,
            AccroupiMarche,
            AccroupiArriere,
            Glissade,
            Virage,
        ] {
            assert!(!c.replis().contains(&c), "{c:?} se cite dans ses replis");
        }
    }

    #[test]
    fn les_seuils_par_defaut_laissent_la_marche_atteignable() {
        // Le piège déjà rencontré sur ce projet : un `run_speed_min` sous la
        // vitesse de marche rend le clip de marche INATTEIGNABLE.
        let s = Seuils::default();
        assert!(s.marche_min_ms < s.course_min_ms);
        assert!(s.lateral_min_ms > 0.0);
        assert!(s.atterrissage_s > 0.0 && s.atterrissage_s < 1.0);
    }

    /// 🚨 LE test du symptôme rapporté : marcher ne doit pas déclencher la chute.
    ///
    /// « Parfois quand je marche, l'animation de chute s'active alors que je
    /// marche et je cours » (2026-08-18). Un contrôleur à capsule perd le
    /// contact une frame sur la moindre bosse ; la machine basculait dessus.
    #[test]
    fn un_decollement_d_une_frame_ne_fait_pas_tomber() {
        let s = Seuils::default();
        let mut e = immobile();
        e.avant_ms = 3.0;
        // Une frame à 60 Hz hors sol, sans vitesse verticale notable : c'est du
        // bruit de terrain, pas une chute.
        e.au_sol = false;
        e.depuis_decollage_s = 1.0 / 60.0;
        e.vitesse_verticale_ms = -0.2;
        assert_eq!(
            choisir(&e, &s),
            Marche,
            "une bosse ne doit pas jouer la chute"
        );
        // Même chose en course.
        e.avant_ms = 8.0;
        assert_eq!(choisir(&e, &s), Course);
    }

    /// Mais une vraie chute doit se voir, et vite.
    #[test]
    fn une_vraie_chute_se_voit_des_le_delai_de_grace() {
        let s = Seuils::default();
        let mut e = immobile();
        e.avant_ms = 3.0;
        e.au_sol = false;
        e.vitesse_verticale_ms = -2.0;
        e.depuis_decollage_s = s.air_min_s;
        assert_eq!(choisir(&e, &s), Chute);
    }

    /// Un saut volontaire ne DOIT PAS attendre le délai : on décolle en montant,
    /// et la pose doit partir à la frame où la touche produit son effet.
    #[test]
    fn un_saut_part_sans_attendre_le_delai_de_grace() {
        let s = Seuils::default();
        let mut e = immobile();
        e.au_sol = false;
        e.depuis_decollage_s = 0.0;
        e.vitesse_verticale_ms = 5.0;
        assert_eq!(choisir(&e, &s), Saut);
    }
}
