//! # forgia-core
//!
//! Types core de Forgia V2 : States (AppMode, GameMode, WorldMode), GameSet ordering,
//! Resources globales (RespawnPoint, ActiveMap).
//!
//! **Règle inviolable** : ce crate ne dépend de RIEN dans le workspace.
//! Pattern DAG-libre hérité de forgia-terrain V1.

use bevy::prelude::*;

/// Story-674 — primitives d'aménagement partagées (bruit bleu, compte d'abris).
pub mod layout;
pub mod sectors;

/// Canonical filesystem resolution for shipped and development assets.
pub mod asset_paths {
    use std::path::{Path, PathBuf};

    const ASSET_ROOT_ENV: &str = "FORGIA_ASSET_ROOT";

    fn root_from(start: &Path) -> Option<PathBuf> {
        start
            .ancestors()
            .map(|base| base.join("assets"))
            .find(|path| path.is_dir())
    }

    /// Finds the real asset directory independently from the process working directory.
    pub fn asset_root() -> PathBuf {
        if let Some(path) = std::env::var_os(ASSET_ROOT_ENV).map(PathBuf::from) {
            if path.is_dir() {
                return path;
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(path) = root_from(&cwd) {
                return path;
            }
        }
        if let Ok(executable) = std::env::current_exe() {
            if let Some(path) = executable.parent().and_then(root_from) {
                return path;
            }
        }
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("assets")
    }

    /// Resolves a path relative to `assets/`. An optional leading `assets/` is accepted.
    pub fn asset_path(relative: impl AsRef<Path>) -> PathBuf {
        let relative = relative.as_ref();
        asset_root().join(relative.strip_prefix("assets").unwrap_or(relative))
    }

    /// Root containing `assets/`, used by legacy filesystem readers that still
    /// resolve `assets/...` and `config/...` from the process working directory.
    pub fn runtime_root() -> PathBuf {
        asset_root()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn workspace_assets_are_found_from_the_core_crate() {
            assert!(asset_root().join("genomes").is_dir());
        }

        #[test]
        fn optional_assets_prefix_is_not_duplicated() {
            assert_eq!(asset_path("genomes"), asset_path("assets/genomes"));
        }

        #[test]
        fn runtime_root_contains_the_asset_directory() {
            assert_eq!(runtime_root().join("assets"), asset_root());
        }
    }
}

pub mod prelude {
    pub use crate::cosmetics::{ArmCosmetics, ArmStyle, UiStudioCamera, ViewmodelForcedVisible};
    pub use crate::faction::Faction;
    pub use crate::fps_feel::FpsFeelMetrics;
    pub use crate::hud_visibility::{gameplay_hud_visible, GameplayHudVisible};
    pub use crate::states::{AppMode, GameMode, WorldMode};
    pub use crate::system_set::GameSet;
    pub use crate::ForgiaCorePlugin;
}

/// À quel camp appartient une entité.
///
/// # Pourquoi ça vit ici, et pourquoi maintenant
///
/// Un codebase qui suppose « le joueur contre les ennemis » ne se rétrofite pas en duo ni
/// en 5v5 : la supposition est partout, implicite, et chaque site la ré-encode à sa façon.
/// Poser le concept tôt ne coûte presque rien ; le poser après coup est une refonte
/// transverse. C'est l'une des **quatre portes à ne pas fermer** du GDD §10.
///
/// `forgia-core` est le bon toit : zéro dépendance workspace, donc n'importe quelle crate
/// peut en parler sans créer de cycle.
///
/// # Ce que ce type ne décide PAS
///
/// Il nomme les camps, il n'arbitre rien : ni les dégâts, ni le ciblage, ni l'aggro. Ces
/// règles appartiennent aux systèmes qui les appliquent, et les y laisser évite qu'une
/// table centrale devienne le passage obligé de tout le gameplay.
pub mod faction {
    use bevy::prelude::*;

    #[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub enum Faction {
        /// Le joueur humain.
        #[default]
        Player,
        /// Alliés du joueur — compagnon-bot aujourd'hui, coéquipier humain demain (E9),
        /// coéquipiers d'une équipe 5v5 plus tard (E10). Le même camp, trois sources.
        Allied,
        /// Ce qui attaque le joueur.
        Hostile,
        /// Ni allié ni hostile : PNJ, faune, décor animé.
        Neutral,
    }

