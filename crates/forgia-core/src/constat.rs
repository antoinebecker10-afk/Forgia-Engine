//! `Constat` — le verdict d'un capteur, dont la forme rend trois défauts
//! impossibles au lieu de détectables.
//!
//! # Pourquoi ce type existe
//!
//! Audit du 2026-08-18 sur les 132 capteurs du dépôt :
//!
//! | Défaut | Compte | Ce que ça coûte |
//! |---|---:|---|
//! | écrit par `fs::write` au lieu de la file | **8** | bloque la frame de jeu |
//! | ne publie AUCUNE `severity` | **32** | on ne sait pas s'il va bien |
//! | rend `ok` sans jamais dire ce qu'il a mesuré | *majorité* | un vert d'échantillon vide se lit comme un vert de mesure |
//!
//! Quarante-cinq défauts pour trois formes seulement. Quand le même défaut se
//! répète quarante-cinq fois, ce ne sont pas quarante-cinq erreurs d'auteurs —
//! **c'est l'API qui est mauvaise**. Écrire un capteur correct demandait de
//! savoir, de tête : passer par `enqueue`, publier une sévérité, fournir un
//! remède, et ne pas rendre `ok` sur un échantillon vide. Rien ne le rappelait.
//!
//! # Ce que la forme garantit
//!
//! - **L'écriture asynchrone est le seul chemin.** `publier()` passe par
//!   `sensor_io::enqueue` ; il n'y a pas d'autre sortie.
//! - **Une alerte sans remède NE COMPILE PAS.** `alerte()` rend un
//!   `Constat<SansRemede>`, et `publier()` n'existe que sur `Constat<Pret>` —
//!   seul `.remede()` fait passer de l'un à l'autre.
//! - **Zéro mesuré n'est jamais vert.** `.echantillon(0)` dégrade un `ok` en
//!   `info` et l'annonce. C'est la règle §13 des patterns de carte
//!   (« zéro mesuré n'est pas vert, c'est aveugle »), écrite depuis des mois et
//!   violée le soir même de sa énième relecture : mon propre contrat de mode
//!   rendait `ok` alors que ses trois caméras étaient toutes exemptées.
//!
//! # Usage
//!
//! ```ignore
//! use forgia_core::constat;
//!
//! let c = if lecteurs == 0 {
//!     constat::info("aucun lecteur d'animation — rien a mesurer")
//! } else if muets > 0 {
//!     constat::alerte(format!("{muets} lecteur(s) sans clip actif"))
//!         .remede("verifier le graphe d'animation du corps monte")
//! } else {
//!     constat::ok()
//! };
//! c.echantillon(lecteurs)
//!     .publier("animation", temps, &format!(r#""lecteurs":{lecteurs}"#));
//! ```
//!
//! # Ce que ce type ne fait PAS
//!
//! Il ne décide pas de la sévérité à ta place et ne sait pas ce qu'est un bon
//! seuil. Il garantit la FORME du verdict, pas sa justesse — un capteur qui
//! vise à côté rendra un `Constat` impeccable sur une mesure fausse.

use crate::sensor_io;
use std::marker::PhantomData;

/// Les quatre verdicts, dans l'ordre où le digest les trie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severite {
    /// Mesuré, et tout va bien.
    Ok,
    /// Rien à mesurer ici — ce n'est PAS un feu vert.
    Info,
    /// Défaut réel, le jeu tourne encore.
    Warn,
    /// Défaut qui casse ou va casser.
    Critical,
}

impl Severite {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Critical => "critical",
        }
    }
}

/// État de type : il manque le remède, `publier()` n'existe pas encore.
pub struct SansRemede;
/// État de type : publiable.
pub struct Pret;

