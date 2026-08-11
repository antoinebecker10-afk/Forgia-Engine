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
    pub use crate::fps_feel::FpsFeelMetrics;
    pub use crate::hud_visibility::{gameplay_hud_visible, GameplayHudVisible};
    pub use crate::states::{AppMode, GameMode, WorldMode};
    pub use crate::system_set::GameSet;
    pub use crate::ForgiaCorePlugin;
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
        // wasm32 (story-695) : pas de threads ni de fs — le writer ne peut pas
        // exister. No-op silencieux ; le sink web (inc.2) prendra le relais.
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (path.into(), contents.into());
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
            let _ = path.into();
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