    impl Faction {
        /// Deux camps sont-ils du même bord ?
        ///
        /// `Player` et `Allied` sont distincts — un compagnon n'est pas le joueur, et
        /// certaines règles (le loot personnel, la caméra, les combos élémentaires du
        /// GDD §4) doivent pouvoir les séparer — mais ils sont **du même bord**.
        #[must_use]
        pub fn is_friendly_to(self, other: Self) -> bool {
            use Faction::{Allied, Hostile, Neutral, Player};
            match (self, other) {
                (Neutral, _) | (_, Neutral) => false,
                (Player | Allied, Player | Allied) => true,
                (Hostile, Hostile) => true,
                _ => false,
            }
        }

        /// Doivent-ils se prendre pour cible ?
        ///
        /// Neutre n'est hostile à personne : c'est ce qui distingue « pas mon allié » de
        /// « ma cible ». Confondre les deux ferait tirer les bots sur la faune.
        #[must_use]
        pub fn is_hostile_to(self, other: Self) -> bool {
            use Faction::Neutral;
            self != Neutral && other != Neutral && !self.is_friendly_to(other)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::Faction;
        use super::Faction::{Allied, Hostile, Neutral, Player};

        #[test]
        fn le_joueur_et_son_compagnon_sont_du_meme_bord() {
            assert!(Player.is_friendly_to(Allied));
            assert!(Allied.is_friendly_to(Player));
            assert!(!Player.is_hostile_to(Allied));
        }

        #[test]
        fn les_hostiles_ne_se_tirent_pas_dessus() {
            assert!(Hostile.is_friendly_to(Hostile));
            assert!(!Hostile.is_hostile_to(Hostile));
        }

        #[test]
        fn le_joueur_et_les_hostiles_se_ciblent() {
            assert!(Hostile.is_hostile_to(Player));
            assert!(Player.is_hostile_to(Hostile));
            assert!(Hostile.is_hostile_to(Allied));
        }

        #[test]
        fn neutre_n_est_l_allie_ni_la_cible_de_personne() {
            // La distinction qui compte : « pas mon allié » n'est PAS « ma cible ».
            // Les confondre ferait tirer les bots sur la faune.
            for autre in [Player, Allied, Hostile, Neutral] {
                assert!(!Neutral.is_friendly_to(autre), "{autre:?}");
                assert!(!Neutral.is_hostile_to(autre), "{autre:?}");
                assert!(!autre.is_hostile_to(Neutral), "{autre:?}");
            }
        }

        #[test]
        fn le_defaut_est_le_joueur() {
            // Un composant oublie doit produire l'entite la plus inoffensive du jeu,
            // pas un hostile qui attaquerait tout le monde.
            assert_eq!(Faction::default(), Player);
        }
    }
}

/// Écriture asynchrone et bornée des capteurs de diagnostic.
///
/// Les capteurs ne doivent jamais bloquer le thread de jeu sur une écriture de
/// fichier. Les sauvegardes métier restent volontairement hors de ce module :
/// elles ont leurs propres garanties de durabilité et d'atomicité.
pub mod sensor_io {
    use std::fmt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    #[cfg(not(target_arch = "wasm32"))]
    use std::sync::mpsc::{self, SyncSender};
    #[cfg(not(target_arch = "wasm32"))]
    use std::sync::OnceLock;

    #[cfg(not(target_arch = "wasm32"))]
    const QUEUE_CAPACITY: usize = 256;

    #[cfg(not(target_arch = "wasm32"))]
    enum SensorJob {
        Write { path: PathBuf, contents: String },
        Remove { path: PathBuf },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum EnqueueError {
        Full,
        Disconnected,
    }

    impl fmt::Display for EnqueueError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Full => f.write_str("file de capteurs pleine"),
                Self::Disconnected => f.write_str("writer de capteurs arrêté"),
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    static SENSOR_WRITER: OnceLock<SyncSender<SensorJob>> = OnceLock::new();
    static ENQUEUED: AtomicU64 = AtomicU64::new(0);
    static PROCESSED: AtomicU64 = AtomicU64::new(0);
    static DROPPED_FULL: AtomicU64 = AtomicU64::new(0);
    static DISCONNECTED: AtomicU64 = AtomicU64::new(0);
    static WRITE_FAILURES: AtomicU64 = AtomicU64::new(0);