/// Le verdict d'un capteur, en construction.
///
/// Le paramètre `E` porte l'état : une alerte naît en `SansRemede` et ne
/// devient `Pret` que par `.remede()`. C'est ce qui rend « alerte sans remède »
/// une erreur de compilation plutôt qu'une ligne de plus au tableau.
pub struct Constat<E = Pret> {
    severite: Severite,
    message: String,
    remede: String,
    echantillon: Option<usize>,
    _etat: PhantomData<E>,
}

/// Mesuré, et tout va bien.
///
/// Pense à `.echantillon(n)` : sans lui, personne ne saura sur quoi porte ce
/// vert — et avec `n == 0` il se dégradera tout seul, ce qui est le but.
#[must_use]
pub fn ok() -> Constat<Pret> {
    Constat {
        severite: Severite::Ok,
        message: String::new(),
        remede: String::new(),
        echantillon: None,
        _etat: PhantomData,
    }
}

/// Rien à mesurer — au menu, hors du mode concerné, avant le chargement.
///
/// 🚨 À utiliser au lieu de `ok()` quand l'absence est normale. Un `ok` sur du
/// vide fait passer « je n'ai rien vu » pour « j'ai vu que tout va bien ».
#[must_use]
pub fn info(message: impl Into<String>) -> Constat<Pret> {
    Constat {
        severite: Severite::Info,
        message: message.into(),
        remede: String::new(),
        echantillon: None,
        _etat: PhantomData,
    }
}

/// Un défaut réel. **Exige un `.remede()`** — sinon ça ne compile pas.
#[must_use]
pub fn alerte(message: impl Into<String>) -> Constat<SansRemede> {
    Constat {
        severite: Severite::Warn,
        message: message.into(),
        remede: String::new(),
        echantillon: None,
        _etat: PhantomData,
    }
}

/// Un défaut qui casse. **Exige un `.remede()`** — sinon ça ne compile pas.
#[must_use]
pub fn critique(message: impl Into<String>) -> Constat<SansRemede> {
    Constat {
        severite: Severite::Critical,
        message: message.into(),
        remede: String::new(),
        echantillon: None,
        _etat: PhantomData,
    }
}

impl<E> Constat<E> {
    /// Combien d'éléments ce verdict a-t-il réellement examinés.
    ///
    /// 🚨 `0` dégrade un `ok` en `info`. Ce n'est pas une politesse : c'est la
    /// différence entre « les 3 caméras sont propres » et « il n'y avait aucune
    /// caméra à regarder », que le même mot `ok` confondait.
    #[must_use]
    pub fn echantillon(mut self, n: usize) -> Self {
        self.echantillon = Some(n);
        self
    }
}

impl Constat<SansRemede> {
    /// Ce qu'il faut FAIRE. Sans ça, l'alerte n'est pas publiable.
    ///
    /// Un capteur qui dit « Atmosphere: 2 » laisse chercher ; un capteur qui
    /// dit « bevy exige `.single()`, wgpu paniquera au premier rendu » envoie
    /// au bon endroit du premier coup.
    #[must_use]
    pub fn remede(self, action: impl Into<String>) -> Constat<Pret> {
        Constat {
            severite: self.severite,
            message: self.message,
            remede: action.into(),
            echantillon: self.echantillon,
            _etat: PhantomData,
        }
    }
}

impl Constat<Pret> {
    /// La sévérité effective, après la règle de l'échantillon vide.
    ///
    /// Séparée de `publier` pour être testable sans toucher au disque : c'est
    /// la seule façon de vérifier que la dégradation MORD, au lieu d'espérer.
    #[must_use]
    pub fn verdict(&self) -> (Severite, String) {
        match (self.severite, self.echantillon) {
            (Severite::Ok, Some(0)) => (
                Severite::Info,
                "AVEUGLE : 0 element mesure — ce n'est pas un feu vert, il n'y avait rien a \
                 regarder. Verifier que le producteur tourne et que le mode attendu est actif."
                    .to_string(),
            ),
            _ => {
                let mut m = self.message.clone();
                if !self.remede.is_empty() {
                    if !m.is_empty() {
                        m.push_str(" — ");
                    }
                    m.push_str(&self.remede);
                }
                (self.severite, m)
            }
        }
    }