    /// Instantané de santé du writer asynchrone des capteurs.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct SensorIoStats {
        pub enqueued: u64,
        pub processed: u64,
        pub pending: u64,
        pub dropped_full: u64,
        pub disconnected: u64,
        pub write_failures: u64,
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn sender() -> &'static SyncSender<SensorJob> {
        SENSOR_WRITER.get_or_init(|| {
            let (tx, rx) = mpsc::sync_channel::<SensorJob>(QUEUE_CAPACITY);
            std::thread::Builder::new()
                .name("forgia-sensor-io".to_string())
                .spawn(move || {
                    while let Ok(job) = rx.recv() {
                        // Best effort : un capteur ne doit jamais tuer le writer.
                        match job {
                            SensorJob::Write { path, contents } => {
                                if std::fs::write(path, contents).is_err() {
                                    WRITE_FAILURES.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            SensorJob::Remove { path } => {
                                if let Err(error) = std::fs::remove_file(path) {
                                    // L'absence est le résultat nominal de la
                                    // convention « health file absent = OK ».
                                    if error.kind() != std::io::ErrorKind::NotFound {
                                        WRITE_FAILURES.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                        PROCESSED.fetch_add(1, Ordering::Relaxed);
                    }
                })
                .expect("le thread writer des capteurs doit démarrer");
            tx
        })
    }

    /// Place une mise à jour de capteur en file sans bloquer la frame.
    ///
    /// Si la file est pleine, on préfère perdre l'échantillon le plus récent à
    /// bloquer le jeu. Le prochain heartbeat remplacera naturellement le JSON.
    pub fn enqueue(
        path: impl Into<PathBuf>,
        contents: impl Into<String>,
    ) -> Result<(), EnqueueError> {
        // wasm32 (story-695 inc.2) : pas de threads ni de fs — chaque capteur
        // garde son DERNIER etat en memoire, exporte vers JS par
        // `forgia_dump_sensors()` (bouton diagnostic de la page web).
        #[cfg(target_arch = "wasm32")]
        {
            let path: PathBuf = path.into();
            if let Ok(mut map) = crate::web_sensor_sink::SENSORS.lock() {
                map.insert(path.to_string_lossy().into_owned(), contents.into());
            }
            return Ok(());
        }
        #[cfg(not(target_arch = "wasm32"))]
        match sender().try_send(SensorJob::Write {
            path: path.into(),
            contents: contents.into(),
        }) {
            Ok(()) => {
                ENQUEUED.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(mpsc::TrySendError::Full(_)) => {
                DROPPED_FULL.fetch_add(1, Ordering::Relaxed);
                Err(EnqueueError::Full)
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                DISCONNECTED.fetch_add(1, Ordering::Relaxed);
                Err(EnqueueError::Disconnected)
            }
        }
    }

    /// Programme la suppression d'un fichier de santé sans bloquer la frame.
    pub fn remove(path: impl Into<PathBuf>) -> Result<(), EnqueueError> {
        #[cfg(target_arch = "wasm32")]
        {
            let path: PathBuf = path.into();
            if let Ok(mut map) = crate::web_sensor_sink::SENSORS.lock() {
                map.remove(&path.to_string_lossy().into_owned());
            }
            return Ok(());
        }
        #[cfg(not(target_arch = "wasm32"))]
        match sender().try_send(SensorJob::Remove { path: path.into() }) {
            Ok(()) => {
                ENQUEUED.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(mpsc::TrySendError::Full(_)) => {
                DROPPED_FULL.fetch_add(1, Ordering::Relaxed);
                Err(EnqueueError::Full)
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                DISCONNECTED.fetch_add(1, Ordering::Relaxed);
                Err(EnqueueError::Disconnected)
            }
        }
    }

    /// Lecture non bloquante des compteurs du writer. `pending` est une
    /// estimation (atomiques Relaxed) suffisante pour signaler une saturation.
    pub fn stats() -> SensorIoStats {
        let enqueued = ENQUEUED.load(Ordering::Relaxed);
        let processed = PROCESSED.load(Ordering::Relaxed);
        SensorIoStats {
            enqueued,
            processed,
            pending: enqueued.saturating_sub(processed),
            dropped_full: DROPPED_FULL.load(Ordering::Relaxed),
            disconnected: DISCONNECTED.load(Ordering::Relaxed),
            write_failures: WRITE_FAILURES.load(Ordering::Relaxed),
        }
    }
}

/// story-699 — la sévérité d'un capteur de feature doit regarder l'INACTIVITÉ,
/// pas seulement l'erreur.
///
/// ## Le défaut que ce module corrige
///
/// Le 2026-08-12, trois capteurs rapportaient `severity: "ok"` alors que leur
/// sujet était mort :
///
/// | Capteur | severity | contenu réel |
/// |---|---|---|
/// | `forgia2_gamefeel` | ok | `hitstop_counts` tous à 0, sur 51 kills |
/// | `forgia2_weapon_vfx` | ok | `kill_bursts: 0` |
/// | `forgia2_elements` | ok | `combustions/miasmas/surcharges` tous à 0 |
///
/// **Le capteur disait « ok » parce que rien n'avait échoué. Or rien ne s'était
/// produit non plus.** Un système inerte ne lève aucune erreur : c'est ce qui le
/// rend invisible. Pire, ces trois `ok` ont failli faire fermer automatiquement
/// trois stories prouvées cassées le matin même.
///
/// `map-design-patterns.md` §13 énonçait déjà la règle pour la géométrie —
/// « zéro mesuré n'est pas vert, c'est **aveugle** » — mais elle n'avait jamais
/// été appliquée aux capteurs de feature.
///
/// ## Le piège symétrique, que ce module évite
///
/// Passer tout compteur à zéro en `warn` serait pire : au menu, un compteur de
/// combat à zéro est parfaitement normal, et un chien qui crie au loup finit
/// ignoré. **La sévérité doit donc regarder le CONTEXTE d'attente**, pas la seule
/// valeur du compteur.
pub mod sensor_activity {
    /// Ce qu'on peut honnêtement dire d'un compteur de feature.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Activity {
        /// Le système a produit — il fonctionne.
        Ok,
        /// Rien n'était attendu (mauvais mode, hors combat, système coupé).
        /// **Ce n'est pas un feu vert** : c'est l'absence d'information.
        Blind,
        /// Attendu, actif depuis assez longtemps, et **toujours rien produit**.
        /// C'est un défaut.
        Inert,
    }

    impl Activity {
        /// Sévérité au format capteur Forgia.
        pub fn severity(self) -> &'static str {
            match self {
                Activity::Ok => "ok",
                Activity::Blind => "info",
                Activity::Inert => "warn",
            }
        }
    }

    /// Pur — juge un compteur de feature.
    ///
    /// - `context_active` : le système est-il **censé** produire en ce moment ?
    ///   (en combat pour un compteur de combat, en arène pour un compteur d'arène…)
    /// - `count` : combien il a produit depuis le début de la session.
    /// - `active_secs` : depuis combien de temps le contexte est actif.
    /// - `grace_secs` : délai laissé au système pour démarrer avant de le juger.
    ///
    /// Le délai de grâce n'est pas du confort : sans lui, la première seconde
    /// d'un combat déclencherait une alerte à chaque round.
    pub fn judge(context_active: bool, count: u64, active_secs: f32, grace_secs: f32) -> Activity {
        if !context_active {
            return Activity::Blind;
        }
        if count > 0 {
            return Activity::Ok;
        }
        if active_secs < grace_secs {
            return Activity::Ok;
        }
        Activity::Inert
    }

    /// Délai de grâce par défaut : un système de combat qui n'a rien produit
    /// après **15 s de combat effectif** ne démarre pas — il est mort.
    pub const DEFAULT_GRACE_SECS: f32 = 15.0;

    #[cfg(test)]
    mod tests {
        use super::*;

        /// LE cas du 2026-08-12 : 51 kills, `kill_bursts: 0`, capteur qui dit « ok ».
        #[test]
        fn un_compteur_a_zero_en_plein_contexte_est_un_defaut() {
            assert_eq!(judge(true, 0, 60.0, 15.0), Activity::Inert);
            assert_eq!(judge(true, 0, 60.0, 15.0).severity(), "warn");
        }

        /// Le piège symétrique : au menu, zéro ne veut RIEN dire. Ni vert ni rouge.
        #[test]
        fn hors_contexte_le_capteur_est_aveugle_pas_vert() {
            assert_eq!(judge(false, 0, 999.0, 15.0), Activity::Blind);
            assert_eq!(
                judge(false, 0, 999.0, 15.0).severity(),
                "info",
                "un capteur sans rien a mesurer ne doit pas se declarer OK — \
                 c'est exactement le mensonge que ce module corrige"
            );
        }

        #[test]
        fn le_delai_de_grace_evite_de_crier_au_demarrage() {
            // 2 s de combat, rien encore produit : normal.
            assert_eq!(judge(true, 0, 2.0, 15.0), Activity::Ok);
            // 16 s plus tard, toujours rien : anormal.
            assert_eq!(judge(true, 0, 16.0, 15.0), Activity::Inert);
        }

        #[test]
        fn produire_une_seule_fois_suffit_a_prouver_que_ca_marche() {
            assert_eq!(judge(true, 1, 999.0, 15.0), Activity::Ok);
        }

        /// Le contexte prime sur tout : meme inerte depuis longtemps, hors
        /// contexte on se tait. Sinon chaque retour au menu leverait une alerte.
        #[test]
        fn le_contexte_prime_sur_l_anciennete() {
            assert_eq!(judge(false, 0, 10_000.0, 15.0), Activity::Blind);
        }
    }
}

pub mod hud_visibility {
    use bevy::prelude::*;

    /// Visibilité du HUD de GAMEPLAY (ammo, PV, énergie, confiance, viewmodel…).
    /// `false` = écran-menu in-game (ex. Lobby Roguelite) → masquer tout le HUD de
    /// combat pour ne garder que l'UI de menu. `true` partout ailleurs.
    ///
    /// Vit dans forgia-core (DAG-libre) pour que les crates HUD partagées
    /// (forgia-ui-lib, forgia-viewmodel) la lisent SANS dépendre du crate de mode
    /// (forgia-mode-roguelite) qui la pilote — évite un cycle de dépendances.
    #[derive(Resource, Debug, Clone, Copy)]
    pub struct GameplayHudVisible(pub bool);

    impl Default for GameplayHudVisible {
        fn default() -> Self {
            Self(true)
        }
    }

    /// Run-condition : le HUD de gameplay doit-il s'afficher ? (défaut `true` si la
    /// resource n'est pas encore initialisée — ordre d'init safe).
    pub fn gameplay_hud_visible(v: Option<Res<GameplayHudVisible>>) -> bool {
        v.map(|r| r.0).unwrap_or(true)
    }
}

pub mod cosmetics {
    use bevy::prelude::*;

    /// Style esthétique des bras procéduraux (variation de matériau). Choisi dans
    /// l'onglet Forge. `key()`/`from_key()` = (dé)sérialisation TOML/save.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum ArmStyle {
        /// Mat doux (peau).
        #[default]
        Peau,
        /// Métal (gantelet de forge).
        Gantelet,
        /// Émissif (gant cyber).
        Cyber,
    }

    impl ArmStyle {
        pub fn from_key(s: &str) -> Self {
            match s {
                "gantelet" => ArmStyle::Gantelet,
                "cyber" => ArmStyle::Cyber,
                _ => ArmStyle::Peau,
            }
        }
        pub fn key(self) -> &'static str {
            match self {
                ArmStyle::Peau => "peau",
                ArmStyle::Gantelet => "gantelet",
                ArmStyle::Cyber => "cyber",
            }
        }
        pub fn label(self) -> &'static str {
            match self {
                ArmStyle::Peau => "Peau",
                ArmStyle::Gantelet => "Gantelet",
                ArmStyle::Cyber => "Cyber",
            }
        }
    }

    /// Marqueur des caméras d'aperçu UI en « studio » (RTT : arme, personnage).
    ///
    /// Story-678, audit 2026-08-06. Le grading d'univers
    /// (`color_grading::sys_apply`) s'applique à TOUTES les `Camera3d` — les
    /// caméras d'aperçu comprises, dont le fond sombre ressortait teinté du
    /// rose de l'ambiance. Ce marqueur les en exclut. Le fond d'arène du menu,
    /// lui, n'est PAS marqué : il montre l'univers, il doit en garder le look.
    ///
    /// Vit ici pour la même raison qu'`ArmCosmetics` : les caméras sont
    /// spawnées par `forgia-ui`, le grading par `forgia-game` — forgia-core est
    /// leur seul ancêtre commun sans cycle.
    #[derive(Component, Debug, Clone, Copy, Default)]
    pub struct UiStudioCamera;

    /// Cosmétique des bras procéduraux (couleur + style). Pilotée par l'onglet Forge
    /// (forgia-mode-roguelite), appliquée au matériau par forgia-viewmodel — via
    /// forgia-core (DAG-libre) pour éviter un cycle de dépendances.
    #[derive(Resource, Debug, Clone, Copy, PartialEq)]
    pub struct ArmCosmetics {
        /// Teinte (sRGB) appliquée à la peau / à la combinaison.
        pub color: [f32; 3],
        pub style: ArmStyle,
        /// Teinte des PLAQUES d'armure — la rareté des gants équipés.
        ///
        /// Deux couches distinctes plutôt qu'une : la rareté doit se lire sur
        /// l'armure sans effacer la couleur d'identité choisie au Forgeron. Le
        /// personnage porte les deux, comme sur l'asset qui les sépare déjà en
        /// deux jeux de matériaux. Blanc = aucune pièce équipée, donc aucune
        /// teinte (le blanc est neutre : `base_color` multiplie l'albédo).
        pub armor_rgb: [f32; 3],
    }
    impl Default for ArmCosmetics {
        fn default() -> Self {
            Self {
                color: [0.93, 0.73, 0.57],
                style: ArmStyle::Peau,
                armor_rgb: [1.0, 1.0, 1.0],
            }
        }
    }

    /// Force l'affichage du viewmodel (bras) hors gameplay — ex. APERÇU dans l'onglet
    /// Forge du hub. `false` = le viewmodel suit `GameplayHudVisible` normalement.
    #[derive(Resource, Debug, Clone, Copy, Default)]
    pub struct ViewmodelForcedVisible(pub bool);
}

pub mod fps_feel {
    use bevy::prelude::*;

    /// Resource counters FPS feel — Story-528 phase 1.
    /// Producteurs : forgia-player (dash), forgia-effects (hit feedbacks),
    /// forgia-fps (aim assist). Lecteur : forgia-observability fps_feel_sensor.
    /// Placée ici (foundation DAG-libre) pour éviter cycle observability ↔ player.
    #[derive(Resource, Default)]
    pub struct FpsFeelMetrics {
        pub dash_uses_total: u64,
        pub hit_feedbacks_total: u64,
        pub aim_assist_engagements_total: u64,
    }
}

pub mod states {
    use bevy::prelude::*;

    /// AppMode — gate l'UI et le flow joueur.
    #[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
    pub enum AppMode {
        #[default]
        Boot,
        Menu,
        InGame,
        Paused,
    }

    /// GameMode — joueur a choisi FPS ou RPG depuis le menu.
    /// Gate les plugins forgia-fps vs forgia-rpg dynamiquement.
    #[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
    pub enum GameMode {
        #[default]
        None,
        Fps,
        Rpg,
        // Story-470 V7 M1 — 3e jeu Forgia : roguelite FPS coop 1-3j (cible Next Fest)
        Roguelite,
        /// Démo perf moteur (2026-06-15) — charge un GLB lourd (cyberpunk city)
        /// avec flycam libre pour stress-tester rendu/VRAM. Pas de gameplay.
        /// Géré par `forgia_game::cyber_city::CyberCityDemoPlugin`.
        CyberCity,
        /// Hall de Forgia (2026-07-22) — hub social 3D walkable : château importé
        /// (Unity FANTASTIC Highlands Castle). Zone NEUTRE sans combat, point de
        /// rassemblement (multijoueur à terme). Géré par
        /// `forgia_game::castle_hub::CastleHubPlugin`.
        CastleHub,
        /// Arena Test (2026-07-27) — banc de blockout d'arène, isolé du Roguelite
        /// pour ne rien casser de ce qui tourne. Géométrie grise pilotée par
        /// `assets/genomes/arena_test.toml`, grille au sol à l'échelle des metrics
        /// joueur mesurées, tir autorisé. C'est l'étape « greybox » du process de
        /// level design : on joue la forme avant de l'habiller.
        /// Géré par `forgia_game::arena_test::ArenaTestPlugin`.
        ArenaTest,
        /// Expédition (2026-08-14) — une carte **autorée sous Blender**, chargée
        /// depuis ses deux manifestes (cellules glTF + gameplay) au lieu d'être
        /// générée. Première carte : « Le Vallon », 280 × 200 m, 3 campements
        /// jalonnant un chemin de 358,7 m.
        ///
        /// C'est le mode E2 du GDD, celui que story-704 garde verrouillé au menu
        /// tant qu'il n'existe pas. Géré par
        /// `forgia_mode_expedition::ForgiaExpeditionPlugin`.
        Expedition,
    }

    /// WorldMode — gate la simulation (Editor désactive AI/physics).
    /// Pattern Mass Entity dual phase manager (Epic).
    #[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
    pub enum WorldMode {
        #[default]
        Game,
        Editor,
        Test,
    }
}

pub mod system_set {
    use bevy::prelude::*;

    /// GameSet — chaîne ordering canonique Forgia V2.
    /// Hérité V1 chaîne 7 étapes + ajouts `Network` (entre Input et Movement)
    /// et `Sensors` (entre Effects et UI). Lock L7 reborn.
    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    pub enum GameSet {
        Network,
        Input,
        Movement,
        Physics,
        Camera,
        Combat,
        Effects,
        Sensors,
        UI,
    }
}

/// Plugin agrégateur — init States + configure GameSet ordering.
pub struct ForgiaCorePlugin;

impl Plugin for ForgiaCorePlugin {
    fn build(&self, app: &mut App) {
        use system_set::GameSet;
        // ⚠️ ForgiaCorePlugin DOIT être ajouté APRÈS DefaultPlugins (Bevy 0.18 quirk).
        // DefaultPlugins inclut StatesPlugin qui fournit StateTransition schedule.
        // init_state panique si StateTransition pas encore enregistré.
        // Voir crates/forgia-game/src/main.rs ordre plugins.
        app.init_state::<states::AppMode>()
            .init_state::<states::GameMode>()
            .init_state::<states::WorldMode>()
            .init_resource::<fps_feel::FpsFeelMetrics>()
            .init_resource::<hud_visibility::GameplayHudVisible>()
            .init_resource::<cosmetics::ArmCosmetics>()
            .init_resource::<cosmetics::ViewmodelForcedVisible>()
            .configure_sets(
                Update,
                (
                    GameSet::Network,
                    GameSet::Input,
                    GameSet::Movement,
                    GameSet::Physics,
                    GameSet::Camera,
                    GameSet::Combat,
                    GameSet::Effects,
                    GameSet::Sensors,
                    GameSet::UI,
                )
                    .chain(),
            )
            // Keystone 0.1a-1 (sim déterministe) — MÊME chaîne ordonnée aussi en
            // `FixedUpdate`, prérequis bloquant avant de migrer le moindre système
            // (sans ça l'ordre en FixedUpdate serait indéfini, cf spike R1).
            // Déclaration PURE : aucun système n'est encore `.in_set` sur FixedUpdate
            // (migration des ~35 systèmes en 0.1a-2) → zéro effet runtime ici.
            // Hz : on garde le défaut Bevy `Time<Fixed>` = 64 Hz, qui EST déjà le
            // timestep de Rapier (FixedUpdate) → aucun changement de feel physique.
            .configure_sets(
                FixedUpdate,
                (
                    GameSet::Network,
                    GameSet::Input,
                    GameSet::Movement,
                    GameSet::Physics,
                    GameSet::Camera,
                    GameSet::Combat,
                    GameSet::Effects,
                    GameSet::Sensors,
                    GameSet::UI,
                )
                    .chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_default_correct() {
        assert_eq!(states::AppMode::default(), states::AppMode::Boot);
        assert_eq!(states::GameMode::default(), states::GameMode::None);
        assert_eq!(states::WorldMode::default(), states::WorldMode::Game);
    }

    #[test]
    fn plugin_builds_without_panic() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.add_plugins(ForgiaCorePlugin);
    }
}

// ─── Sink web des capteurs (story-695 inc.2) ─────────────────────────────────

/// Sur wasm, `sensor_io` range le dernier etat de chaque capteur ici : le
/// « regarde » des testeurs web. Exporte vers JS par [`forgia_dump_sensors`].
#[cfg(target_arch = "wasm32")]
pub(crate) mod web_sensor_sink {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    pub(crate) static SENSORS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
}

/// Export JS : tous les capteurs en un objet JSON `{ "forgia2_x.json": "<contenu>" }`.
/// Les contenus sont embarques en CHAINES echappees : un capteur au JSON invalide
/// ne peut pas corrompre le dump entier. Appele par le bouton diagnostic de la page.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn forgia_dump_sensors() -> String {
    fn escape_into(out: &mut String, s: &str) {
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
    }
    let Ok(map) = web_sensor_sink::SENSORS.lock() else {
        return "{}".to_string();
    };
    let mut out = String::with_capacity(map.len() * 256);
    out.push('{');
    for (i, (key, value)) in map.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        escape_into(&mut out, key);
        out.push_str("\":\"");
        escape_into(&mut out, value);
        out.push('"');
    }
    out.push('}');
    out
}

/// Story-695 inc.3 : acces localStorage — la persistance des saves sur web.
/// Cle par fichier de save (`forgia_save:<nom>`), valeurs TOML telles quelles.
#[cfg(target_arch = "wasm32")]
pub mod web_storage {
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    pub fn get(key: &str) -> Option<String> {
        storage()?.get_item(key).ok().flatten()
    }

    pub fn set(key: &str, value: &str) -> bool {
        storage()
            .map(|s| s.set_item(key, value).is_ok())
            .unwrap_or(false)
    }

    pub fn remove(key: &str) {
        if let Some(s) = storage() {
            let _ = s.remove_item(key);
        }
    }
}

/// Lecture des fichiers de DÉFINITION (genomes TOML, registres RON) — story-695.
///
/// Natif : filesystem, source de vérité, hot-reload possible. wasm : pack
/// embarqué à la compilation par `build.rs` (le web n'a pas de fs ; chaque
/// `std::fs::read_to_string` y échoue en « operation not supported » et le
/// système tombe en défauts EN SILENCE — équipement désactivé, avatar absent,
/// 0 cluster champignons, constaté 2026-08-11). Le pack suit le versioning de
/// publication : un build web fige les définitions du commit qui l'a produit.
pub mod def_io {
    #[cfg(target_arch = "wasm32")]
    mod pack {
        include!(concat!(env!("OUT_DIR"), "/web_defs.rs"));
    }

    /// Clé de pack : portion du chemin depuis `genomes/` ou `registry/`,
    /// séparateurs normalisés. Les call sites écrivent leurs chemins de formes
    /// variées (`assets/genomes/...`, chemin absolu joint, backslashes Windows).
    #[cfg(any(target_arch = "wasm32", test))]
    fn pack_key(path: &str) -> Option<String> {
        let norm = path.replace('\\', "/");
        ["genomes/", "registry/"]
            .iter()
            .find_map(|root| norm.find(root).map(|i| norm[i..].to_string()))
    }

    /// Équivalent `std::fs::read_to_string` pour un fichier de définition.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read_def_str(path: &str) -> std::io::Result<String> {
        std::fs::read_to_string(path)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn read_def_str(path: &str) -> std::io::Result<String> {
        if let Some(key) = pack_key(path) {
            if let Some((_, contents)) = pack::WEB_DEFS.iter().find(|(k, _)| *k == key) {
                return Ok((*contents).to_string());
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("définition absente du pack web embarqué: {path}"),
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::pack_key;

        #[test]
        fn pack_key_normalise_les_formes_de_chemins() {
            assert_eq!(
                pack_key("assets/genomes/roguelite/roguelite_equipment.toml").as_deref(),
                Some("genomes/roguelite/roguelite_equipment.toml")
            );
            assert_eq!(
                pack_key(r"C:\jeu\assets\genomes\arena_waves.toml").as_deref(),
                Some("genomes/arena_waves.toml")
            );
            assert_eq!(
                pack_key("assets/registry/asset_meta.ron").as_deref(),
                Some("registry/asset_meta.ron")
            );
            assert_eq!(pack_key("assets/models/foo.glb"), None);
        }
    }
}