    /// Écrit le capteur. **Seule sortie** — donc jamais de `fs::write` bloquant.
    ///
    /// `charge` est un fragment JSON déjà formé (sans accolades), collé après
    /// l'enveloppe standard. Vide est valide.
    pub fn publier(self, id: &str, temps_s: f32, charge: &str) {
        let (severite, next_step) = self.verdict();
        let ech = self
            .echantillon
            .map_or_else(|| "null".to_string(), |n| n.to_string());
        let suite = if charge.trim().is_empty() {
            String::new()
        } else {
            format!(",{charge}")
        };
        let json = format!(
            r#"{{"id":"{}","severity":"{}","next_step":"{}","timestamp_secs":{temps_s:.1},"echantillon":{ech}{suite}}}"#,
            echappe(id),
            severite.as_str(),
            echappe(&next_step),
        );
        let _ = sensor_io::enqueue(format!("forgia2_{id}.json"), json);
    }
}

/// Échappe ce qui casserait le JSON. Les capteurs écrivent des chemins Windows
/// (`C:\...`) et des messages entre guillemets : les deux ont déjà produit des
/// fichiers illisibles.
#[must_use]
pub fn echappe(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🚨 LE test qui porte la raison d'être du type. Mon propre contrat de
    /// mode a rendu `ok` le 2026-08-18 avec trois caméras toutes exemptées :
    /// son contrôle n'avait rien mesuré et l'annonçait vert.
    #[test]
    fn zero_mesure_degrade_le_vert_en_aveugle() {
        let (s, m) = ok().echantillon(0).verdict();
        assert_eq!(s, Severite::Info, "0 mesure ne peut pas rester `ok`");
        assert!(m.contains("AVEUGLE"), "et il faut que ca se LISE : {m}");
    }

    /// La dégradation ne doit pas mordre dès qu'il y a de la matière.
    #[test]
    fn un_echantillon_reel_reste_vert() {
        assert_eq!(ok().echantillon(3).verdict().0, Severite::Ok);
    }

    /// Un capteur qui ne déclare rien reste `ok` : on ne peut pas deviner qu'il
    /// est aveugle. C'est la limite assumée — la règle mord sur le `0` déclaré,
    /// pas sur l'omission. Le cliquet xtask s'occupe des omissions.
    #[test]
    fn sans_echantillon_declare_on_ne_devine_pas() {
        assert_eq!(ok().verdict().0, Severite::Ok);
        assert_eq!(ok().verdict().1, "");
    }

    /// Une alerte porte TOUJOURS son remède dans le message publié — c'est ce
    /// que lit le digest, et c'est la seule ligne que quelqu'un verra.
    #[test]
    fn une_alerte_publie_son_remede_avec_son_symptome() {
        let (s, m) = alerte("2 cameras gardent Atmosphere")
            .remede("bevy exige .single() — wgpu paniquera au premier rendu")
            .verdict();
        assert_eq!(s, Severite::Warn);
        assert!(m.contains("2 cameras") && m.contains(".single()"), "{m}");
    }

    /// Le critique reste critique — la dégradation ne concerne QUE le vert.
    #[test]
    fn un_critique_sur_zero_reste_critique() {
        let (s, _) = critique("le monde physique est vide")
            .remede("RapierPhysicsPlugin non initialise")
            .echantillon(0)
            .verdict();
        assert_eq!(s, Severite::Critical);
    }

    #[test]
    fn l_echappement_protege_les_chemins_windows_et_les_guillemets() {
        let e = echappe(r#"C:\Users\x "y" \n"#);
        assert!(!e.contains(r#"" y"#));
        assert!(e.contains(r"C:\\Users\\x"));
    }
}
